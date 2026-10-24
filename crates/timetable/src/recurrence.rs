//! **Calendar entries: G5's grammar, each line exactly one RFC 5545 RRULE** (milestone 129
//! (scheduled execution); calef ruled "G5 full" on 2026-09-26).
//!
//! A line parses into a [`Rule`], and [`next`] answers the first occurrence strictly after a
//! minute. Every field RFC 5545 would derive from DTSTART is written in the line, so occurrences
//! do not depend on registration (rule 4 of notes/scheduled-execution/calendar-grammar-g5.md). A
//! **minute** counts whole UTC minutes since 1970-01-01T00:00Z, a **day** is that over 1440, and
//! civil dates are `crates/calendar`'s, which Kani proves inverse over its whole range. No seconds
//! (BYSECOND=0) and no zones: a line means UTC forever (rule 5).
//!
//! Name: provisional, minted 2026-09-26 (UTC) by milestone 129's lane, for this module and every
//! public item in it. Naming is calef's.

use calendar::{Civil, days_in_month};

/// Minutes in a day.
pub(crate) const MINUTES_PER_DAY: i64 = 1440;

/// How far [`next`] looks, in days, before it answers `None`. The longest gap any accepted rule can
/// have is a 99-month count, about 3,013 days, plus a month's slack for a month day; a rule limited
/// with `in` repeats within a year. Past this bound the rule has no next occurrence, which is how a
/// line spent by `through` reads too.
pub(crate) const HORIZON_DAYS: i64 = 99 * 31 + 62;

/// How often a rule repeats: RFC 5545's FREQ.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Freq {
    /// `FREQ=DAILY`.
    Daily,
    /// `FREQ=WEEKLY`, weeks starting Monday (`WKST=MO`).
    Weekly,
    /// `FREQ=MONTHLY`.
    Monthly,
}

/// Which days of a month a monthly rule fires on. Never a day past 28: see the module docs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MonthDay {
    /// Not a monthly rule.
    Any,
    /// `BYMONTHDAY=` these days, bit `d` for day `d`, 1 to 28.
    Days(u32),
    /// `BYMONTHDAY=-1`.
    LastDay,
    /// The month's first Monday-to-Friday: `BYDAY=MO,TU,WE,TH,FR;BYSETPOS=1`.
    FirstWeekday,
    /// The month's last Monday-to-Friday: `BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1`.
    LastWeekday,
    /// `BYDAY=<n><dow>`: `n` is 1 to 4, or -1 for the last; `dow` is 0 for Monday to 6 for Sunday.
    Nth(i8, u8),
}

/// **One parsed calendar line**: exactly one RRULE, with `BYSECOND=0;WKST=MO` implied.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rule {
    /// FREQ.
    pub freq: Freq,
    /// INTERVAL, 1 unless the line has a count (2 to 99).
    pub interval: u8,
    /// For a weekly rule, BYDAY: bit 0 is Monday, bit 6 Sunday. All seven for the others.
    pub weekdays: u8,
    /// For a monthly rule, which days of the month.
    pub on: MonthDay,
    /// BYMONTH: bit 0 is January. All twelve when the line has no `in`.
    pub months: u16,
    /// BYHOUR: bit `h` for hour `h`.
    pub hours: u32,
    /// BYMINUTE: bit `m` for minute `m`.
    pub minutes: u64,
    /// DTSTART's day, when the line has `starting`. No occurrence falls before it.
    pub starting: Option<i64>,
    /// UNTIL's day, when the line has `through`: occurrences on that day still count.
    pub through: Option<i64>,
}

/// Every day of the week.
pub(crate) const ALL_DAYS: u8 = 0x7f;
/// Monday to Friday.
pub(crate) const WEEKDAYS: u8 = 0x1f;
/// Saturday and Sunday.
pub(crate) const WEEKEND: u8 = 0x60;
/// Every month.
pub(crate) const ALL_MONTHS: u16 = 0x0fff;

/// **Why a calendar line is refused.** Each message names what to write instead, because every one
/// of these is a line that means something reasonable and would have been answered quietly and
/// wrongly by a grammar that accepted it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refusal {
    /// A token the grammar does not have where it was expected.
    Malformed,
    /// A day of the month from 29 to 31, which would skip short months silently; `last day` says
    /// month-end.
    DayPast28,
    /// `5th <dow>`.
    FifthWeekday,
    /// A count outside 2 to 99.
    BadCount,
    /// `every 15 minutes`, spelled out: one space from `every 15m`, which runs on the other clock.
    SpelledInterval,
    /// A count with `in`.
    CountWithIn,
    /// A count with no `starting`.
    CountWithoutStarting,
    /// A `starting` date the rule would not fire on.
    StartingOffRule,
    /// `through` before `starting`.
    ThroughBeforeStarting,
    /// Times in one `at` list with different minutes: BYHOUR times BYMINUTE is a product.
    MixedMinutes,
    /// A `by` step that neither divides 60 minutes nor is whole hours.
    StepDoesNotDivide,
    /// A range whose start minute is not below its step, or whose end is not its last step. Carries
    /// the last time the range does reach, as minutes of the day, or `None` if it reaches none.
    InexactRange(Option<u16>),
}

impl Refusal {
    /// The fixed sentence, naming the alternative.
    pub fn message(self) -> &'static str {
        match self {
            Refusal::Malformed => {
                "not a calendar line: `every day|weekday|weekend|week on ..|month on ..` then `at HH:MM`"
            }
            Refusal::DayPast28 => "days 29 to 31 skip short months silently; write `on last day`",
            Refusal::FifthWeekday => "a 5th weekday skips most months; write `on last <day>`",
            Refusal::BadCount => "a count is 2 to 99",
            Refusal::SpelledInterval => {
                "`every N minutes` is written `every Nm`, the interval from arming"
            }
            Refusal::CountWithIn => {
                "a count cannot be limited with `in`; drop the count or the `in`"
            }
            Refusal::CountWithoutStarting => {
                "a count needs `starting <date>`, which fixes its phase"
            }
            Refusal::StartingOffRule => "`starting` must be a date the rule itself fires on",
            Refusal::ThroughBeforeStarting => "`through` is before `starting`",
            Refusal::MixedMinutes => {
                "times in one list share a minute, or it fires on every pairing; write two lines"
            }
            Refusal::StepDoesNotDivide => "a step divides 60 minutes or is whole hours",
            Refusal::InexactRange(_) => {
                "a range starts below its step and ends on its last step; see the plan for the last"
            }
        }
    }
}

/// What [`parse`] found after `every`: a calendar rule and the command, or no calendar cadence at
/// all (the line is an interval, `every 30s`, and the caller reads it as one).
pub enum Parsed<'a> {
    /// A calendar line.
    Calendar(Rule, &'a str),
    /// Not a calendar cadence word; the caller's interval grammar owns this line.
    NotCalendar,
}

/// The day of the week of day number `d`, Monday 0 to Sunday 6. 1970-01-01 was a Thursday.
pub(crate) const fn weekday_of(d: i64) -> u8 {
    (d + 3).rem_euclid(7) as u8
}

/// `(year, month 1..=12, day 1..=31)` of day number `d`. `None` outside `crates/calendar`'s range.
fn civil(d: i64) -> Option<(i32, u8, u8)> {
    let c = Civil::from_unix(d.checked_mul(86_400)?).ok()?;
    Some((c.year(), c.month(), c.day()))
}

/// The day number of a civil date, or `None` if it is not one.
fn day_number(year: i32, month: u8, day: u8) -> Option<i64> {
    Civil::new(year, month, day, 0, 0, 0)
        .ok()
        .map(|c| c.days_since_epoch())
}

/// **Does a month day match**, given the day's number in its month, its weekday, and the month's
/// length. The monthly half of [`day_matches`], kept free of the calendar so Kani can reach it.
pub(crate) fn month_day_matches(on: MonthDay, dom: u8, dow: u8, month_len: u8) -> bool {
    match on {
        MonthDay::Any => true,
        MonthDay::Days(bits) => dom <= 28 && bits & (1 << dom) != 0,
        MonthDay::LastDay => dom == month_len,
        // The first Monday-to-Friday is day 1, 2 or 3: day 1 if it is a weekday, else the Monday
        // after a Saturday or a Sunday. So: a weekday, and no weekday before it in the month.
        MonthDay::FirstWeekday => dow < 5 && (dom == 1 || (dom <= 3 && dow == 0)),
        MonthDay::LastWeekday => {
            dow < 5 && (dom == month_len || (dom + 2 >= month_len && dow == 4))
        }
        MonthDay::Nth(n, want) => {
            dow == want
                && if n > 0 {
                    (dom - 1) / 7 + 1 == n as u8
                } else {
                    dom + 7 > month_len
                }
        }
    }
}

/// **Is day `d` one the rule fires on**, ignoring the time of day. RFC 5545's day-level expansion,
/// written as a predicate: `next` asks it of each day in turn, so no matching day can be skipped.
pub(crate) fn day_matches(rule: &Rule, d: i64) -> bool {
    if rule.starting.is_some_and(|s| d < s) || rule.through.is_some_and(|t| d > t) {
        return false;
    }
    let Some((year, month, dom)) = civil(d) else {
        return false;
    };
    if rule.months & (1 << (month - 1)) == 0 {
        return false;
    }
    let dow = weekday_of(d);
    let fits = match rule.freq {
        Freq::Daily => true,
        Freq::Weekly => rule.weekdays & (1 << dow) != 0,
        Freq::Monthly => {
            let len = days_in_month(year, month).unwrap_or(31);
            month_day_matches(rule.on, dom, dow, len)
        }
    };
    fits && in_phase(rule, d, year, month)
}

/// **Is day `d` in an INTERVAL rule's phase**, counted from `starting`, which the grammar requires
/// whenever the interval is above one.
fn in_phase(rule: &Rule, d: i64, year: i32, month: u8) -> bool {
    let n = rule.interval as i64;
    if n <= 1 {
        return true;
    }
    let Some(s) = rule.starting else {
        return false;
    };
    match rule.freq {
        Freq::Daily => (d - s).rem_euclid(n) == 0,
        Freq::Weekly => {
            // Weeks start Monday, so a week is named by its Monday.
            let week = |x: i64| (x - weekday_of(x) as i64) / 7;
            (week(d) - week(s)).rem_euclid(n) == 0
        }
        Freq::Monthly => {
            let Some((sy, sm, _)) = civil(s) else {
                return false;
            };
            let months = (year as i64 * 12 + month as i64) - (sy as i64 * 12 + sm as i64);
            months.rem_euclid(n) == 0
        }
    }
}

/// **The first time of day strictly after minute-of-day `after`** whose hour is in `hours` and whose
/// minute is in `minutes`, or `None`. `after` is -1 for "from the start of the day".
///
/// Twenty-four iterations at most and one trailing-zero count each, which is what lets Kani prove
/// its three properties over every mask (`proofs::a_time_of_day_is_strictly_later_and_listed`).
pub(crate) fn next_time(hours: u32, minutes: u64, after: i32) -> Option<u16> {
    let minutes = minutes & ((1u64 << 60) - 1);
    let mut h = if after < 0 { 0 } else { (after / 60) as u32 };
    while h < 24 {
        if hours & (1 << h) != 0 {
            // Minutes in this hour that are strictly after `after`.
            let floor = after - (h as i32) * 60; // may be below zero
            let usable = if floor < 0 {
                minutes
            } else if floor >= 59 {
                0
            } else {
                minutes & !((1u64 << (floor + 1)) - 1)
            };
            if usable != 0 {
                return Some((h * 60 + usable.trailing_zeros()) as u16);
            }
        }
        h += 1;
    }
    None
}

/// **The first occurrence strictly after `after`**, in minutes since the epoch, or `None` if there is
/// none within [`HORIZON_DAYS`], which is how a line spent by `through` answers.
///
/// Day by day, asking [`day_matches`] of each, so every matching day is visited in order; within a
/// day, [`next_time`]. The answer is therefore an occurrence, strictly later, and nothing between
/// is one: the three properties the host tests check against dateutil's own expansion.
pub fn next(rule: &Rule, after: i64) -> Option<i64> {
    let day = after.div_euclid(MINUTES_PER_DAY);
    let minute = after.rem_euclid(MINUTES_PER_DAY) as i32;
    if day_matches(rule, day)
        && let Some(t) = next_time(rule.hours, rule.minutes, minute)
    {
        return Some(day * MINUTES_PER_DAY + t as i64);
    }
    let first = next_time(rule.hours, rule.minutes, -1)? as i64;
    let mut d = day + 1;
    if let Some(s) = rule.starting {
        d = d.max(s);
    }
    // The horizon counts from the first day that can match, so a line whose `starting` is years
    // away is dormant until then rather than read as spent.
    let end = d + HORIZON_DAYS;
    while d <= end {
        if rule.through.is_some_and(|t| d > t) {
            return None;
        }
        if day_matches(rule, d) {
            return Some(d * MINUTES_PER_DAY + first);
        }
        d += 1;
    }
    None
}

// ===============================================================================================
// The parser.
// ===============================================================================================

/// The first whitespace-delimited word and the rest.
fn word(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    if s.is_empty() {
        return None;
    }
    match s.find(char::is_whitespace) {
        Some(i) => Some((&s[..i], &s[i..])),
        None => Some((s, "")),
    }
}

fn expect<'a>(s: &'a str, w: &str) -> Result<&'a str, Refusal> {
    match word(s) {
        Some((got, rest)) if got == w => Ok(rest),
        _ => Err(Refusal::Malformed),
    }
}

fn dow(s: &str) -> Option<u8> {
    Some(match s {
        "mon" => 0,
        "tue" => 1,
        "wed" => 2,
        "thu" => 3,
        "fri" => 4,
        "sat" => 5,
        "sun" => 6,
        _ => return None,
    })
}

fn month(s: &str) -> Option<u8> {
    Some(match s {
        "jan" => 0,
        "feb" => 1,
        "mar" => 2,
        "apr" => 3,
        "may" => 4,
        "jun" => 5,
        "jul" => 6,
        "aug" => 7,
        "sep" => 8,
        "oct" => 9,
        "nov" => 10,
        "dec" => 11,
        _ => return None,
    })
}

fn digits(s: &str) -> Option<u32> {
    if s.is_empty() || s.len() > 4 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// A comma-joined list in one token, each item through `item`, or-ed into a mask.
fn list<T: Into<u64>>(token: &str, item: impl Fn(&str) -> Option<T>) -> Option<u64> {
    let mut mask = 0u64;
    for part in token.split(',') {
        mask |= 1u64 << item(part)?.into();
    }
    Some(mask)
}

fn day_list(token: &str) -> Result<u8, Refusal> {
    list(token, dow).map(|m| m as u8).ok_or(Refusal::Malformed)
}

/// `HH:MM`, as minutes of the day.
fn hhmm(s: &str) -> Option<u16> {
    let (h, m) = s.split_once(':')?;
    if h.len() != 2 || m.len() != 2 {
        return None;
    }
    let (h, m) = (digits(h)?, digits(m)?);
    (h < 24 && m < 60).then_some((h * 60 + m) as u16)
}

/// `YYYY-MM-DD`, as a day number.
fn date(s: &str) -> Option<i64> {
    let mut it = s.split('-');
    let (y, m, d) = (it.next()?, it.next()?, it.next()?);
    if it.next().is_some() || y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    day_number(digits(y)? as i32, digits(m)? as u8, digits(d)? as u8)
}

/// A month day clause after `on`, consuming one or two tokens.
fn month_day(s: &str) -> Result<(MonthDay, &str), Refusal> {
    let (first, rest) = word(s).ok_or(Refusal::Malformed)?;
    if first.bytes().next().is_some_and(|b| b.is_ascii_digit())
        && !first.ends_with(|c: char| c.is_ascii_alphabetic())
    {
        let mut bits = 0u32;
        for part in first.split(',') {
            let d = digits(part).ok_or(Refusal::Malformed)?;
            match d {
                1..=28 => bits |= 1 << d,
                29..=31 => return Err(Refusal::DayPast28),
                _ => return Err(Refusal::Malformed),
            }
        }
        return Ok((MonthDay::Days(bits), rest));
    }
    let (second, rest) = word(rest).ok_or(Refusal::Malformed)?;
    let n: i8 = match first {
        "1st" => 1,
        "2nd" => 2,
        "3rd" => 3,
        "4th" => 4,
        "5th" => return Err(Refusal::FifthWeekday),
        "first" => {
            return match second {
                "weekday" => Ok((MonthDay::FirstWeekday, rest)),
                _ => Err(Refusal::Malformed),
            };
        }
        "last" => match second {
            "day" => return Ok((MonthDay::LastDay, rest)),
            "weekday" => return Ok((MonthDay::LastWeekday, rest)),
            _ => -1,
        },
        _ => return Err(Refusal::Malformed),
    };
    let d = dow(second).ok_or(Refusal::Malformed)?;
    Ok((MonthDay::Nth(n, d), rest))
}

/// `at HH:MM[,HH:MM..]` or `from HH:MM to HH:MM by Nm`, into BYHOUR and BYMINUTE.
fn times(s: &str) -> Result<(u32, u64, &str), Refusal> {
    let (w, rest) = word(s).ok_or(Refusal::Malformed)?;
    match w {
        "at" => {
            let (list, rest) = word(rest).ok_or(Refusal::Malformed)?;
            let mut hours = 0u32;
            let mut minute = None;
            for part in list.split(',') {
                let t = hhmm(part).ok_or(Refusal::Malformed)?;
                let (h, m) = (t / 60, t % 60);
                if minute.is_some_and(|x| x != m) {
                    return Err(Refusal::MixedMinutes);
                }
                minute = Some(m);
                hours |= 1 << h;
            }
            Ok((hours, 1u64 << minute.ok_or(Refusal::Malformed)?, rest))
        }
        "from" => {
            let (a, rest) = word(rest).ok_or(Refusal::Malformed)?;
            let rest = expect(rest, "to")?;
            let (b, rest) = word(rest).ok_or(Refusal::Malformed)?;
            let rest = expect(rest, "by")?;
            let (step, rest) = word(rest).ok_or(Refusal::Malformed)?;
            let start = hhmm(a).ok_or(Refusal::Malformed)?;
            let end = hhmm(b).ok_or(Refusal::Malformed)?;
            let step = step
                .strip_suffix('m')
                .and_then(digits)
                .filter(|&n| n > 0)
                .ok_or(Refusal::Malformed)? as u16;
            let (hours, minutes) = range(start, end, step)?;
            Ok((hours, minutes, rest))
        }
        _ => Err(Refusal::Malformed),
    }
}

/// **A `from .. to .. by` range as BYHOUR and BYMINUTE**, refused unless the product is exactly the
/// range. The start minute must be below the step, so it is the smallest minute in the set, and the
/// end must be the last time the product reaches, so nothing past it fires either.
///
/// `proofs::a_range_is_exactly_its_steps` proves that an accepted range fires on exactly the times
/// `start + k * step` up to `end`.
pub fn range(start: u16, end: u16, step: u16) -> Result<(u32, u64), Refusal> {
    let (sh, sm) = ((start / 60) as u32, (start % 60) as u32);
    let (eh, em) = ((end / 60) as u32, (end % 60) as u32);
    if step < 60 {
        if 60 % step != 0 {
            return Err(Refusal::StepDoesNotDivide);
        }
        let step = step as u32;
        let mut minutes = 0u64;
        let mut m = sm % step;
        let mut last_minute = 0;
        while m < 60 {
            minutes |= 1 << m;
            last_minute = m;
            m += step;
        }
        let hours = if eh >= sh {
            ((1u64 << (eh + 1)) - (1u64 << sh)) as u32
        } else {
            0
        };
        let last = if em >= last_minute {
            Some(eh * 60 + last_minute)
        } else if eh > sh {
            Some((eh - 1) * 60 + last_minute)
        } else {
            None
        };
        if sm >= step || eh < sh || em != last_minute {
            return Err(Refusal::InexactRange(
                last.filter(|&l| l >= start as u32).map(|l| l as u16),
            ));
        }
        Ok((hours, minutes))
    } else {
        if !step.is_multiple_of(60) {
            return Err(Refusal::StepDoesNotDivide);
        }
        let k = (step / 60) as u32;
        let mut hours = 0u32;
        let mut h = sh;
        let mut last = None;
        while h <= eh && h < 24 {
            hours |= 1 << h;
            if h < eh || em >= sm {
                last = Some(h * 60 + sm);
            }
            h += k;
        }
        if eh < sh || (eh - sh) % k != 0 || em != sm {
            return Err(Refusal::InexactRange(last.map(|l| l as u16)));
        }
        Ok((hours, 1u64 << sm))
    }
}

/// **Parse what follows `every`**, as a calendar line if it opens with a cadence word or a bare count.
///
/// `every 15m` (digits glued to a unit) is not a calendar cadence and comes back
/// [`Parsed::NotCalendar`], so the interval grammar keeps every line it has ever accepted.
pub fn parse(after_every: &str) -> Result<Parsed<'_>, Refusal> {
    let Some((first, rest)) = word(after_every) else {
        return Ok(Parsed::NotCalendar);
    };
    let mut rule = Rule {
        freq: Freq::Daily,
        interval: 1,
        weekdays: ALL_DAYS,
        on: MonthDay::Any,
        months: ALL_MONTHS,
        hours: 0,
        minutes: 0,
        starting: None,
        through: None,
    };
    let cadence = if first.bytes().all(|b| b.is_ascii_digit()) {
        counted_cadence(first, rest, &mut rule)?
    } else {
        named_cadence(first, rest, &mut rule)?
    };
    let Some(rest) = cadence else {
        return Ok(Parsed::NotCalendar);
    };

    // The optional clauses, in their one order.
    let rest = optional_clauses(rest, &mut rule)?;
    let (hours, minutes, rest) = times(rest)?;
    rule.hours = hours;
    rule.minutes = minutes;

    check_rule(&rule)?;
    Ok(Parsed::Calendar(rule, rest))
}

/// A cadence that opens with a count (`every 2 weeks on mon`): the rest of the line after it, or
/// `None` when the line is not a calendar line.
fn counted_cadence<'a>(
    first: &str,
    rest: &'a str,
    rule: &mut Rule,
) -> Result<Option<&'a str>, Refusal> {
    let Some((unit, rest)) = word(rest) else {
        return Ok(None);
    };
    if !matches!(
        unit,
        "days" | "weeks" | "months" | "minutes" | "mins" | "hours" | "seconds"
    ) {
        return Ok(None);
    }
    let count = digits(first).ok_or(Refusal::BadCount)?;
    if !(2..=99).contains(&count) && !matches!(unit, "minutes" | "mins" | "hours" | "seconds") {
        return Err(Refusal::BadCount);
    }
    rule.interval = count.clamp(2, 99) as u8;
    let rest = match unit {
        "days" => rest,
        "weeks" => {
            rule.freq = Freq::Weekly;
            let rest = expect(rest, "on")?;
            let (days, rest) = word(rest).ok_or(Refusal::Malformed)?;
            rule.weekdays = day_list(days)?;
            rest
        }
        "months" => {
            rule.freq = Freq::Monthly;
            let rest = expect(rest, "on")?;
            let (on, rest) = month_day(rest)?;
            rule.on = on;
            rest
        }
        "minutes" | "mins" | "hours" | "seconds" => return Err(Refusal::SpelledInterval),
        // `every 30 least_authority_demo 7` is an interval missing its unit far more often
        // than a calendar line, so the interval grammar answers it.
        _ => return Ok(None),
    };
    Ok(Some(rest))
}

/// A cadence that opens with a word (`every weekday`, `every month on 1`): the rest of the line
/// after it, or `None` when the line is not a calendar line.
fn named_cadence<'a>(
    first: &str,
    rest: &'a str,
    rule: &mut Rule,
) -> Result<Option<&'a str>, Refusal> {
    let rest = match first {
        "day" => rest,
        "weekday" => {
            rule.freq = Freq::Weekly;
            rule.weekdays = WEEKDAYS;
            rest
        }
        "weekend" => {
            rule.freq = Freq::Weekly;
            rule.weekdays = WEEKEND;
            rest
        }
        "week" => {
            rule.freq = Freq::Weekly;
            let rest = expect(rest, "on")?;
            let (days, rest) = word(rest).ok_or(Refusal::Malformed)?;
            rule.weekdays = day_list(days)?;
            rest
        }
        "month" => {
            rule.freq = Freq::Monthly;
            let rest = expect(rest, "on")?;
            let (on, rest) = month_day(rest)?;
            rule.on = on;
            rest
        }
        _ => return Ok(None),
    };
    Ok(Some(rest))
}

/// The optional `in`, `starting` and `through` clauses, in that order: the rest of the line after them.
fn optional_clauses<'a>(rest: &'a str, rule: &mut Rule) -> Result<&'a str, Refusal> {
    let mut rest = rest;
    if let Some(("in", after)) = word(rest) {
        if rule.interval > 1 {
            return Err(Refusal::CountWithIn);
        }
        let (list_token, after) = word(after).ok_or(Refusal::Malformed)?;
        rule.months = list(list_token, month).ok_or(Refusal::Malformed)? as u16;
        rest = after;
    }
    if let Some(("starting", after)) = word(rest) {
        let (d, after) = word(after).ok_or(Refusal::Malformed)?;
        rule.starting = Some(date(d).ok_or(Refusal::Malformed)?);
        rest = after;
    }
    if let Some(("through", after)) = word(rest) {
        let (d, after) = word(after).ok_or(Refusal::Malformed)?;
        rule.through = Some(date(d).ok_or(Refusal::Malformed)?);
        rest = after;
    }
    Ok(rest)
}

/// Whether a parsed rule is consistent: a count has a start, `through` is not before `starting`,
/// and `starting` is a day the rule fires on.
fn check_rule(rule: &Rule) -> Result<(), Refusal> {
    if rule.interval > 1 && rule.starting.is_none() {
        return Err(Refusal::CountWithoutStarting);
    }
    if let (Some(s), Some(t)) = (rule.starting, rule.through)
        && t < s
    {
        return Err(Refusal::ThroughBeforeStarting);
    }
    if let Some(s) = rule.starting
        && !day_matches(rule, s)
    {
        return Err(Refusal::StartingOffRule);
    }
    Ok(())
}

/// Write a rule as the RRULE it is, for the printed plan: `FREQ=MONTHLY;BYDAY=-1FR;BYHOUR=17;...`.
pub(crate) fn write_rrule(rule: &Rule, out: &mut dyn FnMut(&[u8])) {
    const DOW: [&[u8]; 7] = [b"MO", b"TU", b"WE", b"TH", b"FR", b"SA", b"SU"];
    out(match rule.freq {
        Freq::Daily => b"FREQ=DAILY",
        Freq::Weekly => b"FREQ=WEEKLY",
        Freq::Monthly => b"FREQ=MONTHLY",
    });
    if rule.interval > 1 {
        out(b";INTERVAL=");
        num(rule.interval as u32, out);
    }
    if rule.months != ALL_MONTHS {
        out(b";BYMONTH=");
        bits(rule.months as u64, 12, 1, out);
    }
    let days = |mask: u8, out: &mut dyn FnMut(&[u8])| {
        let mut first = true;
        for (i, d) in DOW.iter().enumerate() {
            if mask & (1 << i) != 0 {
                if !first {
                    out(b",");
                }
                out(d);
                first = false;
            }
        }
    };
    match (rule.freq, rule.on) {
        (Freq::Weekly, _) => {
            out(b";BYDAY=");
            days(rule.weekdays, out);
        }
        (_, MonthDay::Days(b)) => {
            out(b";BYMONTHDAY=");
            bits(b as u64, 29, 0, out);
        }
        (_, MonthDay::LastDay) => out(b";BYMONTHDAY=-1"),
        (_, MonthDay::FirstWeekday) => out(b";BYDAY=MO,TU,WE,TH,FR;BYSETPOS=1"),
        (_, MonthDay::LastWeekday) => out(b";BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1"),
        (_, MonthDay::Nth(n, d)) => {
            out(b";BYDAY=");
            if n < 0 {
                out(b"-1");
            } else {
                num(n as u32, out);
            }
            out(DOW[d as usize]);
        }
        _ => {}
    }
    out(b";BYHOUR=");
    bits(rule.hours as u64, 24, 0, out);
    out(b";BYMINUTE=");
    bits(rule.minutes, 60, 0, out);
    if let Some(t) = rule.through {
        out(b";UNTIL=");
        write_date(t, out);
        out(b"T235959Z");
    }
    if let Some(s) = rule.starting {
        out(b" from ");
        write_date(s, out);
    }
}

fn bits(mask: u64, n: u32, base: u32, out: &mut dyn FnMut(&[u8])) {
    let mut first = true;
    for i in 0..n {
        if mask & (1 << i) != 0 {
            if !first {
                out(b",");
            }
            num(i + base, out);
            first = false;
        }
    }
}

fn num(v: u32, out: &mut dyn FnMut(&[u8])) {
    let mut buf = [0u8; 10];
    let mut n = 0;
    let mut v = v;
    loop {
        buf[9 - n] = b'0' + (v % 10) as u8;
        n += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    out(&buf[10 - n..]);
}

fn write_date(d: i64, out: &mut dyn FnMut(&[u8])) {
    let Some((y, m, dd)) = civil(d) else {
        out(b"????????");
        return;
    };
    let pad = |v: u32, w: usize, out: &mut dyn FnMut(&[u8])| {
        let mut buf = [b'0'; 4];
        let mut v = v;
        for i in (0..w).rev() {
            buf[i] = b'0' + (v % 10) as u8;
            v /= 10;
        }
        out(&buf[..w]);
    };
    pad(y as u32, 4, out);
    pad(m as u32, 2, out);
    pad(dd as u32, 2, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(line: &str) -> Rule {
        let after_every = line
            .strip_prefix("every ")
            .expect("a calendar line opens with every");
        let with_command = [after_every, " least_authority_demo 7"].concat();
        match parse(&with_command) {
            Ok(Parsed::Calendar(r, command)) => {
                assert_eq!(
                    command.trim(),
                    "least_authority_demo 7",
                    "{line}: the command is the rest"
                );
                r
            }
            Ok(Parsed::NotCalendar) => panic!("{line}: read as an interval"),
            Err(e) => panic!("{line}: refused: {}", e.message()),
        }
    }

    fn refusal(line: &str) -> Refusal {
        let after_every = line.strip_prefix("every ").unwrap();
        let with_command = [after_every, " least_authority_demo 7"].concat();
        match parse(&with_command) {
            Err(e) => e,
            Ok(_) => panic!("{line}: accepted, and G5 refuses it"),
        }
    }

    fn minute(iso: &str) -> i64 {
        let (d, t) = iso.split_once('T').unwrap();
        let day = date(d).unwrap();
        day * MINUTES_PER_DAY + hhmm(t).unwrap() as i64
    }

    /// **Every answer `next` gives agrees with dateutil's own RRULE expansion**, on 780 samples over
    /// twenty rules and three years, including month ends, a leap day and the edges of `starting`
    /// and `through`. The oracle builds each RRULE by hand from the note's table, not from this
    /// parser, so a parser that misread a line fails here as surely as arithmetic that got a month
    /// wrong. See crates/timetable/oracle/rrule.py for how the table was made.
    #[test]
    fn every_next_occurrence_agrees_with_dateutil() {
        let table = include_str!("../oracle/rrule.txt");
        let mut checked = 0;
        for row in table.lines().filter(|l| !l.starts_with('#')) {
            let mut parts = row.split('|');
            let (line, after, want) = (
                parts.next().unwrap(),
                parts.next().unwrap(),
                parts.next().unwrap(),
            );
            let r = rule(line);
            let got = next(&r, minute(after));
            let want = (want != "none").then(|| minute(want));
            assert_eq!(got, want, "{line}, after {after}");
            checked += 1;
        }
        assert_eq!(checked, 780, "the whole oracle table was read");
    }

    /// **The note's RRULE table, row by row**: each G5 line prints as the RRULE the ruling says it is.
    #[test]
    fn each_line_is_the_rrule_the_ruling_names() {
        let rows = [
            ("every day at 02:00", "FREQ=DAILY;BYHOUR=2;BYMINUTE=0"),
            (
                "every day at 09:00,21:00",
                "FREQ=DAILY;BYHOUR=9,21;BYMINUTE=0",
            ),
            (
                "every weekday at 09:00",
                "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR;BYHOUR=9;BYMINUTE=0",
            ),
            (
                "every weekend at 10:00",
                "FREQ=WEEKLY;BYDAY=SA,SU;BYHOUR=10;BYMINUTE=0",
            ),
            (
                "every week on mon,thu at 08:00",
                "FREQ=WEEKLY;BYDAY=MO,TH;BYHOUR=8;BYMINUTE=0",
            ),
            (
                "every month on 1,15 at 00:00",
                "FREQ=MONTHLY;BYMONTHDAY=1,15;BYHOUR=0;BYMINUTE=0",
            ),
            (
                "every month on last day at 23:00",
                "FREQ=MONTHLY;BYMONTHDAY=-1;BYHOUR=23;BYMINUTE=0",
            ),
            (
                "every month on 2nd tue at 09:00",
                "FREQ=MONTHLY;BYDAY=2TU;BYHOUR=9;BYMINUTE=0",
            ),
            (
                "every month on last fri at 17:00",
                "FREQ=MONTHLY;BYDAY=-1FR;BYHOUR=17;BYMINUTE=0",
            ),
            (
                "every month on last weekday at 18:00",
                "FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1;BYHOUR=18;BYMINUTE=0",
            ),
            (
                "every month on 1 in jan,apr,jul,oct at 06:00",
                "FREQ=MONTHLY;BYMONTH=1,4,7,10;BYMONTHDAY=1;BYHOUR=6;BYMINUTE=0",
            ),
            (
                "every 2 weeks on mon starting 2026-10-05 at 08:00",
                "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO;BYHOUR=8;BYMINUTE=0 from 20261005",
            ),
            (
                "every 3 days starting 2026-10-01 at 04:00",
                "FREQ=DAILY;INTERVAL=3;BYHOUR=4;BYMINUTE=0 from 20261001",
            ),
            (
                "every weekday from 09:00 to 16:45 by 15m",
                "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR;BYHOUR=9,10,11,12,13,14,15,16;BYMINUTE=0,15,30,45",
            ),
            (
                "every day through 2027-03-31 at 12:00",
                "FREQ=DAILY;BYHOUR=12;BYMINUTE=0;UNTIL=20270331T235959Z",
            ),
        ];
        for (line, want) in rows {
            let mut out = [0u8; 160];
            let mut n = 0;
            write_rrule(&rule(line), &mut |b: &[u8]| {
                out[n..n + b.len()].copy_from_slice(b);
                n += b.len();
            });
            assert_eq!(core::str::from_utf8(&out[..n]).unwrap(), want, "{line}");
        }
    }

    /// **Every refusal in the ruling's table, each with its own answer.** A grammar that accepted
    /// any of these would answer quietly and wrongly.
    #[test]
    fn each_refusal_in_the_ruling_is_refused_for_its_own_reason() {
        let cases = [
            ("every month on 31 at 00:00", Refusal::DayPast28),
            ("every month on 1,29 at 00:00", Refusal::DayPast28),
            ("every month on 5th fri at 00:00", Refusal::FifthWeekday),
            ("every day at 09:00,17:30", Refusal::MixedMinutes),
            (
                "every day from 09:00 to 17:00 by 15m",
                Refusal::InexactRange(Some(16 * 60 + 45)),
            ),
            (
                "every day from 09:20 to 16:50 by 15m",
                Refusal::InexactRange(Some(16 * 60 + 50)),
            ),
            (
                "every day from 09:00 to 17:00 by 7m",
                Refusal::StepDoesNotDivide,
            ),
            (
                "every day from 09:00 to 17:00 by 90m",
                Refusal::StepDoesNotDivide,
            ),
            (
                "every 2 weeks on mon in jan starting 2026-01-05 at 08:00",
                Refusal::CountWithIn,
            ),
            (
                "every 2 weeks on mon at 08:00",
                Refusal::CountWithoutStarting,
            ),
            (
                "every 2 weeks on mon starting 2026-10-06 at 08:00",
                Refusal::StartingOffRule,
            ),
            (
                "every day starting 2026-10-06 through 2026-10-01 at 08:00",
                Refusal::ThroughBeforeStarting,
            ),
            ("every 15 minutes at 08:00", Refusal::SpelledInterval),
            (
                "every 100 days starting 2026-10-01 at 08:00",
                Refusal::BadCount,
            ),
            (
                "every 1 days starting 2026-10-01 at 08:00",
                Refusal::BadCount,
            ),
            ("every day at 25:00", Refusal::Malformed),
            ("every week on monday at 08:00", Refusal::Malformed),
            ("every month on first fri at 08:00", Refusal::Malformed),
            ("every day", Refusal::Malformed),
        ];
        for (line, want) in cases {
            assert_eq!(refusal(line), want, "{line}");
        }
        // And the interval grammar keeps everything it had.
        assert!(matches!(
            parse("30s least_authority_demo 7"),
            Ok(Parsed::NotCalendar)
        ));
        assert!(matches!(
            parse("30 least_authority_demo 7"),
            Ok(Parsed::NotCalendar)
        ));
    }

    /// **A range accepts exactly its RRULE product**, and the edges of "exact" are where they should
    /// be: the last step of the hour, and a start minute below the step.
    #[test]
    fn a_range_is_accepted_only_when_it_is_one_rrule() {
        let (h, m) = range(9 * 60, 16 * 60 + 45, 15).unwrap();
        assert_eq!(h, 0b1_1111_1111 << 9 & 0x1ffff, "hours 9 to 16");
        assert_eq!(m, 1 | 1 << 15 | 1 << 30 | 1 << 45);
        let (h, m) = range(5, 22 * 60 + 5, 120).unwrap();
        assert_eq!(h, 0b0101_0101_0101_0101_0101_0101 & 0x7f_ffff);
        assert_eq!(m, 1 << 5);
    }

    /// The month-day predicates, on months that start on each day of the week.
    #[test]
    fn first_and_last_weekdays_land_where_a_calendar_says() {
        // September 2026: the 1st is a Tuesday, so it is the first weekday.
        assert!(month_day_matches(MonthDay::FirstWeekday, 1, 1, 30));
        // August 2026: the 1st is a Saturday, so the first weekday is Monday the 3rd.
        let first_aug = |d: u8| month_day_matches(MonthDay::FirstWeekday, d, (d + 4) % 7, 31);
        assert!(!first_aug(1) && !first_aug(2) && first_aug(3));
        // January 2027: the 31st is a Sunday, so the last weekday is Friday the 29th.
        let last_jan = |d: u8| month_day_matches(MonthDay::LastWeekday, d, (d + 3) % 7, 31);
        assert!(last_jan(29) && !last_jan(30) && !last_jan(31));
    }

    // ---- survivor triage of the 2026-10-03 census ---------------------------------------------

    /// Every day and month name is the one it says. Only some had a line in the oracle table, so a
    /// deleted arm for `wed`, `sun`, `feb`, `may`, `jun`, `aug`, `sep`, `nov` or `dec` was a grammar
    /// that refused a word it advertises.
    #[test]
    fn every_day_and_month_name_is_the_one_it_says() {
        let days = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
        for (i, d) in days.iter().enumerate() {
            let line = ["every week on ", d, " at 09:00"].concat();
            assert_eq!(rule(&line).weekdays, 1 << i, "{d}");
        }
        let months = [
            "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
        ];
        for (i, m) in months.iter().enumerate() {
            let line = ["every day in ", m, " at 09:00"].concat();
            assert_eq!(rule(&line).months, 1 << i, "{m}");
        }
        for (word, n) in [("1st", 1), ("2nd", 2), ("3rd", 3), ("4th", 4), ("last", -1)] {
            let line = ["every month on ", word, " wed at 09:00"].concat();
            assert_eq!(rule(&line).on, MonthDay::Nth(n, 2), "{word}");
        }
    }

    /// Each refusal explains itself in words of its own.
    #[test]
    fn no_two_refusals_say_the_same_thing() {
        let all = [
            Refusal::Malformed,
            Refusal::DayPast28,
            Refusal::FifthWeekday,
            Refusal::BadCount,
            Refusal::SpelledInterval,
            Refusal::CountWithIn,
            Refusal::CountWithoutStarting,
            Refusal::StartingOffRule,
            Refusal::ThroughBeforeStarting,
            Refusal::MixedMinutes,
            Refusal::StepDoesNotDivide,
            Refusal::InexactRange(None),
        ];
        for (i, a) in all.iter().enumerate() {
            assert!(!a.message().is_empty(), "{a:?}");
            for b in &all[i + 1..] {
                assert_ne!(a.message(), b.message());
            }
        }
    }

    /// Numbers and times are digits and nothing else. `+9` parses as 9 in Rust, and a width of five
    /// is not a minute count, so both have to be turned away by the grammar and not by luck.
    #[test]
    fn a_time_or_date_is_exactly_its_digits() {
        for line in [
            "every day at +9:00",
            "every day at 9:00",
            "every day at 09:0",
            "every day at 24:00",
            "every day at 12:60",
            "every day from 09:00 to 17:00 by 00060m",
            "every day from 09:00 to 17:00 by 0m",
            "every day starting 2026-10-1 at 08:00",
            "every day starting 26-10-01 at 08:00",
            "every day starting 2026-1-01 at 08:00",
            "every day starting 2026-10-01-05 at 08:00",
            "every day from 09:00 until 17:00 by 15m",
            "every day from 09:00 to 17:00 per 15m",
        ] {
            assert_eq!(refusal(line), Refusal::Malformed, "{line}");
        }
        // And the last edge of each is still a time: 23:59, and a line that starts the day it ends.
        assert!(rule("every day at 23:59").hours == 1 << 23);
        rule("every day starting 2026-10-06 through 2026-10-06 at 08:00");
    }

    #[test]
    fn the_first_weekday_prints_as_its_rrule() {
        let mut out = [0u8; 120];
        let mut n = 0;
        write_rrule(
            &rule("every month on first weekday at 08:00"),
            &mut |b: &[u8]| {
                out[n..n + b.len()].copy_from_slice(b);
                n += b.len();
            },
        );
        assert_eq!(
            core::str::from_utf8(&out[..n]).unwrap(),
            "FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=1;BYHOUR=8;BYMINUTE=0"
        );
    }

    /// `through` names a day that still counts, and the first rule's day-level predicate agrees with
    /// the answer `next` gives at the edge.
    #[test]
    fn the_through_day_itself_still_fires() {
        let r = rule("every day through 2026-10-06 at 08:00");
        assert!(day_matches(&r, date("2026-10-06").unwrap()));
        assert!(!day_matches(&r, date("2026-10-07").unwrap()));
        assert_eq!(
            next(&r, minute("2026-10-05T09:00")),
            Some(minute("2026-10-06T08:00"))
        );
        assert_eq!(next(&r, minute("2026-10-06T08:00")), None);
    }

    /// A Saturday is never the last weekday, even when it is the last day of the month.
    #[test]
    fn the_last_weekday_is_never_a_saturday() {
        assert!(!month_day_matches(MonthDay::LastWeekday, 31, 5, 31));
        assert!(month_day_matches(MonthDay::LastWeekday, 31, 4, 31));
    }

    /// **INTERVAL rules count from `starting`, in whole days, weeks that begin on Monday, or
    /// months.** The oracle table has no interval rule at all, so the phase arithmetic was never
    /// run against an answer.
    #[test]
    fn an_interval_counts_from_its_start_in_the_unit_it_names() {
        let at = |line: &str, after: &str| next(&rule(line), minute(after));
        // A start whose day number is not a multiple of the interval, so a phase counted from the
        // epoch instead of from `starting` would disagree.
        let daily = "every 3 days starting 2026-10-02 at 04:00";
        assert_eq!(
            at(daily, "2026-10-02T04:00"),
            Some(minute("2026-10-05T04:00"))
        );
        assert_eq!(
            at(daily, "2026-10-05T04:00"),
            Some(minute("2026-10-08T04:00"))
        );
        // Fridays too, so a week is named by its Monday and not by the day that asked.
        let weekly = "every 2 weeks on mon,fri starting 2026-10-05 at 08:00";
        assert_eq!(
            at(weekly, "2026-10-05T08:00"),
            Some(minute("2026-10-09T08:00"))
        );
        assert_eq!(
            at(weekly, "2026-10-09T08:00"),
            Some(minute("2026-10-19T08:00"))
        );
        assert_eq!(
            at(weekly, "2026-10-19T08:00"),
            Some(minute("2026-10-23T08:00"))
        );
        // Intervals of 3 and 7 weeks, where a week count scaled instead of divided, or added to the
        // start's instead of taken from it, comes out the same as the right one for 2.
        let three = "every 3 weeks on mon starting 2026-10-12 at 08:00";
        assert_eq!(
            at(three, "2026-10-12T08:00"),
            Some(minute("2026-11-02T08:00"))
        );
        let seven = "every 7 weeks on mon starting 2026-10-05 at 08:00";
        assert_eq!(
            at(seven, "2026-10-05T08:00"),
            Some(minute("2026-11-23T08:00"))
        );
        // A start on a Wednesday: Monday the 12th is in the NEXT week, so it is skipped.
        let mid = "every 2 weeks on wed,mon starting 2026-10-07 at 08:00";
        assert_eq!(
            at(mid, "2026-10-07T08:00"),
            Some(minute("2026-10-19T08:00"))
        );
        // Months across a year end.
        let monthly = "every 3 months on 15 starting 2026-11-15 at 09:00";
        assert_eq!(
            at(monthly, "2026-11-15T09:00"),
            Some(minute("2027-02-15T09:00"))
        );
        assert_eq!(
            at(monthly, "2027-02-15T09:00"),
            Some(minute("2027-05-15T09:00"))
        );
    }

    /// The longest gap an accepted rule can have is 99 months, and it is still found.
    #[test]
    fn a_ninety_nine_month_gap_is_inside_the_horizon() {
        let r = rule("every 99 months on 15 starting 2026-10-15 at 09:00");
        let got = next(&r, minute("2026-10-15T09:00")).expect("inside the horizon");
        assert_eq!(got, minute("2035-01-15T09:00"));
    }

    /// Hours past 23 are not hours, whatever else the mask holds.
    #[test]
    fn a_mask_bit_past_the_day_names_no_time() {
        assert_eq!(next_time(1 << 24 | 1 << 31, 1, -1), None);
        assert_eq!(next_time(1 << 23, 1, -1), Some(23 * 60));
    }

    /// **An hour mask never names an hour that does not exist.** `range` is public over `u16`s and
    /// an end past midnight is not refused by its signature, so whatever it accepts must still fit
    /// the 24 bits `next_time` reads, for the minute-stepped and the hour-stepped forms alike.
    #[test]
    fn a_range_never_sets_an_hour_past_the_day() {
        for step in [15u16, 60, 120] {
            for end in [24 * 60, 24 * 60 + 15, 25 * 60, 30 * 60] {
                if let Ok((hours, _)) = range(23 * 60, end, step) {
                    assert_eq!(hours >> 24, 0, "step {step} end {end}");
                }
            }
        }
    }

    /// The edges of `range`, in both of its branches, each chosen so that the neighbouring
    /// comparison would answer differently.
    #[test]
    fn a_range_is_exact_at_its_edges() {
        // An hour that starts and ends in the same hour is a range.
        assert_eq!(
            range(9 * 60, 9 * 60 + 45, 15),
            Ok((1 << 9, 1 | 1 << 15 | 1 << 30 | 1 << 45))
        );
        assert_eq!(range(9 * 60, 9 * 60, 120), Ok((1 << 9, 1)));
        // Hourly steps stop at the end, and do not run on to the end of the day.
        assert_eq!(
            range(9 * 60, 17 * 60, 120),
            Ok((1 << 9 | 1 << 11 | 1 << 13 | 1 << 15 | 1 << 17, 1))
        );
        // Midnight to ten past, by quarters: no earlier hour exists to name as the last.
        assert_eq!(range(0, 10, 15), Err(Refusal::InexactRange(None)));
        // The last time reached is named, from either side of the start minute.
        assert_eq!(
            range(9 * 60, 17 * 60 + 30, 120),
            Err(Refusal::InexactRange(Some(17 * 60)))
        );
        assert_eq!(
            range(9 * 60 + 30, 17 * 60 + 10, 120),
            Err(Refusal::InexactRange(Some(15 * 60 + 30)))
        );
        // A span that is not a whole number of steps, however it is measured.
        assert_eq!(
            range(9 * 60, 18 * 60, 120),
            Err(Refusal::InexactRange(Some(17 * 60)))
        );
        assert_eq!(range(60, 7 * 60, 180), Ok((1 << 1 | 1 << 4 | 1 << 7, 1)));
    }
}
