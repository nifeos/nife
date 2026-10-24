//! Tests of `time`: the duration, and the three reasons there is not one, with the duration
//! survivor of the 2026-10-03 mutation census.

use super::*;
use crate::test_support::shown;

/// **The unit boundaries, which are the only place this arithmetic can be wrong.**
///
/// Each pair is a value just under a boundary and the one just over it, because a rendering that
/// picks its unit with the wrong comparison is right everywhere else. The zero-padding is here
/// for the same reason: `4.13 ms` looks like a number and is a different one from `4.013 ms`.
#[test]
fn a_duration_reads_in_the_largest_unit_that_keeps_it_readable() {
    let shown_ns = |nanos| shown(|o| write_duration(nanos, o));

    assert_eq!(shown_ns(0), "0.000 us");
    assert_eq!(shown_ns(999_999), "999.999 us");
    assert_eq!(shown_ns(1_000_000), "1.000 ms");
    assert_eq!(shown_ns(999_999_999), "999.999 ms");
    assert_eq!(shown_ns(1_000_000_000), "1.000 s");
    // The padding, at each width, because the digits come from a writer that omits leading zeros.
    assert_eq!(shown_ns(1_001_000), "1.001 ms");
    assert_eq!(shown_ns(1_010_000), "1.010 ms");
    assert_eq!(shown_ns(1_100_000), "1.100 ms");
    // Sub-tick values still render: the counter's resolution is a fact about the input, and a
    // renderer that refused small numbers would be lying about a different thing.
    assert_eq!(shown_ns(7), "0.007 us");

    // **And it is total on every `u64`.** The shell hands this `end - start` off a counter it
    // does not own, so a clock that went backwards produces a number nobody chose. Rendering
    // it is the right answer; panicking on the boot path is not.
    assert_eq!(shown_ns(u64::MAX), "18446744073.709 s");
}

/// **A timing line is one line and says `real`**, and after §72 there is no second line it can
/// grow: the stepped-clock disclaimer went with the clock, because a monotonic counter cannot
/// be stepped.
#[test]
fn a_timing_line_reports_the_duration_and_nothing_else() {
    let line = shown(|o| write_timing(4_213_000, o));
    assert_eq!(line, "  time: real 4.213 ms\n");
    // `real` is Unix's word, and the absence of `user` and `sys` is the honest part: nothing in
    // this kernel is asked what a thread spent.
    assert!(!line.contains("user"));
    assert!(!line.contains("sys"));
    // A monotonic counter cannot be stepped, so there is nothing to disclaim.
    assert!(!line.contains("stepped"));
}

/// **The one refusal left is a usage error**, not a capability one. `time` reads an ambient
/// counter (§72), so there is no machine on which it must decline to measure.
#[test]
fn an_untimed_line_says_the_one_reason_it_can_be() {
    let said = |r| shown(|o| write_untimed(r, o));
    assert!(said(Untimed::NothingToTime).contains("name a command to time"));
    // Attributed, because a bare sentence at a prompt reads as the command's complaint rather
    // than the shell's.
    assert!(
        said(Untimed::NothingToTime).starts_with("  time: "),
        "{:?}",
        said(Untimed::NothingToTime)
    );
}

/// Durations are `whole.fraction unit`, the fraction being the remainder over the next unit
/// down. A remainder is `%`, so a quotient or a sum in its place changes every row below.
#[test]
fn a_duration_shows_the_remainder_of_its_unit_as_thousandths() {
    let d = |n| shown(|o| write_duration(n, o));
    assert_eq!(d(0), "0.000 us");
    assert_eq!(d(1_234), "1.234 us");
    assert_eq!(d(4_213_000), "4.213 ms");
    assert_eq!(d(4_013_999), "4.013 ms");
    assert_eq!(d(90_500_000_000), "90.500 s");
    assert_eq!(d(3_001_999_999), "3.001 s");
}
