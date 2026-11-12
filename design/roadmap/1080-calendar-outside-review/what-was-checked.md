# What was checked, and how

An appendix to [the milestone](../1080-calendar-outside-review.md).

Verified firsthand, on a fresh shallow clone of `nifeos/nife` at the head of `main`
(`137639a3ac4d`), with the template `nifeos/crate-workflows` cloned beside it:

- `cargo test -p calendar`: 21 unit tests and 3 documentation tests pass, in the dev profile and
  in release. `cargo clippy -p calendar --all-targets -- -D warnings`, `cargo doc` with warnings
  denied, and `cargo rustc -- -D missing_docs` are clean. `typos` finds nothing.
- The crate builds for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and
  `x86_64-unknown-none`. It has no dependencies and no `unsafe`.
- Kani 0.68.0 (CBMC 6.11.0) over the eleven harnesses: all eleven verify, about ten minutes of
  solver time in sequence on this host. The two costly ones are
  `unix_to_civil_and_back_is_the_identity` (195 seconds, the one harness that pays for the 86,400
  division) and `every_format_is_ascii` (95 seconds); the two parser harnesses take 62 and 70
  seconds; the rest are under 70 seconds each. The review replayed two falsification patches and each turned exactly its own harness red (`the_calendar_algorithms_are_mutual_inverses`, `a_day_number_always_decodes_to_a_real_date`). It did not replay the other nine. What the proofs
  do not cover: `UtcOffset::from_hm` (item 4), `DateTime::from_unix` with a non-zero offset, and
  parser inputs longer than 26 bytes (item 9).
- The template's name checks over `src/lib.rs` and `src/tests.rs`: `spelled_out` refuses two names
  (`FMT_CAP`, the test binding `ist`); `register_names` refuses three (`len` fields in `Formatted`,
  `Writer` and the test `Sink`); `unit_symbols`, `british_spelling`, `question_shape`,
  `service_model` and `contract_names` refuse none. The abbreviations report lists 53 components
  that are not words, most of them `unix`, `utc`, `rfc`, `dt` and `buf`.
- `helpers/standalone.py --manifest-dir crates/calendar`: eleven lines, listed in item 3.
- `script/crate-move --dry-run --no-build --template <checkout> calendar`, at template `v7`:
  seven findings, among them one regenerated falsification patch that `git apply --check`
  refuses (item 2), and the nife-side list of six manifests and about fifty other files that name
  the crate.
- A probe crate outside the workspace, depending on the crate by path, built in both profiles:
  `UtcOffset::from_hm(71_582_789, 0)` panics in the dev profile and returns `Ok(44 minutes)` in a
  release profile without overflow checks; `from_hm(i32::MIN, 0)` returns `Ok(UTC)` there (item
  4). `2026-07-30T12:34:56-00:00` parses to offset zero and prints back as `...56Z` (item 10).
  `2026-07-30T12:34:56.1+05:30`, 27 bytes, parses correctly (item 9). `size_of` is 12 bytes for
  `Civil`, 16 for `DateTime`, 33 for `Formatted`, 2 for `UtcOffset`, 1 for `Error`.
- Every caller in the tree: `components/src/date.rs` (`DateTime::from_unix`, `format`,
  `UtcOffset::from_minutes`), `components/src/clock.rs` (`Civil::new`, `to_unix`),
  `components/src/swish/navigation.rs` (`parse_rfc3339_bytes`, `to_unix`),
  `vendor/relibc/header/time/mod.rs` (`Civil::from_unix`, every accessor, `weekday`,
  `day_of_year`, `Civil::new`, `days_since_epoch`), `crates/timetable/src/recurrence.rs`
  (`Civil::from_unix`, `Civil::new`, `days_in_month`) and
  `system_tests/src/user/date_tests.rs` (both parsers).
- The mutation records: the baseline row `calendar 372 4 3 16` in `.cargo/mutants-baseline.txt` (caught, missed, timed out, unviable), the four survivors' rows in `notes/project-metrics/mutation-survivor-classes.csv` (all four classified equivalent), and the ledger's note. The note says two earlier "equivalent" verdicts in this crate were real gaps until a lane re-ran them. The review ran no census.
- crates.io, through its API: another crate holds the name `calendar` (item 1). Others hold `gregorian`, `civil` and `rfc3339` too. At the time of the review, `gregorian_calendar`, `civil_calendar`, `civil_time`, `civil_date`, `proleptic_gregorian`, `calendar_arithmetic` and `unix_calendar` were free.

Inferred from reading, not run: the minimum Rust version (edition 2024 puts it at 1.85; the host had no 1.85 toolchain to build with, so nothing confirms the number). The same holds for the peak memory of `cargo kani` (`helpers/kani-peaks.tsv` records 3381 for this crate; nothing confirms the unit). It also holds for the behavior of the prior-art crates in the table below where the cell says so.

Not run: a mutation census (outside its window), Miri, cargo-llvm-cov, a fuzz campaign (the crate
has no fuzz target), the full dry run with `--build`, and a release dry run.
