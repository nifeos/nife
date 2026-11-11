---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
## Goal

A stranger who finds this crate in its own repository can tell in one screen what it converts and what it refuses. The stranger can build and prove it from the README alone and call it from a `no_std` program with no allocator. Every public constructor either returns a value that is a real date or an `Error` that names the field, on every input and in every build profile. The repository's gates
are green on the day the crate arrives, for the crate's own reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1080-calendar-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1080-calendar-outside-review/prior-art.md).

## The items

Effort: S is half a day for a lane, M a day or two, L longer. A door is one-way when somebody
outside the repository acts on it after the move (a published name, a published behavior, a
layout, a dependency); everything else is two-way and the maintainer decides it.

### P0: blocks the move or the first green run

1. Another crate holds the crate's name on crates.io.
   Finding: `calendar` on crates.io is a scheduling library and command-line tool at 0.1.0 with
   35 downloads, updated within the last half year. A crate that leaves nife takes its Cargo
   package name into its repository name, into every consumer's pin
   (`calendar = { version = "0.1.0", git = "https://github.com/nifeos/calendar", rev = ... }`,
   the line the dry run prints for four manifests), and into its first tag. The publish hold means nothing goes out soon, and the hold is not the point. The move creates the repository and the pins. A package name that can never go out is a name the repository would have to change later, with a second repository and a second round of pins. crates.io
   does not transfer a registered name on request; only the owner can.
   Evidence: the crates.io API response for `calendar`; `script/crate-move`'s nife-side list;
   `crates/calendar/Cargo.toml:2`.
   Change: the architect picks the package name before the repository exists (the decision
   below). The move lane then renames the directory, the manifest and the six manifests that name
   it, and the move tool's follow-up list names the other files.
   Door: one-way. Effort: M for the rename across the tree, S for the manifest alone.

2. The move tool corrupts one of the eleven falsification patches, and the template's proof gate
   refuses it.
   Finding: `helpers/crate_move.py` regenerates each patch over the stripped source by running
   `diff -u` through `run()`, which returns `(stdout + stderr).strip()`
   (`helpers/crate_move.py:576`), and then joins the pieces with `.rstrip('\n')`
   (`helpers/crate_move.py:113` to `:116`). A unified diff whose last context line is blank ends
   in a line holding one space; `strip()` deletes it. The hunk header still counts it. For
   `verification.every_real_date_survives_its_own_day_number.patch` the edit is the last
   expression of `Civil::to_unix`, the third trailing context line is the blank line after the
   closing brace, and the staged patch says `@@ -335,7 +335,7 @@` over six lines. GNU `patch`
   tolerates that, so the dry run's own "patch applies exactly" step passes; `git apply --check`,
   which the template's `falsify.py --check` uses, answers `corrupt patch at line 19`. The staged
   repository's first `proofs / falsify` run is red for a reason no change in the crate removes.
   Any crate whose patch ends on a blank context line hits the same.
   Evidence: `helpers/crate_move.py:113` to `:116` and `:576`; the staged patch in the dry-run
   directory; `git apply --check -p1` on it.
   Change: in `regenerate_patch`, take `diff`'s output unstripped (or strip `\n` only), and add a selftest fixture with a blank line after its edit. Owner: nife's tooling, through its own
   queue; this block names the dependency on the blocked side.
   Door: two-way. Effort: S.

3. The crate does not stand alone: no README, and a manifest and sources that point into this
   tree.
   Finding: `standalone.py` lists eleven lines. There is no README, no `LICENSE-MIT` and no `LICENSE-APACHE`. `license.workspace = true` and `lints.workspace = true` inherit a workspace that will not be there (`Cargo.toml:6`, `:15` to `:16`). There is no `rust-version`. `publish = false` carries a comment naming `crates/publish-allowlist.txt` (`Cargo.toml:5`). The manifest's comment holds three milestone citations (`Cargo.toml:8` to `:12`). Eleven paths point into the nife tree (`notes/calendar.md` nine times, `script/verify` at `src/lib.rs:836`, `notes/verification/calendar-proofs.md` at `:843`). Six `Name:` blocks remain (the move tool strips these). Two lines name the architect (`src/lib.rs:11`, `:16`). Beyond what the script sees, the documentation leans on this tree in places a stranger cannot follow. They are "the same discipline the capability types use (a `Cap` you hold is one that was minted)" at `:232`, "a `date -s` verb" at `:247` and `:1054`, "the clock service" and "the NTP client" in the manifest comment, and `VERSION`'s doc comment at `:15` to `:18`. That comment explains the constant by a pull request number and another crate's kind.
   Evidence: `standalone.py` output; the lines above; `packages/homes.toml:130` to `:133`, which
   assigns `notes/calendar.md` and `notes/verification/calendar-proofs.md` to leave with the
   crate.
   Change: a README in the scaffold's order (`scaffold/README-crate.md`), written from `notes/calendar.md`. It holds the one-line purpose and the EXAMPLES section with the round trip and a parse. It states the range and the four refusals (zone database, `strftime`, durations, leap seconds) as the crate's own statements rather than as a note's. The section "What is proven" gives eleven lines in a user's words with the harness names. The build and proof commands carry `cargo kani`'s time and peak memory, measured on the staged tree. The README works one falsification patch as an example. It ends with the minimum Rust version and the license section. In the manifest: `license = "MIT OR
   Apache-2.0"`, `rust-version = "1.85"` (confirmed by a build at that toolchain), `[lints.rust]`
   as literals (`missing_docs = "deny"`, `unexpected_cfgs` with `cfg(kani)`), `description`,
   `repository`, `keywords` and `categories` (`date-and-time`, `no-std`), and the comment
   rewritten without milestone numbers. In the sources, every `notes/...` and `script/...` reference becomes a README section reference or the lane deletes it. `VERSION`'s doc states what it is (the formatting's generation, checked by nothing at run time) and no more. The lane rewrites the `Cap` sentence and the `date -s` sentences for a reader with no nife. Then record a stranger
   run. The two notes become the README and a `docs/` page; the proofs note's table still names
   the monotonicity harness `later_days_are_later_dates`, and the harness is
   `each_day_sorts_after_the_one_before_it` (`src/lib.rs:936`), so the copy fixes that line.
   Door: two-way. Effort: M.

### P1: before the first tag

4. `UtcOffset::from_hm` multiplies before it checks, so a large hour overflows.
   Finding: `from_hm(hours, minutes)` checks the sign pairing and `minutes`' magnitude, then
   calls `from_minutes(hours * 60 + minutes)` (`src/lib.rs:432`) with `hours` unbounded. In this
   tree, where release builds keep overflow checks on, `from_hm(71_582_789, 0)` panics; in a
   release build with the default profile it wraps to 44 and returns `Ok(+00:44)`, and
   `from_hm(i32::MIN, 0)` returns `Ok(UTC)`. A validating constructor that accepts a wrong value
   is the one thing the crate's documentation says it never does. No caller in the tree calls
   `from_hm` with anything but a literal, and the parser bounds its own hours to two digits, so
   no program reaches it today. The Kani harnesses never call `from_hm`.
   Evidence: `src/lib.rs:425` to `:433`; the probe runs above.
   Change: refuse `hours` outside `-23..=23` before the multiply (one compare; the range is the
   grammar's, as the type's doc already says), keep the sign check, and add the two probe values
   and `from_hm(23, 59)` to `tests::utc_offsets`. Add one harness,
   `an_offset_from_hours_and_minutes_is_the_same_offset_in_minutes`: for symbolic `hours` and
   `minutes`, the call never panics and, when it returns `Ok`, `minutes()` equals
   `hours * 60 + minutes` computed in `i64`; ship it with its falsification patch (drop the new
   bound). The spelling of the constructor is item 5's.
   Door: two-way (only inputs that today panic or wrap change their answer). Effort: S.

5. Names the rules refuse or a stranger misreads.
   Finding, in the order the checks report them. `FMT_CAP` (`src/lib.rs:522`) fails the
   spelled-out rule (`cap` is `capability`) and the unit rule (a limit carries its unit);
   `helpers/vendored_name_gates.py:156` already carries it as a known violator at a ceiling. The
   `len` fields of `Formatted` (`:529`), `Writer` (`:572`) and the test `Sink` (`tests.rs:463`)
   fail the unit rule. The test binding `ist` (`tests.rs:243`) fails the spelled-out rule
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
   (`src/lib.rs:50` to `:90`). It implements neither `core::fmt::Display` nor
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
   which divides it back out. `crates/timetable/src/recurrence.rs:166` to `:174` does exactly that. Its comment keeps the monthly half of its logic "free of the calendar so Kani can
   reach it", because the division is what the proofs note measured as the expensive part.
   `vendor/relibc/header/time/mod.rs:519` to `:529` builds `mktime` from a day number the same
   way. Both callers would take a day-number constructor.
   Evidence: the two callers; `src/lib.rs:133` and `:354` to `:357`;
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
   Finding: `parse_is_total_on_hostile_bytes` quantifies over lengths up to 26 (`src/lib.rs:1059`),
   and the shortest input that carries both a fraction and a numeric offset is 27 bytes, so the proof's success path never sees that combination. Neither does any unit test
   (`tests::parse_accepts_rfc3339` pairs every fraction with `Z`). The probe shows it parses
   correctly. No test pins `DateTime::from_unix` at `MINIMUM_TIMESTAMP` with a negative offset
   (`tests::utc_offsets` pins the upper edge), nor `UtcOffset::from_minutes(1439)`.
   Evidence: `src/lib.rs:1056` to `:1072`; `src/tests.rs:285` to `:315`, `:370` to `:392`.
   Change: raise the harness's `N` to 28 and its unwind to match, and measure the cost. The proofs note says length drives it. If the run leaves the budget, add a second harness fixed at 27 bytes with the fraction and offset positions concrete. Add the three rows to the tests.
   Door: two-way. Effort: S.

### P2: later

The P2 items are in [the appendix](1080-calendar-outside-review/later-items.md).

## Decisions for the architect

One decision is a one-way door. Every other name in this block is provisional until the release
review, where the naming rules put it.

### The package name, since `calendar` is taken on crates.io

Background. A crate that leaves nife gets a repository under `nifeos` and a tag, and four manifests here pin it by that repository's address and the package name. Its package
name is the one crates.io would publish it under. On crates.io `calendar` belongs to another author's scheduling library, and crates.io does not transfer a registered name on request. The publish hold does not change this. The move makes the repository and the pins. A package name that can never go out is one the repository would have to change later, with a second
repository, a second tag and a second bump of every pin. The ratified nife name `calendar` stays right for the directory it leaves from. This decision is the name it leaves under.

Recommendation: `gregorian_calendar`. It says what the crate is, the proleptic Gregorian calendar
over Unix seconds, in a term every reader knows and the naming rules prefer for an external
standard (`network_time_protocol` keeps its word). It keeps the ratified noun. It is free. Second
choice `civil_calendar`, the adjective Hinnant and `jiff` use for a date with no zone, also free;
`civil_time` and `proleptic_gregorian` are free but name a kind of time or a rule rather than a
thing. Others already hold `gregorian`, `civil` and `rfc3339`.

Prior art: `httpdate` and `humantime` name what they read and write; `chrono`, `time` and `jiff` carry names chosen for a brand; Google's Go package for a zoneless date is `civil`;
Hinnant's library is `date`.

Concrete example: the manifest line becomes `name = "gregorian_calendar"`, the four consumer
manifests pin `gregorian_calendar = { version = "0.1.0", git =
"https://github.com/nifeos/gregorian_calendar", rev = "<commit of the tag>" }`, and the `use`
lines in the six callers change the crate path and nothing else.

## Done when

The checks are in [the appendix](1080-calendar-outside-review/done-when.md).

## BUGS

- The minimum Rust version is an inference from the edition; no build confirms it. The move lane confirms it at
  `cargo +1.85 test` before writing it.
- The dry run reports two architect-name lines, twelve nife-path lines and five `nife` mentions
  whose first hits are in the scaffold's own files at template `v7`
  (`.github/pull_request_template.md`, `CONTRIBUTING.md`, `.githooks/commit-msg`, `deny.toml`,
  `docs/stranger-test.md`, `rustfmt.toml`). Whether those are the template's at `v7` or an
  artifact of reading a local checkout was not verified; the template's own lint is where they
  belong if they are real.
- The prior-art table's unverified cells are from memory of each library's documentation. A lane
  that writes the README's comparison checks each one before stating it.
- `helpers/kani-peaks.tsv` records 3381 for this crate with no unit in the row read here; the
  README states the figure only after measuring it on the staged tree.
- Item 1's facts about crates.io are a snapshot. The owner could yank or transfer the registered `calendar`, and anyone could take any free candidate, between this review and the repository's
  creation; the move lane re-checks the chosen name on the day.

## Index row

An outside review of `calendar` before it leaves. Another crate holds the package name on crates.io, which needs the architect. The move tool corrupts one falsification patch that the template's proof gate refuses. A README and a standalone manifest are missing. `UtcOffset::from_hm` overflows on a large hour. Nine further items cover names the rules refuse, `Error` without `Display`, a
day-number constructor two callers want, the mutation exemptions, the proof's input length,
`-00:00`, `forbid(unsafe_code)` with a fuzz target, and a weekday table test.
