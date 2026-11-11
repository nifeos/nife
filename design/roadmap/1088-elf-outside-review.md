---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: 857, 1039
decision_dependencies: 46, 151, 154
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1088. `elf`: what an outside maintainer would fix before it leaves this repository

Raised by a read-only review lane. A reviewer with no prior exposure to this crate read
`crates/elf` at the head of `main` (`cf1328af6`) in a fresh clone, the way a stranger meeting it in
its own repository would. The review changed nothing; every item below is work for a later lane.

**The crate has no review package.** `notes/crate-reviews/` holds no `elf.md` and
[the index](../../notes/crate-reviews.md) has no row for it, so there is no settled baseline of
names and decisions to leave closed. The one ratified name is the crate's own, `elf`, kept under
§154 (the acronym test is whether the phrase is spoken) and recorded in `src/lib.rs`; this block does
not reopen it. Every other public name is marked provisional in the source. The baseline the review
did take is [milestone 784 (reuse what the tree wrote: which first-party code an existing library should replace)'s comparison of the crate against three readers](0784-reuse-what-the-tree-wrote/elf-against-three-readers.md),
which measured fit, hostile input, proofs, cost and upkeep and ended in the ruling that the crate
stays ours, and the order in which milestone 857 (crates released on their own) moves it.

Reuse: the template `nifeos/crate-workflows` and its scaffold, `script/crate-review-packet`,
`script/crate-standalone`, Kani, cargo-mutants, cargo-fuzz, the crate's eight falsification patches,
and milestone 784's probe script. No new tool.

## Does it leave?

The rule: a crate leaves this repository only if it is publishable or used by more than one
repository. Judged against both halves:

- **Shared: yes, and on the kernel's critical path.** Eleven packages link it today
  (`kernel`, `uefi_loader`, `components`, `jig`, `fixtures`, `system_tests`, `measured_boot`,
  `system_initializer`, `program_asks`, `process_builder`, `fuzz`), all inside this repository. But
  [milestone 1039 (the kernel leaves nife)](1039-the-kernel-leaves-nife.md) requires every crate the
  kernel links to have left first, and the kernel's loader (`kernel/src/user/loader.rs`) is the main
  user. The progenitor, the shell, `jig` and the UEFI loader are not kernel code and link it too. So
  once the kernel moves, at least two repositories link `elf`, and the kernel cannot move until `elf`
  has. The order the split already ruled meets the shared test.
- **Publishable: not as it stands.** The crates.io name `elf` belongs to another crate (0.8.0, 14.1
  million downloads, 3.98 million in the last 90 days, measured from the crates.io API during this
  review). The machine the parser accepts is a compile-time constant of the host. The refusals that
  are this loader's policy (W^X, one segment per page at 4 KiB, 64 headers, `ET_EXEC` only) are
  hard-wired with no way to state another page size. Shipped doc comments cite this tree's paths,
  milestones, sections and the architect by name twenty-nine times. There is no README, no license
  file and no manifest metadata. Every one of these is fixable; the first is a one-way door.

Verdict: a candidate to leave, on the shared ground, as a `publish = false` repository first, the
way the other extracted crates left. Publication is a separate later question, and the publishing
hold stands; nothing here lifts it. The P0 items are what the move needs. The P1 items are what a
first tag needs. The decisions section holds the three one-way doors a release would close.

## Goal

A stranger who finds the repository can tell in one screen that this is a validating front half of a
loader for static ELF64 executables, not a general ELF reader. The same stranger can see which
refusals are the ELF format's and which are this loader's policy. They can build it, run its tests,
verify its eight proofs and replay one falsification from the README alone. The crate reads nothing
outside its own repository, names no project but itself, and compiles for every target it claims
without a width assumption it does not state. The public vocabulary follows the organization's
naming rules before anyone outside depends on it.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1088-elf-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1088-elf-outside-review/prior-art.md).

## Findings

### P0: before the move

#### Shipped text cites this tree's paths, milestones, sections and the architect

Finding: `src/error.rs` lines 70 to 71 explains `WritableAndExecutable` by `paging::Flags` and
`notes/elf/parser.md`. `src/error.rs` lines 75 to 76 explains `SegmentsOverlap` by
`crates/user_mode_runtime/link.ld`. `src/segment.rs` lines 26 to 31 sends the reader to `kernel/src/user.rs`,
`kernel/link-x86_64.ld`, milestone 200 (A virtual address and a physical address stop being the same type) and `notes/elf.md`. `src/lib.rs` lines 7 and 19 to 25 cite two notes, §14 (the project's direction: a verified-Rust capability microkernel that runs real workloads)
and milestone 18 (verify the capability core, then spread inward). `src/machine.rs` lines 1 to 2, 6, 14 and 40 say what "nife runs". `src/machine.rs` lines 46 to 49 record a
provisional name in prose with the architect's name, outside a `Name:` block, so the move's
stripping will not remove it. `src/note.rs` lines 3 to 5 cite milestone 597 (A program carries its manifest in an ELF note), §197 (A package is one archive file, named and vouched for by its recipe) and the architect.
`src/note.rs` lines 244 and 271 give `b"nife"` as the example owner. The Kani harness at `src/note.rs` line 355
searches for `b"nife"`. `src/parse.rs` lines 259 to 261 speak of "halting the kernel". The packet counts 29
such lines across `src/`. A stranger can follow none of them, and a crate that has left must not
reference back into this repository.

Change: rewrite each in the crate's own words. The loader-policy rationale (why W^X, why one segment
per page, why 64 headers, why the machine stays fixed) becomes a "What it refuses, and why" section of
the README, phrased for any loader. The doc comments keep one sentence each and link the README
section. The example owner becomes a neutral one (`b"GNU"` is the one every reader knows). `Name:`
blocks go in the move commit, as ruled for every extracted crate. Door: two-way. Effort: M.

#### The fuzz seed test reads files outside the crate, and the fuzz target lives in this tree

Finding: `tests/fuzz_seed.rs` lines 18 to 20 `include_bytes!` three seeds from
`../../../fuzz/seeds/elf_parse/`, and `fuzz/fuzz_targets/elf_parse.rs` with its `fuzz/Cargo.toml`
entry are in this repository's fuzz workspace. The target searches for owner `b"nife"` and type 1.
Packaged on its own the test fails to compile. This is the shape milestone 1017 (the contract crates leave in this order) recorded for
`capability_manifest_note`: the seeds and the target leave with the crate.

Change: the new repository carries `fuzz/` with the target and `fuzz/seeds/elf_parse/`. The test
reads `../fuzz/seeds/` or a copy under `tests/fixtures/`. The owner and type in the target are a
constant of the target with a comment, not a project's name. This repository's `fuzz/Cargo.toml`
drops the `elf_parse` target, and `fuzz/seeds/README.md` loses its row. The template's `fuzz.yml`
runs it weekly only if the repository opts in; the crate has no `unsafe`, so it may. Door: two-way.
Effort: S.

#### The manifest and the repository files a stranger needs

Finding: `Cargo.toml` has no `description`, `repository`, `keywords`, `categories`, `readme` or
`rust-version`. The manifest inherits `license` and `lints` from the workspace (lines 6 and 8-9). The
comment on `publish = false` (line 5) sends the reader to `crates/publish-allowlist.txt`, a file the
new repository will not have. There is no README, `LICENSE-MIT`, `LICENSE-APACHE`, `CHANGELOG.md`,
`SECURITY.md` or `CONTRIBUTING.md`. `cargo package --list` warns about the metadata.

Change: write the inherited keys as literals, as `jh7110_entropy` did. Take the scaffold's files
(`README-crate.md`, `CHANGELOG.md`, `SECURITY.md`, `CONTRIBUTING.md`, `CODEOWNERS`, `_typos.toml`,
`deny.toml`, `rustfmt.toml`, the caller workflows, `docs/stranger-test.md`). State `rust-version` as
the other packages do (1.88) after a build at that version confirms it (the crate uses edition 2024,
`let else` and `div_ceil`, so 1.85 is the floor by reading; not verified). Replace the `publish =
false` comment with one sentence about the publishing hold. The README states the scope in its first
paragraph: static, little-endian ELF64 `ET_EXEC` for one machine, program headers only, with a
validating parse and a note lookup. Door: two-way. Effort: S.

#### A public function and three public constants name the project or fix the machine to the build

Finding: `machine_no_nife_build_accepts` (`src/machine.rs` line 80) carries the project's name in a
public identifier. `KNOWN_MACHINES`, `NATIVE_MACHINE` and `FOREIGN_MACHINES`
(`src/machine.rs` lines 19, 38 and 50) are a test-support surface for this tree's three architectures, exported
because `tests/fuzz_seed.rs` and `system_tests` iterate them (`KNOWN_MACHINES` 3 uses,
`FOREIGN_MACHINES` 1, `NATIVE_MACHINE` 11, all in tests and forgeries). A crate that has left names
no project, and a published crate that lists "the machines some build accepts" lists one
organization's targets.

Change: the shape is decision D1's. Whatever D1 rules, `machine_no_nife_build_accepts` leaves the
public surface before the move (it has two callers, both tests in this crate), and the three
constants either move behind `cfg(test)` and into `system_tests`' own forgery helper, or stay public
under D1's vocabulary. Door: one-way at release, two-way now. Effort: S for the function, M with the
callers if the constants move.

### P2: later

The P2 items are in [the appendix](1088-elf-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1088-elf-outside-review/before-the-tag.md).

## Decisions for the architect

Three doors a release would close. Each is reversible today and stays so until a tag or a publish.
A recording lane brings them one at a time, in this order, each with the background below and the
recommendation first.

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

### D3. The public names before anyone outside spells them

Background. The naming rules say an address spells `virtual_address` or `physical_address`
everywhere, a length carries its unit, a specification's abbreviation stays only with its expansion and an alias, and a parameter follows the same rules as an item. The crate's public surface predates those rules and uses the ELF specification's own field names, and the source marks every name but the crate's provisional. After a release each is a one-way door. Today
they cost one rename lane across eleven dependents (`vaddr` 7 uses, `paddr` 5, `memsz` 3, the flag
constants 48, `page_range` 8). The crate name `elf` has its ratification and is not in question.

Recommendation: rule the table as one decision, A, and let the lane sweep.

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

### Held, not asked now: the crates.io name

`elf` on crates.io belongs to cole14's crate (14.1 million downloads). This crate cannot publish
under its ratified name. The publishing hold makes the question moot until a combine review has
happened, and the move does not need it. The repository can be `nifeos/elf` and the package `elf`
with `publish = false`. A later rename of the package costs the dependents one manifest line each
(`elf = { package = "<new name>", ... }`) with no change to any `use elf::`. Measured free during
the review: `static_elf`, `elf_static`, `elf_executable`, `elf_program_headers`, `elf_image`,
`elf_segments`, `static_executable`, `executable_and_linkable_format`. Taken: `elf64`, `elf_loader`.
When publication comes up, the reviewer would bring `static_elf` first: it names the one kind
of file the crate accepts, in two words a reader says. That is for then.

## Done when

The checks are in [the appendix](1088-elf-outside-review/done-when.md).

## BUGS

- Items 5 and 7 come from reading. The review installed no 32-bit target and wrote no failing
  test, so each is a hole in a claim rather than a defect shown.
- The prior-art rows for Linux and OpenBSD come from recall, not a re-read. A lane that cites them in
  the README reads the source first.
- The review did not run `script/crate-standalone`, so the list of citations in item 1 is the
  packet's count (29 lines) and the lane doing the sweep will find the exact set.
- The verdict that `elf` leaves rests on milestone 1039's order (every crate the kernel links
  leaves first). If that order changes, the shared ground changes with it, and the publishable
  ground alone does not carry the move until D1, item 1 and the name all have rulings.
- The P2 order is the reviewer's guess at what a user asks for first; nobody outside has asked.

## Index row

An outside reading of `elf` before it leaves. No review package exists. The crate leaves on the
shared ground (the kernel links it and must follow it out), not on publishability, which the taken
crates.io name, a build-time machine constant and 29 citations of this tree rule out as it stands.
Four items block the move (citations, the fuzz seeds and target, the manifest and files, a public
name that says nife). Seven go before the first tag (a page-size hole in a public helper and in the
overlap check, four truncating casts, `Display`, the naming sweep, a README with an example, one
harness). Five follow it. Three decisions for the architect, one at a time: the machine as an input,
the policy staying inside, and the public names. Tests, clippy, docs, three targets, eight proofs
and eight falsifications all ran clean; the census records no survivor.
