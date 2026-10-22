//! **The switch**: adopting the boot context as thread 0, the queue tokens a switch moves,
//! `schedule` and the work `finish_switch` does on the far side, and the per-thread state
//! installed on the core about to run a thread.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// Adopt the context we are already running in as thread 0.
///
/// It has no stack of its own and no saved context. **The first switch *away* from it fills
/// that in**, which is why the boot thread needs no special case: a thread's context is written
/// by the act of leaving it.
pub fn init() {
    // **The floating-point unit is shut on this core before any thread exists**
    // (milestone 447 (a thread's vector registers are its own)).
    //
    // Here rather than in `arch::init`, and the reason is a real trap rather than taste. What the
    // invariant is *about* is threads: `crate::fp` marks a thread `live` when it takes the first-use
    // trap, and a core that started with the unit already open never produces one, so every thread
    // on it stays `live == false` and two of them quietly share a register file. This is the
    // function where threads begin to exist, so the property and the mechanism are in the same
    // place. `arch::init` looked like the obvious home and is not one: **RISC-V's boot hart never
    // calls it.** `main`'s RISC-V tour installs `stvec` with `arch::exceptions::init()` directly,
    // reaches `sched::init` and never passes through `arch::init` at all, and OpenSBI hands the
    // kernel a hart with `sstatus.FS` already set. That cost this milestone a red suite and is
    // exactly the shape of failure AGENTS.md's ladder is about: it worked on two architectures and
    // was invisible on the third.
    crate::arch::fp::init();

    // **The machine statistics page, before the first thread** (milestone 126, DECISIONS §225 (`free` sees the machine and your share)),
    // for `fp::init`'s reason: this is where threads begin, so it is where the counters that watch
    // them begin, on all three architectures through the one function each boot path calls.
    crate::machine_statistics::publish();

    let mut sched = IPC_TABLES.lock();

    // **Install the empty tables FIRST, then name the boot thread through them**, rather than
    // building a `Threads` as a local and moving it in afterwards. That ordering is a stack
    // measurement rather than a preference, and it is what lets [`MAX_THREADS`] be raised at all.
    //
    // `IpcTables` is two generational tables and nothing else that matters, and the thread table
    // is `MAX_THREADS` slots wide. Built as a local and then moved into the `Option`, an
    // unoptimised build carries the table twice on the boot stack, so **every slot added to
    // `MAX_THREADS` cost about 80 bytes of boot stack** rather than the ~40 the table itself is.
    // Measured 2026-08-27: at 128 slots this function's frame was 43,952 bytes, the deepest in
    // the kernel and the reason the suite's boot-stack high-water sat at 54,336 of 65,504 (82%);
    // a probe raise to 256 took the high-water to 64,640 (98%) and failed `stack::report_high_water`'s
    // own gate, in a run where nothing else about the boot had changed. Assigning
    // [`EMPTY_TABLES`], a `const`, gives the compiler a `.rodata` aggregate to copy from and
    // leaves the boot thread's insert to go straight into the installed table.
    *sched = Some(EMPTY_TABLES);
    let tables = sched
        .as_mut()
        .expect("the tables were just installed on this line");
    // The table names the boot thread at insert. The first name a fresh table mints is 0 by
    // construction (slot 0, generation 0), so "the boot thread is tid 0" survives, now as a
    // property of the table rather than a hardcoded key.
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

    // (The run queue and inbox used to have capacity reserved here, so a push from the timer IRQ
    // could never allocate. The queues are intrusive now: a push is two pointer writes and
    // *cannot* allocate, so there is nothing to reserve. §9's rule became structural.)

    // The idle thread. Its entire body is "wait for an interrupt, then let the scheduler look for
    // work." It is deliberately kept OUT of the ready queue (see cpu::PerCpu::idle): the scheduler picks it
    // only when nothing else is runnable, so it never steals a turn from real work.
    //
    // Built on its own TCB page, as `spawn_on` builds every other kernel thread (milestone 124 (a
    // thread is born where it lives: the spawn path's copies)), rather than as a value carried
    // there: the by-value `Thread::spawn` this used held three `Thread`s in one frame in an
    // unoptimised build, 4368 bytes once the capability table grew to 32 slots, over the 4096-byte
    // guard page (milestone 126 (the `procps` package), 2026-09-27, UTC).
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

/// The raw TCB pointer of a live thread, for queueing (milestone 14 phase A.2). Caller holds
/// `IPC_TABLES`.
///
/// The pointer's validity while queued is the queue discipline, stated once here: a thread on a
/// run queue or inbox is `Ready`, a thread on an rendezvous wait queue is `Blocked` (A.3), the
/// reaper frees only `Finished` threads, and a thread is never two of those at once. The `Box` in
/// the table pins the address (see `IpcTables::threads`), so a pointer taken here is good until
/// the thread is popped, however many queue hops (inbox to run queue) it makes in between.
/// The queue-able pointer to a live thread.
///
/// Returns [`core::ptr::NonNull`] rather than `*mut`, and that is not decoration: the pointer is derived from a
/// `&mut Thread` handed out by the thread table, so **non-nullness is a fact of construction rather
/// than a promise the caller keeps**. Saying so in the type removes null from the intrusive queue's
/// safety contract entirely, which is one of the two things CodeQL's `rust/access-invalid-pointer`
/// alerts were pointing at (milestone 45). What the type still cannot express is that the pointee
/// outlives its time on the queue; that is the caller's rule 2, and no type available here can carry
/// it for an intrusive structure.
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

/// **Take `tid`'s queue token off its own TCB, to put it on a queue** (milestone 139 (drive the
/// unsafe count down), round 10). Caller holds `IPC_TABLES`.
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
/// One cold function for every site rather than an `expect` at each, measured rather than assumed:
/// an `expect` sets up a message and a location inline, and seven of them were the larger part of
/// what the token added to `script/fastpath-footprint`'s closures.
#[cold]
#[inline(never)]
pub(super) fn missing_token() -> ! {
    panic!("a thread without its queue token: on a queue already, or its token was lost")
}

/// **Give a token a queue just handed back to the thread it names, and say which thread that is.**
/// Every pop and removal in this file ends here or on another queue, so the thread holds its token
/// again before anything decides what happens to it. Caller holds `IPC_TABLES`.
///
/// This is where the old sites' `(*waiter.as_ptr()).id` went: they read the id through the pointer
/// a queue returned, and so does this, once.
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

/// Install the incoming thread's cycle-counter grant on the core about to run it, immediately after
/// its address-space root (milestone 229, DECISIONS §139 option 4).
///
/// **Built only under `test` or `--features cycle_counter_grant`, and both the read (in `schedule`)
/// and this install are `#[cfg]`-gated at the switch site**, not carried through the shared switch
/// tuple. Milestone 139 threaded the grant through the tuple and called this unconditionally on every
/// arch, trusting the optimizer to fold the constant `false` away when no grant is built; milestone
/// 237 made the mechanism a feature and left the fold in place. Under the debug build the icount gate
/// measures, that fold does not happen: the const-`false` tuple element and the empty install stayed
/// in the shipping switch and cost `yield_switch` +35.6 and `ctx_switch` +36.0 ticks per switch on
/// aarch64 (milestone 300). This is the same leak milestone 299 found for the port grant, and the fix
/// is the same shape: `#[cfg]` the read and the install, keep the tuple at its pre-139 width. When the
/// feature is off `PMUSERENR_EL0` (and riscv64's `scounteren.CY`) keeps the closed value milestone 228
/// wrote at boot for the whole life of the kernel, and none of this code exists to touch it. Closing
/// what we claim is closed is right whether or not anyone can be granted an exception, which is why
/// 228's default write is NOT behind the feature and this is. `kernel/Cargo.toml`'s
/// `cycle_counter_grant` block carries the rest of the measurement.
#[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
pub(super) fn install_cycle_counter_grant(granted: bool) {
    crate::arch::timer::set_cycle_counter_grant(granted);
}

/// Install the incoming thread's x86 port grant into the core about to run it, at the last point
/// `IPC_TABLES` is held; milestone 315 (a port revoke that reaches every core) moved it there and
/// the call site says why. The lazy write
/// lives in `arch::segments`.
///
/// **`x86_64` only, and both the read (in `switch_operands`) and this install are `#[cfg]`-gated
/// at the switch site**, not carried through the shared switch tuple. An earlier version threaded
/// the value through the tuple the way the cycle-counter grant then did and trusted the optimizer
/// to fold a constant `None` away on the other two architectures; it did not (icount measured
/// `yield_switch` and `ctx_switch` up 13% on aarch64). The cycle-counter grant turned out to leak
/// the same way for the same reason (the fold does not happen in the debug build the icount gate
/// measures), and milestone 300 (decompose the icount baseline drift) gave it this exact treatment;
/// see `install_cycle_counter_grant`. A port grant is present in every x86 build, so keeping it off
/// the other two ISAs' switch path takes a `#[cfg]`, not a constant. See the x86-only rationale in
/// DECISIONS §152 (the port-range capability).
#[cfg(target_arch = "x86_64")]
pub(super) fn install_port_grant(grant: Option<(u16, u16)>) {
    crate::arch::segments::set_port_range_grant(grant);
}

/// Pick another thread and go there.
///
/// May be called from normal context (a voluntary `yield_now`) or from the tail of the timer
/// IRQ handler (a preemption). The two paths are identical from here down, which is a large
/// part of why this is only forty lines.
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(target_os = "none", unsafe(link_section = ".text.hot.sched.schedule"))]
pub fn schedule() {
    // **Not from an interrupt stack** (milestone 124). A switch parks the running `sp` in the
    // outgoing thread's `Context` and resumes it there, arbitrarily later; a per-core interrupt
    // stack cannot promise those bytes will still be the thread's, so a thread parked there would
    // resume on whatever the next interrupt on that core had written. The interrupt path defers its
    // switch to `preempt_if_needed`, which its dispatcher calls one frame outside the trampoline,
    // back on the interrupted thread's own stack.
    //
    // Debug-only because it costs a `sp` read and a scan of `MAX_CPUS` spans on the hottest path in
    // the kernel, and because `script/stack-depth-check` proves the same property statically in CI,
    // on both architectures, by showing no context switch is reachable from the interrupt-stack
    // entry point. This is the runtime half of that pair, for the edges a call graph cannot see.
    debug_assert!(
        !crate::interrupt_stack::contains(crate::arch::current_sp()),
        "schedule() called on an interrupt stack: the outgoing thread would be parked on memory \
         that belongs to a core (see kernel/src/interrupt_stack.rs)",
    );

    // Rule 2: no interrupts across the decision *or* the switch. Between "I chose a thread" and
    // "I am running it" there must be no window for the timer to choose again.
    //
    // The saved state is a local, on **this thread's stack**, which is exactly what makes it
    // correct: when someone eventually switches back to us, `switch_to` returns here, and this
    // frame (with the right `was_enabled` in it) is still sitting where we left it.
    let was_enabled = crate::arch::interrupts::disable();

    // The incoming thread's cycle-counter grant, carried out of the decision block the same way and
    // for the same reason (milestone 300, the fix milestone 299 gave the port grant just above).
    // Written inside the block under the lock, read at the install site after the lock drops; `false`
    // unless the block decides to switch. Built only when a grant can exist (`test` or
    // `--features cycle_counter_grant`), so every shipping build's `schedule()` gains nothing at all
    // and the switch tuple stays at its pre-139 width. See `install_cycle_counter_grant`.
    #[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
    let mut next_cycle_counter = false;

    // A labeled block, so every exit path leaves through the SAME point: the guard drops at the
    // block's end and interrupts are restored ONCE, AFTER it. The earlier version called
    // `interrupts::restore(was_enabled)` and `return` from *inside* this block, which re-enabled
    // interrupts while still holding `IPC_TABLES`: a one-instruction window in which a
    // timer could fire, re-enter `schedule()`, and try to take a lock we already held. It was
    // intermittent and it was real; see the lock-rank violation it produced.
    let switch = 'decide: {
        let mut guard = IPC_TABLES.lock();
        let Some(sched) = guard.as_mut() else {
            break 'decide None;
        };

        let current = current_thread_id();

        // **Forcible teardown, before anything else** (DECISIONS §16 amendment; §28 made it
        // load-bearing). A thread `DESTROY` marked killed must never run again. Convert it to a
        // `Finished` corpse here, at the top of the decision, so *every* path below reaps it: not
        // only the switch path, but the "nothing else to run, keep current" path a runaway alone on
        // its core takes. Before §28's scattering, a killed runaway shared a core with other work, so
        // a switch always happened and the old requeue-time check sufficed; once a runaway can be the
        // only thread on its core, that check was unreachable and the runaway spun forever.
        // The *test* stays here, on the hot path, because it is two loads and a compare. The
        // *body* is out of line (milestone 188 phase 3): converting a killed thread and freeing its
        // callers is a teardown, so its bytes have no business in the kernel's hottest function.
        //
        // **And a suspension order** (DECISIONS §300 (the paused state, a TCB verb, silent)), for a
        // current thread still `Running`: it parks before the pick, so the pick neither requeues it
        // nor keeps it. One test of the two flags here, and which order applies in the cold body.
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

        // Running, with on_cpu set until ITS successor's finish_switch, one switch from now.
        // **Did this thread just cross cores?** Recorded here rather than at any wake site, because
        // this is the one place every path to a CPU passes through, whatever moved the thread: a
        // rendezvous wake onto the waker's core, a work steal, a load-aware placement, or spawn.
        // See `thread::Thread::last_cpu` for why the placement counter could not answer this, and
        // for why this is behind `feature = "soak_test"`: it is the hottest line of the hottest
        // function, and shipping it everywhere put `ipc_fastpath` over milestone 132's bound.
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

        // Hand the outgoing thread to the incoming one to finish up AFTER the switch, when it is
        // provably off its stack: reap it if it Finished, clear its on_cpu (and complete a
        // deferred wake) otherwise. Not here, and not by another core: we are still running on
        // its stack this instant. `current` is the local (the outgoing tid); `set_current`
        // above already moved the per-CPU current to `next`. See finish_switch.
        //
        // **And count the switch, through the same block** (milestone 629 (the context-switch statistic stops costing the switch path)): `vmstat`'s `cs` is this
        // word, copied to the machine statistics page by the tick, so the switch pays one add on a
        // block it is already writing rather than a walk to the page. See `PerCpu::switches`.
        let here = cpu::current();
        here.switched_from.store(current, Ordering::Relaxed);
        here.switches.fetch_add(1, Ordering::Relaxed);

        // The incoming thread's cycle-counter grant (milestone 229, DECISIONS §139 option 4), read
        // here for the same reason `switch_operands` reads the root: this is the last point the
        // lock is held. A kernel thread's is `false`, like every user thread nobody granted it to.
        // Written into the `#[cfg]`-gated variable declared above the block, not the switch tuple,
        // so it costs the shipping build nothing at all (milestone 300; see
        // `install_cycle_counter_grant` for why the old tuple-and-fold did not fold in the debug
        // build the icount gate measures).
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
        // Install the incoming thread's address space FIRST. `TTBR0_EL1` is one register, shared
        // by everybody, and a thread that resumes at EL0 in the previous thread's low half is
        // running a stranger's code. (No-ops, including no TLB flush, when the root is already
        // right, which is every switch between two kernel threads.)
        //
        // SAFETY: `next_root` is `reserved_root()`, or the composed value of the `AddressSpace`
        // owned by thread `next`, which the block above popped off this core's run queue and marked
        // `Running` with `on_cpu` set before releasing `IPC_TABLES`. No other core can pick it up in that
        // state and nothing reaps a thread that is on a CPU, so the root is still live here even
        // though the lock is not held. The lock is released on purpose (rule 1, above), which is
        // exactly why this obligation cannot be a borrow and has to be a sentence.
        unsafe { crate::arch::mmu::switch_user_root(next_root) };

        // And the incoming thread's authority to read the cycle counter, which is the same kind of
        // fact about the same instant: one register, shared by everybody, that has to say what the
        // thread about to run was granted rather than what the last one was. Needs no `unsafe`,
        // because unlike `TTBR0_EL1` this register names no memory and points at nothing that can
        // be freed: getting it wrong opens or closes a counter, it does not hand a thread a
        // stranger's pages. Costs a compare when the value already matches, which is every switch
        // on a machine where nothing is granted. `#[cfg]`-gated, not folded (milestone 300): it
        // exists only in a build that can grant the counter, the same shape as the port grant just
        // below. See `arch::timer::set_cycle_counter_grant` and `install_cycle_counter_grant`.
        #[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
        install_cycle_counter_grant(next_cycle_counter);

        // And the register file the two threads are about to share a core over (milestone 447).
        // This is beside `switch_to` rather than inside it because the two save different
        // quantities for different reasons: `switch_to` saves what a *function call* may destroy,
        // and this moves what a *thread* owns. It runs here, as the outgoing thread, with the lock
        // released and interrupts masked, so the register file is already right when the switch
        // lands. `crate::fp::hand_over` has the four cases and the CVE that decided them.
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
/// The body of [`schedule`]'s forcible-teardown check, out of line (milestone 188 phase 3): the
/// predicate is two loads on the kernel's hottest path, the body is a teardown that no IPC reaches.
///
/// A thread `DESTROY` marked killed must never run again, and this runs at the top of the decision
/// so *every* path below reaps it: not only the switch path, but the "nothing else to run, keep
/// current" path a runaway alone on its core takes. Before §28's scattering a killed runaway shared
/// a core with other work, so a switch always happened and the old requeue-time check sufficed;
/// once a runaway can be the only thread on its core, that check was unreachable and the runaway
/// spun forever.
///
/// A killed thread never reaches `depart`, so this is the one place its callers can be freed before
/// its capability table goes (milestone 254).
///
/// Caller holds `IPC_TABLES` and has already checked that `current` is killed and `Running`.
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
/// guard against boot 8's downstream catastrophe, out of line (milestone 188 phase 3) because it is
/// unreachable on every path this kernel is meant to take.
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

/// Reap the thread this core just switched away from, if it had finished.
///
/// The safe half of the two-part reaper. `schedule()` records a finished outgoing thread in this
/// core's `to_reap` *before* the switch; this runs on the incoming thread *after* the switch, when
/// the outgoing thread is provably off its stack (its registers are saved and we are on a
/// different stack). Dropping the `Thread` unmaps its stack and frees its address space, which is
/// exactly why it must not happen while any core still stands on it.
///
/// Called from two places, because a thread can resume two ways: from `schedule()` (an existing
/// thread returning from `switch_to`) and from `thread_entry` (a brand-new thread, which never
/// passes through `schedule()`'s post-switch point). Both run on this core, so both see this core's
/// `to_reap`. See DECISIONS §11 and thread.rs.
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
        // Out of line, and `#[cold]` says why: a switch-out reaps only when the outgoing thread
        // FINISHED, which no IPC does. Left inline its bytes sat in `finish_switch`, which is on
        // both IPC closures, and `script/fastpath-footprint` could only exclude the *callees* it
        // reaches (`KernelStack`, `AddressSpace`, `drop_in_place`) and not the arm's own setup.
        // Milestone 188 phase 3: the classification now lives in the code rather than in a regex.
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

/// **Reap the thread this core just switched away from.** The `Reap` arm of [`finish_switch`],
/// out of line because it runs when a thread has ended and never on an IPC.
///
/// **Two short critical sections with the expensive part between them.**
///
/// 1. Under the caller's `IPC_TABLES` guard: take the thread's kernel stack and its address space
///    out, and mark it [`being_reaped`](crate::thread::Thread::being_reaped). It stays in the
///    table, `Finished`, so its name still resolves and region teardown still refuses it.
/// 2. With no lock held: free the stack, then the address space. The stack is six page unmaps,
///    each discharging its TLB obligation; on riscv64 every one is an SBI remote fence that
///    interrupts every other hart and waits for them, and on x86_64 an NMI shootdown round; under
///    `IPC_TABLES` every other core would wait behind it
///    (notes/job-mix/null-syscall-under-load.md). The address space could never be dropped under
///    the lock: its teardown is `memory_region::destroy` (milestone 14 (kernel objects from
///    untyped) phase B.4), whose §13 revocation sweep takes `IPC_TABLES` itself.
/// 3. Under `IPC_TABLES` again: remove the thread. Only now does it stop occupying its region.
///
/// **Everything the thread owned is gone before step 3, and that order is the point.** Region
/// teardown (`reclaim_region`) relies on a resident thread's space being gone before the thread is
/// (its `being_reaped` refusal holds the region until step 3), and an owner whose `DESTROY` succeeds
/// may reuse the memory at once. The rest of the `Thread` (its quota token, its capability table)
/// drops under the lock in step 3.
///
/// Nothing else removes a `Finished` thread between steps 1 and 3: the only other remover is region
/// teardown, and `region_reap_verdict` refuses a thread that is being reaped.
///
/// **Since §249 the space is taken out of the address-space registry in step 2**, by the name in the
/// thread's copy, and the region sweep may have taken it already (this thread is a corpse off its
/// stack); `Table::remove` hands it to one of the two.
///
/// # BUGS
///
/// - **A corpse whose TCB is outside region R and whose root is inside it still has a narrow
///   window.** If this reaper takes the space in step 2 and a `DESTROY(R)` on another core runs
///   wholly before the drop finishes, the sweep finds no entry, R comes back, and the drop's
///   `revoke::forget_root` can land on a root that page has been given to since. The sweep taking
///   the space first is the common order and is safe; this is the other order. Milestone 765 (a
///   destroyed region cannot free the root a running thread walks) is the natural place to count a
///   space that is still being dropped.
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

/// **Record that the running thread has started using the floating-point unit** (milestone 447).
///
/// The scheduler half of [`crate::fp::enable_for_current`], and the only thing that ever sets the
/// flag. Returns false when there is no scheduler or no current thread, which is a trap the caller
/// must turn into a fault rather than return from: the flag is what stops the same instruction
/// taking the same trap forever.
///
/// **Taking `IPC_TABLES` inside a trap handler is ordinary here**, not a liberty. Every syscall
/// does it from the same place, through `syscall::dispatch`, and the argument is the same: this
/// trap came from a thread that was *running*, so this core cannot already hold the lock. What
/// would break that is kernel code executing an FP instruction while holding it, and the kernel is
/// built `softfloat` and executes none.
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

pub fn is_running() -> bool {
    IPC_TABLES.lock().is_some()
}
