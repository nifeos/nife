# Review package: `static_elf`

*Name: provisional, minted by lane milestone/1088-static-elf-review-package. The `static_elf` package of
[crate-reviews.md](../crate-reviews.md), in its own file because the packages together outgrow the
page.*

The architect reads this to clear the crate for its move to its own repository. The kernel cannot
leave nife until every crate it links has left, so this crate is on the critical path of
[milestone 1039 (the kernel leaves nife)](../../design/roadmap/1039-the-kernel-leaves-nife.md).
The review that raised the work is
[milestone 1088 (what an outside maintainer would fix before it leaves)](../../design/roadmap/1088-elf-outside-review.md).
The comparison with the `elf` and `object` crates is in
[static-elf-vs-dependencies.md](static-elf-vs-dependencies.md); the crate stays our own code
(ruled 2026-11-19 UTC).

## In one paragraph

The front half of a program loader. It parses a static, little-endian, 64-bit ELF executable
(`ET_EXEC`) for one machine and checks what a loader needs before it maps a page: the header, the
program-header table, each segment's bounds, the entry point, the permissions and the overlap of
the segments. It hands back the segments to map and finds one note in a `PT_NOTE` segment, over a
whole image or one segment at a time. It reads program headers only. Five refusals of well-formed
files are this loader's policy and have no switch: a writable and executable segment, overlapping
pages, an entry outside the executable bytes, more than 64 program headers, and anything but a
static executable. The crate is `no_std`, has no dependency and no `unsafe`. Nine Kani harnesses
prove the arithmetic a hostile header can weaponize, and a fuzz target covers the rest of the
parse. Nothing in it points back into nife.

## Identity

| item | value |
|---|---|
| crate | `static_elf`, ratified 2026-11-14 (UTC); `static_elf64` refused (no 32-bit target exists), `elf` taken on crates.io by a different crate |
| identity values | none: the ELF specification owns the bytes. The magic is private to the parser |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, both texts at the crate root, written in the manifest with `description`, `repository`, `readme`, `keywords` and `categories` |
| `publish` | `false`; the publishing hold stands |
| dependencies | none |
| size | `src` 2,263 lines with tests (shipping code in `note.rs` 727, `parse.rs` 341, `segment.rs` 137, `machine.rs` 104, `error.rs` 118, `lib.rs` 44; tests in `src/tests.rs` 620; harnesses in `src/verification.rs` 84), `tests/fuzz_seed.rs` 88, three seed files under `tests/fixtures/`, one example |

## Who in nife depends on it

Eleven packages link it, each by a path dependency: `kernel` (its loader), `uefi_loader`,
`components`, `jig`, `fixtures`, `measured_boot`, `system_initializer`, `program_asks`,
`process_builder` and `fuzz`, plus `system_tests` as a development dependency. The kernel's loader
is the main user. `jig` and the UEFI loader are not kernel code and link it too, so once the kernel
moves, at least two repositories link the crate.

Other places name it without a dependency line: `packages/contracts.package.toml` lists it,
`helpers/trust_boundary_crates.tsv` classes it as shared, `helpers/template_report_baseline.txt`
holds four rows for it (abbreviations 102, prose acronyms 8, README acronyms 1, register names 9), and `packages/homes.toml` holds a note about it that leaves with it.
About 36 further files outside `design/`, `notes/` and `briefs/` name `crates/static_elf` in a path
or a comment; the dry run lists them, and the move lane updates each.

What the crate references back in nife: nothing. `script/crate-move` reports "mentions nife: none"
on the staged copy.

## What was run

Each check ran on this branch on 2026-11-19 (UTC).

| check | result |
|---|---|
| `cargo test -p static_elf` | 54 unit tests, 2 seed tests and 3 doctests pass |
| `script/crate-standalone static_elf` | Build passes on the host and three bare-metal targets on Rust 1.88. 9 of 9 Kani harnesses verify, and each of the 9 fails under its falsification patch. The one failure is the names check, which reports 44 provisional names that calef has not ratified. |
| `script/crate-move static_elf --dry-run --into <scratch>` | stages at template `v8` (`a69ab4829b02`). It copies 26 files, adds 27 scaffold files and strips the `Name:` blocks from 6 files. It regenerates 9 patches, and they apply with no offset and no fuzz. It stages the `elf_parse` fuzz target with 3 corpus inputs, and the target builds and runs once over its corpus. Host tests and the three targets build on 1.88. The findings that remain are under "What the move needs" |
| `script/names static_elf` and `python3 helpers/name_release.py --draft static_elf` | the crate name is ratified; every other public name is provisional, and several items carry no block yet (the move strips blocks, so the architect ratifies from the table below) |
| `script/crate-review-packet static_elf` | the public API below; no foreign type in a public signature; no `unsafe` site; no dependency; 9 harnesses paired with 9 falsification patches |

Not run, and so not claimed:

- Miri. The crate has no `unsafe` to check.
- Mutation. No baseline row exists for the crate. The milestone records four timeouts from the
  review's census, and the first weekly run in the new repository triages them.
- The stranger test, the template's workflows, a fuzz campaign beyond one pass over the corpus,
  `cargo-semver-checks` (a first release has no baseline) and `cargo package` (the manifest
  names its repository before the repository exists).

## Public API

| kind | items |
|---|---|
| parser | `Elf::{parse, parse_for_machine, parse_with, entry, segments, note}` |
| a segment | `Segment` with fields `virtual_address`, `physical_address`, `memory_bytes`, `flags`, `data`; methods `is_readable`, `is_writable`, `is_executable`, `page_range` |
| page size | `PageBytes::{new, get, FOUR_KIBIBYTES}`, which holds only a power of two |
| flags | `READABLE`, `WRITABLE`, `EXECUTABLE` |
| machines | `MACHINE_AARCH64`, `MACHINE_RISCV`, `MACHINE_X86_64`, `NATIVE_MACHINE` |
| errors | `Error` (seventeen variants, from `TooSmall` to `TooManyProgramHeaders`), `NoteError` (five variants); both implement `Display` and `core::error::Error` |
| notes | `NoteSegments::from_head`, `NoteSegment` with fields `offset`, `bytes`, `alignment` and method `range`, `NoteSearch::{new, segment}` |

No nife type, capability or handoff appears in a public signature.

What a stranger may find surprising:

- `Elf::parse` accepts the build's own machine only. `parse_for_machine` names another, and
  `parse_with` also names the page size the overlap check rounds to. `NATIVE_MACHINE` stays public
  because nine files outside the crate forge a header with it.
- The five policy refusals have no setting. The README lists them with their reasons and lists what
  the parse does not check (`e_flags`, `p_align`, section headers and more).
- A tail entry in zero-filled memory is refused as `EntryNotExecutable`.
- The note reader reads a `p_align` of 0 or 1 as 4-byte padding, which differs from LLVM for the
  value 1. The README's BUGS section says so; nobody has yet read LLVM's source to state the rule.

## Proofs

Nine Kani harnesses, each with a falsification patch in `falsifications/`, which
`script/crate-standalone` applies to see its harness fail. Five in `src/verification.rs` prove the
per-segment bounds check panic-free, in bounds and free of address overflow, a program-header
offset a whole header inside the table, and `Segment::page_range` panic-free and ordered for every
power-of-two page size. Four in `src/note.rs` prove the note reader panic-free and in bounds.
Totality of `Elf::parse` over every byte string is past bounded model checking, so the
`elf_parse` fuzz target and the tests cover the shell by example. No Miri run and no mutation run
backs these claims today (see above).

## Names

Every name below is provisional. The architect ratifies them later, one at a time; this package
ratifies none. The ELF specification's spelling is a `#[doc(alias)]` on each item that has one.

| item | kind | spelling and alias |
|---|---|---|
| `static_elf` | crate | ratified 2026-11-14 (UTC) |
| `Elf` | type | the format's name, which the acronym test keeps |
| `Elf::{parse, parse_for_machine, parse_with, entry, segments, note}` | methods | `parse_for_machine` and `parse_with` are provisional |
| `Segment` and its fields `virtual_address`, `physical_address`, `memory_bytes`, `flags`, `data` | type, fields | aliases `p_vaddr`, `p_paddr`, `p_memsz`, `p_flags`, `p_filesz` |
| `Segment::{is_readable, is_writable, is_executable, page_range}` | methods | spelled out |
| `READABLE`, `WRITABLE`, `EXECUTABLE` | constants | aliases `PF_R`, `PF_W`, `PF_X` |
| `PageBytes`, `PageBytes::{new, get, FOUR_KIBIBYTES}` | type, constructor, accessor, constant | unit in the name |
| `MACHINE_AARCH64`, `MACHINE_RISCV`, `MACHINE_X86_64` | constants | aliases `EM_AARCH64`, `EM_RISCV`, `EM_X86_64` |
| `NATIVE_MACHINE` | constant | the machine `Elf::parse` accepts in this build; open to removal before a tag |
| `Error` and its seventeen variants | enum | `TooSmall`, `BadMagic`, `Not64Bit`, `NotLittleEndian`, `BadVersion`, `WrongMachine`, `NeedsRelocation`, `NotExecutable`, `BadProgramHeaders`, `SegmentOutOfBounds`, `SegmentTruncated`, `WritableAndExecutable`, `SegmentUnreachable`, `SegmentsOverlap`, `EntryNotExecutable`, `AddressOverflow`, `TooManyProgramHeaders` |
| `NoteError` and its five variants | enum | `NotReadable`, `SegmentOutOfBounds`, `BadAlignment`, `Truncated`, `Duplicate` |
| `NoteSegments`, `NoteSegments::from_head`, `NoteSegment` with fields `offset`, `bytes`, `alignment` and method `range`, `NoteSearch`, `NoteSearch::{new, segment}` | types, methods, fields | aliases `p_offset`, `p_filesz`, `p_align` on the three fields |

The abbreviations `ELF` and `PT_NOTE` stay as the format's own terms. `Name:` blocks in the crate
source stay as built, and `script/crate-move` strips them from the moved copy.

## License

`license = "MIT OR Apache-2.0"`. The crate adapts no source: it implements the System V ELF
specification. The three seed files under `tests/fixtures/` are minimal executables written for
the crate's tests; no source file of another project is among them. The README names the other loaders a reader may weigh
against it; the comparison note holds the measurements.

## What the move needs

The move lane runs `script/crate-move static_elf` after the clearance. The dry run reports these
findings:

| item | owner |
|---|---|
| The template's `fuzz opt-in` step looks for a `fuzz: false` line in the staged weekly caller, and template `v8` has none, so the dry run reports a FAIL. The helper and the template disagree; one changes before the move passes. The crate has no `unsafe`, so the opt-in is allowed. | move lane, with the template |
| Carry the four mutation timeouts into the first weekly run's triage, and add the crate's exemptions to `.github/mutants-exempts` with reasons. | move lane |
| The baseline rows (abbreviations 102, prose acronyms 8, README acronyms 1, register names 9) have no counterpart in the new repository; write `.github/accepted-words` entries or fix the names the architect rules on. | move lane |
| Nife side: drop the workspace member; convert the eleven path dependencies to `git` with `rev` and `version = "0.1.0"`; delete the fuzz dependency, the `elf_parse` target and its rows in `script/fuzz`, `fuzz/seeds/README.md` and `notes/fuzzing/targets.md`; refresh `fuzz/Cargo.lock`; update the files the dry run lists. | move lane, in a nife pull request |
| Run the stranger test (`docs/stranger-test.md`) and the first weekly mutation run in the new repository. | move lane |

## Open questions

The review's clearance, which is the architect's: clear the review, ratify the names one at a time
and create `nifeos/static_elf` as a `publish = false` repository. Milestone 1088's open decisions
section holds the options.

## Recommendation

Ready for review.
