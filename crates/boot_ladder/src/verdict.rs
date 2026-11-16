//! **The self-test's tally and the lines it prints**, shared by its two halves: the kernel, which
//! runs the checks of its own internals, and `boot_self_test`, the userspace program the progenitor
//! runs for the checks that need a user program. Both print through this code, so a check line and the verdict read
//! the same whichever half printed them.
//!
//! Name: provisional, minted 2026-11-10 (UTC) by lane
//! `milestone/1063-the-boot-self-test-spawns-from-userspace`, for what the module renders.

use core::fmt;

use crate::{Heads, SELF_TEST_ABSENT, SELF_TEST_CHECKS, SELF_TEST_FAILED};

/// How many checks there are: the length of [`SELF_TEST_CHECKS`].
///
/// Name: provisional.
pub const CHECKS: usize = SELF_TEST_CHECKS.len();

// The tally is three bitmasks over the list, one bit per check, and its packed form puts the three
// in one word, [`PACKED_FIELD_BITS`] each.
const _: () = assert!(CHECKS <= PACKED_FIELD_BITS as usize);

/// **How many bits each of the three masks takes in [`Tally::to_packed`]'s word.** Three fields of
/// 21 fit 63 bits, so the list can grow to 21 checks before the packed form must change.
///
/// Name: provisional.
pub const PACKED_FIELD_BITS: u32 = 21;

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
    ///
    /// Name: provisional.
    pub const fn new() -> Self {
        Self {
            passed: 0,
            failed: 0,
            absent: 0,
        }
    }

    /// Record `check`'s outcome. A second record of the same check replaces the first.
    ///
    /// Name: provisional.
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
    ///
    /// Name: provisional.
    pub fn is_recorded(&self, check: usize) -> bool {
        (self.passed | self.failed | self.absent) & (1 << check) != 0
    }

    /// **Fail every listed check that recorded nothing, and return their names**, for the caller
    /// to print a line for each before the verdict. The total is the list's length, not how many
    /// checks ran, so a check that never ran reads as `FAILED` by name on every architecture.
    ///
    /// Name: provisional.
    pub fn fail_unrecorded(&mut self) -> impl Iterator<Item = &'static str> + use<> {
        let missing = !(self.passed | self.failed | self.absent) & all_checks();
        self.failed |= missing;
        names(missing)
    }

    /// The tally as the three words the kernel hands over: passed, failed, absent.
    ///
    /// Name: provisional.
    pub const fn to_words(&self) -> [u64; 3] {
        [self.passed, self.failed, self.absent]
    }

    /// A tally from [`to_words`](Tally::to_words)'s three words. Bits past the list are dropped,
    /// and a bit set in more than one word keeps the worst: failed, then absent, then passed.
    ///
    /// Name: provisional.
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

    /// **The tally in one word**: passed in the low [`PACKED_FIELD_BITS`] bits, failed in the next,
    /// absent in the next. The form a one-word message carries: `boot_self_test` sends it to the
    /// progenitor with its verdict, and the progenitor sends it on to the kernel, which paints the
    /// verdict on a screen it still holds.
    ///
    /// Name: provisional.
    pub const fn to_packed(&self) -> u64 {
        self.passed | (self.failed << PACKED_FIELD_BITS) | (self.absent << (2 * PACKED_FIELD_BITS))
    }

    /// A tally from [`to_packed`](Tally::to_packed)'s word, checked as
    /// [`from_words`](Tally::from_words) checks: bits past the list are dropped, and a check claimed
    /// twice keeps the worst outcome.
    ///
    /// Name: provisional.
    pub const fn from_packed(word: u64) -> Self {
        let field = (1u64 << PACKED_FIELD_BITS) - 1;
        Self::from_words([
            word & field,
            (word >> PACKED_FIELD_BITS) & field,
            (word >> (2 * PACKED_FIELD_BITS)) & field,
        ])
    }

    /// The verdict line, without its newline, under `heads`'s [`self_test`](Heads::self_test)
    /// head: [`Verdict`]'s `Display`.
    ///
    /// Name: provisional.
    pub fn verdict<'a>(&'a self, heads: Heads<'a>) -> Verdict<'a> {
        Verdict { tally: self, heads }
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

/// **The verdict line**: `<system> self-test: 9 of 15 passed, 4 absent: block_device
/// device_interrupt clock true_random_number_generator, 2 FAILED: exceptions userspace`.
///
/// The counts are there on success as well as on failure, so "ran nothing and passed" reads
/// differently from "ran the set and passed". Absences come before failures, so a matcher reading
/// the names after `FAILED: ` to the end of the line reads only failures. Names appear in list
/// order. Built by `Display` rather than by joining, so nothing allocates.
///
/// Name: provisional (milestone 1063's lane, 2026-11-10 UTC).
pub struct Verdict<'a> {
    tally: &'a Tally,
    heads: Heads<'a>,
}

impl fmt::Display for Verdict<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let t = self.tally;
        let head = self.heads.self_test();
        write!(f, "{head}{} of {CHECKS} passed", t.passed.count_ones())?;
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

/// **A thread the self-test kills on purpose, named before its death is taken**:
/// `<system> self-test subject: thread 19, which the userspace check kills on purpose`.
///
/// The thread id is decimal, as the kernel prints it in its own report of the kill (`user thread
/// 19 killed: ...`), so a gate can match the two by number. `swish-check` excludes from its
/// killed-thread check exactly the threads these lines name, and fails every other kill;
/// [`subject_thread`] reads the number back.
///
/// Name: provisional (milestone 1063's lane, 2026-11-11 UTC).
pub struct SubjectLine<'a> {
    /// The system's heads, whose [`self_test_subject`](Heads::self_test_subject) begins the line.
    pub heads: Heads<'a>,
    /// The thread's id, as the kernel reports it.
    pub thread: u64,
    /// The check that kills it, from [`SELF_TEST_CHECKS`].
    pub check: &'a str,
}

impl fmt::Display for SubjectLine<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}, which the {} check kills on purpose",
            self.heads.self_test_subject(),
            self.thread,
            self.check
        )
    }
}

/// **The thread a [`SubjectLine`] under `heads` names**, from anywhere in `line`, or `None` when
/// `line` carries no such subject line.
///
/// Name: provisional (milestone 1063's lane, 2026-11-11 UTC).
pub fn subject_thread(heads: Heads<'_>, line: &str) -> Option<u64> {
    let at = heads.self_test_subject().find_in(line)?.end;
    let digits = line[at..]
        .find(|c: char| !c.is_ascii_digit())
        .map_or(&line[at..], |end| &line[at..at + end]);
    digits.parse().ok()
}

/// **A verdict line's tail, read back**: the counts and the two name lists that follow the
/// [`self_test`](Heads::self_test) head, in the grammar [`Verdict`] prints. Writer and readers
/// share this one grammar, so a change to a separator breaks the round-trip test here and not a
/// reader on a bench.
///
/// `X of Y passed`, then optionally `, N absent: <names>`, then optionally `, M FAILED: <names>`.
/// Names are `[a-z0-9_]` words separated by single spaces. It borrows the line and allocates
/// nothing.
///
/// Name: provisional.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParsedVerdict<'a> {
    /// How many checks passed.
    pub passed: usize,
    /// How many checks the verdict counted: [`CHECKS`] for a kernel built from this list, and one
    /// of [`EARLIER_SELF_TEST_CHECK_COUNTS`](crate::EARLIER_SELF_TEST_CHECK_COUNTS) for an older one.
    pub total: usize,
    absent: Option<(usize, &'a str)>,
    failed: Option<(usize, &'a str)>,
}

impl<'a> ParsedVerdict<'a> {
    /// **The verdict `text` holds under `heads`**, found anywhere in the line (a serial link may
    /// put bytes before the head), or `None` when the line has no verdict head or its tail is not
    /// the grammar.
    ///
    /// Name: provisional.
    pub fn in_line(heads: Heads<'_>, line: &'a str) -> Option<Self> {
        let at = heads.self_test().find_in(line)?.end;
        Self::parse(&line[at..])
    }

    /// **The verdict whose tail is `tail`**, the text after the head, or `None` for anything that
    /// is not the grammar, so an unfamiliar line is left alone rather than reported as counts it
    /// never printed. Trailing white space is ignored.
    ///
    /// Name: provisional.
    pub fn parse(tail: &'a str) -> Option<Self> {
        let tail = tail.trim_ascii_end();
        let (passed, rest) = digits(tail)?;
        let (total, rest) = digits(rest.strip_prefix(" of ")?)?;
        let rest = rest.strip_prefix(" passed")?;
        let (absent, rest) = match segment(rest, SELF_TEST_ABSENT) {
            Some((count, names, rest)) => (Some((count, names)), rest),
            None => (None, rest),
        };
        let (failed, rest) = match segment(rest, SELF_TEST_FAILED) {
            Some((count, names, rest)) => (Some((count, names)), rest),
            None => (None, rest),
        };
        rest.is_empty().then_some(Self {
            passed,
            total,
            absent,
            failed,
        })
    }

    /// How many checks the verdict says were absent: `0` when it has no absent list.
    ///
    /// Name: provisional.
    pub fn absent_count(&self) -> usize {
        self.absent.map_or(0, |(count, _)| count)
    }

    /// How many checks the verdict says failed: `0` when it has no failed list.
    ///
    /// Name: provisional.
    pub fn failed_count(&self) -> usize {
        self.failed.map_or(0, |(count, _)| count)
    }

    /// Whether the verdict carries a failed list, which is what makes it red.
    ///
    /// Name: provisional.
    pub fn is_red(&self) -> bool {
        self.failed.is_some()
    }

    /// The names the verdict lists as absent, in the order printed.
    ///
    /// Name: provisional.
    pub fn absent_names(&self) -> impl Iterator<Item = &'a str> + use<'a> {
        self.absent
            .map_or("", |(_, names)| names)
            .split(' ')
            .filter(|n| !n.is_empty())
    }

    /// The names the verdict lists as failed, in the order printed.
    ///
    /// Name: provisional.
    pub fn failed_names(&self) -> impl Iterator<Item = &'a str> + use<'a> {
        self.failed
            .map_or("", |(_, names)| names)
            .split(' ')
            .filter(|n| !n.is_empty())
    }
}

/// The decimal number `text` begins with and what follows it, or `None` when it begins with no
/// digit or the number does not fit.
fn digits(text: &str) -> Option<(usize, &str)> {
    let (value, end) = leading_number(text.as_bytes())?;
    Some((value, &text[end..]))
}

/// The value of the decimal number `bytes` begins with and where it ends, or `None` when it begins
/// with no digit or the number does not fit.
fn leading_number(bytes: &[u8]) -> Option<(usize, usize)> {
    let mut value = 0usize;
    let mut end = 0;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        value = value
            .checked_mul(10)?
            .checked_add(usize::from(bytes[end] - b'0'))?;
        end += 1;
    }
    (end > 0).then_some((value, end))
}

/// One `, N <marker><names>` segment at the front of `text`: its count, its names (up to the next
/// comma or the end) and what follows.
fn segment<'a>(text: &'a str, marker: &str) -> Option<(usize, &'a str, &'a str)> {
    let (count, rest) = digits(text.strip_prefix(", ")?)?;
    let rest = rest.strip_prefix(' ')?.strip_prefix(marker)?;
    let end = rest.bytes().position(|b| b == b',').unwrap_or(rest.len());
    // A check's name is `[a-z0-9_]`, so a list holds those and the spaces between them.
    let plain = |b: &u8| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b' ');
    rest.as_bytes()[..end]
        .iter()
        .all(plain)
        .then(|| (count, &rest[..end], &rest[end..]))
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::format;
    use std::vec::Vec;

    use super::*;

    const HEADS: Heads<'static> = Heads::new("example");

    /// The line a green tally prints is the shape `serial_console` reads.
    #[test]
    fn a_tally_prints_the_verdict_the_console_reads() {
        let mut t = Tally::new();
        for i in 0..CHECKS {
            t.record(i, Outcome::Passed);
        }
        t.record(check("block_device"), Outcome::Absent);
        t.record(check("userspace"), Outcome::Failed);
        assert_eq!(
            format!("{}", t.verdict(HEADS)),
            format!(
                "example self-test: {} of {CHECKS} passed, 1 absent: block_device, 1 FAILED: userspace",
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
        assert!(format!("{}", t.verdict(HEADS)).ends_with("1 FAILED: teardown"));
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

    /// The packed word carries the same tally as the three words, and keeps the worst outcome.
    #[test]
    fn the_packed_word_round_trips_and_keeps_the_worst() {
        let mut t = Tally::new();
        for i in 0..CHECKS {
            t.record(i, Outcome::Passed);
        }
        t.record(check("true_random_number_generator"), Outcome::Absent);
        t.record(check("frames"), Outcome::Failed);
        assert_eq!(Tally::from_packed(t.to_packed()), t);
        let both = 1 << check("mapping");
        let forged = Tally::from_packed(both | (both << PACKED_FIELD_BITS));
        assert_eq!(forged.to_words(), [0, both, 0]);
        assert_eq!(
            Tally::from_packed(u64::MAX).to_words()[1].count_ones() as usize,
            CHECKS
        );
    }

    /// A subject line reads back as the thread it names, and nothing else does.
    #[test]
    fn a_subject_line_names_its_thread() {
        let line = format!(
            "{}",
            SubjectLine {
                heads: HEADS,
                thread: 8_589_934_611,
                check: "supervised",
            }
        );
        assert_eq!(
            line,
            "example self-test subject: thread 8589934611, which the supervised check kills on purpose"
        );
        assert_eq!(subject_thread(HEADS, &line), Some(8_589_934_611));
        assert_eq!(
            subject_thread(HEADS, "  user thread 19 killed: Data abort"),
            None
        );
        assert_eq!(
            subject_thread(HEADS, "example self-test subject: thread x"),
            None
        );
        assert!(
            !HEADS.self_test().is_in(&line),
            "a subject line is not a verdict line"
        );
    }

    /// A check has an outcome once it is recorded, whatever the outcome, and recording one check
    /// leaves every other check's outcome alone.
    #[test]
    fn recording_one_check_keeps_the_others() {
        let mut t = Tally::new();
        assert!(!t.is_recorded(check("timer")));
        t.record(check("timer"), Outcome::Passed);
        t.record(check("clock"), Outcome::Failed);
        t.record(check("block_device"), Outcome::Absent);
        t.record(check("true_random_number_generator"), Outcome::Absent);
        for name in [
            "timer",
            "clock",
            "block_device",
            "true_random_number_generator",
        ] {
            assert!(t.is_recorded(check(name)), "{name}");
        }
        assert!(!t.is_recorded(check("mapping")));
        assert_eq!(
            format!("{}", t.verdict(HEADS)),
            format!(
                "example self-test: 1 of {CHECKS} passed, 2 absent: block_device true_random_number_generator, 1 FAILED: clock"
            )
        );
    }

    /// With every check recorded, nothing is failed for being unrecorded and the verdict is clean.
    #[test]
    fn a_full_tally_fails_nothing_unrecorded() {
        let mut t = Tally::new();
        for i in 0..CHECKS {
            t.record(i, Outcome::Passed);
        }
        assert_eq!(t.fail_unrecorded().count(), 0);
        assert_eq!(
            format!("{}", t.verdict(HEADS)),
            format!("example self-test: {CHECKS} of {CHECKS} passed")
        );
    }

    /// A failed or absent check is recorded, so only the checks with no outcome at all are failed
    /// for being unrecorded.
    #[test]
    fn failed_and_absent_checks_are_not_unrecorded() {
        let mut t = Tally::new();
        for i in 0..CHECKS {
            t.record(i, Outcome::Passed);
        }
        t.record(check("clock"), Outcome::Failed);
        t.record(check("true_random_number_generator"), Outcome::Absent);
        assert_eq!(t.fail_unrecorded().count(), 0);
    }

    /// A name off the list stops at the list's end, with the message that names the list.
    #[test]
    #[should_panic(expected = "not a check in boot_ladder::SELF_TEST_CHECKS")]
    fn a_name_off_the_list_is_refused() {
        check("not_a_check");
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

    /// **What the writer prints, the reader reads back**, for every combination of outcomes the
    /// tally can hold: the counts, and each name list in order. The test that fails when a
    /// separator changes on one side only.
    #[test]
    fn a_printed_verdict_parses_back_to_its_tally() {
        for (passed, absent, failed) in [
            (0usize, 0usize, 0usize),
            (CHECKS, 0, 0),
            (CHECKS - 2, 1, 1),
            (1, 3, 2),
            (0, 0, CHECKS),
            (0, CHECKS, 0),
        ] {
            let mut t = Tally::new();
            for i in 0..CHECKS {
                let outcome = if i < passed {
                    Outcome::Passed
                } else if i < passed + absent {
                    Outcome::Absent
                } else if i < passed + absent + failed {
                    Outcome::Failed
                } else {
                    Outcome::Passed
                };
                t.record(i, outcome);
            }
            let line = format!("{}", t.verdict(HEADS));
            let read = ParsedVerdict::in_line(HEADS, &line).expect("the printed verdict parses");
            let expected = |mask: u64| -> Vec<&'static str> { super::names(mask).collect() };
            assert_eq!(read.total, CHECKS, "{line}");
            assert_eq!(read.passed, t.passed.count_ones() as usize, "{line}");
            assert_eq!(
                read.absent_count(),
                t.absent.count_ones() as usize,
                "{line}"
            );
            assert_eq!(
                read.failed_count(),
                t.failed.count_ones() as usize,
                "{line}"
            );
            assert_eq!(read.absent_names().collect::<Vec<_>>(), expected(t.absent));
            assert_eq!(read.failed_names().collect::<Vec<_>>(), expected(t.failed));
            assert_eq!(read.is_red(), t.failed != 0);
        }
    }

    /// The reader finds the head after bytes a serial link corrupted, ignores trailing white
    /// space, and takes an older kernel's total as printed.
    #[test]
    fn the_reader_takes_what_a_log_holds() {
        let read = ParsedVerdict::in_line(
            HEADS,
            "\u{fffd}x example self-test: 4 of 5 passed, 1 absent: clock \r",
        )
        .expect("a verdict after noise");
        assert_eq!((read.passed, read.total), (4, 5));
        assert_eq!(read.absent_names().collect::<Vec<_>>(), ["clock"]);
        assert!(!read.is_red());
    }

    /// Anything that is not the grammar is left alone: no head, a different separator, a wrong
    /// order, text after the lists, a count that does not fit.
    #[test]
    fn a_line_that_is_not_the_grammar_is_refused() {
        for tail in [
            "",
            "3 of three passed",
            "3 off 4 passed",
            "3 of 4 passed,",
            "3 of 4 passed, 1 FAILED: clock, 1 absent: timer",
            "3 of 4 passed, 1 absent: clock; 1 FAILED: timer",
            "3 of 4 passed, one absent: clock",
            "3 of 4 passed and more",
            "99999999999999999999999 of 4 passed",
        ] {
            assert_eq!(ParsedVerdict::parse(tail), None, "{tail:?}");
        }
        assert_eq!(
            ParsedVerdict::in_line(HEADS, "example self-test subject: thread 4"),
            None
        );
        assert_eq!(ParsedVerdict::in_line(HEADS, "nothing here"), None);
    }
}

/// Proofs, run by `cargo kani` and compiled by nothing else.
#[cfg(kani)]
mod verification {
    use super::*;

    /// **Reading a count never overflows, however many digits arrive.** Twenty digits overflow a
    /// 64-bit count, so the bound is one digit longer than that. Other bytes end the count at the
    /// first one, which the segment harness below covers.
    ///
    /// Falsification: replayable `crates/boot_ladder/falsifications/verdict.verification.a_count_is_read_without_overflow.patch`
    #[kani::proof]
    #[kani::unwind(23)]
    fn a_count_is_read_without_overflow() {
        const LEN: usize = 21;
        let bytes: [u8; LEN] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= LEN);
        if let Some((_, end)) = leading_number(&bytes[..len]) {
            assert!(end <= len);
        }
    }

    /// **Reading one list segment never panics and returns only plain names**, whatever bytes
    /// follow its marker, so a name list holds nothing a reader could mistake for the grammar.
    ///
    /// Falsification: replayable `crates/boot_ladder/falsifications/verdict.verification.a_segment_is_read_without_panicking.patch`
    #[kani::proof]
    #[kani::unwind(14)]
    fn a_segment_is_read_without_panicking() {
        const LEN: usize = 12;
        let bytes: [u8; LEN] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= LEN);
        let mut i = 0;
        while i < LEN {
            kani::assume(bytes[i].is_ascii());
            i += 1;
        }
        let text = core::str::from_utf8(&bytes[..len]).unwrap();
        if let Some((_, names, rest)) = segment(text, SELF_TEST_ABSENT) {
            assert!(names.len() + rest.len() <= text.len());
            assert!(
                names.bytes().all(|b| b.is_ascii_lowercase()
                    || b.is_ascii_digit()
                    || b == b'_'
                    || b == b' ')
            );
        }
    }
}
