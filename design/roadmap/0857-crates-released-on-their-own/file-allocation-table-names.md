# The names in `file_allocation_table`, for ratification

An appendix of [milestone 857 (crates released on their own)](../0857-crates-released-on-their-own.md), for milestone 813's crate. A release-track crate's names are ratified before it leaves this tree (ruled 2026-10-25 UTC), so this file lists every name a stranger meets in it, as the record `helpers/name_release.py` reads.

Nothing here is ratified by this file. Until the line below carries a date, the crate's own `Name:` blocks are the record and the status column is a snapshot of them, taken 2026-10-25 (UTC); `python3 helpers/name_release.py --draft file_allocation_table` prints a fresh one. At ratification each row's status takes the ruling, with any refusal its block holds, and the blocks leave the crate.

Ratified: not yet.

A status of `unrecorded` means the name has no block in the crate. 26 names: 21 no block, 4 provisional, 1 ratified. 3 are not reachable from outside the crate (private modules, and `pub` items in them that are not re-exported). The last column applies design/naming.md's tests and 857's Fork 5 (a name stands on its own, since the crate may outlive nife): `clear`, `term` for a name the specification or databook owns (kept in its spelling, as design/naming.md asks of a name across somebody else's interface), `nife` for a name of nife's own process plan (address-space windows, the handoff a server receives), which a stranger does not have and which the crate may shed before it leaves, `scope` for a name that belongs to something other than the crate's subject, and otherwise what a stranger would trip on; 13 rows say something other than clear or term. Enum variants, fields and trait methods appear only where a block names one, as `script/names` counts them. Nothing is renamed here.

| name | kind | path | status | outside | a stranger reads it |
|---|---|---|---|---|---|
| `file_allocation_table` | crate | `src/lib.rs` | ratified 2026-10-07. | yes | clear |
| `long_name` | module | `src/long_name.rs` | provisional. | no | clear: the specification's long name; private |
| `verification` | module | `src/long_name/verification.rs` | provisional. | no | clear; private, the Kani harnesses |
| `volume` | module | `src/volume.rs` | provisional. | no | clear; private |
| `BOOT_CLUSTER` | item | `src/volume.rs` | unrecorded. | yes | unclear alone: names the one volume shape this writer lays out |
| `CLUSTER` | item | `src/volume.rs` | unrecorded. | yes | unclear: holds bytes per cluster; `CLUSTER_BYTES` says it |
| `CLUSTER_SECTORS` | item | `src/volume.rs` | unrecorded. | yes | clear: the specification's sectors per cluster |
| `EFI_CLUSTER` | item | `src/volume.rs` | unrecorded. | yes | unclear alone: names the one volume shape this writer lays out |
| `Error` | item | `src/volume.rs` | unrecorded. | yes | unclear once the reader lands beside `ParseError` and `ReadError`: it is the writer's refusal |
| `FATS` | item | `src/volume.rs` | unrecorded. | yes | unclear: the acronym inside a crate that spells it out; the reader #2709 adds says `tables` |
| `FILE_CLUSTER` | item | `src/volume.rs` | unrecorded. | yes | unclear alone: names the one volume shape this writer lays out |
| `MIN_CLUSTERS` | item | `src/volume.rs` | unrecorded. | yes | unclear once FAT12 and FAT16 are read: it is FAT32's floor |
| `NameRefused` | item | `src/volume.rs` | provisional. | yes | clear: a variant of `Error` |
| `RESERVED_SECTORS` | item | `src/volume.rs` | unrecorded. | yes | clear: the specification's reserved sector count |
| `ROOT_CLUSTER` | item | `src/volume.rs` | unrecorded. | yes | clear: the specification's root cluster |
| `SECTOR` | item | `src/volume.rs` | unrecorded. | yes | unclear: reads as a sector, holds a size in bytes (512); `SECTOR_BYTES` says it |
| `Volume` | item | `src/volume.rs` | unrecorded. | yes | unclear once the reader lands: it is a volume to be written, not one read |
| `cluster_sector` | item | `src/volume.rs` | unrecorded. | yes | unclear beside the reader's `cluster_first_sector` for the same quantity |
| `clusters` | item | `src/volume.rs` | unrecorded. | yes | clear |
| `data_first_sector` | item | `src/volume.rs` | unrecorded. | yes | unclear beside the reader's `first_data_sector` for the same quantity |
| `fat_sectors` | item | `src/volume.rs` | unrecorded. | yes | unclear beside the reader's `table_sectors` for the same quantity |
| `file_first_sector` | item | `src/volume.rs` | unrecorded. | yes | clear within the writer's one-file shape |
| `file_sectors` | item | `src/volume.rs` | unrecorded. | yes | clear within the writer's one-file shape |
| `metadata_sectors` | item | `src/volume.rs` | unrecorded. | yes | clear |
| `new` | item | `src/volume.rs` | unrecorded. | yes | clear |
| `sector` | item | `src/volume.rs` | unrecorded. | yes | unclear: reads as a getter; it fills `out` with one sector's bytes |

## Names a stranger meets that the table does not hold

- `Error`'s variants: `NotFat32`, `TooLarge`, `NameRefused` (in the table, since a block names it)
  and `FileTooBig`.
- Pull request #2709 (milestone 813's stage 2, the reader) adds these, each `provisional` in its
  block, and they join the table when it merges. Module `boot_sector`: `BootSector` (`parse`,
  `read`, `sector_size`, `sectors_per_cluster`, `bytes_per_cluster`, `fat_type`, `clusters`,
  `total_sectors`, `reserved_sectors`, `tables`, `table_sectors`, `table_first_sector`,
  `active_table`, `first_data_sector`, `root_directory`, `media`, `hidden_sectors`,
  `fs_info_sector`, `backup_boot_sector`, `volume_id`, `cluster_first_sector`), `FatType`
  (`Fat12`, `Fat16`, `Fat32`, `from_clusters`, `entry_bits`), `RootDirectory` (`Region`,
  `Cluster`), `ParseError` (18 variants), `ReadError` (`Device`, `Refused`,
  `SectorSizeMismatch`). Module `sector`: `SectorSize` (`Bytes512` to `Bytes4096`, `MAX`,
  `from_bytes`, `bytes`, `shift`) and the trait `SectorDevice` (`Error`, `sector_size`,
  `read_sector`). Every one passes the stranger test as the specification's own vocabulary, with
  one exception: `tables` counts the copies of the table, and `table_count` would say so.

## The pattern in this list

The writer came first and its names describe the one volume it lays out. The reader arriving in
#2709 names the same quantities differently: `fat_sectors` and `table_sectors`,
`data_first_sector` and `first_data_sector`, `cluster_sector` and `cluster_first_sector`, `FATS`
and `tables`. A stranger meets both in one crate, so each pair wants one spelling before
ratification. The reader's spelling follows the specification's (`FirstDataSector`), which argues
for it.
