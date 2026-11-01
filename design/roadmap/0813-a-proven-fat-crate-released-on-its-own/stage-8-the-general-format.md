# Stage 8: the general format and the writer's fuzz target

Appendix to [813. `file_allocation_table`](../0813-a-proven-fat-crate-released-on-its-own.md). It
records what stage 8 built and the evidence. The writer's fuzz target, which the block lists among
the properties, lands here.

## What stage 8 built

- `FormatPlan` lays out an empty FAT12, FAT16 or FAT32 volume of any size at any of the four sector
  sizes, as sectors a caller writes (`fill_sector`, `write_to`). `FormatOptions` takes the
  device's size and sector size, and optionally the type, the cluster size (a power of two from 1
  to 128 sectors), the serial number, a label and the hidden sectors. With no type and no cluster
  size the smallest cluster that gives a legal count wins and the type follows the count, as the
  specification has it. `FormatError` says why none exists: too small, no cluster size gives the
  type a legal count, a label or a cluster size refused.
- One piece of arithmetic sizes every volume: Microsoft's own sizing, which over-sizes the table
  slightly, then the data area rounded to start on a cluster boundary. FAT32 grows its tables to
  get there; FAT12 and FAT16, whose start can be odd, pad their reserved sectors.
- The one-file EFI volume (`VolumeLayout`, the installer's call) takes its numbers from the same
  sizing and writes its boot sector and `FSInfo` through the same builders, so the BPB is written in
  one place. Its public API and every byte it writes are as before. The `RedoxFS` roles of the
  installer and `packages/base-image.toml` are not touched.
- A FAT32 format writes the boot sector and `FSInfo` with their backups, both tables whole, and the
  root cluster. A FAT12 or FAT16 format writes the reserved sector, both tables and the fixed root.
  A label also becomes the root's volume-label entry. The data clusters are not touched.
- `fuzz/fuzz_targets/file_allocation_table_writer.rs`, the writer's fuzz target, which lands in this
  stage: decoded operations on a hostile image or on a volume this crate formats.

## The evidence

| check | result |
|---|---|
| Kani | `a_geometry_parses_back_to_itself` covers every device size, type and cluster size at 512-byte sectors: when the format accepts, the boot sector it writes parses to the same type, clusters, table size, cluster size and data start, with the data area on a cluster boundary and every cluster inside the device. Its first run found a real fault: Microsoft's table sizing falls two entries short when the volume divides it exactly (FAT16 with one-sector clusters at 8,289 sectors), so the format grows the table by a sector then, and a unit test sweeps every size to pin it |
| unit tests | the type follows the count from the smallest cluster, the FAT32 numbers equal the EFI volume's, alignment for every type and cluster size, the 16-bit and 32-bit sector totals at 65,535 and 65,536, `FSInfo` and its backups, labels, and a device that fails |
| oracle tests | nine formats across FAT12, FAT16 and FAT32 at 512, 1024 and 4096-byte sectors and several cluster sizes each parse, mount in rust-fatfs with the cluster count this crate computed, pass `fsck.fat -n`, take a directory and a file from this crate's writer, and read back in rust-fatfs. Where `mkfs.fat` is installed, three FAT12 and FAT16 volumes it made (one-sector, four-sector and 32-sector clusters) take this crate's writer, and this crate's cluster count for the same size is within eight of `mkfs.fat`'s |
| fuzz | 143,000 runs in 90 seconds of the writer target, no crash |

## Names minted

Ratified 2026-11-01 (UTC): `FormatPlan`, `FormatOptions` (`with_file_allocation_table_type`, `with_sectors_per_cluster`, `with_volume_id`,
`with_hidden_sectors`, `with_label`), `FormatError`, and the module `format`.
