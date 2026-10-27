//! **calendar: Unix seconds to a civil date, and back.** The part of wall-clock time with no
//! authority in it: seconds to a year, month and day and back, and the few textual forms the system
//! needs. No IO, syscalls or allocation, so it is host-tested and Kani-checked.
//!
//! The calendar is proleptic Gregorian (year 0 exists and is a leap year), negative timestamps are
//! supported, and the range is years 0000 through 9999 ([`MIN_TIMESTAMP`] to [`MAX_TIMESTAMP`]),
//! RFC 3339's four-digit year; outside it is [`Error::OutOfRange`]. Leap seconds are
//! [`Error::LeapSecond`]. Zones are a fixed [`UtcOffset`]; the IANA database is out of scope
//! permanently. No durations, month arithmetic or `strftime`. See notes/calendar.md.
//!
//! Name: ratified 2026-08-23 (calef): the plain noun for what the crate does.

#![no_std]

/// The lowest supported year, 0000. See the module docs for why the range is what it is.
pub const MIN_YEAR: i32 = 0;

/// The highest supported year, 9999, which is RFC 3339's four-digit `date-fullyear`.
pub const MAX_YEAR: i32 = 9999;

/// 0000-01-01T00:00:00Z, the earliest instant this crate will name.
pub const MIN_TIMESTAMP: i64 = days_from_civil(MIN_YEAR, 1, 1) * SECS_PER_DAY;

/// 9999-12-31T23:59:59Z, the latest instant this crate will name.
pub const MAX_TIMESTAMP: i64 = days_from_civil(MAX_YEAR, 12, 31) * SECS_PER_DAY + SECS_PER_DAY - 1;

/// Seconds in a day. Exactly 86,400, always, by the definition of Unix time (see the module docs on
/// leap seconds).
pub const SECS_PER_DAY: i64 = 86_400;

/// What can go wrong. One enum for conversion and parsing both, because the API is small enough that
/// two would only make callers convert between them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// A timestamp outside [`MIN_TIMESTAMP`]..=[`MAX_TIMESTAMP`], or a year outside
    /// [`MIN_YEAR`]..=[`MAX_YEAR`].
    OutOfRange,
    /// Month was not 1..=12.
    BadMonth,
    /// Day was not 1..=(the length of that month in that year). This is the one that catches
    /// February 30, and February 29 in a non-leap year.
    BadDay,
    /// Hour was not 0..=23.
    BadHour,
    /// Minute was not 0..=59.
    BadMinute,
    /// Second was not 0..=59.
    BadSecond,
    /// A UTC offset beyond ±23:59, which RFC 3339's grammar cannot write.
    BadOffset,
    /// A seconds field of 60. Well-formed RFC 3339, and unrepresentable as a Unix timestamp; see the
    /// module docs.
    LeapSecond,
    /// The text is not RFC 3339.
    Syntax,
}

impl Error {
    /// A short message, for a shell that has no allocator and wants to print the refusal.
    pub const fn as_str(self) -> &'static str {
        match self {
            Error::OutOfRange => "date outside the supported range (years 0000-9999)",
            Error::BadMonth => "month out of range",
            Error::BadDay => "day out of range for that month",
            Error::BadHour => "hour out of range",
            Error::BadMinute => "minute out of range",
            Error::BadSecond => "second out of range",
            Error::BadOffset => "UTC offset out of range",
            Error::LeapSecond => "leap second (:60) has no Unix timestamp",
            Error::Syntax => "not an RFC 3339 timestamp",
        }
    }
}

/// Is this a leap year in the proleptic Gregorian calendar?
///
/// The rule people half-remember, in full: every fourth year, except every hundredth, except every
/// four-hundredth. 1900 is **not** a leap year and 2000 **is**, which is the pair that breaks date
/// code written from memory, and the pair the tests pin.
pub const fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// How many days that month has in that year, or `None` if `month` is not 1..=12.
pub const fn days_in_month(year: i32, month: u8) -> Option<u8> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 => Some(if is_leap_year(year) { 29 } else { 28 }),
        _ => None,
    }
}

// Howard Hinnant's civil-date algorithms ("chrono-Compatible Low-Level Date Algorithms"): the year
// starts in March so the leap day ends it, `(153*m + 2)/5` walks the month lengths, and a 400-year
// era of 146,097 days makes the century corrections plain division. No loops and no tables. See
// notes/calendar.md ("Why a calendar is worth writing down at all").

/// Days since 1970-01-01 for a civil date. Private: the public entry points validate first, and this
/// has a precondition (`month` in 1..=12, `day` in 1..=31) it does not check.
///
/// `-719468` is the shift from the internal era-relative day count to the Unix epoch: it is how many
/// days lie between 0000-03-01 and 1970-01-01.
const fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let (y, m, d) = (year as i64, month as i64, day as i64);
    // March-based year: January and February belong to the *previous* one.
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // year of era, [0, 399]
    let mp = if m > 2 { m - 3 } else { m + 9 }; // March = 0, February = 11
    let doy = (153 * mp + 2) / 5 + d - 1; // day of the March-based year, [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // day of era, [0, 146096]
    era * 146_097 + doe - 719_468
}

/// The inverse. Private for the same reason: the caller has already bounded `days`.
const fn civil_from_days(days: i64) -> (i32, u8, u8) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    // The three corrections undo the leap-day insertions: /1460 removes the 4-year days, /36524 puts
    // back the century non-leaps, /146096 removes the 400-year exception to that exception.
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // back to January = 1
    let y = if m <= 2 { y + 1 } else { y }; // undo the March-based shift
    (y as i32, m as u8, d as u8)
}

/// A day of the week. `Sunday` is first because that is the order the day-number arithmetic falls
/// out in (`(days + 4) mod 7`, since 1970-01-01 was a Thursday); nothing here depends on the
/// convention, and [`Weekday::iso_number`] gives the ISO 8601 one, where Monday is 1.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Weekday {
    /// ISO 8601 day 7.
    Sunday,
    /// ISO 8601 day 1, the week's first day per [`Weekday::iso_number`].
    Monday,
    /// ISO 8601 day 2.
    Tuesday,
    /// ISO 8601 day 3.
    Wednesday,
    /// ISO 8601 day 4.
    Thursday,
    /// ISO 8601 day 5.
    Friday,
    /// ISO 8601 day 6.
    Saturday,
}

impl Weekday {
    /// The three-letter abbreviation, which is the only natural language in this crate. See
    /// [`Format::Human`] for why there is no month-name table to go with it.
    pub const fn abbrev(self) -> &'static str {
        match self {
            Weekday::Sunday => "Sun",
            Weekday::Monday => "Mon",
            Weekday::Tuesday => "Tue",
            Weekday::Wednesday => "Wed",
            Weekday::Thursday => "Thu",
            Weekday::Friday => "Fri",
            Weekday::Saturday => "Sat",
        }
    }

    /// The full English name.
    pub const fn name(self) -> &'static str {
        match self {
            Weekday::Sunday => "Sunday",
            Weekday::Monday => "Monday",
            Weekday::Tuesday => "Tuesday",
            Weekday::Wednesday => "Wednesday",
            Weekday::Thursday => "Thursday",
            Weekday::Friday => "Friday",
            Weekday::Saturday => "Saturday",
        }
    }

    /// ISO 8601's numbering: Monday is 1, Sunday is 7.
    pub const fn iso_number(self) -> u8 {
        match self {
            Weekday::Monday => 1,
            Weekday::Tuesday => 2,
            Weekday::Wednesday => 3,
            Weekday::Thursday => 4,
            Weekday::Friday => 5,
            Weekday::Saturday => 6,
            Weekday::Sunday => 7,
        }
    }

    const fn from_days(days: i64) -> Weekday {
        // 1970-01-01 was a Thursday, so day 0 is index 4 with Sunday at 0.
        match (days + 4).rem_euclid(7) {
            0 => Weekday::Sunday,
            1 => Weekday::Monday,
            2 => Weekday::Tuesday,
            3 => Weekday::Wednesday,
            4 => Weekday::Thursday,
            5 => Weekday::Friday,
            _ => Weekday::Saturday,
        }
    }
}

/// A calendar date and a wall-clock time, with **no zone attached**. "The 30th of July 2026 at
/// 12:34:56" is a `Civil`; which instant that is depends on an offset, which is [`DateTime`]'s job.
///
/// The fields are private and the constructor validates, so **a `Civil` that exists is a real
/// date**: there is no February 30 in this type, and [`Civil::to_unix`] is therefore infallible.
/// That is the same discipline the capability types use (a `Cap` you hold is one that was minted),
/// and it is what makes the conversion arithmetic total rather than defensive.
///
/// The derived `Ord` is chronological, because the fields are declared most-significant first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Civil {
    year: i32,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
}

impl Civil {
    /// Build a date, checking every field. Returns the *specific* [`Error`] for the field that was
    /// wrong, so a `date -s` verb can say what it did not like.
    ///
    /// # Examples
    ///
    /// ```
    /// use calendar::{Civil, Error};
    ///
    /// assert_eq!(Civil::new(2026, 13, 1, 0, 0, 0), Err(Error::BadMonth));
    /// // 2026 is not a leap year, so there is no 29 February.
    /// assert_eq!(Civil::new(2026, 2, 29, 0, 0, 0), Err(Error::BadDay));
    /// assert!(Civil::new(2024, 2, 29, 0, 0, 0).is_ok());
    /// ```
    pub const fn new(
        year: i32,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
    ) -> Result<Civil, Error> {
        if year < MIN_YEAR || year > MAX_YEAR {
            return Err(Error::OutOfRange);
        }
        let Some(len) = days_in_month(year, month) else {
            return Err(Error::BadMonth);
        };
        if day == 0 || day > len {
            return Err(Error::BadDay);
        }
        if hour > 23 {
            return Err(Error::BadHour);
        }
        if minute > 59 {
            return Err(Error::BadMinute);
        }
        if second > 59 {
            return Err(Error::BadSecond);
        }
        Ok(Civil {
            year,
            month,
            day,
            hour,
            minute,
            second,
        })
    }

    /// The UTC civil date of a Unix timestamp.
    ///
    /// # Examples
    ///
    /// The two conversions are inverses:
    ///
    /// ```
    /// use calendar::{Civil, Weekday};
    ///
    /// // The Unix epoch, as a civil date. 1970-01-01 was a Thursday.
    /// let epoch = Civil::from_unix(0).unwrap();
    /// assert_eq!((epoch.year(), epoch.month(), epoch.day()), (1970, 1, 1));
    /// assert_eq!(epoch.weekday(), Weekday::Thursday);
    /// assert_eq!(epoch.to_unix(), 0);
    ///
    /// // And round-tripping an arbitrary instant.
    /// let d = Civil::new(2026, 8, 2, 12, 30, 15).unwrap();
    /// assert_eq!(Civil::from_unix(d.to_unix()).unwrap(), d);
    /// ```
    ///
    /// One second before the epoch is the last second of 1969, which truncating division gets wrong:
    ///
    /// ```
    /// use calendar::Civil;
    ///
    /// // One second before the epoch is the last second of 1969, not the first of 1970.
    /// let d = Civil::from_unix(-1).unwrap();
    /// assert_eq!((d.year(), d.month(), d.day()), (1969, 12, 31));
    /// assert_eq!((d.hour(), d.minute(), d.second()), (23, 59, 59));
    /// ```
    pub const fn from_unix(secs: i64) -> Result<Civil, Error> {
        if secs < MIN_TIMESTAMP || secs > MAX_TIMESTAMP {
            return Err(Error::OutOfRange);
        }
        // Euclidean division, not truncating: for a negative timestamp, `-1 / 86400` is 0 in Rust and
        // the answer we need is -1 (the day before the epoch). Truncating division here is the classic
        // way a date library gets 1969-12-31 wrong.
        let days = secs.div_euclid(SECS_PER_DAY);
        let sod = secs.rem_euclid(SECS_PER_DAY);
        let (year, month, day) = civil_from_days(days);
        Ok(Civil {
            year,
            month,
            day,
            hour: (sod / 3600) as u8,
            minute: ((sod / 60) % 60) as u8,
            second: (sod % 60) as u8,
        })
    }

    /// The Unix timestamp of this date, read as UTC. Infallible: [`Civil::new`] already established
    /// that the date is real and in range.
    pub const fn to_unix(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day) * SECS_PER_DAY
            + self.hour as i64 * 3600
            + self.minute as i64 * 60
            + self.second as i64
    }

    /// Days since 1970-01-01, discarding the time of day.
    pub const fn days_since_epoch(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }

    /// Which day of the week this date falls on.
    pub const fn weekday(&self) -> Weekday {
        Weekday::from_days(self.days_since_epoch())
    }

    /// Day of the year, 1..=366. January 1 is 1; 366 happens only in a leap year, which is one of
    /// the Kani harnesses rather than a comment.
    pub const fn day_of_year(&self) -> u16 {
        let jan1 = days_from_civil(self.year, 1, 1);
        (self.days_since_epoch() - jan1 + 1) as u16
    }

    /// The year, 0..=9999.
    pub const fn year(&self) -> i32 {
        self.year
    }
    /// The month, 1..=12.
    pub const fn month(&self) -> u8 {
        self.month
    }
    /// The day of the month, 1..=31.
    pub const fn day(&self) -> u8 {
        self.day
    }
    /// The hour, 0..=23.
    pub const fn hour(&self) -> u8 {
        self.hour
    }
    /// The minute, 0..=59.
    pub const fn minute(&self) -> u8 {
        self.minute
    }
    /// The second, 0..=59.
    pub const fn second(&self) -> u8 {
        self.second
    }
}

/// A fixed offset from UTC, in minutes. **Not a time zone**: notes/calendar.md ("Time zones")
/// says why the IANA database is out of scope rather than pending.
///
/// The range is ±23:59, which is RFC 3339's grammar rather than geography (real offsets run
/// -12:00..+14:00). Parsing accepts what the grammar allows; nothing here needs to be stricter than
/// the format it reads.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct UtcOffset {
    minutes: i16,
}

impl UtcOffset {
    /// UTC itself, the offset almost everything in this system should be using.
    pub const UTC: UtcOffset = UtcOffset { minutes: 0 };

    /// An offset of `minutes` from UTC, positive east. Refuses anything beyond ±1439.
    pub const fn from_minutes(minutes: i32) -> Result<UtcOffset, Error> {
        if minutes <= -1440 || minutes >= 1440 {
            return Err(Error::BadOffset);
        }
        Ok(UtcOffset {
            minutes: minutes as i16,
        })
    }

    /// An offset written the way people write it: `from_hm(-8, 0)` is `-08:00`, `from_hm(5, 30)` is
    /// `+05:30`. Both arguments carry the sign, and mixing them (`from_hm(-8, 30)` meaning
    /// -08:30) is why `minutes` is signed here rather than a magnitude plus a flag.
    pub const fn from_hm(hours: i32, minutes: i32) -> Result<UtcOffset, Error> {
        if (hours < 0) != (minutes < 0) && hours != 0 && minutes != 0 {
            return Err(Error::BadOffset);
        }
        if minutes <= -60 || minutes >= 60 {
            return Err(Error::BadOffset);
        }
        UtcOffset::from_minutes(hours * 60 + minutes)
    }

    /// The offset in minutes, positive east of Greenwich.
    pub const fn minutes(self) -> i32 {
        self.minutes as i32
    }

    /// The offset in seconds, which is the form the conversions want.
    pub const fn seconds(self) -> i64 {
        self.minutes as i64 * 60
    }
}

/// An instant, seen from a particular offset. This is what a timestamp on the wire is: a civil
/// reading plus the offset it was read at, which together name one instant.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DateTime {
    civil: Civil,
    offset: UtcOffset,
}

impl DateTime {
    /// Pair a civil reading with the offset it was read at. Infallible: both halves are already
    /// valid, and no arithmetic happens until [`DateTime::to_unix`].
    pub const fn new(civil: Civil, offset: UtcOffset) -> DateTime {
        DateTime { civil, offset }
    }

    /// What the clock reads at `offset` when the Unix time is `secs`.
    ///
    /// `OutOfRange` here means the *local* reading falls outside years 0000-9999, which for any
    /// offset differs from the UTC bound by less than a day. That edge is the only place a caller
    /// can be surprised, and it is why this returns a `Result` where [`DateTime::new`] does not.
    pub const fn from_unix(secs: i64, offset: UtcOffset) -> Result<DateTime, Error> {
        let Some(local) = secs.checked_add(offset.seconds()) else {
            return Err(Error::OutOfRange);
        };
        match Civil::from_unix(local) {
            Ok(civil) => Ok(DateTime { civil, offset }),
            Err(e) => Err(e),
        }
    }

    /// The instant this names, as a Unix timestamp.
    ///
    /// Note this is *not* bounded by [`MIN_TIMESTAMP`]/[`MAX_TIMESTAMP`]: a valid local reading at
    /// the extreme end of the range with a large offset lands up to a day outside it. The value is
    /// still exact; it just may not survive a trip back through [`Civil::from_unix`].
    pub const fn to_unix(&self) -> i64 {
        self.civil.to_unix() - self.offset.seconds()
    }

    /// The local civil reading.
    pub const fn civil(&self) -> Civil {
        self.civil
    }

    /// The offset it was read at.
    pub const fn offset(&self) -> UtcOffset {
        self.offset
    }
}

/// The formats this crate can print: five, each one something in this system needs.
///
/// No `strftime`, which would be a second parser whose errors appear at runtime, and no month
/// names, since `date`'s traditional layout does not sort and a month-name table opens the locale
/// question. See notes/calendar.md ("The five formats, and why not `strftime`").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    /// `2026-07-30T12:34:56Z`, or `2026-07-30T12:34:56+05:30` away from UTC. RFC 3339, the
    /// interchange form: what [`DateTime::parse_rfc3339`] reads back, what a log line or an NTP
    /// exchange should carry. Zero offset prints as `Z`.
    Rfc3339,
    /// `2026-07-30`. The date alone, for a directory listing or a filename.
    Date,
    /// `12:34:56`. The time alone.
    Time,
    /// `Thu 2026-07-30 12:34:56 UTC`, with the offset in place of `UTC` when there is one. The
    /// default a `date` command with no arguments should print: sortable, unambiguous, and it tells
    /// you the day of the week, which is the one thing a human wants that ISO does not give.
    Human,
    /// `1785414896`. The raw seconds, for arithmetic and for a caller that wants to hand the number
    /// straight back to the clock service.
    Unix,
}

/// The longest output any [`Format`] produces is `Human` at an extreme offset,
/// `Fri 9999-12-31 23:59:59 +23:59`, which is 30 bytes.
const FMT_CAP: usize = 32;

/// A formatted timestamp, in a fixed buffer. No allocator, and no lifetime tied to the value it came
/// from, so a caller can format and then print at leisure.
#[derive(Clone, Copy)]
pub struct Formatted {
    buf: [u8; FMT_CAP],
    len: u8,
}

impl Formatted {
    /// The bytes. Always ASCII (proved: `every_format_is_ascii`).
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len as usize]
    }

    /// The text.
    ///
    /// Total, with no panic path: the bytes are ASCII by construction and the Kani harness
    /// `every_format_is_ascii` proves the `unwrap_or` branch is unreachable for every timestamp and
    /// every format. A `.expect()` here would be a panic in a userspace program to defend against a
    /// case that cannot arise.
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(self.as_bytes()).unwrap_or("")
    }
}

impl core::fmt::Display for Formatted {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl core::fmt::Debug for Formatted {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

impl AsRef<str> for Formatted {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// A cursor that writes into the fixed buffer. Every `push` is bounds-checked and silently drops on
/// overflow, which cannot happen ([`FMT_CAP`] is two bytes larger than the longest output) and is
/// still better than a panic in a program that only wanted to print the time.
struct Writer {
    buf: [u8; FMT_CAP],
    len: usize,
}

impl Writer {
    const fn new() -> Writer {
        Writer {
            buf: [0; FMT_CAP],
            len: 0,
        }
    }

    fn byte(&mut self, b: u8) {
        if self.len < FMT_CAP {
            self.buf[self.len] = b;
            self.len += 1;
        }
    }

    fn str(&mut self, s: &str) {
        for &b in s.as_bytes() {
            self.byte(b);
        }
    }

    /// A fixed-width zero-padded number, which is every numeric field in every format here except
    /// `Unix`.
    fn pad(&mut self, value: u32, width: usize) {
        let mut digits = [0u8; 10];
        let mut n = 0;
        let mut v = value;
        loop {
            digits[n] = b'0' + (v % 10) as u8;
            v /= 10;
            n += 1;
            if v == 0 {
                break;
            }
        }
        for _ in n..width {
            self.byte(b'0');
        }
        while n > 0 {
            n -= 1;
            self.byte(digits[n]);
        }
    }

    fn signed(&mut self, value: i64) {
        if value < 0 {
            self.byte(b'-');
        }
        // Via u64 so i64::MIN has no special case.
        let mut v = value.unsigned_abs();
        let mut digits = [0u8; 20];
        let mut n = 0;
        loop {
            digits[n] = b'0' + (v % 10) as u8;
            v /= 10;
            n += 1;
            if v == 0 {
                break;
            }
        }
        while n > 0 {
            n -= 1;
            self.byte(digits[n]);
        }
    }

    /// `+HH:MM` / `-HH:MM`.
    fn offset(&mut self, offset: UtcOffset) {
        let m = offset.minutes();
        self.byte(if m < 0 { b'-' } else { b'+' });
        let m = m.unsigned_abs();
        self.pad(m / 60, 2);
        self.byte(b':');
        self.pad(m % 60, 2);
    }

    fn finish(self) -> Formatted {
        Formatted {
            buf: self.buf,
            len: self.len as u8,
        }
    }
}

impl DateTime {
    /// Render in one of the five [`Format`]s. No allocation; the result owns its bytes.
    pub fn format(&self, format: Format) -> Formatted {
        let mut w = Writer::new();
        let c = self.civil;
        match format {
            Format::Rfc3339 => {
                write_date(&mut w, &c);
                w.byte(b'T');
                write_time(&mut w, &c);
                if self.offset.minutes == 0 {
                    w.byte(b'Z');
                } else {
                    w.offset(self.offset);
                }
            }
            Format::Date => write_date(&mut w, &c),
            Format::Time => write_time(&mut w, &c),
            Format::Human => {
                w.str(c.weekday().abbrev());
                w.byte(b' ');
                write_date(&mut w, &c);
                w.byte(b' ');
                write_time(&mut w, &c);
                w.byte(b' ');
                if self.offset.minutes == 0 {
                    w.str("UTC");
                } else {
                    w.offset(self.offset);
                }
            }
            Format::Unix => w.signed(self.to_unix()),
        }
        w.finish()
    }
}

impl Civil {
    /// Render this date as UTC. Shorthand for `DateTime::new(c, UtcOffset::UTC).format(f)`, which is
    /// what most callers want.
    pub fn format(&self, format: Format) -> Formatted {
        DateTime::new(*self, UtcOffset::UTC).format(format)
    }
}

fn write_date(w: &mut Writer, c: &Civil) {
    w.pad(c.year as u32, 4);
    w.byte(b'-');
    w.pad(c.month as u32, 2);
    w.byte(b'-');
    w.pad(c.day as u32, 2);
}

fn write_time(w: &mut Writer, c: &Civil) {
    w.pad(c.hour as u32, 2);
    w.byte(b':');
    w.pad(c.minute as u32, 2);
    w.byte(b':');
    w.pad(c.second as u32, 2);
}

impl DateTime {
    /// Parse RFC 3339, which is the interoperable subset of ISO 8601 and the only textual timestamp
    /// this crate reads.
    ///
    /// ```text
    /// YYYY-MM-DDTHH:MM:SS[.fraction](Z | ±HH:MM)
    /// ```
    ///
    /// Accepts lowercase `t` and `z` and a space for `T` (RFC 3339 section 5.6 and its NOTE), and
    /// discards fractional seconds; otherwise strict, and `:60` is [`Error::LeapSecond`]. See
    /// notes/calendar.md ("Parsing: RFC 3339, strictly").
    pub fn parse_rfc3339(s: &str) -> Result<DateTime, Error> {
        DateTime::parse_rfc3339_bytes(s.as_bytes())
    }

    /// [`DateTime::parse_rfc3339`] over bytes, which is what the grammar is defined on: RFC 3339 is
    /// ASCII, and every byte outside it is a syntax error whether or not it forms valid UTF-8.
    ///
    /// This is the real implementation and the `&str` version is its wrapper, so a caller holding a
    /// buffer need not validate UTF-8 first and the totality proof quantifies over arbitrary bytes.
    pub fn parse_rfc3339_bytes(b: &[u8]) -> Result<DateTime, Error> {
        // "0000-00-00T00:00:00Z" is 20 bytes, the shortest legal input.
        if b.len() < 20 {
            return Err(Error::Syntax);
        }
        let year = number(b, 0, 4)? as i32;
        expect(b, 4, b'-')?;
        let month = number(b, 5, 2)? as u8;
        expect(b, 7, b'-')?;
        let day = number(b, 8, 2)? as u8;
        match b[10] {
            b'T' | b't' | b' ' => {}
            _ => return Err(Error::Syntax),
        }
        let hour = number(b, 11, 2)? as u8;
        expect(b, 13, b':')?;
        let minute = number(b, 14, 2)? as u8;
        expect(b, 16, b':')?;
        let second = number(b, 17, 2)? as u8;
        if second == 60 {
            return Err(Error::LeapSecond);
        }

        let mut i = 19;
        if b[i] == b'.' {
            i += 1;
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i == start {
                return Err(Error::Syntax); // a '.' with no digits after it
            }
        }

        if i >= b.len() {
            return Err(Error::Syntax);
        }
        let offset = match b[i] {
            b'Z' | b'z' => {
                i += 1;
                UtcOffset::UTC
            }
            sign @ (b'+' | b'-') => {
                // ±HH:MM, six bytes.
                if i + 6 > b.len() {
                    return Err(Error::Syntax);
                }
                let oh = number(b, i + 1, 2)? as i32;
                expect(b, i + 3, b':')?;
                let om = number(b, i + 4, 2)? as i32;
                if om > 59 {
                    return Err(Error::BadOffset);
                }
                i += 6;
                let total = oh * 60 + om;
                UtcOffset::from_minutes(if sign == b'-' { -total } else { total })?
            }
            _ => return Err(Error::Syntax),
        };
        if i != b.len() {
            return Err(Error::Syntax);
        }

        let civil = Civil::new(year, month, day, hour, minute, second)?;
        Ok(DateTime::new(civil, offset))
    }
}

/// `width` ASCII digits at `at`, as a number. Refuses anything else, including a leading `+` or a
/// space, which is what keeps the grammar fixed-width.
fn number(b: &[u8], at: usize, width: usize) -> Result<u32, Error> {
    if at + width > b.len() {
        return Err(Error::Syntax);
    }
    let mut v = 0u32;
    for &c in &b[at..at + width] {
        if !c.is_ascii_digit() {
            return Err(Error::Syntax);
        }
        v = v * 10 + (c - b'0') as u32;
    }
    Ok(v)
}

fn expect(b: &[u8], at: usize, c: u8) -> Result<(), Error> {
    if b.get(at) == Some(&c) {
        Ok(())
    } else {
        Err(Error::Syntax)
    }
}

#[cfg(test)]
mod tests;

/// The machine-checked half (`script/verify`).
///
/// The calendar half (day numbers to dates and back) is proved over the full range. The seconds
/// half, the only place the expensive `div_euclid(86_400)` appears, is proved over a four-year
/// window straddling the epoch. The two compose: `from_unix` is
/// `civil_from_days(secs.div_euclid(86_400))` plus a time of day, `to_unix` its inverse, and the
/// unit test `format_and_parse_round_trip_across_the_range` walks the ten thousand years at a
/// stride. See notes/verification/calendar-proofs.md.
#[cfg(kani)]
mod verification {
    use super::*;

    /// The first and last day numbers in the supported range: 0000-01-01 and 9999-12-31.
    const FIRST_DAY: i64 = days_from_civil(MIN_YEAR, 1, 1);
    const LAST_DAY: i64 = days_from_civil(MAX_YEAR, 12, 31);

    /// Any representable `DateTime`, built from six symbolic fields and a symbolic offset, or
    /// `None` for the field combinations that are not a real date.
    ///
    /// Building from fields skips `from_unix` and its expensive division, which is what lets the
    /// text harnesses cover the whole range; every `DateTime` is a valid `Civil` and offset.
    fn any_datetime() -> Option<DateTime> {
        let year: i32 = kani::any();
        let month: u8 = kani::any();
        let day: u8 = kani::any();
        let hour: u8 = kani::any();
        let minute: u8 = kani::any();
        let second: u8 = kani::any();
        let om: i32 = kani::any();
        kani::assume((-1439..=1439).contains(&om));
        let civil = Civil::new(year, month, day, hour, minute, second).ok()?;
        Some(DateTime::new(civil, UtcOffset::from_minutes(om).unwrap()))
    }

    /// The central theorem: the two calendar algorithms are mutual inverses. For every one of the
    /// 3,652,425 days in the supported range, `civil_from_days` then `days_from_civil` returns the
    /// day it started with. No bound below the type's own.
    ///
    /// A bijection cannot have the leap-year rules or month lengths wrong in a way that cancels.
    /// Falsification: replayable `crates/calendar/falsifications/verification.the_calendar_algorithms_are_mutual_inverses.patch`
    #[kani::proof]
    fn the_calendar_algorithms_are_mutual_inverses() {
        let days: i64 = kani::any();
        kani::assume((FIRST_DAY..=LAST_DAY).contains(&days));
        let (y, m, d) = civil_from_days(days);
        assert_eq!(days_from_civil(y, m, d), days);
    }

    /// A day number always decodes to a date that exists: year in range, month 1..=12, day within
    /// that month's length in that year, and `Civil::new` accepts it. This stops the bijection
    /// above from being vacuous; a decoder that consistently invented February 30 would still be
    /// one.
    /// Falsification: replayable `crates/calendar/falsifications/verification.a_day_number_always_decodes_to_a_real_date.patch`
    #[kani::proof]
    fn a_day_number_always_decodes_to_a_real_date() {
        let days: i64 = kani::any();
        kani::assume((FIRST_DAY..=LAST_DAY).contains(&days));
        let (y, m, d) = civil_from_days(days);
        assert!((MIN_YEAR..=MAX_YEAR).contains(&y));
        assert!((1..=12).contains(&m));
        assert!(1 <= d && d <= days_in_month(y, m).unwrap());
        assert!(Civil::new(y, m, d, 0, 0, 0).is_ok());
    }

    /// The other direction, over every field value there is (February 30, hour 99 and year 10,000
    /// among them). Every combination `Civil::new` accepts survives a trip through its day number,
    /// and its timestamp lies inside the supported range; `to_unix` multiplies, the cheap
    /// direction, so the range assertion holds for every representable date.
    /// Falsification: replayable `crates/calendar/falsifications/verification.every_real_date_survives_its_own_day_number.patch`
    #[kani::proof]
    fn every_real_date_survives_its_own_day_number() {
        let year: i32 = kani::any();
        let month: u8 = kani::any();
        let day: u8 = kani::any();
        let hour: u8 = kani::any();
        let minute: u8 = kani::any();
        let second: u8 = kani::any();

        let Ok(civil) = Civil::new(year, month, day, hour, minute, second) else {
            return; // refused, and `a_day_number_always_decodes_to_a_real_date` covers the accepted set
        };
        assert_eq!(
            civil_from_days(civil.days_since_epoch()),
            (year, month, day)
        );
        let secs = civil.to_unix();
        assert!((MIN_TIMESTAMP..=MAX_TIMESTAMP).contains(&secs));
        // Not vacuous, and specifically not on the case that matters: the constructor does accept a
        // 29th of February, so this is quantifying over leap days rather than over an input set the
        // validation quietly emptied.
        kani::cover!(month == 2 && day == 29);
    }

    /// Time only moves forward: every day in the range decodes to a date that sorts strictly after
    /// the day before it under `Civil`'s derived `Ord`, which makes `Format::Date` sortable.
    ///
    /// Stated on adjacent days, the same theorem by transitivity and several times cheaper than an
    /// arbitrary pair (see notes/verification/calendar-proofs.md).
    /// Falsification: replayable `crates/calendar/falsifications/verification.each_day_sorts_after_the_one_before_it.patch`
    #[kani::proof]
    fn each_day_sorts_after_the_one_before_it() {
        let days: i64 = kani::any();
        kani::assume((FIRST_DAY..LAST_DAY).contains(&days));
        let (ay, am, ad) = civil_from_days(days);
        let (by, bm, bd) = civil_from_days(days + 1);
        assert!(
            Civil::new(ay, am, ad, 0, 0, 0).unwrap() < Civil::new(by, bm, bd, 0, 0, 0).unwrap()
        );
    }

    /// Day of year is 1..=366, and 366 happens only on 31 December of a leap year, for every day in
    /// the range. A day-of-year that saturates at 365, or reaches 366 in a common year, fails this.
    /// Falsification: replayable `crates/calendar/falsifications/verification.day_of_year_is_bounded_and_366_means_leap.patch`
    #[kani::proof]
    fn day_of_year_is_bounded_and_366_means_leap() {
        let days: i64 = kani::any();
        kani::assume((FIRST_DAY..=LAST_DAY).contains(&days));
        let (y, m, d) = civil_from_days(days);
        let c = Civil::new(y, m, d, 0, 0, 0).unwrap();
        let doy = c.day_of_year();
        assert!((1..=366).contains(&doy));
        assert_eq!(doy == 366, is_leap_year(y) && m == 12 && d == 31);
        assert!(doy <= 365 || is_leap_year(y));
        // The interesting value is reachable: without this, "366 implies leap year" would hold
        // vacuously on an implementation that never returns 366.
        kani::cover!(doy == 366);
    }

    /// The week advances one day at a time and closes after seven, over every day in the range:
    /// consecutive days differ, the ISO number steps 1..=7 in order, and seven days on is the same
    /// weekday. Together they pin the `rem_euclid` a truncating `%` would break before the epoch.
    /// Falsification: replayable `crates/calendar/falsifications/verification.weekdays_advance_one_day_at_a_time.patch`
    #[kani::proof]
    fn weekdays_advance_one_day_at_a_time() {
        let days: i64 = kani::any();
        kani::assume((FIRST_DAY..LAST_DAY).contains(&days));
        let today = Weekday::from_days(days);
        let tomorrow = Weekday::from_days(days + 1);
        assert!(today != tomorrow);
        assert_eq!(tomorrow.iso_number(), today.iso_number() % 7 + 1);
        assert_eq!(Weekday::from_days(days + 7), today);
    }

    /// The seconds round trip: the `86_400` split, and whether the code divides the way a calendar
    /// needs.
    ///
    /// Bounded to 1968-01-01T00:00:00Z through 1971-12-31T23:59:59Z. The window straddles the
    /// epoch, so it holds the negative timestamps truncating division gets wrong, a leap day
    /// (1968-02-29), all twelve month lengths and three year boundaries. The calendar at other
    /// years is `the_calendar_algorithms_are_mutual_inverses`'s.
    /// Falsification: replayable `crates/calendar/falsifications/verification.unix_to_civil_and_back_is_the_identity.patch`
    #[kani::proof]
    fn unix_to_civil_and_back_is_the_identity() {
        const START: i64 = days_from_civil(1968, 1, 1) * SECS_PER_DAY;
        const END: i64 = days_from_civil(1971, 12, 31) * SECS_PER_DAY + SECS_PER_DAY - 1;
        let secs: i64 = kani::any();
        kani::assume((START..=END).contains(&secs));

        let c = Civil::from_unix(secs).expect("in range");
        assert!(c.hour() <= 23 && c.minute() <= 59 && c.second() <= 59);
        assert_eq!(c.to_unix(), secs);
        // The two things the window was chosen for are reachable inside it, checked rather than
        // asserted in prose: a pre-epoch timestamp (where truncating division breaks) and a leap day.
        kani::cover!(secs < 0);
        kani::cover!(c.month() == 2 && c.day() == 29);
    }

    /// Formatting is total, never truncates, and is always printable ASCII, for every format and
    /// every representable `DateTime` at every legal UTC offset.
    ///
    /// So the writer's silent-drop branch is never taken, and since ASCII is a subset of UTF-8,
    /// `Formatted::as_str`'s `unwrap_or("")` is unreachable.
    /// Falsification: replayable `crates/calendar/falsifications/verification.every_format_is_ascii.patch`
    #[kani::proof]
    #[kani::unwind(34)]
    fn every_format_is_ascii() {
        let Some(dt) = any_datetime() else { return };
        printable(&dt.format(Format::Rfc3339));
        printable(&dt.format(Format::Date));
        printable(&dt.format(Format::Time));
        printable(&dt.format(Format::Human));
        printable(&dt.format(Format::Unix));
    }

    /// The check `every_format_is_ascii` applies, as an index loop over the fixed buffer: a `for`
    /// over a slice of symbolic length makes CBMC branch on the length at every step, and did not
    /// finish in ten minutes.
    fn printable(f: &Formatted) {
        assert!(f.len as usize <= FMT_CAP);
        assert!(f.len > 0, "no format is empty");
        let mut i = 0;
        while i < FMT_CAP {
            if i < f.len as usize {
                let b = f.buf[i];
                assert!(b.is_ascii() && b >= 0x20, "output must be printable ASCII");
            }
            i += 1;
        }
    }

    /// The `Unix` format fits any `i64`, `i64::MIN` included (a `-` and nineteen digits). Separate
    /// and unbounded because this arm's length depends on the number's magnitude, which is what a
    /// fixed buffer is at risk from.
    /// Falsification: replayable `crates/calendar/falsifications/verification.the_unix_format_fits_any_i64.patch`
    #[kani::proof]
    #[kani::unwind(21)]
    fn the_unix_format_fits_any_i64() {
        let secs: i64 = kani::any();
        let mut w = Writer::new();
        w.signed(secs);
        let f = w.finish();
        assert!(f.len as usize <= 20); // "-9223372036854775808"
        assert!(f.len as usize <= FMT_CAP);
    }

    /// The parser never panics on arbitrary bytes, every byte symbolic, at every length up to 26.
    /// It may return any `Error`, but never index out of bounds, overflow or divide by zero. The
    /// input is text this program did not write (a `date -s` argument), so this is a security
    /// property.
    /// Falsification: replayable `crates/calendar/falsifications/verification.parse_is_total_on_hostile_bytes.patch`
    #[kani::proof]
    #[kani::unwind(28)]
    fn parse_is_total_on_hostile_bytes() {
        const N: usize = 26;
        let bytes: [u8; N] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= N);
        // Totality is the claim, and a panic is what turns it red. The assertion states the one thing
        // a success implies about the input: the shortest legal text is 20 bytes. A `cover!` alone
        // can never fail a Kani harness (Kani 0.67.0), so the success path is also asserted on.
        if DateTime::parse_rfc3339_bytes(&bytes[..len]).is_ok() {
            assert!(len >= 20);
        }
        // Not vacuous: some input in this set actually parses, so the harness is proving the
        // success path total and not merely that every early return returns.
        kani::cover!(DateTime::parse_rfc3339_bytes(&bytes[..len]).is_ok());
    }

    /// Everything this crate prints, it can read back, for every representable `DateTime` at every
    /// minute of offset from -23:59 to +23:59: same instant and same offset, since the equality is
    /// on the whole value.
    /// Falsification: replayable `crates/calendar/falsifications/verification.rfc3339_output_parses_back_to_itself.patch`
    #[kani::proof]
    #[kani::unwind(28)]
    fn rfc3339_output_parses_back_to_itself() {
        let Some(dt) = any_datetime() else { return };
        let text = dt.format(Format::Rfc3339);
        assert_eq!(DateTime::parse_rfc3339_bytes(text.as_bytes()), Ok(dt));
        // Both output shapes are reachable: the 20-byte `Z` form and the 25-byte `±HH:MM` form. They
        // are different code on both sides, so a harness that only ever saw one would prove half.
        kani::cover!(dt.offset().minutes() == 0);
        kani::cover!(dt.offset().minutes() != 0);
    }
}
