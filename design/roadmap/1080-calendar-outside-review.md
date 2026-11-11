---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1080. `calendar`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/calendar` as a stranger to this tree would meet it. The
review read the crate's fourteen tracked files, every caller in the tree, the naming rules, the
crate template and the two notes that leave with the crate, and ran a static dry run of the move.
It changes nothing; it lists what the next lanes change.

There is no review package for this crate in `notes/crate-reviews/`. The accepted baseline is
therefore what the tree already records, and this block does not reopen it. The baseline is the
crate name as a nife name (ratified in the module block of `crates/calendar/src/lib.rs`) and the
five ratified constants (`MINIMUM_YEAR`, `MAXIMUM_YEAR`, `MINIMUM_TIMESTAMP`, `MAXIMUM_TIMESTAMP`,
`SECONDS_PER_DAY`). It also holds the scope rulings of milestone 51 (wall-clock time, the `date`
command, and an NTP service) that `notes/calendar.md` states. Those rulings are years 0000 through
9999, proleptic Gregorian, a fixed offset and no zone database, no `strftime`, and a leap second
that is an error with its own name. Last come the refusal recorded as milestone 484 (IANA zone
rules, and `strftime`) and the `VERSION` constant of milestone 956 (the linked interfaces carry
checkable versions) for a library with no peer. Where a later rule contradicts
that baseline, the item says so; item 5 does (`FMT_CAP` against the spelled-out and unit rules).
The crate's step in the order of milestone 1017 (the contract crates leave in this order) is step
8, with `environment_contract`, and its disposition there is contribute; this block changes
neither.

Reuse: the template `nifeos/crate-workflows` and its helpers, `script/crate-move --dry-run`,
Kani, cargo-mutants, cargo-fuzz, and the two notes `packages/homes.toml` already assigns to leave
with the crate (`notes/calendar.md`, `notes/verification/calendar-proofs.md`). No new tool.

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
   `crates/calendar/Cargo.toml` line 2.
   Change: the architect picks the package name before the repository exists (the decision
   below). The move lane then renames the directory, the manifest and the six manifests that name
   it, and the move tool's follow-up list names the other files.
   Door: one-way. Effort: M for the rename across the tree, S for the manifest alone.

2. The move tool corrupts one of the eleven falsification patches, and the template's proof gate
   refuses it.
   Finding: `helpers/crate_move.py` regenerates each patch over the stripped source by running
   `diff -u` through `run()`, which returns `(stdout + stderr).strip()`
   (`helpers/crate_move.py` line 576), and then joins the pieces with `.rstrip('\n')`
   (`helpers/crate_move.py` line 113 to `:116`). A unified diff whose last context line is blank ends
   in a line holding one space; `strip()` deletes it. The hunk header still counts it. For
   `verification.every_real_date_survives_its_own_day_number.patch` the edit is the last
   expression of `Civil::to_unix`, the third trailing context line is the blank line after the
   closing brace, and the staged patch says `@@ -335,7 +335,7 @@` over six lines. GNU `patch`
   tolerates that, so the dry run's own "patch applies exactly" step passes; `git apply --check`,
   which the template's `falsify.py --check` uses, answers `corrupt patch at line 19`. The staged
   repository's first `proofs / falsify` run is red for a reason no change in the crate removes.
   Any crate whose patch ends on a blank context line hits the same.
   Evidence: `helpers/crate_move.py` line 113 to `:116` and `:576`; the staged patch in the dry-run
   directory; `git apply --check -p1` on it.
   Change: in `regenerate_patch`, take `diff`'s output unstripped (or strip `\n` only), and add a selftest fixture with a blank line after its edit. Owner: nife's tooling, through its own
   queue; this block names the dependency on the blocked side.
   Door: two-way. Effort: S.

3. The crate does not stand alone: no README, and a manifest and sources that point into this
   tree.
   Finding: `standalone.py` lists eleven lines. There is no README, no `LICENSE-MIT` and no `LICENSE-APACHE`. `license.workspace = true` and `lints.workspace = true` inherit a workspace that will not be there (`Cargo.toml` line 6, `:15` to `:16`). There is no `rust-version`. `publish = false` carries a comment naming `crates/publish-allowlist.txt` (`Cargo.toml` line 5). The manifest's comment holds three milestone citations (`Cargo.toml` line 8 to `:12`). Eleven paths point into the nife tree (`notes/calendar.md` nine times, `script/verify` at `src/lib.rs` line 836, `notes/verification/calendar-proofs.md` at `:843`). Six `Name:` blocks remain (the move tool strips these). Two lines name the architect (`src/lib.rs` line 11, `:16`). Beyond what the script sees, the documentation leans on this tree in places a stranger cannot follow. They are "the same discipline the capability types use (a `Cap` you hold is one that was minted)" at `:232`, "a `date -s` verb" at `:247` and `:1054`, "the clock service" and "the NTP client" in the manifest comment, and `VERSION`'s doc comment at `:15` to `:18`. That comment explains the constant by a pull request number and another crate's kind.
   Evidence: `standalone.py` output; the lines above; `packages/homes.toml` line 130 to `:133`, which
   assigns `notes/calendar.md` and `notes/verification/calendar-proofs.md` to leave with the
   crate.
   Change: a README in the scaffold's order (`scaffold/README-crate.md`), written from `notes/calendar.md`. It holds the one-line purpose and the EXAMPLES section with the round trip and a parse. It states the range and the four refusals (zone database, `strftime`, durations, leap seconds) as the crate's own statements rather than as a note's. The section "What is proven" gives eleven lines in a user's words with the harness names. The build and proof commands carry `cargo kani`'s time and peak memory, measured on the staged tree. The README works one falsification patch as an example. It ends with the minimum Rust version and the license section. In the manifest: `license = "MIT OR
   Apache-2.0"`, `rust-version = "1.85"` (confirmed by a build at that toolchain), `[lints.rust]`
   as literals (`missing_docs = "deny"`, `unexpected_cfgs` with `cfg(kani)`), `description`,
   `repository`, `keywords` and `categories` (`date-and-time`, `no-std`), and the comment
   rewritten without milestone numbers. In the sources, every `notes/...` and `script/...` reference becomes a README section reference or the lane deletes it. `VERSION`'s doc states what it is (the formatting's generation, checked by nothing at run time) and no more. The lane rewrites the `Cap` sentence and the `date -s` sentences for a reader with no nife. Then record a stranger
   run. The two notes become the README and a `docs/` page; the proofs note's table still names
   the monotonicity harness `later_days_are_later_dates`, and the harness is
   `each_day_sorts_after_the_one_before_it` (`src/lib.rs` line 936), so the copy fixes that line.
   Door: two-way. Effort: M.

### P1: before the first tag

The P1 items are in [the appendix](1080-calendar-outside-review/before-the-tag.md).

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
