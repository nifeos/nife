---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 857, 1039
decision_dependencies: 46, 151, 154
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1088. `static_elf`: what an outside maintainer would fix before it leaves this repository

Raised by a read-only review lane, when the crate was still named `elf`. A reviewer with no prior
exposure to this crate read `crates/static_elf` at the head of `main` (`cf1328af6`) in a fresh clone.
The reviewer read it the way a stranger meeting it in its own repository would. The review changed nothing; every item below is work for a later lane.

**The crate has no review package.** `notes/crate-reviews/` holds no `elf.md` and
[the index](../../notes/crate-reviews.md) has no row for it, so there is no settled baseline of
names and decisions to leave closed. The one ratified name is the crate's own, `static_elf`,
recorded in `src/lib.rs` and ruled below; this block does not reopen it. The architect ratified the public names on 2026-11-21 (UTC), apart from the three `MACHINE_*` constants, and renamed `Error::SegmentTruncated` to `Error::MemorySmallerThanFile` and `Error::BadProgramHeaders` to `Error::ProgramHeadersOutOfBounds`; the source records each. The baseline the review
did take is [milestone 784 (reuse what the tree wrote: which first-party code an existing library should replace)'s comparison of the crate against three readers](0784-reuse-what-the-tree-wrote/elf-against-three-readers.md).
That comparison measured fit, hostile input, proofs, cost and upkeep. It ended in the ruling that the crate
stays ours. The baseline also takes the order in which milestone 857 (crates released on their own) moves it.

Reuse: the template `nifeos/crate-workflows` and its scaffold, `script/crate-review-packet`,
`script/crate-standalone`, Kani, cargo-mutants, cargo-fuzz, the crate's falsification patches,
and milestone 784's probe script. No new tool.

## Does it leave?

The rule: a crate leaves this repository only if it is publishable or used by more than one
repository. Judged against both halves:

- **Shared: yes, and on the kernel's critical path.** Eleven packages link it
  (`kernel`, `uefi_loader`, `components`, `jig`, `fixtures`, `system_tests`, `measured_boot`,
  `system_initializer`, `program_asks`, `process_builder`, `fuzz`), all inside this repository. But
  [milestone 1039 (the kernel leaves nife)](1039-the-kernel-leaves-nife.md) requires every crate the
  kernel links to have left first, and the kernel's loader (`kernel/src/user/loader.rs`) is the main
  user. The progenitor, the shell, `jig` and the UEFI loader are not kernel code and link it too. So
  once the kernel moves, at least two repositories link `static_elf`, and the kernel cannot move until `static_elf`
  has. The order the split already ruled meets the shared test.
- **Publishable: not yet.** The crates.io name `elf` belongs to a different crate (0.8.0, 14.1
  million downloads, 3.98 million in the last 90 days, measured from the crates.io API during this
  review), which is why the crate is named `static_elf`. The review found the machine fixed to the
  build, the page size of the overlap check fixed at 4 KiB, 29 citations of this tree in shipped
  text, and no README, license file or manifest metadata. All of those are now fixed (Finding
  status); what remains for a release is the move's own items.

Verdict: a candidate to leave, on the shared ground, as a `publish = false` repository first, the
way the other extracted crates left. Publication is a separate later question, and the publishing
hold stands; nothing here lifts it. The P0 items are what the move needs. The P1 items are what a
first tag needs. The decisions section holds the three one-way doors a release would close.

## Finding status

One line per finding, in the review's order. The P0 items are in [the before-the-move
appendix](1088-elf-outside-review/before-the-move.md) (items 1 to 4). The P1 items are in [the
before-the-tag appendix](1088-elf-outside-review/before-the-tag.md) (items 5 to 11), and the P2
items in [the later-items appendix](1088-elf-outside-review/later-items.md) (items 12 to 16). The
decisions below and the release steps follow.

- Item 1 (P0), shipped text cites this tree: Done,
  [#3642](https://github.com/nifeos/nife/pull/3642). The provisional `Name:` blocks that remain go
  in the move commit.
- Item 2 (P0), fuzz seeds and target: Open. The seeds live in `tests/fixtures/`, and the target
  cites nothing in this tree. The `elf_parse` target and its `fuzz/Cargo.toml` entry are still in
  this repository's fuzz workspace and move to the new repository's `fuzz/`, opted in since the
  crate has no `unsafe`.
- Item 3 (P0), manifest and repository files: Open. The manifest states every key as a literal with
  `rust-version` 1.88, and the license files are in the crate
  ([#3642](https://github.com/nifeos/nife/pull/3642)); the scaffold's `CHANGELOG.md`,
  `SECURITY.md`, `CONTRIBUTING.md`, `deny.toml`, `_typos.toml` and caller workflows come at the
  move.
- Item 4 (P0), a public name says nife and the machine list is public: Done,
  [#3642](https://github.com/nifeos/nife/pull/3642). The guard is the test-only `unknown_machine`;
  `NATIVE_MACHINE` is under D1.
- Item 5 (P1), `page_range` panics on a bad page size: Done,
  [#3642](https://github.com/nifeos/nife/pull/3642). `PageBytes` holds only a power of two, and a
  harness quantifies over every one.
- Item 6 (P1), the overlap check hard-codes 4 KiB: Done,
  [#3642](https://github.com/nifeos/nife/pull/3642). `Elf::parse_with` takes the loader's page
  size, and the kernel passes its frame size.
- Item 7 (P1), four truncating casts: Done, [#3565](https://github.com/nifeos/nife/pull/3565).
- Item 8 (P1), `Display`, `core::error::Error` and derives: Done,
  [#3565](https://github.com/nifeos/nife/pull/3565). The consumers keep their own wording by
  design: the UEFI loader's `refusal` table words each refusal for a machine that will not boot,
  and the kernel's `LoadError` display prints a parser refusal by its `Debug` form.
- Item 9 (P1), the public vocabulary: Done, [#3642](https://github.com/nifeos/nife/pull/3642). The
  names D3 ruled, across the crate and its dependents, each with the specification's alias.
- Item 10 (P1), README, example, shared forge and stranger run: Open. The README and
  `examples/parse_static_executable.rs` are in
  [#3642](https://github.com/nifeos/nife/pull/3642); the stranger run in `docs/stranger-test.md`
  comes at the move, and the shared test forge is not built because no finding stands behind it
  (the unit tests, doc examples and dependents each forge differently). The maintainer rules
  whether the forge goes.
- Item 11 (P1), the program-header index has no proof: Done,
  [#3642](https://github.com/nifeos/nife/pull/3642). Nine harnesses, each with a falsification
  patch.
- Item 12 (P2), an entry in the zero-filled tail: Done,
  [#3642](https://github.com/nifeos/nife/pull/3642). Refused as `EntryNotExecutable`, a maintainer
  ruling, provisional (2026-11-16 UTC).
- Item 13 (P2), what the parser ignores is unwritten: Done,
  [#3642](https://github.com/nifeos/nife/pull/3642). The README's "What it does not check",
  congruence included.
- Item 14 (P2), `segments()` discards an unreachable error: Done,
  [#3565](https://github.com/nifeos/nife/pull/3565). A comment states the invariant, and
  `exactly_the_maximum_header_count_is_accepted` counts.
- Item 15 (P2), four mutation timeouts: Open. The first weekly mutation run in the new repository
  triages them.
- Item 16 (P2), the note padding rule differs from LLVM: Open. The `padding` doc and the README's
  `BUGS` state the difference from the review's recollection; nobody has yet read LLVM's note
  iterator source and stated the rule from it.
- D1, the machine as an input: Ruled (option A, maintainer, provisional, 2026-11-14 UTC). Not
  built: `NATIVE_MACHINE` stays public against the ruling's list, for the reason the decision
  records; the maintainer either removes it before the tag or records the deviation as the rule.
- D2, the policy stays inside the parser: Done, [#3642](https://github.com/nifeos/nife/pull/3642).
  All five refusals stay, the README lists them as policy, and the page size is an input.
- D3, the public names: Done, [#3642](https://github.com/nifeos/nife/pull/3642).
- The crate name: Done, [#3595](https://github.com/nifeos/nife/pull/3595). Ruled 2026-11-14 UTC.
- Review package: Done. [`notes/crate-reviews/static-elf.md`](../../notes/crate-reviews/static-elf.md)
  lists the identity, the dependents, the checks and every public name, all provisional.
- Review clearance: Done. Ruled 2026-11-20 (UTC), option A: the architect cleared the review package
  and gave the go for `nifeos/static_elf`. Name ratification: Open. The names stay provisional.

## Open decisions

None. The architect ruled the clearance; Decisions for the architect below records it. The ratification of the
package's provisional names, one at a time, waits for the architect.

## Goal

A stranger who finds the repository can tell in one screen that this is a validating front half of a
loader for static ELF64 executables, not a general ELF reader. The same stranger can see which
refusals are the ELF format's and which are this loader's policy. They can build it, run its tests,
verify its proofs and replay one falsification from the README alone. The crate reads nothing
outside its own repository, names no project but itself, and compiles for every target it claims
without a width assumption it does not state. The public vocabulary follows the organization's
naming rules before anyone outside depends on it.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1088-elf-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1088-elf-outside-review/prior-art.md).

## Findings

What each finding became is under Finding status above.

### P0: before the move

The P0 items are in [the appendix](1088-elf-outside-review/before-the-move.md).

### P2: later

The P2 items are in [the appendix](1088-elf-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1088-elf-outside-review/before-the-tag.md).

## Decisions for the architect

Each decision's background, options and prior art are in
[the appendix](1088-elf-outside-review/decisions.md). The rules in force:

- The clearance of the review of `static_elf` and its repository. Ruled 2026-11-20 (UTC), option
  A: the architect cleared the review package (`notes/crate-reviews/static-elf.md`) and gave the go
  for `nifeos/static_elf` as a `publish = false` repository, with no tag and no publish. The
  clearance does not ratify the package's provisional public names; the architect ratifies them
  later, one at a time, and each stays provisional until then. The move lane runs the stranger test, the template's
  workflows and the first weekly mutation run in the new repository. The clearance gates
  milestone 1039.
- D1, which machine the parser accepts. Decided by the maintainer on 2026-11-14 (UTC),
  provisional, a two-way door before the first tag: option A. `Elf::parse` keeps the build's
  machine, a second constructor takes the machine, and three public machine constants carry the
  `EM_` spelling as `#[doc(alias)]`. `KNOWN_MACHINES`, `FOREIGN_MACHINES`, `NATIVE_MACHINE` and the
  machine guard leave the public surface. As built, `NATIVE_MACHINE` stays public: nine files
  outside the crate forge a header with it, and the architect may still remove it before the tag.
- D2, whether the loader's policy stays inside the parser. Decided by the maintainer on 2026-11-14
  (UTC), provisional, a two-way door before the first tag: option A. All five refusals stay, with
  no knobs, the overlap check's page size is an input, and the README lists the five as this
  loader's policy.
- D3, the public names. Decided by the maintainer on 2026-11-14 (UTC), provisional, a two-way door
  before the first tag: option A, the appendix's table, with the ELF specification's spelling as
  `#[doc(alias)]` and no alias of this tree's earlier spelling.
- The crate name. Ruled 2026-11-14 (UTC), option A: the crate is `static_elf`, in code and in
  package name, with no `package =` rename and no `doc(alias)` of the old spelling. The repository,
  when the crate leaves, is `nifeos/static_elf`. The type `Elf` and every other public name keep
  their spelling, and the notes about the file format keep theirs.
- `static_elf` stays this project's own code, with no switch to the `elf` or `object` crates.
  Ruled 2026-11-19 (UTC), option A, on the measurements in
  [`notes/crate-reviews/static-elf-vs-dependencies.md`](../../notes/crate-reviews/static-elf-vs-dependencies.md).
  The ruling is open to revisit on either of two triggers: `elf` publishes a release with the
  fixes and accepts our harnesses, or `object` offers a build that needs no allocator. Either would
  come back as a new ruling under rule 6.
- The crate keeps `static_elf`, not `static_elf64`. Ruled 2026-11-19 (UTC): no 32-bit target exists,
  and the crate refuses ELFCLASS32 the way it refuses any other unsupported file. A future 32-bit
  crate would take the name `static_elf32`.

## Done when

The checks are in [the appendix](1088-elf-outside-review/done-when.md).

## BUGS

- Items 5 and 7 come from reading. The review installed no 32-bit target and wrote no failing
  test, so each is a hole in a claim rather than a defect shown.
- The prior-art rows for Linux and OpenBSD come from recall, not a re-read. A lane that cites them in
  the README reads the source first.
- The review did not run `script/crate-standalone`, so the list of citations in item 1 is the
  packet's count (29 lines) and the lane doing the sweep will find the exact set.
- The verdict that `static_elf` leaves rests on milestone 1039's order (every crate the kernel links
  leaves first). If that order changes, the shared ground changes with it, and the publishable
  ground alone does not carry the move until D1, item 1 and the name all have rulings.
- The P2 order is the reviewer's guess at what a user asks for first; nobody outside has asked.

## Follow-on

- **Outstanding.** Every finding that Finding status above marks Open or Ruled, each with what
  remains, and each open decision under Open decisions. A lane checked each finding line against
  the crate and its pull requests on main.

## Index row

An outside reading of `static_elf` (then named `elf`) before it leaves. The crate leaves on the
shared ground: the kernel links it and must follow it out. Of 22 findings and steps, 15 are done.
D1 has a ruling the crate does not yet follow, since `NATIVE_MACHINE` stays public. Five are open:
the move's fuzz target and scaffold, the stranger run, the four mutation timeouts and the LLVM
padding check. The architect cleared the review (2026-11-20 UTC) and gave the go for the repository; the
public names stay provisional until the architect ratifies them.
