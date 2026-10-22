//! **Spawning a kernel thread**: [`spawn`], [`spawn_on`] a named core, and [`spawn_with_quota`].
//!
//! Cut out of `sched.rs` unchanged on 2026-10-12 (UTC) by lane maintainer/near-ceiling-splits, so
//! the parent stays under §266 (a Rust source file stays under 2,000 lines) and moves toward §282
//! (when a file is split, the parent becomes an index). The other end of a thread's life (`exit`,
//! `exit_thread`, `fault` and the departure they share) stays in `sched.rs`: `syscall::dispatch`
//! reaches it, and moving it changed what `script/fastpath-footprint` measures as `syscall_entry`
//! (aarch64 +16 B when it was tried).
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::*;

/// Put an already-created thread onto core `target`'s run queue. Caller holds `IPC_TABLES`.
///
/// Local: straight onto our own queue (`IPC_TABLES` masks interrupts, which `with_runq` needs). Remote:
/// into the target's inbox, and the SGI (sent after `IPC_TABLES` is released, by the caller) makes it
/// drain. The inbox push under `IPC_TABLES` is rank-safe (INBOX < `IPC_TABLES`), and the inbox's own lock supplies
/// the release/acquire that orders our thread-table insert before the target's drain (§11).
///
/// **Returns the core that owes an SGI**, `Some(target)` when the thread went into a remote inbox
/// and `None` when it went onto this core's own run queue, and that return value is the whole
/// point rather than a convenience. A caller must not decide "was this remote?" a second time by
/// comparing `target` against [`cpu::id()`] again: this function runs under `IPC_TABLES`, which
/// masks interrupts, while the caller's second comparison does not, so the calling thread can be
/// preempted and **stolen onto a different core** in between and the two answers disagree. When
/// they disagree in the direction that skips the SGI, the placed thread sits `Ready` in a remote
/// core's inbox that nothing will ever drain (only the reschedule-SGI handler calls
/// [`drain_inbox`]), the idle target refuses to steal because [`cpu::PerCpu::runnable`] counts that
/// inbox as its own work, and the machine wedges with every core idle. That is the 2026-08-28
/// riscv64 CPU-matrix hang, whose trace ring caught the migration in the act:
///
/// ```text
/// core 2: ... switch:0x0 ... switch:0x1000000076 steal:0x0/3 ...   gave tid 0 away to core 3
/// core 3: ... drain:0x1 switch:0x0 place:0x500000077/2 block:0x0/177 switch:0x4
/// ```
///
/// Tid 0 read `target == cpu::id()` on core 2, was preempted and stolen to core 3, and finished the
/// same `spawn_on` there: the push went to core 2's inbox and the stale "local" answer skipped the
/// poke.
///
/// **A skipped SGI is usually invisible, which is what makes it dangerous.** The next SGI aimed at
/// that core for any reason drains the whole inbox, so a strand is normally repaired within
/// milliseconds and nothing is ever seen. It wedges only when the stranded thread is the work
/// everything else was about to wait on, so no further SGI is generated. Do not take "it has not
/// hung" as evidence that a placement path pokes correctly. See notes/scheduler.md.
#[must_use = "a remote placement owes a reschedule SGI once IPC_TABLES is released, or the thread \
              sits in an inbox nothing drains"]
/// Spawn a thread and place it on a **specific** core (SMP step 3c).
///
/// The cross-core placement primitive. `spawn` puts work on the calling core; this puts it on
/// `target`, which is what lets the machine actually spread load. A remote target is handed the
/// thread through its inbox and then poked with the reschedule SGI. (Wiring `spawn` itself to
/// round-robin over `target` is the trivial next step, once the mechanism is proven.)
pub fn spawn_on<F: FnOnce() + Send + 'static>(target: usize, f: F) -> Option<ThreadId> {
    let (id, remote) = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut()?;
        // **The Thread is built on its own TCB page, not carried there** (milestone 124). The old
        // shape called `Thread::spawn(f)` for a value and moved it through this closure, and every
        // instantiation of this generic function carried 3888 to 4592 bytes of frame as a result:
        // over the 4096-byte guard page, which is the size at which one frame can step past the
        // guard in a single move and corrupt the neighbouring stack with no fault at all.
        let id = sched.threads.insert_in_place(|tid, dst| {
            // SAFETY: `dst` is the fresh, exclusively-ours TCB page `insert_in_place` claimed; it
            // is aligned for `Thread` and holds no live one, so `write` drops nothing.
            unsafe { Thread::spawn_into(f, tid, dst) }
        })?;
        // Record where it was placed, so a survey can report it (`abi::survey::record::PLACEMENT`).
        // Here rather than inside `place_on`, and that siting is the whole cost argument: `place_on`
        // is also the wake path, so a store there would be a store per wake on the IPC fastpath,
        // which is what put `Thread::last_cpu` behind a soak-build feature. One store per thread
        // creation is free by comparison. `insert_in_place` just returned this id, so the `if let`
        // is belt and braces rather than a case that happens.
        if let Some(t) = sched.threads.get_mut(id) {
            t.placement = target as u8;
        }
        // The placement decision is made ONCE, here, with interrupts masked, and carried out of
        // the critical section as a value. Re-deriving it below from `target != cpu::id()` is the
        // lost-wakeup bug `place_on` documents: this thread can be stolen onto another core
        // between the two reads.
        let remote = place_on(target, take_token(sched, id));
        (id, remote)
    }; // IPC_TABLES released here, before the SGI, so the target's schedule() can take it

    if let Some(target) = remote {
        // Poke the target: its handler drains the inbox we just pushed to and reschedules.
        crate::arch::irq::send_reschedule(target);
    }
    Some(id)
}

pub fn spawn<F: FnOnce() + Send + 'static>(f: F) -> Option<ThreadId> {
    // Placement is the power of two choices (DECISIONS §28): the new thread lands on the lighter of
    // two randomly sampled cores, not always on the spawner's, so work spreads instead of piling on
    // one core beside idle ones (the FS-server starvation lesson). `spawn_on` carries the thread to
    // the chosen core over the §11 inbox/SGI path.
    spawn_on(pick_spawn_target(), f)
}

/// Spawn a thread against a **quota**: at most `budget` of these may be alive at once.
///
/// Reserving a slot is an atomic decrement; the slot lives inside the spawned `Thread` as a
/// [`QuotaToken`] and comes back when the thread is reaped. Returns `None` if the budget is
/// exhausted (too many children already alive) OR the kernel is out of memory: the caller cannot
/// tell the two apart, and does not need to: either way it could not spawn, and it must degrade
/// rather than panic. This is the bound that stops a spawn flood or a leaked-thread pile-up from
/// exhausting kernel memory. See notes/quotas.md and notes/security.md.
///
/// # It has no caller today, and that is worth saying plainly
///
/// Its one caller was the kernel-wired `shell_service`'s spawn service, which DECISIONS §28 retired
/// and milestone 41 deleted. Nothing in the kernel spawns against a quota now, because **the bound
/// moved**: a userspace process spawns out of its own untyped budget (§10, §16), so the budget *is*
/// the quota and it is enforced by retyping rather than by a counter. This function is the bound for
/// *kernel* threads, and no kernel thread is currently spawned in a loop by anything untrusted.
///
/// Kept rather than deleted because removing a documented safety mechanism is a design decision, not
/// dead-code triage, and notes/quotas.md and notes/security.md both describe it. Allowed
/// unconditionally and on purpose (DECISIONS §38, disposition 3): there is no configuration in which
/// something calls it, and pretending otherwise with a `cfg` predicate would be the dishonest option.
#[allow(dead_code)]
pub fn spawn_with_quota<F: FnOnce() + Send + 'static>(
    budget: &'static AtomicU32,
    f: F,
) -> Option<ThreadId> {
    // Reserve a slot: decrement only if there is one. A compare-exchange loop, so it is exactly
    // one atomic decrement and it never dips below zero (returning `None` = "quota exhausted").
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
    // Built in place on its TCB page rather than as a value (see `spawn_on`, milestone 124): the
    // by-value path held three `Thread`s in one frame in an unoptimised build.
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

/// **Hold off an armed kill while the current kernel thread builds a user address space.**
///
/// `schedule` carries out a kill on whatever thread it finds running, kernel code included, and a
/// thread it converts is never resumed. A spawn closure holds the space it is loading as a local on
/// its own stack until [`adopt_address_space`] gives it to the registry, so a kill landing between
/// the two leaves that local undropped: the image, its page tables and its revocation log, about
/// 22 frames, are lost for the boot. `Holding::release` arms exactly that kill when a test ends
/// before the service it started has finished loading, and the frame ledger reported it as an
/// intermittent overrun. See notes/frames.md.
///
/// Pair it with [`allow_kill`] right after the space is adopted. Nothing in between may block.
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
