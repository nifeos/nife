# Review package: `page_tables`

*Name: provisional, minted by lane 1104-renames-package. The `page_tables` package of
[crate-reviews.md](../crate-reviews.md), in its own file because the packages together outgrow the
page.*

The architect reads this to ratify the crate's public names in one pass and to clear the crate for its
move to its own repository, the outcome of
[milestone 1104 (paging outside review)](../../design/roadmap/1104-paging-outside-review.md). It
describes the crate as `main` plus this branch leaves it.

## In one paragraph

Page tables for six hardware formats: aarch64, Sv39, the T-Head Sv39 variant, x86-64 (IA-32e), Intel
VT-d and AMD-Vi. One `Mapper` builds, edits and walks a table tree in any of them through a
`PageFormat` trait. The caller supplies a frame allocator and a function from a physical address to a
pointer, so the crate runs under a kernel, a hypervisor or a host test. It maps a page, a block of
2 MiB or 1 GiB, a span and a range, and it unmaps, translates and walks. It refuses a mapping that
is writable and executable at once, a virtual address in the wrong half and a range that overflows.
A removed mapping returns a `TlbFlush` token the caller must honor. The crate is `no_std`, has no
dependency and has `unsafe` in one place of consequence: a raw pointer to a table frame.
Thirty-eight Kani harnesses prove the encodings, and 42 falsification patches prove the harnesses
can fail. Nothing in it points back into nife. The renames below are provisional; this package asks
the architect to ratify them.

## Identity

| item | value |
|---|---|
| crate | `page_tables`, ratified 2026-11-14 (UTC) (milestone 1104, decision 1) |
| version | `0.1.0`; no tag |
| edition | 2024; no `rust-version` yet (the move adds it) |
| license | inherited from the nife workspace; the move writes it into the manifest |
| `publish` | `false` |
| dependencies | none |
| size | 16 files in `src` and three test files; about 4,700 lines in `src` |

## Who in nife depends on it

`kernel` (the mapping of the kernel image, user processes and the direct-memory-access domains, for
the three architectures) and `system_tests` (development only). Each by a path dependency. No program
or other crate names it.

What references back into nife from the crate: the `Name:` blocks, the 12 section and 21 milestone
citations and the nife paths in the falsification records. The move strips the blocks and rewrites
the records; the documentation pass is item 4 of the milestone, and the move lane does it.

## What was run

Run on this branch, after the renames and after the falsification patches were cut again.

| check | result |
|---|---|
| `cargo test -p page_tables` | pass |
| `cargo clippy -p page_tables --all-targets -- -D warnings` | no warning |
| `cargo doc -p page_tables --no-deps` with warnings denied | no warning |
| host `cargo check` of `kernel` and `system_tests` | pass; every caller carries the new names |
| bare-metal builds for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and `x86_64-unknown-none` | pass |
| `cargo miri test -p page_tables` | pass; the integer-to-pointer warnings remain (item 19 below) |
| `script/falsifications --check` | all patches apply; twelve were cut again because the longer names moved rustfmt's line breaks under them |
| `script/names page_tables` | the crate name ratified; the other public names provisional, which this package asks to ratify |
| `cargo xtask test --arch aarch64,riscv64,x86_64 --no-host --image kernel` | the x86_64 OVMF legs pass (kernel suite 164 passed, 14 skipped; system_tests 318 passed, 85 skipped); the screen checks pass. The aarch64 and riscv64 legs ran in the same invocation and the command exited 0 |
| `script/falsifications --check` over the whole tree | no patch fails to apply |
| the four `nth_allocation.*` patches applied by hand, each test run | each test goes red with the defect applied |
| `script/crate-standalone page_tables` | 15 failures, none new: no README, no license files, the manifest inherits `license` and `lints`, 3 prose-cap lines, 9 spelled-out-word lines, and the 80 citation lines of the milestone's items 1, 3, 4 and 11. The build passes on the host and the three bare-metal targets. `proofs`: 38 harnesses verified, 0 failures. Every Kani falsification patch fails its harness as recorded. The four `nth_allocation` patches are reported "still passes" because the replay does not run the integration test, the gap in the milestone's BUGS |
| `script/crate-move --dry-run --into <dir> page_tables` | stages at template `v8`; 42 patches regenerate and apply with no offset and no fuzz; `Name:` blocks removed from 11 files. 9 findings for the move lane: the missing README, the manifest's inherited keys (so `cargo generate-lockfile` fails), the prose caps, the citations and nine private test or harness names that still contain `va` (`the_user_va_gate_...`), which the spelled-out check refuses; renaming them moves with the harness registries and the three patch file names, so the move lane takes them |

Not run, and so not claimed:

- Mutation: a census runs in the Pacific night, 08:00 to 13:00 UTC, and the renames change no logic.
  The new repository's first weekly run takes it.
- The stranger test and the template's workflows: they run in the new repository.
- `cargo-semver-checks`: a first release has no baseline.
- Dependents outside nife: there are none.
- Items 1, 3, 4, 9 and 11 of the milestone: the move lane's work.

## Public API

| kind | items |
|---|---|
| format trait | `PageFormat` and its six implementors `Aarch64`, `Sv39`, `Sv39Mae`, `Ia32e`, `Vtd`, `AmdVi` |
| mapper | `Mapper::{new, root, half, map, map_block, map_span, map_range, unmap, unlink_table, translate, walk}`, `Half`, `MapError` |
| walk | `Walk::{steps, end}`, `WalkStep { table, index, entry }`, `WalkEnd` |
| sizes | `PAGE_SIZE`, `ENTRIES`, `PageSize::{FourKibibytes, TwoMebibytes, OneGibibyte}`, `PageSize::{ALL, bytes, levels_above_bottom, largest_fitting}` |
| flags | `Flags` with eleven constructors, `Flags::ALL` and the predicates |
| table | `PageTable::new`, the public `entries` field |
| flush | `TlbFlush::{flush, assume_no_stale_entry, address}` |
| gate | `is_user_page_virtual_address` |
| domain | `DirectMemoryAccessRegion`, `grant_pages`, `grant_page`, `build_identity_domain` |
| format modules | `aarch64::mair`, `x86_64::pat`, the `AmdVi` device table entries, `Sv39Mae::memory_type_of` |

No nife type, capability or handoff appears in a public signature; the packet's foreign-types table
is empty. The packet is `script/crate-review-packet page_tables`.

## Proofs

Thirty-eight Kani harnesses, each with a falsification patch under `falsifications/`, cover the
indices and offsets of each format, the two halves, the user gate, the writable-and-executable
refusal, block and leaf encodings, the grant enumeration of the domain builder and the reserved bits
of the VT-d and AMD-Vi entries. Four host tests carry falsification patches
(`nth_allocation.*`): a failed map leaks no table and a failed span or domain build maps a prefix.
The template replays Kani falsifications only, so the four host-test patches have no gate in the new
repository until someone adds one.

## Names

Every name below is provisional, ruled by the maintainer on 2026-11-19 (UTC) under the standing units
rule, a two-way door before the first publish. The architect ratifies from here. The "proposal" column
is what this package recommends.

| item | kind | status | proposal |
|---|---|---|---|
| `page_tables` | crate | ratified 2026-11-14 | stands |
| `PageSize::FourKibibytes`, `TwoMebibytes`, `OneGibibyte` | variants | provisional; were `Size4KiB`, `Size2MiB`, `Size1GiB`; each carries a `doc(alias)` of the old spelling | ratify; the alias keeps the `x86_64` crate's spelling findable |
| `is_user_page_virtual_address` | function | provisional; was `is_user_page_va` | ratify |
| `PageFormat::entry_physical_address` | trait method | provisional; was `entry_pa` | ratify |
| `virtual_address`, `physical_address` | parameters of `Mapper`, `PageFormat`, `PageSize::largest_fitting` and `TlbFlush` | provisional; were `va`, `pa` | ratify |
| `physical_to_pointer`, `allocate_page_frame` | `Mapper::new` parameters | provisional; were `phys_to_ptr`, `alloc_page_frame` | ratify |
| `DirectMemoryAccessRegion` | struct | provisional; was `DmaRegion` | ratify; alias `DMA` is a candidate the architect may add |
| `Flags::kernel_read_only_data`, `user_read_only_data` | constructors | provisional; were `kernel_rodata`, `user_rodata` | ratify |
| `x86_64::pat::MODEL_SPECIFIC_REGISTER` | constant | provisional; was `MSR`; alias `MSR` and `IA32_PAT` | ratify |
| `Flags::ALL` | constant | provisional | ratify |
| `Aarch64`, `Sv39`, `Sv39Mae`, `Ia32e`, `Vtd`, `AmdVi` | types | kept specification names, each with `doc(alias)` for the vendor spelling | ratify as kept names |
| `mair`, `pat` | modules | kept names, with aliases of the full register names | ratify as kept names |
| `TlbFlush` | struct | kept; the doc opens with the spelled-out term and the alias `translation lookaside buffer` | ratify as a kept term of art |
| `Mapper`, `Half`, `MapError` and its variants, `Walk`, `WalkEnd`, `WalkStep`, `PageTable`, `PAGE_SIZE`, `ENTRIES` | | provisional, unchanged | ratify as built |
| `Mapper::unlink_table`, `walk`, `makes_writable_executable_pair`, `is_write_combining`, `write_combining`, `user_device_read_only` | | provisional, unchanged; `script/names` lists them as crossing into the kernel and `system_tests` | ratify as built |
| `MapError::WritableAndExecutable` | variant | provisional, unchanged; item 12b proposes moving it to the kernel | decide with item 12b below |

No alias carries an old nife spelling except the variants' `x86_64` spellings, which are foreign terms
of art. `Name:` blocks in the crate still read provisional for these items; the move strips them, so
a lane rewrites each to ratified once the architect rules.

## Findings of the review: where each stands

| item | state |
|---|---|
| P0-2, the crate name | done; the crate is `page_tables` |
| P1-5, the renames | done in the tree, provisional |
| P1-6, `doc(alias)` | done in the tree |
| P2-14, the leaf-size spelling | done in the tree, provisional |
| P0-1, P0-3, P1-4, P1-9, P1-11 (items 1, 3, 4, 9, 11) | move lane |
| P2-12b, `WritableAndExecutable` in `MapError` | open; a change to a public enum, left for a ruling |
| P2-17, the test pool shape | open; not converted here |
| P2-19, Miri's integer-to-pointer casts | open; `with_exposed_provenance` still draws the warnings, so the crate keeps `physical_to_pointer`'s contract and the warnings stay recorded in `notes/undefined-behavior.md` |
| the rest of P1 and P2 | done in #3567 and #3594 |

## What still blocks the move

| item | owner |
|---|---|
| Items 1, 3, 4, 9 and 11 of the milestone: a standalone manifest with `rust-version`, replayable records, documentation for a stranger, the mutation exemptions and a README. | move lane |
| Rewrite the provisional `Name:` blocks to ratified once the architect rules. | a later lane |
| `Cargo.toml` of `kernel` and `system_tests`: convert the path dependency to a `git` pin after the repository exists. | move lane |
| A replay step for the four host-test patches. | move lane |

## Open questions

1. Whether `MapError::WritableAndExecutable` leaves the crate (item 12b). Recommendation: it stays
   until the first release is near; the maintainer decides it, a two-way door.
2. Whether to add the alias `DMA` to `DirectMemoryAccessRegion`. Recommendation: yes.

## Recommendation

Ready for review.
