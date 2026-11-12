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

The ruling of 2026-11-12 (UTC) under §46 (thin primitives or whole subsystems): nife does not write
its own calendar. This milestone deletes `crates/calendar`, and every caller uses the third-party
crate `jiff` 0.2 (Unlicense OR MIT) with its default features off. The ruling approves that
dependency for these callers. The branch is `milestone/1105-calendar-becomes-jiff`.

Reuse: this milestone takes `jiff` 0.2. `time` 0.3 lost because it formats only with `std`. `chrono`
0.4 lost because it writes RFC 3339 into an allocated `String`. `date` is a `#![no_std]` program with no
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

What a user sees stays the same. A host harness outside the tree compared each caller with the
removed crate:

- `date`'s five formats, at offsets from -23:59 to +23:59, over years 0000 to 9999: byte-identical.
- The timetable's day numbers, civil dates and month lengths over the whole range and past it on
  both sides: identical, refusals included.
- `gmtime` and `mktime` on 400,000 timestamps and 400,000 `struct tm` values: identical.
- `touch -t` on fixed edge cases, three million mutated operands and one million generated ones:
  the same instant accepted, or the same operand refused.

The range stays years 0000 through 9999. The callers keep it through civil arithmetic from the
epoch, because a `jiff::Timestamp` ends 26 hours before the end of 9999. `touch -t` refuses a leap
second, and negative timestamps are ordinary. notes/calendar.md states the rule.

The crate's eleven Kani harnesses and eleven falsification patches leave with it. Nothing on the
verification path needed them. §46's amendment of 2026-10-04 makes taking a crate the default
outside the kernel and the crates Kani proves, and the calendar is neither.

## BUGS

- jiff's Temporal parser accepts more than RFC 3339: a leap second clamped to `:59`, an offset
  without a colon or with seconds, the compact form, bracketed annotations. `touch -t` checks the
  operand's shape itself before it hands jiff anything. That check is first-party code with tests
  and no proof.
- The lock file names `portable-atomic`, `portable-atomic-util` and `jiff-static`. No target of
  this tree builds them: all three targets have pointer-width atomics, and `jiff-static` sits behind
  `cfg(any())`. Each still appears in `Cargo.lock`, and the supply-chain gates read it.

## Follow-on

- **Proposed.** Four blocks still name the crate as present: milestones 51, 857, 956 and 1017.
  `design/roadmap/proposals/blocks-that-name-the-removed-calendar-crate.md` lists each edit.

## Index row

nife stops writing its own calendar: `date`, the clock service, `touch -t`, the timetable and the C library's `gmtime` and `mktime` use `jiff` with default features off, and what a user sees is byte-for-byte unchanged.
