//! Machine-checked proofs for the calendar arithmetic (milestone 129 (scheduled execution)). The
//! proofs of `next_after` moved with the function to `crates/periodic_deadline/src/proofs.rs`
//! (milestone 360).
//!
//! A wrong calendar answer is quiet: a monthly job a week off, a time of day listed twice. The
//! inputs that reach those are ones nobody thinks to type, so Kani holds them rather than samples.

// ===============================================================================================
// Calendar entries (G5): the parts of the next-occurrence arithmetic whose wrong answers are quiet.
// ===============================================================================================
//
// `recurrence::next` walks days in order and asks `day_matches` of each, so "no matching day is
// skipped" holds by construction and is checked against dateutil in the host tests. What is proved
// here is everything inside a day and inside a month: the time-of-day selection, the month-day
// predicates against their definitions, and the range grammar's claim that a range is exactly one
// RRULE. Every quantity is 32 bits or less and no modulo is 64-bit, which is the path
// `next_after`'s phase harness stalled on (see notes/periodic-deadline-proofs.md).

use crate::recurrence::{MonthDay, month_day_matches, next_time, range};

/// **A time of day is strictly later than `after`, and both its hour and its minute are listed.**
///
/// The polling loop asks `next` after each fire with the fire's own minute, so "strictly later" is
/// what stops one occurrence firing twice; "listed" is what makes every fire an occurrence.
/// Falsification: replayable `crates/timetable/falsifications/proofs.a_time_of_day_is_strictly_later_and_listed.patch`
#[kani::proof]
#[kani::unwind(25)]
fn a_time_of_day_is_strictly_later_and_listed() {
    let hours: u32 = kani::any();
    let minutes: u64 = kani::any();
    let after: i32 = kani::any();
    kani::assume((-1..1440).contains(&after));
    if let Some(t) = next_time(hours, minutes, after) {
        assert!((t as i32) > after);
        assert!(t < 1440);
        assert!(hours & (1 << (t / 60)) != 0);
        assert!(minutes & (1 << (t % 60)) != 0);
    }
}

/// **Nothing listed lies between `after` and the answer**, and when there is no answer, nothing
/// listed lies after `after` in the day at all. Together with the harness above, the answer is the
/// first listed time strictly later, which is RFC 5545's expansion for one day.
/// Falsification: replayable `crates/timetable/falsifications/proofs.a_time_of_day_skips_nothing.patch`
#[kani::proof]
#[kani::unwind(25)]
fn a_time_of_day_skips_nothing() {
    let hours: u32 = kani::any();
    let minutes: u64 = kani::any();
    let after: i32 = kani::any();
    let x: u16 = kani::any();
    kani::assume((-1..1440).contains(&after));
    kani::assume(x < 1440 && (x as i32) > after);
    let listed = hours & (1 << (x / 60)) != 0 && minutes & (1 << (x % 60)) != 0;
    match next_time(hours, minutes, after) {
        Some(t) => assert!(!(listed && x < t)),
        None => assert!(!listed),
    }
}

/// The weekday (Monday 0) of day `dom` in a month whose 1st falls on `first`.
fn dow_of(first: u8, dom: u8) -> u8 {
    (first + (dom - 1) % 7) % 7
}

/// **`<n><dow>` is the n-th such weekday of the month, and `last <dow>` the last**, checked against
/// counting them one day at a time. The closed form `(dom - 1) / 7 + 1` is where an off-by-one would
/// quietly move a monthly job a week.
/// Falsification: replayable `crates/timetable/falsifications/proofs.an_nth_weekday_is_counted_from_its_month.patch`
#[kani::proof]
#[kani::unwind(33)]
fn an_nth_weekday_is_counted_from_its_month() {
    let first: u8 = kani::any();
    let len: u8 = kani::any();
    let dom: u8 = kani::any();
    let want: u8 = kani::any();
    let n: i8 = kani::any();
    kani::assume(first < 7 && (28..=31).contains(&len) && dom >= 1 && dom <= len && want < 7);
    kani::assume((1..=4).contains(&n) || n == -1);
    let dow = dow_of(first, dom);
    // By counting: how many days up to and including `dom` share its weekday, and whether a later
    // day in the month does.
    let mut before = 0u8;
    let mut later = false;
    let mut d = 1u8;
    while d <= len {
        if dow_of(first, d) == dow {
            if d <= dom {
                before += 1;
            } else {
                later = true;
            }
        }
        d += 1;
    }
    let by_counting = dow == want && if n > 0 { before == n as u8 } else { !later };
    assert_eq!(
        month_day_matches(MonthDay::Nth(n, want), dom, dow, len),
        by_counting
    );
}

/// **The first and last weekdays of a month are what their names say**: a Monday-to-Friday with no
/// Monday-to-Friday before it, or after it, in the month. The closed forms look at the 1st and at
/// the month's end only, which is exactly the kind of shortcut that is right for 27 months in 28.
/// Falsification: replayable `crates/timetable/falsifications/proofs.the_first_and_last_weekdays_are_what_they_say.patch`
#[kani::proof]
#[kani::unwind(33)]
fn the_first_and_last_weekdays_are_what_they_say() {
    let first: u8 = kani::any();
    let len: u8 = kani::any();
    let dom: u8 = kani::any();
    kani::assume(first < 7 && (28..=31).contains(&len) && dom >= 1 && dom <= len);
    let dow = dow_of(first, dom);
    let mut weekday_before = false;
    let mut weekday_after = false;
    let mut d = 1u8;
    while d <= len {
        if dow_of(first, d) < 5 {
            if d < dom {
                weekday_before = true;
            }
            if d > dom {
                weekday_after = true;
            }
        }
        d += 1;
    }
    assert_eq!(
        month_day_matches(MonthDay::FirstWeekday, dom, dow, len),
        dow < 5 && !weekday_before
    );
    assert_eq!(
        month_day_matches(MonthDay::LastWeekday, dom, dow, len),
        dow < 5 && !weekday_after
    );
}

/// **An accepted range fires on exactly `start + k * step` up to `end`**, which is the ruling's
/// claim that `from .. to .. by` is one RRULE: BYHOUR times BYMINUTE, a product, equal to the range a
/// person wrote. A product that also held 17:00, or dropped 09:00, is what the exactness refusals
/// exist to prevent.
/// Falsification: replayable `crates/timetable/falsifications/proofs.a_range_is_exactly_its_steps.patch`
#[kani::proof]
#[kani::unwind(62)]
fn a_range_is_exactly_its_steps() {
    let start: u16 = kani::any();
    let end: u16 = kani::any();
    let step: u16 = kani::any();
    let t: u16 = kani::any();
    kani::assume(start < 1440 && end < 1440 && t < 1440);
    kani::assume(step > 0 && step < 60);
    if let Ok((hours, minutes)) = range(start, end, step) {
        let fires = hours & (1 << (t / 60)) != 0 && minutes & (1 << (t % 60)) != 0;
        let in_range = t >= start && t <= end && (t - start).is_multiple_of(step);
        assert_eq!(fires, in_range);
    }
}
