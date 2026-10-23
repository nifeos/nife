//! **The switch**: adopting the boot context as thread 0, the queue tokens a switch moves,
//! `schedule` and the work `finish_switch` does on the far side, and the per-thread state
//! installed on the core about to run a thread. See notes/scheduler/switch.md.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// Adopt the context we are already running in as thread 0, and create this core's idle thread.
///
/// The boot thread has no stack of its own and no saved context; the first switch away from it
/// writes one. Panics if a fresh table refuses an insert or the idle thread cannot be built. See
/// notes/scheduler/switch.md.
///
/// Context: takes `IPC_TABLES` twice, never across the spawn.
pub fn init() {
    // The FP unit is shut before any thread exists: a core that starts with it open never takes
    // the first-use trap that marks a thread `live`. Here, not in `arch::init`, because RISC-V's
    // boot hart never calls `arch::init` and OpenSBI hands it over with `sstatus.FS` set.
    crate::arch::fp::init();

    // The machine statistics page begins where threads begin (DECISIONS §225 (`free` sees the
    // machine and your share)).
    crate::machine_statistics::publish();

    let mut sched = IPC_TABLES.lock();

    // Install the empty tables first, then insert the boot thread into them: built as a local and
    // moved in, an unoptimised build carries the thread table twice on the boot stack, which caps
    // [`MAX_THREADS`]. Assigning the `const` [`EMPTY_TABLES`] copies from `.rodata`.
    *sched = Some(EMPTY_TABLES);
    let tables = sched
        .as_mut()
        .expect("the tables were just installed on this line");
    // The first name a fresh table mints is 0 (slot 0, generation 0), so the boot thread is tid 0.
    let boot_tid = tables
        .threads
        .insert_with(|tid| {
            let mut boot = Thread::boot();
            boot.id = tid;
            boot
        })
        .expect("a fresh table refused its first insert");

    // This core (core 0) is running the boot thread.
    let boot_tcb = tables
        .threads
        .pointer(boot_tid)
        .expect("the boot thread was just inserted");
    // SAFETY: `boot_tid`'s page pointer from the table, live.
    unsafe { set_current(boot_tid, boot_tcb) };

    drop(sched); // release before spawning, which takes the lock itself

    // The idle thread, built in place on its own TCB page as `spawn_on` builds every kernel thread:
    // a by-value build overflows the frame's guard page in an unoptimised build.
    let mut sched = IPC_TABLES.lock();
    let s = sched.as_mut().unwrap();
    let idle_id = s
        .threads
        .insert_in_place(|tid, dst| {
            // SAFETY: `dst` is the fresh, exclusively-ours TCB page `insert_in_place` claimed.
            unsafe { Thread::spawn_into(|| run_idle(), tid, dst) }
        })
        .expect("could not create the idle thread (no kernel stack, no TCB page, or a full table)");
    drop(sched);
    // NOT pushed onto `ready`: the idle thread is a fallback, not a peer.
    cpu::current().idle.store(idle_id, Ordering::Relaxed);
}

/// The queue-able pointer to live thread `tid`, good until the thread is popped. Panics on a
/// dead `tid`.
///
/// The queue discipline keeps it valid: a queued thread is `Ready` or `Blocked`, the reaper frees
/// only `Finished` ones, and the table pins the TCB page. [`core::ptr::NonNull`] because it comes
/// from a `&mut Thread`; that the pointee outlives its time on the queue stays the caller's rule.
/// See notes/scheduler/switch.md.
///
/// Context: caller holds `IPC_TABLES`.
pub(super) fn thread_control_block_ptr(
    sched: &mut IpcTables,
    tid: ThreadId,
) -> core::ptr::NonNull<Thread> {
    core::ptr::NonNull::from(
        sched
            .threads
            .get_mut(tid)
            .expect("thread_control_block_ptr of a dead thread"),
    )
}

/// **Take `tid`'s queue token off its own TCB, to put it on a queue.** Caller holds `IPC_TABLES`.
///
/// The token is there exactly when the thread is on no queue (see `thread::Thread::own_token`), so
/// a `None` here is a thread somebody is about to put on a second queue ([`missing_token`]).
#[inline(always)]
pub(super) fn take_token(sched: &mut IpcTables, tid: ThreadId) -> Unqueued<Thread> {
    // Two `let`-`else`s rather than a combinator chain: the icount gate measures a debug build,
    // where each closure is a call.
    let Some(t) = sched.threads.get_mut(tid) else {
        missing_token()
    };
    let Some(token) = t.own_token.take() else {
        missing_token()
    };
    token
}

/// **A thread that should hold its token does not**: it is on a queue already (and is about to be
/// put on a second), or it is a running thread whose token went somewhere it should not. Loud in
/// every build, because the alternative is the corrupted list the token exists to rule out.
///
/// One cold function for every site rather than an `expect` at each, which would set up a message
/// and a location inline on the fastpath closures.
#[cold]
#[inline(never)]
pub(super) fn missing_token() -> ! {
    panic!("a thread without its queue token: on a queue already, or its token was lost")
}

/// **Give a token a queue just handed back to the thread it names, and say which thread that is.**
/// Every pop and removal in this file ends here or on another queue, so the thread holds its token
/// again before anything decides what happens to it. Caller holds `IPC_TABLES`.
#[inline(always)]
pub(super) fn hold_token(token: Unqueued<Thread>) -> ThreadId {
    hold_token_reading_order(token).0
}

/// [`hold_token`], also answering whether the thread carries a suspension order
/// (`Thread::suspend_requested`), read through the pointer already in hand. For the pick, where a
/// second table lookup per switch cost bytes on every IPC closure.
#[inline(always)]
pub(super) fn hold_token_reading_order(token: Unqueued<Thread>) -> (ThreadId, bool) {
    // SAFETY: a token's thread is live for as long as the token exists (`Threads::mint_token`'s
    // first obligation), `IPC_TABLES` (held by every caller) serializes access to every `Thread`,
    // and no other reference to this one is live: the caller has just taken it off a queue.
    let t = unsafe { &mut *token.as_ptr() };
    t.own_token = Some(token);
    (t.id, t.suspend_requested)
}

/// Install the incoming thread's cycle-counter grant (`granted`) on this core, right after its
/// address-space root (DECISIONS §139 (who may read the cycle counter) option 4).
///
/// Built only under `test`, `system_tests` or `cycle_counter_grant`. The read in `schedule` and
/// this install are `#[cfg]`-gated rather than carried through the switch tuple, because the debug
/// build the icount gate measures does not fold a constant `false` away. With the feature off
/// the counter keeps the closed value boot wrote. See notes/scheduler/switch.md.
#[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
pub(super) fn install_cycle_counter_grant(granted: bool) {
    crate::arch::timer::set_cycle_counter_grant(granted);
}

/// Install the incoming thread's x86 port grant (`grant`) on this core, at the last point
/// `IPC_TABLES` is held, so a revocation's NMI sweep cannot race it. The lazy write lives in
/// `arch::segments`.
///
/// `x86_64` only, `#[cfg]`-gated at the switch site rather than carried through the switch tuple,
/// for `install_cycle_counter_grant`'s reason. See DECISIONS §152 (the port-range capability) and
/// notes/scheduler/switch.md.
///
/// Context: caller holds `IPC_TABLES` with interrupts masked.
#[cfg(target_arch = "x86_64")]
pub(super) fn install_port_grant(grant: Option<(u16, u16)>) {
    crate::arch::segments::set_port_range_grant(grant);
}

/// Pick another thread and go there.
///
/// May be called from normal context (a voluntary `yield_now`) or from the tail of the timer
/// IRQ handler (a preemption); the two paths are identical from here down. Returns when some
/// later switch comes back to this thread. See notes/scheduler/switch.md.
///
/// Context: takes `IPC_TABLES`, never across the switch; masks interrupts across the decision
/// and the switch. Must not run on an interrupt stack.
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(target_os = "none", unsafe(link_section = ".text.hot.sched.schedule"))]
pub fn schedule() {
    // Never on an interrupt stack: the switch parks `sp` in the outgoing context, and the next
    // interrupt on this core overwrites that stack. Debug-only; `script/stack-depth-check` proves
    // it statically, and this catches the edges a call graph cannot see.
    debug_assert!(
        !crate::interrupt_stack::contains(crate::arch::current_sp()),
        "schedule() called on an interrupt stack: the outgoing thread would be parked on memory \
         that belongs to a core (see kernel/src/interrupt_stack.rs)",
    );

    // Rule 2: no interrupts across the decision or the switch, or the timer could choose again in
    // between. `was_enabled` is on this thread's stack, so it is still here when a switch returns.
    let was_enabled = crate::arch::interrupts::disable();

    // The incoming cycle-counter grant: written under the lock, installed after it drops, `false`
    // unless the block switches. See `install_cycle_counter_grant`.
    #[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
    let mut next_cycle_counter = false;

    // A labeled block, so every exit drops the guard before interrupts are restored, once, after
    // it: restoring them with `IPC_TABLES` held lets a timer re-enter `schedule()` and retake it.
    let switch = 'decide: {
        let mut guard = IPC_TABLES.lock();
        let Some(sched) = guard.as_mut() else {
            break 'decide None;
        };

        let current = current_thread_id();

        // A killed thread (DECISIONS §16 (object revocation) amendment) never runs again, and a
        // suspension order (§300 (the paused state, a TCB verb, silent)) parks a running one. Both
        // are carried out before the pick, so every path below sees them, "keep current" included.
        if sched
            .threads
            .get(current)
            .is_some_and(|t| t.killed || t.suspend_requested)
        {
            carry_out_orders(sched, current);
        }

        // The pick: this core's queue, the idle thread when it is empty, and the outgoing thread
        // requeued if it can still run. `None` keeps the current thread with no switch.
        let Some(next) = switch_decision::choose_next(sched, current) else {
            break 'decide None;
        };

        // Running, with `on_cpu` set until its successor's `finish_switch`. A core crossing is
        // recorded here, where every path to a CPU passes, and only under `soak_test`, because
        // this is the hottest line in the kernel (see `thread::Thread::last_cpu`).
        #[cfg(feature = "soak_test")]
        {
            let here = cpu::id();
            let t = sched.threads.get_mut(next).unwrap();
            let from = t.last_cpu;
            t.last_cpu = here as u8;
            if from != u8::MAX && from as usize != here {
                trace::record(trace::Event::Migrated, next, from);
            }
        }
        let next_tcb = sched.threads.pointer(next).unwrap();
        // SAFETY: the page pointer of a live thread, and `IPC_TABLES` (held) serializes every
        // access to its `Thread`. One lookup serves the handshake and `set_current` both.
        unsafe { (*next_tcb).handshake.switch_in() };
        // SAFETY: `next_tcb` is `next`'s page pointer from the table, live.
        unsafe { set_current(next, next_tcb) };
        trace::record(trace::Event::SwitchTo, next, 0);

        // The incoming thread finishes the outgoing one after the switch, once it is off its
        // stack; not here, and not on another core, because we are still running on it. See
        // `finish_switch`. The switch count rides the same block (`PerCpu::switches`).
        let here = cpu::current();
        here.switched_from.store(current, Ordering::Relaxed);
        here.switches.fetch_add(1, Ordering::Relaxed);

        // Read at the last point the lock is held, into the variable above, not the switch tuple.
        #[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
        {
            next_cycle_counter = sched.threads.get(next).unwrap().cycle_counter_grant;
        }

        // The incoming root and the four raw pointers the switch needs, read under the lock.
        // SAFETY: the lock is held and interrupts are masked, both tids are live, and `next_tcb`
        // is `next`'s page pointer, switched in and made current just above.
        Some(unsafe { switch_decision::switch_operands(sched, current, next, next_tcb) })
    };
    // Rule 1: THE LOCK IS RELEASED HERE, before the switch. Holding it across `switch_to` would
    // leave it held by a thread that is not running, and the next thread to want it would spin
    // forever waiting for a thread that can only be scheduled by taking the lock.

    if let Some((prev_slot, next_ctx, next_root, prev_fp, next_fp)) = switch {
        // The incoming address space first: `TTBR0_EL1` is shared, and a thread resuming at EL0
        // in the previous thread's low half runs a stranger's code. A no-op when already right.
        //
        // SAFETY: `next_root` is `reserved_root()`, or the composed value of the `AddressSpace`
        // owned by thread `next`, which the block above popped off this core's run queue and marked
        // `Running` with `on_cpu` set before releasing `IPC_TABLES`. No other core can pick it up in that
        // state and nothing reaps a thread that is on a CPU, so the root is still live here even
        // though the lock is not held. The lock is released on purpose (rule 1, above), which is
        // exactly why this obligation cannot be a borrow and has to be a sentence.
        unsafe { crate::arch::mmu::switch_user_root(next_root) };

        // The cycle-counter grant, for the same instant: one shared register that must say what the
        // incoming thread was granted. No `unsafe`, because the register names no memory.
        #[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
        install_cycle_counter_grant(next_cycle_counter);

        // The register file, beside `switch_to` rather than in it: `switch_to` saves what a call
        // may destroy, and this moves what a thread owns. `crate::fp::hand_over` has the cases.
        //
        // SAFETY: `prev_fp` and `next_fp` name the `FpState`s of the outgoing and incoming threads,
        // pinned by the same argument the two lines below make for their contexts.
        unsafe { crate::fp::hand_over(prev_fp, next_fp) };

        // SAFETY: both pointers name live `Context`s owned by boxed `Thread`s in the map, and
        // interrupts are masked so nothing can reorder underneath us.
        //
        // This call does not return here. It returns *in another thread*, at the point where
        // that thread last called `switch_to`. We come back only when somebody switches to us.
        unsafe { switch_to(prev_slot, next_ctx) };

        // We are now the incoming thread, resuming. Reap whoever we switched away from, if it had
        // finished: it is off its stack now, and we are on the same core that set `to_reap`.
        finish_switch();
    }

    crate::arch::interrupts::restore(was_enabled);
}

/// **Convert a killed current thread into a corpse, and free the callers it can no longer answer.**
///
/// The body of [`schedule`]'s check, out of line because it is a teardown no IPC reaches. A killed
/// thread never reaches `depart`, so this is the one place its callers are freed before its
/// capability table goes. See notes/scheduler/switch.md.
///
/// Context: caller holds `IPC_TABLES`; `current` is killed, and running or parked.
#[cold]
#[inline(never)]
pub(super) fn finish_killed_current(sched: &mut IpcTables, current: ThreadId) {
    // A killed thread that has just parked itself (milestone 812: a process member its process's
    // end marked killed while it ran) is unlinked from what it waits on, since nothing will wake it.
    if sched
        .threads
        .get(current)
        .is_some_and(|t| t.handshake.state == State::Blocked)
    {
        finish_blocked_resident(sched, current);
    }
    if let Some(t) = sched.threads.get_mut(current) {
        t.handshake.state = State::Finished;
    }
    strand_callers_of(sched, current);
    process::on_killed(sched, current); // a supervised member reports its process's end
}

/// **Carry out a kill or a suspension order on the current thread**, `schedule`'s check, out of
/// line for [`finish_killed_current`]'s reason. The kill wins, so a doomed thread dies rather than
/// stopping. Caller holds `IPC_TABLES` and found one of the two flags set.
#[cold]
#[inline(never)]
pub(super) fn carry_out_orders(sched: &mut IpcTables, current: ThreadId) {
    let Some(t) = sched.threads.get(current) else {
        return;
    };
    let running = t.handshake.state == State::Running;
    if t.killed
        && !t.kill_deferred
        && (running || (t.handshake.state == State::Blocked && !t.handshake.wake_pending))
    {
        finish_killed_current(sched, current);
    } else if t.suspend_requested && running {
        suspension::park_suspended_current(sched, current);
    }
}

/// **The running thread came off its own run queue, which cannot legally happen.** [`schedule`]'s
/// guard, out of line because it is unreachable on every path this kernel is meant to take.
///
/// The pop precedes the requeue, so a legal schedule can never hand back `current`; if it ever
/// does, something queued a thread that was still running, and switching into it would restore
/// `t.context`, a pointer to a frame this very thread has already resumed and consumed: execution
/// time-travels to its previous switch-out point on a reused stack and spins there forever, off
/// every instrument. Debug builds fail loudly; the board build heals by keeping the thread running,
/// which is the only state that is still coherent.
#[cold]
#[inline(never)]
pub(super) fn heal_self_pop(sched: &mut IpcTables, current: ThreadId) {
    if cfg!(debug_assertions) {
        panic!("schedule() popped its own current thread from the run queue");
    }
    if let Some(t) = sched.threads.get_mut(current) {
        t.handshake.state = State::Running;
    }
}

/// Finish the switch-out of the thread this core just left: reap it if it finished, complete a
/// deferred wake, or clear its `on_cpu`.
///
/// The safe half of the two-part reaper, run on the incoming thread once the outgoing one is off
/// its stack. Called from `schedule()` and from `thread_entry`, the two ways a thread resumes,
/// both on the core that recorded `switched_from`. See DECISIONS §11 (per-CPU run queues) and
/// notes/scheduler/switch.md.
///
/// Context: takes `IPC_TABLES`; interrupts are masked on both callers' paths.
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(
    target_os = "none",
    unsafe(link_section = ".text.hot.sched.finish_switch")
)]
pub(crate) fn finish_switch() {
    let prev = cpu::current()
        .switched_from
        .swap(cpu::NO_TID, Ordering::Relaxed);
    if prev == cpu::NO_TID {
        return;
    }
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return;
    };
    let Some(t) = sched.threads.get_mut(prev) else {
        return;
    };
    // The predecessor's context is saved now (we are running, so switch_to completed). What that
    // makes legal is `thread_wake_handshake::Handshake::finish_switch`'s verdict: reap a Finished
    // predecessor, complete a wake that was deferred mid-switch-out, or simply clear `on_cpu` so
    // other cores may run it. The transition is the crate's (loom searches it; see
    // notes/interleaving.md); the reap and the queue push are ours.
    let verdict = t.handshake.finish_switch();
    match verdict {
        // Out of line and `#[cold]`: only a finished thread is reaped, which no IPC does, and
        // `script/fastpath-footprint` cannot exclude an inline arm's own setup from its closures.
        SwitchOutVerdict::Reap => reap_switched_out(guard, prev),
        SwitchOutVerdict::WakeCompleted => {
            trace::record(trace::Event::WakeCompleted, prev, 0);
            // A deferred wake was deferred precisely because the waker did NOT queue it, so the
            // token is still on the thread. IRQs are still masked on both callers' paths.
            let Some(token) = t.own_token.take() else {
                missing_token()
            };
            cpu::current().with_runq(|q| q.push_back(token));
        }
        SwitchOutVerdict::Cleared => {}
    }
}

/// **Reap `prev`, the thread this core just switched away from**, consuming the caller's `guard`.
///
/// Takes the stack and address space out and marks the thread
/// [`being_reaped`](crate::thread::Thread::being_reaped) under the lock, frees them with no lock
/// held, then removes the thread under the lock again, so everything it owned is gone before it
/// leaves its region. See notes/scheduler/switch.md.
///
/// # BUGS
/// - A corpse whose TCB is outside region R and whose root is inside it has a narrow window in
///   which a `DESTROY(R)` frees the root's page under the drop; see notes/scheduler/switch.md.
#[cold]
#[inline(never)]
pub(super) fn reap_switched_out(
    guard: crate::sync::IrqSafeGuard<'_, Option<IpcTables>>,
    prev: ThreadId,
) {
    let process = guard
        .as_ref()
        .and_then(|s| s.threads.get(prev))
        .and_then(|t| t.process);
    process::reap_corpse(guard, prev);
    if let Some(pid) = process {
        process::reap_corpses(pid); // members its end finished where they were blocked
    }
}

/// **Record that the running thread has started using the floating-point unit.**
///
/// The scheduler half of [`crate::fp::enable_for_current`], and the only setter of the flag.
/// Returns false when there is no scheduler or no current thread, which the caller must turn into
/// a fault: the flag is what stops the same instruction taking the same trap forever.
///
/// Context: takes `IPC_TABLES` from a trap handler; the trap came from a running thread, and the
/// kernel is built `softfloat`, so this core cannot already hold it. See notes/scheduler/switch.md.
pub fn mark_current_fp_live() -> bool {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return false;
    };
    let current = current_thread_id();
    let Some(pointer) = sched.threads.pointer(current) else {
        return false;
    };
    // The register file is in the TCB page beside the struct, so this goes through the page pointer
    // rather than through a `&mut Thread`; see `Threads::pointer`.
    //
    // SAFETY: a live thread's TCB page, held under IPC_TABLES.
    unsafe { (*crate::thread::fp_state_of(pointer)).set_live() };
    true
}

/// Whether the scheduler's tables are installed.
pub fn is_running() -> bool {
    IPC_TABLES.lock().is_some()
}
