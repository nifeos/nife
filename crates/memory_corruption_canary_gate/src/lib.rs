//! **The corruption canary's arm/check/disarm serialization**, lifted out of `kernel/src/sched.rs`
//! so loom can explore every interleaving of it on the host: one word, four states, every
//! transition a single compare-exchange. [`arm`](Gate::arm) spins until idle and returns an
//! [`ArmGuard`], whose drop publishes `ARMED`; [`try_check`](Gate::try_check) returns a
//! [`CheckGuard`] or [`None`]; [`disarm`](Gate::disarm) quiesces, so once it returns no pass is
//! running or can start. One guard of either kind at a time. No fairness: `arm` and `disarm` spin,
//! bounded by the kernel's callers, outside IRQ context. See notes/interleaving/what-was-modeled.md,
//! "the corruption canary's serialization".
//!
//! Name: ratified 2026-08-23 (calef), renamed from `canary_gate`: a memory-corruption detector
//! over arbitrary byte ranges, not a stack canary.

#![cfg_attr(not(test), no_std)]

// The one line that makes this crate model-checkable. Under `--cfg loom` the atomic is loom's,
// which records every access and lets the model checker replay the protocol under every
// interleaving and every reordering the C11 model permits; under anything else it is the real one
// and this crate is exactly what the kernel compiles. Nothing but `script/interleaving-check` sets
// the cfg. See notes/interleaving.md.
#[cfg(not(loom))]
use core::sync::atomic::{AtomicU8, Ordering};

#[cfg(loom)]
use loom::sync::atomic::{AtomicU8, Ordering};

/// Nothing is watched; a check refuses instantly.
const DISARMED: u8 = 0;
/// A watch plan is published and nobody is touching it.
const ARMED: u8 = 1;
/// An armer owns the instrument and is rewriting the plan.
const ARMING: u8 = 2;
/// A checker owns the instrument and is running a pass.
const CHECKING: u8 = 3;

/// One spin-wait step. Loom needs an explicit yield to know the loop is waiting on another
/// thread; the kernel wants the architectural pause hint.
#[cfg(not(loom))]
fn pause() {
    core::hint::spin_loop();
}

#[cfg(loom)]
fn pause() {
    loom::thread::yield_now();
}

/// The canary instrument's one-word state machine; the module documentation has the protocol.
///
/// # Examples
///
/// ```
/// # // A worker with a deadline, as this crate's tests use: the spins below would hang, not fail.
/// # let (done, finished) = std::sync::mpsc::channel();
/// # std::thread::spawn(move || {
/// use memory_corruption_canary_gate::Gate;
///
/// static GATE: Gate = Gate::new();
///
/// // Arm: exclusive while the watch plan and shadow are (re)written.
/// let guard = GATE.arm();
/// // ... snapshot the watched ranges into the shadow ...
/// drop(guard); // publishes ARMED
///
/// // The timer tick: a sampling instrument may skip a beat.
/// if let Some(pass) = GATE.try_check() {
///     // ... re-read every watched byte against the shadow ...
///     drop(pass);
/// }
///
/// // A caller that must have a completed pass (the kernel test) loops instead:
/// let pass = loop {
///     if let Some(p) = GATE.try_check() {
///         break p;
///     }
///     core::hint::spin_loop();
/// };
/// drop(pass);
///
/// // Disarm quiesces: after this returns, nothing reads the watched ranges.
/// GATE.disarm();
/// # let _ = done.send(());
/// # });
/// # finished
/// #     .recv_timeout(std::time::Duration::from_secs(5))
/// #     .expect("the example panicked (see above), or it never returned");
/// ```
pub struct Gate {
    state: AtomicU8,
}

/// Exclusive ownership of the instrument for (re)writing the watch plan. Dropping it publishes
/// the plan (`ARMED`, release).
#[must_use = "dropping the guard is what publishes the plan; hold it across the writes"]
pub struct ArmGuard<'a> {
    gate: &'a Gate,
}

/// Exclusive ownership of the instrument for one check pass. Dropping it reopens the gate
/// (`ARMED`, release), so the next pass sees this one's shadow updates.
#[must_use = "dropping the guard is what reopens the gate; hold it across the pass"]
pub struct CheckGuard<'a> {
    gate: &'a Gate,
}

impl Gate {
    /// A disarmed gate.
    ///
    /// `const` off the loom path so a kernel static can hold one. Loom's atomics have no const
    /// constructor (they carry model-checker state), which is why this is written twice; the
    /// kernel only ever compiles the first.
    #[cfg(not(loom))]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(DISARMED),
        }
    }

    /// A disarmed gate. See the `const` twin above for why there are two.
    #[cfg(loom)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: AtomicU8::new(DISARMED),
        }
    }

    /// **Take the instrument to (re)write the watch plan.** Spins while a check pass or another
    /// armer holds it; returns only with exclusive ownership.
    ///
    /// **Acquire** on success, pairing with every guard drop's release, so the armer sees the
    /// last pass's shadow writes before overwriting them. Must not be called from a context that
    /// could preempt the current owner on the same core (the kernel calls it from thread context;
    /// the tick-side `try_check` never spins, so a tick landing mid-arm skips rather than
    /// deadlocks).
    pub fn arm(&self) -> ArmGuard<'_> {
        loop {
            let seen = self.state.load(Ordering::Relaxed);
            if (seen == DISARMED || seen == ARMED)
                && self
                    .state
                    .compare_exchange(seen, ARMING, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
            {
                return ArmGuard { gate: self };
            }
            pause();
        }
    }

    /// **Try to start one check pass.** One compare-exchange from `ARMED`: [`Some`] is exclusive
    /// ownership, [`None`] means disarmed, mid-arm, or another pass is running.
    ///
    /// A [`None`] on a sampling path (the timer tick) is a skipped beat, not an error. A caller
    /// that must observe a completed pass loops until it holds the guard itself; returning `None`
    /// silently to such a caller was the c906 flake (module docs).
    ///
    /// **Acquire** on success, pairing with the arm guard's release (the pass sees the whole
    /// plan, never a torn one) and the previous check guard's release (it sees the absorbed
    /// divergences).
    pub fn try_check(&self) -> Option<CheckGuard<'_>> {
        self.state
            .compare_exchange(ARMED, CHECKING, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
            // `then`, not `then_some`: the guard's Drop reopens the gate, so a guard built
            // eagerly on the refused path would store `ARMED` into a gate it never held.
            .then(|| CheckGuard { gate: self })
    }

    /// **Disarm, and quiesce.** When this returns, no check pass is mid-flight and none can
    /// start: the caller may free or repurpose the watched memory. Spins while a pass or an
    /// armer holds the instrument.
    ///
    /// **Acquire** on the closing compare-exchange, pairing with the final guard drop's release,
    /// so everything the last pass did happens-before anything the caller does to the watched
    /// ranges afterwards.
    pub fn disarm(&self) {
        loop {
            let seen = self.state.load(Ordering::Relaxed);
            if seen == DISARMED {
                return;
            }
            if seen == ARMED
                && self
                    .state
                    .compare_exchange(ARMED, DISARMED, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
            {
                return;
            }
            pause();
        }
    }

    /// **Is a plan published right now?** One relaxed load: the unarmed tick path's whole cost,
    /// and a hint only. The answer can be stale by the time the caller acts;
    /// [`try_check`](Self::try_check)'s compare-exchange is what decides.
    #[must_use]
    pub fn is_armed_hint(&self) -> bool {
        self.state.load(Ordering::Relaxed) == ARMED
    }
}

#[cfg(not(loom))]
impl Default for Gate {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ArmGuard<'_> {
    fn drop(&mut self) {
        // Release: publish the plan the holder wrote.
        self.gate.state.store(ARMED, Ordering::Release);
    }
}

impl Drop for CheckGuard<'_> {
    fn drop(&mut self) {
        // Release: publish this pass's shadow updates to the next owner.
        self.gate.state.store(ARMED, Ordering::Release);
    }
}

// --- The ordinary host tests -----------------------------------------------------------------
//
// Single-threaded: what each transition means, and that the refusals are refusals. The
// *interleavings* are the loom module below, which is a different question and a different tool.
#[cfg(all(test, not(loom)))]
mod tests {
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::thread;
    use std::time::Duration;

    use super::*;

    /// **Every test body runs on a worker with a deadline**: `arm` and `disarm` spin until they
    /// win, so a mutant that breaks one never returns, and a suite calling it directly would hang
    /// rather than fail. Four mutants did (both halves of `arm`'s acceptance condition, `disarm`'s,
    /// and deleting `ArmGuard`'s `Drop`). A panic inside the body drops the sender, so the two
    /// failures are told apart. See the `## 2026-09-20` section of
    /// notes/mutation-testing/new-crate-backlog.md.
    fn bounded(what: &str, body: impl FnOnce() + Send + 'static) {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            body();
            let _ = tx.send(());
        });
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(()) => {}
            Err(RecvTimeoutError::Disconnected) => {
                panic!("{what}: the body panicked; its own message is above")
            }
            Err(RecvTimeoutError::Timeout) => {
                panic!("{what}: a spin loop never made progress, so the gate deadlocked")
            }
        }
    }

    #[test]
    fn a_disarmed_gate_refuses_a_check() {
        bounded("a disarmed gate refuses a check", || {
            let gate = Gate::new();
            assert!(gate.try_check().is_none());
            assert!(!gate.is_armed_hint());
        });
    }

    #[test]
    fn arming_publishes_only_when_the_guard_drops() {
        bounded("arming publishes only when the guard drops", || {
            let gate = Gate::new();
            let guard = gate.arm();
            assert!(!gate.is_armed_hint(), "mid-arm must not read as armed");
            assert!(
                gate.try_check().is_none(),
                "a check must not see a torn plan"
            );
            drop(guard);
            assert!(gate.is_armed_hint());
            assert!(gate.try_check().is_some());
        });
    }

    #[test]
    fn a_pass_is_single_flight_and_the_gate_reopens_after_it() {
        bounded("a pass is single flight", || {
            let gate = Gate::new();
            drop(gate.arm());
            let pass = gate.try_check().expect("armed gate admits a pass");
            assert!(gate.try_check().is_none(), "one pass at a time");
            drop(pass);
            assert!(gate.try_check().is_some(), "the gate reopens");
        });
    }

    #[test]
    fn disarm_closes_the_gate_and_is_idempotent() {
        bounded("disarm closes the gate and is idempotent", || {
            let gate = Gate::new();
            drop(gate.arm());
            gate.disarm();
            assert!(gate.try_check().is_none());
            gate.disarm(); // already disarmed: returns at once, changes nothing
            assert!(gate.try_check().is_none());
        });
    }

    #[test]
    fn rearming_a_disarmed_gate_works() {
        bounded("rearming a disarmed gate works", || {
            let gate = Gate::new();
            drop(gate.arm());
            gate.disarm();
            drop(gate.arm());
            assert!(gate.try_check().is_some());
        });
    }

    /// **Re-arming an *armed* gate takes it out of `ARMED` first**, which is the transition the
    /// suite had no test for: every other path reaches `arm` from `DISARMED`, and the two states
    /// differ in exactly the way the torn-plan bug was about.
    ///
    /// An `arm` that returned its guard without winning the compare-exchange would leave the gate
    /// readable as armed and admit a check pass *while the plan is being rewritten*, which is the
    /// hole the module documentation calls the torn plan. From `DISARMED` that mistake is
    /// invisible (the state is not `ARMED` either way), so it takes a re-arm to see it.
    #[test]
    fn rearming_an_armed_gate_takes_it_out_of_armed() {
        bounded("rearming an armed gate takes it out of armed", || {
            let gate = Gate::new();
            drop(gate.arm());
            assert!(gate.is_armed_hint(), "the first arm published a plan");

            let guard = gate.arm();
            assert!(
                !gate.is_armed_hint(),
                "a re-arm must take the gate out of ARMED, not leave the old plan readable"
            );
            assert!(
                gate.try_check().is_none(),
                "a check must not run against a plan being rewritten"
            );
            drop(guard);
            assert!(gate.try_check().is_some(), "the re-armed plan is published");
        });
    }
}

// --- The loom model --------------------------------------------------------------------------
#[cfg(all(test, loom))]
mod interleavings {
    //! Every interleaving of the arm/check/disarm protocol, and every reordering C11 permits
    //! within it.
    //!
    //! Run with `script/interleaving-check`. See notes/interleaving.md for the method, what this
    //! model cannot reach, and what it cost. The shared `plan` cell below stands in for the
    //! kernel's watch table and shadow buffer: loom's `UnsafeCell` records every access, so any
    //! interleaving in which two owners touch it at once fails the run. That is precisely the
    //! torn-plan hole in the module docs, restated as a checkable property.

    use loom::cell::UnsafeCell;
    use loom::sync::Arc;
    use loom::thread;

    use super::*;

    /// **Non-vacuity, which a model checker needs for exactly the reason a solver does.**
    ///
    /// A harness whose interesting branch is never reached passes without checking anything, and
    /// reports success while doing it. Kani's answer is `kani::cover!`; loom has no equivalent, so
    /// these count reachability across the model's executions and the harness asserts on the count
    /// afterwards. The flag is a *real* atomic, deliberately: it lives outside the model and
    /// accumulates across every execution loom runs, where a loom atomic would be reset by each.
    ///
    /// See notes/verification.md's non-vacuity section for why this is not optional.
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

    /// **A re-arm never rewrites the plan a checker is reading.**
    ///
    /// This is the torn-plan hole made falsifiable: the armer writes the cell under an
    /// `ArmGuard`, the checker reads it under a `CheckGuard`, and loom fails any execution in
    /// which the two overlap. The old two-variable protocol fails this harness (the checker
    /// between its `ARMED` load and its `IN_CHECK` win was invisible to the armer's drain).
    #[test]
    fn an_armer_never_rewrites_the_plan_a_checker_is_reading() {
        static CHECK_RAN: Reached = Reached::new();
        static CHECK_REFUSED: Reached = Reached::new();

        loom::model(|| {
            let shared = Arc::new((Gate::new(), UnsafeCell::new(0u64)));

            // First arm: publish a plan, so the checker has something to race the RE-arm for.
            {
                let (gate, plan) = &*shared;
                let guard = gate.arm();
                plan.with_mut(|p| unsafe { *p = 1 });
                drop(guard);
            }

            let checker = {
                let shared = Arc::clone(&shared);
                thread::spawn(move || {
                    let (gate, plan) = &*shared;
                    if let Some(pass) = gate.try_check() {
                        CHECK_RAN.mark();
                        let seen = plan.with(|p| unsafe { *p });
                        drop(pass);
                        // Whole values only: 1 (before the re-arm) or 2 (after). A torn plan
                        // has no third value to show in this model; what loom actually checks
                        // is that the accesses never overlap at all.
                        assert!(seen == 1 || seen == 2, "torn plan: {seen}");
                    } else {
                        CHECK_REFUSED.mark();
                    }
                })
            };

            // The re-arm, racing the checker.
            {
                let (gate, plan) = &*shared;
                let guard = gate.arm();
                plan.with_mut(|p| unsafe { *p = 2 });
                drop(guard);
            }

            checker.join().unwrap();
        });

        CHECK_RAN.assert("a pass that ran against the re-arm");
        CHECK_REFUSED.assert("a pass refused while the armer held the gate");
    }

    /// **Disarm returns only after the in-flight pass has let go.**
    ///
    /// The quiescence promise: after `disarm()` returns, the caller owns the watched memory
    /// outright. The main thread writes the cell bare-handed after disarming; loom fails any
    /// execution in which a checker still holds it.
    #[test]
    fn disarm_returns_only_after_the_pass_has_let_go() {
        static PASS_WAS_IN_FLIGHT: Reached = Reached::new();

        loom::model(|| {
            let shared = Arc::new((Gate::new(), UnsafeCell::new(0u64)));

            {
                let (gate, plan) = &*shared;
                let guard = gate.arm();
                plan.with_mut(|p| unsafe { *p = 1 });
                drop(guard);
            }

            let checker = {
                let shared = Arc::clone(&shared);
                thread::spawn(move || {
                    let (gate, plan) = &*shared;
                    if let Some(pass) = gate.try_check() {
                        PASS_WAS_IN_FLIGHT.mark();
                        plan.with(|p| unsafe { *p });
                        drop(pass);
                    }
                })
            };

            let (gate, plan) = &*shared;
            gate.disarm();
            // Quiesced: this bare write is legal only if no pass can still hold the cell.
            plan.with_mut(|p| unsafe { *p = 99 });

            checker.join().unwrap();
        });

        PASS_WAS_IN_FLIGHT.assert("a disarm that raced an in-flight pass");
    }

    /// **Two checkers never run a pass at once.**
    ///
    /// Single-flight, checked rather than asserted: both threads mutate the cell under their
    /// guard (the real pass writes the shadow), so an overlap is an access-tracking failure, not
    /// a value we might fail to notice.
    #[test]
    fn two_checkers_never_run_a_pass_at_once() {
        static BOTH_TRIED_ONE_REFUSED: Reached = Reached::new();
        static BOTH_RAN_IN_TURN: Reached = Reached::new();

        loom::model(|| {
            let shared = Arc::new((Gate::new(), UnsafeCell::new(0u64)));
            drop(shared.0.arm());

            let spawn_checker = |shared: &Arc<(Gate, UnsafeCell<u64>)>| {
                let shared = Arc::clone(shared);
                thread::spawn(move || {
                    let (gate, plan) = &*shared;
                    if let Some(pass) = gate.try_check() {
                        plan.with_mut(|p| unsafe { *p += 1 });
                        drop(pass);
                        true
                    } else {
                        false
                    }
                })
            };

            let a = spawn_checker(&shared);
            let b = spawn_checker(&shared);
            let (ran_a, ran_b) = (a.join().unwrap(), b.join().unwrap());

            if ran_a && ran_b {
                BOTH_RAN_IN_TURN.mark();
                shared.1.with(|p| assert_eq!(unsafe { *p }, 2));
            } else if ran_a || ran_b {
                BOTH_TRIED_ONE_REFUSED.mark();
            } else {
                unreachable!("an armed, uncontended gate refused both passes");
            }
        });

        BOTH_TRIED_ONE_REFUSED.assert("a pass refused because its twin held the gate");
        BOTH_RAN_IN_TURN.assert("both passes running, in turn");
    }
}
