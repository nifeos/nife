# The calendar, and where BMC's cost actually is

*Name: ratified 2026-10-18 (calef, convention-rulings Rule 3).*

An appendix to [notes/verification.md](../verification.md), which keeps the lesson. This file holds
the eleven calendar harnesses, the measurements that located the cost, and the two falsifications.

## The harnesses

Eleven in `crates/calendar/src/lib.rs`, the civil-date arithmetic of milestone 51 (wall-clock time,
the `date` command, and an NTP service). See notes/calendar.md for the crate itself. It looks like
the ideal target: closed-form integer arithmetic, no loops, no tables, no allocation, and a
round-trip property that is a single equation. Half of that turned out to be true. The half that did
not is the useful part of this entry.

| Harness | Property |
|---|---|
| `the_calendar_algorithms_are_mutual_inverses` | **the central theorem**: for every one of the 3,652,425 days in the supported range, `civil_from_days` then `days_from_civil` returns the day it started from. The leap-year rules, the month lengths, the century exception and its exception all live inside those two functions, and a bijection cannot have them wrong in a way that cancels |
| `a_day_number_always_decodes_to_a_real_date` | every day number decodes to a date that *exists*: month 1..=12, day within that month's length in that year, and the validating constructor accepts the result. This is what stops the bijection from being vacuous (a decoder that consistently invented February 30 would still be one) |
| `every_real_date_survives_its_own_day_number` | the other direction, over fully symbolic year/month/day/hour/minute/second: every field combination the constructor accepts round-trips through its day number, and its timestamp lands inside the supported range |
| `later_days_are_later_dates` | monotonicity: for any two days, the earlier decodes to a date that sorts earlier under the derived `Ord`. What makes `Format::Date` sortable text, and what rules out a boundary (month end, leap day, century) stepping the calendar backwards for one day |
| `day_of_year_is_bounded_and_366_means_leap` | day of year is 1..=366, and 366 **iff** 31 December of a leap year |
| `weekdays_advance_one_day_at_a_time` | consecutive days differ, the ISO number steps 1..=7 *in order*, and seven days on is the same weekday: the cycle, not merely the change |
| `unix_to_civil_and_back_is_the_identity` | the seconds round trip, over a four-year window straddling the epoch (below) |
| `every_format_is_ascii` | all five formats, for every representable date at every legal offset: within the fixed buffer, never truncated, every byte printable ASCII |
| `the_unix_format_fits_any_i64` | the decimal writer fits any `i64`, `i64::MIN` included, in 20 bytes |
| `parse_is_total_on_hostile_bytes` | the RFC 3339 parser never panics on **arbitrary bytes** at any length up to 26: no UTF-8 assumption, no ASCII assumption. A `date -s` argument and an NTP-adjacent exchange are both text this program did not write |
| `rfc3339_output_parses_back_to_itself` | everything the crate prints, it reads back, for every representable `DateTime`: same instant *and* same offset (the equality is on the whole value, so an offset silently normalized away fails it) |

## The finding: the arithmetic is cheap and the `&str` boundary is not

Two costs surprised us, in opposite directions from the guess.

A 64-bit division by 86,400 is the expensive part of a calendar. The Hinnant algorithms round-trip
over the entire ten-thousand-year range in under a minute. Add one `div_euclid(86_400)` over a
symbolic 64-bit timestamp, and the *same* property does not finish in twenty minutes, or with kissat
instead of CaDiCaL. That is not about calendars. A bit-blasted 64-bit divider with an unconstrained
dividend is close to the worst thing you can hand a CDCL solver. The divisions inside
`civil_from_days` are cheap only because the era arithmetic keeps their operands small. Measured in
the same harness at different widths: one day of timestamps took 3s, four years 64s, and the full
range finished neither in twenty minutes nor with a different solver.

So the harnesses are factored along that seam, rather than bounded uniformly. The calendar half
(days to a date, and everything derived from a day number) is proved over the full range, unbounded
below the type. The seconds half is the only place the expensive division appears, and it is proved
over 1968-01-01 through 1971-12-31.

That window is chosen, not convenient. The risk it carries is "did someone write truncating division
where Euclidean was needed". That is a property of the *code*, and shows up only on a negative
timestamp. In Rust `-1 / 86400` is `0`, so the naive version reports 1970-01-01 for every instant in
the last day of 1969. The window straddles the epoch. It contains negative seconds, zero and positive
seconds, a leap day and three year boundaries. `kani::cover!` checks that the first two are actually
reachable inside it, rather than argued to be.

The composition is stated in the crate. `from_unix` is `civil_from_days(secs.div_euclid(86_400))`
plus a time of day. `to_unix` is `days_from_civil(...) * 86_400` plus the same time of day. So the
two halves join.

Iterating a slice whose length is symbolic costs more than the parser it wraps. Through `&str`,
three harnesses each ran past ten minutes *in symbolic execution*, before the solver saw anything. The cause was not the calendar or the grammar. It was `core::str::from_utf8`
and `for &b in slice` over a variable-length slice, which makes CBMC branch on the length at every
step. Two changes fixed it, and both improved the code rather than weakening the proof:

- `Formatted`'s ASCII check became an index loop over the fixed 32-byte buffer with an `i < len`
  guard. The "therefore `from_utf8` succeeds" step became one line of composition (ASCII is a subset
  of UTF-8) instead of a proof through the standard library's validator. Ten minutes and counting
  became 130 seconds, for a property that now covers every representable date at every offset rather
  than one day.
- The parser grew a byte-level entry point, `parse_rfc3339_bytes`, with the `&str` version as its
  wrapper. RFC 3339 *is* ASCII, so bytes are what the grammar is defined on. A caller holding a
  network buffer no longer has to validate UTF-8 for a function that rejects every non-ASCII byte
  anyway. Totality went from ten minutes-plus to 17 seconds, and got stronger. It now quantifies over
  arbitrary bytes, including sequences that are not UTF-8 at all, which is exactly the input a
  network client will hand it. The print-then-parse round trip went from not finishing to 38 seconds
  over the full range.

The general lesson is the same one the ELF parser taught in a different costume. When BMC stalls,
look at what the harness *touches* rather than at what it is trying to prove. Here the property was
fine and the plumbing was expensive.

A third cut came free from restating a theorem rather than weakening it. Monotonicity was written
over an arbitrary pair of days and cost 228s, because a second symbolic day number buys a second copy
of the whole decode. Written over *adjacent* days it costs 38-50s, and it is the same theorem. The
order is transitive, so `d(n) < d(n+1)` for every `n` gives `d(a) < d(b)` for every `a < b`. It is
the general property, phrased as its induction step.

The crate verifies in about seven minutes on an M-series laptop. That is the largest single entry in
`script/verify`, and it is worth knowing before adding to it. Two costs dominate.
`every_format_is_ascii` takes 150-170s (five formats over every representable date at every offset).
`unix_to_civil_and_back_is_the_identity` takes 81-84s; it is the one harness that pays for the 86,400
division. Timings vary by 10-20% with host load; those are from two full runs.

## Falsified before believed

Per the parent's rule, the two properties that carry the crate were broken on purpose, and the
harnesses caught both:

| Break | Harness | Result |
|---|---|---|
| `is_leap_year` reduced to `year % 4 == 0`, dropping both century clauses | `every_real_date_survives_its_own_day_number` | FAILED in 8s. The constructor accepts 1900-02-29, the day number decodes back to 1900-03-01, and the round trip is not the identity |
| `div_euclid`/`rem_euclid` replaced with `/` and `%` in `from_unix` | `unix_to_civil_and_back_is_the_identity` | FAILED in 32s, on the hour-bound assertion: a negative remainder makes the hour negative before the cast. This is the bug the window straddling the epoch exists to catch, and it is caught |
