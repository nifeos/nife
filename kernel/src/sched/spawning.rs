//! Spawning a kernel thread: [`spawn`], [`spawn_on`] a named core, and [`spawn_with_quota`]. See
//! notes/scheduler.md ("Placement").
//!
//! The other end of a thread's life (`exit`, `exit_thread`, `fault` and the departure they share)
//! is in `sched.rs`, because `syscall::dispatch` reaches it and `script/fastpath-footprint`
//! measures it as part of `syscall_entry`.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::*;

#[must_use = "a remote placement owes a reschedule SGI once IPC_TABLES is released, or the thread \
              sits in an inbox nothing drains"]
/// Spawn a thread running `f` and place it on core `target`, the cross-core placement primitive.
///
/// A remote target is handed the thread through its inbox and then poked with the reschedule SGI,
/// once `IPC_TABLES` is released. Returns the new thread's id, or `None` if the thread table or the
/// kernel budget is full.
///
/// Context: takes `IPC_TABLES`, and sends the SGI after releasing it.
pub fn spawn_on<F: FnOnce() + Send + 'static>(target: usize, f: F) -> Option<ThreadId> {
    let (id, remote) = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut()?;
        // Built on its own TCB page rather than moved there: a `Thread` carried by value put this
        // generic function's frame over the 4096-byte guard page.
        let id = sched.threads.insert_in_place(|tid, dst| {
            // SAFETY: `dst` is the fresh, exclusively-ours TCB page `insert_in_place` claimed; it
            // is aligned for `Thread` and holds no live one, so `write` drops nothing.
            unsafe { Thread::spawn_into(f, tid, dst) }
        })?;
        // Recorded here rather than in `place_on`, which is also the wake path: one store per
        // creation instead of one per wake on the IPC fastpath.
        if let Some(t) = sched.threads.get_mut(id) {
            t.placement = target as u8;
        }
        // Decided once, here, with interrupts masked, and carried out as a value: re-deriving it
        // from `target != cpu::id()` below races a steal onto another core (see `place_on`).
        let remote = place_on(target, take_token(sched, id));
        (id, remote)
    }; // IPC_TABLES released here, before the SGI, so the target's schedule() can take it

    if let Some(target) = remote {
        // Poke the target: its handler drains the inbox we just pushed to and reschedules.
        crate::arch::irq::send_reschedule(target);
    }
    Some(id)
}

/// Spawn a thread running `f` on the lighter of two randomly sampled online cores (§28 (SMP
/// placement)), carried there by [`spawn_on`]. Returns its id, or `None` if the table or the kernel
/// budget is full.
pub fn spawn<F: FnOnce() + Send + 'static>(f: F) -> Option<ThreadId> {
    spawn_on(pick_spawn_target(), f)
}

/// Spawn a thread running `f` against a quota: at most `budget` of these may be alive at once.
///
/// Reserving a slot is an atomic decrement; the slot rides in the `Thread` as a [`QuotaToken`] and
/// comes back when the thread is reaped. Returns `None` if the budget is exhausted or the kernel is
/// out of memory, which the caller need not tell apart: either way it must degrade, not panic.
///
/// No caller: a userspace process spawns out of its own untyped budget, which is its quota. Kept as
/// the bound for kernel threads, allowed unconditionally (§38 (a suppression is scoped to an item
/// and carries a reason), disposition 3). See notes/quotas.md.
#[allow(dead_code)]
pub fn spawn_with_quota<F: FnOnce() + Send + 'static>(
    budget: &'static AtomicU32,
    f: F,
) -> Option<ThreadId> {
    // Decrement only if there is a slot, so the count never dips below zero.
    let mut remaining = budget.load(Ordering::Relaxed);
    loop {
        if remaining == 0 {
            return None;
        }
        match budget.compare_exchange_weak(
            remaining,
            remaining - 1,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(actual) => remaining = actual,
        }
    }

    // The reserved slot is held by this token from here on, so every early return below hands it
    // back by dropping it, and a thread that is built carries it until it is reaped.
    let token = QuotaToken::new(budget);

    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut()?;
    // Built in place on its TCB page, as `spawn_on` does.
    let id = sched.threads.insert_in_place(|tid, dst| {
        // SAFETY: `dst` is the fresh, exclusively-ours TCB page `insert_in_place` claimed.
        unsafe { Thread::spawn_into(f, tid, dst) }
    })?;
    // `insert_in_place` just returned this id, so the thread is there.
    if let Some(t) = sched.threads.get_mut(id) {
        t.quota = Some(token); // returned to `budget` when the thread is reaped
    }
    let token = take_token(sched, id);
    // This core's queue: IPC_TABLES held, IRQs masked.
    cpu::current().with_runq(|q| q.push_back(token));
    Some(id)
}

/// Hold off an armed kill while the current kernel thread builds a user address space.
///
/// `schedule` ends a killed thread wherever it finds it running, and a spawn closure holds the
/// space it is loading as a local until [`adopt_address_space`] gives it to the registry, so a kill
/// in between would lose the image and its page tables for the boot. Pair it with [`allow_kill`]
/// right after the space is adopted; nothing in between may block. See notes/frames.md.
///
/// Name: provisional (lane/frame-ledger-overrun, 2026-10-22 UTC).
pub fn defer_kill() {
    set_kill_deferred(true);
}

/// **End [`defer_kill`]'s window**, and die here if a kill arrived during it. The space is the
/// thread's now, so the reaper frees it like any other corpse's.
///
/// Name: provisional (lane/frame-ledger-overrun, 2026-10-22 UTC).
pub fn allow_kill() {
    if set_kill_deferred(false) {
        yield_now(); // `schedule` carries the kill out and does not return
    }
}

/// Set the current thread's deferral; returns whether a kill is armed on it.
fn set_kill_deferred(on: bool) -> bool {
    let current = current_thread_id();
    let mut guard = IPC_TABLES.lock();
    let t = guard
        .as_mut()
        .and_then(|sched| sched.threads.get_mut(current))
        .expect("no current thread");
    t.kill_deferred = on;
    t.killed
}
