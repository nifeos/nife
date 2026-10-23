//! **The file service's client windows, as the progenitor hands them to jobs** (milestone 599 (a
//! frame per filesystem client channel), milestone 685 (a job is finished when its memory is
//! back)).
//!
//! Every directory-granted job gets a window of its own: a page of the file service's pool and the
//! service's endpoint badged with the window's number. [`Windows::take`] hands a window out only
//! once its last holder has been reaped and its badge unbound, so no two live jobs share one
//! (confinement claim 24). [`ReapsDue`] names the finished jobs whose reap is on its way. Pure
//! logic, tested on the host; the progenitor (`crates/system_initializer`) owns the capabilities
//! and the binding. See notes/bound-grants-in-the-progenitor.md, including its BUGS.
//!
//! Name: provisional (milestone 685's lane).

/// The most windows a pool can track. The file service's pool has seven
/// (`login_contract::DURABLE_WINDOW` is the last), so this is room, not a target.
pub const MAX_WINDOWS: usize = 16;

/// **A job's label**: the badge the progenitor puts on the job's supervision capability, which the
/// kernel hands back beside the job's death message (DECISIONS §148 (resolves by asking the kernel), milestone 105 (the two forks)) and
/// `job_undertaker` forwards in its "reaped" message. Never `0`, which is "unlabeled".
pub type Label = u64;

/// **Which window the next directory-granted job gets, and who holds each one.**
pub struct Windows {
    /// The first window this pool hands out.
    first: u64,
    /// One past the last.
    end: u64,
    /// Where the rotation looks first.
    next: u64,
    /// `holder[w]` is the label of the job window `w` was last handed to and that has not been
    /// reaped, or `0` when the window is free.
    holder: [Label; MAX_WINDOWS],
    /// Bit `w` set means window `w`'s badge is bound to a grant and must be unbound before reuse
    /// (milestone 606 (a directory walk costs what it does on Linux), ruling D).
    bound: u64,
}

impl Windows {
    /// A pool handing out windows `first..end`.
    ///
    /// # Panics
    ///
    /// If the range is empty or reaches past [`MAX_WINDOWS`]: a pool that could hand out nothing,
    /// or a window this table cannot record, is a wiring mistake, caught where the pool is made.
    #[must_use]
    pub const fn new(first: u64, end: u64) -> Self {
        assert!(first < end && end <= MAX_WINDOWS as u64);
        Self {
            first,
            end,
            next: first,
            holder: [0; MAX_WINDOWS],
            bound: 0,
        }
    }

    /// **The window job `label` gets**, recorded as its holder, or `None` when every window's
    /// last holder is unreaped or its badge is still bound. Round robin from where the last take
    /// stopped, skipping held and bound windows, so reuse is spread over the pool rather than
    /// concentrated on the lowest free one.
    pub fn take(&mut self, label: Label) -> Option<u64> {
        let n = self.end - self.first;
        let w = (0..n)
            .map(|i| self.first + (self.next - self.first + i) % n)
            .find(|&w| self.holder[w as usize] == 0 && !self.is_bound(w))?;
        self.next = if w + 1 < self.end { w + 1 } else { self.first };
        self.holder[w as usize] = label;
        Some(w)
    }

    /// **Job `label` was reaped**: the window it held, if any, is free once it is unbound. Returns
    /// that window, which the caller unbinds now if [`Windows::is_bound`] says so.
    pub fn reaped(&mut self, label: Label) -> Option<u64> {
        if label == 0 {
            return None;
        }
        let w = (self.first..self.end).find(|&w| self.holder[w as usize] == label)?;
        self.holder[w as usize] = 0;
        Some(w)
    }

    /// **Window `w` was taken and no job came to hold it** (the build or the start failed), so it is
    /// free now rather than at a reap that will never come, once the caller has unbound it.
    pub fn release(&mut self, w: u64) {
        if (self.first..self.end).contains(&w) {
            self.holder[w as usize] = 0;
        }
    }

    /// Whether window `w`'s badge is bound to a grant.
    #[must_use]
    pub const fn is_bound(&self, w: u64) -> bool {
        self.bound & (1 << w) != 0
    }

    /// Record whether window `w`'s badge is bound.
    pub fn set_bound(&mut self, w: u64, bound: bool) {
        if bound {
            self.bound |= 1 << w;
        } else {
            self.bound &= !(1 << w);
        }
    }
}

/// How many finished jobs [`ReapsDue`] tracks. A full table only stops the progenitor waiting for
/// the reaps it could not record, so this bounds a liveness improvement, not a safety rule.
pub const MAX_DUE: usize = 8;

/// **The finished jobs whose reap is on its way** (milestone 685): jobs whose whole answer went to
/// the shell's result endpoint, supervised by `job_undertaker`, not yet reaped.
///
/// Waiting for such a reap waits for an event that will come; a job whose output went elsewhere
/// may be waiting on a stage not yet built, so it is not recorded. See
/// notes/bound-grants-in-the-progenitor.md.
///
/// Name: provisional.
pub struct ReapsDue {
    labels: [Label; MAX_DUE],
}

impl ReapsDue {
    /// Nothing due.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            labels: [0; MAX_DUE],
        }
    }

    /// Record that job `label`'s reap is due. `false` when the table is full (or `label` is `0`),
    /// and then nothing will wait for it.
    pub fn expect(&mut self, label: Label) -> bool {
        if label == 0 {
            return false;
        }
        match self.labels.iter_mut().find(|l| **l == 0) {
            Some(slot) => {
                *slot = label;
                true
            }
            None => false,
        }
    }

    /// Job `label` was reaped.
    pub fn reaped(&mut self, label: Label) {
        if label == 0 {
            return;
        }
        for l in &mut self.labels {
            if *l == label {
                *l = 0;
            }
        }
    }

    /// Whether any reap is still due: the condition under which a short pool waits rather than
    /// refuses.
    #[must_use]
    pub fn any(&self) -> bool {
        self.labels.iter().any(|&l| l != 0)
    }
}

impl Default for ReapsDue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec::Vec;

    use super::*;

    /// The file service's pool as the progenitor makes it: windows 1 to 6 (0 is the boot's
    /// long-lived clients', 7 is `login`'s durable sessions').
    fn pool() -> Windows {
        Windows::new(1, 7)
    }

    /// **`take` never hands out a window whose last holder is unreaped** (confinement claim 24's
    /// fix, milestone 685). Six jobs hold all six windows and none has been reaped, so a seventh
    /// gets nothing; once one is reaped, its window and only its window comes back.
    ///
    /// Falsification: replayable `crates/grant_plan/falsifications/job_windows.tests.take_never_hands_out_a_window_whose_last_holder_is_unreaped.patch`
    #[test]
    fn take_never_hands_out_a_window_whose_last_holder_is_unreaped() {
        let mut p = pool();
        let held: Vec<u64> = (1..=6)
            .map(|job| p.take(job).expect("six windows for six jobs"))
            .collect();
        assert_eq!(
            p.take(7),
            None,
            "a seventh job got a window while all six holders were unreaped"
        );
        assert_eq!(
            p.reaped(3),
            Some(held[2]),
            "job 3's window did not come back"
        );
        assert_eq!(
            p.take(8),
            Some(held[2]),
            "the reaped job's window was not handed out again"
        );
        assert_eq!(p.take(9), None, "a window came back that no reap released");
    }

    /// **Rotation still spreads reuse, and skips the windows still held.** Jobs 1 and 2 hold the
    /// first two windows, job 1 is reaped, and four more jobs come: none of them may get job 2's.
    #[test]
    fn take_skips_a_held_window_on_its_way_round() {
        let mut p = pool();
        let a = p.take(1).unwrap();
        let b = p.take(2).unwrap();
        p.reaped(1);
        for job in 3..=6 {
            let w = p.take(job).expect("a free window remained");
            assert_ne!(w, b, "job {job} got the window job 2 still holds");
        }
        let w = p.take(7).expect("job 1's window is free");
        assert_eq!(w, a);
    }

    /// **A window whose job never started is free at once**, and a reap of a label nobody holds, or
    /// of `0`, frees nothing.
    #[test]
    fn release_frees_and_a_stranger_reap_does_not() {
        let mut p = Windows::new(1, 2);
        let w = p.take(1).unwrap();
        assert_eq!(p.reaped(0), None);
        assert_eq!(p.reaped(99), None);
        assert_eq!(p.take(2), None, "the one window is still held by job 1");
        p.release(w);
        assert_eq!(p.take(2), Some(w));
    }

    /// **A released window is not the next one handed out**: the rotation moved past it, so a
    /// job that failed to start does not hand the next job the same window at once.
    #[test]
    fn a_released_window_waits_its_turn() {
        let mut p = pool();
        let w = p.take(1).unwrap();
        p.release(w);
        assert_eq!(p.take(2), Some(w + 1));
    }

    /// **A window's bound bit is its own**: setting one leaves the others, and clearing it clears
    /// only it.
    #[test]
    fn each_window_is_bound_and_unbound_on_its_own() {
        let mut p = pool();
        assert!(!p.is_bound(3));
        p.set_bound(3, true);
        p.set_bound(5, true);
        assert!(p.is_bound(3) && p.is_bound(5));
        assert!(!p.is_bound(2) && !p.is_bound(4));
        p.set_bound(3, false);
        assert!(!p.is_bound(3));
        assert!(p.is_bound(5), "clearing window 3 cleared window 5");
    }

    /// **A bound window comes back only once it is unbound** (milestone 606, ruling D's "UNBIND at
    /// reap"). Job 1's window is bound and job 1 is reaped: the next take must not hand that window
    /// out, because the only `UNBIND` left would be one at reuse, after the window had a new
    /// holder. Once the reap's `UNBIND` has cleared the bit, the window is free.
    ///
    /// Falsification: replayable `crates/grant_plan/falsifications/job_windows.tests.take_never_hands_out_a_bound_window.patch`
    #[test]
    fn take_never_hands_out_a_bound_window() {
        let mut p = Windows::new(1, 2);
        let w = p.take(1).unwrap();
        p.set_bound(w, true);
        assert_eq!(p.reaped(1), Some(w), "job 1's reap did not name its window");
        assert!(
            p.is_bound(w),
            "the reap cleared the bound bit without an UNBIND"
        );
        assert_eq!(
            p.take(2),
            None,
            "a bound window was handed out: the UNBIND would run at reuse, not at reap"
        );
        p.set_bound(w, false);
        assert_eq!(p.take(2), Some(w), "the unbound window did not come back");
        p.set_bound(w, true);
        p.release(w);
        assert_eq!(
            p.take(3),
            None,
            "a released window was handed out still bound"
        );
    }

    /// **A due reap is forgotten when it arrives, and only then.**
    #[test]
    fn a_due_reap_is_cleared_by_its_own_reap() {
        let mut d = ReapsDue::new();
        assert!(!d.any());
        assert!(d.expect(4));
        assert!(
            !d.expect(0),
            "label 0 is unlabeled and nothing waits for it"
        );
        d.reaped(5);
        assert!(d.any(), "another job's reap cleared job 4's");
        d.reaped(4);
        assert!(!d.any());
        for job in 1..=MAX_DUE as u64 {
            assert!(d.expect(job));
        }
        assert!(!d.expect(99), "a full table said it recorded a ninth");
    }
}
