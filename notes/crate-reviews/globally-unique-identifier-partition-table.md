# Review package: `globally_unique_identifier_partition_table`

*Name: provisional, minted by lane lane/review-prep-guid-partition-table. The package of
[crate-reviews.md](../crate-reviews.md) for this crate, in its own file because the packages
together outgrow the page.*

The architect reads this to clear the crate for its move to its own repository, at step 2 of
[milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md).
Its disposition is contribute: [milestone 857 (crates released on their own)](../../design/roadmap/0857-crates-released-on-their-own.md)
weighs offering its proofs to `google/gpt-disk-rs` in
[an appendix](../../design/roadmap/0857-crates-released-on-their-own/contribute-proofs-instead-of-releasing.md),
and the architect deferred that offer until a published crate has users who ask for it. The crate
moves in the ruled order meanwhile. The package describes the crate as this branch leaves it.

## In one paragraph

The GUID Partition Table of UEFI 2.10 section 5.3 as pure computation. It parses a primary table
from the bytes a caller read, checks the backup copy against it, builds a new table and lays out the
protective MBR and both headers. It is `no_std`, with no allocation, no `unsafe` and no I/O. The one
dependency is `uuid`, with default features off. Eight Kani harnesses prove the CRC, the GUID byte
order, the entry and header layouts, the overlap rule and that `create` never builds a table
`parse` would refuse. The tests run against tables that `sgdisk` and macOS's `diskutil` wrote. What
blocks the move is the crate's own vocabulary: two type GUIDs that belong to nife, and 82 names
without a ratified block.

## Identity

| item | value |
|---|---|
| crate | `globally_unique_identifier_partition_table`, ratified (§154 (the acronym test is whether the phrase is spoken)); `gpt` and `guid_partition_table` refused |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.89: `uuid` 1.28 needs 1.89, one above the 1.88 the other packages share |
| license | `MIT OR Apache-2.0`, both texts at the crate root; nothing adapted |
| `publish` | `false` |
| dependency | `uuid = { version = "1.28.0", default-features = false }`, no transitive dependency |
| size | 2,159 lines of `src` in 11 files, 1,231 lines of tests, 4 fixture files of 16.5 to 17 KiB |
| identity values | no `INTERFACE_ID` and no magic of its own; `VERSION` 1, with a provisional name. See [Identity values](#identity-values) |
| consumers in nife | `components`, `boot_slot`, `redoxfs_server`, `tools/redoxfs_host`, `uefi_loader`, `xtask` and the `fuzz` workspace, all by path |

## What was run

Each check ran in this lane's worktree, on this branch.

| check | result |
|---|---|
| `script/crate-standalone globally_unique_identifier_partition_table` | one failure, `names`: 82 names not ratified. The build on 1.89 for the host and the three bare-metal targets passes; Kani proves 8 of 8; each of the 8 falsification patches makes its harness fail |
| `patch -p1 --dry-run -F0` for each patch | all 8 apply with no offset and no fuzz. On `main` all 8 applied only at an offset of 16 to 31 lines; this branch cuts them again, and no defect changes |
| `script/crate-move --dry-run` | the copy builds and tests on 1.89 for every target, and all 8 regenerated patches apply exactly. Three findings, all in the template's scaffold (below) |
| `cargo test --release` | 59 tests and the README and module doctests pass |
| `script/names` and `python3 helpers/name_release.py --draft` | 6 names ratified with the crate, 1 recorded, 3 provisional and 77 with no block |
| the template's checks (`vendor/crate-workflows/helpers/`, `--manifest Cargo.toml` over `src/*.rs`) | `unit_symbols`, `question_shape`, `british_spelling`: 432 names, 0 refused. `spelled_out`: 1, a test function. `register_names`: 10, five of them public. `abbreviations` (report only): 72 components, chiefly `lba`, `mbr` and `guid` |
| `helpers/lint_externalized_crates.py` | 21 hits, under its ceiling, which this branch lowers from 23 |
| `script/crate-review-packet` | the facts behind the tables here |

Not run, and so not claimed: the stranger test, mutation (the baseline row records 3 missed),
`cargo-semver-checks` (a first release has no baseline) and the template's workflows.

## Public API

| kind | items |
|---|---|
| table | `GloballyUniqueIdentifierPartitionTable`: `parse`, `create`, `check_backup`, `check_protective_mbr`, `write_protective_mbr`, `write_primary_header`, `write_backup_header`, and 14 readers (`header`, `disk_guid`, `block_size`, `block_count`, `first_usable_lba`, `last_usable_lba`, `backup_header_lba`, `primary_entry_lba`, `backup_entry_lba`, `entry_array_blocks`, `entry_count`, `entry_array`, `entry`, `partitions`) |
| header | `header::Header` with nine public fields, `decode`, `encode_into`, `SIGNATURE`, `REVISION` |
| entry | `entry::Entry` with six public fields, `UNUSED`, `new`, `with_name`, `is_used`, `blocks`, `name_utf8`, `decode`, `encode`, `overlaps`; `SIZE`, `NAME_UNITS` and three `ATTR_` bits |
| GUIDs | `guid::from_disk`, `guid::to_disk`, the `Uuid` re-export, and `guid::types` with eight constants and `name` |
| MBR | `mbr::write`, `mbr::validate`, `PROTECTIVE_OPERATING_SYSTEM_TYPE` |
| reads | `span::Span { first_block, blocks, offset }`, `covering`, `buffer_bytes` |
| errors | `Error` (23 variants, `Display`), `BackupMismatch` (9), `MbrProblem` (5) |
| root | `MINIMUM_BLOCK_SIZE`, `MAXIMUM_BLOCK_SIZE`, `PRIMARY_HEADER_LBA`, `DEFAULT_ENTRY_COUNT`, `ENTRY_ARRAY_BYTES`, `VERSION`, `is_block_size_ok` |
| helpers | `crc::crc32`, `testing::sample_disk` |

The one foreign type in a public signature is `uuid::Uuid`. No nife type, capability or handoff
appears.

What a stranger may find surprising:

- `types::NIFE_DATA` and `types::NIFE_BOOT` are nife's own partition types, and `types::name`
  answers "nife data" and "nife boot slot" for them. Decision 1 below asks where they live.
- `Error`'s `Display` prints `BackupMismatch` and `MbrProblem` with `{:?}`, so a person reads
  "the backup header disagrees about MyLba". Neither has a `Display` of its own.
- `Error` implements `Display` but not `core::error::Error`, and no enum is `#[non_exhaustive]`.
- `#![allow(missing_docs)]` covers 23 undocumented items: 18 fields of `Error` variants, four
  `BackupMismatch` variants and `MbrProblem::ExtraRecord`'s field. The README's `BUGS` says so.
- `parse` takes a header block and an entry array; the caller reads the disk, and the `span` module
  says which blocks to read.

## Proofs and falsification patches

Eight harnesses in `src/verification.rs`, each with one patch under `falsifications/`.

| harness | what it proves |
|---|---|
| `crc32_matches_its_bitwise_definition` | the table-driven CRC-32 equals the bitwise definition on every 8-byte input |
| `a_single_byte_change_always_changes_the_crc` | over every 8-byte buffer, a change to any one byte changes the CRC |
| `an_entry_survives_the_round_trip` | `encode(decode(b)) == b` for all 2^1024 entries |
| `a_guid_survives_the_on_disk_layout` | `to_disk(from_disk(b)) == b` for every 16 bytes |
| `a_headers_fields_survive_the_round_trip` | the nine header fields survive `encode_fields` then `decode_fields` |
| `the_header_fields_partition_the_block` | decoding then encoding any 512-byte block reproduces bytes 24 to 92, so no two fields share a byte and none goes unclaimed |
| `overlap_is_exactly_sharing_a_block` | `overlaps` is true exactly when two used entries share a block, and symmetric |
| `create_never_lays_out_a_table_parse_would_reject` | for any disk size and one partition in a one-entry array, `create` satisfies every geometry rule `parse` checks |

Not proved: that `Header::decode` and `parse` are total on hostile bytes. The tests count every
single-byte corruption of a real header block and of a small table, and nife's fuzz workspace has a
target for the crate; a harness through the CRC chains does not finish, as `verification.rs` says.
The CRC harnesses stop at 8 bytes, and the tests cover the real lengths.

## Names

`script/crate-standalone` fails on names alone. Six items read ratified: `from_disk` and
`to_disk` (#1795), `MINIMUM_BLOCK_SIZE`, `MAXIMUM_BLOCK_SIZE` and
`PROTECTIVE_OPERATING_SYSTEM_TYPE` (#2797), and the crate. `REVISION` reads "recorded", the
specification's field. `VERSION`, `error` and `table` read provisional, and 77 items have no
block. The ratification is one pull request with the table `name_release.py --draft` prints.

The list below proposes a name, provisional, for each item a rule reaches. The lane renames none.
Every other unratified item passes the rules as it stands and is proposed for ratification under
its present name. The fields of `Header` and `Entry` and the variants of the three error enums are
not listed by `name_release.py`, and are listed here.

| # | items | rule | proposed |
|---|---|---|---|
| 1 | `types::NIFE_DATA`, `types::NIFE_BOOT` | a crate repository does not mention nife | decision 1; no new name in this crate |
| 2 | every `lba` component: the five `Header` LBA fields, `Entry::first_lba` and `last_lba`, `PRIMARY_HEADER_LBA`, six table readers, `BackupMismatch::MyLba` to `EntryArrayLba`, `MbrProblem::StartLba` and `SizeLba` | the acronym test | decision 2 |
| 3 | `mbr`, `MbrProblem`, `Error::Mbr`, `check_protective_mbr`, `write_protective_mbr` | the acronym test | decision 2 |
| 4 | `MINIMUM_BLOCK_SIZE`, `MAXIMUM_BLOCK_SIZE` | a limit ends with its unit; ratified before that rule, so the release review reopens them | `MINIMUM_BLOCK_BYTES`, `MAXIMUM_BLOCK_BYTES` |
| 5 | `entry::SIZE` | a size carries its unit | `entry::ENTRY_BYTES`, as `block_roster` spells it |
| 6 | `Header::entry_size`, `BackupMismatch::EntrySize`, `Error::EntrySize` | a size carries its unit; UEFI's field is `SizeOfPartitionEntry` | `entry_bytes`, `EntryBytes`, each with `#[doc(alias = "SizeOfPartitionEntry")]` |
| 7 | `GloballyUniqueIdentifierPartitionTable::block_size`, `create`'s parameter `block_size` | a size carries its unit | `block_bytes` |
| 8 | `ATTR_REQUIRED`, `ATTR_NO_BLOCK_IO`, `ATTR_LEGACY_BIOS_BOOTABLE` | a bit constant copied from a source is spelled out, its source spelling the alias | `ATTRIBUTE_REQUIRED_PARTITION`, `ATTRIBUTE_NO_BLOCK_IO_PROTOCOL`, `ATTRIBUTE_LEGACY_BIOS_BOOTABLE`, each aliased to UEFI's attribute name |
| 9 | `primary_entry_lba`, `backup_entry_lba` | one concept, one word: the header field is `entry_array_lba` and the reader `entry_array_blocks` | `primary_entry_array_lba`, `backup_entry_array_lba`, under decision 2's spelling |
| 10 | `GloballyUniqueIdentifierPartitionTable::entry(index)` | a method takes std's spelling | `get`, as `block_roster::Roster::get` returns a value |
| 11 | `Span::covering`'s and `buffer_bytes`'s parameter `transfer` | a size carries its unit, parameters included | `transfer_bytes` |
| 12 | `Error::PartitionsOverlap { a, b }` | single letters in a public field | `first`, `second` |
| 13 | the `Header` and `Entry` fields that mirror UEFI's (`my_lba`, `disk_guid`, `entry_count`, `entry_array_crc`, `type_guid`, `unique_guid`) | a specification's spelling goes in `#[doc(alias)]` | no rename; add `MyLBA`, `DiskGUID`, `NumberOfPartitionEntries`, `PartitionEntryArrayCRC32`, `PartitionTypeGUID`, `UniquePartitionGUID` |
| 14 | `APPLE_HFS_PLUS`, `APPLE_APFS` | a kept foreign name opens its doc with its full name and carries an alias | no rename; aliases `Mac OS Extended` and `Apple File System` |
| 15 | the private test `the_three_reads_a_gpt_reader_makes` | `gpt` is spelled out; the template's `spelled_out` fails it at a ceiling of 0 | `the_three_reads_a_partition_table_reader_makes` |
| 16 | `VERSION` | provisional; see [Identity values](#identity-values) | keep the name |

The crate's documentation already quotes most of row 13's spellings. `MyLBA`,
`PartitionEntryArrayCRC32` and row 8's attribute names come from memory of UEFI 2.10 section 5.3.
The lane that adds them checks each against the specification first. Rows 4 to 12 are
two-way doors: nothing outside nife has acted on the crate. Row 13 and row 14 add aliases and break
nothing.

## Identity values

The crate defines no run-time format of nife's. [Milestone 956 (the linked interfaces carry
checkable versions)](../../design/roadmap/0956-the-linked-interfaces-carry-checkable-versions.md)
classifies it among the four linked crates that stay Rust APIs, and
`helpers/interface_declarations_check.py` gives the reason in `NOT_A_FORMAT`: UEFI's on-disk
table, whose revision the header carries. So it has no row in 956's id table and is owed no
`INTERFACE_ID`.

| constant | value | meaning |
|---|---|---|
| `VERSION` | `1` | the generation of this crate's reading and writing; a declaration nothing reads at run time, and no file in nife names `globally_unique_identifier_partition_table::VERSION` |
| `header::SIGNATURE` | `EFI PART` | UEFI's, not the crate's |
| `header::REVISION` | `0x0001_0000` | UEFI's, the only revision there is |
| `types::NIFE_DATA` | `EC5CC08B-D749-4434-AC38-A274C50385BA` | nife's data partition type; fixed on every disk written |
| `types::NIFE_BOOT` | `11631EE3-E18F-4AFC-9D7F-171572635629` | nife's boot slot type; `boot_slot` owns its attribute bits 48 to 63 |

The lane changes no value. The two type GUIDs act as the crate's real identity values: every
installed disk carries them, and the loader, the installer and the surveyor match on them.

## What points back into nife

The crate itself points nowhere: no path dependency, no dev-dependency, and the tests read only
their own fixtures. Pointing from nife to the crate, which the direction permits:

- the seven consumer manifests above;
- the fuzz target `fuzz/fuzz_targets/globally_unique_identifier_partition_table.rs` and its seeds
  in nife's fuzz workspace, which move with the crate under
  [milestone 1043 (a moved crate is fuzzed in its own repository)](../../design/roadmap/1043-a-moved-crate-is-fuzzed-in-its-own-repository.md);
- 37 other files the dry run lists, among them the Kani reach list, the undefined-behavior check,
  `script/verify`, `helpers/trust_boundary_crates.tsv`, `helpers/kani-peaks.tsv`,
  `packages/contracts.package.toml`, the two interface helpers and the mutation baseline.

`boot_slot` links the crate for its `crc` and is not on the externalization list, so it stays in
nife.

## Findings

Fixed on this branch, against the crate:

- The README's "written for the nife kernel and its disk programs", `create`'s "the `mkpart` half
  of the milestone", a test comment naming `disk_partitioner` and `system_installer`, and two
  comments speaking of "this OS" and "this project".
- The eight falsification patches, cut again at their exact lines.

For the template, already in [the runbook](../crate-move.md)'s `BUGS`: the scaffold's commit hook,
`deny.toml`, `rustfmt.toml` and `docs/stranger-test.md` name nife, and the pinned release `v7`
lacks the naming gates. The dry run also reports the pull request template's architect line and
`CONTRIBUTING.md`'s `script/record-ruling` path; the template's `main` has neither.

For the move lane, or for decision 1:

| item | owner |
|---|---|
| `NIFE_DATA` and `NIFE_BOOT`, their `name` strings, `testing::sample_disk`'s "nife data" partition, and the tests and harnesses that use them | architect, decision 1 |
| `guid::types`' doc says the constants answer "which one is ours" | the lane that applies decision 1 |
| `tests/real_disks.rs` and the README's fixture recipe label one partition "cricker data", the operating system's earlier name, on purpose: the fixture is a disk from before the rename. A stranger meets the old name without the reason | move lane: say in the README that the label is arbitrary |
| the 23 undocumented items, and `#![allow(missing_docs)]` | the lane that ratifies the names, since it touches each item |
| `Display` for `BackupMismatch` and `MbrProblem`, `core::error::Error` for `Error` | maintainer, provisional; nothing outside nife has acted on the enums |
| `script/crate-move`'s nife check skips a word joined by an underscore, so it passes `NIFE_DATA` that the externalized-crates lint catches | the runbook's `BUGS` |
| convert the seven path dependencies; move the fuzz target; read the 37 listed files | move lane |

## Open questions

1. Whether `parse` should get a totality harness bounded small enough to finish (a 512-byte block,
   a one-entry array). Recommendation: try it in the crate repository, since `parse` is the
   hostile-input surface and the fuzz target is its only machine check today. The maintainer can
   decide it.
2. Whether `VERSION` should stay, since nothing reads it and 956 classes the crate as no format.
   Recommendation: keep it, since a released package can then state which reader it carries, and
   removing it later costs one minor version. The maintainer can decide it.

## Decisions for the architect

Decision 1 comes first, since the move cannot proceed without it, and decision 2 settles most of
the name table.

1. Where nife's two partition type GUIDs live. Recommendation: in a crate that stays in nife, with
   their values unchanged, and `types::name` in this crate drops them.
2. Whether `lba` and `mbr` stay whole or are spelled out. Recommendation: spell both out, as §154
   spelled out `gpt`, the same specification's acronym.
3. Clear the move to `nifeos/globally_unique_identifier_partition_table` once 1, 2 and the name
   ratification land, with no tag and no publication.

## Recommendation

Not ready for review. The crate builds, proves and passes its patches on its own, and points
nowhere into nife. Its two nife type GUIDs and its unratified names block the move.
