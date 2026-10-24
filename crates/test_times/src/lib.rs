//! **What each kernel test cost, as the lines the suite prints and `xtask` reads** (milestone 807
//! (the kernel suite reports what each test cost), §254 (a gate prints what each item cost), Fork 1).
//!
//! The kernel's test runner stamps the timer before every test and, after the suite's other
//! reports and before `test result:`, prints a block on the console, the one channel all three
//! architectures share (§19 (architectural parity is a tenet)). The kernel prints it and `xtask`
//! parses it, so the grammar is a crate (rule 7). See notes/system-tests-and-the-kernel-crate.md,
//! "What each test cost, and its budget".

//! # The grammar
//!
//! A line is `time`, one space, the elapsed milliseconds in decimal, one space, the test's full
//! path as `core::any::type_name` gives it. No other spacing is accepted, and a path may not
//! contain whitespace. A trailing `\r` is tolerated because a serial transcript may carry one. The
//! block opens with [`HEADING`] on a line of its own, and `xtask` reads record lines only after it.

//! # EXAMPLES
//!
//! ```text
//! time per test, in milliseconds:
//! time 3 kernel::memory::tests::frames_are_zeroed
//! time 10512 system_tests::timetable_tests::a_calendar_entry_keeps_time_by_its_granted_clock
//! ```

//! # BUGS
//!
//! - The figure is guest time: under TCG a slow runner reads as a slow test, and `xtask`'s
//!   cross-check bounds the guest clock against the host's, not against a fair machine.
//! - The heading is a wording, not a rung of [`boot_ladder`]'s kind, and nothing else keys on it.
//!   Both programs take it from here, so changing it is a compile error rather than a silent drift.
//!
//! [`boot_ladder`]: ../boot_ladder/index.html
//!
//! Name: provisional, minted 2026-10-07 (UTC) by milestone 807's lane: a plain plural noun for what
//! it holds. Refused `test_time_record` (`script/lint` reads a name ending in `d` as a claim,
//! §39 (a component is named for what it is)). Refused `test_timing` (reads as a mechanism). Refused
//! `test_cost` (would claim more than elapsed time; the frame ledger is a per-test cost too).

#![no_std]

use core::fmt;

/// The line that opens the record block. Printed once per image, after the suite's other reports.
pub const HEADING: &str = "time per test, in milliseconds:";

/// What opens a repeated run's transcript (`NIFE_TEST_REPEAT`), followed by the count and
/// ` times`. `xtask` reads it to know the record holds a sum over runs.
///
/// Name: provisional, minted 2026-10-22 (UTC) by lane/321-repeat-count.
pub const REPEAT_ANNOUNCEMENT: &str = "repeat: each selected test runs";

/// What every record line starts with, its trailing space included so a match is on a boundary.
pub const PREFIX: &str = "time ";

/// One test's record: how long it ran, and which test it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line<'a> {
    /// The elapsed time from the stamp the runner takes before the test body to the one after it.
    pub milliseconds: u64,
    /// The test's full path, `core::any::type_name` of the test function.
    pub path: &'a str,
}

impl fmt::Display for Line<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{PREFIX}{} {}", self.milliseconds, self.path)
    }
}

/// Why a line is not a record line. Each names the first thing that was wrong.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// The line does not start with [`PREFIX`].
    NoPrefix,
    /// The milliseconds field is empty, is not all decimal digits, or does not fit a `u64`.
    BadMilliseconds,
    /// There is no test path after the milliseconds, or it is separated by something other than
    /// exactly one space.
    NoPath,
    /// The path contains whitespace, so it is not one `type_name`.
    SpaceInPath,
}

/// Read one record line.
///
/// # Errors
///
/// [`Malformed`] names the first thing wrong. A line that is not a record line at all is
/// [`Malformed::NoPrefix`], so a caller scanning a block can tell "not mine" from "mine but bad".
///
/// # Examples
///
/// ```
/// use test_times::{Line, parse};
///
/// let line = Line { milliseconds: 42, path: "kernel::sched::tests::a_thread_runs" };
/// let mut printed = String::new();
/// core::fmt::write(&mut printed, format_args!("{line}")).unwrap();
/// assert_eq!(printed, "time 42 kernel::sched::tests::a_thread_runs");
/// assert_eq!(parse(&printed), Ok(line));
/// ```
pub fn parse(line: &str) -> Result<Line<'_>, Malformed> {
    let line = line.strip_suffix('\r').unwrap_or(line);
    let rest = line.strip_prefix(PREFIX).ok_or(Malformed::NoPrefix)?;
    let (digits, path) = rest.split_once(' ').ok_or(Malformed::NoPath)?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Malformed::BadMilliseconds);
    }
    let milliseconds = digits.parse().map_err(|_| Malformed::BadMilliseconds)?;
    if path.is_empty() || path.starts_with(' ') {
        return Err(Malformed::NoPath);
    }
    if path.chars().any(char::is_whitespace) {
        return Err(Malformed::SpaceInPath);
    }
    Ok(Line { milliseconds, path })
}

/// **The default per-test wall-clock budget, in seconds.** Deliberately tight: almost every test in
/// this suite is milliseconds, and a handful of the userspace ones are a few seconds. 90 s is far
/// above anything honest while still being a real net, so a two-second unit test that starts
/// spinning fails in a minute and a half rather than never.
///
/// Here rather than in `kernel/src/testing.rs` (2026-10-11 (UTC), lane
/// `lane/a-hung-test-fails-with-its-name`) because two programs now read it under rule 7: the
/// kernel's per-test ceiling enforces it in guest time, and `xtask`'s host watchdog
/// (`time_record::bound_for`) enforces it plus a grace in host time, for the guest that can no
/// longer enforce anything. One table, so a test that declares a longer budget gets it from both.
pub const DEFAULT_BUDGET_SECS: u64 = 90;

/// **The known-slow tests, each declaring its own cost.** A test whose honest runtime exceeds
/// [`DEFAULT_BUDGET_SECS`] must say so here, with the reason, so the exception is visible and
/// reviewable. Matched as a substring of the full test path, so the entry covers a test on every
/// architecture. Keep budgets roughly 2x the measured time, taken from the kernel's report of any
/// test over 5 s or from the `time per test` record. Empty since 2026-10-11 (UTC); the measurement
/// behind that is in notes/system-tests-and-the-kernel-crate.md.
pub const SLOW_TESTS: &[(&str, u64)] = &[];

/// The budget for a test, in seconds, by its full path: its [`SLOW_TESTS`] entry if it has one,
/// else [`DEFAULT_BUDGET_SECS`]. When two entries match, the later one wins.
///
/// # EXAMPLES
///
/// ```
/// use test_times::{DEFAULT_BUDGET_SECS, budget_secs_for};
///
/// assert_eq!(budget_secs_for("kernel::memory::tests::frames_are_zeroed"), DEFAULT_BUDGET_SECS);
/// ```
#[must_use]
pub fn budget_secs_for(path: &str) -> u64 {
    let mut secs = DEFAULT_BUDGET_SECS;
    for &(needle, allowed) in SLOW_TESTS {
        if path.contains(needle) {
            secs = allowed;
        }
    }
    secs
}

/// Counter ticks to whole milliseconds, rounding down.
///
/// In 128 bits because the product can outgrow 64: a 900 s ceiling on a 3 GHz TSC is 2.7e12 ticks,
/// times 1000 is 2.7e15, which fits, but nothing here should depend on a ceiling staying small. A
/// zero frequency (a timer that never calibrated) gives zero rather than a division trap, and
/// `xtask`'s cross-check is what notices a clock that lies.
#[must_use]
pub fn milliseconds(ticks: u64, frequency: u64) -> u64 {
    if frequency == 0 {
        return 0;
    }
    let ms = u128::from(ticks) * 1000 / u128::from(frequency);
    u64::try_from(ms).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::string::String;

    use super::*;

    fn printed(line: Line<'_>) -> String {
        let mut s = String::new();
        fmt::write(&mut s, format_args!("{line}")).unwrap();
        s
    }

    #[test]
    fn a_printed_line_parses_back_to_itself() {
        let line = Line {
            milliseconds: 10_512,
            path: "system_tests::timetable_tests::a_calendar_entry",
        };
        assert_eq!(parse(&printed(line)), Ok(line));
        let zero = Line {
            milliseconds: 0,
            path: "kernel::a",
        };
        assert_eq!(parse(&printed(zero)), Ok(zero));
    }

    #[test]
    fn a_serial_carriage_return_is_tolerated() {
        assert_eq!(
            parse("time 7 kernel::x\r"),
            Ok(Line {
                milliseconds: 7,
                path: "kernel::x"
            })
        );
    }

    #[test]
    fn malformed_lines_are_refused_with_the_first_fault() {
        assert_eq!(parse("test kernel::x ... ok"), Err(Malformed::NoPrefix));
        assert_eq!(
            parse("time per test, in milliseconds:"),
            Err(Malformed::BadMilliseconds)
        );
        assert_eq!(parse("time 12"), Err(Malformed::NoPath));
        assert_eq!(parse("time  12 kernel::x"), Err(Malformed::BadMilliseconds));
        assert_eq!(parse("time 12  kernel::x"), Err(Malformed::NoPath));
        assert_eq!(parse("time 12 "), Err(Malformed::NoPath));
        assert_eq!(parse("time -1 kernel::x"), Err(Malformed::BadMilliseconds));
        assert_eq!(parse("time +1 kernel::x"), Err(Malformed::BadMilliseconds));
        assert_eq!(parse("time 1.5 kernel::x"), Err(Malformed::BadMilliseconds));
        assert_eq!(
            parse("time 99999999999999999999 kernel::x"),
            Err(Malformed::BadMilliseconds)
        );
        assert_eq!(parse("time 3 kernel::x ok"), Err(Malformed::SpaceInPath));
        assert_eq!(parse("time 3 kernel::x\tok"), Err(Malformed::SpaceInPath));
    }

    #[test]
    fn the_heading_is_not_a_record_line() {
        // xtask reads record lines only after the heading; the heading itself must never parse.
        assert!(parse(HEADING).is_err());
    }

    #[test]
    fn a_declared_slow_test_gets_its_own_budget_and_every_other_test_the_default() {
        assert_eq!(budget_secs_for("kernel::a::b"), DEFAULT_BUDGET_SECS);
        for &(needle, secs) in SLOW_TESTS {
            assert_eq!(
                budget_secs_for(&std::format!("system_tests::x::{needle}")),
                secs
            );
        }
    }

    #[test]
    fn ticks_convert_to_milliseconds_without_overflow() {
        assert_eq!(milliseconds(62_500_000, 62_500_000), 1000);
        assert_eq!(milliseconds(9_999_999, 10_000_000_000), 0);
        assert_eq!(milliseconds(15, 10_000), 1);
        assert_eq!(milliseconds(u64::MAX, 1), u64::MAX);
        assert_eq!(milliseconds(u64::MAX, 3_000_000_000), u64::MAX / 3_000_000);
        assert_eq!(milliseconds(5, 0), 0);
    }
}
