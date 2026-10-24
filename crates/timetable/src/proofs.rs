//! Machine-checked proofs for the firing arithmetic (milestone 129 (scheduled execution)).
//!
//! The arithmetic rather than the parser: a wrong parse is a document that does not load, which is
//! loud, while a wrong fire is quiet (an occurrence fired twice at a period boundary, a phase that
//! drifts, a `+ period` that wraps a deadline into the past and turns the scheduler into a spin),
//! and the inputs that reach those are ones nobody thinks to type. Phase preservation is
//! host-tested at values up to `u64::MAX / 4`, not proved: a second 64-bit modulo is where CBMC
//! stops finishing, and the snapping expression can produce nothing but the phase. See
//! notes/scheduled-execution/next-after-proofs.md.

use crate::next_after;

/// Nanoseconds this system could plausibly be dealing with: `1 << 50`, about thirteen days, which is
/// longer than this system has ever stayed up.
///
/// The bound exists to keep `saturating_add` out of the proofs rather than to keep them tractable:
/// at `u64::MAX` the saturation is the *correct* answer, and asserting "strictly greater than now"
/// against it would be asserting a falsehood. What the bound says is "on any machine that has not
/// been up for thirty-eight thousand years", which is the regime these properties are about.
const HORIZON: u64 = 1 << 50;

/// **A fire is always strictly in the future**, so a polling loop cannot fire the same occurrence
/// twice however often it looks.
///
/// This is the property the `+ 1` in `next_after` exists for, and the one a `>=` comparison would
/// have broken exactly on period boundaries: the case a hand-written test is least likely to pick
/// and a polling scheduler hits constantly.
/// Falsification: replayable `crates/timetable/falsifications/proofs.a_fire_is_strictly_in_the_future.patch`
#[kani::proof]
fn a_fire_is_strictly_in_the_future() {
    let prev: u64 = kani::any();
    let period: u64 = kani::any();
    let now: u64 = kani::any();
    kani::assume(period > 0 && period <= HORIZON);
    kani::assume(prev <= HORIZON);
    kani::assume(now <= HORIZON);

    let next = next_after(prev, period, now);
    assert!(next > now);
}

/// **A fire is always at least one whole period on from the last one**, which is what stops a
/// schedule from tightening under load: an entry that fired late must not have its next occurrence
/// pulled forward to compensate.
///
/// Its own harness rather than a second assertion on the one above, because the two claims have
/// very different costs and pairing them would make the cheap one wait for the expensive one every
/// time the suite runs.
/// Falsification: replayable `crates/timetable/falsifications/proofs.a_fire_is_at_least_one_whole_period_on.patch`
#[kani::proof]
fn a_fire_is_at_least_one_whole_period_on() {
    let prev: u64 = kani::any();
    let period: u64 = kani::any();
    let now: u64 = kani::any();
    kani::assume(period > 0 && period <= HORIZON);
    kani::assume(prev <= HORIZON);
    kani::assume(now <= HORIZON);

    assert!(next_after(prev, period, now) >= prev + period);
}

/// **A stall is skipped, not caught up.** One call advances past `now` by at most one period, so an
/// entry that missed two hundred occurrences fires once rather than two hundred times.
///
/// The bound is the whole claim: `next <= now + period` says that no matter how far behind the
/// scheduler fell, exactly one occurrence survives the gap. Without it the honest implementation
/// and a catch-up implementation would satisfy every other property here.
/// Falsification: replayable `crates/timetable/falsifications/proofs.a_stall_costs_one_fire_and_not_a_backlog.patch`
#[kani::proof]
fn a_stall_costs_one_fire_and_not_a_backlog() {
    let prev: u64 = kani::any();
    let period: u64 = kani::any();
    let now: u64 = kani::any();
    kani::assume(period > 0 && period <= HORIZON);
    kani::assume(prev <= HORIZON);
    kani::assume(now <= HORIZON);
    // The only regime the bound is claimed in: time has not gone backwards since the last fire was
    // scheduled. `due` never calls it otherwise, because it only advances a row it found overdue.
    kani::assume(now >= prev);

    let next = next_after(prev, period, now);
    assert!(next <= now + period);
}

/// **It is minimal**, which is what makes "the next fire" a true description rather than "a fire
/// somewhere later". Nothing congruent to `prev` sits between `now` and the answer.
///
/// Stated as: one period earlier is not in the future. Together with the phase property above, that
/// pins the answer exactly, and it is the property that would catch a `+ 2` where the `+ 1` is.
/// Falsification: replayable `crates/timetable/falsifications/proofs.nothing_on_the_beat_is_skipped_between_now_and_the_answer.patch`
#[kani::proof]
fn nothing_on_the_beat_is_skipped_between_now_and_the_answer() {
    let prev: u64 = kani::any();
    let period: u64 = kani::any();
    let now: u64 = kani::any();
    kani::assume(period > 0 && period <= HORIZON);
    kani::assume(prev <= HORIZON);
    kani::assume(now <= HORIZON);
    kani::assume(now >= prev);

    let next = next_after(prev, period, now);
    // `next - period` is the previous occurrence on the same beat. It must not be a fire we could
    // have returned instead, which means it is at or before `now`, or it is before the first
    // occurrence after `prev` at all.
    assert!(next - period <= now);
}

/// **A zero period never fires**, rather than dividing by zero.
///
/// `parse` refuses `every 0s`, so no document reaches this; the proof is here because the function
/// is `pub` and a caller who did not read the doc comment is exactly the caller a total function
/// exists for.
/// Falsification: replayable `crates/timetable/falsifications/proofs.a_zero_period_is_never_due.patch`
#[kani::proof]
fn a_zero_period_is_never_due() {
    let prev: u64 = kani::any();
    let now: u64 = kani::any();
    assert_eq!(next_after(prev, 0, now), u64::MAX);
}

// ===============================================================================================
// Calendar entries (G5): the parts of the next-occurrence arithmetic whose wrong answers are quiet.
// ===============================================================================================
//
// `recurrence::next` walks days in order and asks `day_matches` of each, so "no matching day is
// skipped" holds by construction and is checked against dateutil in the host tests. What is proved
// here is everything inside a day and inside a month: the time-of-day selection, the month-day
// predicates against their definitions, and the range grammar's claim that a range is exactly one
// RRULE. Every quantity is 32 bits or less and no modulo is 64-bit, which is the path
// `next_after`'s phase harness stalled on (see the module docs above).

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
