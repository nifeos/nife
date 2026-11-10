//! **The self-test's tally and the lines it prints**, shared by its two halves: the kernel, which
//! runs the checks of its own internals, and `boot_self_test`, the userspace program the progenitor
//! runs for the checks that need a user program (milestone 1063). Both print through this code, so
//! a check line and the verdict read the same whichever half printed them. See
//! notes/self-test-checks.md, "The verdict's mechanics".
//!
//! Name: provisional, minted 2026-11-10 (UTC) by lane
//! `milestone/1063-the-boot-self-test-spawns-from-userspace`, for what the module renders.

use core::fmt;

use crate::{SELF_TEST, SELF_TEST_ABSENT, SELF_TEST_CHECKS, SELF_TEST_FAILED};

/// How many checks there are: the length of [`SELF_TEST_CHECKS`].
pub const CHECKS: usize = SELF_TEST_CHECKS.len();

// The tally is three bitmasks over the list, one bit per check.
const _: () = assert!(CHECKS <= 64);

/// **The index of `name` in [`SELF_TEST_CHECKS`].** Evaluated in a `const`, an unlisted name is a
/// compile error, so a check the list does not name cannot record an outcome at all:
/// `const { boot_ladder::verdict::check("timer") }`.
///
/// # Panics
///
/// When `name` is not on the list, which in a `const` stops the build.
///
/// Name: provisional (milestone 1063's lane, 2026-11-10 UTC).
pub const fn check(name: &str) -> usize {
    let mut i = 0;
    while i < CHECKS {
        if str_eq(SELF_TEST_CHECKS[i], name) {
            return i;
        }
        i += 1;
    }
    panic!("not a check in boot_ladder::SELF_TEST_CHECKS");
}

/// `a == b` in a `const fn`, where `PartialEq` on `str` cannot be called.
const fn str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// **What one check came to.**
///
/// Name: provisional (milestone 1063's lane, 2026-11-10 UTC).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// It ran and its claim held.
    Passed,
    /// It ran and its claim did not hold, or it could not run on a machine that has the device.
    Failed,
    /// The machine has nothing for it to test (`boot_ladder::SELF_TEST_ABSENT`).
    Absent,
}

/// **The running tally**: which checks passed, failed or were absent, one bit each, by index into
/// [`SELF_TEST_CHECKS`]. No allocation, because a self-test that could fail to report by running
/// out of memory would report on the wrong thing.
///
/// The kernel records its half and hands the three words to the progenitor
/// ([`to_words`](Tally::to_words)); `boot_self_test` reads them back
/// ([`from_words`](Tally::from_words)), records the rest and prints the verdict.
///
/// Name: provisional (milestone 1063's lane, 2026-11-10 UTC).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    passed: u64,
    failed: u64,
    absent: u64,
}

impl Tally {
    /// A tally with nothing recorded.
    pub const fn new() -> Self {
        Self {
            passed: 0,
            failed: 0,
            absent: 0,
        }
    }

    /// Record `check`'s outcome. A second record of the same check replaces the first.
    pub fn record(&mut self, check: usize, outcome: Outcome) {
        let bit = 1u64 << check;
        self.passed &= !bit;
        self.failed &= !bit;
        self.absent &= !bit;
        match outcome {
            Outcome::Passed => self.passed |= bit,
            Outcome::Failed => self.failed |= bit,
            Outcome::Absent => self.absent |= bit,
        }
    }

    /// Whether `check` has an outcome yet.
    pub fn is_recorded(&self, check: usize) -> bool {
        (self.passed | self.failed | self.absent) & (1 << check) != 0
    }

    /// **Fail every listed check that recorded nothing, and return their names**, for the caller
    /// to print a line for each before the verdict. The total is the list's length, not how many
    /// checks ran, so a check that never ran reads as `FAILED` by name on every architecture.
    pub fn fail_unrecorded(&mut self) -> impl Iterator<Item = &'static str> + use<> {
        let missing = !(self.passed | self.failed | self.absent) & all_checks();
        self.failed |= missing;
        names(missing)
    }

    /// The tally as the three words the kernel hands over: passed, failed, absent.
    pub const fn to_words(&self) -> [u64; 3] {
        [self.passed, self.failed, self.absent]
    }

    /// A tally from [`to_words`](Tally::to_words)'s three words. Bits past the list are dropped,
    /// and a bit set in more than one word keeps the worst: failed, then absent, then passed.
    pub const fn from_words(words: [u64; 3]) -> Self {
        let failed = words[1] & all_checks();
        let absent = words[2] & all_checks() & !failed;
        let passed = words[0] & all_checks() & !failed & !absent;
        Self {
            passed,
            failed,
            absent,
        }
    }

    /// The verdict line, without its newline: [`Verdict`]'s `Display`.
    pub fn verdict(&self) -> Verdict<'_> {
        Verdict(self)
    }
}

/// Every listed check's bit.
const fn all_checks() -> u64 {
    if CHECKS == 64 {
        u64::MAX
    } else {
        (1u64 << CHECKS) - 1
    }
}

/// The names of the checks whose bits are set in `mask`, in list order.
fn names(mask: u64) -> impl Iterator<Item = &'static str> {
    SELF_TEST_CHECKS
        .iter()
        .enumerate()
        .filter(move |(i, _)| mask & (1 << i) != 0)
        .map(|(_, &name)| name)
}

/// **The verdict line**: `nife self-test: 9 of 15 passed, 4 absent: blockdev device_interrupt clock
/// trng, 2 FAILED: exceptions userspace`.
///
/// The counts are there on success as well as on failure, so "ran nothing and passed" reads
/// differently from "ran the set and passed". Absences come before failures, so a matcher reading
/// the names after `FAILED: ` to the end of the line reads only failures. Names appear in list
/// order. Built by `Display` rather than by joining, so nothing allocates.
///
/// Name: provisional (milestone 1063's lane, 2026-11-10 UTC).
pub struct Verdict<'a>(&'a Tally);

impl fmt::Display for Verdict<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let t = self.0;
        write!(f, "{SELF_TEST}{} of {CHECKS} passed", t.passed.count_ones())?;
        if t.absent != 0 {
            write!(f, ", {} {SELF_TEST_ABSENT}", t.absent.count_ones())?;
            write_names(f, t.absent)?;
        }
        if t.failed != 0 {
            write!(f, ", {} {SELF_TEST_FAILED}", t.failed.count_ones())?;
            write_names(f, t.failed)?;
        }
        Ok(())
    }
}

/// Names separated by single spaces.
fn write_names(f: &mut fmt::Formatter<'_>, mask: u64) -> fmt::Result {
    for (i, name) in names(mask).enumerate() {
        if i > 0 {
            f.write_str(" ")?;
        }
        f.write_str(name)?;
    }
    Ok(())
}

/// **One check's line**: `  self-test       : timer      ok      <detail>`, with `FAILED` or
/// `absent` in the outcome's place. The detail carries the numbers the check measured, because
/// `ok` alone tells a reader nothing they can act on when the next boot says `FAILED`.
///
/// Name: provisional (milestone 1063's lane, 2026-11-10 UTC).
pub struct CheckLine<'a> {
    /// The check's name, from [`SELF_TEST_CHECKS`].
    pub name: &'a str,
    /// What it came to.
    pub outcome: Outcome,
    /// The numbers it measured, or the reason it was absent.
    pub detail: fmt::Arguments<'a>,
}

impl fmt::Display for CheckLine<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self.name;
        match self.outcome {
            Outcome::Passed => write!(f, "  self-test       : {name:<10} ok      {}", self.detail),
            Outcome::Failed => write!(f, "  self-test       : {name:<10} FAILED  {}", self.detail),
            Outcome::Absent => write!(f, "  self-test       : {name:<10} absent  {}", self.detail),
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::format;
    use std::vec::Vec;

    use super::*;

    /// The line a green tally prints is the shape `serial_console` reads.
    #[test]
    fn a_tally_prints_the_verdict_the_console_reads() {
        let mut t = Tally::new();
        for i in 0..CHECKS {
            t.record(i, Outcome::Passed);
        }
        t.record(check("blockdev"), Outcome::Absent);
        t.record(check("userspace"), Outcome::Failed);
        assert_eq!(
            format!("{}", t.verdict()),
            format!(
                "nife self-test: {} of {CHECKS} passed, 1 absent: blockdev, 1 FAILED: userspace",
                CHECKS - 2
            )
        );
    }

    /// A check nobody recorded is named as a failure, so a half that never ran shows by name.
    #[test]
    fn an_unrecorded_check_fails_by_name() {
        let mut t = Tally::new();
        for i in 0..CHECKS {
            if i != check("teardown") {
                t.record(i, Outcome::Passed);
            }
        }
        let missing: Vec<_> = t.fail_unrecorded().collect();
        assert_eq!(missing, ["teardown"]);
        assert!(format!("{}", t.verdict()).ends_with("1 FAILED: teardown"));
    }

    /// The three words survive the hand-over, and a bit claimed twice keeps the worst outcome.
    #[test]
    fn the_words_round_trip_and_keep_the_worst() {
        let mut t = Tally::new();
        t.record(check("exceptions"), Outcome::Passed);
        t.record(check("device_interrupt"), Outcome::Absent);
        t.record(check("timer"), Outcome::Failed);
        assert_eq!(Tally::from_words(t.to_words()), t);
        let both = 1 << check("mapping");
        let forged = Tally::from_words([both, both, 0]);
        assert_eq!(forged.to_words(), [0, both, 0]);
    }

    /// The line keeps the column widths the kernel always printed.
    #[test]
    fn a_check_line_keeps_its_columns() {
        let line = CheckLine {
            name: "timer",
            outcome: Outcome::Passed,
            detail: format_args!("advanced"),
        };
        assert_eq!(
            format!("{line}"),
            "  self-test       : timer      ok      advanced"
        );
    }
}
