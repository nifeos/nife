//! The timer's decision core for §147 (a timer a userspace service cannot hold): one armed
//! deadline per timer, the cached earliest deadline the tick compares against, and the expiry walk.
//!
//! The cached earliest is never later than any armed deadline; it may be earlier, which costs one
//! walk that fires nothing. See `notes/inter-process-communication.md` ("The timer"). The module
//! name is provisional.

//! # Examples
//!
//! ```
//! use inter_process_communication::timer::{Arm, Timer, earliest, lowered, next_due, NEVER};
//!
//! // Two timers, each signalling a different (notification, bits) target.
//! let mut a: Timer<(u64, u64)> = Timer::new();
//! let mut b: Timer<(u64, u64)> = Timer::new();
//! let mut cached = NEVER;
//!
//! // Arm both at time 100. A deadline in the future is pending; the cache moves down to meet it.
//! assert_eq!(a.arm(150, (7, 0b01), 100), Arm::Pending);
//! cached = lowered(cached, 150);
//! assert_eq!(b.arm(120, (7, 0b10), 100), Arm::Pending);
//! cached = lowered(cached, 120);
//! assert_eq!(cached, 120);
//!
//! // A deadline already passed fires at once instead of waiting for a tick.
//! let mut c: Timer<(u64, u64)> = Timer::new();
//! assert_eq!(c.arm(90, (8, 1), 100), Arm::DueNow((8, 1)));
//! assert!(!c.is_armed());
//!
//! // The tick at 130 is due (130 >= 120). The walk fires b and only b, then recomputes the cache.
//! let mut fired = Vec::new();
//! while let Some(t) = next_due([&mut a, &mut b].into_iter(), 130) {
//!     fired.push(t);
//! }
//! assert_eq!(fired, vec![(7, 0b10)]);
//! cached = earliest([&a, &b].into_iter());
//! assert_eq!(cached, 150);
//!
//! // Re-arming replaces the old deadline: the old one can never fire.
//! a.arm(500, (7, 0b01), 130);
//! assert_eq!(a.expire(150), None);
//! // And a cancel reports whether it stopped anything.
//! assert!(a.cancel());
//! assert!(!a.cancel());
//! ```

/// "No deadline": the cached earliest when nothing is armed. A timer armed at exactly this value is
/// pending forever (until cancelled), which is the same thing said the other way round.
pub const NEVER: u64 = u64::MAX;

/// What [`Timer::arm`] decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arm<T> {
    /// The deadline is in the future: armed, and the caller lowers the cache with [`lowered`].
    Pending,
    /// The deadline is at or before `now`: nothing was armed, and the caller signals `T` at once
    /// rather than a tick late.
    DueNow(T),
}

/// **One timer**: at most one pending deadline, and what to signal when it passes. `T` is the
/// kernel's (notification name, bits) pair; this module never looks inside it.
#[derive(Debug, Clone, Copy)]
pub struct Timer<T: Copy> {
    armed: Option<(u64, T)>,
}

impl<T: Copy> Default for Timer<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Copy> Timer<T> {
    /// A timer with nothing armed.
    pub const fn new() -> Self {
        Timer { armed: None }
    }

    /// Arm for `deadline`, replacing whatever was pending; a replaced deadline can never fire.
    /// Returns [`Arm::DueNow`] without arming when `deadline` is at or before `now`.
    pub fn arm(&mut self, deadline: u64, target: T, now: u64) -> Arm<T> {
        if deadline <= now {
            self.armed = None;
            Arm::DueNow(target)
        } else {
            self.armed = Some((deadline, target));
            Arm::Pending
        }
    }

    /// **Disarm.** `true` if a deadline was pending and now never will fire; `false` if nothing
    /// was armed (never armed, already fired, or already cancelled). A caller that gets `false`
    /// back knows the signal, if any, has already been sent.
    pub fn cancel(&mut self) -> bool {
        self.armed.take().is_some()
    }

    /// **Fire if due**: `Some(target)` and disarmed when the deadline is at or before `now`,
    /// otherwise `None` and untouched. Fires at most once per arm.
    pub fn expire(&mut self, now: u64) -> Option<T> {
        match self.armed {
            Some((deadline, target)) if deadline <= now => {
                self.armed = None;
                Some(target)
            }
            _ => None,
        }
    }

    /// The pending deadline, or [`NEVER`].
    pub fn deadline(&self) -> u64 {
        self.armed.map_or(NEVER, |(d, _)| d)
    }

    /// Whether a deadline is pending.
    pub fn is_armed(&self) -> bool {
        self.armed.is_some()
    }
}

/// **The cache after arming `deadline`**: never raised, only lowered. Called under the same lock
/// as the arm, so the cache and the timers never disagree for anyone who holds it.
pub fn lowered(cached: u64, deadline: u64) -> u64 {
    cached.min(deadline)
}

/// **Whether the tick must walk.** The one comparison per idle tick `notes/timed-wait.md` priced.
pub fn is_due(cached: u64, now: u64) -> bool {
    now >= cached
}

/// One step of the expiry walk: fire the first due timer and return its target, or `None` when
/// none is due. The kernel calls it in a loop and signals each target between calls, so no buffer
/// bounds how many fire in one tick.
pub fn next_due<'a, T: Copy + 'a>(
    timers: impl Iterator<Item = &'a mut Timer<T>>,
    now: u64,
) -> Option<T> {
    for t in timers {
        if let Some(target) = t.expire(now) {
            return Some(target);
        }
    }
    None
}

/// **The cache, recomputed exactly**: the earliest pending deadline, or [`NEVER`]. Run after a
/// walk, which is the only time the cache is raised.
pub fn earliest<'a, T: Copy + 'a>(timers: impl Iterator<Item = &'a Timer<T>>) -> u64 {
    timers.fold(NEVER, |e, t| e.min(t.deadline()))
}

/// Machine-checked proofs of the timer arithmetic, over three timers: enough for "the first due
/// timer is not the only due timer" and "a due timer sits between two that are not", which are the
/// orders a walk can get wrong.
#[cfg(kani)]
mod verification {
    use super::*;

    type T3 = [Timer<u8>; 3];

    /// Three timers in an arbitrary state: each armed at a symbolic deadline or not, with a target
    /// equal to its index so a firing says which timer it was.
    fn any_timers() -> T3 {
        let mut ts = [Timer::new(), Timer::new(), Timer::new()];
        for (i, t) in ts.iter_mut().enumerate() {
            if kani::any() {
                t.armed = Some((kani::any(), i as u8));
            }
        }
        ts
    }

    fn cache_is_sound(ts: &T3, cached: u64) -> bool {
        ts.iter().all(|t| !t.is_armed() || cached <= t.deadline())
    }

    /// **The cache is never later than an armed deadline**, across every operation the kernel
    /// performs on it: arm (and lower), cancel (and leave it), or walk (and recompute), starting
    /// from any sound cache, which includes a stale-low one a cancel left behind.
    /// Falsification: replayable `crates/inter_process_communication/falsifications/timer.verification.the_cache_is_never_later_than_an_armed_deadline.patch`
    #[kani::proof]
    #[kani::unwind(5)]
    fn the_cache_is_never_later_than_an_armed_deadline() {
        let mut ts = any_timers();
        let mut cached: u64 = kani::any();
        kani::assume(cache_is_sound(&ts, cached));
        let i: usize = kani::any();
        kani::assume(i < 3);
        let now: u64 = kani::any();
        match kani::any::<u8>() {
            0 => {
                let deadline: u64 = kani::any();
                if ts[i].arm(deadline, i as u8, now) == Arm::Pending {
                    cached = lowered(cached, deadline);
                }
            }
            1 => {
                ts[i].cancel();
            }
            _ => {
                if is_due(cached, now) {
                    while next_due(ts.iter_mut(), now).is_some() {}
                    cached = earliest(ts.iter());
                }
            }
        }
        assert!(cache_is_sound(&ts, cached));
    }

    /// **A walk fires exactly the due timers, each once, and leaves none due.** A timer whose
    /// deadline is at or before `now` fires; one after it is untouched, deadline and all. And a
    /// tick the cache calls not due really has nothing due, so skipping the walk skips nothing.
    /// Falsification: replayable `crates/inter_process_communication/falsifications/timer.verification.a_walk_fires_exactly_the_due_timers.patch`
    #[kani::proof]
    #[kani::unwind(5)]
    fn a_walk_fires_exactly_the_due_timers() {
        let mut ts = any_timers();
        let before = ts;
        let now: u64 = kani::any();
        let cached = earliest(ts.iter());
        let mut fired = [false; 3];
        let mut steps = 0;
        while let Some(target) = next_due(ts.iter_mut(), now) {
            let k = target as usize;
            assert!(!fired[k], "a timer fired twice in one walk");
            fired[k] = true;
            steps += 1;
            assert!(steps <= 3);
        }
        for k in 0..3 {
            let due = before[k].is_armed() && before[k].deadline() <= now;
            assert_eq!(fired[k], due);
            if !due {
                assert_eq!(ts[k].deadline(), before[k].deadline());
            }
            assert!(!(ts[k].is_armed() && ts[k].deadline() <= now));
        }
        if !is_due(cached, now) {
            assert!(!fired.iter().any(|&f| f));
        }
    }

    /// **A cancelled or re-armed timer never fires its old deadline.** Arm for `d1` with target
    /// 1, then either cancel or re-arm for `d2` with target 2; at any later `now` the timer yields
    /// target 1 never, and target 2 exactly when `d2` has passed.
    /// Falsification: replayable `crates/inter_process_communication/falsifications/timer.verification.a_cancelled_or_rearmed_timer_never_fires_stale.patch`
    #[kani::proof]
    fn a_cancelled_or_rearmed_timer_never_fires_stale() {
        let mut t: Timer<u8> = Timer::new();
        let (d1, t0): (u64, u64) = (kani::any(), kani::any());
        kani::assume(d1 > t0);
        assert_eq!(t.arm(d1, 1, t0), Arm::Pending);
        let rearm: bool = kani::any();
        let d2: u64 = kani::any();
        let t1: u64 = kani::any();
        let arm2 = if rearm {
            Some(t.arm(d2, 2, t1))
        } else {
            assert!(t.cancel());
            None
        };
        let now: u64 = kani::any();
        let got = t.expire(now);
        assert!(got != Some(1));
        match arm2 {
            Some(Arm::Pending) => assert_eq!(got.is_some(), d2 <= now),
            Some(Arm::DueNow(x)) => {
                assert_eq!(x, 2);
                assert!(d2 <= t1);
                assert_eq!(got, None);
            }
            None => assert_eq!(got, None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_deadline_exactly_now_is_due_now_rather_than_a_tick_late() {
        let mut t: Timer<u8> = Timer::new();
        assert_eq!(t.arm(100, 9, 100), Arm::DueNow(9));
        assert!(!t.is_armed());
        assert_eq!(t.deadline(), NEVER);
    }

    #[test]
    fn a_stale_low_cache_costs_one_walk_and_is_then_exact() {
        let mut a: Timer<u8> = Timer::new();
        let mut cached = NEVER;
        a.arm(50, 1, 0);
        cached = lowered(cached, 50);
        assert!(a.cancel());
        a.arm(80, 1, 0);
        cached = lowered(cached, 80);
        assert_eq!(cached, 50, "a cancel does not raise the cache");
        assert!(is_due(cached, 60));
        assert_eq!(next_due([&mut a].into_iter(), 60), None);
        cached = earliest([&a].into_iter());
        assert_eq!(cached, 80);
        assert!(!is_due(cached, 60));
    }

    #[test]
    fn several_timers_due_in_one_tick_all_fire() {
        let mut ts = [Timer::new(), Timer::new(), Timer::new()];
        ts[0].arm(10, 0u8, 0);
        ts[1].arm(30, 1, 0);
        ts[2].arm(20, 2, 0);
        let mut fired = Vec::new();
        while let Some(t) = next_due(ts.iter_mut(), 25) {
            fired.push(t);
        }
        assert_eq!(fired, vec![0, 2]);
        assert_eq!(earliest(ts.iter()), 30);
    }

    /// The pending-ness accessor follows the slot: armed after a future arm, not after a cancel or
    /// after the deadline fires.
    #[test]
    fn is_armed_follows_the_slot() {
        let mut t: Timer<u8> = Timer::new();
        assert!(!t.is_armed());
        assert_eq!(t.arm(100, 1, 0), Arm::Pending);
        assert!(t.is_armed());
        assert_eq!(t.expire(100), Some(1));
        assert!(!t.is_armed());
        t.arm(200, 2, 0);
        assert!(t.cancel());
        assert!(!t.is_armed());
    }
}
