//! **The document**: a timetable as a person writes it, parsed into entries, each refusal naming
//! the line it is about. It never sees a grant; what an entry may become is `admission`'s question.
//!
//! The crate root re-exports every public item, as §294 (the timetable crate root splits into
//! private modules) lays out.
//!
//! Name: provisional (milestone 877, 2026-10-12), as are its sibling modules' names.

use crate::{MAX_ENTRIES, NANOS_PER_SEC, recurrence};

/// **When an entry fires.** Two shapes on the monotonic counter, and one on the wall clock.
///
/// The calendar shape is G5, which calef ruled on 2026-09-26 with S3 and its `SET` fix for clock
/// steps ([`Registry::observe`](crate::Registry::observe)).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Schedule {
    /// Once, as soon as the scheduler is armed, and never again.
    AtBoot,
    /// Every interval, in **nanoseconds**, from the moment the scheduler is armed.
    ///
    /// Nanoseconds because that is what `user_mode_runtime::monotonic_nanos` reports and what the arithmetic
    /// in [`next_after`](crate::next_after) runs in; the document is written in `ms`, `s` and `m`, and [`parse`]
    /// converts once, where the conversion can be tested.
    Every(u64),
    /// **A time of day in UTC**, by a calendar rule (G5, `recurrence`): `every day at 02:00`.
    /// Needs the wall clock, so a timetable holding none answers [`Unbacked::WallClock`](crate::Unbacked::WallClock).
    Calendar(recurrence::Rule),
}

/// One line of the document: when, and what.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Entry<'a> {
    /// The 1-based line it was written on, so every refusal can point at it.
    pub line: usize,
    /// When it fires.
    pub schedule: Schedule,
    /// **The grant expression, exactly as a person would type it at the prompt.** Bytes rather than
    /// a parsed structure, because the thing that parses it is `grant_plan` and this crate must not
    /// grow a second opinion about what a command line means.
    pub command: &'a [u8],
}

/// A parsed document: the entries, in the order written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Document<'a> {
    entries: [Entry<'a>; MAX_ENTRIES],
    n: usize,
}

/// Why a document does not parse. Every variant carries the **1-based line number** it went wrong
/// on, because a configuration error a person cannot find is a configuration error they will not
/// fix. The retired `multicast_dns_config` reached the same shape for the same reason (notes/mdns.md).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// A line that is neither blank, a comment, nor a schedule word followed by a command.
    Malformed(usize),
    /// The first word is not `every` or `at-boot`.
    UnknownSchedule(usize),
    /// `every` with no interval after it.
    MissingInterval(usize),
    /// An interval that is not `<digits><unit>`, with the unit one of `ms`, `s`, `m`.
    BadInterval(usize),
    /// `every 0s`. A zero interval is not a fast schedule, it is a spin, and refusing it here is
    /// cheaper than discovering it as a hung machine.
    ZeroInterval(usize),
    /// A schedule word with nothing after it. There is nothing to run.
    MissingCommand(usize),
    /// More entries than [`MAX_ENTRIES`], reported at the line that overflowed rather than dropped.
    TooManyEntries(usize),
    /// A calendar line the G5 grammar refuses, and why (`recurrence::Refusal`).
    Calendar(usize, recurrence::Refusal),
}

impl Error {
    /// The 1-based line this error is about.
    pub fn line(self) -> usize {
        match self {
            Error::Malformed(l)
            | Error::UnknownSchedule(l)
            | Error::MissingInterval(l)
            | Error::BadInterval(l)
            | Error::ZeroInterval(l)
            | Error::MissingCommand(l)
            | Error::TooManyEntries(l)
            | Error::Calendar(l, _) => l,
        }
    }

    /// The fixed half of the sentence a reader gets. Host-tested, so the wording cannot drift.
    pub fn message(self) -> &'static str {
        match self {
            Error::Malformed(_) => "not a schedule and a command",
            Error::UnknownSchedule(_) => "the schedule must be `every <interval>` or `at-boot`",
            Error::MissingInterval(_) => "`every` needs an interval, like `every 30s`",
            Error::BadInterval(_) => "an interval is digits and one of ms, s, m",
            Error::ZeroInterval(_) => "a zero interval is a spin, not a schedule",
            Error::MissingCommand(_) => "a schedule with nothing to run",
            Error::TooManyEntries(_) => "more entries than this timetable holds",
            Error::Calendar(_, r) => r.message(),
        }
    }

    /// For a range that does not end on its last step, the last time it does reach, as `(hour,
    /// minute)`, so the refusal can name the line to write instead.
    pub fn last_reachable(self) -> Option<(u8, u8)> {
        match self {
            Error::Calendar(_, recurrence::Refusal::InexactRange(Some(t))) => {
                Some(((t / 60) as u8, (t % 60) as u8))
            }
            _ => None,
        }
    }
}

/// Parse a timetable document.
///
/// Fails on the **first** problem, with its line: in a document a person edits, the errors after a
/// typo are usually that typo again.
///
/// # Examples
///
/// ```
/// let doc = timetable::parse("# housekeeping\nevery 30s  least_authority_demo 7\nat-boot  least_authority_demo 3\n")
///     .expect("that document is well formed");
/// assert_eq!(doc.entries().len(), 2);
/// assert_eq!(doc.entries()[0].schedule, timetable::Schedule::Every(30 * timetable::NANOS_PER_SEC));
/// assert_eq!(doc.entries()[1].schedule, timetable::Schedule::AtBoot);
/// assert_eq!(doc.entries()[1].command, b"least_authority_demo 3");
///
/// // A zero interval is refused rather than run, and the refusal names the line.
/// let bad = timetable::parse("every 0s least_authority_demo 7\n").unwrap_err();
/// assert_eq!(bad.line(), 1);
/// assert_eq!(bad.message(), "a zero interval is a spin, not a schedule");
/// ```
pub fn parse(doc: &str) -> Result<Document<'_>, Error> {
    let mut entries = [Entry {
        line: 0,
        schedule: Schedule::AtBoot,
        command: b"",
    }; MAX_ENTRIES];
    let mut n = 0usize;

    for (i, raw) in doc.lines().enumerate() {
        let no = i + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        let (word, rest) = split_word(line).ok_or(Error::Malformed(no))?;
        let (schedule, command) = match word {
            "at-boot" => (Schedule::AtBoot, rest),
            "every" => match recurrence::parse(rest).map_err(|r| Error::Calendar(no, r))? {
                recurrence::Parsed::Calendar(rule, command) => (Schedule::Calendar(rule), command),
                recurrence::Parsed::NotCalendar => {
                    let (interval, command) = split_word(rest).ok_or(Error::MissingInterval(no))?;
                    let nanos = interval_nanos(interval).ok_or(Error::BadInterval(no))?;
                    if nanos == 0 {
                        return Err(Error::ZeroInterval(no));
                    }
                    (Schedule::Every(nanos), command)
                }
            },
            _ => return Err(Error::UnknownSchedule(no)),
        };
        let command = command.trim();
        if command.is_empty() {
            return Err(Error::MissingCommand(no));
        }
        if n == MAX_ENTRIES {
            return Err(Error::TooManyEntries(no));
        }
        entries[n] = Entry {
            line: no,
            schedule,
            command: command.as_bytes(),
        };
        n += 1;
    }
    Ok(Document { entries, n })
}

impl<'a> Document<'a> {
    /// The entries, in the order the document wrote them.
    pub fn entries(&self) -> &[Entry<'a>] {
        &self.entries[..self.n]
    }
}

/// Everything up to a `#`. No escape, because a schedule document has no use for a literal `#` and
/// an escape nobody needs is a rule everybody has to know.
fn strip_comment(line: &str) -> &str {
    match line.split_once('#') {
        Some((before, _)) => before,
        None => line,
    }
}

/// The first whitespace-delimited word and everything after it, or `None` for an empty string.
fn split_word(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    if s.is_empty() {
        return None;
    }
    match s.find(char::is_whitespace) {
        Some(i) => Some((&s[..i], &s[i..])),
        None => Some((s, "")),
    }
}

/// `<digits><unit>` in nanoseconds, where the unit is `ms`, `s` or `m`.
///
/// Three units and no more. Anything shorter than a millisecond is below the resolution a
/// yield-polled scheduler can honour (see `components/src/timetable.rs`'s `BUGS`), and anything longer
/// than a minute is better said as a calendar line: `every 86400s` is a daily job counted from
/// arming, where `every day at 02:00` is one at a time of day.
fn interval_nanos(s: &str) -> Option<u64> {
    let digits = s.len() - s.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 {
        return None;
    }
    let value: u64 = s[..digits].parse().ok()?;
    let per = match &s[digits..] {
        "ms" => NANOS_PER_SEC / 1000,
        "s" => NANOS_PER_SEC,
        "m" => 60 * NANOS_PER_SEC,
        _ => return None,
    };
    value.checked_mul(per)
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::string::String;

    use super::*;
    use crate::{Held, Registry};

    #[test]
    fn comments_and_blank_lines_and_spacing_are_ignored() {
        let doc = parse(
            "# a heading\n\
             \n\
             every   30s     least_authority_demo 7   # trailing\n\
             \n\
             at-boot least_authority_demo 3\n",
        )
        .unwrap();
        assert_eq!(doc.entries().len(), 2);
        assert_eq!(doc.entries()[0].command, b"least_authority_demo 7");
        assert_eq!(doc.entries()[0].line, 3);
        assert_eq!(doc.entries()[1].command, b"least_authority_demo 3");
        assert_eq!(doc.entries()[1].line, 5);
    }

    #[test]
    fn an_empty_document_is_a_timetable_with_nothing_in_it() {
        let doc = parse("# nothing scheduled yet\n").unwrap();
        assert_eq!(doc.entries().len(), 0);
        let reg = Registry::register(&doc, Held::default());
        assert_eq!(reg.admitted(), 0);
        assert_eq!(reg.next_deadline(), None);
    }

    #[test]
    fn every_interval_unit_converts_once_and_correctly() {
        let doc = parse("every 250ms x\nevery 30s x\nevery 5m x\n").unwrap();
        assert_eq!(doc.entries()[0].schedule, Schedule::Every(250_000_000));
        assert_eq!(
            doc.entries()[1].schedule,
            Schedule::Every(30 * NANOS_PER_SEC)
        );
        assert_eq!(
            doc.entries()[2].schedule,
            Schedule::Every(300 * NANOS_PER_SEC)
        );
    }

    #[test]
    fn each_error_points_at_the_line_that_is_wrong() {
        let head = "# heading\nat-boot least_authority_demo 3\n";
        for (bad, want) in [
            ("weekly least_authority_demo 7\n", Error::UnknownSchedule(3)),
            ("every\n", Error::MissingInterval(3)),
            (
                "every fortnight least_authority_demo 7\n",
                Error::BadInterval(3),
            ),
            ("every 30 least_authority_demo 7\n", Error::BadInterval(3)),
            ("every 30d least_authority_demo 7\n", Error::BadInterval(3)),
            ("every 0s least_authority_demo 7\n", Error::ZeroInterval(3)),
            ("every 0ms least_authority_demo 7\n", Error::ZeroInterval(3)),
            ("every 30s\n", Error::MissingCommand(3)),
            ("at-boot\n", Error::MissingCommand(3)),
        ] {
            let mut doc = String::from(head);
            doc.push_str(bad);
            assert_eq!(parse(&doc), Err(want), "for {bad:?}");
        }
    }

    #[test]
    fn more_entries_than_the_table_holds_is_refused_rather_than_dropped() {
        let mut doc = String::new();
        for _ in 0..MAX_ENTRIES + 1 {
            doc.push_str("every 30s least_authority_demo 7\n");
        }
        assert_eq!(parse(&doc), Err(Error::TooManyEntries(MAX_ENTRIES + 1)));
    }

    /// **The accessor, as distinct from the variant.**
    /// `each_error_points_at_the_line_that_is_wrong` compares whole `Error` values and never calls
    /// `Error::line`, which is what a reader is sent to the file with.
    #[test]
    fn error_line_reads_the_number_each_variant_carries() {
        let errs = [
            Error::Malformed(2),
            Error::UnknownSchedule(3),
            Error::MissingInterval(4),
            Error::BadInterval(5),
            Error::ZeroInterval(6),
            Error::MissingCommand(7),
            Error::TooManyEntries(8),
        ];
        for (i, e) in errs.into_iter().enumerate() {
            assert_eq!(e.line(), i + 2, "{e:?}");
        }
        // And it is the document's line rather than the entry's index: the comment counts.
        assert_eq!(parse("# heading\nevery\n").unwrap_err().line(), 2);
        assert_eq!(parse("# heading\nevery 5\n").unwrap_err().line(), 2);
    }

    // ---- survivor triage of the 2026-10-03 census ---------------------------------------------

    #[test]
    fn a_range_refusal_names_the_line_to_write_instead() {
        let e = Error::Calendar(4, recurrence::Refusal::InexactRange(Some(17 * 60 + 45)));
        assert_eq!(e.last_reachable(), Some((17, 45)));
        let none = Error::Calendar(4, recurrence::Refusal::InexactRange(None));
        assert_eq!(none.last_reachable(), None);
        assert_eq!(Error::ZeroInterval(4).last_reachable(), None);
    }
}
