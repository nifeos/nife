//! Interrupt routes: which rendezvous an interrupt line is bound to, and the notification that
//! turns a raised line into a message. See notes/capabilities.md ("An interrupt is a message").
//!
//! Name: provisional (milestone 840's lane, 2026-10-11 UTC; §271 (the scheduler file splits into
//! submodules of `sched`)).

use super::*;

/// intid -> rendezvous id + 1 (0 means "not routed"). A hardware interrupt, delivered as a
/// message to whoever holds the matching rendezvous.
///
/// A plain atomic array, read lock-free from the interrupt handler, where taking a lock to find
/// the destination would be one more thing to go wrong. 256 covers every INTID in use (SGIs 0-15,
/// the timer PPI at 30, virtio SPIs in the 40s).
pub(super) const MAXIMUM_INTID: usize = 256;
#[doc(alias = "irq")]
pub(super) static INTERRUPT_ROUTES: [AtomicU64; MAXIMUM_INTID] =
    [const { AtomicU64::new(0) }; MAXIMUM_INTID];

/// Route hardware interrupt `intid` to the rendezvous `ep`: when it fires, whoever is blocked on
/// `ep` wakes, and if nobody is, the signal is counted so it is not lost.
///
/// From then on `ep` refuses every send (§101 (notification objects), ruling B), because an
/// interrupt arrives as `w0 = 1`, a word any sender could forge. The rendezvous refuses
/// (`Rendezvous::bind_to_interrupt`), so it holds however the endpoint was created; `SEND`,
/// `SEND_CAPABILITY` and `CALL` answer [`abi::Error::NotPermitted`]. Marked before the route is published,
/// so no instant exists where the line is live and the endpoint accepts a send.
///
/// A sender already parked on `ep` is refused the same way, under the same hold: it wakes and its
/// `SEND`, `SEND_CAPABILITY` or `CALL` answers `NotPermitted`, and any capability it staged is
/// dropped. A corpse parked with a death message is dropped, as `deliver_death` drops one that
/// finds `ep` already bound. So the driver's first `RECEIVE` is never handed a sender's words.
///
/// Name: ratified 2026-10-27 (#2797).
#[doc(alias = "irq")]
pub fn bind_interrupt(intid: u32, ep: RendezvousId) {
    assert!(
        (intid as usize) < MAXIMUM_INTID,
        "intid {intid} out of range"
    );
    {
        let mut guard = IPC_TABLES.lock();
        if let Some(sched) = guard.as_mut()
            && let Some(rendezvous) = rendezvous_of(sched, ep)
        {
            // Refuse inside the drain, as `reap_region_objects` wakes inside its own: the pop has
            // freed the intrusive link, and `rendezvous_of`'s `'static` does not hold `sched`.
            rendezvous.bind_to_interrupt(|sender| {
                let tid = hold_token(sender);
                refuse_parked_sender(sched, tid);
            });
        }
    }
    // +1 so 0 keeps meaning "not routed"; a name is never u64::MAX, so this cannot wrap.
    INTERRUPT_ROUTES[intid as usize].store(ep + 1, Ordering::Release);
}

/// **Answer a sender that was parked on a rendezvous when it was bound to an interrupt.** A live
/// sender is aborted with the refusal flag set ([`set_ipc_aborted`] drops a staged capability, and
/// [`set_ipc_refused`] makes the syscall layer answer `NotPermitted`), then woken. A corpse is
/// waiting on nothing now; it is left dead, and its death message is dropped.
///
/// Context: caller holds `IPC_TABLES`; `tid` has left the sender queue and holds its token.
///
/// Name: provisional (2026-11-11 UTC): calef names public items.
#[cold]
#[inline(never)]
fn refuse_parked_sender(sched: &mut IpcTables, tid: ThreadId) {
    let Some(t) = sched.threads.get_mut(tid) else {
        return;
    };
    if t.handshake.state == State::Dead {
        t.handshake.wait_on = None;
        return;
    }
    set_ipc_aborted(sched, tid);
    set_ipc_refused(sched, tid);
    wake(sched, tid);
}

/// The rendezvous an interrupt is routed to, if any. Read from the interrupt handler; lock-free.
///
/// Name: ratified 2026-10-27 (#2797).
#[doc(alias = "irq")]
pub fn interrupt_route(intid: u32) -> Option<RendezvousId> {
    if (intid as usize) >= MAXIMUM_INTID {
        return None;
    }
    match INTERRUPT_ROUTES[intid as usize].load(Ordering::Acquire) {
        0 => None,
        n => Some(n - 1),
    }
}

/// Deliver an interrupt as a message to the rendezvous `ep`: wake a blocked waiter, or count the
/// signal so the next `RECEIVE` returns at once. An interrupt never waits for a receiver and is
/// never lost while the receiver is busy. A stale `ep` drops it.
///
/// Context: interrupt context. Takes `IPC_TABLES`, which the interrupted code cannot hold, because
/// `InterruptSafeMutex` masks interrupts while held (§9 (locking)); sends any SGI after releasing
/// it.
///
/// Name: ratified 2026-10-27 (#2797).
///
/// `#[inline(never)]` keeps these bytes out of `riscv_trap_body`, which `script/fastpath-footprint`
/// counts as syscall entry: folded there, this call and its two neighbors in the riscv64 trap
/// dispatcher grew that entry from 2,010 to 3,116 bytes, though no syscall fetches them
/// (milestone 368 (the entry set is flat)).
#[doc(alias = "irq")]
#[inline(never)]
pub fn interrupt_notify(ep: RendezvousId) {
    // Counted for `vmstat`'s `in` column.
    crate::machine_statistics::interrupt();
    #[cfg(feature = "bench")]
    if WATCHED.load(Ordering::Relaxed) == ep + 1 {
        WATCHED_AT.store(crate::arch::timer::now(), Ordering::Relaxed);
    }
    // Load-aware (§28 (SMP placement), part 2): a remote placement owes an SGI, sent after
    // `IPC_TABLES` is released.
    let remote = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");

        // `signal` wakes a waiting receiver or counts the signal; it never blocks.
        let Some(rendezvous) = rendezvous_of(sched, ep) else {
            return;
        };
        if let Some(waiter) = rendezvous.signal() {
            let waiter = hold_token(waiter);
            let t = sched.threads.get_mut(waiter).unwrap();
            t.mailbox = [1, 0, 0, 0, 0];
            t.handshake.serve(); // the signal is the delivery (the boot-8 gate)
            trace::record(trace::Event::Served, waiter, 7);
            wake_load_aware(sched, waiter)
        } else {
            None
        }
    };
    if let Some(target) = remote {
        crate::arch::interrupt_controller::send_reschedule(target);
    }
}

/// The rendezvous whose interrupts [`interrupt_notify`] timestamps for a bench row, plus one (0
/// means
/// none). See [`watch_interrupt`].
#[cfg(feature = "bench")]
static WATCHED: AtomicU64 = AtomicU64::new(0);
/// The counter reading at the last interrupt delivered to the watched rendezvous.
#[cfg(feature = "bench")]
static WATCHED_AT: AtomicU64 = AtomicU64::new(0);

/// Timestamp every interrupt delivered to `ep` from now on, for a bench row that times a driver
/// from its interrupt to its answer (`bench::keyboard`). Bench builds only: the store costs one
/// compare on every interrupt.
///
/// Name: provisional, minted 2026-10-24 (UTC) for `bench::keyboard`.
#[cfg(feature = "bench")]
#[doc(alias = "irq")]
pub fn watch_interrupt(ep: RendezvousId) {
    WATCHED.store(ep + 1, Ordering::Relaxed);
}

/// The counter reading at the last interrupt [`interrupt_notify`] delivered to the watched
/// rendezvous.
///
/// Relaxed on both sides: a reader learns that the interrupt happened through an IPC the driver
/// sent after waking from it, and that IPC passes through `IPC_TABLES`, which orders this load
/// after the store.
///
/// Name: provisional, minted 2026-10-24 (UTC) for `bench::keyboard`.
#[cfg(feature = "bench")]
#[doc(alias = "irq")]
pub fn watched_interrupt_at() -> u64 {
    WATCHED_AT.load(Ordering::Relaxed)
}
