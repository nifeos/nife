# Stage 2: the sector interface, the boot sector, and the oracle's pin

Appendix to [813. `file_allocation_table`](../0813-a-proven-fat-crate-released-on-its-own.md). It
records what stage 2 built, the provenance of the names it minted, and where the oracle's
dev-dependency pin, which this stage did not need, is recorded.

## What stage 2 built

- `SectorDevice`, the sector interface. A caller states a `SectorSize` (512, 1024, 2048 or 4096)
  and implements `read_sector(index, buffer)`, where `buffer` is always one sector. It names no
  nife type, no capability and no partition: a caller reads a volume inside a partition through
  an implementation that offsets the index. A blanket implementation covers `&mut D`.
- `BootSector::parse`, over any byte slice, reading only bytes 0 to 511. It requires the signature
  and the jump, checks every BPB field a later read depends on, and refuses with a `ParseError`
  that names the field. What it accepts has its regions in order inside its own sector count, at
  least one cluster, and a table entry for every cluster.
- The FAT type by cluster count, per Microsoft's specification, never by the type string. The
  parse refuses a BPB whose layout contradicts its count. The case met in the field is the FAT32 layout
  under 65525 clusters that `mkfs.fat -F 32` makes with a warning; Linux reads it as FAT32 and the
  specification as FAT16, so no answer here can agree with both.
- `BootSector::read`, which reads sector 0 through a device and refuses a volume whose sector size
  is not the device's.
- The parse keeps FAT32's pointers into the reserved region (`FSInfo`, the backup boot sector)
  only when they name a reserved sector. Otherwise they read as `None` instead of refusing the
  volume, which is what Linux does with a bad `FSInfo` number.

The evidence:

| check | result |
|---|---|
| Kani, four harnesses over every 512-byte sector | `what_the_parse_accepts_lies_inside_the_volume`, `the_fat_type_is_the_cluster_counts`, `the_table_has_an_entry_for_every_cluster`, `only_a_data_cluster_has_sectors`; 27 seconds together |
| falsification sweep | every patch red, each at its own assertion |
| fuzz target `file_allocation_table_boot_sector` | 26 million executions in 120 seconds, no crash |
| `cargo mutants --in-diff` | 168 caught, 8 unviable, none missed |
| line coverage | `boot_sector.rs` and `sector.rs` 100% |
| fixtures | six boot sectors dosfstools' `mkfs.fat` 4.2 formatted (FAT12, FAT16 and FAT32, at 512 and 4096 bytes), rebuilt by `tests/fixtures/make-boot-sectors.sh` |

#1803's macOS images are 64 MiB each and are not in the tree, and no macOS host was at hand, so
this stage reads no macOS-made boot sector. Stage 4 reads both images to their hashes.

## Names

The crate's sources carry no `Name:` blocks, because the crate publishes on its own and its
sources cite nothing of this tree. The ruling of 2026-10-25 (UTC) ratified every name below and
refused `tables` for `BootSector::table_count`. The record is
[file-allocation-table-names.md](../0857-crates-released-on-their-own/file-allocation-table-names.md).
The table gives the reason for each:

| name | why this one |
|---|---|
| `SectorDevice` | the caller supplies a device it addresses by sector. Not `BlockDevice`, the word `embedded-sdmmc` and `hadris-fat` use: FAT's unit is the sector, and this tree's own block protocol is a different thing |
| `SectorSize` | the four sizes the specification permits, as an enum, so no wrong size exists to pass |
| `BootSector` | the specification's name for sector 0 |
| `FatType` | the specification's term ("FAT type"). `Fat12`, `Fat16` and `Fat32` keep Microsoft's spelling, as a name arriving across somebody else's format does |
| `RootDirectory` | where the root lives: a region on FAT12 and FAT16, a cluster on FAT32 |
| `ParseError`, `ReadError` | a refusal of the bytes, and a failure of the read that fetched them |
| `boot_sector`, `sector` | the modules, named for the on-disk structure each one parses |

## The oracle's pin

Stage 2 needed no oracle: its tests check a boot sector against `mkfs.fat`'s output. A ruling of
2026-10-25 (UTC) set the pin for stage 3, which reads tables rust-fatfs wrote:
`lamfat = "=0.4.3"`, a dev-dependency of this crate alone.
[The stage 3 appendix](stage-3-the-table-and-the-chain.md) records the rule, the `fsck` check
its condition asked for, and what holds it.
