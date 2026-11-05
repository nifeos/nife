---
status: PROPOSED
raised: 2026-11-05
milestone_dependencies: 1034
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# A moved crate is fuzzed in its own repository

Raised by lane/crate-move-gaps on 2026-11-05 (UTC), closing a gap the crate move runbook's `BUGS`
named. The integrator mints the number at promotion. The title and slug are drafts.

Reuse: the shared template's `weekly / fuzz` job, which already runs cargo-fuzz when a caller passes
`fuzz: true`; cargo-fuzz's own layout; and `script/crate-move`
([milestone 1034 (a runbook for moving a crate out of nife)](../1034-a-crate-move-runbook.md)),
which stages the crate repository. It writes no new job.

## Background

nife fuzzes its parsers with cargo-fuzz. Every target lives in this tree's `fuzz/`, with its
seeds in `fuzz/seeds/<target>`, and `script/fuzz` lists them. The `fuzz` shards of `ci.yml` run
each target for 60 seconds from the committed seeds on every merge group, and the aggregate is a
required check ([notes/fuzzing.md](../../../notes/fuzzing.md)).

A crate that moves out ([the runbook](../../../notes/crate-move.md)) leaves its fuzz targets behind,
and nothing says where they go. `file_allocation_table`, the next crate to move, has two:
`file_allocation_table_boot_sector` and `file_allocation_table_writer`. Left as they are, after the
move they fuzz the crate at the commit nife pins, inside nife's merge queue.

The premise needed one correction. The continuous fuzz host does not run these targets. It runs the
in-guest syscall drivers against the kernel, and its filer files private advisories on nifeos/nife
alone ([notes/fuzzing/continuous-host.md](../../../notes/fuzzing/continuous-host.md)). A cargo-fuzz
crash, in nife or in a crate repository, is a red hosted job with a public log. The template also
has a fuzz job already, `weekly / fuzz`, which has run five minutes per target since the template's
first release; no crate turns it on, since the pilot `jh7110_entropy` parses no bytes.

## What was measured

- nife's merge queue ran `ci.yml` 300 times between 2026-11-01 and 2026-11-05 (UTC), about 62 runs
  a day. No cargo-fuzz shard failed in any of them. The red fuzz jobs in that window were all the
  in-guest `fuzz-guests` shards (26 jobs in 25 runs, two runs unreadable), which are not cargo-fuzz
  and stay in nife.
- The cargo-fuzz bugs on record, in [the three bugs](../../../notes/fuzzing/three-bugs.md), came from
  writing a target or from a long run by hand. None came from the 60-second shard.
- The continuous host has filed no advisory and published no `fuzz-*` release; its filer is built
  and not yet installed.
- Time per target per week: nife's queue gives each target about 62 minutes a day from the same
  seeds, roughly seven hours a week. `weekly / fuzz` gives five minutes a week.

## Prior art

- cargo-fuzz puts the targets beside the crate. `cargo fuzz init` writes `fuzz/Cargo.toml` with
  `[dependencies.<crate>] path = ".."` (its `templates.rs`), and `cargo fuzz run <target>` reads
  `fuzz/corpus/<target>` by default (its `project.rs`). Read from the cargo-fuzz source.
- OSS-Fuzz asks for the targets in the project's own repository. Its ideal-integration page says
  the targets "should be part of the project's source code repository", and calls hosting them in
  the OSS-Fuzz repository a fallback that "may quickly bitrot". Bug reports go to the contacts in
  each project's own `project.yaml`.
- image-rs splits its crates the way nife now does. `image-rs/image-png` and `image-rs/image` are two
  repositories and two OSS-Fuzz projects, each with its own `main_repo` and contact, and
  `image-png/fuzz/` holds the decoder's own targets against `path = ".."`. The `image` repository
  keeps a target that decodes PNG through its dependency, `fuzzer_script_png.rs`, so a PNG bug it
  finds lands with the consumer first, which forwards it upstream.
- nife's own flow files a fuzz finding where the fix lands: one private advisory on the repository
  that holds the code, worked by a fix lane in that advisory's private fork
  ([milestone 1015 (fuzz findings filed with no person in the loop)](../1015-fuzz-findings-without-a-person.md)).

## The options

A. Keep the targets in nife during the soak. `fuzz/Cargo.toml` depends on the git pin, and the
queue keeps fuzzing the crate, nearly 90 times as long each week as option C. It loses for three
reasons. A crash turns a required check red for every nife pull request, yet nife cannot fix the code: the
fix takes a crate pull request, a tag and a pin bump, in two repositories. nife fuzzes the pinned
commit while the crate's `main` moves ahead unfuzzed. And nife leaves this repository bottom-up
(milestone 857 (crates released on their own) rules it), so the targets would move a second time. The extra fuzz time found
nothing in the measured window.

B. A short fuzz job on every pull request in the template's `check`. It would match nife's shard
per change, at a cost every crate pays on every pull request whether it parses bytes or not. It is
an addition to C, not an alternative, and nothing measured asks for it yet.

C. The targets move with the crate, in the move itself. They live in the crate repository's
`fuzz/`, in cargo-fuzz's layout, and the weekly caller passes `fuzz: true`. A crash is red where the
fix lands. The weekly run already counts toward the soak (condition 2), so fuzzing is part of the
evidence the soak ends on, with no new condition. A long run by hand stays the way to search
deeper, as in nife.

## Provisional ruling

Option C, decided by the maintainer, provisional. Moving the targets back is one pull request in
each repository, and nothing outside the two repositories acts on where they live. The
recommendation is not about effort: option A is less work at the move and loses on where a finding
lands.

## What it builds, with `file_allocation_table` as the example

1. `script/crate-move` stages the crate's targets. For `file_allocation_table` the staged tree
   gains:
   - `fuzz/Cargo.toml`, depending on `file_allocation_table = { path = ".." }` and on
     `libfuzzer-sys` at nife's requirement, `"0.4"`;
   - `fuzz/fuzz_targets/file_allocation_table_boot_sector.rs` and
     `fuzz/fuzz_targets/file_allocation_table_writer.rs`, with their names unchanged;
   - `fuzz/corpus/file_allocation_table_boot_sector/`, holding `small-tree.img` and the
     dosfstools-made boot sectors from `tests/fixtures`, which nife's `script/fuzz` passes as a
     second seed directory and the template's job does not;
   - `fuzz/corpus/file_allocation_table_writer/`, holding its two seeds;
   - `fuzz/make-fat12-tree.py`, the script that writes `small-tree.img`;
   - `fuzz/README.md`, holding the two targets' sections from
     [notes/fuzzing/targets.md](../../../notes/fuzzing/targets.md), since text about one crate goes
     into that crate (§310 (read a note against its code)). The boot sector target's doc comment
     points there instead of at this tree's note.
   - `.github/workflows/weekly.yml` with `fuzz: true`.
2. The dry run builds the targets with `cargo +nightly fuzz build` and runs each over its corpus
   once (`-runs=0`), so a target that no longer compiles outside nife fails before anyone creates
   the repository.
3. The nife-side pull request (the runbook's step 14) deletes the two targets from
   `fuzz/Cargo.toml` with the crate's dependency line, their names and the fixtures case from
   `script/fuzz`, `fuzz/seeds/file_allocation_table_*`, `fuzz/seeds/make-fat12-tree.py` with its
   paragraph in `fuzz/seeds/README.md`, and both sections of `notes/fuzzing/targets.md`. The dry run's
   follow-up list names each.
4. The runbook's step 14 says the targets moved, and its `BUGS` entry on the fuzz targets goes.

## Exit

`script/crate-move --dry-run --into <scratch> file_allocation_table` stages `fuzz/` as above and
runs both targets over their corpus. After the move, the crate repository's first `weekly` run
shows `weekly / fuzz` succeeded, and `script/fuzz --list` in nife names neither target.

## BUGS

- A crash in `weekly / fuzz` prints its input to a public log, as nife's shard does today. A
  finding meant to stay private has no path in a crate repository: the fuzz host's App and filer
  reach nifeos/nife alone, and widening either is a credential decision for the architect, not this
  milestone.
- `weekly / fuzz` does not keep its corpus between runs, as nife's shard does not, so each week
  starts from the committed seeds.
