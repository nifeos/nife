//! Where a thread runs: spawn placement, the secondary cores' idle loops and inboxes, work
//! stealing, and the choice of a core for a woken thread. See notes/scheduler.md.
//!
//! Name: provisional (milestone 840's lane, 2026-10-11 UTC; §271 (the scheduler file splits into
//! submodules of `sched`)).

use super::*;

/// Make this secondary core a scheduler participant, once, as it comes online.
///
/// It adopts the context it is running on as this core's idle thread, as [`init`] does for the
/// boot core. The core's run queue is then empty, so it runs its idle thread until work lands.
///
/// Context: interrupts masked (the caller has not enabled them yet); takes `IPC_TABLES`.
pub fn adopt_secondary_idle() {
    // This core is about to own threads, so its FP state is set up first, as `init` does.
    crate::arch::fp::init();

    let id = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard
            .as_mut()
            .expect("adopt_secondary_idle before sched::init");
        let id = sched
            .threads
            .insert_in_place(|tid, dst| {
                // SAFETY: `dst` is a fresh, exclusively-owned TCB page, per `insert_in_place`.
                unsafe { Thread::write_adopted_current(dst, tid) };
                true
            })
            .expect("thread table full while bringing a core online");
        // This core is currently running that thread.
        let tcb = sched
            .threads
            .pointer(id)
            .expect("the idle thread was just inserted");
        // SAFETY: `id`'s page pointer from the table, live.
        unsafe { set_current(id, tcb) };
        id
    };

    // And it is also this core's idle fallback.
    cpu::current().idle.store(id, Ordering::Relaxed);
}

/// The reschedule SGI: a core that hands another a thread through its inbox fires this at the
/// target, whose handler drains the inbox and reschedules. INTID 0, distinct from the
/// rendezvous-bound test SGIs (1 and 2). aarch64 only: riscv64 recognizes the IPI by its SBI
/// cause, and `x86_64` sends its own reschedule IPI.
#[cfg_attr(not(target_arch = "aarch64"), allow(dead_code))]
pub const RESCHED_SGI: u32 = 0;

/// Drain this core's migration inbox into its run queue, and request a reschedule.
///
/// Context: the reschedule-SGI handler, in interrupt context with interrupts masked, which
/// `with_runq` needs. Takes only this core's inbox lock (§11 (SMP)).
pub fn drain_inbox() {
    // Every caller is a cross-core interrupt arm, so the machine statistics page counts it here.
    crate::machine_statistics::interrupt();
    let mut moved = 0u64;
    let mut inbox = cpu::current().inbox.lock();
    while let Some(thread) = inbox.pop_front() {
        // Pure token movement, nothing dereferenced, so no `IPC_TABLES`.
        cpu::current().with_runq(|q| q.push_back(thread));
        moved += 1;
    }
    // Mirror the empty inbox under its lock (§28 (SMP placement)); the run queue's mirror moved by
    // the same count.
    cpu::current().note_inbox_len(inbox.len());
    // The one place a remote placement arrives on this core.
    cpu::current().note_adopted(moved);
    drop(inbox);
    if moved > 0 {
        trace::record(trace::Event::InboxDrain, moved, 0);
        cpu::current().need_resched.store(true, Ordering::Relaxed);
    }
}

/// Put an already-created thread onto core `target`'s run queue: this core's own, or the target's
/// inbox. Returns the core that owes a reschedule SGI (`Some(target)` for a remote inbox), which
/// the caller sends after releasing `IPC_TABLES`. The caller must act on this answer and never compare
/// `target` with [`cpu::id()`] again: outside the lock it can be stolen to another core in between,
/// and a skipped SGI strands the thread in an inbox nothing drains. The inbox lock's
/// release/acquire orders the caller's thread-table insert before the target's drain (§11 (SMP)).
/// See notes/scheduler/missed-reschedule-poke.md.
///
/// Context: caller holds `IPC_TABLES` (interrupts masked); takes the target's inbox lock.
pub(super) fn place_on(target: usize, thread: Unqueued<Thread>) -> Option<usize> {
    // A remote parked core's inbox is drained by nothing (notes/visionfive2.md, third stop).
    // Placing onto oneself is exempt: a secondary's bring-up probe spawns onto its own core one
    // step before that core sets its online bit, and a local push needs no drain.
    debug_assert!(
        target == cpu::id() || {
            let mask = crate::smp::online_harts_mask();
            mask & (1 << target) != 0
        },
        "placement onto parked cpu {target} (online mask {:#b}): nothing drains a parked core's \
         inbox, so this thread would never run",
        crate::smp::online_harts_mask(),
    );
    if target == cpu::id() {
        cpu::current().with_runq(|q| q.push_back(thread));
        None
    } else {
        // Read before the push gives the token away. SAFETY: a token's thread is live while the
        // token exists (`Threads::mint_token`), and IPC_TABLES (held) serializes every `Thread`.
        let tid = unsafe { (*thread.as_ptr()).id };
        // The inbox mutex serializes access to the link.
        let mut inbox = cpu::inbox_of(target).lock();
        inbox.push_back(thread);
        // Mirror the inbox depth so it counts as load (§28), under the inbox lock.
        cpu::of(target).note_inbox_len(inbox.len());
        trace::record(trace::Event::PlaceRemote, tid, target as u8);
        Some(target)
    }
}

/// Which core a new thread should run on: the power of two choices (§28.1). Samples two random
/// online cores' runnable counters (relaxed, possibly stale) and returns the lighter; on one online
/// core it is this one.
pub(super) fn pick_spawn_target() -> usize {
    let n = crate::smp::online_count();
    if n <= 1 {
        return cpu::id();
    }
    // The k-th online CPU, not index k: the online set is not contiguous from zero.
    let a = crate::smp::nth_online(cpu::current().rng_next() as usize);
    let b = crate::smp::nth_online(cpu::current().rng_next() as usize);
    if cpu::of(a).runnable() <= cpu::of(b).runnable() {
        a
    } else {
        b
    }
}

/// Serve a pending work-steal request (§28.3): hand an idle requester one thread from this core's
/// run queue, never the running one, and poke it. An empty queue gives nothing, and the requester
/// asks again next tick.
///
/// Context: interrupt context (the reschedule-SGI handler), interrupts masked; takes the
/// requester's inbox lock.
pub fn serve_steal_request() {
    // The acquire/release pairing and the read-and-clear are in `steal_request`, where loom checks
    // them (notes/interleaving.md).
    let Some(requester) = cpu::current().steal_request.take() else {
        return;
    };
    let requester = requester as usize;
    // One thread off our own queue, if any. `with_runq` keeps the runnable mirror exact.
    let thread = cpu::current().with_runq(|q| q.pop_front());
    if let Some(t) = thread {
        // SAFETY: reading the id of the thread we just popped and hold exclusively.
        let tid = unsafe { (*t.as_ptr()).id };
        trace::record(trace::Event::StealServe, tid, requester as u8);
        // The requester's inbox mutex orders this pop before its drain, as in `place_on`.
        let mut inbox = cpu::inbox_of(requester).lock();
        inbox.push_back(t);
        cpu::of(requester).note_inbox_len(inbox.len());
        crate::arch::irq::send_reschedule(requester);
    }
}

/// An idle core asks the most-loaded other core for work (§28.3): if it has a queued thread to
/// spare, claim its one-slot steal request and poke it. The victim serves it at its next scheduler
/// entry, and the stolen thread arrives through this core's inbox.
pub(super) fn try_initiate_steal() {
    // Work already arriving in this core's inbox counts in `runnable`, so defer to it.
    if cpu::current().runnable() > 0 {
        return;
    }
    let me = cpu::id();
    let mut victim = None;
    let mut best = 0usize;
    for c in crate::smp::online_cpus() {
        if c != me {
            // Steal only a run-queue backlog, never a victim's inbox in transit (see `runq_len`).
            let r = cpu::of(c).runq_len();
            if r > best {
                best = r;
                victim = Some(c);
            }
        }
    }
    if let Some(v) = victim
        && cpu::of(v).steal_request.claim(me as u32)
    {
        crate::arch::irq::send_reschedule(v);
    }
}

/// The idle thread's body, shared by the boot core's idle thread and every secondary's. Each pass
/// tries to steal (§28), parks in `wfi` until an interrupt, then yields to whatever arrived. Never
/// returns, and never sits in a run queue.
///
/// It does not drain its own inbox before parking (§133 (whether an idle core should drain its own
/// inbox before parking)): a missed reschedule SGI then wedges loudly, with a watchdog dump naming
/// the stranded thread, instead of starting threads late. See
/// notes/scheduler/missed-reschedule-poke.md.
pub fn run_idle() -> ! {
    loop {
        // Gauges, here because this is where the kernel reaches after every phase of a boot with
        // nothing else to do (`cap::report_peak`).
        crate::cap::report_peak();
        crate::progenitor_stack::report_peak();
        try_initiate_steal();
        crate::arch::wait_for_interrupt();
        yield_now();
    }
}

/// Which core a device-interrupt wake should place its driver on (§28.2): the least-loaded online
/// core, with this core winning ties, so a driver that takes an interrupt per request stays put
/// until a core is strictly lighter. A full scan is fine: `MAX_CPUS` is small.
pub(super) fn pick_wake_target() -> usize {
    let here = cpu::id();
    let mut best = here;
    let mut best_load = cpu::current().runnable();
    // The online set, never `0..count`: a parked slot's zero `runnable()` would win every time.
    for c in crate::smp::online_cpus() {
        if c == here {
            continue;
        }
        let load = cpu::of(c).runnable();
        if load < best_load {
            best_load = load;
            best = c;
        }
    }
    best
}

/// A device-interrupt wake (§28.2): load-aware, not local, placed on [`pick_wake_target`]'s choice,
/// because an interrupt carries no locality. Returns `Some(target)` when that is a remote core, so
/// the caller sends the reschedule SGI after releasing `IPC_TABLES`; `None` when it stayed local or
/// the wake was refused or deferred. See notes/scheduler.md ("Wakes").
///
/// Context: caller holds `IPC_TABLES`.
pub(super) fn wake_load_aware(sched: &mut IpcTables, tid: ThreadId) -> Option<usize> {
    let t = sched.threads.get_mut(tid)?;
    // The decision is `Handshake::try_wake`, which loom searches (notes/interleaving.md).
    match t.handshake.try_wake() {
        WakeVerdict::NotBlocked => None,
        WakeVerdict::Refused => {
            trace::record(trace::Event::WakeRefused, tid, 0);
            None
        }
        WakeVerdict::Deferred => {
            // Forward progress for the watchdog; a deferred wake completes in `finish_switch`
            // without a placement policy.
            #[cfg(any(test, feature = "system_tests"))]
            crate::testing::note_progress();
            trace::record(trace::Event::WakeDeferred, tid, 0);
            None
        }
        WakeVerdict::Queue => {
            #[cfg(any(test, feature = "system_tests"))]
            crate::testing::note_progress();
            // Blocked -> Ready: whatever unlinked it handed its token back (`hold_token`).
            let Some(token) = t.own_token.take() else {
                missing_token()
            };
            trace::record(trace::Event::Wake, tid, 0);
            // `place_on`'s answer is the only thing that may tell `irq_notify` to poke the target.
            place_on(pick_wake_target(), token)
        }
    }
}
