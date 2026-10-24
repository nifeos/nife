//! **The work-steal request slot**: the one lock-free, cross-core handshake in the scheduler. An
//! idle core cannot reach a loaded core's run queue (§11 (SMP: per-CPU run queues, message-based
//! migration)), so it parks a request here and pokes the victim, which hands one thread into the
//! requester's inbox (§28 (SMP placement)). The slot holds 0 (nobody asking) or `id + 1`.
//!
//! [`Slot::claim`] is a release compare-exchange from zero, so idle cores collapse to one request
//! per victim; [`Slot::take`] is an acquire swap to zero, so a take with no hand-off still lets the
//! next thief in. The pair carries nothing yet and makes a request field safe the day one is
//! added; loom falsifies the relaxed spelling. A failed claim is relaxed: the loser acts on
//! nothing. See notes/interleaving/what-was-modeled.md, "`crates/work_steal_slot`, the pilot".

//! # BUGS
//!
//! - A take that finds an empty run queue drops the request: this promises delivery of the
//!   request, never a thread (§28's bounded cost of an empty give; the idle loop retries).
//! - A poked victim that never reaches a scheduler entry locks every other thief out, outside
//!   loom's model.
//! - The identity is not authenticated; the only caller passes its own `cpu::id()`.
//! - [`MAX_REQUESTER`] guards the bias's addition, unreachable and covered by a host test alone.
//!
//! Name: ratified 2026-08-23 (calef), renamed from `steal_request`: the type is `Slot` with
//! `claim`/`take`, and "work-steal" is the field's compound (Cilk, Go, Rayon).

#![cfg_attr(not(test), no_std)]

// The one line that makes this crate model-checkable. Under `--cfg loom` the atomic is loom's,
// which records every access and lets the model checker replay the protocol under every
// interleaving and every reordering the C11 model permits; under anything else it is the real one
// and this crate is exactly what the kernel compiles. Nothing but `script/interleaving-check` sets
// the cfg. See notes/interleaving.md.
#[cfg(not(loom))]
use core::sync::atomic::{AtomicU32, Ordering};

#[cfg(loom)]
use loom::sync::atomic::{AtomicU32, Ordering};

/// The empty slot. Zero, so a freshly zeroed per-CPU block starts unclaimed.
const EMPTY: u32 = 0;

/// The largest requester id the biased encoding can carry.
///
/// The bias is `id + 1`, so `u32::MAX` would encode as zero and read back as *empty*: a request
/// that vanished. [`Slot::claim`] refuses that id instead of wrapping into it. A kernel cannot
/// reach this (`MAX_CPUS` is single digits), which is exactly why the refusal is written down
/// rather than left to the arithmetic.
pub const MAX_REQUESTER: u32 = u32::MAX - 1;

/// One core's work-steal request slot: at most one pending request, first come first served.
///
/// See the module documentation for the protocol and for what it does *not* promise.
///
/// # Examples
///
/// The two halves of one round, as the kernel spells them:
///
/// ```
/// use work_steal_slot::Slot;
///
/// // On core 3, idle, having picked core 0 as the most loaded victim:
/// let victim_slot = Slot::new();
/// assert!(victim_slot.claim(3));           // ... then send core 0 a reschedule interrupt
///
/// // A second idle core, core 5, finds the slot already spoken for and moves on:
/// assert!(!victim_slot.claim(5));
///
/// // On core 0, at its next scheduler entry:
/// assert_eq!(victim_slot.take(), Some(3)); // ... then pop one thread into core 3's inbox
/// assert_eq!(victim_slot.take(), None);    // and the slot is open again
/// ```
pub struct Slot {
    /// `EMPTY`, or the requesting core's id plus one.
    inner: AtomicU32,
}

impl Slot {
    /// An unclaimed slot.
    ///
    /// `const` off the loom path so a static per-CPU array can hold one. Loom's atomics have no
    /// const constructor (they carry model-checker state), which is why this is written twice; the
    /// kernel only ever compiles the first.
    #[cfg(not(loom))]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            inner: AtomicU32::new(EMPTY),
        }
    }

    /// An unclaimed slot. See the `const` twin above for why there are two.
    #[cfg(loom)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: AtomicU32::new(EMPTY),
        }
    }

    /// **Ask this core for work.** Returns `true` if the request was parked, `false` if the slot
    /// already holds someone else's (or if `requester` is past [`MAX_REQUESTER`]).
    ///
    /// A `false` is not an error and is not a queue position: the caller is idle, and it will ask
    /// again next time round its idle loop. That is what keeps a thundering herd of idle cores to
    /// one steal per victim per round.
    ///
    /// **Release**, so that anything the caller wrote before this is visible to whoever takes the
    /// claim. Relaxed on failure, because a caller that lost the race acts on nothing it read.
    pub fn claim(&self, requester: u32) -> bool {
        let Some(encoded) = requester.checked_add(1) else {
            return false;
        };
        self.inner
            .compare_exchange(EMPTY, encoded, Ordering::Release, Ordering::Relaxed)
            .is_ok()
    }

    /// **Look at a pending request without taking it.** Diagnostic only (the hang dump): a slot
    /// that stays claimed across dumps means the victim never reached a scheduler entry to serve
    /// it, which is a wedged victim seen from the thief's side.
    ///
    /// A relaxed load, deliberately outside the claim/take pairing: it publishes nothing and the
    /// reader acts on nothing but the printed byte. Never use it to decide a hand-off; that is
    /// what [`take`](Self::take)'s read-and-clear exists for.
    pub fn peek(&self) -> Option<u32> {
        match self.inner.load(Ordering::Relaxed) {
            EMPTY => None,
            encoded => Some(encoded - 1),
        }
    }

    /// **Take a pending request, and open the slot for the next one.** Returns the requesting
    /// core's id, or `None` if nobody was asking.
    ///
    /// Read and clear in one step, so a request is delivered to exactly one caller and cannot be
    /// served twice. **Acquire**, pairing with the release in [`claim`](Self::claim).
    pub fn take(&self) -> Option<u32> {
        match self.inner.swap(EMPTY, Ordering::Acquire) {
            EMPTY => None,
            encoded => Some(encoded - 1),
        }
    }
}

#[cfg(not(loom))]
impl Default for Slot {
    fn default() -> Self {
        Self::new()
    }
}

// --- The ordinary host tests -----------------------------------------------------------------
//
// Single-threaded: what the encoding means, and that the two entry points are total. The
// *interleavings* are the loom module below, which is a different question and a different tool.
#[cfg(all(test, not(loom)))]
mod tests {
    use super::*;

    #[test]
    fn an_untouched_slot_holds_nothing() {
        assert_eq!(Slot::new().take(), None);
        assert_eq!(Slot::default().take(), None);
    }

    #[test]
    fn a_claim_round_trips_the_requester_id() {
        // Core 0 is the interesting one: it encodes as 1, and an unbiased encoding would have made
        // a request from the boot core indistinguishable from an empty slot.
        for id in [0, 1, 7, MAX_REQUESTER] {
            let slot = Slot::new();
            assert!(slot.claim(id), "a fresh slot refused a claim from {id}");
            assert_eq!(slot.take(), Some(id));
        }
    }

    #[test]
    fn a_claimed_slot_refuses_a_second_claim_and_keeps_the_first() {
        let slot = Slot::new();
        assert!(slot.claim(2));
        assert!(
            !slot.claim(5),
            "the slot took a second claim over a pending one"
        );
        assert_eq!(slot.take(), Some(2), "the second claim displaced the first");
    }

    #[test]
    fn a_peek_reads_without_taking() {
        let slot = Slot::new();
        assert_eq!(slot.peek(), None);
        assert!(slot.claim(2));
        assert_eq!(slot.peek(), Some(2), "peek missed a pending request");
        assert_eq!(slot.peek(), Some(2), "peek consumed the request");
        assert_eq!(slot.take(), Some(2), "peek made the take miss");
        assert_eq!(slot.peek(), None);
    }

    #[test]
    fn a_take_opens_the_slot_again() {
        let slot = Slot::new();
        assert!(slot.claim(2));
        assert_eq!(slot.take(), Some(2));
        assert_eq!(slot.take(), None, "a request was served twice");
        assert!(slot.claim(5), "the slot stayed shut after being served");
        assert_eq!(slot.take(), Some(5));
    }

    /// The bias is an addition, so it has an overflow, so it has a refusal. Unreachable in a
    /// kernel; see the module's BUGS note.
    #[test]
    fn an_id_the_encoding_cannot_carry_is_refused_rather_than_wrapped() {
        let slot = Slot::new();
        assert!(!slot.claim(u32::MAX));
        assert_eq!(
            slot.take(),
            None,
            "an unencodable id wrapped into the slot instead of being refused"
        );
    }
}

// --- The loom model --------------------------------------------------------------------------
#[cfg(all(test, loom))]
mod interleavings {
    //! Every interleaving of the steal handshake, and every reordering C11 permits within it.
    //!
    //! Run with `script/interleaving-check`. See notes/interleaving.md for the method, what this
    //! model cannot reach, and what it cost.

    use loom::cell::UnsafeCell;
    use loom::sync::Arc;
    use loom::sync::atomic::AtomicUsize;
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

    /// **Two idle cores race for one victim, and exactly one wins.**
    ///
    /// The claim that "a thundering herd of idle cores collapses to one steal per victim per round"
    /// is a claim about a compare-exchange under concurrency, which is exactly the sentence a test
    /// on one thread cannot check.
    #[test]
    fn two_thieves_race_and_exactly_one_claim_is_granted() {
        // Both orders must be explored, or "exactly one wins" is being checked against a search
        // that only ever ran them in one sequence.
        static A_WON: Reached = Reached::new();
        static B_WON: Reached = Reached::new();

        loom::model(|| {
            let slot = Arc::new(Slot::new());

            let a = {
                let s = Arc::clone(&slot);
                thread::spawn(move || s.claim(0))
            };
            let b = {
                let s = Arc::clone(&slot);
                thread::spawn(move || s.claim(1))
            };

            let won_a = a.join().unwrap();
            let won_b = b.join().unwrap();
            assert!(
                won_a ^ won_b,
                "both thieves were granted the slot, or neither was: {won_a} {won_b}"
            );

            let winner = if won_a {
                A_WON.mark();
                0
            } else {
                B_WON.mark();
                1
            };
            assert_eq!(
                slot.take(),
                Some(winner),
                "the slot held someone other than the thief whose claim succeeded"
            );
            assert_eq!(slot.take(), None, "the request survived being served");
        });

        A_WON.assert("an interleaving in which the first thief wins the race");
        B_WON.assert("an interleaving in which the second thief wins the race");
    }

    /// **A granted claim is served exactly once, or not yet, and never both.**
    ///
    /// The victim polls while the thief is still claiming, which is the real shape: `take` runs
    /// from a scheduler entry that has no idea whether a claim is in flight. The property is
    /// conservation. A request that was granted is either in the victim's hand or still in the
    /// slot, and a request that was never granted is in neither.
    #[test]
    fn a_granted_claim_is_served_exactly_once() {
        // Both outcomes must occur across the search, or the harness is only checking one of them.
        static CAUGHT_IN_FLIGHT: Reached = Reached::new();
        static STILL_PARKED: Reached = Reached::new();

        loom::model(|| {
            let slot = Arc::new(Slot::new());

            let thief = {
                let s = Arc::clone(&slot);
                thread::spawn(move || {
                    assert!(s.claim(3), "an uncontended claim was refused");
                })
            };

            let victim = {
                let s = Arc::clone(&slot);
                thread::spawn(move || {
                    // Two scheduler entries. More would only re-explore the same states, and loom
                    // pays for every one of them.
                    let mut seen = None;
                    for _ in 0..2 {
                        if let Some(id) = s.take() {
                            assert!(seen.is_none(), "the same request was served twice");
                            seen = Some(id);
                        }
                    }
                    seen
                })
            };

            thief.join().unwrap();
            let served = victim.join().unwrap();

            match served {
                Some(id) => {
                    assert_eq!(id, 3, "the victim was handed a requester nobody claimed as");
                    assert_eq!(slot.take(), None, "a served request stayed in the slot");
                    CAUGHT_IN_FLIGHT.mark();
                }
                None => {
                    assert_eq!(
                        slot.take(),
                        Some(3),
                        "the request was neither served nor parked: it evaporated"
                    );
                    STILL_PARKED.mark();
                }
            }
        });

        CAUGHT_IN_FLIGHT.assert("an interleaving where the victim's poll catches the claim");
        STILL_PARKED.assert("an interleaving where the claim lands after the victim's last poll");
    }

    /// **The pairing, and what it buys.**
    ///
    /// The thief writes before it claims; the victim reads after it takes. Under release/acquire
    /// the write is visible, and loom's `UnsafeCell` is what turns "visible" into a checked fact:
    /// it records every access and fails the model if two of them are unordered.
    ///
    /// The kernel puts nothing in this window today (a request carries only an identity, and the
    /// identity rides inside the compare-exchange). This harness is what makes it safe to start.
    #[test]
    fn a_take_sees_everything_the_thief_wrote_before_claiming() {
        // Without this the harness would pass on a search in which the main thread always takes
        // first and never reads the payload at all: the pairing would be untested, and the run
        // would say SUCCESS.
        static READ_THE_PAYLOAD: Reached = Reached::new();

        loom::model(|| {
            let slot = Arc::new(Slot::new());
            let payload = Arc::new(UnsafeCell::new(0u32));

            let thief = {
                let s = Arc::clone(&slot);
                let p = Arc::clone(&payload);
                thread::spawn(move || {
                    // SAFETY: the only write to this cell, and it is ordered before the claim that
                    // publishes it. Any reader reaches it only through a successful `take`, which
                    // acquires what this releases. That edge is the property under test, and loom
                    // fails the model if it is missing rather than trusting this comment.
                    p.with_mut(|v| unsafe { *v = 7 });
                    assert!(s.claim(1));
                })
            };

            if slot.take().is_some() {
                // SAFETY: the take above acquired the thief's release, so the write happened-before
                // this read and no write can be concurrent with it (the thief performs exactly one,
                // before claiming).
                payload.with(|v| {
                    assert_eq!(
                        unsafe { *v },
                        7,
                        "the take saw a claim but not the write that preceded it"
                    );
                });
                READ_THE_PAYLOAD.mark();
            }

            thief.join().unwrap();
        });

        READ_THE_PAYLOAD.assert("an interleaving where the take catches the claim");
    }

    /// **The read and the clear are one step**: two victims take the same claimed slot, and exactly
    /// one may come away with the request; a `take` that read and then cleared would hand it to both.
    ///
    /// The kernel has one victim per slot today (`serve_steal_request` runs on the owning core with
    /// interrupts masked), so this guards the discipline rather than the running code: with the swap
    /// replaced by a load and a store, only this harness fails. See notes/interleaving.md, "And the
    /// pilot found nothing, which is its own result".
    #[test]
    fn a_second_victim_cannot_serve_the_same_request() {
        static BOTH_TRIED: Reached = Reached::new();

        loom::model(|| {
            let slot = Arc::new(Slot::new());
            assert!(slot.claim(4));

            let first = {
                let s = Arc::clone(&slot);
                thread::spawn(move || s.take())
            };
            let second = {
                let s = Arc::clone(&slot);
                thread::spawn(move || s.take())
            };

            let a = first.join().unwrap();
            let b = second.join().unwrap();

            assert!(
                !(a.is_some() && b.is_some()),
                "one request was served to two victims: {a:?} and {b:?}"
            );
            assert_eq!(
                [a, b].iter().flatten().count(),
                1,
                "the request was served to nobody: {a:?} {b:?}"
            );
            BOTH_TRIED.mark();
        });

        BOTH_TRIED.assert("any execution at all");
    }

    /// The relaxed twin of [`Slot`], which exists only to be broken.
    ///
    /// A harness that cannot be made to fail is not evidence (notes/verification.md's rule, applied
    /// to a model checker instead of a solver). This is the same protocol with the pairing removed.
    struct RelaxedSlot {
        inner: AtomicU32,
    }

    impl RelaxedSlot {
        fn new() -> Self {
            Self {
                inner: AtomicU32::new(EMPTY),
            }
        }
        fn claim(&self, requester: u32) -> bool {
            self.inner
                .compare_exchange(EMPTY, requester + 1, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
        }
        fn take(&self) -> Option<u32> {
            match self.inner.swap(EMPTY, Ordering::Relaxed) {
                EMPTY => None,
                encoded => Some(encoded - 1),
            }
        }
    }

    /// **The falsification.** The same handshake with `Relaxed` on both ends must fail.
    ///
    /// If this test ever stops panicking, either loom has stopped modelling the C11 relaxed
    /// ordering or the harness above has stopped depending on the pairing. Both are things we would
    /// want to hear about, and neither would be visible from the passing test alone.
    #[test]
    #[should_panic(expected = "Causality violation")]
    fn a_relaxed_pairing_publishes_nothing() {
        loom::model(|| {
            let slot = Arc::new(RelaxedSlot::new());
            let payload = Arc::new(UnsafeCell::new(0u32));

            let thief = {
                let s = Arc::clone(&slot);
                let p = Arc::clone(&payload);
                thread::spawn(move || {
                    // SAFETY: deliberately unsound, and that is the point of this harness: with
                    // relaxed orderings there is no edge making this write safe to read, and loom
                    // is expected to say so. Nothing outside this module uses `RelaxedSlot`.
                    p.with_mut(|v| unsafe { *v = 7 });
                    assert!(s.claim(1));
                })
            };

            if slot.take().is_some() {
                // SAFETY: see the write above. This read is the unsound half of a deliberate
                // counterexample.
                payload.with(|v| assert_eq!(unsafe { *v }, 7));
            }

            thief.join().unwrap();
        });
    }

    /// **The gossip claim, checked** (DECISIONS §28): a thief reads its victim's run-queue depth
    /// *relaxed and possibly stale*, and the design accepts that on purpose. This is the
    /// interleaving where the reading is wrong by the time it is acted on: the victim drains its
    /// last thread between the thief's load and the thief's claim.
    ///
    /// The property is that a stale reading costs a wasted round and nothing else. The victim is
    /// never handed a requester nobody claimed as, and the slot always ends open.
    #[test]
    fn a_stale_load_reading_costs_a_round_and_nothing_more() {
        // The whole point is the interleaving in which the reading is already wrong when it is
        // acted on. If the search never produced it, this harness would be checking the easy case.
        static ACTED_ON_A_STALE_READING: Reached = Reached::new();
        static SAW_THE_DRAIN: Reached = Reached::new();

        loom::model(|| {
            let slot = Arc::new(Slot::new());
            let runq_len = Arc::new(AtomicUsize::new(1));

            let thief = {
                let s = Arc::clone(&slot);
                let q = Arc::clone(&runq_len);
                thread::spawn(move || {
                    // The victim looked worth robbing when we looked. It may not be by the time
                    // the claim lands, and that is the accepted trade.
                    if q.load(Ordering::Relaxed) > 0 {
                        s.claim(2)
                    } else {
                        false
                    }
                })
            };

            // The victim runs its last queued thread, then reaches a scheduler entry and serves.
            runq_len.store(0, Ordering::Relaxed);
            let served = slot.take();

            let claimed = thief.join().unwrap();
            if let Some(id) = served {
                assert_eq!(id, 2, "the victim served an id nobody claimed as");
            }
            let parked = slot.take();
            if let Some(id) = parked {
                assert_eq!(id, 2, "the slot held an id nobody claimed as");
            }
            assert_eq!(
                claimed,
                served.is_some() || parked.is_some(),
                "a claim was granted and then lost, or one appeared without being granted"
            );
            if claimed {
                ACTED_ON_A_STALE_READING.mark();
            } else {
                SAW_THE_DRAIN.mark();
            }
        });

        ACTED_ON_A_STALE_READING
            .assert("an interleaving where the thief claims a victim that has already drained");
        SAW_THE_DRAIN
            .assert("an interleaving where the thief's load sees the drain and stands down");
    }
}
