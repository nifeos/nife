# Stage 3: the table, the chain walk, and directory slots

Appendix to [813. `file_allocation_table`](../0813-a-proven-fat-crate-released-on-its-own.md). It
records what stage 3 built, the oracle's pin and the check its ruling asked for, and the evidence.

## What stage 3 built

- `Volume`, a volume open for reading. It owns the caller's `SectorDevice` and the checked
  `BootSector`, and caches the last table sector it read.
- `TableEntry` and `Volume::entry`: a data cluster's entry, decoded by the FAT type's width and
  markers. The decode ignores a FAT32 entry's top four bits, as the specification says. A FAT12 entry
  that starts at a sector's last byte costs two reads, and the second sector is still the table's.
- `Volume::chain`, the walk. It yields the first cluster and each cluster the table names, and
  ends at an end-of-chain entry. It refuses an entry that is free, reserved or bad, or that names a
  cluster the volume lacks, and once it has yielded as many clusters as the volume holds it fails
  with `TooLong`. No table contents can make it loop. It reads the table FAT32 marks live, or the
  first.
- `Slot`, `DirectoryEntry`, `ShortEntry` and `LongNameEntry`: one 32-byte directory slot, parsed
  from hostile bytes. `Volume::root_directory` and `Volume::directory` read a directory slot by
  slot through the FAT12 and FAT16 root region or through a chain, skip free slots, and stop at
  the end slot. A chain directory inherits the walk's bound.

Stage 4 assembles a long name from its pieces and checks their checksum against the short entry,
and reads #1803's macOS images. Stage 5 reads files and the MBR.

## The oracle's pin

The rule, ruled 2026-10-25 (UTC): the test oracle is `lamfat = "=0.4.3"` from crates.io, a
republish of rust-fatfs's main line under MIT. It is a `[dev-dependencies]` line of
`file_allocation_table` and of nothing else, its checksum is pinned in `Cargo.lock`, and it never
enters the base graph. The ruling carried a condition: before any test relies on 0.4.3, #1803's
`fsck` check runs again on images 0.4.3 writes, subdirectories included, and a failure stops the
stage. The next section records that run.

Three things hold the rule:

- `deny.toml` lists `lamfat` with `wrappers = ["file_allocation_table"]`, so cargo-deny refuses
  it to any other dependent, in every graph `script/supply-chain` checks.
- `script/lint` reads each workspace package's dependencies and fails when `lamfat` appears as
  anything but a dev-dependency of this crate. cargo-deny cannot see a dependency's kind.
- The version is exact and comes from crates.io, so the crate's own repository resolves the same
  package with the same checksum, from its own lockfile, when the crate moves.

## The `fsck` check on 0.4.3

Run on 2026-10-25 (UTC) on Linux x86_64, with dosfstools' `fsck.fat` 4.2 and `-n`. On each image,
0.4.3 wrote three levels of long-named subdirectories, a 300,000-byte file and 40 long-named files,
enough to grow their directory past one cluster. It also deleted a file, renamed one across
directories and made an empty directory. It then remounted the volume and read the large file and
the renamed one back byte for byte.

| image | formatted by | `fsck.fat -n` |
|---|---|---|
| FAT12, 4 MiB | `mkfs.fat -F 12` | exit 0 |
| FAT16, 32 MiB | `mkfs.fat -F 16` | exit 0 |
| FAT32, 64 MiB | `mkfs.fat -F 32` | exit 0 |
| FAT32, 300 MiB, 4096-byte sectors | `mkfs.fat -F 32 -S 4096` | exit 0 |
| FAT12, 4 MiB | lamfat's `format_volume` | exit 0 |
| FAT16, 32 MiB | lamfat's `format_volume` | exit 0 |
| FAT32, 64 MiB | lamfat's `format_volume` | exit 0 |
| control: FAT32, 64 MiB, two nested directories by `fatfs` 0.3.6 | `mkfs.fat -F 32` | exit 1: no valid `.` and `..` entries in either subdirectory |

The control shows the check can tell a broken writer from a sound one: it fails `fatfs` 0.3.6 on
the defect #1803 found with `fsck_msdos`. The run was on Linux, so `fsck_msdos` itself did not
judge 0.4.3. `tests/the_oracle_agrees.rs` runs whichever of the two a host has on every image it
makes.

## The evidence

| check | result |
|---|---|
| Kani, five new harnesses | `every_entry_lies_inside_the_table`, `an_entry_decodes_by_its_markers`, `no_chain_walk_outlasts_the_volume`, `every_slot_parses_as_its_bytes_say`, `a_directory_reads_only_its_own_sectors`; the crate's twelve together take 3 min 43 s on one x86_64 host |
| falsification sweep | all twelve of the crate's patches turn their harnesses red; the five new ones take 3 to 56 s each |
| oracle tests | rust-fatfs formats and fills FAT12, FAT16 (512- and 4096-byte sectors) and FAT32 volumes; this crate lists the same tree, long-name units, sizes and bytes, reads each chain at exactly the clusters its size needs, and counts the same free clusters. rust-fatfs reads back the volume this crate's writer lays out, long name included. `fsck.fat -n` passes every image |
| a planted FAT12 decode defect | the oracle test fails on FAT12 and passes on the rest |
| fuzz target `file_allocation_table_boot_sector`, now walking the whole tree | 2.7 million executions in 121 seconds, no crash, seeded with `small-tree.img` |
| `cargo-deny`, the workspace graph | advisories, bans, licenses and sources all pass with `lamfat` in it |
| the new `script/lint` check | passes, and fails when `lamfat` moves to `[dependencies]` |

Kani bounds what it unrolls. The table harnesses and the slot harness have no loop, so they cover
every boot sector, cluster number, entry and slot. The walk harness covers volumes of one to eight
clusters over a table whose entries the solver picks fresh on every read, and the directory
harness covers up to three clusters of up to four sectors. Whole images are the fuzz target's.

## Names minted

Every new public name is provisional, with its reason, in
[the FAT names record](../0857-crates-released-on-their-own/file-allocation-table-names.md).
