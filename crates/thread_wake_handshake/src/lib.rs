//! The scheduler's block/wake handshake: the per-thread state machine that decides when a blocked
//! thread may become runnable. The kernel's `Thread` embeds a [`Handshake`] and calls these
//! transitions, so loom explores on the host the code the kernel runs.
//!
//! Every field is written under `IPC_TABLES`. The races live in the window between a thread marking
//! itself `Blocked` and its core's `switch_to` saving its context, and each is a rule here: wake
//! before switch-out ([`Handshake::wake_pending`]), the undelivered wake ([`Handshake::ipc_served`],
//! [`Handshake::ipc_aborted`]), and a steal racing a switch-out ([`Handshake::switch_in`]'s
//! tripwire). The queues and the out-of-protocol writes of `state` stay kernel-side. See
//! notes/scheduler/wake-handshake.md.

//! # BUGS
//!
//! - Nothing makes a new block path use [`park`](Handshake::park); a site writing `Blocked` by
//!   hand through the public fields skips the gate.
//! - [`park`](Handshake::park) leaves a stale [`ipc_aborted`](Handshake::ipc_aborted) set, so a
//!   thread that blocks again without calling `take_aborted` parks with the gate half open. Every
//!   current caller takes it; nothing enforces that.
//! - Loom checks the protocol under the kernel's locking discipline, not the discipline itself
//!   (notes/interleaving.md). Liveness is out of scope.
//!
//! Name: ratified 2026-08-23 (calef), renamed from `wake_handshake`. Refused `block_wake` (two
//! verbs), `wake_gate` (one rule of three), `sched_state` and `run_state` (too generic), `parking`
//! (std's `park` means something narrower).

#![cfg_attr(not(test), no_std)]

/// Where a thread is in its life. The kernel re-exports this as `thread::State`;
/// [`Handshake::try_wake`] and [`Handshake::finish_switch`] branch on it.
///
/// Only `Ready`, `Running` and `Blocked` take part in the handshake. The kernel enters `Embryo`,
/// `Finished` and `Dead` (retype, exit, fault), and the handshake only reads them: a wake ignores
/// them, and a finish-switch hands a `Finished` predecessor back to the kernel to reap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    /// Retyped but not yet started (milestone 19c.3): in no queue, never picked, `START` makes it
    /// `Ready`.
    Embryo,
    /// On exactly one run queue or migration inbox, waiting for a CPU.
    Ready,
    /// On a CPU. See [`Handshake::on_cpu`] for the tail end, where a thread is no longer `Running`
    /// but still standing on its core's stack.
    Running,
    /// Waiting for an IPC rendezvous that has not happened yet; on at most one endpoint wait
    /// queue. Only [`Handshake::try_wake`] leaves this state.
    Blocked,
    /// Exited; reaped by its successor's [`Handshake::finish_switch`] the moment it is off its
    /// stack.
    Finished,
    /// A supervised corpse (DECISIONS §26): never runs again, never auto-reaped, persists for its
    /// supervisor's postmortem until revocation.
    Dead,
}

/// What a wake attempt decided. The caller (the kernel's `wake` or `wake_load_aware`, holding
/// `IPC_TABLES`) acts on it; only [`Queue`](WakeVerdict::Queue) comes with an obligation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeVerdict {
    /// The thread is not `Blocked`; the wake is a no-op. (A second waker lost the race, or the
    /// target is an embryo or a corpse.)
    NotBlocked,
    /// The undelivered-wake gate fired: the thread waits on an endpoint and this wake delivered
    /// nothing. The thread stays parked and linked on its endpoint's wait queue, and the kernel
    /// records `refuse:tid` on the event ring. See notes/scheduler/undelivered-wake-gate.md.
    Refused,
    /// The thread is still standing on its CPU (its saved context is stale), so the wake was
    /// parked in [`Handshake::wake_pending`] instead of queueing a thread another core could
    /// switch into. Its own core completes it in [`Handshake::finish_switch`]. Nothing to queue.
    Deferred,
    /// The thread is `Ready` and off every queue: the caller must queue it now, on whatever
    /// core its policy picks (the waker's core for a rendezvous, the least-loaded for a device
    /// IRQ).
    Queue,
}

/// What a finished switch found in the predecessor. Returned by [`Handshake::finish_switch`],
/// which runs on the incoming thread once the predecessor is provably off its stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchOutVerdict {
    /// The predecessor `Finished`: the caller reaps it (drops the `Thread`, freeing stack and
    /// address space). The handshake is left untouched because the thread is about to not exist.
    Reap,
    /// A wake raced the switch-out and was parked ([`WakeVerdict::Deferred`]). The context is
    /// saved now, so the wake completed: the thread is `Ready` and the caller must queue it on
    /// this core, without load-aware placement (notes/scheduler/wake-handshake.md).
    WakeCompleted,
    /// The ordinary case: the predecessor's context is saved and other cores may now run it.
    /// Nothing to do.
    Cleared,
}

/// One thread's block/wake handshake: its state, flags and transitions. `W` is the payload naming
/// what the thread waits on; the handshake only asks whether one is present. Every method is
/// called with `IPC_TABLES` held, so the struct has no interior mutability and no atomics. See
/// notes/scheduler/wake-handshake.md.
///
/// # Examples
///
/// One rendezvous, as the kernel spells it, with the waker racing the receiver's switch-out:
///
/// ```
/// use thread_wake_handshake::{Handshake, RunState, SwitchOutVerdict, WakeVerdict};
///
/// // The receiver, running, parks itself in ipc_receive (under IPC_TABLES):
/// let mut hs: Handshake<(u64, char)> = Handshake::on_cpu_now();
/// hs.park((0, 'r')); // Blocked, waiting on endpoint 0 as a receiver, nothing delivered yet
///
/// // A spurious wake, nothing delivered: refused, the receiver stays parked (boot 8's gate).
/// assert_eq!(hs.try_wake(), WakeVerdict::Refused);
/// assert_eq!(hs.state, RunState::Blocked);
///
/// // A sender stages the mailbox and wakes, but the receiver's core has not switched away yet
/// // (on_cpu is still set): the wake is deferred, not queued.
/// hs.serve();
/// assert_eq!(hs.try_wake(), WakeVerdict::Deferred);
///
/// // The receiver's core finishes the switch: the context is saved, the deferred wake completes,
/// // and the caller queues the thread.
/// assert_eq!(hs.finish_switch(), SwitchOutVerdict::WakeCompleted);
/// assert_eq!(hs.state, RunState::Ready);
///
/// // Some core pops it and switches in; the receive tail's tripwire holds.
/// hs.switch_in();
/// assert!(hs.is_delivered());
/// ```
#[derive(Debug)]
pub struct Handshake<W> {
    /// See [`RunState`]. Out-of-protocol transitions (spawn, exit, kill, `START`) write this
    /// directly by design; the module's BUGS section names the cost.
    pub state: RunState,

    /// Still standing on a CPU, so its saved context is stale. Set by [`switch_in`](Self::switch_in);
    /// cleared by that core's successor in [`finish_switch`](Self::finish_switch), after the switch
    /// away has saved this thread's context.
    pub on_cpu: bool,

    /// A wake arrived while this thread was still switching out. Parked here by
    /// [`try_wake`](Self::try_wake), completed by [`finish_switch`](Self::finish_switch) on the
    /// thread's own core, when the context is provably saved.
    pub wake_pending: bool,

    /// What this thread is blocked on, or `None` when not waiting. Written by [`park`](Self::park)
    /// in the same statement as `state = Blocked`; cleared where a wake makes the thread `Ready`.
    /// A `Blocked` beside a `None` is a state byte no block path wrote.
    pub wait_on: Option<W>,

    /// The counterparty completed the pending rendezvous: a message staged, a signal counted, a
    /// reply filled. Set by [`serve`](Self::serve) in the critical section that delivered, before
    /// the wake; cleared by [`park`](Self::park). One half of the undelivered-wake gate.
    pub ipc_served: bool,

    /// The blocking IPC was aborted (revocation tore the endpoint down, or it was stale at the
    /// call). Set by [`abort`](Self::abort); read and cleared by the syscall layer through
    /// [`take_aborted`](Self::take_aborted). The other half of the gate. Not cleared by `park`;
    /// see BUGS.
    pub ipc_aborted: bool,
}

impl<W> Handshake<W> {
    /// A thread adopted mid-run (the boot thread, a secondary core's idle): `Running`, standing on
    /// its CPU right now.
    #[must_use]
    pub const fn on_cpu_now() -> Self {
        Self {
            state: RunState::Running,
            on_cpu: true,
            wake_pending: false,
            wait_on: None,
            ipc_served: false,
            ipc_aborted: false,
        }
    }

    /// A freshly spawned thread: `Ready`, on no CPU, about to be queued by its spawner.
    #[must_use]
    pub const fn ready() -> Self {
        Self {
            state: RunState::Ready,
            on_cpu: false,
            wake_pending: false,
            wait_on: None,
            ipc_served: false,
            ipc_aborted: false,
        }
    }

    /// A retyped, unstarted TCB (milestone 19c.3): `Embryo`, inert to every transition here until
    /// the kernel's `START` makes it `Ready`.
    #[must_use]
    pub const fn embryo() -> Self {
        Self {
            state: RunState::Embryo,
            on_cpu: false,
            wake_pending: false,
            wait_on: None,
            ipc_served: false,
            ipc_aborted: false,
        }
    }

    /// Park the running thread in a blocking IPC (send, receive, call and their
    /// capability-carrying twins). `Blocked`, waiting on `wait`, with the gate armed: only a
    /// counterparty that delivers, or an abort, may complete this wait.
    ///
    /// The caller has already linked the thread on the endpoint's wait queue (or, for a `CALL`
    /// caller, deliberately on none) and stays under `IPC_TABLES` until both are recorded, so a
    /// timer-driven `schedule()` in the gap sees a consistent pair.
    pub fn park(&mut self, wait: W) {
        debug_assert!(
            self.state == RunState::Running,
            "a thread parked itself while not running"
        );
        self.state = RunState::Blocked;
        self.wait_on = Some(wait);
        self.ipc_served = false; // parked: only a delivering counterparty may wake us
    }

    /// Record a delivery, in the same `IPC_TABLES` critical section that staged it (a mailbox
    /// written, a signal counted, a reply filled, a death message collected). It lets the next
    /// [`try_wake`](Self::try_wake) through the gate. See notes/scheduler/wake-handshake.md.
    pub fn serve(&mut self) {
        self.ipc_served = true;
    }

    /// Abort the wait (the endpoint is stale or was revoked under the sleeper). Passes the
    /// gate like a delivery, because the woken thread does get something to return: the error the
    /// syscall layer reads through [`take_aborted`](Self::take_aborted).
    pub fn abort(&mut self) {
        self.ipc_aborted = true;
    }

    /// Whether a resume has something to return: the receive tail's tripwire (`debug_assert!` in the
    /// kernel), and the model's central assertion.
    #[must_use]
    pub fn is_delivered(&self) -> bool {
        self.ipc_served || self.ipc_aborted
    }

    /// Read and clear the abort flag, exactly `core::mem::take`: the syscall layer calls this
    /// right after a blocking primitive returns, and `true` means "hand the caller an error, not
    /// the mailbox".
    pub fn take_aborted(&mut self) -> bool {
        core::mem::take(&mut self.ipc_aborted)
    }

    /// Try to make a blocked thread runnable. Checks, in order: not blocked, the undelivered-wake
    /// gate, the switch-out deferral, then `Ready`. Returns a [`WakeVerdict`]; only
    /// [`Queue`](WakeVerdict::Queue) hands back a thread to place.
    ///
    /// On `Queue` the thread leaves its wait (`wait_on` cleared) but keeps `ipc_served` until its
    /// next [`park`](Self::park): the resumed IPC tail still needs to see what was delivered.
    pub fn try_wake(&mut self) -> WakeVerdict {
        if self.state != RunState::Blocked {
            return WakeVerdict::NotBlocked;
        }
        // A wake that delivered nothing has not dequeued the thread from its endpoint, and its
        // parked IPC has nothing to return: refuse it and leave the thread parked.
        if self.wait_on.is_some() && !self.is_delivered() {
            return WakeVerdict::Refused;
        }

        // Still on a CPU means the saved context is stale; queueing now would let another core
        // switch into it. finish_switch completes the parked wake once the context is saved.
        if self.on_cpu {
            self.wake_pending = true;
            return WakeVerdict::Deferred;
        }
        self.state = RunState::Ready;
        self.wait_on = None;
        WakeVerdict::Queue
    }

    /// A core is switching this thread in (`schedule()`'s chosen `next`): `Running`, and standing
    /// on a CPU until a successor's [`finish_switch`](Self::finish_switch) says otherwise.
    ///
    /// The `debug_assert` on `on_cpu` is the steal-vs-switch-out tripwire; release builds proceed.
    /// See notes/scheduler/wake-handshake.md.
    pub fn switch_in(&mut self) {
        debug_assert!(
            !self.on_cpu,
            "switching into a thread that is still on a cpu"
        );
        // Running is the idle thread: it is never marked Ready when a core leaves it.
        debug_assert!(
            matches!(self.state, RunState::Ready | RunState::Running),
            "switching into a thread that was neither Ready nor a parked idle"
        );
        self.state = RunState::Running;
        self.on_cpu = true;
    }

    /// The timer took this thread off its CPU with work left (`schedule()`'s requeue): back to
    /// `Ready`, and the caller pushes it on this core's run queue. `on_cpu` stays set until the
    /// successor's [`finish_switch`](Self::finish_switch), which is the one legal overlap of queue
    /// and CPU (notes/scheduler/wake-handshake.md).
    pub fn preempt(&mut self) {
        debug_assert!(
            self.state == RunState::Running,
            "preempting a thread that was not running"
        );
        self.state = RunState::Ready;
    }

    /// The switch away from this thread has completed; we are its successor, on its core, and
    /// its context is provably saved (we are running, so `switch_to` finished writing it). Reap it
    /// if it finished; otherwise clear [`on_cpu`](Self::on_cpu) and complete a deferred wake if
    /// one was parked. See [`SwitchOutVerdict`] for the caller's obligations.
    pub fn finish_switch(&mut self) -> SwitchOutVerdict {
        if self.state == RunState::Finished {
            // Untouched: the caller drops the whole Thread.
            return SwitchOutVerdict::Reap;
        }
        self.on_cpu = false;
        if self.wake_pending {
            self.wake_pending = false;
            self.state = RunState::Ready;
            self.wait_on = None;
            return SwitchOutVerdict::WakeCompleted;
        }
        SwitchOutVerdict::Cleared
    }
}

// --- The ordinary host tests -----------------------------------------------------------------
//
// Single-threaded: the transition table, edge by edge. The loom module below checks interleavings.
#[cfg(all(test, not(loom)))]
mod tests {
    use super::*;

    #[test]
    fn the_constructors_state_their_names() {
        let adopted: Handshake<u8> = Handshake::on_cpu_now();
        assert_eq!(adopted.state, RunState::Running);
        assert!(adopted.on_cpu);

        let spawned: Handshake<u8> = Handshake::ready();
        assert_eq!(spawned.state, RunState::Ready);
        assert!(!spawned.on_cpu);

        let retyped: Handshake<u8> = Handshake::embryo();
        assert_eq!(retyped.state, RunState::Embryo);
        assert!(!retyped.on_cpu);
    }

    #[test]
    fn an_undelivered_wake_is_refused_and_the_thread_stays_parked() {
        let mut hs: Handshake<u8> = Handshake::on_cpu_now();
        hs.park(3);
        assert_eq!(hs.try_wake(), WakeVerdict::Refused);
        assert_eq!(hs.state, RunState::Blocked, "a refused wake unparked");
        assert_eq!(hs.wait_on, Some(3), "a refused wake cleared the wait");
        assert!(
            !hs.wake_pending,
            "a refused wake left a pending wake behind"
        );
    }

    #[test]
    fn a_served_wake_defers_while_on_cpu_and_queues_after() {
        let mut hs: Handshake<u8> = Handshake::on_cpu_now();
        hs.park(3);
        hs.serve();
        assert_eq!(hs.try_wake(), WakeVerdict::Deferred);
        assert_eq!(hs.state, RunState::Blocked);
        assert_eq!(hs.finish_switch(), SwitchOutVerdict::WakeCompleted);
        assert_eq!(hs.state, RunState::Ready);
        assert_eq!(hs.wait_on, None);
        assert!(!hs.wake_pending);

        // The same wake landing after the switch-out queues directly.
        let mut hs: Handshake<u8> = Handshake::on_cpu_now();
        hs.park(3);
        assert_eq!(hs.finish_switch(), SwitchOutVerdict::Cleared);
        hs.serve();
        assert_eq!(hs.try_wake(), WakeVerdict::Queue);
        assert_eq!(hs.state, RunState::Ready);
    }

    #[test]
    fn an_abort_passes_the_gate_and_is_taken_exactly_once() {
        let mut hs: Handshake<u8> = Handshake::on_cpu_now();
        hs.park(3);
        hs.abort();
        assert_eq!(hs.finish_switch(), SwitchOutVerdict::Cleared);
        assert_eq!(hs.try_wake(), WakeVerdict::Queue);
        assert!(hs.is_delivered());
        assert!(hs.take_aborted());
        assert!(!hs.take_aborted(), "the abort flag survived being taken");
    }

    #[test]
    fn a_wake_on_a_thread_that_is_not_blocked_is_a_no_op() {
        let mut ready: Handshake<u8> = Handshake::ready();
        assert_eq!(ready.try_wake(), WakeVerdict::NotBlocked);
        let mut embryo: Handshake<u8> = Handshake::embryo();
        assert_eq!(embryo.try_wake(), WakeVerdict::NotBlocked);
        let mut corpse: Handshake<u8> = Handshake::ready();
        corpse.state = RunState::Dead;
        assert_eq!(corpse.try_wake(), WakeVerdict::NotBlocked);
    }

    #[test]
    fn a_preempted_thread_keeps_standing_on_its_cpu_until_the_switch_finishes() {
        let mut hs: Handshake<u8> = Handshake::on_cpu_now();
        hs.preempt();
        assert_eq!(hs.state, RunState::Ready);
        assert!(
            hs.on_cpu,
            "preemption cleared on_cpu before the context was saved"
        );
        assert_eq!(hs.finish_switch(), SwitchOutVerdict::Cleared);
        assert!(!hs.on_cpu);
        hs.switch_in();
        assert_eq!(hs.state, RunState::Running);
        assert!(hs.on_cpu);
    }

    #[test]
    fn a_finished_predecessor_is_handed_back_for_reaping_untouched() {
        let mut hs: Handshake<u8> = Handshake::on_cpu_now();
        hs.state = RunState::Finished; // depart's kernel-side write
        assert_eq!(hs.finish_switch(), SwitchOutVerdict::Reap);
        assert!(
            hs.on_cpu,
            "finish_switch dressed a corpse instead of reaping it"
        );
    }

    #[test]
    fn a_second_wake_after_a_deferral_is_idempotent() {
        // Two wakers in one window (a rendezvous plus a revocation abort, say): the second finds
        // the first's parked wake and adds nothing, and finish_switch completes exactly one.
        let mut hs: Handshake<u8> = Handshake::on_cpu_now();
        hs.park(3);
        hs.serve();
        assert_eq!(hs.try_wake(), WakeVerdict::Deferred);
        hs.abort();
        assert_eq!(hs.try_wake(), WakeVerdict::Deferred);
        assert_eq!(hs.finish_switch(), SwitchOutVerdict::WakeCompleted);
        assert_eq!(hs.try_wake(), WakeVerdict::NotBlocked);
    }

    #[test]
    fn park_rearms_the_gate_a_previous_delivery_opened() {
        let mut hs: Handshake<u8> = Handshake::on_cpu_now();
        hs.park(1);
        hs.serve();
        assert_eq!(hs.finish_switch(), SwitchOutVerdict::Cleared);
        assert_eq!(hs.try_wake(), WakeVerdict::Queue);
        hs.switch_in();
        // Parking again clears the stale ipc_served, or every later spurious wake would pass.
        hs.park(2);
        assert_eq!(hs.try_wake(), WakeVerdict::Refused);
    }
}

// --- The loom model --------------------------------------------------------------------------
#[cfg(all(test, loom))]
mod interleavings {
    //! The block/wake handshake's critical sections, interleaved every way loom can order them.
    //!
    //! Run with `script/interleaving-check`. `IPC_TABLES` is a `loom::sync::Mutex`, and the saved
    //! context is a `loom::cell::UnsafeCell` written outside it, where `switch_to` writes the real
    //! one. Every harness runs to exhaustion, and each reconstruction of an old race is
    //! `#[should_panic]`. The model assumes the kernel's locking discipline rather than checking
    //! it. See notes/scheduler/wake-handshake.md and notes/interleaving.md.

    use loom::cell::UnsafeCell;
    use loom::sync::{Arc, Mutex};
    use loom::thread;

    use super::*;

    /// Non-vacuity, `work_steal_slot`'s pattern: a real (non-loom) atomic that accumulates across
    /// every execution of the search, asserted afterwards, so a bound that quietly empties the
    /// interesting branch fails loudly. See notes/verification.md's non-vacuity section.
    struct Reached(core::sync::atomic::AtomicBool);

    impl Reached {
        const fn new() -> Self {
            Self(core::sync::atomic::AtomicBool::new(false))
        }
        fn mark(&self) {
            self.0.store(true, core::sync::atomic::Ordering::Relaxed);
        }
        fn assert(&self, what: &str) {
            assert!(
                self.0.load(core::sync::atomic::Ordering::Relaxed),
                "vacuous harness: no execution ever reached {what}, so nothing was checked"
            );
        }
    }

    /// The world one modelled thread lives in: its handshake and its queue membership, under the
    /// one lock, plus its saved context outside it. `queued` counts queue events rather than
    /// holding a flag so that conservation ("woken exactly once") is an assertion, not a hope.
    struct Machine {
        sched: Mutex<Core>,
        /// The thread's saved context: written by its core's `switch_to` after `IPC_TABLES` is
        /// released, read by whichever core switches it back in. The loom `UnsafeCell` is the
        /// instrument: any two accesses not ordered by the model's own edges fail the run.
        ctx: UnsafeCell<u32>,
    }

    struct Core {
        hs: Handshake<u8>,
        /// How many times the thread has been handed to a run queue. Exactly one wake may queue.
        queued: u32,
    }

    impl Machine {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                sched: Mutex::new(Core {
                    hs: Handshake::on_cpu_now(),
                    queued: 0,
                }),
                ctx: UnsafeCell::new(0),
            })
        }
    }

    const SAVED: u32 = 7;

    /// A waker races the receiver's switch-out, and the resumer always sees a saved context.
    ///
    /// The wake-before-switch-out race (notes/intrusive-queues.md), as the kernel runs it: the
    /// receiver parks under `IPC_TABLES`, releases, saves its context (the write outside the lock),
    /// and its successor's `finish_switch` clears `on_cpu`; a sender on another core delivers and
    /// wakes somewhere in between. Whichever side ends up queueing the thread, the switch-in that
    /// follows must read the context the victim wrote, and exactly one side may queue.
    #[test]
    fn a_wake_racing_a_switch_out_resumes_on_a_saved_context() {
        // Both windows must be explored, or the deferral is being checked against a search that
        // never raced it.
        static DEFERRED: Reached = Reached::new();
        static QUEUED_DIRECT: Reached = Reached::new();

        loom::model(|| {
            let m = Machine::new();

            // The park is sequenced before both cores, as endpoint queue linking sequences the
            // real waker behind the real park.
            m.sched.lock().unwrap().hs.park(3);

            // The receiver's core: save the context, then finish the switch.
            let victim = {
                let m = Arc::clone(&m);
                thread::spawn(move || {
                    m.ctx.with_mut(|c| {
                        // SAFETY: the model's write side of the property under test: nothing may
                        // read this cell until an edge orders it after this write, and loom fails
                        // the run if anything does.
                        unsafe { *c = SAVED }
                    });
                    let completed = {
                        let mut core = m.sched.lock().unwrap();
                        match core.hs.finish_switch() {
                            SwitchOutVerdict::WakeCompleted => {
                                core.queued += 1;
                                true
                            }
                            SwitchOutVerdict::Cleared => false,
                            SwitchOutVerdict::Reap => unreachable!("nothing finished"),
                        }
                    };
                    if completed {
                        DEFERRED.mark();
                        resume(&m);
                    }
                })
            };

            // The waker's core: deliver and wake in one critical section.
            let waker = {
                let m = Arc::clone(&m);
                thread::spawn(move || {
                    let queued = {
                        let mut core = m.sched.lock().unwrap();
                        core.hs.serve();
                        match core.hs.try_wake() {
                            WakeVerdict::Queue => {
                                core.queued += 1;
                                true
                            }
                            WakeVerdict::Deferred => false,
                            v => unreachable!("a served wake of a parked thread decided {v:?}"),
                        }
                    };
                    if queued {
                        QUEUED_DIRECT.mark();
                        resume(&m);
                    }
                })
            };

            victim.join().unwrap();
            waker.join().unwrap();

            let core = m.sched.lock().unwrap();
            assert_eq!(
                core.queued, 1,
                "one wake queued the thread {} times",
                core.queued
            );
            assert_eq!(core.hs.state, RunState::Running, "the thread never resumed");
        });

        DEFERRED.assert("an interleaving where the wake lands mid-switch-out and is deferred");
        QUEUED_DIRECT.assert("an interleaving where the wake lands after the switch-out");
    }

    /// Switch the thread in and read its saved context: the shared tail of every harness. The
    /// asserts restate `switch_in`'s tripwires because `script/interleaving-check` compiles
    /// `--release`, where a `debug_assert` would leave the reconstructions nothing to fire.
    fn resume(m: &Machine) {
        {
            let mut core = m.sched.lock().unwrap();
            assert!(core.hs.is_delivered(), "resumed with nothing delivered");
            assert!(
                !core.hs.on_cpu,
                "switching into a thread that is still on a cpu"
            );
            core.hs.switch_in();
        }
        m.ctx.with(|c| {
            // SAFETY: the read side of the property: a resume is ordered after the victim's
            // context save by the handshake (defer until finish_switch) plus the lock edges, and
            // loom fails the run if that ordering is ever absent.
            assert_eq!(unsafe { *c }, SAVED, "resumed on an unsaved context");
        });
    }

    /// A wake that ignores `on_cpu` lets two cores share one thread. The wake respects the gate
    /// but queues a still-switching thread directly, and loom must find a resume into a thread
    /// whose core has not saved its context. `resume`'s still-on-a-cpu assert names it; the `ctx`
    /// cell race behind it is the second net.
    #[test]
    #[should_panic(expected = "still on a cpu")]
    fn a_wake_that_ignores_on_cpu_switches_into_an_unsaved_context() {
        loom::model(|| {
            let m = Machine::new();
            m.sched.lock().unwrap().hs.park(3);

            let victim = {
                let m = Arc::clone(&m);
                thread::spawn(move || {
                    m.ctx.with_mut(|c| {
                        // SAFETY: deliberately unsound under the broken wake below, which is the
                        // point: with the deferral gone, nothing orders the resumer's read after
                        // this write, and the harness requires the model to fail.
                        unsafe { *c = SAVED }
                    });
                    let mut core = m.sched.lock().unwrap();
                    let _ = core.hs.finish_switch();
                })
            };

            let waker = {
                let m = Arc::clone(&m);
                thread::spawn(move || {
                    {
                        let mut core = m.sched.lock().unwrap();
                        core.hs.serve();
                        // The historical wake: gate respected, deferral skipped.
                        assert_eq!(core.hs.state, RunState::Blocked);
                        core.hs.state = RunState::Ready;
                        core.hs.wait_on = None;
                        core.queued += 1;
                    }
                    resume(&m);
                })
            };

            victim.join().unwrap();
            waker.join().unwrap();
        });
    }

    /// A stolen thread is never switched in before its context is saved.
    ///
    /// A preempted thread sits `Ready` in its core's run queue with `on_cpu` still set. Only the
    /// owning core pops that queue, after `finish_switch`, so `serve_steal_request` hands a thief
    /// a saved context. The model encodes that as program order on the victim core and checks
    /// that the inbox handoff (its own lock in the kernel, a flag here) carries the ordering.
    #[test]
    fn a_stolen_thread_resumes_on_its_saved_context() {
        static STOLEN: Reached = Reached::new();

        loom::model(|| {
            let m = Machine::new();
            let inbox = Arc::new(Mutex::new(false));

            // The victim core preempts T: Ready, still on_cpu, in the victim's queue.
            {
                let mut core = m.sched.lock().unwrap();
                core.hs.preempt();
                core.queued += 1;
            }

            // The victim core, in the kernel's program order: save, finish the switch, then serve
            // the steal at the next scheduler entry.
            let victim = {
                let m = Arc::clone(&m);
                let inbox = Arc::clone(&inbox);
                thread::spawn(move || {
                    m.ctx.with_mut(|c| {
                        // SAFETY: as in the first harness: the write the resume must be ordered
                        // after, checked by loom rather than argued.
                        unsafe { *c = SAVED }
                    });
                    {
                        let mut core = m.sched.lock().unwrap();
                        assert_eq!(core.hs.finish_switch(), SwitchOutVerdict::Cleared);
                    }
                    {
                        let mut core = m.sched.lock().unwrap();
                        assert_eq!(core.queued, 1);
                        core.queued -= 1;
                    }
                    *inbox.lock().unwrap() = true;
                })
            };

            // The thief polls the inbox a bounded number of times; loom runs the victim between.
            let thief = {
                let m = Arc::clone(&m);
                let inbox = Arc::clone(&inbox);
                thread::spawn(move || {
                    for _ in 0..2 {
                        let arrived = core::mem::take(&mut *inbox.lock().unwrap());
                        if arrived {
                            {
                                // A steal moves a Ready thread: no delivery to assert.
                                let mut core = m.sched.lock().unwrap();
                                assert!(
                                    !core.hs.on_cpu,
                                    "switching into a thread that is still on a cpu"
                                );
                                core.hs.switch_in();
                            }
                            m.ctx.with(|c| {
                                // SAFETY: the read side; ordered through the victim's program
                                // order plus the inbox handoff, or loom fails the run.
                                assert_eq!(unsafe { *c }, SAVED, "stole an unsaved context");
                            });
                            STOLEN.mark();
                            return;
                        }
                        thread::yield_now();
                    }
                })
            };

            victim.join().unwrap();
            thief.join().unwrap();
        });

        STOLEN.assert("an execution in which the thief actually receives the stolen thread");
    }

    /// A thief that pops the victim's queue directly steals a thread whose context is still
    /// being saved. The single-owner run-queue rule is broken on purpose, and `switch_in`'s
    /// tripwire or loom's cell check, whichever the interleaving reaches first, must fire.
    #[test]
    #[should_panic(expected = "still on a cpu")]
    fn a_thief_that_pops_a_foreign_queue_steals_an_unsaved_context() {
        loom::model(|| {
            let m = Machine::new();
            {
                let mut core = m.sched.lock().unwrap();
                core.hs.preempt();
                core.queued += 1;
            }

            let victim = {
                let m = Arc::clone(&m);
                thread::spawn(move || {
                    m.ctx.with_mut(|c| {
                        // SAFETY: deliberately racy under the broken thief; the harness requires
                        // the model to fail.
                        unsafe { *c = SAVED }
                    });
                    let mut core = m.sched.lock().unwrap();
                    let _ = core.hs.finish_switch();
                })
            };

            let thief = {
                let m = Arc::clone(&m);
                thread::spawn(move || {
                    let took = {
                        let mut core = m.sched.lock().unwrap();
                        if core.queued == 1 {
                            core.queued -= 1;
                            // The tripwire, as a real assert (see resume()).
                            assert!(
                                !core.hs.on_cpu,
                                "switching into a thread that is still on a cpu"
                            );
                            core.hs.switch_in();
                            true
                        } else {
                            false
                        }
                    };
                    if took {
                        m.ctx.with(|c| {
                            // SAFETY: the deliberately unsound read; either this cell check or
                            // the tripwire above ends the run.
                            assert_eq!(unsafe { *c }, SAVED);
                        });
                    }
                })
            };

            victim.join().unwrap();
            thief.join().unwrap();
        });
    }

    /// An undelivered wake racing a park strands nobody. A spurious waker fires at a
    /// parked receiver with nothing delivered, racing the receiver's own switch-out; the gate
    /// refuses it in every interleaving, the receiver stays parked and waiting, and the real
    /// sender that follows still completes the rendezvous, after which the resume sees a
    /// delivery and a saved context.
    #[test]
    fn an_undelivered_wake_racing_a_park_strands_nobody() {
        static REFUSED_MID_SWITCH: Reached = Reached::new();
        static REFUSED_AFTER: Reached = Reached::new();

        loom::model(|| {
            let m = Machine::new();
            m.sched.lock().unwrap().hs.park(3);

            let victim = {
                let m = Arc::clone(&m);
                thread::spawn(move || {
                    m.ctx.with_mut(|c| {
                        // SAFETY: as in the first harness.
                        unsafe { *c = SAVED }
                    });
                    let completed = {
                        let mut core = m.sched.lock().unwrap();
                        match core.hs.finish_switch() {
                            SwitchOutVerdict::WakeCompleted => {
                                core.queued += 1;
                                true
                            }
                            SwitchOutVerdict::Cleared => false,
                            SwitchOutVerdict::Reap => unreachable!("nothing finished"),
                        }
                    };
                    if completed {
                        resume(&m);
                    }
                })
            };

            // One loom thread carries the spurious waker and then the real sender, in that
            // order, each racing the victim's switch-out.
            let wakers = {
                let m = Arc::clone(&m);
                thread::spawn(move || {
                    {
                        // The spurious wake delivers nothing; the gate refuses it either way.
                        let mut core = m.sched.lock().unwrap();
                        assert_eq!(
                            core.hs.try_wake(),
                            WakeVerdict::Refused,
                            "an undelivered wake was granted"
                        );
                        assert_eq!(core.hs.state, RunState::Blocked);
                        assert!(
                            core.hs.wait_on.is_some(),
                            "a refused wake unlinked the wait"
                        );
                        if core.hs.on_cpu {
                            REFUSED_MID_SWITCH.mark();
                        } else {
                            REFUSED_AFTER.mark();
                        }
                    }
                    let queued = {
                        // The real sender: deliver and wake in one critical section.
                        let mut core = m.sched.lock().unwrap();
                        core.hs.serve();
                        match core.hs.try_wake() {
                            WakeVerdict::Queue => {
                                core.queued += 1;
                                true
                            }
                            WakeVerdict::Deferred => false,
                            v => unreachable!("a served wake of a parked thread decided {v:?}"),
                        }
                    };
                    if queued {
                        resume(&m);
                    }
                })
            };

            victim.join().unwrap();
            wakers.join().unwrap();

            let core = m.sched.lock().unwrap();
            assert_eq!(
                core.queued, 1,
                "the receiver was queued {} times",
                core.queued
            );
            assert_eq!(
                core.hs.state,
                RunState::Running,
                "the receiver never resumed"
            );
        });

        REFUSED_MID_SWITCH.assert("a spurious wake refused while the receiver was mid-switch-out");
        REFUSED_AFTER.assert("a spurious wake refused after the receiver was fully parked");
    }

    /// Without the gate, a spurious wake completes a rendezvous that never happened. The wake
    /// keeps the switch-out deferral but never asks whether anything was delivered, so every
    /// interleaving strands the receiver and the resume's delivery tripwire must fire.
    #[test]
    #[should_panic(expected = "resumed with nothing delivered")]
    fn without_the_gate_a_spurious_wake_completes_an_empty_rendezvous() {
        loom::model(|| {
            let m = Machine::new();
            m.sched.lock().unwrap().hs.park(3);

            let victim = {
                let m = Arc::clone(&m);
                thread::spawn(move || {
                    m.ctx.with_mut(|c| {
                        // SAFETY: single writer; the failure this harness requires is the
                        // delivery tripwire, not a cell race.
                        unsafe { *c = SAVED }
                    });
                    let completed = {
                        let mut core = m.sched.lock().unwrap();
                        matches!(core.hs.finish_switch(), SwitchOutVerdict::WakeCompleted)
                    };
                    if completed {
                        // The receiver resumes with an empty mailbox: the strand.
                        resume(&m);
                    }
                })
            };

            let spurious = {
                let m = Arc::clone(&m);
                thread::spawn(move || {
                    let queued = {
                        let mut core = m.sched.lock().unwrap();
                        // The historical wake: deferral kept, gate absent.
                        assert_eq!(core.hs.state, RunState::Blocked);
                        if core.hs.on_cpu {
                            core.hs.wake_pending = true;
                            false
                        } else {
                            core.hs.state = RunState::Ready;
                            core.hs.wait_on = None;
                            true
                        }
                    };
                    if queued {
                        resume(&m);
                    }
                })
            };

            victim.join().unwrap();
            spurious.join().unwrap();
        });
    }
}
