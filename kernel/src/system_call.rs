//! The system call boundary. **Five calls <!--count:system_calls-->.**
//!
//! `exit`, `exit_thread`, `yield` and `capability_delete` are authority over the caller itself, and
//! `invoke(capability, method, a0, a1, a2)` is everything else, on a capability the caller was handed.
//! Every argument is a scalar in a register, so the kernel follows no user pointer. The ABI is
//! `partsfoundry/moho_abi`, which the kernel and every user program share.
//!
//! See nifeos/record:notes/system-call-boundary.md, and DECISIONS §10 (process model: capability-based,
//! microkernel) for the choice this surface carries out.

use moho_abi::Error;

use crate::arch::exceptions::TrapFrame;
use crate::capability_object::{Capability, Object, Rights};
use crate::sched;

// The methods, one child module per object family, for §282 (when a file is split, the parent
// becomes an index).
mod address_space;
mod memory_region;
mod object_methods;
mod page_frame;
mod rendezvous;
mod retype;
mod thread_control_block;
use address_space::address_space_invoke;
use memory_region::memory_region_invoke;
#[cfg(target_arch = "x86_64")]
use object_methods::port_range_invoke;
use object_methods::{
    aborted_send_error, interrupt_acknowledge, interrupt_wait, notification_invoke, process_invoke,
    reboot_invoke, rendezvous_badge, rendezvous_reap, reply_other, timer_invoke, virtual_io_invoke,
};
use page_frame::{page_frame_map, page_frame_revoke, page_frame_slice};
use rendezvous::{
    rendezvous_call, rendezvous_receive, rendezvous_receive_capability, rendezvous_send,
    rendezvous_send_capability, rendezvous_survey,
};
use thread_control_block::thread_control_block_invoke;

// Index exception (2026-10-12): `script/fastpath-footprint` finds the system call entry by this
// path, `kernel11system_call8dispatch` in the mangled name, and counts it flat as `system_call_entry`.
/// Decode and run one system call, and write its result to the first argument register.
///
/// Called from the `svc` arm of `exception_body` (`ecall` on riscv64, in `riscv64_trap_body`). A
/// system call stays on the thread's own stack, not the interrupt stack, because it can block.
///
/// `#[inline(never)]` keeps this a symbol `script/fastpath-footprint` can name and sum flat as
/// `system_call_entry`; nifeos/record:notes/system-call-boundary.md has what an inlining flip does to that sum.
#[inline(never)]
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(
    target_os = "none",
    unsafe(link_section = ".text.hot.system_call.dispatch")
)]
pub(crate) fn dispatch(frame: &mut TrapFrame) {
    // Through the frame's accessors: each architecture puts the number and arguments in
    // different registers.
    let nr = frame.system_call_nr();
    // Read now: a result may overwrite the method register before the stack-depth sample.
    #[cfg(any(test, feature = "ipc_stack_depth"))]
    let method = frame.arg(1);

    // `exit` never comes back, so it is not part of the result-writing path below.
    if nr == moho_abi::SYSTEM_CALL_EXIT {
        sched::exit();
    }
    // Ends this thread only, and like `exit` never comes back.
    if nr == moho_abi::SYSTEM_CALL_EXIT_THREAD {
        sched::exit_thread(frame.arg(0));
    }

    let result: Result<i64, Error> = match nr {
        moho_abi::SYSTEM_CALL_YIELD => {
            sched::yield_now();
            Ok(0)
        }
        // An empty slot is not an error: a loader recycling slots need not track emptiness.
        moho_abi::SYSTEM_CALL_CAPABILITY_DELETE => {
            let _ = sched::delete_current_capability(frame.arg(0));
            Ok(0)
        }
        moho_abi::SYSTEM_CALL_INVOKE => invoke(
            frame,
            frame.arg(0),
            frame.arg(1),
            frame.arg(2),
            frame.arg(3),
            frame.arg(4),
        ),
        _ => Err(Error::BadSystemCall),
    };

    // The trap-restore path pops the frame into the user's registers.
    frame.set_arg(
        0,
        match result {
            Ok(v) => v as u64,
            Err(e) => (e as i64) as u64,
        },
    );

    // Last, so the sample covers the whole system call; one relaxed load for every thread the
    // instrument is not measuring.
    #[cfg(any(test, feature = "ipc_stack_depth"))]
    crate::ipc_stack_depth::after_system_call(nr, method);
}

// Index exception (2026-10-12): the decoder inlines into `dispatch` above, and moving it to a
// child would let a codegen-unit split change what `system_call_entry` measures.
/// Invoke `method` on the capability in the caller's `slot`, with arguments `a0`, `a1` and `a2`.
///
/// The lookup is the security mechanism: `slot` indexes this thread's own table in kernel memory,
/// and an empty slot is `NoSuchSlot` (there is nothing there), not a refusal. Then the object's
/// rights checks, then the method. Returns the method's result word or its `moho_abi::Error`.
///
/// Public so kernel tests drive this path, not a copy of it (`user/reap_tests.rs` does). See
/// nifeos/record:notes/system-call-boundary.md.
pub fn invoke(
    frame: &mut TrapFrame,
    slot: u64,
    method: u64,
    a0: u64,
    a1: u64,
    a2: u64,
) -> Result<i64, Error> {
    let capability = sched::current_capability(slot).map_err(|_| Error::NoSuchSlot)?;

    match capability.object {
        Object::Rendezvous(ep, badge) => match method {
            // `#[inline(always)]` in `rendezvous`, so the round trip stays inside `invoke`.
            moho_abi::rendezvous::SEND => {
                rendezvous_send(capability.rights, ep, badge, [a0, a1, a2])
            }
            moho_abi::rendezvous::RECEIVE => rendezvous_receive(frame, capability.rights, ep),
            moho_abi::rendezvous::SEND_CAPABILITY => {
                rendezvous_send_capability(capability.rights, ep, badge, [a0, a1, a2])
            }
            moho_abi::rendezvous::RECEIVE_CAPABILITY => {
                rendezvous_receive_capability(frame, capability.rights, ep)
            }
            moho_abi::rendezvous::CALL => {
                rendezvous_call(frame, capability.rights, ep, badge, [a0, a1])
            }

            // READ, the right to receive deaths here, which a send-only peer lacks (DECISIONS §32
            // (a supervisor may collect a corpse without being able to build one)).
            moho_abi::rendezvous::REAP => {
                if !capability.rights.allows(Rights::READ) {
                    return Err(Error::NotPermitted);
                }
                rendezvous_reap(ep, a0)
            }
            moho_abi::rendezvous::SURVEY => rendezvous_survey(frame, capability.rights, ep, a0, a1),
            moho_abi::rendezvous::BADGE => rendezvous_badge(ep, capability.rights, badge, a0),
            _ => Err(Error::BadMethod),
        },

        // §12 (Call/Reply IPC: a one-shot reply capability).
        Object::Reply(tid) => match method {
            moho_abi::reply::REPLY => {
                // Minted WRITE-only, so a narrowed copy cannot answer, and without GRANT, so it
                // cannot have been delegated here.
                if !capability.rights.allows(Rights::WRITE) {
                    return Err(Error::NotPermitted);
                }
                sched::ipc_reply(tid, [a0, a1]);
                // Consumed, so the caller cannot be answered twice.
                let _ = sched::delete_current_capability(slot);
                Ok(0)
            }
            // Every other method, `REPLY_CAPABILITY` and `BadMethod` alike, is decided out of line.
            _ => reply_other(slot, method, a0, a1, a2),
        },

        Object::AddressSpace(name) => {
            address_space_invoke(frame, capability, name, method, [a0, a1, a2])
        }

        Object::ThreadControlBlock(tid) => {
            thread_control_block_invoke(frame, capability.rights, tid, method, [a0, a1, a2])
        }

        Object::Notification(id) => notification_invoke(capability.rights, id, method, a0),

        Object::Timer(id) => timer_invoke(capability.rights, id, method, a0, a1, a2),

        Object::Process(pid) => process_invoke(capability.rights, pid, method),

        // No rights bit: holding the capability is the whole authority (DECISIONS §251 (restarting
        // the machine is a kernel object the progenitor hands out)).
        Object::Reboot => reboot_invoke(method),

        Object::MemoryRegion(region) => {
            memory_region_invoke(capability, region, method, [a0, a1, a2])
        }

        Object::PageFrame(phys, count) => match method {
            // `MAP` checks its rights inside `page_frame_map`, because they depend on `a1`.
            moho_abi::page_frame::MAP => page_frame_map(slot, a0, a1, a2),
            // Both `REVOKE` arms test `GRANT` here rather than through `Capability::delegate`,
            // because a revoke takes authority back and mints nothing. No harness covers the arm:
            // `allows` is proved in `capability_table`, and proving the arm itself would mean
            // stubbing the current thread, its table and the frame revoke, which the kernel
            // proofs do not stub. Its shape is the one-line rights test of every arm around it.
            moho_abi::page_frame::REVOKE => {
                if !capability.rights.allows(Rights::GRANT) {
                    return Err(Error::NotPermitted);
                }
                page_frame_revoke(phys, count.get())
            }
            moho_abi::page_frame::SLICE => page_frame_slice(slot, a0, a1),
            _ => Err(Error::BadMethod),
        },

        Object::DeviceFrame(phys) => match method {
            // Spares the invoker's own capability and mapping, unlike a frame revoke: the kernel
            // mints a `DeviceFrame` once (DECISIONS §41 (the endpoint is the broker, and a device
            // is revoked by taking it back); `revoke::revoke_device_from_others` has why).
            moho_abi::page_frame::REVOKE => {
                if !capability.rights.allows(Rights::GRANT) {
                    return Err(Error::NotPermitted);
                }
                crate::revoke::revoke_device_from_others(phys);
                Ok(0)
            }
            _ => Err(Error::BadMethod),
        },

        Object::VirtualIo(id) => {
            if !capability.rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            virtual_io_invoke(id, method, a0, a1)
        }

        Object::Interrupt(intid) => match method {
            moho_abi::interrupt::WAIT => {
                if !capability.rights.allows(Rights::READ) {
                    return Err(Error::NotPermitted);
                }
                interrupt_wait(frame, intid)
            }
            moho_abi::interrupt::ACKNOWLEDGE => {
                if !capability.rights.allows(Rights::READ) {
                    return Err(Error::NotPermitted);
                }
                interrupt_acknowledge(intid)
            }
            _ => Err(Error::BadMethod),
        },

        // `x86_64` only; the other two architectures' match is exhaustive without it.
        #[cfg(target_arch = "x86_64")]
        Object::PortRange(base, count) => port_range_invoke(capability.rights, base, count, method),
    }
}

/// Kani proofs over the kernel's own source: the run arithmetic both mapping paths guard with.
///
/// A proof over this crate runs with global assembly skipped, `asm!` unverifiable and no panic
/// handler; nifeos/record:notes/kernel-proofs.md lists what that leaves these proofs unable to say.
#[cfg(kani)]
mod proofs;

#[cfg(test)]
mod tests;
