# Fuzzing the parse surface: the appendices to notes/fuzzing.md

[`notes/fuzzing.md`](../fuzzing.md) carries what fuzzing finds that Kani does not, which parsers
are fuzzed, the three bugs in summary, the CI job, corpus discipline, examples and every limitation.
These files carry the evidence its prose budget has no room for.

- [`kani-and-fuzzing.md`](kani-and-fuzzing.md): three crates where the bound is the whole story.
- [`not-fuzzed.md`](not-fuzzed.md): the parsers deliberately not fuzzed, each with its reason.
- [`why-cargo-fuzz.md`](why-cargo-fuzz.md): the three candidates, and the one license exception.
- [`three-bugs.md`](three-bugs.md): each bug, its fix and how it was found.
- [`ci-budget.md`](ci-budget.md): the exec rates behind sixty seconds, and the revert test that
  showed the job is a sweep.
- [`elf-seeds.md`](elf-seeds.md): why there is one ELF seed per machine, and what a seed buys.
- [`targets.md`](targets.md): the longer reasons behind each parser target.
- [`the-syscall-driver.md`](the-syscall-driver.md): the seeded syscall driver's oracle, its corpus
  as measured, and its `BUGS` in full.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default, `notes/X/` beside
`notes/X.md`. calef names directories.*

Each stem below is named for its content, and calef names files.

- `kani-and-fuzzing` Name: provisional.
- `not-fuzzed` Name: provisional.
- `why-cargo-fuzz` Name: provisional. Named after the main page's own heading.
- `three-bugs` Name: provisional. Named after the main page's own heading.
- `ci-budget` Name: provisional.
- `elf-seeds` Name: provisional.
- `targets` Name: provisional.
- `the-syscall-driver` Name: provisional, minted 2026-10-24 (UTC) by comments/929-system_tests-06.
  calef names files.
Each stem here is named for its content, and calef names files.
