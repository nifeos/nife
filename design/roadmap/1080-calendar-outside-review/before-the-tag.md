# P1: before the first tag

An appendix to [the milestone](../1080-calendar-outside-review.md).

4. `UtcOffset::from_hm` multiplies before it checks, so a large hour overflows.
   Finding: `from_hm(hours, minutes)` checks the sign pairing and `minutes`' magnitude, then
   calls `from_minutes(hours * 60 + minutes)` (`src/lib.rs` line 432) with `hours` unbounded. In this
   tree, where release builds keep overflow checks on, `from_hm(71_582_789, 0)` panics; in a
   release build with the default profile it wraps to 44 and returns `Ok(+00:44)`, and
   `from_hm(i32::MIN, 0)` returns `Ok(UTC)`. A validating constructor that accepts a wrong value
   is the one thing the crate's documentation says it never does. No caller in the tree calls
   `from_hm` with anything but a literal, and the parser bounds its own hours to two digits, so
   no program reaches it today. The Kani harnesses never call `from_hm`.
   Evidence: `src/lib.rs` line 425 to `:433`; the probe runs above.
   Change: refuse `hours` outside `-23..=23` before the multiply (one compare; the range is the
   grammar's, as the type's doc already says), keep the sign check, and add the two probe values
   and `from_hm(23, 59)` to `tests::utc_offsets`. Add one harness,
   `an_offset_from_hours_and_minutes_is_the_same_offset_in_minutes`: for symbolic `hours` and
   `minutes`, the call never panics and, when it returns `Ok`, `minutes()` equals
   `hours * 60 + minutes` computed in `i64`; ship it with its falsification patch (drop the new
   bound). The spelling of the constructor is item 5's.
   Door: two-way (only inputs that today panic or wrap change their answer). Effort: S.

5. Names the rules refuse or a stranger misreads.
   Finding, in the order the checks report them. `FMT_CAP` (`src/lib.rs` line 522) fails the
   spelled-out rule (`cap` is `capability`) and the unit rule (a limit carries its unit);
   `helpers/vendored_name_gates.py` line 156 already carries it as a known violator at a ceiling. The
   `len` fields of `Formatted` (`:529`), `Writer` (`:572`) and the test `Sink` (`tests.rs` line 463)
   fail the unit rule. The test binding `ist` (`tests.rs` line 243) fails the spelled-out rule
   (`interrupt_stack_table`); `pst` (`:254`) is the same kind. Beyond the checks, `Weekday::abbrev` (`:174`) is itself an abbreviation. `UtcOffset::from_hm` (`:425`) is two initials. The private `Writer::byte` and `Writer::str` (`:583`, `:590`) are `push` and `push_str` in the standard library's spelling, which the shapes rule asks for. `Civil` (`:236`) is an adjective standing alone, where every library in the table names the thing (`NaiveDateTime`, `PrimitiveDateTime`, `civil::DateTime`). The abbreviations report lists
   `unix`, `utc`, `rfc` and `iso` as not words; each is a name nobody says in full, so the
   repository's `.github/accepted-words` should list them with that reason rather than rename
   them. `3600` and `60` appear as literals in `from_unix` and `to_unix` (`:339` to `:341`,
   `:349` to `:350`) beside a named `SECONDS_PER_DAY`.
   Change, as recommendations for the maintainer's provisional ruling and the architect's
   ratification at the release review: `FMT_CAP` to `FORMATTED_CAPACITY_BYTES`; the three `len` fields to `used_bytes`; `ist` and `pst` to `plus_five_thirty` and `minus_eight`. Rename `abbrev` to `abbreviation`, `from_hm` to `from_hours_and_minutes`, and `Writer::byte` and `Writer::str` to `push` and `push_str`. Rename `Civil` to `CivilDateTime`, with `DateTime` kept (it is a civil reading plus the offset it came with, and the doc says so). Add `SECONDS_PER_HOUR` and `SECONDS_PER_MINUTE` as public constants beside `SECONDS_PER_DAY`. Before the first release no
   `#[doc(alias)]` carries the old spelling, per the naming rules. The `Civil` rename touches
   five callers in this tree and lands with the move's pin bump or before it.
   Door: two-way until the tag; one-way after. Effort: S inside the crate, M with the callers.

6. Callers cannot display or box `Error`, and the value types miss the traits a user reaches for.
   Finding: `Error` derives `Clone, Copy, PartialEq, Eq, Debug` and offers `as_str`
   (`src/lib.rs` line 50 to `:90`). It implements neither `core::fmt::Display` nor
   `core::error::Error`, so `?` into a caller's error type, `anyhow`, and `eprintln!("{e}")` all
   fail to compile; every caller in this tree writes `e.as_str()` or `{e:?}`. `Weekday` has no `PartialOrd`, `Ord` or `Hash`. `Civil` has no `Hash` although it is `Eq` and `Ord`. `Formatted` has no `PartialEq` although it is `Copy`. `DateTime`'s `Eq` is on the whole value, so two readings of one instant at different offsets are unequal, which is right, though no doc a user reads says so.
   Evidence: the derive lines at `:50`, `:153`, `:235`, `:403`, `:448`, `:526`.
   Change: `impl core::fmt::Display for Error` writing `as_str`, and `impl core::error::Error for
   Error` (stable since Rust 1.81, under the crate's 1.85). Derive `Hash` on `Civil`, `Weekday`,
   `UtcOffset` and `Error`; `PartialOrd` and `Ord` on `Weekday` in the enum's own order (Sunday
   first), with the doc saying so; `PartialEq` and `Eq` on `Formatted` by the bytes. One sentence
   on `DateTime` about equality, and `DateTime::to_unix` already gives the instant to compare.
   Adding a trait implementation is compatible.
   Door: two-way. Effort: S.

7. The civil date of a day number is reachable only through the expensive division.
   Finding: `Civil::days_since_epoch` is public and its inverse is the private
   `civil_from_days`. A caller holding a day number multiplies by 86,400 and calls `from_unix`,
   which divides it back out. `crates/timetable/src/recurrence.rs` line 166 to `:174` does exactly that. Its comment keeps the monthly half of its logic "free of the calendar so Kani can
   reach it", because the division is what the proofs note measured as the expensive part.
   `vendor/relibc/header/time/mod.rs` line 519 to `:529` builds `mktime` from a day number the same
   way. Both callers would take a day-number constructor.
   Evidence: the two callers; `src/lib.rs` line 133 and `:354` to `:357`;
   `notes/verification/calendar-proofs.md`, the section on where the cost is.
   Change: `Civil::from_days_since_epoch(days: i64) -> Result<Civil, Error>` returning midnight
   of that day, `OutOfRange` outside `FIRST_DAY..=LAST_DAY` (the two constants exist in the
   verification module and become public as `MINIMUM_DAY` and `MAXIMUM_DAY`), with a doc test
   and a line in `every_real_date_survives_its_own_day_number` asserting the round trip through
   it. The timetable then loses its multiply and its proof gets cheaper.
   Door: two-way. Effort: S.

8. The mutation record travels as a comment, and the four survivors need their reasons.
   Finding: the dry run carries the baseline row into `.github/mutants-exempts` as a comment and says the first weekly run will list the survivors. This tree already argues and re-runs the four survivors. One is the `+` to `-` at the offset-length guard in `parse_rfc3339_bytes` (`number` and `expect` bound the same bytes behind it). Two are the `<` to `<=` in `from_hm` (masked by the `!= 0` clauses, and item 4 changes that code). One is the sign branch in `Writer::offset` at zero (both format paths print `Z` or `UTC` before a zero offset reaches it). The template's verdict refuses a line without a reason and a line naming a survivor that
   no longer exists, so the lane writes the list once the first run names the lines.
   Evidence: `notes/project-metrics/mutation-survivor-classes.csv:19` to `:21`;
   `notes/mutation-testing/baseline-survivors-abi-to-ipc.md`, the `calendar` section;
   `scaffold/.github/mutants-exempts`.
   Change: after the first `mutation / mutation` run on `main`, write the exemption lines with
   the four reasons above, re-checking the `from_hm` pair against item 4's new code (the new
   bound may kill them). Dispatch the run before the tag so the release gate has a green full run.
   Door: two-way. Effort: S.

9. The tests and the hostile-bytes proof stop short of the longest legal input.
   Finding: `parse_is_total_on_hostile_bytes` quantifies over lengths up to 26 (`src/lib.rs` line 1059),
   and the shortest input that carries both a fraction and a numeric offset is 27 bytes, so the proof's success path never sees that combination. Neither does any unit test
   (`tests::parse_accepts_rfc3339` pairs every fraction with `Z`). The probe shows it parses
   correctly. No test pins `DateTime::from_unix` at `MINIMUM_TIMESTAMP` with a negative offset
   (`tests::utc_offsets` pins the upper edge), nor `UtcOffset::from_minutes(1439)`.
   Evidence: `src/lib.rs` line 1056 to `:1072`; `src/tests.rs` line 285 to `:315`, `:370` to `:392`.
   Change: raise the harness's `N` to 28 and its unwind to match, and measure the cost. The proofs note says length drives it. If the run leaves the budget, add a second harness fixed at 27 bytes with the fraction and offset positions concrete. Add the three rows to the tests.
   Door: two-way. Effort: S.
