//! The syscall boundary. **Four calls <!--count:syscalls-->.**
//!
//! DECISIONS §4 rule 3 said the syscall surface stays narrow and explicit, *"a boundary, not a
//! habit."* §8 said milestone 7 was a hard decision point and that hacking one in without the
//! conversation meant the plan had failed. §10 had the conversation and chose capabilities.
//!
//! This is what that buys:
//!
//! ```text
//!   exit(code)                          authority over yourself
//!   yield()                             likewise
//!   cap_delete(slot)                    likewise: your own capability table is your own
//!   invoke(cap, method, a0, a1, a2)     EVERYTHING ELSE
//! ```
//!
//! Three of the four are authority over yourself and the fourth is everything else, which is the
//! property worth remembering rather than the count. This header said "three calls" from
//! 2026-07-14 until the 2026-08-17 documentation sweep, `abi::SYS_CAP_DELETE` having arrived on
//! 2026-07-24 without it; the count is now a re-derived `<!--count:syscalls-->` claim.
//!
//! No `open`. No `read`. No `write`. No `fork`. **A process can only act on things it was
//! handed.** The ABI lives in `crates/abi`, which both the kernel and every user program depend
//! on, so the boundary is *one artifact* rather than two files that agree by luck.
//!
//! # No pointer ever crosses this boundary
//!
//! There used to be a `user_slice` here: the console `write` syscall took a `(ptr, len)` from
//! userspace and the kernel read the user's memory, which is why it needed the `AT S1E0R`
//! confused-deputy defence. Milestone 8 moved the console to a userspace server and deleted that
//! path. Today every argument is a scalar in a register (a capability slot, a method, a `va`, a
//! word), so the kernel follows no user pointer and there is no deputy to confuse. The primitive
//! that made the old check possible, `mmu::user_can_read`, is kept for the next syscall that does
//! take a user pointer.

use abi::Error;

use crate::arch::exceptions::TrapFrame;
use crate::arch::mmu;
use crate::cap::{Object, Rights};
use crate::sched;

// **The `ThreadControlBlock` methods, in a file of their own** because this one reached §266 (a Rust
// source file stays under 2,000 lines)'s ceiling when milestone 812 (`std::thread::spawn` runs
// real threads in one address space) added `SET_THREAD_POINTER`. They were already one
// out-of-line seam; [`invoke`] reaches them through one call.
mod thread_control_block;
use thread_control_block::thread_control_block_invoke;

/// Called from the `svc` arm of `exception_body` (`ecall` on riscv64, in `riscv_trap_body`). The
/// *body*, not the dispatcher, since milestone 124 split the two: a syscall arrives from user mode,
/// which is the case that deliberately does NOT move to the interrupt stack, because this path can
/// block and a blocked thread's frames must live on its own stack.
///
/// **`#[inline(never)]` keeps this a symbol `script/fastpath-footprint` can name**, which is the
/// third instance of a pattern the tree has now recorded twice
/// (`design/roadmap/0368-a-flat-entry-set-counts-bytes-no-syscall-fetches.md`): that gate's
/// `syscall_entry` half sums a flat list of symbols, so an LLVM inlining flip moves bytes into or
/// out of the measurement without anything on the syscall path changing. Milestone 220's lane hit
/// the *outward* direction, which is the one that reads as good news and is not: adding an
/// unrelated dependency to this crate made LLVM fold these 1160 bytes into the aarch64 exception
/// handler, and the gate reported `syscall_entry` shrinking 35% while the code a syscall fetches
/// was identical. Re-recording the baseline there would have locked in an under-measurement.
///
/// It also stands on its own, which is the test the tree asks of every `#[inline(never)]` it
/// carries (`timer::tick`, `plic::disable`, `sched::grant_cycle_counter`, milestone 156's spawn
/// bodies): this is a large match executed once per syscall, and one `bl` is not a cost worth
/// duplicating a kilobyte of dispatcher into a handler that also serves faults and interrupts.
#[inline(never)]
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(
    target_os = "none",
    unsafe(link_section = ".text.hot.syscall.dispatch")
)]
pub fn dispatch(frame: &mut TrapFrame) {
    // The syscall number and arguments come from the trap frame through arch accessors, not raw
    // register indices, because the ABI register file differs per architecture (aarch64 `svc` with
    // the number in x8 and args in x0..x5; RISC-V `ecall` with the number in a7 and args in a0..a5).
    // `TrapFrame::{syscall_nr, arg, set_arg}` hide that mapping so this dispatcher stays portable.
    // See DECISIONS §10/§17.
    let nr = frame.syscall_nr();
    // Milestone 134's per-IPC stack-depth instrument keys each sample by method, and the method
    // register may be overwritten by a result before the end of this function. Not in any build
    // that measures time or footprint: see `crate::ipc_stack_depth`.
    #[cfg(any(test, feature = "ipc_stack_depth"))]
    let method = frame.arg(1);

    // `exit` never comes back, so it is not part of the result-writing path below.
    if nr == abi::SYS_EXIT {
        sched::exit();
    }

    let result: Result<i64, Error> = match nr {
        abi::SYS_YIELD => {
            sched::yield_now();
            Ok(0)
        }
        // Drop a capability from the caller's own capability table (milestone 19d). Deleting an empty slot
        // is a no-op, not an error: a loader recycling slots should not have to track emptiness.
        abi::SYS_CAP_DELETE => {
            let _ = sched::delete_current_cap(frame.arg(0));
            Ok(0)
        }
        abi::SYS_INVOKE => invoke(
            frame,
            frame.arg(0),
            frame.arg(1),
            frame.arg(2),
            frame.arg(3),
            frame.arg(4),
        ),
        _ => Err(Error::BadSyscall),
    };

    // The return value goes back in the first argument register, which the trap-restore path pops
    // into the register the user is waiting on. Writing to the trap frame IS writing to the user's
    // registers.
    frame.set_arg(
        0,
        match result {
            Ok(v) => v as u64,
            Err(e) => (e as i64) as u64,
        },
    );

    // Last, so the sample covers the whole syscall; one relaxed load for every thread the
    // instrument is not measuring.
    #[cfg(any(test, feature = "ipc_stack_depth"))]
    crate::ipc_stack_depth::after_syscall(nr, method);
}

/// **A Reply method other than `REPLY`.** One exists: **`REPLY_CAPABILITY`, `REPLY` plus one
/// capability**, `carried` its slot in our table (§255 (each socket is its own capability)). `GRANT` on the carried capability is checked under the hold that
/// files it, and a refused source consumes nothing, so the caller still waits and this Reply can
/// still answer it. `Ok(1)` when the answer went but the copy did not reach the caller (a full
/// table, or a caller no longer waiting), so the server can undo what it minted it for.
///
/// Out of line on purpose, and handed the syscall's own registers so the call site is a jump. It
/// looks the Reply up again rather than taking it from [`invoke`], which costs a cold method one
/// table read. [`invoke`] inlines into `syscall::dispatch`, which `script/fastpath-footprint`
/// counts flat as `syscall_entry`; with this arm folded in, x86_64's entry grew 144 bytes (8.6%)
/// on every syscall for a method only the network stack uses. Plain `REPLY` stays inline.
#[inline(never)]
fn reply_other(slot: u64, method: u64, a0: u64, a1: u64, carried: u64) -> Result<i64, Error> {
    if method != abi::reply::REPLY_CAPABILITY {
        return Err(Error::BadMethod);
    }
    let reply = sched::current_cap(slot).map_err(|_| Error::NoSuchSlot)?;
    let Object::Reply(tid) = reply.object else {
        return Err(Error::BadMethod);
    };
    // As for `REPLY`: minted WRITE-only and without GRANT.
    if !reply.rights.allows(Rights::WRITE) {
        return Err(Error::NotPermitted);
    }
    let filed = sched::ipc_reply_capability(tid, [a0, a1], carried)?;
    // One-shot, as for `REPLY`.
    let _ = sched::delete_current_cap(slot);
    Ok(if filed { 0 } else { 1 })
}

/// Act on a capability.
///
/// **The lookup is the security mechanism, and it is a bounds check.** `slot` is an index into
/// *this thread's* table, which lives in kernel memory. An empty slot is `NoSuchSlot`: not
/// "permission denied", but *there is nothing there*. That difference is what no-ambient-authority
/// feels like from the inside.
///
/// `pub(crate)` so kernel tests in other modules can drive **this** path rather than a
/// re-implementation of it: an authorization test that calls `sched` directly proves the helper, not
/// the boundary. See `user/reap_tests.rs`.
pub fn invoke(
    frame: &mut TrapFrame,
    slot: u64,
    method: u64,
    a0: u64,
    a1: u64,
    a2: u64,
) -> Result<i64, Error> {
    let cap = sched::current_cap(slot).map_err(|_| Error::NoSuchSlot)?;

    match cap.object {
        Object::Rendezvous(ep, badge) => match method {
            // SEND takes WRITE, RECEIVE takes READ. The *same* endpoint, handed out with different
            // rights, is a one-way pipe in whichever direction each holder was trusted with.
            abi::rendezvous::SEND => {
                if !cap.rights.allows(Rights::WRITE) {
                    return Err(Error::NotPermitted);
                }
                // The three words are already in registers. **Nothing is read from user memory**,
                // so there is no pointer to validate and no confused-deputy question to ask. That
                // is the fastpath, and it is why IPC carries control and not bulk data (§10).
                // The badge rides in word 3 (milestone 613 (a system log service), provisional):
                // a byte-sink writer's identity is its capability's badge, never its own claim.
                sched::ipc_send_badged(ep, [a0, a1, a2], badge);
                // If the endpoint was revoked (stale, or reclaimed while we blocked), the send never
                // happened: report it rather than a silent success. Object revocation, notes/.
                //
                // `Gone`, not `NoSuchSlot`, since milestone 50. The slot is not empty; a real
                // capability names an object that has been destroyed, and a writer branches on the
                // difference in opposite directions (`abi::Error::Gone`, notes/sink-protocol.md).
                // This one line is what turns a dead pipe reader into something the producer can
                // act on, which is why the ABI grew a variant rather than the sink protocol growing
                // a heartbeat.
                //
                // **Or it was refused** (milestone 603 (provisional), DECISIONS §101 (notification objects) ruling B): the
                // endpoint carries a hardware interrupt, whose driver reads `w0 = 1` as "the device
                // fired", so no program may deposit anything there. `NotPermitted`, because it is
                // the answer to an operation the capability names but may not perform, and it is
                // asked only once the abort branch is taken, so a send that went through pays
                // nothing for it.
                if sched::take_ipc_aborted() {
                    return Err(aborted_send_error());
                }
                Ok(0)
            }
            abi::rendezvous::RECEIVE => {
                if !cap.rights.allows(Rights::READ) {
                    return Err(Error::NotPermitted);
                }
                let msg = sched::ipc_receive(ep);
                if sched::take_ipc_aborted() {
                    return Err(Error::Gone); // endpoint revoked; the message is a placeholder
                }
                // Word 0 goes back the way every syscall result does, in x0 (dispatch writes it
                // from our return value). Words 1..4 we place directly, because a syscall return is
                // one register and a message is up to five: ordinary IPC fills three and leaves the
                // top two zero, and a fault/exit notification (DECISIONS §26) fills all five.
                frame.set_arg(1, msg[1]);
                frame.set_arg(2, msg[2]);
                frame.set_arg(3, msg[3]);
                frame.set_arg(4, msg[4]);
                Ok(msg[0] as i64)
            }

            // Delegation. `a0` is the slot of the capability to pass on, `a1` the rights to narrow
            // it to, `a2` one data word. Two rights are in play and they are different questions:
            // WRITE on *this* endpoint (may I send here?) and GRANT on the *delegated* capability
            // (was I trusted to pass it on?). Without GRANT you may use a thing and not lend it.
            abi::rendezvous::SEND_CAP => {
                if !cap.rights.allows(Rights::WRITE) {
                    return Err(Error::NotPermitted);
                }
                // The source is read, checked (GRANT, narrow-only) and copied inside `sched`, under
                // the hold that files the copy, so no revocation sweep can fall between the read and
                // the filing. Reading it here first was that gap (`sched::Delegation`).
                #[cfg(feature = "system_tests")]
                crate::delegation_pause::here(); // no lock held
                sched::ipc_delegate_cap(
                    ep,
                    a2,
                    sched::Delegation {
                        slot: a0,
                        rights: Rights::from_bits(a1 as u32),
                    },
                    badge, // the endpoint we send on may be badged (milestone 599 (a frame per filesystem client channel))
                )?;
                if sched::take_ipc_aborted() {
                    // Revoked, or refused (§101 ruling B); either way the delegation did not happen
                    // and the capability is still the sender's.
                    return Err(aborted_send_error());
                }
                Ok(0)
            }
            abi::rendezvous::RECEIVE_CAP => {
                if !cap.rights.allows(Rights::READ) {
                    return Err(Error::NotPermitted);
                }
                let msg = sched::ipc_receive_cap(ep);
                if sched::take_ipc_aborted() {
                    return Err(Error::Gone); // endpoint revoked; the message is a placeholder
                }
                // x1 carries the slot the received capability landed in, or NO_CAP if the message
                // brought none; x2 the second data word (a CALL's, or 0); x3 the sender's badge
                // (milestone 599), 0 when its capability was unbadged. x0 returns the first word.
                // x4 is `abi::notification::BOUND` when the bound notification ended this receive,
                // `abi::rendezvous::REPLY_DELIVERED` when x1 is a CALL's Reply (§245 (a `CALL`
                // server tells a Reply from a delegation)), and 0 otherwise (milestone 151
                // (notification objects)): the one register here no sender can write, so the one a
                // bound server, and every CALL server, tests.
                frame.set_arg(1, msg[1]);
                frame.set_arg(2, msg[2]);
                frame.set_arg(3, msg[3]);
                frame.set_arg(4, msg[4]);
                Ok(msg[0] as i64)
            }

            // Call: send two words and block until replied. The kernel mints a one-shot Reply cap
            // naming us into the server (delivered by its RECEIVE_CAP); we return here only when the
            // server invokes it. See §12 and notes/ipc-naming.md. Sending needs WRITE, like SEND.
            abi::rendezvous::CALL => {
                if !cap.rights.allows(Rights::WRITE) {
                    return Err(Error::NotPermitted);
                }
                let reply = sched::ipc_call_badged(ep, [a0, a1], badge);
                if sched::take_ipc_aborted() {
                    return Err(aborted_send_error()); // revoked or refused; no call, no reply
                }
                frame.set_arg(1, reply[1]); // r1; r0 returns in x0 below
                // x2: the slot a `REPLY_CAPABILITY` filed in our table, or `NO_CAP` (§255 (each socket is
                // its own capability)). Written on every CALL so a caller reads the kernel's word,
                // never what it left in x2 itself.
                frame.set_arg(2, reply[2]);
                Ok(reply[0] as i64)
            }

            // **Collect a corpse this endpoint supervises** (DECISIONS §32). The method that lets a
            // supervisor reap without holding the authority to *build*: reaping used to mean
            // `MemoryRegion::DESTROY`, which needs WRITE on the region, and WRITE on a region is also what
            // retypes a thread and an address space out of it.
            //
            // READ, not WRITE: the authority to collect a death is the authority to *receive* deaths
            // here, which is what a supervisor holds and what a send-only holder (a peer that can
            // report to this supervisor) deliberately does not. `a0` is the tid the kernel stamped on
            // the death message, and it is authorized *relative to this endpoint* (sched's
            // reap_supervised), so it is a name inside a relationship rather than a global handle.
            abi::rendezvous::REAP => {
                if !cap.rights.allows(Rights::READ) {
                    return Err(Error::NotPermitted);
                }
                rendezvous_reap(ep, a0)
            }

            // **Read one entry of the domain this endpoint supervises** (milestone 126 (the `procps` package)). The view
            // half of what REAP is the control half of, and scoped by the same relationship, so a
            // supervisor sees exactly the children whose deaths would arrive here.
            //
            // READ for the same reason REAP takes READ: the authority to see who may die here is
            // the authority to receive deaths here. **A send-only holder is refused rather than
            // shown an empty domain**, which is the whole point of the method; a monitor that
            // reports nothing because it could not look is the worst failure this tool has, and an
            // empty answer is reserved for a domain that really is empty.
            //
            // `a0` is the cursor: 0 to start, then whatever the last call returned, until a
            // `survey::DONE` comes back. `a1` is the RECORD: which per-thread fact the caller
            // wants. x1 carries the tid and x2 that record's word.
            //
            // **The selector is calef's 2026-09-21 ruling, and it replaces growing this row.** The
            // earlier plan put each new fact in a further return register; he expects a third and a
            // fourth fact, and a mechanism that has to be redesigned at the sixth field is the
            // wrong mechanism at the fourth. Nobody grows a register row: Linux reached 52 fields
            // through a pseudo-file and Zircon 41 topics through a selector. So x0 and x1 are the
            // frame, the same for every record, and only x2 belongs to the record.
            //
            // **`record::STATE` is 0 so that every caller written before the selector keeps
            // working**, having passed 0 into an argument that was then unused. That is a claim
            // about a wire rather than a hope, and it is asserted in `abi`'s own tests.
            //
            // **Why the placement record needs no authority beyond the `ENUMERATE` below**, said
            // here rather than in a decisions file the reader would have to go and find. §150 (how does a
            // thread's CPU time reach userspace?)
            // already weighed that a viewer holding `ENUMERATE` learns something aggregate about
            // threads it cannot otherwise name, and accepted it for a CPU-time counter. A placement
            // is strictly less than what was accepted: one bounded value out of at most 64, written
            // once when the thread started and never again, against a counter that moves
            // continuously and can therefore be differenced into a timing channel. A viewer that
            // can already see a thread's tid and run state learns which of a handful of cores it
            // was put on. **It can never name a thread outside the domain it was handed**; what
            // it can learn about them is one noisy bit per spawn (2026-09-24 security audit):
            // `pick_spawn_target` samples two cores and takes the one with the shorter run queue,
            // and that queue counts every domain's threads, so a placement says which of two
            // random cores was lighter at that instant. That is the counting channel the
            // 2026-08-17 audit recorded, at lower bandwidth, and it is accepted on the same
            // reasoning. The right to look is still the capability, and nothing here widens it.
            abi::rendezvous::SURVEY => {
                // `ENUMERATE`, not `READ`, and the distinction is the method's whole safety
                // argument: `READ` here also unlocks `RECEIVE` and `REAP`, so a viewer granted it
                // could reap a child. A domain names its members and does not act on them, and one
                // bit for three operations cannot say that. See `Rights::ENUMERATE`.
                if !cap.rights.allows(Rights::ENUMERATE) {
                    return Err(Error::NotPermitted);
                }
                let (next, tid, word) = sched::survey_supervised(ep, a0, a1)?;
                frame.set_arg(1, tid);
                frame.set_arg(2, word);
                Ok(next as i64)
            }

            // Body extracted, the pattern of milestone 156 (`syscall_entry`'s measured size is every method
            // combined): minting a badge is spawn-time delegation, never a step of the IPC round
            // trip, so it stays out of `invoke`'s own bytes (milestone 599).
            abi::rendezvous::BADGE => rendezvous_badge(ep, cap.rights, badge, a0),
            _ => Err(Error::BadMethod),
        },

        // A one-shot reply to a specific caller (§12). Minted by the kernel at a CALL rendezvous,
        // named by the caller's tid, consumed on use.
        Object::Reply(tid) => match method {
            abi::reply::REPLY => {
                // Minted WRITE-only, so a narrowed derivative could not answer; and minted without
                // GRANT, so it could not have been delegated here in the first place.
                if !cap.rights.allows(Rights::WRITE) {
                    return Err(Error::NotPermitted);
                }
                sched::ipc_reply(tid, [a0, a1]);
                // One-shot: consume it, so a second reply is NoSuchSlot and the caller cannot be
                // answered twice. This is the guarantee a pre-wired reply endpoint cannot make.
                let _ = sched::delete_current_cap(slot);
                Ok(0)
            }
            // Every other method, `REPLY_CAPABILITY` and `BadMethod` alike, is decided out of line.
            _ => reply_other(slot, method, a0, a1, a2),
        },

        // Another process's memory, under construction (19b). WRITE on the address space cap is the
        // authority to shape it; the frame's own rights gate what kind of mapping, exactly as
        // frame::MAP; the va gate is the proved paging::is_user_page_va, as everywhere.
        Object::AddressSpace(name) => match method {
            abi::address_space::MAP_INTO => address_space_map_into(cap, name, a0, a1, a2),
            // List what this address space has mapped, one entry per call, without the ability
            // to change any of it (milestone 126's `pmap`, DECISIONS §114): `Rendezvous::SURVEY`'s
            // shape one object type over, and pointedly `ENUMERATE` rather than `WRITE`, which is
            // what `MAP_INTO` above takes. See `abi::address_space::LIST` for the wire contract and
            // DECISIONS §114 for why this method's mere existence is the thing that makes
            // `ENUMERATE` live on every address-space capability minted since 2026-08-17 (the
            // `Rights::ALL`-on-creation invariant): the audit that check required is in
            // notes/process-view.md.
            abi::address_space::LIST => {
                if !cap.rights.allows(Rights::ENUMERATE) {
                    return Err(Error::NotPermitted);
                }
                address_space_list(frame, name, a0)
            }
            // This space gives up the page it maps at `va` (milestone 95 (an unmap primitive),
            // DECISIONS §162 (whether a holder can give up a mapping), option A).
            // `WRITE`, the authority `MAP_INTO` takes: shaping a space is one right in both
            // directions. See `abi::address_space::UNMAP` and notes/unmap.md.
            abi::address_space::UNMAP => {
                if !cap.rights.allows(Rights::WRITE) {
                    return Err(Error::NotPermitted);
                }
                address_space_unmap(name, a0)
            }
            // Futex wait and wake (milestone 812 (`std::thread::spawn` runs real threads in one
            // address space), §269 (how threads share a process) fork 2), out of line: neither is
            // a step of the IPC round trip the fastpath footprint bounds.
            abi::address_space::WAIT | abi::address_space::WAKE => {
                address_space_futex(cap.rights, name, method, a0, a1, a2)
            }
            _ => Err(Error::BadMethod),
        },

        // A thread under construction (19c.3). WRITE on the TCB cap is the authority to shape
        // and start it. Every method refuses a thread that is not an embryo, in the scheduler.
        Object::ThreadControlBlock(tid) => {
            thread_control_block_invoke(frame, cap.rights, tid, method, [a0, a1, a2])
        }

        // A notification (milestone 151, DECISIONS §101 (notification objects)). Extracted and `#[inline(never)]` for
        // `memory_region_map`'s reason: none of these is a step of the IPC round trip
        // `script/fastpath-footprint` bounds, so the new arm costs the flat `syscall_entry` one
        // call rather than four method bodies.
        Object::Notification(id) => notification_invoke(cap.rights, id, method, a0),

        // A timer (milestone 106, DECISIONS §147 (a timer a userspace service cannot hold)). Out of line for the notification arm's reason:
        // neither method is a step of the IPC round trip the fastpath footprint bounds.
        Object::Timer(id) => timer_invoke(cap.rights, id, method, a0, a1, a2),

        // The reboot object (milestone 805 (`reboot` at the prompt), DECISIONS §251 (restarting the
        // machine is a kernel object the progenitor hands out)). Out of line for the timer arm's
        // reason, and colder than any of them: a successful call never returns. Holding the
        // capability is the whole authority, so no rights bit is asked for (§251, "The method").
        Object::Reboot => reboot_invoke(method),

        Object::MemoryRegion(region) => match method {
            // Body extracted (milestone 156): all five `MemoryRegion` methods are memory-management
            // administration a spawner runs while building a process, never a step of the IPC
            // round trip `script/fastpath-footprint` bounds, so each stays out of `invoke`'s own
            // bytes and `#[inline(never)]` on purpose. See `address_space_list`, the pattern this copies.
            abi::memory_region::MAP => {
                if !cap.rights.allows(Rights::WRITE) {
                    return Err(Error::NotPermitted);
                }
                memory_region_map(region, a0)
            }
            abi::memory_region::RETYPE_OBJ => {
                if !cap.rights.allows(Rights::WRITE) {
                    return Err(Error::NotPermitted);
                }
                memory_region_retype_obj(region, a0)
            }
            abi::memory_region::RETYPE => {
                if !cap.rights.allows(Rights::WRITE) {
                    return Err(Error::NotPermitted);
                }
                memory_region_retype(region, a0)
            }
            abi::memory_region::SPLIT => {
                if !cap.rights.allows(Rights::WRITE) {
                    return Err(Error::NotPermitted);
                }
                memory_region_split(cap, region, a0)
            }
            abi::memory_region::DESTROY => {
                if !cap.rights.allows(Rights::WRITE) {
                    return Err(Error::NotPermitted);
                }
                memory_region_destroy(region)
            }
            // What the region was spent on (milestone 126's `free`, DECISIONS §225 (`free` sees the machine and your share) part 1). Under
            // `ENUMERATE` alone, `address_space::LIST`'s rule one object type over: learning what
            // a budget went to is not the authority to spend it. An unknown record is refused
            // before the region is looked up, `SURVEY`'s order.
            abi::memory_region::USAGE => memory_region_usage(cap, region, a0),
            _ => Err(Error::BadMethod),
        },

        Object::PageFrame(phys, count) => match method {
            // Body extracted (milestone 156), the same reason as `MemoryRegion`'s five methods:
            // neither `MAP` nor `REVOKE` is a step of the IPC round trip, so both move out of
            // `invoke`'s own bytes. `MAP`'s rights check is data-dependent (branches on `a1`), so
            // it lives inside `page_frame_map` rather than at the call site here, unlike the fixed
            // single-right checks the other extractions keep in `invoke`.
            //
            // §102 (2026-08-20): `count` rides on the capability, not on the syscall's arguments,
            // so `MAP`'s and `REVOKE`'s wire shape is exactly what it was before the object could
            // name a run. A single-page frame (`count: 1`) runs each loop below once.
            abi::page_frame::MAP => page_frame_map(slot, a0, a1, a2),
            abi::page_frame::REVOKE => {
                if !cap.rights.allows(Rights::GRANT) {
                    return Err(Error::NotPermitted);
                }
                page_frame_revoke(phys, count.get())
            }
            abi::page_frame::SLICE => page_frame_slice(slot, a0, a1),
            _ => Err(Error::BadMethod),
        },

        // A device's MMIO page is almost passive: it is handed to MAP_INTO as the page to map
        // (19d.2), and since milestone 23 it answers exactly one invocation, `REVOKE`.
        Object::DeviceFrame(phys) => match method {
            // **Take the registers back from everyone else** (DECISIONS §41). The step live
            // replacement needs between tearing one driver down and endowing the next, so that a
            // device never has two owners. Needs `GRANT`, the same rule `PageFrame::REVOKE` uses: you
            // were trusted to lend the device on, so you may take it back.
            //
            // Unlike a frame revoke this **spares the invoker's own** capability and mapping, and
            // it must: only the kernel mints a `DeviceFrame`, and it does so once at boot, so a
            // symmetric revoke would make the device unreachable for the rest of the machine's
            // life. `revoke::revoke_device_from_others` carries the full argument.
            abi::page_frame::REVOKE => {
                if !cap.rights.allows(Rights::GRANT) {
                    return Err(Error::NotPermitted);
                }
                crate::revoke::revoke_device_from_others(phys);
                Ok(0)
            }
            _ => Err(Error::BadMethod),
        },

        Object::Virtio(id) => {
            if !cap.rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            virtio_invoke(id, method, a0, a1)
        }

        Object::Irq(intid) => match method {
            // Body extracted (milestone 156). `WAIT` does block on the same `sched::ipc_receive`
            // the `Rendezvous` fastpath uses, but the caller here is a driver waiting on a device
            // interrupt, not the IPC round trip this gate bounds, so it moves out of `invoke`'s
            // own bytes with the rest. See `memory_region_map`'s doc comment for the full reasoning.
            abi::irq::WAIT => {
                if !cap.rights.allows(Rights::READ) {
                    return Err(Error::NotPermitted);
                }
                irq_wait(frame, intid)
            }
            abi::irq::ACK => {
                if !cap.rights.allows(Rights::READ) {
                    return Err(Error::NotPermitted);
                }
                irq_ack(intid)
            }
            _ => Err(Error::BadMethod),
        },

        // A port range is enforced at the context switch (the TSS I/O bitmap), so like a
        // `DeviceFrame` it is almost passive on the syscall path: it answers exactly one invocation,
        // `REVOKE`, the take-back a live driver replacement needs (DECISIONS §121, milestone 299).
        // Extracted, `#[inline(never)]`, for `memory_region_map`'s reason: a rare administrative
        // method has no business growing the flat `syscall_entry` footprint the IPC round trip is
        // measured against (`script/fastpath-footprint`). The variant is `x86_64`-only, so on the
        // other two architectures this match is exhaustive without it and their dispatcher is
        // unchanged.
        #[cfg(target_arch = "x86_64")]
        Object::PortRange(base, count) => port_range_invoke(cap.rights, base, count, method),
    }
}

/// **Which error an aborted `SEND`, `SEND_CAP` or `CALL` answers** (milestone 603 (provisional)).
/// `NotPermitted` when the endpoint carries an interrupt and refused the deposit (DECISIONS §101,
/// ruling B), `Gone` when it was stale or revoked. Out of line and `#[cold]`, because it runs only
/// on an abort and the three arms that call it are on the IPC fastpath.
///
/// Name: provisional (milestone 603 (provisional)): calef names public items.
#[cold]
#[inline(never)]
fn aborted_send_error() -> Error {
    if sched::take_ipc_refused() {
        Error::NotPermitted
    } else {
        Error::Gone
    }
}

/// `PortRange::REVOKE` (milestone 299): take the port range back from every other holder. The only
/// method a port capability answers; enforcement is at the switch, not here.
///
/// **`#[cold]`, not just `#[inline(never)]`.** A live-driver-replacement take-back is among the
/// rarest things a program does, and `syscall_entry`'s footprint bound is measured over the
/// *non-cold* calls reachable from the dispatcher (`script/fastpath-footprint`). Marking this cold
/// keeps it and `revoke_port_range_from_others` out of that closure, so a new capability method does
/// not spend the IPC round trip's L1i budget on a path the round trip never takes.
#[cfg(target_arch = "x86_64")]
#[cold]
#[inline(never)]
fn port_range_invoke(rights: Rights, base: u16, count: u16, method: u64) -> Result<i64, Error> {
    match method {
        abi::port_range::REVOKE => {
            if !rights.allows(Rights::GRANT) {
                return Err(Error::NotPermitted);
            }
            crate::revoke::revoke_port_range_from_others(base, count);
            Ok(0)
        }
        _ => Err(Error::BadMethod),
    }
}

/// The four `Notification` methods (milestone 151, DECISIONS §101). Rights are §101's: `WRITE` to
/// signal and to bind, `READ` to wait and to poll. `BIND` also needs `WRITE` on the thread it names,
/// because binding changes what that thread's receives return. See `abi::notification`.
#[inline(never)]
fn notification_invoke(
    rights: Rights,
    id: sched::NotificationId,
    method: u64,
    a0: u64,
) -> Result<i64, Error> {
    match method {
        abi::notification::SIGNAL => {
            if !rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            sched::notification_signal(id, a0)?;
            Ok(0)
        }
        abi::notification::WAIT => {
            if !rights.allows(Rights::READ) {
                return Err(Error::NotPermitted);
            }
            sched::notification_wait(id).map(|w| w as i64)
        }
        abi::notification::POLL => {
            if !rights.allows(Rights::READ) {
                return Err(Error::NotPermitted);
            }
            sched::notification_poll(id).map(|w| w as i64)
        }
        abi::notification::BIND => {
            if !rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            let thread = sched::current_cap(a0).map_err(|_| Error::NoSuchSlot)?;
            let Object::ThreadControlBlock(tid) = thread.object else {
                return Err(Error::WrongObject);
            };
            if !thread.rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            sched::notification_bind(id, tid)?;
            Ok(0)
        }
        _ => Err(Error::BadMethod),
    }
}

/// `Reboot::REBOOT` (milestone 805, DECISIONS §251): restart the machine, or answer why not, one of
/// the four `abi::Error::Reset…` reasons. See kernel/src/reboot.rs.
#[inline(never)]
fn reboot_invoke(method: u64) -> Result<i64, Error> {
    match method {
        abi::reboot::REBOOT => Err(crate::reboot::restart()),
        _ => Err(Error::BadMethod),
    }
}

/// The two `Timer` methods (milestone 106, DECISIONS §147). `WRITE` on the timer for both, and
/// for `ARM` also `WRITE` on the notification it names, because arming is signalling later and
/// `SIGNAL` takes `WRITE`. See `abi::timer`.
#[inline(never)]
fn timer_invoke(
    rights: Rights,
    id: sched::TimerId,
    method: u64,
    a0: u64,
    a1: u64,
    a2: u64,
) -> Result<i64, Error> {
    if !rights.allows(Rights::WRITE) {
        return Err(Error::NotPermitted);
    }
    match method {
        abi::timer::ARM => {
            let target = sched::current_cap(a1).map_err(|_| Error::NoSuchSlot)?;
            let Object::Notification(notification) = target.object else {
                return Err(Error::WrongObject);
            };
            if !target.rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            sched::timer_arm(id, a0, notification, a2)?;
            Ok(0)
        }
        abi::timer::CANCEL => sched::timer_cancel(id).map(i64::from),
        _ => Err(Error::BadMethod),
    }
}

/// `MemoryRegion::MAP`: retype a page out of the untyped and map it, writable, at `va` in the caller's
/// own address space. Both the page and any page tables come from the untyped, so the KERNEL
/// ALLOCATES NOTHING: the leaf is retyped here and the tables by the closure below, both bumping
/// the untyped's watermark.
///
/// Pulled out of [`invoke`] and marked `#[inline(never)]` (milestone 156, the pattern
/// `address_space_list` proved on milestone 126's `LIST`): `syscall_entry` is measured flat, so a rare
/// administrative arm inlined into the hot dispatcher grows every syscall's footprint for a
/// method that never runs on an IPC round trip.
///
/// **Retype, map and record are one [`MappingHold`](crate::revoke::MappingHold)** (the
/// map-revocation-window lane, 2026-10-04 UTC). A region is generational, so the window here was
/// narrower than `PageFrame::MAP`'s, and it was there: a retype that succeeded before
/// `MemoryRegion::DESTROY` claimed the region (the claim is the generation bump, after which a
/// retype refuses) and recorded after `revoke_region` had scanned left a live mapping of a page the
/// allocator then reused. Under the hold, the claim lands before the retype (it refuses) or the
/// scan lands after the record (it finds it).
/// `map_revocation_window_tests::a_destroy_inside_a_memory_region_map_leaves_no_mapping` drives it.
#[inline(never)]
fn memory_region_map(region: u64, va: u64) -> Result<i64, Error> {
    // Reject the cheap failures BEFORE retyping a page for them: a non-page-aligned or
    // non-low-half address can never be mapped, and without this pre-check each such attempt
    // would silently spend a page of the process's own untyped (a self-inflicted budget leak the
    // audit noted). An already-mapped `va` still costs one page, which is process-local and
    // bounded by the untyped. The gate itself is proved: every address it admits is aligned and
    // in the low half (see `paging::is_user_page_va` and its harness).
    if !paging::is_user_page_va::<crate::arch::mmu::Format>(va) {
        return Err(Error::BadPointer);
    }
    #[cfg(feature = "system_tests")]
    crate::delegation_pause::here(); // no lock held
    let mut hold = crate::revoke::hold();
    let root = mmu::current_user_root();
    // The leaf first, then the tables, recorded as tables as they are retyped: the region pays
    // for both, and its `DESTROY` has to find the tables to cut them out of this walk before the
    // pages go back (`revoke::revoke_region`). An already-mapped `va` spends the leaf, as before.
    let Some(phys) = crate::memory_region::retype_page(region) else {
        return Err(Error::OutOfMemory);
    };
    match mmu::map_current_user_page_frame(va, phys, paging::Flags::user_data(), || {
        hold.retype_table(region, root, va)
    }) {
        Ok(()) => {
            // Record the mapping so it can be revoked before the region is ever reclaimed (§13).
            // MemoryRegion::MAP pages are process-private, but they still must be unmapped before
            // memory_region::destroy frees the region under them. The record is paid from the caller's
            // own address-space budget (phase C); if it cannot afford the record, it cannot keep
            // the mapping: an unrecorded mapping is invisible to revocation, the §13 hole.
            // No capability names this page (§132): `MemoryRegion::MAP` retypes and maps in one
            // step, so there is never a `PageFrame` capability and no derivation family to scope a
            // revoke to. Reclamation finds the record regardless, because `revoke_region`'s unmap
            // sweep is object-blind by design.
            if !hold.record_mapping(phys, root, va, crate::revoke::PageMapSource::NoCapability) {
                mmu::unmap_user_at(root, va);
                return Err(Error::OutOfMemory);
            }
            Ok(0)
        }
        Err(paging::MapError::OutOfPageFrames) => Err(Error::OutOfMemory),
        Err(_) => Err(Error::BadPointer), // misaligned, already mapped, or wrong half
    }
}

/// `Rendezvous::BADGE`: **mint a badged copy of an endpoint** (milestone 599 (a frame per
/// filesystem client channel), provisional). The new capability names the same endpoint `ep` with
/// the holder's own `rights`, plus `new_badge`, in a free slot of the caller's table, and the slot
/// is the result, the way `RETYPE_OBJ` answers with the slot it minted.
///
/// GRANT-gated, because minting a delegatable view is a delegation-class power (the gate `SEND_CAP`
/// and `CAP_INSERT` use), and refused for a zero badge (the unbadged value) or an already-badged
/// source (`held_badge != 0`), so a badge is set once and never changed: seL4's rule, and what lets
/// a server trust the badge it is delivered. Out of line for `memory_region_map`'s reason: it is
/// spawn-time administration, and `script/fastpath-footprint` measures `invoke`'s own bytes.
#[inline(never)]
fn rendezvous_badge(
    ep: sched::RendezvousId,
    rights: Rights,
    held_badge: u64,
    new_badge: u64,
) -> Result<i64, Error> {
    if !rights.allows(Rights::GRANT) || new_badge == 0 || held_badge != 0 {
        return Err(Error::NotPermitted);
    }
    let slot = sched::grant(crate::cap::rendezvous_cap_badged(ep, rights, new_badge))
        .map_err(|_| Error::OutOfMemory)?;
    Ok(slot as i64)
}

/// `MemoryRegion::RETYPE_OBJ`: retype a page into a page-resident KERNEL OBJECT the caller now owns
/// (19a). The object lives in the carved page, the region is pinned (a live endpoint's page must
/// never be freed under a blocked thread), and the caller gets full rights on its own object,
/// delegation narrowing them as ever. `#[inline(never)]` for the reason `memory_region_map` gives.
#[inline(never)]
fn memory_region_retype_obj(region: u64, kind: u64) -> Result<i64, Error> {
    match kind {
        abi::objtype::RENDEZVOUS => {
            let ep = sched::create_rendezvous_from(region).ok_or(Error::OutOfMemory)?;
            // `Rights::ALL`, not a list. The comment above has always said the creator gets full
            // rights on its own object; spelling the set out meant "full" silently stopped being
            // full the day `ENUMERATE` was added, and the symptom was three steps away: the progenitor
            // could not narrow `deaths` to a right it did not itself hold, `CAP_INSERT` refused
            // the widen, and the spawn surfaced as `OutOfMemory` at a prompt. A rights set that
            // must be updated by hand whenever a right is added is rung four; `ALL` is the
            // invariant.
            let slot = sched::grant(crate::cap::rendezvous_cap(ep, Rights::ALL))
                .map_err(|_| Error::OutOfMemory)?;
            Ok(slot as i64)
        }
        // An address space (19b): the page becomes the L0 root, the untyped becomes the space's
        // backing region for tables and records (one budget model; see the abi doc and
        // design/init-and-granular-spawn.md).
        abi::objtype::ADDRESS_SPACE => {
            let name = crate::user::user_address_space_create(region).ok_or(Error::OutOfMemory)?;
            // `Rights::ALL` for the RENDEZVOUS arm's reason: "full rights on its own object" is the
            // invariant, and a hand-listed set stops being full the next time a right is added.
            // `AddressSpace` does not consult `ENUMERATE` today and is expected to when `pmap` is
            // built; holding a right nothing checks confers nothing, and not holding it is what
            // blocks a future grant.
            let slot = sched::grant(crate::cap::address_space_cap(name, Rights::ALL))
                .map_err(|_| Error::OutOfMemory)?;
            Ok(slot as i64)
        }
        // A thread (19c.3): the page holds an embryo TCB, born in no queue and not runnable
        // until CONFIGURE + START. The page is the creator's region's.
        abi::objtype::THREAD_CONTROL_BLOCK => {
            let tid = sched::create_thread_control_block(region).ok_or(Error::OutOfMemory)?;
            let slot = sched::grant(crate::cap::thread_control_block_cap(tid, Rights::ALL))
                .map_err(|_| Error::OutOfMemory)?;
            Ok(slot as i64)
        }
        // A notification (milestone 151, DECISIONS §101): a word and a wait queue in the page.
        // `Rights::ALL` for the RENDEZVOUS arm's reason.
        abi::objtype::NOTIFICATION => {
            let id = sched::create_notification_from(region).ok_or(Error::OutOfMemory)?;
            let slot = sched::grant(crate::cap::notification_cap(id, Rights::ALL))
                .map_err(|_| Error::OutOfMemory)?;
            Ok(slot as i64)
        }
        // A timer (milestone 106, DECISIONS §147): one deadline and its target in the page.
        // `Rights::ALL` for the RENDEZVOUS arm's reason.
        abi::objtype::TIMER => {
            let id = sched::create_timer_from(region).ok_or(Error::OutOfMemory)?;
            let slot = sched::grant(crate::cap::timer_cap(id, Rights::ALL))
                .map_err(|_| Error::OutOfMemory)?;
            Ok(slot as i64)
        }
        _ => Err(Error::BadMethod), // no such object type
    }
}

/// `MemoryRegion::RETYPE`: retype `a0` pages (`0` meaning one) into one `PageFrame` capability the
/// caller now holds, instead of mapping them in one shot. The caller gets full rights on its own
/// frame (read, write, and the right to pass it on); delegation is where those narrow. Nothing is
/// mapped yet. The count arrived with calef's ruling of 2026-09-26 and a run is the `PageFrame` run of
/// §102 (a Frame names a run of pages); every caller before it passed `0`. `#[inline(never)]` for the reason `memory_region_map`
/// gives.
#[inline(never)]
fn memory_region_retype(region: u64, requested: u64) -> Result<i64, Error> {
    let (phys, count) =
        crate::memory_region::retype_run(region, requested).ok_or(Error::OutOfMemory)?;
    // The run is non-empty by construction (`retype_pages` never returns zero).
    let count = core::num::NonZeroU64::new(count).ok_or(Error::OutOfMemory)?;
    // Capability table full. **BUGS: the run stays retyped** (recorded by milestone 757 (a test
    // kernel fails a process on its Nth retype), provisional): the watermark has moved and no
    // capability names the pages, so each call against a full table spends `count` pages until the
    // region is destroyed. `memory_region_split` below gives its child back in the same position;
    // a run has no such inverse today. `memory_region_retype_obj`'s arms are the same shape, an
    // object nobody can name. Bounded by the caller's own region, so a self-inflicted cost, not a
    // leak into anyone else's budget.
    let slot = sched::grant(crate::cap::page_frame_run_cap(phys, count, Rights::ALL))
        .map_err(|_| Error::OutOfMemory)?;
    Ok(slot as i64)
}

/// `MemoryRegion::SPLIT`: carve a child untyped off this one (subdivision), so a spawner can give each
/// child its own reclaimable region. `count` is the child's page count. `#[inline(never)]` for
/// the reason `memory_region_map` gives.
#[inline(never)]
fn memory_region_split(cap: crate::cap::Cap, region: u64, count: u64) -> Result<i64, Error> {
    let child = crate::memory_region::split(region, count).ok_or(Error::OutOfMemory)?;
    // The child inherits THIS capability's rights, never more (milestone 31). SPLIT is a fresh
    // mint, so it must honor the derive-never-widens invariant by hand: a process holding a
    // spend-only (GRANT-less) untyped must not SPLIT itself a GRANT-bearing child over the same
    // memory and manufacture the right its capability withheld. `Cap::mint_child` is that
    // inheriting mint, and `split_never_widens_rights` (crates/capability) proves it never widens,
    // at the one mint site outside `derive` the caps proofs otherwise miss (milestone 35). Rights
    // narrow monotonically from the delegable root budget down; the progenitor holds that root with GRANT
    // and hands narrowed budgets on. See DECISIONS §16.
    //
    // **A full capability table gives the child back rather than orphaning it**, milestone 601 (the
    // region table prints its peak), a provisional number. This used to be a bare `?`, which
    // returned `OutOfMemory` with the child still live and no capability anywhere naming it: its region slot was held until reboot, and so was
    // the parent, because the orphan's count on it could never come down (the same consequence as
    // `RegionTable::split`'s BUGS entry, by a different road). The child is unpinned, childless and
    // at the top of the parent's watermark, so `destroy` returns its pages to the parent LIFO and
    // drops the count, leaving the parent exactly as it was before the call.
    match sched::grant(cap.mint_child(crate::cap::Object::MemoryRegion(child))) {
        Ok(slot) => Ok(slot as i64),
        Err(_) => {
            crate::memory_region::destroy(child);
            Err(Error::OutOfMemory) // capability table full
        }
    }
}

/// `MemoryRegion::DESTROY`: reclaim this region and every object retyped from it (object revocation):
/// tear the objects down and return the memory. Refused (`NotPermitted`) while a live thread still
/// occupies it, or if it has been split into children (destroy those first). Generational names
/// make every capability to the reclaimed objects stale on next use. `#[inline(never)]` for the
/// reason `memory_region_map` gives.
#[inline(never)]
fn memory_region_destroy(region: u64) -> Result<i64, Error> {
    sched::reclaim_region(region).map_err(|_| Error::NotPermitted)?;
    Ok(0)
}

/// `AddressSpace::MAP_INTO`: map an existing frame into *another* process's address space under
/// construction (19b), the other half of [`page_frame_map`]. `#[inline(never)]` for the reason
/// `memory_region_map` gives, **found late** (milestone 142's review): this arm sat inline in
/// [`invoke`]'s dispatch since before milestone 156's discipline existed, and nobody had retrofitted
/// it. It stayed under the `syscall_entry` footprint budget by luck until MAJOR 2 and MAJOR 3's real,
/// necessary fixes (the rollback loop, the tail-VA check) added enough code to an already-inline arm
/// to trip the 5% tripwire on riscv64 -- the fix was correct, it just landed in the wrong place
/// structurally. Extracted here rather than shrunk, the same move every sibling administrative arm
/// in this file already made.
#[inline(never)]
fn address_space_map_into(
    cap: crate::cap::Cap,
    name: u64,
    a0: u64,
    a1: u64,
    a2: u64,
) -> Result<i64, Error> {
    if !cap.rights.allows(Rights::WRITE) {
        return Err(Error::NotPermitted);
    }
    let va = a0;
    // **The frame is read under the hold that maps and records it** (the map-revocation-window
    // lane, 2026-10-04 UTC). It used to be read here with `sched::current_cap`, in a critical
    // section of its own, and a sweep could fall between that read and the record and leave the
    // mapping live: `revoke::MappingHold` has the whole account. The space's registry comes first
    // because it ranks above the mapping registry; neither touches `IPC_TABLES`.
    #[cfg(feature = "system_tests")]
    crate::delegation_pause::here(); // no lock held
    crate::user::with_user_address_space(name, |space| {
        let mut hold = crate::revoke::hold();
        let frame = hold.current_cap(a1).map_err(|_| Error::NoSuchSlot)?;
        // The mappable object is a PageFrame (normal memory) or a DeviceFrame (a device's
        // MMIO, device-typed): the driver a userspace progenitor builds gets its registers this
        // way (19d.2). a2 chooses the shape for a PageFrame; a DeviceFrame is always
        // device-typed read/write and needs WRITE on the cap.
        let (phys, count, flags) = match frame.object {
            Object::DeviceFrame(phys) => {
                if !frame.rights.allows(Rights::WRITE) {
                    return Err(Error::NotPermitted);
                }
                (phys, 1u64, paging::Flags::user_device())
            }
            // §102 (2026-08-20): `count` is the run's length. A single-page frame is
            // `count: 1`, so this arm's behavior for every existing caller is unchanged; a
            // run-capable frame maps the whole run in this one MAP_INTO call, exactly as
            // `page_frame::MAP` does below.
            Object::PageFrame(phys, count) => {
                // 0 read-only, 1 read/write, 2 executable code (a loader's child .text).
                // Code is W^X: user_code is RX, never writable, so it needs only READ.
                let flags = match a2 {
                    abi::address_space::MAP_RW => {
                        if !frame.rights.allows(Rights::WRITE) {
                            return Err(Error::NotPermitted);
                        }
                        paging::Flags::user_data()
                    }
                    abi::address_space::MAP_CODE => {
                        if !frame.rights.allows(Rights::READ) {
                            return Err(Error::NotPermitted);
                        }
                        paging::Flags::user_code()
                    }
                    _ => {
                        if !frame.rights.allows(Rights::READ) {
                            return Err(Error::NotPermitted);
                        }
                        paging::Flags::user_rodata()
                    }
                };
                (phys, count.get(), flags)
            }
            _ => return Err(Error::WrongObject),
        };
        // **The run's last page is checked, not only its first** (milestone 142's review,
        // MAJOR 3). This used to check `va` alone, which was right when a frame was one
        // page and wrong the moment `count` could exceed 1: a run placed near the top of
        // the low half passed the check and then walked out of it partway through the loop
        // below, refused three layers down by `Mapper::map`'s own `Half::Low` re-check
        // rather than here. Same guard `page_frame_map` uses, same reason: reject the whole
        // request before mapping any of it.
        let Some(last_va) = run_end_va(va, count) else {
            return Err(Error::BadPointer);
        };
        if !paging::is_user_page_va::<crate::arch::mmu::Format>(va)
            || !paging::is_user_page_va::<crate::arch::mmu::Format>(last_va)
        {
            return Err(Error::BadPointer);
        }
        // A `MAP_INTO` naming a space that does not exist fails before it spends a page
        // table, and the loop's own `NotMapped` would otherwise report that as a mapping
        // failure with nothing to roll back.
        let Some(space) = space else {
            return Err(Error::BadPointer);
        };
        let root = space.root();
        for k in 0..count {
            let (page_phys, page_va) = (phys + k * paging::PAGE_SIZE, va + k * paging::PAGE_SIZE);
            // `phys` is the run's base for a `PageFrame` and the page itself for a
            // `DeviceFrame`, so in both arms it is the object the invoked capability names
            // (§132). The device case never scopes a revoke by it: `DeviceFrame::REVOKE`
            // scopes by holder, not by capability, which `revoke_device_from_others`
            // explains.
            match space.map_physical_held(
                &mut hold,
                page_va,
                page_phys,
                flags,
                crate::revoke::PageMapSource::Capability(phys),
            ) {
                Ok(()) => {
                    // When userspace maps a frame it wrote executable (a spawner building a
                    // child's code, MAP_CODE), the instruction fetcher must be made to see
                    // the bytes the writer stored: RISC-V's `fence.i`, aarch64's
                    // dcache-clean + icache-invalidate, both behind `sync_icache`. The
                    // kernel-side ELF loader does this (user.rs map_segments); this is the
                    // userspace-built path, which a fast spawn+reap loop (bench::spawn_el0)
                    // is the first thing to stress. A child that fetches unsynced code
                    // takes an illegal-instruction fault at its entry.
                    if flags.is_user_executable() {
                        crate::arch::sync_icache(
                            crate::arch::mmu::phys_to_virt(page_phys),
                            paging::PAGE_SIZE as usize,
                        );
                    }
                }
                // **All or nothing across the run** (milestone 142's review, MAJOR 2).
                // Whatever this loop mapped before failing is unmapped again, so a caller
                // that gets an error never has to wonder how much of its run landed: the
                // answer is always none of it. Before this the prefix stayed mapped and
                // recorded with no way to ask about it, which is the pre-§102 single-page
                // path's own rollback quietly narrowed to one page by the widening.
                Err(e) => {
                    unmap_run_prefix(&mut hold, root, phys, va, k);
                    return Err(match e {
                        paging::MapError::OutOfPageFrames => Error::OutOfMemory,
                        // misaligned, already mapped, unknown space
                        _ => Error::BadPointer,
                    });
                }
            }
        }
        Ok(0)
    })
}

/// `AddressSpace::UNMAP` (milestone 95, DECISIONS §162 option A; the two semantics §162 left open
/// are provisional, and `notes/unmap.md` argues each): take the one page at `va` out of the space
/// `name`, out of every core's TLB, and out of the space's mapping record. `BadPointer` when nothing
/// is mapped there. No capability is read, consumed or changed. `#[inline(never)]` for the reason
/// `memory_region_map` gives: giving up a window is setup work, never a step of the IPC round trip.
///
/// **Under the space registry and then the mapping hold, the order `MAP_INTO` takes them**
/// (`ADDRESS_SPACES` above `MAPPINGS`), so an `UNMAP` and a `MAP_INTO` of the same `va` are ordered
/// whole, and a revoke's unmap pass (which takes the same hold) either finds the record and unmaps
/// the page itself, or finds neither. The table walk and the record are changed in the same hold,
/// so no sweep can see one without the other.
///
/// **The TLB obligation is `mmu::unmap_user_at`'s**, the function every revoke already unmaps with:
/// `tlbi vaae1is` on aarch64 (every ASID, every core in the inner-shareable domain), a local
/// `sfence.vma` plus an SBI remote fence on riscv64, `invlpg` plus the NMI shootdown on x86_64.
#[inline(never)]
fn address_space_unmap(name: u64, va: u64) -> Result<i64, Error> {
    if !paging::is_user_page_va::<crate::arch::mmu::Format>(va) {
        return Err(Error::BadPointer);
    }
    crate::user::with_user_address_space(name, |space| {
        // A space that no longer resolves (its thread reaped, or its region reclaimed) has nothing
        // this capability can reach, which is what `MAP_INTO` answers for it too. A running space
        // does resolve since §249, and `unmap_user_at`'s flush is what reaches its thread's core.
        let Some(space) = space else {
            return Err(Error::BadPointer);
        };
        let root = space.root();
        let mut hold = crate::revoke::hold();
        // Both halves, and neither is allowed to short-circuit the other: a page in the tables with
        // no record (none should exist since 2026-09-21, when every route began recording) is
        // still a window to close, and a record with no page behind it is still a record a later
        // revoke would act on.
        let unmapped = mmu::unmap_user_at(root, va);
        let forgotten = hold.forget_mapping_at(root, va);
        if unmapped.is_none() && forgotten.is_none() {
            return Err(Error::BadPointer);
        }
        Ok(0)
    })
}

/// `PageFrame::MAP`: map the run of frames the capability in `slot` names at consecutive pages
/// starting at `va` in the caller's own address space (`a1` writable 0/1, `a2` an untyped slot the
/// page tables come from). Un-share is `page_frame_revoke`; this is the other half.
/// `#[inline(never)]` for the reason `memory_region_map` gives.
///
/// §102: `count` is fixed on the capability, not passed here, so this is one `MAP` call regardless
/// of the run's length; a single-page frame (`count: 1`) runs the loop below once, exactly the
/// pre-§102 behavior.
///
/// **It takes the slot, not the capability `invoke` dispatched on** (the map-revocation-window
/// lane, 2026-10-04 UTC). That read was a critical section of its own, and a sweep could run
/// wholly between it and the record: the capability was deleted, the unmap pass scanned a log the
/// mapping was not yet in, and the mapping survived `PageFrame::REVOKE` and `MemoryRegion::DESTROY`
/// alike. `system_tests::user::map_revocation_window_tests` drove it. The frame is now read again
/// under the [`MappingHold`](crate::revoke::MappingHold) that maps and records it, and that hold
/// has the argument. A slot that holds something other than a `PageFrame` by then answers
/// `NoSuchSlot`: it was emptied in between, and an empty slot is what a `MAP` made at that instant
/// would have found.
#[inline(never)]
fn page_frame_map(slot: u64, va: u64, writable: u64, ut_slot: u64) -> Result<i64, Error> {
    #[cfg(feature = "system_tests")]
    crate::delegation_pause::here(); // no lock held
    let mut hold = crate::revoke::hold();
    let frame = hold.current_cap(slot).map_err(|_| Error::NoSuchSlot)?;
    let Object::PageFrame(phys, count) = frame.object else {
        return Err(Error::NoSuchSlot);
    };
    let count = count.get();
    // Checked against the run's last page, not just its first: a `va` that only overflows partway
    // through the run must be refused before anything is mapped, the same "reject the cheap
    // failures before spending a page" discipline `memory_region_map` documents.
    let Some(last_va) = run_end_va(va, count) else {
        return Err(Error::BadPointer);
    };
    if !paging::is_user_page_va::<crate::arch::mmu::Format>(va)
        || !paging::is_user_page_va::<crate::arch::mmu::Format>(last_va)
    {
        return Err(Error::BadPointer);
    }
    // A read/write mapping needs WRITE on the frame; a read-only one needs READ. This is where a
    // delegated, narrowed frame is confined: a peer handed READ alone can map it to look, never
    // to change it. One check for the whole run: rights live on the capability, not per page.
    let flags = if writable != 0 {
        if !frame.rights.allows(Rights::WRITE) {
            return Err(Error::NotPermitted);
        }
        paging::Flags::user_data()
    } else {
        if !frame.rights.allows(Rights::READ) {
            return Err(Error::NotPermitted);
        }
        paging::Flags::user_rodata()
    };
    // Page tables come from an untyped the caller holds, so mapping a frame, like everything a
    // process spends, comes out of its own budget and not the kernel's. Read under the hold only
    // because it sits here in the refusal order: a region is generational (§16 (object revocation)), so one destroyed
    // after this read refuses to retype, and the hold is not what protects it.
    let ut = sched::current_cap(ut_slot).map_err(|_| Error::NoSuchSlot)?;
    let Object::MemoryRegion(region) = ut.object else {
        return Err(Error::WrongObject);
    };
    if !ut.rights.allows(Rights::WRITE) {
        return Err(Error::NotPermitted);
    }
    let root = mmu::current_user_root();
    for k in 0..count {
        let (page_phys, page_va) = (phys + k * paging::PAGE_SIZE, va + k * paging::PAGE_SIZE);
        // Each table is recorded against this space as it is retyped, because the region it comes
        // from need not be the space's own, and its `DESTROY` has to find the table to cut it out
        // of this walk before the page goes back (`revoke::revoke_region`).
        match mmu::map_current_user_page_frame(page_va, page_phys, flags, || {
            hold.retype_table(region, root, page_va)
        }) {
            Ok(()) => {
                // Record the mapping so a later REVOKE (or memory_region::destroy) can pull this
                // page out of every holder before it is reused (§13). Unrecordable means
                // unmappable, at the mapper's own expense (phase C): see MemoryRegion::MAP.
                // `phys` (the run's base, not `page_phys`) is the object: it names the capability
                // this mapping was made under, derivatives included, which is what lets a later
                // `REVOKE` take back this authority without touching an overlapping holder's
                // (DECISIONS §132).
                if !hold.record_mapping(
                    page_phys,
                    root,
                    page_va,
                    crate::revoke::PageMapSource::Capability(phys),
                ) {
                    mmu::unmap_user_at(root, page_va);
                    unmap_run_prefix(&mut hold, root, phys, va, k);
                    return Err(Error::OutOfMemory);
                }
            }
            // **All or nothing across the run** (milestone 142's review, MAJOR 2): see the same
            // rollback at `MAP_INTO`, which fails the same way for the same reasons.
            Err(e) => {
                unmap_run_prefix(&mut hold, root, phys, va, k);
                return Err(match e {
                    paging::MapError::OutOfPageFrames => Error::OutOfMemory,
                    // misaligned, already mapped, or wrong half
                    _ => Error::BadPointer,
                });
            }
        }
    }
    Ok(0)
}

/// The **last** virtual address a `count`-page run starting at `va` covers, or `None` if the run
/// does not fit in a `u64`.
///
/// Both mapping paths guard on this, and both used to compute it inline as
/// `va.checked_add((count - 1) * paging::PAGE_SIZE)`, where only the *addition* was checked: the
/// multiply was not, so a `count` large enough to wrap `(count - 1) * 4096` back around to a small
/// number produced a `last_va` near `va` and the guard cheerfully passed a run that spans the
/// address space (milestone 142's review, MAJOR 4). `count` is a `NonZeroU64` on the capability
/// now, so the subtraction cannot underflow; this closes the other half.
fn run_end_va(va: u64, count: u64) -> Option<u64> {
    va.checked_add(count.checked_sub(1)?.checked_mul(paging::PAGE_SIZE)?)
}

/// **The first proofs over the kernel's own source** (milestone 193).
///
/// Every other `#[kani::proof]` in this tree sits in a crate under `crates/`, because until this
/// milestone `script/verify` could not reach `kernel/src` at all: its own header says
/// `cargo kani -p <crate>` never compiles the kernel. Milestone 191 measured what that cost, and
/// the answer was every defect the corpus actually had. This module is the other side of that
/// door, and `notes/kernel-proofs.md` is what a reader should open first, because a proof over
/// this crate carries stubs that a proof over a pure crate does not.
///
/// # What is stubbed, and therefore what these proofs do NOT say
///
/// The list is short and it is exhaustive, which is the only reason it is worth writing down:
///
/// - **Global assembly is skipped**, by `--ignore-global-asm` in `script/verify`. That is the boot
///   entry, the vector table and the context switch on all three architectures. Nothing below
///   reaches any of it, and nothing below claims anything about it.
/// - **`asm!` is an unsupported construct to Kani**, so every function that reaches one is
///   unverifiable rather than verified: all of `kernel/src/arch/`, plus `cpu.rs` once and `user.rs`
///   twice. If a harness ever calls into that code Kani reports the construct rather than proving
///   past it, which is the failure direction we want.
/// - **The panic handler is absent under `cfg(kani)`** (`panic.rs` says why: Kani links `std`,
///   which already defines the lang item). Nothing here says what the kernel does after a panic.
///
/// The functions proved below touch none of that. They are integer arithmetic on words a user
/// program chose, which is exactly where milestone 142's review found four defects by reading.
#[cfg(kani)]
mod proofs {
    use super::*;

    /// The page size the run arithmetic steps by, spelled once so the harnesses below and
    /// `run_end_va` cannot drift apart.
    const PAGE: u128 = paging::PAGE_SIZE as u128;

    /// **`run_end_va` is exact, and it refuses exactly the runs that do not fit.**
    ///
    /// This is milestone 142's MAJOR 4 written as a property. The code it replaced computed
    /// `va.checked_add((count - 1) * paging::PAGE_SIZE)` and checked only the addition, so a
    /// `count` big enough to wrap the *multiply* produced a `last_va` sitting a few pages above
    /// `va`, and the guard at both call sites cheerfully admitted a run that spans the address
    /// space. A unit test cannot find that: the witnesses are a measure-zero set of `count`s near
    /// multiples of 2^52, and nobody writes them down. The solver enumerates the whole domain.
    ///
    /// The claim is stated against `u128` arithmetic, which cannot wrap in this range, so the
    /// harness does not repeat the implementation's own expression back to it.
    ///
    /// Falsification: replayable `kernel/falsifications/syscall.proofs.the_run_end_is_exact_and_refuses_exactly_what_does_not_fit.patch`
    #[kani::proof]
    fn the_run_end_is_exact_and_refuses_exactly_what_does_not_fit() {
        let va: u64 = kani::any();
        let count: u64 = kani::any();
        // `count` rides on the capability as a `NonZeroU64` (cap.rs, DECISIONS §102), so zero is
        // not a state a caller can reach. Assuming it here rather than proving it is honest: the
        // type is the mechanism, and this harness is about the arithmetic above it.
        kani::assume(count >= 1);

        let want = va as u128 + (count as u128 - 1) * PAGE;

        match run_end_va(va, count) {
            Some(last) => {
                assert!(
                    last as u128 == want,
                    "the last page of the run is va + (count - 1) * PAGE_SIZE, with no wrap"
                );
                assert!(last >= va, "the run cannot end below where it starts");
            }
            None => assert!(
                want > u64::MAX as u128,
                "a run that fits in the address space must not be refused"
            ),
        }
    }

    /// **Checking the two ends of a run is enough to check every page in it.**
    ///
    /// Both mapping paths (`page_frame_map` and `MAP_INTO`) guard `va` and `run_end_va(va, count)`
    /// and then walk `va + k * PAGE_SIZE` for `k` in `0..count` with **nothing re-checking the
    /// pages in between**. That is milestone 142's MAJOR 3 in its fixed form, and it is only sound
    /// because the user half is a contiguous prefix of the address space: on aarch64
    /// `is_user_page_va` is `va & 0xfff == 0 && va >> 48 == 0`, so an aligned address between two
    /// user addresses is itself a user address.
    ///
    /// Sound, but not obviously so, and the loop is where a future widening of `Half::Low` would
    /// break it silently. `k` is chosen by the solver rather than iterated, so this covers every
    /// page of every run without an unwind bound.
    ///
    /// **The guard below is a model of two call sites, and a model can go stale.** Milestone
    /// 213's sweep read `page_frame_map` and `MAP_INTO` on 2026-09-02 and both apply exactly
    /// this: `run_end_va(va, count)`, then `is_user_page_va` on `va` and on `last_va`, then the
    /// loop. So the model is faithful today, checked rather than assumed. Nothing checks it
    /// tomorrow: a call site that added an alignment condition, or dropped one of the two ends,
    /// would leave this harness green while proving a claim about a guard nobody applies. Unlike
    /// the harnesses 213 rewrote, the duplication here cannot be removed by calling something,
    /// because what is duplicated is a *caller's* control flow rather than a function.
    ///
    /// Falsification: replayable `kernel/falsifications/syscall.proofs.every_page_between_the_checked_ends_is_itself_a_user_page.patch`
    #[kani::proof]
    fn every_page_between_the_checked_ends_is_itself_a_user_page() {
        let va: u64 = kani::any();
        let count: u64 = kani::any();
        let k: u64 = kani::any();
        kani::assume(count >= 1);
        kani::assume(k < count);

        // The guard the call sites apply, verbatim.
        let Some(last_va) = run_end_va(va, count) else {
            return;
        };
        kani::assume(paging::is_user_page_va::<crate::arch::mmu::Format>(va));
        kani::assume(paging::is_user_page_va::<crate::arch::mmu::Format>(last_va));

        // What the loop body computes. In a release build this is wrapping arithmetic; the claim
        // is that it never gets the chance to wrap.
        let offset = k as u128 * PAGE;
        assert!(
            va as u128 + offset <= last_va as u128,
            "no page of the run is computed past the end the guard checked"
        );
        let page_va = va + k * paging::PAGE_SIZE;
        assert!(
            paging::is_user_page_va::<crate::arch::mmu::Format>(page_va),
            "every page the map loop touches is an aligned user-half page"
        );
    }
}

/// **Undo the `mapped` pages a failed run-map had already established**, in the space rooted at
/// `root`: unmap each page and tombstone its revocation record, leaving the space exactly as the
/// call found it.
///
/// The rollback the pre-§102 code got for free by mapping one page. A partially mapped run is
/// worse than a failed one: the caller is told `OutOfMemory` or `BadPointer` with no way to ask
/// how much of its request survived, and a shared surface half-mapped is a peer reading pixels
/// that are not there.
///
/// # BUGS
///
/// **The page tables the mapped prefix retyped are not returned**, so a failed multi-page map
/// still costs the caller whatever L3s (and their parents) the prefix needed, permanently. A
/// region is spend-only (`MemoryRegion::RETYPE` never un-retypes), so giving them back is not a
/// matter of calling something; it is the reverse of the model. The mapping is undone, the budget
/// is not. Recorded in notes/frames.md.
fn unmap_run_prefix(
    hold: &mut crate::revoke::MappingHold,
    root: u64,
    phys: u64,
    va: u64,
    mapped: u64,
) {
    for k in 0..mapped {
        let (page_phys, page_va) = (phys + k * paging::PAGE_SIZE, va + k * paging::PAGE_SIZE);
        mmu::unmap_user_at(root, page_va);
        hold.forget_mapping(page_phys, root, page_va);
    }
}

/// `PageFrame::SLICE` (milestone 599, calef's option-4 ruling of 2026-09-27): a capability naming
/// the `len` pages `first` pages into this run, with the same rights. `abi::page_frame::SLICE` has
/// the rules; `#[inline(never)]` for the reason `memory_region_map` gives, since slicing is
/// spawn-time wiring and never a step of the IPC round trip.
#[inline(never)]
fn page_frame_slice(slot: u64, first: u64, len: u64) -> Result<i64, Error> {
    // The source is re-read under the table lock the slice is filed under (`sched::grant_derived`),
    // not taken from the capability `invoke` dispatched on: that read was a critical section of its
    // own, and a reclamation sweep could delete the run between it and the filing, leaving a slice
    // of pages the allocator was about to reuse (`sched::Delegation`). The source that matters is the
    // one standing when the slice is filed.
    #[cfg(feature = "system_tests")]
    crate::delegation_pause::here(); // no lock held
    let slot = sched::grant_derived(slot, |src| {
        let Object::PageFrame(phys, count) = src.object else {
            return Err(Error::WrongObject);
        };
        if !src.rights.allows(Rights::GRANT) {
            return Err(Error::NotPermitted);
        }
        let (Some(end), Some(len)) = (first.checked_add(len), core::num::NonZeroU64::new(len))
        else {
            return Err(Error::BadPointer);
        };
        if end > count.get() {
            return Err(Error::BadPointer);
        }
        let base = phys + first * page_frames::FRAME_SIZE;
        Ok(crate::cap::page_frame_run_cap(base, len, src.rights))
    })?;
    Ok(slot as i64)
}

/// `PageFrame::REVOKE`: un-share the run of `count` frames starting at `phys` from every holder and
/// delete every capability naming the run, including the caller's own. Does not reclaim the pages
/// (untyped is spend-only); that is `MemoryRegion::DESTROY`. §13, §102.
/// `#[inline(never)]` for the reason `memory_region_map` gives.
#[inline(never)]
fn page_frame_revoke(phys: u64, count: u64) -> Result<i64, Error> {
    crate::revoke::revoke_page_frame_run(phys, count);
    Ok(0)
}

/// `AddressSpace::WAIT` and `WAKE`: the checks every futex method shares, then the scheduler.
/// The order of the refusals is the contract `abi::address_space::WAIT` states: the flags first
/// (`BadMethod`, a form this kernel does not answer), then the right (`NotPermitted`), the address
/// (`BadPointer`), and last whether the space is the caller's own (`WrongObject`).
#[inline(never)]
fn address_space_futex(
    rights: Rights,
    space: u64,
    method: u64,
    va: u64,
    flags: u64,
    word: u64,
) -> Result<i64, Error> {
    if !abi::futex::admitted(flags) {
        return Err(Error::BadMethod);
    }
    if !rights.allows(Rights::READ) {
        return Err(Error::NotPermitted);
    }
    if !va.is_multiple_of(4)
        || !<crate::arch::mmu::Format as paging::PageFormat>::is_in_half(paging::Half::Low, va)
    {
        return Err(Error::BadPointer);
    }
    if !sched::is_current_space(space) {
        return Err(Error::WrongObject);
    }
    if method == abi::address_space::WAIT {
        // The expected value is the low 32 bits; the size admitted is 32 (§269).
        sched::futex_wait(space, va, word as u32).map(|r| r as i64)
    } else {
        Ok(sched::futex_wake(space, va, word) as i64)
    }
}

/// `Irq::WAIT`: block on the endpoint the kernel routed this interrupt to. The interrupt arrives
/// as a message (`sched::irq_notify`), exactly like any other. `#[inline(never)]` for the reason
/// `memory_region_map` gives.
#[inline(never)]
fn irq_wait(frame: &mut TrapFrame, intid: u32) -> Result<i64, Error> {
    let ep = sched::irq_route(intid).ok_or(Error::WrongObject)?;
    let m = sched::ipc_receive(ep);
    // A driver with a bound notification (milestone 151) is woken here by either the interrupt
    // (`1` in x0, x1 and x4 zero) or the notification (`abi::notification::BOUND` in x0 and x4, the
    // word in x1). This is what lets a driver wait on its device and a deadline at once, which is
    // the complaint milestone 106 (a wait that ends on either the interrupt or the deadline) makes about `net_stack`. Unconditional stores: an interrupt's mailbox has
    // zeros in both, so a driver that never bound anything reads the same zeros it would have.
    frame.set_arg(1, m[1]);
    frame.set_arg(4, m[4]);
    Ok(m[0] as i64)
}

/// `Irq::ACK`: re-enable the interrupt at the controller. The kernel masked it when it fired; now
/// that the driver has serviced the device, it is safe to let it fire again. This names
/// `arch::irq`, not a specific controller: the GIC on aarch64, the PLIC on RISC-V.
/// `#[inline(never)]` for the reason `memory_region_map` gives.
#[inline(never)]
fn irq_ack(intid: u32) -> Result<i64, Error> {
    crate::arch::irq::enable(intid);
    Ok(0)
}

/// `Virtio`'s four register-level methods (`READ_REG`, `WRITE_REG`, `SETUP_QUEUE`, `NOTIFY`): a
/// driver's transport-level plumbing, not a step of the IPC round trip. `#[inline(never)]` for
/// the reason `memory_region_map` gives.
#[inline(never)]
fn virtio_invoke(id: usize, method: u64, a0: u64, a1: u64) -> Result<i64, Error> {
    use crate::virtio::TransportError;
    let map = |e: TransportError| match e {
        TransportError::DmaEscape => Error::DeviceRefused,
        _ => Error::WrongObject,
    };
    match method {
        abi::virtio::READ_REG => crate::virtio::read_register(id, a0)
            .map(|v| v as i64)
            .ok_or(Error::WrongObject),
        abi::virtio::WRITE_REG => crate::virtio::write_register(id, a0, a1 as u32)
            .map(|_| 0)
            .map_err(map),
        abi::virtio::SETUP_QUEUE => crate::virtio::setup_queue(id, a0 as u16, a1 as u16)
            .map(|_| 0)
            .map_err(map),
        abi::virtio::NOTIFY => crate::virtio::notify(id, a0 as u16).map(|_| 0).map_err(map),
        _ => Err(Error::BadMethod),
    }
}

/// `Rendezvous::REAP`: collect a corpse this rendezvous supervises (DECISIONS §32), the control
/// half `rendezvous::SURVEY` is the view half of. Administrative, not the IPC round trip: rare
/// enough (one call per child death, not per message) that it moves out of `invoke`'s own bytes
/// with the rest of this arm's non-fastpath methods. `#[inline(never)]` for the reason
/// `memory_region_map` gives.
#[inline(never)]
fn rendezvous_reap(ep: crate::sched::RendezvousId, tid: u64) -> Result<i64, Error> {
    sched::reap_supervised(ep, tid)?;
    Ok(0)
}

/// The body of `abi::memory_region::USAGE` (milestone 126 (the `procps` package), `free`),
/// out of line and `#[inline(never)]` for [`address_space_list`]'s reason: `syscall_entry` is
/// measured flat, and a method only `free`, `vmstat` and `slabtop` call must not grow every
/// syscall's footprint. The first CI run with it inline measured `syscall_entry` 5.6% larger on
/// riscv64 and 8.8% on `x86_64`, over the 5% bound.
/// The rights check lives here too, `page_frame_map`'s shape, so the arm in `invoke` is one call.
#[inline(never)]
fn memory_region_usage(cap: crate::cap::Cap, region: u64, record: u64) -> Result<i64, Error> {
    if !cap.rights.allows(Rights::ENUMERATE) {
        return Err(Error::NotPermitted);
    }
    if !abi::usage::is_known(record) {
        return Err(Error::BadMethod);
    }
    crate::memory_region::usage_record(region, record)
        .map(|pages| pages as i64)
        .ok_or(Error::Gone)
}

/// The body of `abi::address_space::LIST` (milestone 126's `pmap`, DECISIONS §114), pulled out of
/// [`invoke`] and marked `#[inline(never)]` on purpose: `syscall_entry` is measured flat
/// (`script/fastpath-footprint`), so a rare administrative loop inlined into the hot dispatcher
/// grows every syscall's instruction footprint for a method almost nothing calls. One call-site's
/// worth of bytes in `invoke` costs far less than this loop's own bytes would.
#[inline(never)]
fn address_space_list(frame: &mut TrapFrame, name: u64, cursor: u64) -> Result<i64, Error> {
    // The capability names a registry entry by generation; once the space dies (its thread reaped
    // or its region destroyed, §249 (a running address space stays nameable)), the entry is gone
    // and `root` is `None` from here on for every capability that pointed at it. That is not a refusal (the capability is real and was never
    // widened past what it always held): it reads as an empty listing, symmetric to `SURVEY`'s
    // "before the scheduler exists there is no domain to report."
    let Some(root) = crate::user::user_address_space_root(name) else {
        frame.set_arg(1, 0);
        frame.set_arg(2, 0);
        return Ok(abi::survey::DONE as i64);
    };
    // The cursor is the caller's word, so it is checked before it is followed, in the same
    // SPACES hold that walks the chain (revoke::list_mapping): only a cursor this space's own
    // log minted may name a log page. Milestone 779 (fuzz the surface a confined process can
    // reach)'s confined fuzzer drew a random cursor here and the kernel walked it as a log page
    // (a data abort on a kernel-mapped address; anything the page held would have come back as
    // a mapping record), 2026-10-06 UTC.
    // Skip an entry whose `va` no longer translates (a race with revocation of a shared page this
    // space had mapped) rather than report a fabricated `kind` for it; bounded by the log's own
    // finite length, so this cannot loop forever.
    let mut cursor = cursor;
    loop {
        match crate::revoke::list_mapping(root, cursor) {
            crate::revoke::Listing::Done => {
                frame.set_arg(1, 0);
                frame.set_arg(2, 0);
                return Ok(abi::survey::DONE as i64);
            }
            // Only the caller's own word can be foreign: every cursor after the first is
            // minted by the arm below. BadPointer, not DONE, per DECISIONS §114 (`pmap` gets
            // its listing: `ENUMERATE` extends to the address-space object)'s 2026-10-06
            // addendum: a dead space's stale cursor is DONE, a live space's foreign word is
            // garbage, and the boundary answers garbage with BadPointer everywhere else.
            crate::revoke::Listing::ForeignCursor => return Err(Error::BadPointer),
            crate::revoke::Listing::Entry(next, va) => {
                if let Some((_, flags)) = crate::arch::mmu::translate_at(root, va) {
                    let kind = if flags.is_user_executable() {
                        abi::address_space::MAP_CODE
                    } else if flags.is_writable() {
                        abi::address_space::MAP_RW
                    } else {
                        abi::address_space::MAP_RO
                    };
                    frame.set_arg(1, va);
                    frame.set_arg(2, kind);
                    return Ok(next as i64);
                }
                cursor = next;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// **A run that fails partway through leaves nothing mapped** (milestone 142's review, MAJOR
    /// 2), driven through the real `MAP_INTO` handler.
    ///
    /// The regression: the pre-§102 single-page path rolled back on failure, and the widening
    /// narrowed that rollback to the one page that failed. So a three-page run whose second page
    /// could not be mapped returned `BadPointer` with its first page mapped and recorded, and the
    /// caller had no method to ask which. Silent partial state is worse than a refusal, and this is
    /// the assertion that says so: after the error, page 0 is not mapped.
    ///
    /// The failure is injected by occupying the run's middle virtual address first, so the loop's
    /// second iteration takes `AlreadyMapped`. That is the cheapest deterministic mid-run failure
    /// available; an exhausted page-table budget would do it too, and would depend on arithmetic
    /// about region sizes that has drifted before.
    #[test_case]
    fn a_partly_mapped_run_is_rolled_all_the_way_back() {
        let mut trap = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);

        let space_region = crate::memory_region::create(16).expect("no space region");
        let name = crate::user::user_address_space_create(space_region).expect("no address space");
        let root = crate::user::user_address_space_root(name).expect("no root");
        let space_slot = sched::grant(crate::cap::address_space_cap(name, Rights::ALL))
            .expect("grant the address space");

        let frame_region = crate::memory_region::create(8).expect("no frame region");
        let base = crate::memory_region::retype_page(frame_region).expect("retype 0");
        let second = crate::memory_region::retype_page(frame_region).expect("retype 1");
        let third = crate::memory_region::retype_page(frame_region).expect("retype 2");
        assert_eq!(second, base + paging::PAGE_SIZE, "run must be contiguous");
        assert_eq!(third, second + paging::PAGE_SIZE, "run must be contiguous");
        let frame_slot = sched::grant(crate::cap::page_frame_run_cap(
            base,
            crate::cap::page_frame_run_len(3),
            Rights::ALL,
        ))
        .expect("grant the run");

        // Occupy the middle of where the run wants to go, out of a page the run does not name.
        let va = 0x40_0000u64;
        let squatter = crate::memory_region::retype_page(frame_region).expect("retype squatter");
        crate::user::user_address_space_map(
            name,
            va + paging::PAGE_SIZE,
            squatter,
            paging::Flags::user_data(),
            crate::revoke::PageMapSource::NoCapability,
        )
        .expect("the squatter maps");

        let outcome = invoke(
            &mut trap,
            space_slot,
            abi::address_space::MAP_INTO,
            va,
            frame_slot,
            abi::address_space::MAP_RW,
        );
        assert_eq!(
            outcome,
            Err(Error::BadPointer),
            "MAP_INTO over an occupied page must refuse",
        );
        assert!(
            mmu::translate_at(root, va).is_none(),
            "a failed MAP_INTO left the run's first page mapped: silent partial state",
        );
        assert_eq!(
            mmu::translate_at(root, va + paging::PAGE_SIZE).map(|(phys, _)| phys),
            Some(squatter),
            "the rollback unmapped a page the failed call never mapped",
        );

        // Give everything back, so the free-frame baseline later tests measure is undisturbed. The
        // space's region is pinned by its root page, so it comes back through `reclaim_region`
        // (which reaps the space itself) rather than through `destroy`.
        let _ = sched::delete_current_cap(space_slot);
        let _ = sched::delete_current_cap(frame_slot);
        let _ = sched::reclaim_region(space_region);
        crate::memory_region::destroy(frame_region);
    }

    /// **A slice maps only its window** (milestone 599, calef's option-4 ruling of 2026-09-27).
    /// The progenitor's pool is one capability over every client's window; `SLICE` is what lets it
    /// hand a client exactly one. Through the real handlers: slice page 2 of a four-page pool,
    /// `MAP_INTO` a space with the slice, and the space holds that page and none of its
    /// neighbours. Then the refusals: out of range, empty, and without `GRANT`.
    #[test_case]
    fn a_slice_maps_only_its_window() {
        let mut trap = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
        let space_region = crate::memory_region::create(16).expect("no space region");
        let name = crate::user::user_address_space_create(space_region).expect("no address space");
        let root = crate::user::user_address_space_root(name).expect("no root");
        let space_slot = sched::grant(crate::cap::address_space_cap(name, Rights::ALL))
            .expect("grant the address space");

        let frame_region = crate::memory_region::create(4).expect("no frame region");
        let region_slot =
            sched::grant(crate::cap::memory_region_root_cap(frame_region)).expect("grant");
        let pool = invoke(&mut trap, region_slot, abi::memory_region::RETYPE, 4, 0, 0)
            .expect("a four-page pool") as u64;
        let Object::PageFrame(base, _) = sched::current_cap(pool).expect("the pool").object else {
            panic!("RETYPE must mint a PageFrame");
        };

        let slice = invoke(&mut trap, pool, abi::page_frame::SLICE, 2, 1, 0)
            .expect("page 2 of four is inside the pool") as u64;
        let cap = sched::current_cap(slice).expect("the slice");
        assert_eq!(
            cap.object,
            Object::PageFrame(
                base + 2 * paging::PAGE_SIZE,
                crate::cap::page_frame_run_len(1)
            ),
            "a slice names exactly the pages asked for",
        );

        // Map the slice, then look at the window and both neighbours' would-be addresses.
        let va = 0x40_0000u64;
        invoke(
            &mut trap,
            space_slot,
            abi::address_space::MAP_INTO,
            va,
            slice,
            abi::address_space::MAP_RW,
        )
        .expect("the slice maps");
        assert_eq!(
            mmu::translate_at(root, va).map(|(phys, _)| phys),
            Some(base + 2 * paging::PAGE_SIZE),
            "the slice mapped its own page",
        );
        for off in [va - paging::PAGE_SIZE, va + paging::PAGE_SIZE] {
            assert!(
                mmu::translate_at(root, off).is_none(),
                "a slice's mapping reached past its window",
            );
        }

        // Refusals: past the end, empty, overflowing, and a source without GRANT.
        for (first, len) in [(3, 2), (4, 1), (1, 0), (u64::MAX, 2)] {
            assert_eq!(
                invoke(&mut trap, pool, abi::page_frame::SLICE, first, len, 0),
                Err(Error::BadPointer),
                "slice ({first}, {len}) of a four-page pool",
            );
        }
        let narrow = sched::grant(crate::cap::page_frame_run_cap(
            base,
            crate::cap::page_frame_run_len(4),
            Rights::WRITE,
        ))
        .expect("a pool without GRANT");
        assert_eq!(
            invoke(&mut trap, narrow, abi::page_frame::SLICE, 0, 1, 0),
            Err(Error::NotPermitted),
            "slicing without GRANT",
        );

        for s in [space_slot, slice, narrow, pool, region_slot] {
            let _ = sched::delete_current_cap(s);
        }
        let _ = sched::reclaim_region(space_region);
        crate::memory_region::destroy(frame_region);
    }

    /// **`RETYPE` mints a run, `0` still means one page, and a run that does not fit moves
    /// nothing** (calef's ruling of 2026-09-26, option A of
    /// design/roadmap/0659-a-region-retypes-a-frame-run.md). Through the real handler, so the
    /// argument reaches the proved arithmetic and the capability names the whole run.
    #[test_case]
    fn retype_mints_a_run_and_a_refused_run_moves_nothing() {
        let mut trap = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
        let region = crate::memory_region::create(4).expect("a region to retype from");
        let slot = sched::grant(crate::cap::memory_region_root_cap(region)).expect("grant it");

        let run = invoke(&mut trap, slot, abi::memory_region::RETYPE, 3, 0, 0)
            .expect("three pages fit in four") as u64;
        let Object::PageFrame(phys, count) = sched::current_cap(run).expect("the run's cap").object
        else {
            panic!("RETYPE must mint a PageFrame");
        };
        assert_eq!(count.get(), 3, "one capability names the whole run");
        // SAFETY: the run's pages are ours, retyped above; reading the direct map is how the zeroing
        // is observed rather than assumed.
        let zeroed = (0..3 * paging::PAGE_SIZE).step_by(512).all(|off| unsafe {
            core::ptr::read_volatile((mmu::phys_to_virt(phys) + off) as *const u64) == 0
        });
        assert!(
            zeroed,
            "every page of the run must be zeroed, not only the first"
        );
        assert_eq!(crate::memory_region::usage(region), Some((3, 4)));

        assert_eq!(
            invoke(&mut trap, slot, abi::memory_region::RETYPE, 2, 0, 0),
            Err(Error::OutOfMemory),
            "two pages do not fit in one",
        );
        assert_eq!(
            crate::memory_region::usage(region),
            Some((3, 4)),
            "a refused run moved the region's watermark",
        );

        let one = invoke(&mut trap, slot, abi::memory_region::RETYPE, 0, 0, 0)
            .expect("the last page, asked for the way every existing caller asks")
            as u64;
        let Object::PageFrame(last, one_count) = sched::current_cap(one).expect("cap").object
        else {
            panic!("RETYPE must mint a PageFrame");
        };
        assert_eq!(one_count.get(), 1, "0 means one page");
        assert_eq!(
            last,
            phys + 3 * paging::PAGE_SIZE,
            "the watermark is contiguous"
        );

        let _ = sched::delete_current_cap(run);
        let _ = sched::delete_current_cap(one);
        let _ = sched::delete_current_cap(slot);
        crate::memory_region::destroy(region);
    }

    /// **`SPLIT` never widens rights: a spend-only untyped splits into spend-only children.** SPLIT
    /// gates only on `WRITE`, and mints a fresh capability to the child budget, so it must honor the
    /// derive-never-widens invariant by hand or a process could manufacture authority it was denied:
    /// hold a deliberately `GRANT`-less untyped, `SPLIT` it, and receive a `GRANT`-bearing child over
    /// the same memory, then delegate what its own capability could not. This drives the real syscall
    /// path (not the region-level `memory_region::split`, which carries no rights) and pins the child's
    /// rights to the parent capability's, at the mint site the Kani proofs do not cover. The contrast
    /// arm shows the delegable root does pass `GRANT` down, so the inheritance is real, not a blanket
    /// deny. Milestone 31; see DECISIONS §16 amendment.
    #[test_case]
    fn split_inherits_the_parent_capabilitys_rights_never_widening() {
        // `for_user_entry` is the portable frame constructor (both ISAs); SPLIT does not read it.
        let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);

        // A spend-only (WRITE, no GRANT) untyped, exactly what a leaf child is handed.
        let spend_only_region = crate::memory_region::create(8).expect("a region to split");
        let parent =
            sched::grant(crate::cap::memory_region_cap(spend_only_region)).expect("grant parent");
        assert!(
            !sched::current_cap(parent)
                .unwrap()
                .rights
                .allows(Rights::GRANT),
            "the parent capability must lack GRANT for this test to mean anything",
        );

        // SPLIT through the real handler. WRITE permits it; the child must inherit WRITE only.
        let child_slot = invoke(&mut frame, parent, abi::memory_region::SPLIT, 2, 0, 0)
            .expect("split succeeds: the parent holds WRITE") as u64;
        let child = sched::current_cap(child_slot).expect("the child capability exists");
        assert!(
            !child.rights.allows(Rights::GRANT),
            "escalation: a GRANT-less untyped split into a GRANT-bearing child",
        );
        // Because it lacks GRANT it cannot be delegated: SEND_CAP and CAP_INSERT both refuse without
        // it (the exact gate at syscall.rs lines ~143 and ~319), so the child is non-transferable.
        assert!(
            !child.rights.allows(Rights::GRANT),
            "a spend-only child must be un-delegatable",
        );
        let Object::MemoryRegion(child_region) = child.object else {
            panic!("SPLIT must mint an untyped");
        };

        // Contrast: the delegable root (READ|WRITE|GRANT, what the progenitor holds) passes GRANT to its
        // children, so a spawner can hand a budget on. Inheritance, not a blanket deny.
        let root_region = crate::memory_region::create(8).expect("a root region");
        let root =
            sched::grant(crate::cap::memory_region_root_cap(root_region)).expect("grant root");
        let root_child_slot = invoke(&mut frame, root, abi::memory_region::SPLIT, 2, 0, 0)
            .expect("split the root") as u64;
        let root_child = sched::current_cap(root_child_slot).expect("root child exists");
        assert!(
            root_child.rights.allows(Rights::GRANT),
            "a delegable root must split into delegable children",
        );
        let Object::MemoryRegion(root_child_region) = root_child.object else {
            panic!("SPLIT must mint an untyped");
        };

        // Clean up: reclaim children (LIFO top of each parent) then parents, and drop the cap slots,
        // so the test returns every frame it borrowed and leaves the test thread's capability table as it found
        // it (the free-frame baseline that later tests measure against).
        sched::reclaim_region(child_region).expect("reclaim the spend-only child");
        sched::reclaim_region(spend_only_region).expect("reclaim the spend-only parent");
        sched::reclaim_region(root_child_region).expect("reclaim the root child");
        sched::reclaim_region(root_region).expect("reclaim the root parent");
        for slot in [parent, child_slot, root, root_child_slot] {
            let _ = sched::delete_current_cap(slot);
        }
    }

    /// **A `SPLIT` refused for a full capability table leaves the parent as it found it.** The
    /// child is minted before its capability, so a caller whose table has no free slot used to get
    /// `OutOfMemory` with an orphaned child still live: a region slot nobody could name, and a
    /// parent that `reclaim_region` refused for the rest of the boot. The last assertion is the
    /// one that failed before the fix; the `usage` check pins that the carve was given back too,
    /// not merely uncounted. Milestone 601 (the region table prints its peak).
    #[test_case]
    fn split_refused_for_a_full_capability_table_orphans_no_child() {
        let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
        let region = crate::memory_region::create(8).expect("a region to split");
        let parent = sched::grant(crate::cap::memory_region_root_cap(region)).expect("grant");

        // Fill every remaining slot. Bounded by the table's capacity, which `grant` enforces.
        let mut filler = [0u64; crate::cap::CAPABILITY_TABLE_SLOTS];
        let mut filled = 0;
        while let Ok(slot) = sched::grant(crate::cap::memory_region_root_cap(region)) {
            filler[filled] = slot;
            filled += 1;
        }

        let refused = invoke(&mut frame, parent, abi::memory_region::SPLIT, 2, 0, 0);
        assert_eq!(
            refused,
            Err(Error::OutOfMemory),
            "no slot for the child's capability"
        );
        assert!(
            !crate::memory_region::has_children(region),
            "the refused split left a child nobody holds a capability to",
        );
        assert_eq!(
            crate::memory_region::usage(region),
            Some((0, 8)),
            "the child's pages went back to the parent",
        );

        for &slot in filler[..filled].iter().chain([&parent]) {
            let _ = sched::delete_current_cap(slot);
        }
        sched::reclaim_region(region).expect("the parent is reclaimable, as if SPLIT never ran");
    }
}
