//! Notifications: creating one, signal, wait and poll, and binding one to a thread so a signal
//! reaches a receiver parked on its rendezvous. See notes/notification-objects.md ("The kernel
//! half").
//!
//! Name: provisional (milestone 840's lane, 2026-10-11 UTC; §271 (the scheduler file splits into
//! submodules of `sched`)).

use super::*;

/// The most notifications that can exist at once, whole machine: a registry bound like
/// [`MAX_RENDEZVOUS`], and half of it, since §101's consumers want about one per thread or service.
/// It bounds only the name table, 16 bytes a slot; the pages come from the creators' regions.
pub(super) const MAX_NOTIFICATIONS: usize = 256;

/// A notification's name: a generational name over the notification registry, what an
/// `Object::Notification` capability carries. Stale-safe like every other name.
pub type NotificationId = u64;

/// What lives at the start of a notification's page: the proved state machine, and the one thread
/// it is bound to, if any. The binding is here and on the thread (`Thread::bound_notification`) so
/// a signal and a receive each find the other side without a search.
pub(super) struct NotificationPage {
    pub(super) state: inter_process_communication::notification::Notification<Thread>,
    /// Set once by `BIND` and never cleared. A generational name, so a dead thread leaves it stale
    /// and every reader treats a miss as unbound.
    pub(super) bound: Option<ThreadId>,
}

/// The notification behind a name, or `None` if it no longer resolves. Caller holds `IPC_TABLES`.
/// The `'static` is [`rendezvous_of`]'s, for the same reason: the page is pinned while the name
/// resolves, and `IPC_TABLES` serializes every access.
pub(super) fn notification_of(
    sched: &IpcTables,
    id: NotificationId,
) -> Option<&'static mut NotificationPage> {
    let phys = *sched.notification_table.get(id)?;
    // SAFETY: retyped exclusively for this notification, its region pinned while the name
    // resolves, direct-mapped, and serialized by IPC_TABLES, which every caller holds.
    Some(unsafe { &mut *(crate::arch::mmu::phys_to_virt(phys) as *mut NotificationPage) })
}

/// Create a notification in `region`'s memory: one page retyped and pinned, the object at its
/// start, a fresh name. `RETYPE_OBJ`'s engine, shaped like [`try_create_rendezvous_from`]: the
/// registry is checked before a page is spent. `None` when the region or the registry is full,
/// which userspace sees as one `OutOfMemory`.
pub fn create_notification_from(region: u64) -> Option<NotificationId> {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut()?;
    if sched.notification_table.len() >= MAX_NOTIFICATIONS {
        return None;
    }
    let phys = crate::memory_region::retype_object_page(
        region,
        crate::memory_region::ObjectKind::Notification,
    )?;
    // SAFETY: fresh page, exclusively ours, direct-mapped.
    unsafe {
        (crate::arch::mmu::phys_to_virt(phys) as *mut NotificationPage).write(NotificationPage {
            state: inter_process_communication::notification::Notification::new(),
            bound: None,
        });
    }
    // Cannot fail: capacity was checked above under this same lock hold.
    sched.notification_table.insert_with(|_| phys)
}

/// The mailbox of a receive the bound notification ended, `(BOUND, word, 0, 0, BOUND)`: §101's
/// `w0 = 2`, the word in `w1`, and the tag again in `w4`, the one register no sender can write (see
/// `abi::notification::BOUND`, and notes/notification-objects.md for why `w0` alone is forgeable).
pub(super) const fn bound_delivery(word: u64) -> [u64; 5] {
    [
        abi::notification::BOUND,
        word,
        0,
        0,
        abi::notification::BOUND,
    ]
}

/// The bound thread and the rendezvous it is parked on, if it is parked in a receive right now:
/// `Blocked` as a `Receiver` with nothing delivered yet, which is exactly "still linked on that
/// receiver queue". This is the boolean the crate's `signal` takes on trust; the argument is in
/// notes/notification-objects.md ("Who counts as a bound receiver").
///
/// Context: caller holds `IPC_TABLES`.
pub(super) fn bound_receiver(
    sched: &IpcTables,
    page: &NotificationPage,
) -> Option<(ThreadId, RendezvousId)> {
    let tid = page.bound?;
    let t = sched.threads.get(tid)?;
    match t.handshake.wait_on {
        Some(Wait::Rendezvous(ep, WaitRole::Receiver))
            if t.handshake.state == State::Blocked && !t.handshake.is_delivered() =>
        {
            Some((tid, ep))
        }
        _ => None,
    }
}

/// Deliver `word` to the bound thread `tid` parked receiving on `ep`: unlink it from the receiver
/// queue (a drain and repush), fill its mailbox with [`bound_delivery`], and record the delivery.
/// The caller wakes it.
///
/// Context: caller holds `IPC_TABLES`, and got `(tid, ep)` from [`bound_receiver`] in the same hold.
pub(super) fn deliver_bound(sched: &mut IpcTables, tid: ThreadId, ep: RendezvousId, word: u64) {
    let ptr = thread_control_block_ptr(sched, tid);
    // `ptr` is only compared by the remove; the token it hands back goes home to the thread.
    let unlinked = rendezvous_of(sched, ep).and_then(|r| r.remove_receiver(ptr));
    debug_assert!(
        unlinked.is_some(),
        "a bound receiver was not on the receiver queue it was parked on"
    );
    if let Some(token) = unlinked {
        hold_token(token);
    }
    let t = sched
        .threads
        .get_mut(tid)
        .expect("bound receiver vanished under IPC_TABLES");
    t.mailbox = bound_delivery(word);
    t.handshake.serve(); // delivered: this wake passes the boot-8 gate
    trace::record(trace::Event::Served, tid, 10);
}

/// Where a notification wake queues the thread it woke: a thread's signal is a local [`wake`], and
/// a signal from interrupt context is [`wake_load_aware`] (§28 step 2), because an interrupt or a
/// timer expiry carries no locality.
#[derive(Clone, Copy)]
pub(super) enum WakePlacement {
    Local,
    // The timer expiry walk (`expire_timers`) and `signal_notification_from_interrupt`.
    LoadAware,
}

/// Wake `tid` by `placement`. Returns the remote core to poke once `IPC_TABLES` is released, as
/// [`wake_load_aware`] does; a local wake never needs one.
pub(super) fn wake_placed(
    sched: &mut IpcTables,
    tid: ThreadId,
    placement: WakePlacement,
) -> Option<usize> {
    match placement {
        WakePlacement::Local => {
            wake(sched, tid);
            None
        }
        WakePlacement::LoadAware => wake_load_aware(sched, tid),
    }
}

/// The one signal path, for a thread's `SIGNAL` and a kernel-originated signal alike: OR `bits`
/// into notification `id`, waking a waiter or the bound receiver by `placement`. `Err(Gone)` if the
/// name no longer resolves; otherwise the remote core to poke, if any.
///
/// Context: caller holds `IPC_TABLES`; legal in interrupt context (notes/notification-objects.md,
/// "One signal path").
pub(super) fn signal_locked(
    sched: &mut IpcTables,
    id: NotificationId,
    bits: u64,
    placement: WakePlacement,
) -> Result<Option<usize>, abi::Error> {
    use inter_process_communication::notification::Signal;
    let page = notification_of(sched, id).ok_or(abi::Error::Gone)?;
    let bound = bound_receiver(sched, page);
    match page.state.signal(bits, bound.is_some()) {
        Signal::Woke(waiter, word) => {
            let tid = hold_token(waiter);
            let t = sched
                .threads
                .get_mut(tid)
                .expect("a notification waiter vanished under IPC_TABLES");
            t.mailbox = [word, 0, 0, 0, 0];
            t.handshake.serve();
            trace::record(trace::Event::Served, tid, 9);
            Ok(wake_placed(sched, tid, placement))
        }
        Signal::ToBound(word) => {
            let (tid, ep) = bound.expect("the crate answered ToBound without a bound receiver");
            deliver_bound(sched, tid, ep, word);
            Ok(wake_placed(sched, tid, placement))
        }
        Signal::Counted | Signal::Empty => Ok(None),
    }
}

/// `Notification::SIGNAL`: OR `bits` in, waking a waiter or the bound receiver. Never blocks.
pub fn notification_signal(id: NotificationId, bits: u64) -> Result<(), abi::Error> {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().expect("no scheduler");
    // A local wake never returns a core to poke.
    signal_locked(sched, id, bits, WakePlacement::Local).map(|_| ())
}

/// Signal a notification from interrupt context, placed load-aware: safe from a handler for
/// [`irq_notify`]'s reason. A stale name is dropped silently, as `irq_notify` drops a revoked
/// route.
///
/// Only a kernel test calls it: the timer expiry walk ([`expire_timers`]) calls [`signal_locked`]
/// directly under one hold for several timers. Kept for a source that signals one at a time.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn signal_notification_from_interrupt(id: NotificationId, bits: u64) {
    let remote = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");
        signal_locked(sched, id, bits, WakePlacement::LoadAware)
            .ok()
            .flatten()
    };
    if let Some(target) = remote {
        crate::arch::irq::send_reschedule(target);
    }
}

/// `Notification::WAIT`: take the word, or block until a signal arrives. `Err(Gone)` if the
/// notification is stale or destroyed while this thread waits (the region sweep aborts waiters).
pub fn notification_wait(id: NotificationId) -> Result<u64, abi::Error> {
    use inter_process_communication::notification::Wait as Waited;
    let immediate = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");
        let current = current_thread_id();
        let page = notification_of(sched, id).ok_or(abi::Error::Gone)?;
        // A queued token stays live: the reaper never frees a `Blocked` thread.
        match page.state.wait(take_token(sched, current)) {
            Waited::Word(word, me) => {
                hold_token(me);
                Some(word)
            }
            Waited::Blocked => {
                let t = sched.threads.get_mut(current).expect("running thread");
                t.handshake.park(Wait::Notification(id)); // only a signal (or an abort) may wake us
                trace::record(trace::Event::BlockSelf, current, id as u8);
                None
            }
        }
    };
    if let Some(word) = immediate {
        return Ok(word);
    }
    schedule(); // blocks; a signal fills our mailbox and wakes us
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().expect("no scheduler");
    let t = sched
        .threads
        .get_mut(current_thread_id())
        .expect("running thread");
    debug_assert!(
        t.handshake.is_delivered(),
        "notification wait resumed with nothing delivered"
    );
    if t.handshake.take_aborted() {
        return Err(abi::Error::Gone);
    }
    Ok(t.mailbox[0])
}

/// `Notification::POLL`: take the word without blocking; `0` if nothing was pending.
pub fn notification_poll(id: NotificationId) -> Result<u64, abi::Error> {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().expect("no scheduler");
    let page = notification_of(sched, id).ok_or(abi::Error::Gone)?;
    Ok(page.state.poll())
}

/// `Notification::BIND`: bind notification `id` to thread `tid`, once each (§101).
///
/// `Gone` if the notification is stale or the thread is gone or dead; `NotPermitted` if either side
/// is bound to something that still exists. A word already waiting is delivered at once if the
/// thread is receiving, or a server blocked forever in `RECEIVE` would never see it.
pub fn notification_bind(id: NotificationId, tid: ThreadId) -> Result<(), abi::Error> {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().expect("no scheduler");
    let page = notification_of(sched, id).ok_or(abi::Error::Gone)?;
    if page.bound.is_some_and(|b| sched.threads.get(b).is_some()) {
        return Err(abi::Error::NotPermitted);
    }
    let t = sched.threads.get(tid).ok_or(abi::Error::Gone)?;
    if matches!(t.handshake.state, State::Finished | State::Dead) {
        return Err(abi::Error::Gone);
    }
    if t.bound_notification
        .is_some_and(|n| notification_of(sched, n).is_some())
    {
        return Err(abi::Error::NotPermitted);
    }
    page.bound = Some(tid);
    sched
        .threads
        .get_mut(tid)
        .expect("checked above under this hold")
        .bound_notification = Some(id);

    if page.state.word() != 0
        && let Some((tid, ep)) = bound_receiver(sched, page)
    {
        let word = page.state.poll();
        deliver_bound(sched, tid, ep, word);
        wake(sched, tid);
    }
    Ok(())
}

/// The receive-side half of the binding: a bound thread entering a receive takes a word counted
/// while it was elsewhere, and returns without blocking. `None` when the thread has no live binding
/// or the word is zero. Returns the bare word, which comes back in a register.
///
/// `#[cold]` and out of line, so only a bound thread pays for it and its bytes stay out of
/// `script/fastpath-footprint`'s closure.
///
/// Context: caller holds `IPC_TABLES`.
#[cold]
#[inline(never)]
pub(super) fn take_bound_signal(sched: &mut IpcTables, tid: ThreadId) -> Option<u64> {
    let id = sched.threads.get(tid)?.bound_notification?;
    match notification_of(sched, id)?.state.poll() {
        0 => None,
        word => Some(word),
    }
}

/// Tear down every notification whose page lies in `[base, end)`: abort and wake each waiter, then
/// drop the name, rescanning as the rendezvous sweep does (notes/region-reaping.md). A thread bound
/// to one keeps a stale name, which every reader treats as unbound.
///
/// Context: caller holds `IPC_TABLES`.
pub(super) fn reap_region_notifications(sched: &mut IpcTables, base: u64, end: u64) {
    loop {
        let doomed = sched
            .notification_table
            .iter()
            .find(|&(_, &phys)| base <= phys && phys < end)
            .map(|(name, _)| name);
        let Some(name) = doomed else { break };
        if let Some(page) = notification_of(sched, name) {
            page.state.drain_waiters(|w| {
                let tid = hold_token(w);
                set_ipc_aborted(sched, tid);
                wake(sched, tid);
            });
        }
        sched.notification_table.remove(name);
    }
}
