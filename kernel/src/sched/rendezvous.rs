//! **Rendezvous**: creating one, and send, receive, call and reply on it, with capability transfer
//! and the stranding of callers whose server is gone. See notes/scheduler/rendezvous.md.
//!
//! Name: provisional (milestone 840's lane, 2026-10-11 UTC; §271 (the scheduler file splits into
//! submodules of `sched`)).

use super::*;

/// Why creating an rendezvous failed. The two causes need telling apart because they call for opposite
/// responses: a full region means carve more memory and retry, a full registry means give up.
pub(super) enum RendezvousFailure {
    /// The region has no page left to retype. Nothing was spent.
    RegionFull,
    /// The registry is at [`MAX_RENDEZVOUS`]. Checked *before* spending a page, so nothing was spent.
    RegistryFull,
}

/// Create an rendezvous **in `region`'s memory** (milestone 19a): one page retyped and pinned, the
/// rendezvous at its start, a fresh generational name in the registry. The shared engine of the
/// `RETYPE_OBJ` syscall and the kernel's own [`create_rendezvous`]. Returns the name, or which
/// limit refused it; a refusal spends nothing.
///
/// Context: takes `IPC_TABLES`, and `MEMORY_REGION` beneath it.
pub(super) fn try_create_rendezvous_from(region: u64) -> Result<RendezvousId, RendezvousFailure> {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().ok_or(RendezvousFailure::RegistryFull)?;

    // Checked before the retype, so a full registry spends no page.
    if sched.rendezvous_table.len() >= MAX_RENDEZVOUS {
        return Err(RendezvousFailure::RegistryFull);
    }

    // Rank: MEMORY_REGION (58) under IPC_TABLES (60) is a legal descent; the pin rides in the same lock
    // hold as the carve, so no destroy can race the page away (see retype_object_page).
    let phys = crate::memory_region::retype_object_page(
        region,
        crate::memory_region::ObjectKind::Rendezvous,
    )
    .ok_or(RendezvousFailure::RegionFull)?;

    // The page arrives zeroed, and an all-zero Rendezvous happens to be valid; write it explicitly
    // anyway, because "happens to be" is the kind of truth that stops being one silently.
    // SAFETY: fresh page, exclusively ours, direct-mapped.
    unsafe { (crate::arch::mmu::phys_to_virt(phys) as *mut Rendezvous).write(Rendezvous::new()) };

    // Cannot fail: capacity was checked above under this same lock hold.
    let name = sched
        .rendezvous_table
        .insert_with(|_| phys)
        .ok_or(RendezvousFailure::RegistryFull)?;
    PEAK_RENDEZVOUS.fetch_max(sched.rendezvous_table.len(), Ordering::Relaxed);
    if sched.kernel_ep_region == Some(region) {
        KERNEL_CHUNK_RENDEZVOUS.fetch_add(1, Ordering::Relaxed);
    }
    Ok(name)
}

/// Create an rendezvous in `region`'s memory. `None` when the region is out of budget or the registry
/// is full. The `RETYPE_OBJ` syscall's engine; userspace gets one flat failure because a process
/// cannot act on the difference (it holds one region and cannot enlarge the kernel's registry).
pub fn create_rendezvous_from(region: u64) -> Option<RendezvousId> {
    try_create_rendezvous_from(region).ok()
}

/// Create an IPC rendezvous on the kernel's own budget, from chunks carved lazily and grown on
/// demand, and return the name that goes inside an `Object::Rendezvous`.
///
/// Panics on the unrecoverable: the registry at [`MAX_RENDEZVOUS`], the chunk bound reached, or no
/// memory left to carve. `pub(crate)`, so the system tests cannot call it: the chunks are never
/// freed, so a test endpoint made here would cost the boot a page for good. A test carves from a
/// region it owns with [`create_rendezvous_from`]. See notes/scheduler/rendezvous.md.
///
/// Context: takes `IPC_TABLES` (a new chunk is carved under it) and releases it before
/// `try_create_rendezvous_from` takes it again.
pub(crate) fn create_rendezvous() -> RendezvousId {
    loop {
        // Take, or lazily carve, the current chunk.
        let region = {
            let mut guard = IPC_TABLES.lock();
            let sched = guard.as_mut().expect("no scheduler");
            match sched.kernel_ep_region {
                Some(r) => r,
                None => {
                    assert!(
                        sched.kernel_ep_chunks < MAX_KERNEL_EP_CHUNKS,
                        "the kernel carved all {MAX_KERNEL_EP_CHUNKS} rendezvous chunks; \
                         with {MAX_RENDEZVOUS} registry slots this should be unreachable",
                    );
                    let r = crate::memory_region::create(KERNEL_EP_CHUNK_PAGES)
                        .expect("no memory for a kernel rendezvous chunk");
                    sched.kernel_ep_chunks += 1;
                    sched.kernel_ep_region = Some(r);
                    r
                }
            }
        };

        match try_create_rendezvous_from(region) {
            Ok(ep) => return ep,
            // The chunk is spent. Drop it and let the next pass carve a fresh one; the loop runs at
            // most twice per call, because a fresh chunk always has a page. Clearing the handle is
            // what "forgotten deliberately" means in the field's doc comment: the pages stay pinned
            // and the endpoints already in them stay live.
            Err(RendezvousFailure::RegionFull) => {
                let mut guard = IPC_TABLES.lock();
                let sched = guard.as_mut().expect("no scheduler");
                // Only clear the handle we just failed on. Another core may already have replaced it.
                if sched.kernel_ep_region == Some(region) {
                    sched.kernel_ep_region = None;
                }
            }
            Err(RendezvousFailure::RegistryFull) => {
                panic!(
                    "out of rendezvous points: {MAX_RENDEZVOUS} live at once, raise MAX_RENDEZVOUS"
                )
            }
        }
    }
}

// --- Notifications: milestone 151 (notification objects), DECISIONS §101 (notification objects)
//
// The asynchronous half of IPC: a word a signaller ORs into and a waiter takes. The decision core
// (what a signal, a wait and a poll do) is `inter_process_communication::notification`, proved
// there; what lives here is what the crate cannot see: the registry, the mailboxes, the wake, and
// the TCB binding, which reaches into another object's wait queue. See notes/notification-objects.md.

/// Widen a three-word IPC message `m` into the five-word mailbox, with `badge` (the badge on the
/// endpoint capability the sender invoked, 0 when unbadged) in word 3 and zero in word 4. Only a
/// fault or exit message (DECISIONS §26 (the fault endpoint)) fills the top two otherwise, so the
/// fault path reuses the rendezvous machinery. A plain `SEND` carries its badge because §242 (a
/// system log) stamps every record from it; see notes/scheduler/rendezvous.md.
pub(super) fn wide(m: [u64; 3], badge: u64) -> [u64; 5] {
    [m[0], m[1], m[2], badge, 0]
}

/// **Send three words `msg` to rendezvous `ep`, blocking until a receiver takes them.**
///
/// A receiver already waiting gets the message in its mailbox and is woken, and nobody blocks.
/// Otherwise the message is parked in our own mailbox, we join the sender queue and block, and a
/// later receiver collects it and wakes us. A user thread reaches this through the `SEND` method.
/// A stale `ep` sets the abort flag and an interrupt-carrying one the refusal flag, instead.
/// See notes/scheduler/rendezvous.md.
///
/// Context: takes `IPC_TABLES`; may block.
#[inline(always)]
pub fn ipc_send(ep: RendezvousId, msg: [u64; 3]) {
    ipc_send_badged(ep, msg, 0);
}

/// **Sends that found no receiver waiting and blocked, since boot, in the bench build.**
///
/// `ipc_rtt`'s row is the receiver-first mode of its ping-pong, in which every send meets a waiting
/// receiver (calef, 2026-10-22 UTC, on milestone 939 (the icount bench runs without a preemption
/// tick in its windows)). `kernel/src/bench/ipc.rs` reads this across the row's window and fails
/// the run if it moved, so a change that flips the row to sender-first stops the bench rather
/// than reading as a step in the floor.
///
/// Name: provisional, minted 2026-10-22 (UTC) by milestone 939's lane.
#[cfg(feature = "bench")]
pub(crate) fn sends_blocked() -> u64 {
    SENDS_BLOCKED.load(core::sync::atomic::Ordering::Relaxed)
}

#[cfg(feature = "bench")]
static SENDS_BLOCKED: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// [`ipc_send`] through a badged endpoint capability: the receiver's `RECEIVE` sees `badge` in word 3
/// (milestone 613 (a system log service), provisional; see [`wide`] for why a plain send carries
/// it). The syscall layer passes the badge of the capability the sender invoked, so it is the
/// kernel's word and never the sender's. [`ipc_call_badged`] is the same split for `CALL`.
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(
    target_os = "none",
    unsafe(link_section = ".text.hot.sched.ipc_send_badged")
)]
pub fn ipc_send_badged(ep: RendezvousId, msg: [u64; 3], badge: u64) {
    // E3's footprint-perturbation experiment (milestone 134): reachable but never taken; see
    // `crate::fastpath_pad` for what this is and why it costs nothing when the feature is off.
    #[cfg(feature = "fastpath_pad")]
    crate::fastpath_pad::maybe_pad();
    let msg = wide(msg, badge);
    let block = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");
        let current = current_thread_id();

        // A stale rendezvous (its region was revoked): mark this send aborted and do not block. The
        // kernel-side `ipc_send` wrapper never hits this (its endpoints are never revoked); the
        // syscall layer reads the flag and returns an error.
        let Some(rendezvous) = rendezvous_of(sched, ep) else {
            set_ipc_aborted(sched, current);
            return;
        };
        // The running thread's token: if it queues, it stays live, since a thread queued on a
        // rendezvous is Blocked, which the reaper never touches. Every verdict that did not queue
        // it hands it back, and it goes home to the thread (`hold_token`).
        match rendezvous.send(take_token(sched, current)) {
            inter_process_communication::Send::Rendezvous(receiver, me) => {
                hold_token(me);
                let receiver = hold_token(receiver);
                let r = sched.threads.get_mut(receiver).unwrap();
                r.mailbox = msg;
                r.handshake.serve(); // delivered: this wake passes the boot-8 gate
                trace::record(trace::Event::Served, receiver, 1);
                wake(sched, receiver);
                false
            }
            inter_process_communication::Send::Blocked => {
                #[cfg(feature = "bench")]
                SENDS_BLOCKED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                // `send` has already queued `current` as a sender; we record why it is parked.
                let me = sched.threads.get_mut(current).unwrap();
                me.mailbox = msg;
                me.handshake.park(Wait::Rendezvous(ep, WaitRole::Sender)); // only a collecting receiver may wake us
                trace::record(trace::Event::BlockSelf, current, ep as u8);
                true
            }
            // The rendezvous carries an interrupt (§101 ruling B): nothing was delivered or queued.
            inter_process_communication::Send::Refused(me) => {
                hold_token(me);
                set_ipc_refused(sched, current);
                false
            }
        }
    };

    // Block OUTSIDE the lock (rule 1), and only after we have already recorded ourselves as
    // blocked, so a timer-driven `schedule()` in the gap does the right thing either way.
    if block {
        schedule();
    }
}

/// **Receive three words from an rendezvous, blocking until one arrives.** The mirror of
/// [`ipc_send`].
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(
    target_os = "none",
    unsafe(link_section = ".text.hot.sched.ipc_receive")
)]
pub fn ipc_receive(ep: RendezvousId) -> [u64; 5] {
    let immediate = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");
        let current = current_thread_id();

        // A stale rendezvous (revoked): mark aborted and return a placeholder; the syscall layer sees
        // the flag and errors. (A thread revoked *while blocked* below is handled the same way: the
        // reaper sets the flag and wakes it, and it returns its stale mailbox for the layer to drop.)
        let Some(rendezvous) = rendezvous_of(sched, ep) else {
            set_ipc_aborted(sched, current);
            return [0, 0, 0, 0, 0];
        };
        // One lookup serves both things this path needs from the running thread: whether it is
        // bound, and its token.
        let me = sched.threads.get_mut(current).expect("running thread");
        let bound = me.bound_notification.is_some();
        let Some(token) = me.own_token.take() else {
            missing_token()
        };
        // A bound notification's pending signal ends the receive before it can block, ahead of a
        // queued sender because it arrived earlier (DECISIONS §101). An arm of the decision, not an
        // early return, which would duplicate the unlock path.
        if bound && let Some(word) = take_bound_signal(sched, current) {
            hold_token(token);
            Some(bound_delivery(word))
        } else {
            // As in ipc_send: the running thread's token, and Blocked-while-queued keeps it live.
            match rendezvous.receive(token) {
                // An interrupt already fired while we were not waiting. Take it and do not block.
                inter_process_communication::Receive::Signal(me) => {
                    hold_token(me);
                    Some([1, 0, 0, 0, 0])
                }
                inter_process_communication::Receive::FromSender(sender, me) => {
                    hold_token(me);
                    let sender = hold_token(sender);
                    let msg = sched.threads.get(sender).unwrap().mailbox;
                    // A **dead sender** (a §26 corpse) and a **caller** (its outgoing cap is the
                    // one-shot Reply a CALL minted, §12 (call/reply IPC)) get their words delivered
                    // and are not woken as a completed send: the corpse never runs again, and the
                    // caller is answered `Gone` (§246 (a plain `RECEIVE` never takes a capability),
                    // PROVISIONAL number). See `collected_without_serving`.
                    let leave_blocked = matches!(
                        sched.threads.get(sender).unwrap().outgoing_cap,
                        Some(c) if matches!(c.object, crate::cap::Object::Reply(_))
                    ) || sched.threads.get(sender).unwrap().handshake.state
                        == State::Dead;
                    if !leave_blocked {
                        // A plain RECEIVE delivers no capability: drop the delegation a `SEND_CAP`
                        // sender parked, or it rides that sender's next plain `SEND` to another
                        // rendezvous. The sender keeps its own copy.
                        sched.threads.get_mut(sender).unwrap().outgoing_cap = None;
                        // Collected: the sender's rendezvous is complete, which is what lets its
                        // wake through the boot-8 gate.
                        sched.threads.get_mut(sender).unwrap().handshake.serve();
                        trace::record(trace::Event::Served, sender, 2);
                        wake(sched, sender);
                    } else {
                        // A corpse or a caller: neither is woken as a completed send. Out of line,
                        // because neither is on the fastpath, and keeping both arms here put
                        // x86_64's `ipc_send_receive` past `script/fastpath-footprint`'s band.
                        collected_without_serving(sched, sender);
                    }
                    Some(msg)
                }
                inter_process_communication::Receive::Blocked => {
                    // `receive` has already queued `current` as a receiver.
                    let me = sched.threads.get_mut(current).unwrap();
                    me.handshake.park(Wait::Rendezvous(ep, WaitRole::Receiver)); // only a delivering sender may wake us
                    trace::record(trace::Event::BlockSelf, current, ep as u8);
                    None
                }
            }
        }
    };

    match immediate {
        Some(msg) => msg,
        None => {
            schedule(); // blocks; a sender fills our mailbox and wakes us
            let guard = IPC_TABLES.lock();
            let sched = guard.as_ref().expect("no scheduler");
            let t = sched.threads.get(current_thread_id()).unwrap();
            // The boot-8 gate makes an undelivered resume unreachable; this is its tripwire,
            // loud in every QEMU test build, on the path where the strand was observed.
            debug_assert!(
                t.handshake.is_delivered(),
                "receive resumed with nothing delivered"
            );
            t.mailbox
        }
    }
}

/// The x1 value a `RECEIVE_CAP` returns when no capability accompanied the message. Mirrors
/// `abi::rendezvous::NO_CAP`; kept here too so the scheduler names it without reaching into the ABI.
pub(super) const NO_CAP: u64 = u64::MAX;

/// **A `CALL` met a receiver parked in plain `RECEIVE`** (§246 (a plain `RECEIVE` never takes a
/// capability), PROVISIONAL number; calef's ruling A, 2026-10-04 UTC). The receiver gets the
/// request's words exactly as it would had the caller parked first (`ipc_receive`'s collect), and
/// no capability: a plain `RECEIVE` has no slot it asked to have filled, and the Reply would be one
/// it never reads. The caller is answered `Gone`, since no Reply exists for anyone to send. Caller
/// holds `IPC_TABLES`; `caller` is the running thread and has not parked.
///
/// `#[cold]` and out of line for `set_ipc_aborted`'s reason: every `CALL` server in the tree
/// receives with `RECEIVE_CAP`, so the `CALL` fastpath pays one branch for this and none of its bytes.
#[cold]
#[inline(never)]
pub(super) fn call_meets_plain_receive(
    sched: &mut IpcTables,
    receiver: ThreadId,
    caller: ThreadId,
    msg: [u64; 2],
    badge: u64,
) {
    let r = sched.threads.get_mut(receiver).unwrap();
    r.mailbox = [msg[0], msg[1], 0, badge, 0];
    r.handshake.serve(); // delivered: this wake passes the boot-8 gate
    trace::record(trace::Event::Served, receiver, 5);
    wake(sched, receiver);
    set_ipc_aborted(sched, caller);
}

/// **A plain `RECEIVE` collected a `sender` it does not wake as a completed send**: a corpse or a
/// caller.
///
/// A dead sender (a DECISIONS §26 corpse) waits on nothing now and is never woken; its label goes
/// to the receiver, the running thread. A caller is answered `Gone` (§246 (a plain `RECEIVE` never
/// takes a capability), PROVISIONAL number), the sender-first half of [`call_meets_plain_receive`],
/// and `set_ipc_aborted` drops the Reply in its `outgoing_cap`, the only copy.
///
/// Context: caller holds `IPC_TABLES`; `rendezvous.receive` has already popped `sender`.
#[cold]
#[inline(never)]
pub(super) fn collected_without_serving(sched: &mut IpcTables, sender: ThreadId) {
    if sched.threads.get(sender).unwrap().handshake.state == State::Dead {
        let corpse = sched.threads.get_mut(sender).unwrap();
        corpse.handshake.wait_on = None;
        let label = corpse.fault_label;
        hand_over_label(sched.threads.get(current_thread_id()).unwrap(), label);
    } else {
        set_ipc_aborted(sched, sender);
        wake(sched, sender);
    }
}

/// **Hand a death message's `label` to the supervisor `receiver`**, in argument register 5 of
/// its saved user frame (`x5`, `a5` or `r9`; DECISIONS §148 (a supervisor restarts by asking),
/// ruling R3).
///
/// Beside the mailbox, so only a death pays for the sixth word. Only a plain `RECEIVE` by a user
/// thread gets it, and no ordinary message writes it, so a child cannot forge one. See
/// notes/scheduler/rendezvous.md.
///
/// Context: caller holds `IPC_TABLES`.
///
/// Name: provisional, milestone 105 (the two forks)'s lane, 2026-10-05 (UTC). An alternative, `deliver_fault_label`, was suggested
/// to match its siblings `deliver_death` and `deliver_capability`.
#[cold]
#[inline(never)]
pub(super) fn hand_over_label(receiver: &crate::thread::Thread, label: u64) {
    if receiver.receiving_cap || receiver.space.is_none() {
        return;
    }
    if let Some(stack) = receiver.stack.as_ref() {
        // SAFETY: `receiver` is a user thread (it has an address space) parked in, or running, a
        // plain RECEIVE it trapped into from user mode, so its frame is at its stack top; it is
        // blocked or is this core's current thread, and IPC_TABLES is held, so nothing else writes it.
        unsafe { crate::arch::exceptions::set_user_arg(stack.top(), 5, label) };
    }
}

/// **The `x4` a receive returns for a `CALL` whose Reply landed at `slot`** (milestone 706 (a
/// `CALL` server can tell a Reply from a delegation), DECISIONS §245 (a `CALL` server tells a Reply
/// from a delegation)): `abi::rendezvous::REPLY_DELIVERED` when the Reply was installed, `0` when
/// the receiver's table was full and there is no Reply to name. Written only on the two paths a
/// `CALL` reaches a receiver (`ipc_call_badged`'s rendezvous and `ipc_receive_cap`'s collect of a
/// parked caller), so a `SEND_CAP` delegation, which reaches the same `x1`, never carries it.
#[inline(always)]
pub(super) fn reply_tag(slot: u64) -> u64 {
    if slot == NO_CAP {
        0
    } else {
        abi::rendezvous::REPLY_DELIVERED
    }
}

/// **Send a `capability` the caller minted, plus one `data` word and `badge`, to `ep`.**
///
/// A receiver parked in `RECEIVE_CAP` gets the capability filed in its table and the slot in its
/// mailbox, and is woken; a plain `RECEIVE` gets the data word and no capability. Otherwise the
/// word and the capability are parked for a later receiver. A full receiver table drops the
/// capability and reports `NO_CAP`. A copy of a held capability goes through [`ipc_delegate_cap`]
/// ([`Delegation`]); only system tests call this, hence the `allow`.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn ipc_send_cap(ep: RendezvousId, data: u64, capability: crate::cap::Cap, badge: u64) {
    let sent = ipc_send_cap_from(ep, data, badge, |_, _| Ok(capability));
    debug_assert!(sent.is_ok(), "a minted capability has no source to lose");
}

/// **`SEND_CAP`'s body: send a narrowed copy of a capability the running thread holds.** The source
/// is read, checked and copied under the `IPC_TABLES` hold that delivers or parks the copy, so no
/// revocation sweep can fall between the read and the filing ([`Delegation`]). `Err` is the source's
/// answer (`NoSuchSlot`, `NotPermitted`), with nothing sent; a stale or refusing rendezvous is
/// reported as before, through `take_ipc_aborted`. Name provisional.
pub(crate) fn ipc_delegate_cap(
    ep: RendezvousId,
    data: u64,
    delegation: Delegation,
    badge: u64,
) -> Result<(), abi::Error> {
    ipc_send_cap_from(ep, data, badge, |sched, current| {
        let table = sched
            .threads
            .capabilities(current)
            .ok_or(abi::Error::NoSuchSlot)?;
        delegation.derive(&table.lock())
    })
}

/// The body [`ipc_send_cap`] and [`ipc_delegate_cap`] share. `source` runs first, under the hold,
/// and its error returns before the rendezvous is touched, which is the order the syscall layer
/// answered in when it read the source itself.
pub(super) fn ipc_send_cap_from(
    ep: RendezvousId,
    data: u64,
    badge: u64,
    source: impl FnOnce(&IpcTables, ThreadId) -> Result<crate::cap::Cap, abi::Error>,
) -> Result<(), abi::Error> {
    let block = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");
        let current = current_thread_id();
        let capability = source(sched, current)?;

        let Some(rendezvous) = rendezvous_of(sched, ep) else {
            set_ipc_aborted(sched, current);
            return Ok(()); // stale rendezvous: aborted, syscall layer errors
        };
        // As in ipc_send.
        match rendezvous.send(take_token(sched, current)) {
            inter_process_communication::Send::Rendezvous(receiver, me) => {
                hold_token(me);
                let receiver = hold_token(receiver);
                let (r, caps) = sched.threads.get_mut_with_capabilities(receiver).unwrap();
                if r.receiving_cap {
                    let slot = deliver_capability(caps, capability);
                    // Word 3 carries the sender's badge (milestone 599 (a frame per filesystem client channel)): the same store that used to
                    // write a zero here, so RECEIVE_CAP surfaces it at no extra instruction on this path.
                    r.mailbox = [data, slot, 0, badge, 0];
                    // A capability was installed, so RECEIVE_CAP's x1 is a real slot (milestone 634 (a plain SEND
                    // received by RECEIVE_CAP never hands the receiver a sender-chosen slot)).
                    r.cap_delivered = true;
                } else {
                    // A plain RECEIVE takes no capability (§246 (a plain `RECEIVE` never takes a
                    // capability), PROVISIONAL number): the data word arrives, the copy is dropped,
                    // and the sender keeps its own.
                    r.mailbox = [data, 0, 0, badge, 0];
                }
                r.handshake.serve(); // delivered: this wake passes the boot-8 gate
                trace::record(trace::Event::Served, receiver, 3);
                wake(sched, receiver);
                false
            }
            inter_process_communication::Send::Blocked => {
                // `send` queued `current`; we park the data word and the capability to hand over.
                // Word 3 is the badge, read back by the eventual RECEIVE_CAP (milestone 599).
                let me = sched.threads.get_mut(current).unwrap();
                me.mailbox = [data, 0, 0, badge, 0];
                me.outgoing_cap = Some(capability);
                me.handshake.park(Wait::Rendezvous(ep, WaitRole::Sender)); // only a collecting receiver may wake us
                trace::record(trace::Event::BlockSelf, current, ep as u8);
                true
            }
            // As in `ipc_send`. The capability stays with the sender: it was never moved.
            inter_process_communication::Send::Refused(me) => {
                hold_token(me);
                set_ipc_refused(sched, current);
                false
            }
        }
    };

    if block {
        schedule();
    }
    Ok(())
}

/// **File a capability an IPC is delivering in the receiving thread's table**, or [`NO_CAP`] if the
/// table is full (the receiver still gets the data words). Caller holds `IPC_TABLES`; this takes the
/// table's own lock beneath it (rank 60 then 57, `sync::rank::CAPABILITY_TABLE`).
///
/// **Out of line, one copy for the three delivering paths** (`ipc_send_cap`, `ipc_call`'s hand-off
/// to a waiting server, and `ipc_receive_cap`'s collect): inlined, the second lock's mask, rank
/// check and release grew both fastpath closures past their band. Name provisional.
#[inline(never)]
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(
    target_os = "none",
    unsafe(link_section = ".text.hot.sched.deliver_capability")
)]
pub(super) fn deliver_capability(table: &CapabilityTableLock, capability: crate::cap::Cap) -> u64 {
    table.lock().insert(capability).unwrap_or(NO_CAP)
}

/// **Receive a data word and, if one was sent, a capability.** The mirror of [`ipc_send_cap`], and
/// the receiver's half of delegation. Returns `[data, received_slot, w1, badge]`, where
/// `received_slot` is where an incoming capability landed in *our* capability table, or [`NO_CAP`]
/// if the message carried none, and `badge` is the badge on the endpoint capability the sender
/// invoked (milestone 599, provisional; 0 when the sender's capability was unbadged).
///
/// A capability-carrying send and this share the ordinary sender/receiver queues, so either side
/// may arrive first, exactly as with the plain path.
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(
    target_os = "none",
    unsafe(link_section = ".text.hot.sched.ipc_receive_cap")
)]
pub fn ipc_receive_cap(ep: RendezvousId) -> [u64; 5] {
    let immediate = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");
        let current = current_thread_id();

        let Some(rendezvous) = rendezvous_of(sched, ep) else {
            set_ipc_aborted(sched, current);
            return [0, 0, 0, 0, 0]; // stale rendezvous: aborted, syscall layer errors
        };
        // One lookup for the binding test and the token, as in `ipc_receive`.
        let me = sched.threads.get_mut(current).expect("running thread");
        let bound = me.bound_notification.is_some();
        let Some(token) = me.own_token.take() else {
            missing_token()
        };
        // The binding's receive-side half, exactly as in `ipc_receive`.
        if bound && let Some(word) = take_bound_signal(sched, current) {
            hold_token(token);
            Some(bound_delivery(word))
        } else {
            match rendezvous.receive(token) {
                // An interrupt signal is not a delegation; it carries no capability and no badge.
                inter_process_communication::Receive::Signal(me) => {
                    hold_token(me);
                    Some([1, NO_CAP, 0, 0, 0])
                }
                inter_process_communication::Receive::FromSender(sender, me) => {
                    hold_token(me);
                    let sender = hold_token(sender);
                    let msg = sched.threads.get(sender).unwrap().mailbox;
                    let capability = sched.threads.get_mut(sender).unwrap().outgoing_cap.take();
                    // A caller's outgoing cap is the one-shot Reply the kernel minted for its CALL (§12); a
                    // SEND_CAP sender's is the capability it chose to delegate. The difference is liveness:
                    // a caller stays blocked awaiting its reply, so it must NOT be woken here; a SEND_CAP
                    // sender's rendezvous is complete the moment we take the cap.
                    let is_reply = matches!(capability, Some(c) if matches!(c.object, crate::cap::Object::Reply(_)));
                    let slot = match capability {
                        Some(c) => {
                            deliver_capability(sched.threads.capabilities(current).unwrap(), c)
                        }
                        None => NO_CAP,
                    };
                    if !is_reply {
                        // Collected: the sender's rendezvous is complete (the boot-8 gate).
                        sched.threads.get_mut(sender).unwrap().handshake.serve();
                        trace::record(trace::Event::Served, sender, 4);
                        wake(sched, sender);
                    }
                    // x0 = word0, x1 = the delivered slot, x2 = word1 (a CALL's second word; 0 for a plain
                    // SEND_CAP, whose sender parked mailbox[1] = 0), x3 = the sender's badge (msg[3],
                    // milestone 599), x4 = REPLY_DELIVERED iff x1 is this caller's Reply (milestone
                    // 706), else 0. Never BOUND: that is the arm above.
                    let tag = if is_reply { reply_tag(slot) } else { 0 };
                    Some([msg[0], slot, msg[1], msg[3], tag])
                }
                inter_process_communication::Receive::Blocked => {
                    let me = sched.threads.get_mut(current).unwrap();
                    // Clear before parking: whoever wakes us sets this iff it installs a
                    // capability, so a plain SEND (which installs none) leaves it false (milestone 634).
                    me.cap_delivered = false;
                    // And say which receive this is, so a sender that meets us may install one
                    // (§246, PROVISIONAL number). Cleared when this receive resumes, below, so a
                    // later plain RECEIVE parks with it false.
                    me.receiving_cap = true;
                    me.handshake.park(Wait::Rendezvous(ep, WaitRole::Receiver)); // only a delivering sender may wake us
                    trace::record(trace::Event::BlockSelf, current, ep as u8);
                    None
                }
            }
        }
    };

    match immediate {
        Some(msg) => msg,
        None => {
            schedule(); // a capability-carrying sender fills our mailbox and wakes us
            let mut guard = IPC_TABLES.lock();
            let sched = guard.as_mut().expect("no scheduler");
            let t = sched.threads.get_mut(current_thread_id()).unwrap();
            debug_assert!(
                t.handshake.is_delivered(),
                "receive_cap resumed with nothing delivered"
            );
            // No longer parked in RECEIVE_CAP (§246, PROVISIONAL number). Cleared here rather than
            // at a plain RECEIVE's park, which keeps the store off `ipc_send_receive`'s closure:
            // with it there, x86_64's closure went over `script/fastpath-footprint`'s 5% band.
            t.receiving_cap = false;
            // The whole mailbox: RECEIVE_CAP's three words, the sender's badge at m[3] (milestone 599),
            // and `w4`, which is `abi::notification::BOUND` when the bound notification ended this
            // receive, `abi::rendezvous::REPLY_DELIVERED` when a CALL's Reply was installed
            // (milestone 706, written by `ipc_call_badged`), and `0` for every other delivery a
            // sender or the kernel's death path makes (milestone 151).
            // **x1 is NO_CAP unless a capability was installed for this delivery** (milestone 634,
            // fatal risk 7). A plain SEND that reached us parked drops its three words straight into
            // the mailbox, so m[1] is the sender's chosen word; returning it would hand a sender a
            // slot number where a CALL server reads a reply slot. A bound-notification delivery is
            // exempt: its x4 == BOUND and its m[1] is the notification word, not a sender's choice.
            let m = t.mailbox;
            if t.cap_delivered || m[4] == abi::notification::BOUND {
                m
            } else {
                [m[0], NO_CAP, m[1], m[3], 0]
            }
        }
    }
}

/// **Call: send two words `msg` to `ep` and block until replied**, returning the reply's words.
///
/// The kernel mints a one-shot `Reply` naming this caller and hands it to the server through
/// [`ipc_receive_cap`], and the caller is reachable only through it (DECISIONS §12). See
/// notes/scheduler/rendezvous.md.
///
/// # BUGS
/// - A live server that never replies, or had no room for the Reply, blocks its caller forever:
///   a `CALL` has no deadline. One that can no longer answer frees it ([`strand_reply_caller`]).
/// - A Reply names a thread, not a call (milestone 371 (a reply capability that names a call)).
// Inlined so the unbadged fastpath (`ipc_call_reply`, the shape real services run and the one
// `script/icount` measures) gains no call frame: this forwards to `ipc_call_badged` with badge 0,
// which writes the same mailbox word 3 it always did.
#[inline(always)]
pub fn ipc_call(ep: RendezvousId, msg: [u64; 2]) -> [u64; 3] {
    ipc_call_badged(ep, msg, 0)
}

/// [`ipc_call`] carrying the invoked endpoint capability's `badge`, which reaches the server's
/// [`ipc_receive_cap`] in word 3; a plain [`ipc_call`] passes 0. A separate name, so the many
/// `ipc_call(ep, msg)` sites and the fastpath's shape are unchanged.
///
/// Context: takes `IPC_TABLES`, and the server's capability-table lock beneath it. Blocks until
/// replied, unless the call is refused or aborted.
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(
    target_os = "none",
    unsafe(link_section = ".text.hot.sched.ipc_call_badged")
)]
pub fn ipc_call_badged(ep: RendezvousId, msg: [u64; 2], badge: u64) -> [u64; 3] {
    // E3's footprint padding (`crate::fastpath_pad`), on the CALL side as well as the SEND side:
    // the CALL/reply closure is the shape real services run. One call site per shape.
    #[cfg(feature = "fastpath_pad")]
    crate::fastpath_pad::maybe_pad();
    {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");
        let current = current_thread_id();
        let reply = crate::cap::reply_cap(current);

        // `send` decides the rendezvous exactly as a plain SEND: a waiting server, or block. The
        // difference is the caller *always* blocks awaiting the reply, whether or not it met a server.
        let Some(rendezvous) = rendezvous_of(sched, ep) else {
            set_ipc_aborted(sched, current);
            return [0, 0, 0]; // stale rendezvous: aborted, syscall layer errors
        };
        // As in ipc_send; a caller queued here is Blocked until its Reply arrives. A caller that
        // meets a server is blocked too, but on nothing, so its token goes home to its TCB, where
        // the reply's wake finds it.
        match rendezvous.send(take_token(sched, current)) {
            inter_process_communication::Send::Rendezvous(receiver, me) => {
                hold_token(me);
                let receiver = hold_token(receiver);
                // A server is parked in RECEIVE_CAP: hand it the reply cap and the two words now.
                let (r, caps) = sched.threads.get_mut_with_capabilities(receiver).unwrap();
                if !r.receiving_cap {
                    // A plain RECEIVE cannot hold the Reply, so this CALL is answered `Gone`
                    // (§246, PROVISIONAL number). Out of line: no server in the tree does this.
                    call_meets_plain_receive(sched, receiver, current, msg, badge);
                    return [0, 0, 0];
                }
                let slot = deliver_capability(caps, reply);
                // Word 3 is the caller's badge (milestone 599): the same store as before with a
                // value instead of a zero, so the server's RECEIVE_CAP surfaces which client called.
                // Word 4 says x1 is a Reply (milestone 706), which only this path and the collect
                // in `ipc_receive_cap` may say; a SEND_CAP's delivery leaves it 0.
                r.mailbox = [msg[0], slot, msg[1], badge, reply_tag(slot)];
                // A Reply capability was installed, so RECEIVE_CAP's x1 is a real slot (milestone 634).
                r.cap_delivered = true;
                r.handshake.serve(); // delivered: this wake passes the boot-8 gate
                trace::record(trace::Event::Served, receiver, 5);
                wake(sched, receiver);
            }
            inter_process_communication::Send::Blocked => {
                // No server yet; `send` queued us as a sender. Park the words and ride the reply cap
                // in `outgoing_cap` so the eventual RECEIVE_CAP hands it over and, seeing a Reply, leaves
                // us blocked (see ipc_receive_cap).
                // Park the words with the badge at word 3 (milestone 599), where the eventual
                // RECEIVE_CAP reads it back out of this caller's mailbox.
                let me = sched.threads.get_mut(current).unwrap();
                me.mailbox = [msg[0], msg[1], 0, badge, 0];
                me.outgoing_cap = Some(reply);
            }
            // The rendezvous carries an interrupt (§101 ruling B). Return before parking: a caller
            // whose request was refused has no reply coming, and would otherwise wait for ever.
            inter_process_communication::Send::Refused(me) => {
                hold_token(me);
                set_ipc_refused(sched, current);
                return [0, 0, 0];
            }
        }
        // Either way we block until the reply arrives. We are NOT queued as a receiver; the Reply
        // capability, which carries our tid, is the only thing that can wake us.
        let me = sched.threads.get_mut(current).unwrap();
        me.handshake.park(Wait::Rendezvous(ep, WaitRole::Reply)); // only the reply (or an abort) may wake us
        trace::record(trace::Event::BlockSelf, current, ep as u8);
    }

    schedule(); // returns once ipc_reply has filled our mailbox and woken us

    let guard = IPC_TABLES.lock();
    let sched = guard.as_ref().expect("no scheduler");
    let t = sched.threads.get(current_thread_id()).unwrap();
    debug_assert!(
        t.handshake.is_delivered(),
        "call resumed with nothing delivered"
    );
    let m = t.mailbox;
    [m[0], m[1], m[2]] // a reply is two words plus the pad; the fault path owns the top two
}

/// **Reply: deliver two words to a blocked caller and wake it** (milestone 12). The other half of
/// [`ipc_call`], reached by invoking the one-shot Reply capability, which carries the caller's `tid`.
/// A `caller` that is gone, or is not parked awaiting a reply, gets nothing.
///
/// Context: takes `IPC_TABLES`.
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(target_os = "none", unsafe(link_section = ".text.hot.sched.ipc_reply"))]
pub fn ipc_reply(caller: ThreadId, msg: [u64; 2]) {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().expect("no scheduler");
    if let Some(t) = sched.threads.get_mut(caller) {
        // Only a caller awaiting a reply is touched: a Reply names a tid, not a wait state, and a
        // thread parked as a receiver is still linked on a wait queue, so waking it would
        // double-enqueue its one intrusive link.
        if !matches!(
            t.handshake.wait_on,
            Some(Wait::Rendezvous(_, WaitRole::Reply))
        ) {
            return;
        }
        // Word 2 is `NO_CAP`: a plain `REPLY` carries no capability, and `CALL` returns word 2 in
        // `x2` so a caller of a `REPLY_CAPABILITY` server can tell "none" from slot 0 (§255
        // (each socket is its own capability)). The store was a zero; it costs the round trip nothing.
        t.mailbox = [msg[0], msg[1], NO_CAP, 0, 0];
        t.handshake.serve(); // delivered: this wake passes the boot-8 gate
        trace::record(trace::Event::Served, caller, 6);
        wake(sched, caller);
    }
}

/// **Reply with `msg` and a copy of the running thread's capability in `slot`**, filed in
/// `caller`'s table with the landing slot as word 2 (§255 (each socket is its own capability)).
///
/// Read, checked (`GRANT`) and filed under one hold of `IPC_TABLES` ([`Delegation`]); the copy
/// keeps its rights, so a server narrows first. `Err` is the source's answer, the caller still
/// waiting. `Ok(true)`: the copy is in the caller's table. `Ok(false)`: the caller was not awaiting
/// a reply, or its full table got the words and `NO_CAP`. Out of line, off the `REPLY` round trip.
///
/// Context: takes `IPC_TABLES`, and capability-table locks beneath it.
#[inline(never)]
pub(crate) fn ipc_reply_capability(
    caller: ThreadId,
    msg: [u64; 2],
    slot: u64,
) -> Result<bool, abi::Error> {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().expect("no scheduler");
    let current = current_thread_id();
    let capability = {
        let ours = sched
            .threads
            .capabilities(current)
            .ok_or(abi::Error::NoSuchSlot)?;
        let table = ours.lock();
        let src = table.get(slot).map_err(|_| abi::Error::NoSuchSlot)?;
        Delegation {
            slot,
            rights: src.rights,
        }
        .derive(&table)?
    };
    let delivered;
    {
        let Some((t, theirs)) = sched.threads.get_mut_with_capabilities(caller) else {
            return Ok(false);
        };
        // `ipc_reply`'s guard, for its reason: only a thread parked awaiting a reply is touched.
        if !matches!(
            t.handshake.wait_on,
            Some(Wait::Rendezvous(_, WaitRole::Reply))
        ) {
            return Ok(false);
        }
        delivered = deliver_capability(theirs, capability);
        t.mailbox = [msg[0], msg[1], delivered, 0, 0];
        t.handshake.serve();
    }
    trace::record(trace::Event::Served, caller, 6);
    wake(sched, caller);
    Ok(delivered != NO_CAP)
}

/// **Delete every unconsumed `Reply` capability naming `caller`**, in every capability table and
/// every `outgoing_cap`, so no Reply names a thread that has left its park.
///
/// Run by [`strand_reply_caller`] before it wakes a caller, and by [`finish_blocked_resident`] on
/// a caller it ends. O(threads x slots), on teardown paths only. See notes/scheduler/rendezvous.md.
///
/// Context: caller holds `IPC_TABLES`; takes each capability table's lock beneath it.
pub(super) fn delete_reply_caps_naming(sched: &mut IpcTables, caller: ThreadId) {
    let target = crate::cap::Object::Reply(caller);
    for (t, table) in sched.threads.iter_mut_with_capabilities() {
        table
            .lock()
            .delete_matching(|object: &crate::cap::Object| *object == target);
        if matches!(t.outgoing_cap, Some(c) if c.object == target) {
            t.outgoing_cap = None;
        }
    }
}

/// **Free one `caller` its server can no longer answer, with `abi::Error::Gone`.**
///
/// A `CALL` caller whose request was taken is linked on no wait queue, and only [`ipc_reply`] wakes
/// it. Every `Reply` naming it is deleted before the wake, or a server holding a stale one could
/// answer the caller's next, unrelated `CALL`. Only a thread parked awaiting a reply is touched;
/// returns whether this did anything. `#[cold]` because every caller is a teardown, one of them
/// the top of [`schedule`]. See notes/scheduler/rendezvous.md and notes/blocked-thread-teardown.md.
///
/// Context: caller holds `IPC_TABLES`; takes capability-table locks beneath it.
#[cold]
#[inline(never)]
pub(super) fn strand_reply_caller(sched: &mut IpcTables, caller: ThreadId) -> bool {
    let parked = sched.threads.get(caller).is_some_and(|t| {
        matches!(
            t.handshake.wait_on,
            Some(Wait::Rendezvous(_, WaitRole::Reply))
        )
    });
    if !parked {
        return false;
    }
    delete_reply_caps_naming(sched, caller); // the sweep, before the wake
    set_ipc_aborted(sched, caller);
    wake(sched, caller);
    true
}

/// **Free every caller `tid` holds a reply capability for**: the server-side trigger, for a thread
/// that can no longer answer anybody. Reached from [`depart`], the forcible-teardown conversion at
/// the top of [`schedule`], and [`reap_region_objects`]'s removal phase.
///
/// Reads the table once into an array of [`crate::cap::CAPABILITY_TABLE_SLOTS`] victims in this
/// `#[inline(never)]` frame, then acts. See notes/scheduler/rendezvous.md.
///
/// Context: caller holds `IPC_TABLES`; takes capability-table locks beneath it.
#[cold]
#[inline(never)]
pub(super) fn strand_callers_of(sched: &mut IpcTables, tid: ThreadId) {
    let mut victims = [0 as ThreadId; crate::cap::CAPABILITY_TABLE_SLOTS];
    let mut found = 0;
    {
        let Some(caps) = sched.threads.capabilities(tid) else {
            return;
        };
        let t = caps.lock();
        // Overwhelmingly the common case on the `depart` path is a thread holding no reply
        // capability at all; an empty table is the case worth not paying for at all.
        if t.used() == 0 {
            return;
        }
        for slot in 0..t.len() as u64 {
            if let Ok(c) = t.get(slot)
                && let crate::cap::Object::Reply(caller) = c.object
            {
                victims[found] = caller;
                found += 1;
            }
        }
    }
    for &caller in &victims[..found] {
        strand_reply_caller(sched, caller);
    }
}

/// **Free every caller reply-parked on `ep`**: the rendezvous-side trigger, for a rendezvous being
/// torn down. `drain_waiters` cannot, because a reply park is linked on no queue; `wait_on` records
/// it.
///
/// Rescans the thread table rather than listing victims, because a list bounded by `MAX_THREADS`
/// would grow the deepest frame in the kernel. The scan ends on the abort flag, not on `wait_on`,
/// which a wake deferred behind `on_cpu` leaves in place. See notes/scheduler/rendezvous.md.
///
/// Context: caller holds `IPC_TABLES`; takes capability-table locks beneath it.
#[cold]
#[inline(never)]
pub(super) fn strand_callers_awaiting(sched: &mut IpcTables, ep: RendezvousId) {
    loop {
        let stranded = sched
            .threads
            .iter_mut()
            .find(|t| {
                matches!(t.handshake.wait_on, Some(Wait::Rendezvous(on, WaitRole::Reply)) if on == ep)
                    && !t.handshake.ipc_aborted
            })
            .map(|t| t.id);
        let Some(caller) = stranded else { break };
        if !strand_reply_caller(sched, caller) {
            break;
        }
    }
}

#[cfg(test)]
mod tests;
