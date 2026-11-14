# Dates and times: `jiff`

nife does not write its own calendar. Converting a Unix timestamp to a civil date and back, the
weekday, the day of the year, a month's length, and printing a date are done by the third-party
crate [`jiff`](https://docs.rs/jiff/0.2) 0.2 (Unlicense OR MIT), with its default features off. The
dependency was ruled 2026-11-12 (UTC) under §46 (thin primitives or whole subsystems).

With default features off, jiff needs neither `std` nor an allocator. The lock file also names
`jiff-core`, `jiff-static` and `portable-atomic`; the last two are behind `cfg` gates that this
tree's targets do not meet.

| Caller | What it takes from jiff |
|---|---|
| `date` (`components/src/date.rs`, notes/date.md) | civil time from Unix seconds, and the weekday |
| the clock service (`components/src/clock.rs`) | an RTC's year, month, day and time to Unix seconds |
| `touch -t` (`components/src/swish/navigation.rs`, notes/touch.md) | the date and time of day of an RFC 3339 operand |
| the timetable (`crates/timetable/src/recurrence.rs`) | day numbers to civil dates and back, and month lengths |
| the C library's `gmtime` and `mktime` (`vendor/relibc/header/time/mod.rs`) | the same, for `struct tm` |
| the guest tests (`system_tests/src/user/date_tests.rs`) | `date`'s output parsed back |

## Why `jiff`

The callers need civil arithmetic and an RFC 3339 reader in programs with no `std` and, for
`date`, no allocator. All three Rust date libraries do the arithmetic without `std`. They differ in
formatting and parsing:

- `time` 0.3 formats only with `std`.
- `chrono` 0.4 writes RFC 3339 into an allocated `String`.
- `jiff` 0.2 formats and parses with neither `std` nor an allocator, and without its time-zone
  features its scope is fixed offsets only, which is the scope nife keeps (below).

`date` itself does not use jiff's formatter (see the formats below), so jiff's formatting advantage
applies to the guest tests and to future callers, not to `date`.

Taking a crate is the default §46's amendment of 2026-10-04 sets for userland programs and tools.
The proofs nife had written for its own calendar crate do not change that: they proved code that
could have been taken, and taking it puts nothing of nife's on the verification path.

## The range: years 0000 through 9999

Every caller keeps to years 0000 through 9999, the years RFC 3339's four-digit `date-fullyear` can
write. `jiff::civil` reaches from -9999 to 9999, so each caller states the bound itself: the
timetable and the C library refuse a year outside it (`EOVERFLOW` from `gmtime`), and `date` refuses
an instant whose local year is negative.

`date` goes through `jiff::Timestamp` and a fixed `Offset`, because jiff's span arithmetic links in its
error formatting, and a debug build of `date` with it no longer fits the region a job is built in
(`program_table::JOB_REGION_PAGES`). A `Timestamp` ends at 9999-12-30T22:00:00Z, so `date` refuses the
last day of year 9999, past the clock's 2100 ceiling. The timetable and the C library do civil
arithmetic from the epoch with a `SignedDuration` and keep the whole range.

The calendar is proleptic Gregorian, as in every library that speaks Unix time: the Gregorian rules
run back past the 1582 reform, and year 0 exists and is a leap year. Negative timestamps are
ordinary. Every timestamp is `i64`, so 2038 is an ordinary year.

**Leap seconds have no Unix time.** `touch -t` refuses `:60` rather than folding it onto a
neighboring second, and nothing prints one.

## Time zones: a fixed offset, never a zone

A fixed UTC offset is in scope. The IANA time zone database is not.

An offset is minutes from UTC, within ±23:59 (RFC 3339's grammar, not geography). It is enough to
print a time the way a local user expects and to read one someone else printed, and it is what the
wire format carries.

A zone is a different kind of thing. "America/Los_Angeles" is not an offset; it is a function from
instants to offsets with a century of political history in it. The compiled rules are about 450 KB
and change several times a year, which makes them a **data distribution problem**: who ships them,
who updates them, which capability lets a program read them, and what happens when they are stale.
So there are no zone names, no daylight saving time and no `TZ`; jiff's `tz-system`, `tzdb-*` and
`tz-fat` features stay off. Milestone 484 (IANA zone rules and `strftime`) holds the question.

## The five formats `date` prints

| Format | Example |
|---|---|
| `Human` | `Thu 2026-07-30 12:34:56 UTC`; at an offset, `Thu 2026-07-30 18:04:56 +05:30` |
| `Rfc3339` | `2026-07-30T12:34:56Z`; at an offset, `2026-07-30T18:04:56+05:30` |
| `Date` | `2026-07-30` |
| `Time` | `12:34:56` |
| `Unix` | `1785414896`, the instant whatever the offset |

`date.rs` writes each layout's digits itself from jiff's civil fields, and the `a0` register picks
the layout, so no format string is read from the user. It does not use jiff's `strtime`: that
formatter needs more stack than the one page a spawned program is given. `Human` keeps the ISO date and prefixes
the weekday rather than printing Unix's `Wed Jul 30 12:34:56 UTC 2026`, which does not sort.

## `touch -t` reads RFC 3339 strictly

jiff's Temporal parser accepts more than RFC 3339: a leap second clamped to `:59`, an offset with
no colon, with seconds or past 23:59, the compact `20300101T000000Z` form, and bracketed
annotations. `touch -t` checks the operand's shape first and hands jiff only the date and time of
day. It accepts RFC 3339's `date-time` with the three relaxations RFC 3339 allows: a lowercase `t`
or `z`, a space in place of `T`, and a fraction of a second, which is read and dropped.
