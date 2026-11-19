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
recorded in `src/lib.rs` and ruled below; this block does not reopen it. Every other public name is marked provisional in the source. The baseline the review
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
  status); what remains for a release is the move's own items and the ratified names.

Verdict: a candidate to leave, on the shared ground, as a `publish = false` repository first, the
way the other extracted crates left. Publication is a separate later question, and the publishing
hold stands; nothing here lifts it. The P0 items are what the move needs. The P1 items are what a
first tag needs. The decisions section holds the three one-way doors a release would close.

## Finding status

One line per finding, in the review's order: the P0 items in
[the before-the-move appendix](1088-elf-outside-review/before-the-move.md) (items 1 to 4), the P1
items in [the before-the-tag appendix](1088-elf-outside-review/before-the-tag.md) (items 5 to 11),
the P2 items in [the later-items appendix](1088-elf-outside-review/later-items.md) (items 12 to 16),
then the decisions below and the release steps.

- Item 1 (P0), shipped text cites this tree: Done,
  [#3642](https://github.com/nifeos/nife/pull/3642). The provisional `Name:` blocks that remain go
  in the move commit.
- Item 2 (P0), fuzz seeds and target: Open. The seeds live in `tests/fixtures/`; the `elf_parse`
  target and its `fuzz/Cargo.toml` entry are still in this repository's fuzz workspace and move to
  the new repository's `fuzz/`, opted in since the crate has no `unsafe`.
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
  whether the forge is dropped.
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
- Item 15 (P2), four mutation timeouts: Open. They are triaged from the first weekly mutation run
  in the new repository.
- Item 16 (P2), the note padding rule differs from LLVM: Open. The `padding` doc and the README's
  `BUGS` state the difference from the review's recollection; LLVM's note iterator source is still
  to be read and the rule stated from it.
- D1, the machine as an input: Ruled (option A, maintainer, provisional, 2026-11-14 UTC). Not
  built: `NATIVE_MACHINE` stays public against the ruling's list, for the reason the decision
  records; the maintainer either removes it before the tag or records the deviation as the rule.
- D2, the policy stays inside the parser: Done, [#3642](https://github.com/nifeos/nife/pull/3642).
  All five refusals stay, the README lists them as policy, and the page size is an input.
- D3, the public names: Done, [#3642](https://github.com/nifeos/nife/pull/3642).
- The crate name: Done, [#3595](https://github.com/nifeos/nife/pull/3595). Ruled 2026-11-14 UTC.
- Review package: Open. `notes/crate-reviews/` has no `static-elf.md`; it is the first step before
  the move, and it carries the name draft.
- Review clearance and name ratification: Open. Under Open decisions.

## Open decisions

### Clear the review of `static_elf` and create its repository

Background. `static_elf` is the front half of this tree's ELF loader: it validates a static ELF64
executable and finds a note in it. The kernel, the UEFI loader and nine other packages link it, and
the kernel cannot leave this repository until every crate it links has left, so `static_elf` must
move to a repository of its own, `nifeos/static_elf`. A crate leaves after its review package
(`notes/crate-reviews/static-elf.md`) lists every public name and decision, and the architect
clears that review. Clearing it is also the go to create the repository. Today every public name but
the crate's own is provisional: `Elf`, `Segment::virtual_address`, `PageBytes`,
`Elf::parse_for_machine`, the three machine constants, `NATIVE_MACHINE` and the rest. A name is
cheap to change while only this tree spells it, and a one-way door once a release does.

Recommendation: A, once the review package exists.

- A. Clear the review when its package is written: ratify the names as the package lists them, and
  create `nifeos/static_elf` as a `publish = false` repository. The move, the stranger run and the
  first weekly mutation run follow.
- B. Create the repository but leave the names provisional until the release review. The move
  happens sooner; every name is decided once, at the tag, with the release review that reopens
  older names anyway.
- C. Hold the crate in this repository. The kernel's move (milestone 1039) waits with it.

Door: one-way. A repository is an outward step other people can see and link, and a ratified name
binds every dependent once anything outside reads it.

Prior art: the Rust project's libs-team API review clears a public surface before it stabilizes, and
renames freely before that. The crates already cleared here (`e1000e`, `extensible_host_controller_interface`,
`generational_table`) took the same path: package, clearance, repository, then tags with no publish.
`static_elf` differs in sitting on the kernel's critical path, so its clearance gates milestone
1039. Concrete example: ratifying `Segment::virtual_address` fixes the spelling the kernel's loader
uses at `kernel/src/user/loader.rs`, and a later rename would sweep eleven packages in two
repositories.

Blocks: the move.

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

Three doors a release would close. Each is reversible today and stays so until a tag or a publish.
All three are decided, provisionally.

### D1. Which machine the parser accepts: a constant of the build, or an input

Background. An ELF header names the machine its code is for. The crate compares it with
`EXPECTED_MACHINE`, a constant chosen by `cfg(target_arch)` at compile time, so the
kernel accepts only its own architecture without a caller naming it, and the host test build accepts the
host's. That was the right economy inside one tree: no caller ever wanted another answer, and the
design note says threading a parameter through the kernel and the progenitor would pass the same
value everywhere. In a crate others link, it means a tool built for one machine cannot validate a
program for another. The only way to test the refusal is a derived list of "the other machines this
organization runs", which is what `KNOWN_MACHINES`, `FOREIGN_MACHINES`, `NATIVE_MACHINE` and
`machine_no_nife_build_accepts` are. Every other reader in the table above hands the caller
`e_machine` and lets the caller judge. Linux fixes it per architecture, as this crate does, because
a kernel is the one program that never wants another answer.

The premise that no caller wants another machine is already false once: `jig` lists manifests only
for the host's architecture because `Elf::parse` would refuse the rest, and the note readers skip the
machine check for the same reason.

Recommendation: A.

- A. `Elf::parse(bytes)` keeps today's meaning (the build's own machine) and a second constructor
  takes the machine as a `u16` with the three `EM_` constants made public (names provisional:
  `Elf::parse_for_machine`, `MACHINE_AARCH64`, `MACHINE_RISCV`, `MACHINE_X86_64`, each with
  `#[doc(alias = "EM_AARCH64")]` and so on). `KNOWN_MACHINES`, `FOREIGN_MACHINES` and
  `machine_no_nife_build_accepts` leave the public surface; `system_tests`' forgeries take the
  machine from the new constants. The kernel changes nothing. Cost: one function and three
  constants; the test module shrinks. Reversibility: adding a constructor later is compatible, so
  this could wait. Removing the four public items later is not, which is why it is now.
- B. The machine becomes a required argument of `parse`. Every caller changes (36 call sites), for
  a value all of them spell the same way. Cleaner surface, more sweep, no gain in behavior.
- C. Leave it. Publishable only to users whose host is their target, and the four items keep an
  organization's target list in a public API.

Prior art: Linux `elf_check_arch` (a per-architecture constant, option C's shape, in the one program
that can afford it). cole14 `elf`, `object` and `goblin` (option B's shape: the caller judges
`e_machine`). Concrete example: a basalt image step on an x86_64 build host validating every member
of an aarch64 package before it ships, which option A allows with one call and options C refuses.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first tag,
which the architect may revisit. Option A: `Elf::parse` keeps the build's machine, a
second constructor takes the machine, and three public machine constants carry the `EM_` spelling
as `#[doc(alias)]`. The constructor's and the constants' names stay provisional. `KNOWN_MACHINES`,
`FOREIGN_MACHINES`, `NATIVE_MACHINE` and `machine_no_nife_build_accepts` leave the public surface,
which is the part a release would close. The before-the-move item on the machine surface is the
work.

As built, `NATIVE_MACHINE` stays public, the one part of the ruling's list the lane did not apply.
It names no organization's targets. It states the machine `Elf::parse` accepts in this build, the
same fact as the `cfg` arms behind it. Nine files outside the crate (14 uses, in `system_tests`,
`fixtures`, `measured_boot`, `system_initializer`, `program_asks` and `uefi_loader`) forge a header
to test what follows the machine check. Without the constant, each would copy the three-arm
`cfg(target_arch)` table. `KNOWN_MACHINES`, `FOREIGN_MACHINES` and the guard (now
`unknown_machine`) are test-only. The architect may still remove `NATIVE_MACHINE` before the tag.

### D2. Whether the loader's policy stays inside the parser

Background. Five of the eighteen refusals are not the ELF format's rules but this loader's. They are
a segment both writable and executable, two segments on one 4 KiB page, an entry outside an
executable segment, more than 64 program headers, and anything but a static `ET_EXEC`. OpenBSD's
kernel refuses the first at exec time. No reader in the table refuses the second, which exists
because this tree's linker script page-aligns every segment. The crate's own comparison appendix
separated "the parse" from "the policy, about 200 lines, ours whichever parser reads the bytes". A
published crate that bakes the policy in is a loader front half for callers who share it. One that
exposes knobs is a general reader with a strict default. Adding a knob later is compatible; removing
a refusal later breaks a caller who relied on it. So the strict default is the one that can wait,
and the restrictive choice is the safe one.

Recommendation: A.

- A. Keep every refusal, with no knobs, and make the page size of the overlap check an input
  (item 6) because the kernel's own frame size will change and that is a correctness fix, not a
  policy change. The README names the five as policy, in one table, so a reader who wants a general
  reader knows to use `object`. Cost: nothing beyond item 6.
- B. A `Rules` value with the five as fields and a strict default. Every refusal becomes
  optional, the tests double, and the proofs' claims gain a precondition. Cost: M, and a wider
  surface to keep compatible.
- C. Split the crate: a reading half and a policy half. Two names to ratify, two repositories or
  one with two packages, for 490 lines of code. The appendix measured the reading half at about 280
  lines.

Prior art: OpenBSD `exec_elf.c` (W^X refused by the kernel, with a per-file opt-out note). Linux
`binfmt_elf.c` (refuses almost nothing that is well-formed; policy lives in the architecture and in
`PT_GNU_STACK`). Concrete example: a bootloader for another system links this crate to place its
kernel. Under A it must link its kernel W^X-clean, as this tree's x86_64 kernel had to be (the RWX
trampoline segment was a real refusal, fixed in the linker script), and under B it would flip a
field.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first tag,
which the architect may revisit. Option A: all five refusals stay, with no knobs, and
the overlap check's page size becomes an input. The README lists the five as this loader's policy.
The five are `WritableAndExecutable`, `SegmentsOverlap`, `EntryNotExecutable`,
`TooManyProgramHeaders` and the refusal of anything but a static `ET_EXEC` (`src/error.rs` lines
60 to 83). The before-the-tag item on the overlap check is the work.

### D3. The public names before anyone outside spells them

Background. The naming rules say an address spells `virtual_address` or `physical_address`
everywhere, a length carries its unit, a specification's abbreviation stays only with its expansion and an alias, and a parameter follows the same rules as an item. The crate's public surface predates those rules and uses the ELF specification's own field names, and the source marks every name but the crate's provisional. After a release each is a one-way door. Today
they cost one rename lane across eleven dependents (`vaddr` 7 uses, `paddr` 5, `memsz` 3, the flag
constants 48, `page_range` 8). The crate name `static_elf` is ruled and is not in question.

Recommendation: rule the table as one decision, A, and let the lane sweep.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first tag,
which the architect may revisit. Option A: the table below, with the ELF
specification's spelling as `#[doc(alias)]` and no alias of this tree's earlier spelling. The
before-the-tag item on the public vocabulary is the work.

| today | recommended | alias | why |
|---|---|---|---|
| `Segment::vaddr` | `virtual_address` | `p_vaddr` | the `va` rule, every location |
| `Segment::paddr` | `physical_address` | `p_paddr` | the `pa` rule |
| `Segment::memsz` | `memory_bytes` | `p_memsz` | a length carries its unit |
| `Segment::flags` | stays | `p_flags` | a flag is a bit, and these are bits |
| `Segment::data` | stays | `p_filesz` for its length | std's word |
| `PF_R`, `PF_W`, `PF_X` | `READABLE`, `WRITABLE`, `EXECUTABLE` | `PF_R` and so on | spelled-out constants, spec name as alias |
| `NoteSegment::p_align` | `alignment` | `p_align` | spelled out |
| `NoteSegment::size` | `bytes` | `p_filesz` | unit |
| `NoteSegment::offset` | stays | `p_offset` | plain word |
| `page_range(page_size)` | `page_range(page_bytes)` or item 5's type | | unit |
| `Elf::entry` | stays | `e_entry` | plain word |
| `NoteSearch::new(owner, kind)` | stays | `n_type` for `kind` | plain words |

- A. The table as written. Cost: M, one lane.
- B. Keep the specification's field names as the identifiers, with the expansion in each doc
  comment, under the clause that a term of art a reader searches for may keep its spelling. Cost:
  nothing. It reads the rule's exception as the rule. `vaddr` is not what a reader searches for,
  `p_vaddr` is, and that is what the alias is for.

Prior art: `object` names them `p_vaddr`, `p_paddr`, `p_memsz` (the specification's spelling,
option B); cole14 `elf` the same; Linux's `struct elf64_phdr` the same. All three are readers whose
users come from the specification. This crate's users come from a loader, and the organization's
rule already decided which way that goes. Concrete example: `kernel/src/user/loader.rs` line 234
reads `let file_hi = seg.vaddr + seg.data.len() as u64;`, which under A reads
`seg.virtual_address + seg.data.len() as u64` and says what it adds.

### The crate name

Ruled 2026-11-14 (UTC), option A: the crate is `static_elf`, in code and in package name, with no
`package =` rename and no `doc(alias)` of the old spelling. The crates.io name `elf` belongs to a
different, widely used parser (14.1 million downloads), and `static_elf` was free on 2026-11-14. The
crate stays the front half of an ELF loader for static executables; the rename changes no behavior.
The type `Elf` and every other public name keep their spelling. The repository, when the crate
leaves, is `nifeos/static_elf`. The notes about the file format keep their names (`notes/elf.md`,
`notes/elf/`).

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

## Index row

An outside reading of `static_elf` (then named `elf`) before it leaves. The crate leaves on the
shared ground: the kernel links it and must follow it out. Of 22 findings and steps, 14 are done,
one is ruled and not built (`NATIVE_MACHINE` still public against D1), and six are open: the
move's fuzz target and scaffold, the stranger run, the four mutation timeouts, the LLVM padding
check and the review package. One decision is open: the review's clearance, which creates
`nifeos/static_elf` and ratifies its names.
