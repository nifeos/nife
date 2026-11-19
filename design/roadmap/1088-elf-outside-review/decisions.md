# Decisions for the architect: background, options and prior art

An appendix to [the milestone](../1088-elf-outside-review.md), which states each rule in force.

Three doors a release would close. Each is reversible today and stays so until a tag or a publish.
All three are decided, provisionally.

## D1. Which machine the parser accepts: a constant of the build, or an input

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

## D2. Whether the loader's policy stays inside the parser

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

## D3. The public names before anyone outside spells them

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

## The crate name

Ruled 2026-11-14 (UTC), option A: the crate is `static_elf`, in code and in package name, with no
`package =` rename and no `doc(alias)` of the old spelling. The crates.io name `elf` belongs to a
different, widely used parser (14.1 million downloads), and `static_elf` was free on 2026-11-14. The
crate stays the front half of an ELF loader for static executables; the rename changes no behavior.
The type `Elf` and every other public name keep their spelling. The repository, when the crate
leaves, is `nifeos/static_elf`. The notes about the file format keep their names (`notes/elf.md`,
`notes/elf/`).
