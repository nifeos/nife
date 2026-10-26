# The names in `file_allocation_table`

An appendix of [milestone 857 (crates released on their own)](../0857-crates-released-on-their-own.md),
for the crate of milestone 813 (a proven FAT crate, released on its own). This file is the record of
every name in the crate, which `helpers/name_release.py` reads in place of `Name:` blocks: the crate's
shipped files carry none.

Ratified: 2026-10-25 (#2720).

The ruling of that date ratified every name below. Each row with a refusal names the spelling the
ruling turned down and why. The writer's three directory clusters sit in `efi_system_partition`, the
module named for the one volume shape the writer lays out. A new name in the crate takes a row here,
`provisional` until a ruling, and `script/lint` fails a module or `pub` item without one.

The column `outside` says whether a caller outside the crate reaches the name. Three groups of names a
stranger meets have no row because `script/names` does not count them: the fields of
`RootDirectory::Region` (`first_sector`, `sectors`, `entries`) and of
`ReadError::SectorSizeMismatch` (`device`, `volume`), and `SectorDevice`'s members (`Error`,
`sector_size`, `read_sector`). The same ruling covers them.

| name | kind | path | status | outside |
|---|---|---|---|---|
| `file_allocation_table` | crate | `src/lib.rs` | ratified 2026-10-07 (#1806). FAT spelled out, as the acronym test asks; permanent once published. | yes |
| `boot_sector` | module | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | no |
| `tests` | module | `src/boot_sector/tests.rs` | ratified 2026-10-25 (#2720). | no |
| `verification` | module | `src/boot_sector/verification.rs` | ratified 2026-10-25 (#2720). | no |
| `efi_system_partition` | module | `src/efi_system_partition.rs` | ratified 2026-10-25 (#2720). The writer's one volume shape, which holds the three clusters below. | yes |
| `long_name` | module | `src/long_name.rs` | ratified 2026-10-25 (#2720). | no |
| `verification` | module | `src/long_name/verification.rs` | ratified 2026-10-25 (#2720). | no |
| `sector` | module | `src/sector.rs` | ratified 2026-10-25 (#2720). | no |
| `volume` | module | `src/volume.rs` | ratified 2026-10-25 (#2720). | no |
| `BootSector` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `FatType` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `Fat12` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `Fat16` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `Fat32` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `from_clusters` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `entry_bits` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `RootDirectory` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `Region` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `Cluster` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `ParseError` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `TooShort` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `NoSignature` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `NoJump` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `SectorSize` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `ClusterSize` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `NoReservedSectors` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `NoTables` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `Media` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `NoSectors` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `NoTableSectors` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `MetadataPastEnd` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `NoClusters` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `LayoutContradictsClusterCount` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `TooManyClusters` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `TableTooSmall` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `RootCluster` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `Version` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `ActiveTable` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `ReadError` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `Device` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `Refused` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `SectorSizeMismatch` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `parse` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `read` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `sector_size` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `sectors_per_cluster` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `bytes_per_cluster` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `fat_type` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `clusters` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `total_sectors` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `reserved_sectors` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `table_count` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). Refused `tables` (it counts the copies of the table, which the name did not say). | yes |
| `table_sectors` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `table_first_sector` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `active_table` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `first_data_sector` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `root_directory` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `media` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `hidden_sectors` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `fs_info_sector` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `backup_boot_sector` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `volume_id` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `cluster_first_sector` | item | `src/boot_sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `EFI_CLUSTER` | item | `src/efi_system_partition.rs` | ratified 2026-10-25 (#2720). | yes |
| `BOOT_CLUSTER` | item | `src/efi_system_partition.rs` | ratified 2026-10-25 (#2720). | yes |
| `FILE_CLUSTER` | item | `src/efi_system_partition.rs` | ratified 2026-10-25 (#2720). | yes |
| `SectorSize` | item | `src/sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `Bytes512` | item | `src/sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `Bytes1024` | item | `src/sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `Bytes2048` | item | `src/sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `Bytes4096` | item | `src/sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `MAX` | item | `src/sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `from_bytes` | item | `src/sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `bytes` | item | `src/sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `shift` | item | `src/sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `SectorDevice` | item | `src/sector.rs` | ratified 2026-10-25 (#2720). | yes |
| `SECTOR_BYTES` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). Refused `SECTOR` (it reads as a sector and holds a size). | yes |
| `CLUSTER_SECTORS` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). | yes |
| `CLUSTER_BYTES` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). Refused `CLUSTER` (it reads as a cluster and holds a size). | yes |
| `RESERVED_SECTORS` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). | yes |
| `TABLE_COUNT` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). Refused `FATS` (the acronym in a crate that spells it out). | yes |
| `ROOT_CLUSTER` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). | yes |
| `FAT32_MIN_CLUSTERS` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). Refused `MIN_CLUSTERS` (it is FAT32's floor alone). | yes |
| `LayoutError` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). Refused `Error` (the writer's refusal, beside the reader's errors). | yes |
| `NotFat32` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). | yes |
| `TooLarge` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). | yes |
| `NameRefused` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). | yes |
| `FileTooBig` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). | yes |
| `VolumeLayout` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). Refused `Volume` (a volume to be written, not one read). | yes |
| `new` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). | yes |
| `clusters` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). | yes |
| `table_sectors` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). Refused `fat_sectors` (the reader's spelling for the same quantity). | yes |
| `first_data_sector` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). Refused `data_first_sector` (the specification's order). | yes |
| `cluster_first_sector` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). Refused `cluster_sector` (the reader's spelling for the same quantity). | yes |
| `metadata_sectors` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). | yes |
| `file_first_sector` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). | yes |
| `file_sectors` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). | yes |
| `fill_sector` | item | `src/volume.rs` | ratified 2026-10-25 (#2720). Refused `sector` (it fills a buffer and reads as a getter). | yes |
