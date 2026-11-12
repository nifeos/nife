---
status: BUILT
built: 2026-11-12
raised: 2026-11-12
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1114. The calendar becomes `jiff`

Ruled 2026-11-12 (UTC) under §46 (thin primitives or whole subsystems): nife does not write its own
calendar. `crates/calendar` is gone, and every caller uses the third-party crate `jiff` 0.2
(Unlicense OR MIT) with its default features off. The ruling is the approval of that dependency for
these callers. The branch is `milestone/1105-calendar-becomes-jiff`.

Reuse: `jiff` 0.2 is taken. `time` 0.3 was considered and formats only with `std`. `chrono` 0.4 was
considered and writes RFC 3339 into an allocated `String`. `date` is a `#![no_std]` program with no
allocator, and `jiff` formats with neither, through its own `jiff::fmt::Write`. Without its
time-zone features it handles fixed offsets only, which is the scope nife keeps.

## What is in force

| Caller | What it takes from `jiff` |
|---|---|
| `date` (`components/src/date.rs`) | civil time from Unix seconds; the five formats as `strtime` layouts |
| the clock service (`components/src/clock.rs`) | an RTC reading to Unix seconds |
| `touch -t` (`components/src/swish/navigation.rs`) | the date and time of day of an RFC 3339 operand whose shape the program checked first |
| the timetable (`crates/timetable/src/recurrence.rs`) | day numbers to civil dates and back, month lengths |
| the C library (`vendor/relibc/header/time/mod.rs`) | `gmtime` and `mktime` |
| the guest tests (`system_tests/src/user/date_tests.rs`) | `date`'s output parsed back |

What a user sees is unchanged. Each caller was compared with the removed crate on the host, the
comparison being a test harness that is not in the tree:

- `date`'s five formats, at offsets from -23:59 to +23:59, over years 0000 to 9999: byte-identical.
- The timetable's day numbers, civil dates and month lengths over the whole range and past it on
  both sides: identical, refusals included.
- `gmtime` and `mktime` on 400,000 timestamps and 400,000 `struct tm` values: identical.
- `touch -t` on fixed edge cases, three million mutated operands and one million generated ones:
  the same instant accepted, or the same operand refused.

The range stays years 0000 through 9999, kept by the callers through civil arithmetic from the
epoch, because a `jiff::Timestamp` ends 26 hours before the end of 9999. A leap second is refused,
and negative timestamps are ordinary. notes/calendar.md states the rule.

The eleven Kani harnesses and the eleven falsification patches went with the crate. No first-party
code on the verification path depended on them: §46's amendment of 2026-10-04 makes taking a crate
the default outside the kernel and the crates Kani proves, and the calendar is neither.

## BUGS

- jiff's Temporal parser accepts more than RFC 3339 (a leap second clamped to `:59`, an offset
  without a colon or with seconds, the compact form, bracketed annotations). `touch -t` checks the
  operand's shape itself before it hands jiff anything, so that check is first-party code with no
  proof behind it, only tests.
- The lock file names `portable-atomic`, `portable-atomic-util` and `jiff-static`. None of them is
  built for this tree's targets, since all three have pointer-width atomics and `jiff-static` sits
  behind `cfg(any())`, but each is in `Cargo.lock` and the supply-chain gates read it.

## Follow-on

- **Proposed, for the maintainer.** Several blocks a lane may not edit still name the crate:
  milestone 51 (wall-clock time) says the library half is `crates/calendar`; milestone 1017 (the
  contract crates leave in this order) lists `calendar` in step 8 and among the contribute crates;
  milestone 857 (crates released on their own) lists it under fork 8; milestone 956 (the linked
  interfaces carry checkable versions) counts it among crates that stay Rust APIs. Each wants its
  line corrected to the crate being gone.

## Index row

nife stops writing its own calendar: `date`, the clock service, `touch -t`, the timetable and the C library's `gmtime` and `mktime` use `jiff` with default features off, and what a user sees is byte-for-byte unchanged.
