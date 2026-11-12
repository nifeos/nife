# Prior art

An appendix to [the milestone](../1080-calendar-outside-review.md).

How the crate sits against the code a reader already knows. Downloads are crates.io's all-time figures at the time of the review. A cell marked unverified is from memory of the documentation, not from a run.

| implementation | civil range | before the epoch | text it reads | lowercase `t`/`z`, space | `-00:00` | `:60` | fixed offset | `no_std`, no allocation | proofs |
|---|---|---|---|---|---|---|---|---|---|
| this crate | years 0000 to 9999 | yes, `div_euclid` | RFC 3339 only, over bytes | accepts all three | reads as UTC, prints `Z` | `Error::LeapSecond` | `UtcOffset`, minutes, ±23:59 | yes | eleven Kani harnesses, each with a falsification patch |
| Hinnant, `days_from_civil` / `civil_from_days` (the algorithms this crate uses; also C++20 `std::chrono`) | millions of years in 32-bit days (unverified) | yes | none | | | | | header only | the paper's exhaustive checks (unverified) |
| musl `__secs_to_tm` / `__tm_to_secs` | any year that fits `int` | yes, 2000-03-01 epoch | none (`strptime` is separate) | | | | `tm_gmtoff` | C | none |
| OpenBSD libc `gmtime_r` (tzcode) | 64-bit `time_t` | yes | `strptime`, `%z` as `±hhmm` (unverified) | | | | | C | none |
| Python `datetime` | years 1 to 9999 | yes on POSIX | `fromisoformat` | rejects lowercase (unverified) | reads as UTC | rejects | `timezone(timedelta)` | | the test table here was cross-checked against it |
| `chrono` 0.4, 970 million downloads | about ±262,000 years | yes | `parse_from_rfc3339` | accepts (unverified) | reads as UTC (unverified) | represented as a nanosecond overflow | `FixedOffset`, seconds | with default features off | none |
| `time` 0.3, 1.07 billion | ±9999, wider with a feature | yes | `Rfc3339` well-known format | accepts (unverified) | reads as UTC (unverified) | accepts only where a leap second can fall (unverified) | `UtcOffset::from_hms`, ±23:59:59 | yes | none |
| `jiff` 0.2, 275 million | years -9999 to 9999 | yes | RFC 3339 and Temporal | accepts | keeps the distinction (unverified) | clamps (unverified) | `tz::Offset`, seconds | partial (unverified) | none |
| `humantime` 2, 490 million | 1970 onward (unverified) | no | RFC 3339, `Z` only | `_weak` variant accepts a space | not representable | rejects | none | `std` | none |
| `httpdate` 1, 721 million | 1970 to 9999 | no | the three HTTP date forms | | | | none | `std` | none |

Where this crate differs: it is the only one in the table with machine-checked proofs. It uses a fixed output buffer instead of a `String`. Its parser works over bytes, so a network buffer needs no UTF-8 validation first. It also refuses more than any of them: no durations, no
month arithmetic, no zone names, no `strftime`, five formats. What it costs: a stranger reaching for `Display` on `Error`, for a hash of a date, or for the civil date of a day number finds nothing (items 6 and 7). The name a stranger would search crates.io for belongs to another crate (item 1).
