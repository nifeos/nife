//! **The registered timetable**: every entry with its decision, and the firing state. Arming,
//! reading the wall clock, replacing a document while keeping its beats, and the fire arithmetic
//! the Kani proofs in `proofs.rs` cover.
//!
//! Moved out of the crate root unchanged by milestone 877 (the timetable crate root is split along
//! its seams), as §294 (the timetable crate root splits into private modules) lays out; the root
//! re-exports every public item at its old path.
//!
//! Name: provisional (milestone 877, 2026-10-12), as are its sibling modules' names.

use grant_plan::{Endowment, Manifest};

use crate::admission::{admit, admit_installed_row};
use crate::{Admission, Document, Entry, Held, MAX_ENTRIES, Schedule, Unbacked, recurrence};

/// One registered entry: what was written, what registration decided, and when it next fires.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Row<'a> {
    /// The document line this came from.
    pub entry: Entry<'a>,
    /// The decision.
    pub admission: Admission,
    /// The monotonic nanosecond reading at which this next fires. `u64::MAX` for anything that
    /// never will, which is every refusal and every [`Schedule::AtBoot`] that already went.
    /// Unused by a [`Schedule::Calendar`] row, which keeps wall time in `wall_next` instead.
    next: u64,
    /// A calendar row's next occurrence, in UTC minutes: [`UNARMED`] until the clock is known,
    /// [`SPENT`] once `through` has passed.
    wall_next: i64,
    /// The wall-clock minute a calendar row last fired at, or [`UNSTAMPED`]. After a backward step
    /// the next fire is the first occurrence after both the clock and this, so nothing fires
    /// twice (S3). A clock `SET` clears it.
    stamp: i64,
}

/// A calendar row not yet armed against a known clock.
pub const UNARMED: i64 = i64::MIN;
/// A calendar row with no occurrence left.
pub const SPENT: i64 = i64::MAX;
/// A calendar row that has never fired, or whose stamp a clock `SET` cleared.
pub const UNSTAMPED: i64 = i64::MIN;

/// **What the wall clock says, as the timetable reads it once per pass** (§43 (reading the clock
/// is a page)). `None` wherever it is used means the time is unknown or no clock is held, and a
/// calendar row is then dormant.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WallReading {
    /// Whole UTC minutes since the epoch.
    pub minute: i64,
    /// The clock page's publication count; a change is a step.
    pub generation: u64,
    /// The publication in force was an operator's `SET`, not an accepted `SYNCED` proposal.
    pub set: bool,
}

impl<'a> Row<'a> {
    /// The endowment this row's child will be granted, or `None` if it never fires.
    pub fn endowment(&self) -> Option<Endowment> {
        match self.admission {
            Admission::Fires(e) => Some(e),
            _ => None,
        }
    }

    /// The monotonic reading at which this next fires, or `None` if it never will.
    pub fn next_fire(&self) -> Option<u64> {
        (self.next != NEVER).then_some(self.next)
    }
}

/// The "never fires again" deadline. `u64::MAX` rather than an `Option` inside the hot loop,
/// because [`Registry::due`] compares it against a reading every pass and a branch that is always
/// taken is cheaper to read than one that is sometimes there.
const NEVER: u64 = u64::MAX;

/// The registered timetable: every entry with its decision, and the firing state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Registry<'a> {
    rows: [Row<'a>; MAX_ENTRIES],
    n: usize,
    /// What it was registered against, so the plan can say what each job holds.
    pub(super) held: Held,
    /// The wall-clock minute at the last [`Registry::observe`], or `None` while it is unknown.
    wall: Option<i64>,
    /// The clock page generation the calendar rows were last armed against.
    generation: u64,
}

/// No generation observed yet, so the next known reading arms every calendar row.
const NO_GENERATION: u64 = u64::MAX;

impl<'a> Registry<'a> {
    /// **Check every entry against its program's manifest and against what this scheduler holds.**
    ///
    /// This is the security boundary milestone 129's block names. Whoever can put a line in this
    /// document can make that line's grants periodic and **nothing more**: the endowment is planned
    /// here, by `grant_plan`, out of the program's own published manifest, and an authority the
    /// scheduler does not hold cannot appear in it however the line is written.
    ///
    /// It fires nothing and touches no clock. [`arm`](Registry::arm) is a separate call so that a
    /// caller can print the whole plan, decide it is wrong, and never arm anything.
    ///
    /// # Examples
    ///
    /// ```
    /// # use timetable::{Admission, Held, Registry, Unbacked};
    /// let doc = timetable::parse(
    ///     "every 5s  least_authority_demo 7\n\
    ///      every 5s  memory_grant_depleter\n\
    ///      every 5s  date\n",
    /// ).unwrap();
    /// let reg = Registry::register(&doc, Held::default());
    ///
    /// // Planned, and the endowment is the whole of what the child will hold.
    /// let e = reg.rows()[0].endowment().expect("least_authority_demo 7 is a legal line");
    /// assert_eq!(e.arg, 7);
    /// assert_eq!(e.mem_pages, 0);
    ///
    /// // The manifest refuses the line, exactly as the prompt would.
    /// assert!(matches!(reg.rows()[1].admission, Admission::Refused(_)));
    ///
    /// // And the line Unix cannot refuse: legal, and unbacked by this scheduler.
    /// assert_eq!(
    ///     reg.rows()[2].admission,
    ///     Admission::Unbacked(Unbacked::Clock),
    /// );
    /// ```
    pub fn register(doc: &Document<'a>, held: Held) -> Self {
        let mut rows = [Row {
            entry: Entry {
                line: 0,
                schedule: Schedule::AtBoot,
                command: b"",
            },
            admission: Admission::Unbacked(Unbacked::Clock),
            next: NEVER,
            wall_next: UNARMED,
            stamp: UNSTAMPED,
        }; MAX_ENTRIES];
        let mut n = 0usize;
        for &entry in doc.entries() {
            // A calendar line needs the wall clock before its program's manifest is even asked:
            // keeping a time of day is this timetable's own need, not the child's.
            let admission = match entry.schedule {
                Schedule::Calendar(_) if !held.clock => Admission::Unbacked(Unbacked::WallClock),
                _ => admit(entry.command, held),
            };
            rows[n] = Row {
                entry,
                admission,
                next: NEVER,
                wall_next: UNARMED,
                stamp: UNSTAMPED,
            };
            n += 1;
        }
        Registry {
            rows,
            n,
            held,
            wall: None,
            generation: NO_GENERATION,
        }
    }

    /// **[`register`](Registry::register), for a document whose programs are installed** (milestone
    /// 152 (durable delegation), Fork 8 ruled D by calef on 2026-09-27, on #1377): each entry is
    /// planned against the manifest its program's bytes carry, which the caller read from the live
    /// activation generation and passes as `manifests[i]` for entry `i`. That is how the prompt binds
    /// an installed program's line (`grant_plan::plan_against` under `grant_plan::IMAGE_ROW`), so a
    /// line means at the prompt and in a schedule the same thing.
    ///
    /// The caller answers first whether each program is installed and [`schedulable`](crate::schedulable) at all; this
    /// only plans. Name: provisional.
    pub fn register_installed(
        doc: &Document<'a>,
        held: Held,
        manifests: &[Manifest; MAX_ENTRIES],
    ) -> Self {
        let mut reg = Registry::register(doc, held);
        for (i, row) in reg.rows.iter_mut().take(reg.n).enumerate() {
            row.admission = admit_installed_row(&row.entry, manifests[i], held);
        }
        reg
    }

    /// **Row `i`'s line, planned again against the manifest its program carries now** (store mode,
    /// milestone 152's Fork 8 D as calef amended it on 2026-09-27, #1377): what a fire of an
    /// installed program builds from, since the program may have been upgraded since the document
    /// was registered. Anything but [`Admission::Fires`] means the beat does not fire, because the
    /// new version asks for more than the line grants or this scheduler holds. Name: provisional.
    pub fn plan_at_fire(&self, i: usize, m: Manifest) -> Admission {
        admit_installed_row(&self.rows[i].entry, m, self.held)
    }

    /// What this registry was registered against.
    pub fn held(&self) -> Held {
        self.held
    }

    /// The registered rows, in document order.
    pub fn rows(&self) -> &[Row<'a>] {
        &self.rows[..self.n]
    }

    /// How many entries will actually fire.
    pub fn admitted(&self) -> usize {
        self.rows()
            .iter()
            .filter(|r| r.endowment().is_some())
            .count()
    }

    /// **The programs this timetable will ever build**, as a bitmask over `Program::id`.
    ///
    /// The point of it being computable at registration: reading the printed plan tells you the
    /// complete set of programs this process will ever start, and a Kani-free host test can hold
    /// that against the document. It is what makes the honest caveat in
    /// `components/src/timetable.rs`'s `BUGS` (the program is handed the whole initial ramdisk
    /// archive, which is wider than its plan) a *stated* residual rather than an unbounded one.
    pub fn programs(&self) -> u64 {
        let mut set = 0u64;
        for r in self.rows() {
            if let Some(e) = r.endowment() {
                set |= 1 << e.program.id();
            }
        }
        set
    }

    /// Arm every admitted row against a monotonic reading. `at-boot` entries become due
    /// immediately; interval entries become due one interval later.
    ///
    /// **One interval later, not immediately**, and that is a decision rather than an
    /// implementation detail: `every 30s` said at a prompt means "thirty seconds from now", and a
    /// scheduler that also ran everything once at startup would make a boot the busiest moment the
    /// machine ever has. An entry that wants both says both, on two lines, which is legible.
    pub fn arm(&mut self, now: u64) {
        for r in self.rows.iter_mut().take(self.n) {
            if r.endowment().is_none() {
                continue;
            }
            r.next = match r.entry.schedule {
                Schedule::AtBoot => now,
                Schedule::Every(p) => now.saturating_add(p),
                Schedule::Calendar(_) => NEVER,
            };
            r.wall_next = UNARMED;
        }
        // Calendar rows arm against the clock at the next `observe`, not here: arming needs a known
        // wall time, and `arm` is given only the counter.
        self.generation = NO_GENERATION;
    }

    /// **Read the wall clock into the registry**, once per pass, before [`due`](Registry::due).
    ///
    /// This is S3 with the `SET` fix, as ruled with G5 (notes/scheduled-execution/calendar-grammar-g5.md):
    ///
    /// - An unknown clock (`None`) leaves every calendar row dormant: nothing is armed and nothing
    ///   fires, because a machine that does not know the date should not guess it.
    /// - A new generation is a step, or the first known reading. Every calendar row is re-armed to
    ///   its first occurrence after both the clock and its stamp, so a forward step fires a row at
    ///   most once and a backward step never fires one twice.
    /// - A generation published by an operator's `SET` first clears the stamps: they were taken on a
    ///   clock the operator has just called wrong. A `SYNCED` step keeps them.
    pub fn observe(&mut self, reading: Option<WallReading>) {
        let Some(r) = reading else {
            self.wall = None;
            self.generation = NO_GENERATION;
            for row in self.rows.iter_mut().take(self.n) {
                row.wall_next = UNARMED;
            }
            return;
        };
        self.wall = Some(r.minute);
        if r.generation == self.generation {
            return;
        }
        self.generation = r.generation;
        for row in self.rows.iter_mut().take(self.n) {
            let Schedule::Calendar(rule) = row.entry.schedule else {
                continue;
            };
            if row.endowment().is_none() {
                continue;
            }
            if r.set {
                row.stamp = UNSTAMPED;
            }
            // The earlier of what was armed and the first occurrence after both the clock and the
            // stamp. A forward step leaves the armed occurrence behind the clock, so it fires once,
            // however many the step covered; a backward step, or a SET that cleared the stamp,
            // brings the next one forward, and the stamp keeps anything already fired from firing
            // again.
            let fresh = recurrence::next(&rule, r.minute.max(row.stamp)).unwrap_or(SPENT);
            row.wall_next = if row.wall_next == UNARMED {
                fresh
            } else {
                row.wall_next.min(fresh)
            };
        }
    }

    /// **Arm this registry as the replacement for `old`**, which is the third sub-ruling of §222 (who holds a user's schedule): an
    /// entry whose text is byte-identical to one in force keeps that entry's beat, and every other
    /// admitted entry arms fresh against `now`, exactly as [`arm`](Registry::arm) would.
    ///
    /// Byte-identical means the schedule and the command both match. An `at-boot` entry that has
    /// already fired therefore stays fired: resending a document is not a reboot. Duplicates pair
    /// off in document order, each old row lending its beat at most once, so a document that adds
    /// a second copy of a line gets one kept beat and one fresh one.
    ///
    /// Returns a mask, bit `i` set when entry `i` kept its beat. The rows it pairs are both
    /// admitted; a line that was refused before and is admitted now has no beat to keep.
    pub fn arm_after(&mut self, old: &Registry<'_>, now: u64) -> u8 {
        self.arm(now);
        let mut lent = 0u8;
        let mut kept = 0u8;
        for i in 0..self.n {
            if self.rows[i].endowment().is_none() {
                continue;
            }
            let mine = self.rows[i].entry;
            for (j, theirs) in old.rows().iter().enumerate() {
                if lent & (1 << j) != 0 || theirs.endowment().is_none() {
                    continue;
                }
                if theirs.entry.schedule == mine.schedule && theirs.entry.command == mine.command {
                    self.rows[i].next = theirs.next;
                    // A calendar row carries its stamp; `wall_next` re-arms from it at the next
                    // `observe`, since `arm` above reset the generation.
                    self.rows[i].stamp = theirs.stamp;
                    lent |= 1 << j;
                    kept |= 1 << i;
                    break;
                }
            }
        }
        kept
    }

    /// **Take one row that is due at `now`, advancing it past `now`.** `None` when nothing is due.
    ///
    /// Call it in a loop until it answers `None`: several entries can come due in one pass, and
    /// answering one at a time is what lets the caller fire each one (which takes real time, and
    /// can block waiting for memory) without holding a borrow of the whole table.
    ///
    /// The row it returns has **already been advanced**, so a caller that fails to build the child
    /// has missed that occurrence rather than retrying it forever. That is the same skip-rather-
    /// than-catch-up rule [`next_after`] states, applied to the failure path, and for the same
    /// reason: an entry that cannot be built now will not be buildable a microsecond from now, and
    /// a retry loop against a full budget is a spin with extra steps.
    pub fn due(&mut self, now: u64) -> Option<usize> {
        let wall = self.wall;
        for i in 0..self.n {
            let r = &mut self.rows[i];
            if let Schedule::Calendar(rule) = r.entry.schedule {
                // Due when the wall clock has reached the armed occurrence. Stamped with the clock's
                // minute rather than the occurrence: no occurrence lies between the two, so the
                // backward-step rule reads the same, and a forward step that covered several
                // occurrences fires once.
                let Some(w) = wall else { continue };
                if r.wall_next == UNARMED || r.wall_next == SPENT || w < r.wall_next {
                    continue;
                }
                r.stamp = w;
                r.wall_next = recurrence::next(&rule, w).unwrap_or(SPENT);
                return Some(i);
            }
            if r.next > now {
                continue;
            }
            r.next = match r.entry.schedule {
                Schedule::AtBoot => NEVER,
                Schedule::Every(p) => next_after(r.next, p, now),
                Schedule::Calendar(_) => NEVER, // handled above; never reaches here
            };
            return Some(i);
        }
        None
    }

    /// The earliest reading at which anything fires, or `None` if nothing ever will.
    ///
    /// What a timed wait would block until, if this kernel had one. It does not (milestone 106 is
    /// `NOT-STARTED` and gated on a decision), so `components/src/timetable.rs` yield-polls instead and
    /// this is the number that says how long it will spin. Computing it anyway is deliberate: when
    /// the timed wait lands, the scheduler's loop changes by one line and this function is already
    /// the argument to it.
    pub fn next_deadline(&self) -> Option<u64> {
        self.rows().iter().filter_map(|r| r.next_fire()).min()
    }
}

// ===============================================================================================
// The arithmetic.
// ===============================================================================================

/// **The next fire after `now`, given the one that was scheduled at `prev` and a period.**
///
/// Three properties, and the third is the decision:
///
/// - it is **strictly after `now`**, so a fire never repeats within one pass of a polling loop;
/// - it keeps the **phase**: the result is congruent to `prev` modulo `period`, so an entry that
///   drifted late because the machine was busy comes back onto its original beat instead of
///   inheriting the delay forever;
/// - it **skips rather than catches up**. If the scheduler was away for an hour, an entry that
///   should have fired two hundred times fires **once** and then resumes. Vixie cron does the same
///   thing for the same reason, and the reason is worth writing down: catching up turns a stall
///   into a stampede, and a housekeeping job that runs two hundred times back to back on a machine
///   that was already struggling is how a slow morning becomes an outage. The cost is stated rather
///   than hidden: occurrences are genuinely lost, and an entry whose work must not be skipped
///   wants a durable queue rather than a scheduler.
///
/// `period == 0` answers [`u64::MAX`] (never), which no caller reaches because [`parse`](crate::parse) refuses a
/// zero interval; it is here so the function is total rather than a panic waiting for a caller that
/// did not read this line.
///
/// Three of those four are machine-checked; the phase one is host-tested, and `src/proofs.rs` says
/// why in the place a reader of the proofs would look for it.
///
/// # Examples
///
/// ```
/// # use timetable::next_after;
/// // The ordinary case: one period on.
/// assert_eq!(next_after(100, 10, 100), 110);
///
/// // Late by eight periods, and it does not fire eight times to make up.
/// assert_eq!(next_after(100, 10, 185), 190);
/// assert_eq!(next_after(100, 10, 190), 200);
///
/// // The phase survives the skip: every result is congruent to `prev` mod `period`.
/// assert_eq!(next_after(103, 10, 1_000) % 10, 3);
/// ```
pub const fn next_after(prev: u64, period: u64, now: u64) -> u64 {
    if period == 0 {
        return NEVER;
    }
    let first = prev.saturating_add(period);
    if first > now {
        return first;
    }
    // Past the first candidate, so snap `now` back onto the beat and add one period. Reaching the
    // answer this way rather than by counting the periods that went by (`first + (gap / period + 1)
    // * period`) is what makes it provable in seconds instead of hours: **there is no
    // multiplication here**, and a 64-by-64 multiplier is an enormous thing to hand a SAT solver.
    // The counting form was written first, and Kani was still grinding on one harness after ten
    // minutes; this form proves in well under one. Same answer, and the tests that pin the answer
    // did not change.
    //
    // `now >= first > prev`, so the subtraction cannot wrap, and `r < period`, so `now - r` cannot
    // either.
    let r = (now - prev) % period;
    (now - r).saturating_add(period)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::with_clock;
    use crate::{NANOS_PER_SEC, parse};

    #[test]
    fn arming_puts_at_boot_first_and_an_interval_one_interval_out() {
        let doc =
            parse("every 30s least_authority_demo 7\nat-boot least_authority_demo 3\n").unwrap();
        let mut reg = Registry::register(&doc, Held::default());
        reg.arm(1_000);
        assert_eq!(reg.rows()[0].next_fire(), Some(1_000 + 30 * NANOS_PER_SEC));
        assert_eq!(reg.rows()[1].next_fire(), Some(1_000));
        assert_eq!(reg.next_deadline(), Some(1_000));
    }

    #[test]
    fn an_at_boot_entry_fires_once_and_never_again() {
        let doc = parse("at-boot least_authority_demo 3\n").unwrap();
        let mut reg = Registry::register(&doc, Held::default());
        reg.arm(0);
        assert_eq!(reg.due(0), Some(0));
        assert_eq!(reg.due(0), None);
        assert_eq!(reg.due(u64::MAX - 1), None);
        assert_eq!(reg.next_deadline(), None);
    }

    #[test]
    fn a_refused_entry_is_never_due_however_long_you_wait() {
        let doc = parse("every 1ms date\nevery 1ms memory_grant_depleter\n").unwrap();
        let mut reg = Registry::register(&doc, Held::default());
        reg.arm(0);
        for t in 0..1_000 {
            assert_eq!(reg.due(t * NANOS_PER_SEC), None, "at t={t}");
        }
        assert_eq!(reg.next_deadline(), None);
    }

    #[test]
    fn several_entries_can_come_due_in_one_pass() {
        let doc = parse("every 10ms least_authority_demo 1\nevery 10ms least_authority_demo 2\nat-boot least_authority_demo 3\n").unwrap();
        let mut reg = Registry::register(&doc, Held::default());
        reg.arm(0);
        let now = 50 * NANOS_PER_SEC;
        let mut seen = [false; 3];
        while let Some(i) = reg.due(now) {
            assert!(!seen[i], "row {i} came due twice in one pass");
            seen[i] = true;
        }
        assert_eq!(seen, [true, true, true]);
    }

    /// **A stall does not become a stampede.** The scheduler is away for an hour; the entry that
    /// should have fired 360,000 times fires once.
    #[test]
    fn a_long_stall_produces_one_fire_and_not_a_backlog() {
        let doc = parse("every 10ms least_authority_demo 7\n").unwrap();
        let mut reg = Registry::register(&doc, Held::default());
        reg.arm(0);
        let hour = 3_600 * NANOS_PER_SEC;
        assert_eq!(reg.due(hour), Some(0));
        assert_eq!(reg.due(hour), None, "it caught up instead of skipping");
        // And it is back on its original beat rather than carrying the delay forever.
        assert_eq!(reg.rows()[0].next_fire().unwrap() % 10_000_000, 0);
    }

    #[test]
    fn next_after_is_strictly_in_the_future_and_keeps_its_phase() {
        // The ordinary case.
        assert_eq!(next_after(100, 10, 100), 110);
        // Exactly on a boundary: strictly greater, so a polling loop cannot fire twice.
        assert_eq!(next_after(100, 10, 110), 120);
        // Late, by a whole number of periods and by a fraction of one.
        assert_eq!(next_after(100, 10, 185), 190);
        assert_eq!(next_after(100, 10, 190), 200);
        // The phase survives.
        for prev in [0u64, 3, 97, 1_000_003] {
            for period in [1u64, 7, 1_000, 999_983] {
                for now in [0u64, 1, 50, 1_000_000, u64::MAX / 4] {
                    let next = next_after(prev, period, now);
                    assert!(next > now, "{prev} {period} {now} -> {next}");
                    assert!(next >= prev.saturating_add(period));
                    assert_eq!(next % period, prev % period);
                    if now >= prev {
                        assert!(next <= now.saturating_add(period));
                    }
                }
            }
        }
        // Total rather than panicking, for a period `parse` will never produce.
        assert_eq!(next_after(1, 0, 5), u64::MAX);
    }

    /// **§222's third sub-ruling: a byte-identical entry keeps its beat, and nothing else does.**
    /// Four cases in one document pair, because each is the negative control for another: the
    /// kept interval proves carrying happens, the edited one proves it is by text rather than by
    /// position, the fired `at-boot` line proves a resend is not a reboot, and the duplicate proves
    /// one old beat is lent once.
    #[test]
    fn a_replacement_keeps_the_beat_of_every_line_it_did_not_change() {
        let old_doc = parse(
            "every 10s least_authority_demo 7\n\
             every 10s least_authority_demo 5\n\
             at-boot least_authority_demo 3\n",
        )
        .unwrap();
        let mut old = Registry::register(&old_doc, Held::default());
        old.arm(1_000);
        // The at-boot row fires; the intervals are due at 10_001_000 and have not come round.
        assert_eq!(old.due(1_000), Some(2));
        assert_eq!(old.due(1_000), None);

        let new_doc = parse(
            "at-boot least_authority_demo 3\n\
             every 10s least_authority_demo 7\n\
             every 10s least_authority_demo 7\n\
             every 20s least_authority_demo 5\n",
        )
        .unwrap();
        let mut new = Registry::register(&new_doc, Held::default());
        let kept = new.arm_after(&old, 5_000_000_000);

        assert_eq!(
            kept, 0b0011,
            "the at-boot line and the first copy of the heartbeat kept"
        );
        assert_eq!(
            new.rows()[0].next_fire(),
            None,
            "an at-boot line that fired stays fired"
        );
        assert_eq!(
            new.rows()[1].next_fire(),
            Some(10_000_001_000),
            "the old beat, not a new one"
        );
        assert_eq!(
            new.rows()[2].next_fire(),
            Some(15_000_000_000),
            "a duplicate arms fresh: one old beat is lent once",
        );
        assert_eq!(
            new.rows()[3].next_fire(),
            Some(25_000_000_000),
            "an edited interval is a new line, whatever position it holds",
        );
    }

    /// A line refused before has no beat to lend, even if the replacement admits the same text.
    #[test]
    fn a_line_that_was_not_armed_lends_no_beat() {
        let doc = parse("every 10s memory_grant_depleter --mem 4\n").unwrap();
        let mut old = Registry::register(&doc, Held::default());
        old.arm(0);
        assert!(
            old.rows()[0].next_fire().is_none(),
            "unbacked in a timetable holding no memory"
        );
        let mut new = Registry::register(
            &doc,
            Held {
                mem_pages: 4,
                ..Held::default()
            },
        );
        assert_eq!(new.arm_after(&old, 7), 0);
        assert_eq!(new.rows()[0].next_fire(), Some(10_000_000_007));
    }

    /// A wall-clock reading at `minute`, published by `generation`.
    fn clock(minute: i64, generation: u64, set: bool) -> Option<WallReading> {
        Some(WallReading {
            minute,
            generation,
            set,
        })
    }

    /// 2026-10-05T00:00Z, a Monday, in minutes.
    const MON: i64 = 20_731 * 1440;

    /// **S3 as ruled with G5, step by step**: dormant while the clock is unknown, armed strictly
    /// after now when it becomes known, one fire for a forward step however much it covered, never
    /// twice after a backward step, and a `SET` that clears the stamps so a correction can fire again.
    #[test]
    fn the_step_rule_fires_once_forward_never_twice_backward_and_a_set_clears() {
        let doc = parse("every day at 02:00 least_authority_demo 7\n").unwrap();
        let mut reg = Registry::register(&doc, with_clock());
        reg.arm(0);

        // Unknown: dormant, even long after any 02:00.
        reg.observe(None);
        assert_eq!(reg.due(0), None);

        // Known at 03:00 Monday: nothing for the past; the next is Tuesday 02:00.
        reg.observe(clock(MON + 180, 1, false));
        assert_eq!(
            reg.due(0),
            None,
            "leaving UNKNOWN fires nothing for the past"
        );
        reg.observe(clock(MON + 1440 + 119, 1, false));
        assert_eq!(reg.due(0), None, "01:59 Tuesday");
        reg.observe(clock(MON + 1440 + 120, 1, false));
        assert_eq!(reg.due(0), Some(0), "02:00 Tuesday");
        assert_eq!(reg.due(0), None, "once");

        // A forward step of five days fires once, not five times.
        reg.observe(clock(MON + 6 * 1440 + 600, 2, false));
        assert_eq!(reg.due(0), Some(0));
        assert_eq!(
            reg.due(0),
            None,
            "a step that covered five occurrences fires one"
        );

        // A SYNCED step back two days fires nothing it already fired, and resumes after the stamp.
        reg.observe(clock(MON + 4 * 1440 + 600, 3, false));
        assert_eq!(reg.due(0), None);
        reg.observe(clock(MON + 6 * 1440 + 1439, 3, false));
        assert_eq!(
            reg.due(0),
            None,
            "Saturday 02:00 already fired, before the step"
        );
        reg.observe(clock(MON + 7 * 1440 + 120, 3, false));
        assert_eq!(reg.due(0), Some(0), "Sunday 02:00 is new");

        // An operator SET back two days is a correction: the stamps go, and Friday fires again.
        reg.observe(clock(MON + 5 * 1440 + 100, 4, true));
        assert_eq!(reg.due(0), None, "01:40 Friday");
        reg.observe(clock(MON + 5 * 1440 + 120, 4, true));
        assert_eq!(
            reg.due(0),
            Some(0),
            "the operator asked for Friday back, and gets it"
        );
    }

    /// A forward step past `through` fires the last covered occurrence once, and the line is spent.
    #[test]
    fn a_step_past_through_fires_once_and_the_line_is_spent() {
        let doc = parse("every day through 2026-10-07 at 02:00 least_authority_demo 7\n").unwrap();
        let mut reg = Registry::register(&doc, with_clock());
        reg.arm(0);
        reg.observe(clock(MON, 1, false));
        reg.observe(clock(MON + 30 * 1440, 2, false));
        assert_eq!(
            reg.due(0),
            Some(0),
            "the step covered the last occurrences: one fire"
        );
        assert_eq!(reg.due(0), None, "and the line is spent");
        // Armed against Monday, then stepped a month: re-arming finds nothing after the step.
        let mut reg = Registry::register(&doc, with_clock());
        reg.arm(0);
        reg.observe(clock(MON, 1, false));
        reg.observe(clock(MON + 3 * 1440, 1, false));
        assert_eq!(reg.due(0), Some(0), "the occurrence the clock reached");
        assert_eq!(reg.due(0), None);
        reg.observe(clock(MON + 30 * 1440, 1, false));
        assert_eq!(reg.due(0), None, "spent after the 7th");
    }

    /// A replacement that keeps a calendar line keeps its stamp, so resending a document is not a
    /// licence to fire today's occurrence twice.
    #[test]
    fn a_replacement_keeps_a_calendar_lines_stamp() {
        let doc = parse("every day at 02:00 least_authority_demo 7\n").unwrap();
        let mut old = Registry::register(&doc, with_clock());
        old.arm(0);
        old.observe(clock(MON + 100, 1, false));
        old.observe(clock(MON + 120, 1, false));
        assert_eq!(old.due(0), Some(0));
        let mut new = Registry::register(&doc, with_clock());
        assert_eq!(new.arm_after(&old, 0), 1);
        // The same generation, but `arm` reset what the registry last saw, so it re-arms from the
        // carried stamp: 02:00 today is behind it.
        new.observe(clock(MON + 120, 1, false));
        assert_eq!(
            new.due(0),
            None,
            "02:00 already fired before the replacement"
        );
    }

    // ---- survivor triage of the 2026-10-03 census ---------------------------------------------

    #[test]
    fn a_registry_remembers_what_it_was_registered_against() {
        let doc = parse("every 5s least_authority_demo 7\n").unwrap();
        let held = Held {
            mem_pages: 6,
            clock: true,
            dir: true,
            ..Held::default()
        };
        assert_eq!(Registry::register(&doc, held).held(), held);
    }

    /// One old beat is lent once. Three identical lines against two identical old ones: the third
    /// finds both spoken for and starts fresh, which is what a second `lent` bit is for.
    #[test]
    fn two_old_beats_are_lent_to_two_new_lines_and_not_a_third() {
        let old_doc = parse(
            "every 10s least_authority_demo 7\n\
             every 10s least_authority_demo 7\n",
        )
        .unwrap();
        let mut old = Registry::register(&old_doc, Held::default());
        old.arm(0);
        let new_doc = parse(
            "every 10s least_authority_demo 7\n\
             every 10s least_authority_demo 7\n\
             every 10s least_authority_demo 7\n",
        )
        .unwrap();
        let mut new = Registry::register(&new_doc, Held::default());
        assert_eq!(new.arm_after(&old, 7), 0b011);
        assert_eq!(new.rows()[0].next_fire(), Some(10_000_000_000));
        assert_eq!(new.rows()[1].next_fire(), Some(10_000_000_000));
        assert_eq!(new.rows()[2].next_fire(), Some(10_000_000_007));
    }
}
