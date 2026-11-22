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
dependency is `uuid`, with default features off. Seventeen Kani harnesses prove the CRC, the GUID
byte order, the entry and header layouts, the overlap rule, that `create` never builds a table
`parse` would refuse, and that `Header::decode` and `parse` are total on a bounded input. The tests run against tables that `sgdisk` and macOS's `diskutil` wrote. The
architect cleared the crate for its move on 2026-11-20 (UTC) with no tag and no publication, and
every `Name:` block in it reads ratified.

## Identity

| item | value |
|---|---|
| crate | `globally_unique_identifier_partition_table`, ratified (§154 (the acronym test is whether the phrase is spoken)); `gpt` and `guid_partition_table` refused |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.89: `uuid` 1.28 needs 1.89, one above the 1.88 the other packages share |
| license | `MIT OR Apache-2.0`, both texts at the crate root; nothing adapted |
| `publish` | `false` |
| dependency | `uuid = { version = "1.28.0", default-features = false }`, no transitive dependency |
| size | about 2,200 lines of `src` in 11 files, 1,231 lines of tests, 4 fixture files of 16.5 to 17 KiB |
| identity values | no `INTERFACE_ID`, no `VERSION` and no magic of its own. See [Identity values](#identity-values) |
| consumers in nife | `components`, `boot_slot`, `redoxfs_server`, `tools/redoxfs_host`, `uefi_loader`, `xtask` and the `fuzz` workspace, all by path |

## What was run

Each check ran in this lane's worktree, on this branch.

| check | result |
|---|---|
| `script/crate-standalone globally_unique_identifier_partition_table` | the `names` check failed on the 82 names that had no ratified block, which the ratification below clears. The build on 1.89 for the host and the three bare-metal targets passes; Kani proves 8 of 8; each of the 8 falsification patches makes its harness fail |
| `patch -p1 --dry-run -F0` for each patch | all 8 apply with no offset and no fuzz. On `main` all 8 applied only at an offset of 16 to 31 lines; this branch cuts them again, and no defect changes |
| `script/crate-move --dry-run` | the copy builds and tests on 1.89 for every target, and all 8 regenerated patches apply exactly. Three findings: two in the template's scaffold, and 18 lines naming nife, 5 of them the scaffold's and 13 the crate's "nife data" and "nife boot slot" strings (decision 1) |
| `cargo test --release` | 58 tests and 4 doctests pass, the README's included; one exhaustive sweep is `#[ignore]` at 163 seconds and did not run |
| `python3 helpers/name_release.py --draft` | 50 names ratified, 1 recorded (`REVISION`), none provisional, and 52 with no block (modules and items with no `Name:` paragraph, which the standalone check does not refuse) |
| the template's checks (`vendor/crate-workflows/helpers/`, `--manifest Cargo.toml` over `src/*.rs`) | `unit_symbols`, `question_shape`, `british_spelling`: 432 names, 0 refused. `spelled_out`: 1, a test function. `register_names`: 10, five of them public. `abbreviations` (report only): 72 components, chiefly `lba`, `mbr` and `guid` |
| `helpers/lint_externalized_crates.py` | 21 hits, under its ceiling, which this branch lowers from 23 |
| `script/crate-review-packet` | the facts behind the tables here |

Not run, and so not claimed: the stranger test, mutation (the baseline row records 3 missed),
`cargo-semver-checks` (a first release has no baseline) and the template's workflows.

## Public API

| kind | items |
|---|---|
| table | `PartitionTable`: `parse`, `create`, `check_backup`, `check_protective_master_boot_record`, `write_protective_master_boot_record`, `write_primary_header`, `write_backup_header`, and 14 readers (`header`, `disk_guid`, `block_bytes`, `block_count`, `first_usable_logical_block_address`, `last_usable_logical_block_address`, `backup_header_logical_block_address`, `primary_entry_array_logical_block_address`, `backup_entry_array_logical_block_address`, `entry_array_blocks`, `entry_count`, `entry_array`, `get`, `partitions`) |
| header | `header::Header` with nine public fields, `decode`, `encode_into`, `SIGNATURE`, `REVISION` |
| entry | `entry::Entry` with six public fields, `UNUSED`, `new`, `with_name`, `is_used`, `blocks`, `name_utf8`, `decode`, `encode`, `overlaps`; `ENTRY_BYTES`, `NAME_UNITS` and three `ATTRIBUTE_` bits |
| GUIDs | `guid::from_disk`, `guid::to_disk`, the `Uuid` re-export, and `guid::types` with six constants and `name` (nife's two type GUIDs are in `nife_partition_types`) |
| master boot record | `master_boot_record::write`, `master_boot_record::validate`, `PROTECTIVE_OPERATING_SYSTEM_TYPE` |
| reads | `span::Span { first_block, blocks, offset }`, `covering`, `buffer_bytes` |
| errors | `Error` (23 variants, `Display`), `BackupMismatch` (9), `MasterBootRecordProblem` (5) |
| root | `MINIMUM_BLOCK_BYTES`, `MAXIMUM_BLOCK_BYTES`, `PRIMARY_HEADER_LOGICAL_BLOCK_ADDRESS`, `DEFAULT_ENTRY_COUNT`, `ENTRY_ARRAY_BYTES`, `is_block_size_ok` |
| helpers | `crc::crc32`, `testing::sample_disk` |

The one foreign type in a public signature is `uuid::Uuid`. No nife type, capability or handoff
appears.

What a stranger may find surprising:

- `types::NIFE_DATA` and `types::NIFE_BOOT` are nife's own partition types, and `types::name`
  answers "nife data" and "nife boot slot" for them. Decision 1 below asks where they live.
- `Error`'s `Display` prints `BackupMismatch` and `MasterBootRecordProblem` with `{:?}`, so a person reads
  "the backup header disagrees about MyLba". Neither has a `Display` of its own.
- `Error` implements `Display` but not `core::error::Error`, and no enum is `#[non_exhaustive]`.
- `#![allow(missing_docs)]` covers 23 undocumented items: 18 fields of `Error` variants, four
  `BackupMismatch` variants and `MbrProblem::ExtraRecord`'s field. The README's `BUGS` says so.
- `parse` takes a header block and an entry array; the caller reads the disk, and the `span` module
  says which blocks to read.

## Proofs and falsification patches

Seventeen harnesses in `src/verification.rs`, each with one patch under `falsifications/`.

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
| `an_entries_block_count_never_panics_and_is_exact` | an entry's block count never overflows and is exact |
| `a_backup_header_restores_the_table_it_was_written_from` | the backup header maps back to the table it was written from, on every disk size `create` accepts |
| `the_header_checks_before_the_crc_are_total` | the checks `Header::decode` makes before its CRC never panic on a 512-byte block |
| `the_header_crc_in_pieces_is_the_crc_of_the_whole` | the CRC fed in three pieces equals the CRC of the joined bytes |
| `a_span_always_contains_what_was_asked_for` | `Span::covering` contains the requested range |
| `a_name_always_decodes_into_its_documented_buffer` | an entry's name always fits the buffer its documentation names |
| `the_entry_array_check_is_total` | for any header and a 128-byte array, the array check never panics and accepts only one 128-byte entry |
| `the_table_checks_are_total_on_one_entry` | for any header with one 128-byte entry, the table checks never panic and an accepted table keeps both arrays outside the usable range |
| `the_partition_checks_are_total_on_one_entry` | for any header and one 128-byte entry, the partition check never panics and accepts a used entry only inside the usable range |

The last three, with the two `Header::decode` stages, make `parse` total on a 512-byte block and a
one-entry array. The first replaces the CRC with a free value, so every comparison with a stored
CRC goes either way; the second replaces the checks on either side with what the first and third
prove of them. They need `-Z stubbing`. The CRC harnesses stop at 8 bytes, and the tests cover the
real lengths and every single-byte corruption of a real header block and of a small table.

## Names

The architect ratified the crate's names on 2026-11-20 (UTC), the whole table as built and the seven
names that were only proposed. Every `Name:` block in the crate reads ratified, and the ratification
is one pull request carrying the table `helpers/name_release.py --draft` prints. `MINIMUM_BLOCK_BYTES`
and `MAXIMUM_BLOCK_BYTES` replace two names #2797 ratified, so they are the ones a release review
reopens. `REVISION` records the specification's field. The crate has no alias for an earlier
nife spelling, since nothing has shipped.

| # | items | name |
|---|---|---|
| 1 | `types::NIFE_DATA`, `types::NIFE_BOOT` | moved to the crate `nife_partition_types`, values unchanged; `types::name` here knows only the well-known types |
| 2 | every `lba` component: the five `Header` fields, `Entry::first_logical_block_address` and `last_logical_block_address`, `PRIMARY_HEADER_LOGICAL_BLOCK_ADDRESS`, the table readers, `BackupMismatch::EntryArrayLogicalBlockAddress` | spelled `logical_block_address`, each specification spelling a `#[doc(alias)]` |
| 3 | `master_boot_record`, `MasterBootRecordProblem`, `Error::MasterBootRecord`, `check_protective_master_boot_record`, `write_protective_master_boot_record` | alias `MBR` |
| 4 | `MINIMUM_BLOCK_BYTES`, `MAXIMUM_BLOCK_BYTES` | unit in the name |
| 5 | `ENTRY_BYTES`, `ENTRY_BYTES_U32`, `HEADER_BYTES`, `at::HEADER_BYTES`, `at::ENTRY_BYTES` | unit in the name |
| 6 | `Header::entry_bytes`, `Error::EntryBytes`, `BackupMismatch::EntryBytes` | alias `SizeOfPartitionEntry` on the field |
| 7 | `PartitionTable`, `block_bytes`, `create`'s parameter `block_bytes` | no alias |
| 8 | `ATTRIBUTE_REQUIRED_PARTITION`, `ATTRIBUTE_NO_BLOCK_IO_PROTOCOL`, `ATTRIBUTE_LEGACY_BIOS_BOOTABLE` | aliases `Required Partition`, `No Block IO Protocol`, `Legacy BIOS Bootable`, UEFI 2.10 section 5.3's attribute names, checked against the specification |
| 9 | `primary_entry_array_logical_block_address`, `backup_entry_array_logical_block_address` | the header field is `entry_array_logical_block_address` |
| 10 | `PartitionTable::get(index)` | the standard library's word for a checked lookup |
| 11 | the parameter `transfer_bytes` of `Span::covering` and `buffer_bytes` | unit in the name |
| 12 | `Error::PartitionsOverlap { first, second }` | the lower index first |
| 13 | `Header` and `Entry` fields that mirror UEFI's | aliases `MyLBA`, `AlternateLBA`, `FirstUsableLBA`, `LastUsableLBA`, `PartitionEntryLBA`, `StartingLBA`, `EndingLBA`, `SizeOfPartitionEntry`, `DiskGUID`, `NumberOfPartitionEntries`, `PartitionEntryArrayCRC32`, `PartitionTypeGUID`, `UniquePartitionGUID`, each checked against Tables 5.5 and 5.6 |
| 14 | `APPLE_HFS_PLUS`, `APPLE_APFS` | no rename; aliases `Mac OS Extended` and `Apple File System` |
| 15 | the private test | `the_three_reads_a_partition_table_reader_makes` |
| 16 | `VERSION` | removed: the crate owns no format, so it has no version to declare (see [Identity values](#identity-values)) |

The lane checked the aliases in rows 8 and 13 against UEFI 2.10 section 5.3 on 2026-11-20 (UTC). Row 14's
two aliases are Apple's product names, which the specification does not carry.

## Identity values

The crate defines no run-time format of nife's. [Milestone 956 (the linked interfaces carry
checkable versions)](../../design/roadmap/0956-the-linked-interfaces-carry-checkable-versions.md)
classifies it among the four linked crates that stay Rust APIs, and
`helpers/interface_declarations_check.py` gives the reason in `NOT_A_FORMAT`: UEFI's on-disk
table, whose revision the header carries. So it has no row in 956's id table, is owed no
`INTERFACE_ID` and declares no `VERSION`, as `master_boot_record` and `nife_partition_types` declare
none. A version would describe a format the crate does not own, and adding one later breaks no
caller, where removing one would.

| constant | value | meaning |
|---|---|---|
| `header::SIGNATURE` | `EFI PART` | UEFI's, not the crate's |
| `header::REVISION` | `0x0001_0000` | UEFI's, the only revision there is |
| `nife_partition_types::NIFE_DATA` | `EC5CC08B-D749-4434-AC38-A274C50385BA` | nife's data partition type; fixed on every disk written |
| `nife_partition_types::NIFE_BOOT` | `11631EE3-E18F-4AFC-9D7F-171572635629` | nife's boot slot type; `boot_slot` owns its attribute bits 48 to 63 |

No value changes. The two type GUIDs, now in `nife_partition_types`, are nife's real identity values: every
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

For the move lane:

| item | owner |
|---|---|
| `tests/real_disks.rs` and the README's fixture recipe label one partition "cricker data", the operating system's earlier name, on purpose: the fixture is a disk from before the rename. A stranger meets the old name without the reason | move lane: say in the README that the label is arbitrary |
| the 23 undocumented items | done; the crate has no `#![allow(missing_docs)]` |
| `Display` for `BackupMismatch` and `MasterBootRecordProblem`, `core::error::Error` for `Error` | maintainer, provisional; nothing outside nife has acted on the enums |
| `script/crate-move`'s nife check skips a word joined by an underscore, so it passes `NIFE_DATA` that the externalized-crates lint catches | the runbook's `BUGS`; the constants have left the crate |
| convert the seven path dependencies; move the fuzz target; read the 37 listed files | move lane |

## What still blocks

Nothing blocks the move to `nifeos/globally_unique_identifier_partition_table`, which waits only for
the move lane. The first tag's condition, a bounded totality harness for `parse`, is met:
`the_entry_array_check_is_total`, `the_table_checks_are_total_on_one_entry` and
`the_partition_checks_are_total_on_one_entry`, with the two `Header::decode` harnesses, prove
`parse` total on a 512-byte block and a one-entry array. They need `-Z stubbing`, so the new
repository's `proofs.yml` passes it.

## Open questions

None. The `parse` totality harnesses exist, and `VERSION` is removed.

## Decisions for the architect

None open. The architect ratified the names and cleared the repository
`nifeos/globally_unique_identifier_partition_table` on 2026-11-20 (UTC), as `publish = false`, with
no tag and no publication.

## Recommendation

Cleared for the move on 2026-11-20 (UTC), with no tag and no publication. The crate builds, proves
and passes its patches on its own, points nowhere into nife, carries no nife type GUID, and the
architect ratified every name in it. The first tag's condition, a bounded `parse` totality
proof, is met, as stated under "What still blocks".
