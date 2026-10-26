# The FAT32 writer

*Name: provisional, minted 2026-10-24 (UTC) by comments/929-crates-16, for the crate it describes.*

`crates/file_allocation_table` lays out one FAT32 volume holding one boot file, as arithmetic with
no I/O. Why the installer needs it, the cluster count that types the volume, and the alignment bug
are in [installing.md](installing.md), section 3. This page holds the volume's shape, the long-name
format, and who reads the result back.

## The shape

The shape is the most boring FAT32 a firmware could be handed, because the reader is somebody
else's driver.

| | |
|---|---|
| Sector size | 512 (`SECTOR_BYTES`); FAT32 permits others and nothing here has met one |
| Sectors per cluster | 8, so a 4096-byte cluster, which is also `filesystem_contract::blk`'s transfer unit |
| Reserved sectors | 32, the usual FAT32 figure: boot sector, `FSInfo`, and the backup pair at 6 and 7 |
| FATs | 2, because every tool writes two and a firmware that reads the second finds it |
| Cluster 2 | the root directory: a volume label and `EFI` |
| Cluster 3 | `EFI`: the dot entries and `BOOT` |
| Cluster 4 | `BOOT`: the dot entries and the boot file, with its long-name run if it has one |
| Cluster 5 onward | the boot file's data, contiguous |

A file name that is 8.3 is written as a short entry alone, the form every FAT driver reads. Any
other name gets VFAT long-name entries ahead of a short alias, so `BOOTRISCV64.EFI` is written as
itself.

The alias always ends in `~1`. That is right only because the volume is created holding one file,
so no other alias can collide with it. A crate that writes a second file has to number aliases
against the directory it writes into.

## Long names

Microsoft's "FAT: General Overview of On-Disk Format" (version 1.03), section 7, is the format. A
long name is up to 255 UCS-2 units, spread thirteen to an entry over up to twenty entries. The run
is written in reverse. The entry holding the name's last units comes first on disk, its ordinal
carrying `LAST_LONG_ENTRY`, and ordinal 1 sits immediately before the short entry. After the
name's last unit comes one `0x0000`, then `0xFFFF` to the end of the entry, unless the name fills
its last entry exactly.

The checksum is the part that fails silently. A driver that finds a run whose checksum does not
match the short entry after it ignores the run and shows the alias. A wrong checksum is therefore
a volume that mounts, lists `BOOTRI~1.EFI`, and boots nothing. The crate's `verification.rs`
proves the checksum and the run's layout for every name.

## Who reads it back

A writer tested against its own reader proves almost nothing, and this crate has no reader at all.
What proves it is somebody else's FAT driver reading what it wrote. Three do:

- OVMF, in `cargo xtask install-boot`.
- EDK2 on riscv64, reading a long name, in `cargo xtask stick-boot`.
- The host's own driver, in `tests/a_third_party_driver_reads_it.rs`. It is the one a person can
  run in seconds while changing the layout.

The host test writes `\EFI\BOOT\BOOTRISCV64.EFI` because a wrong checksum or a malformed run fails
silently. Only a reader that lists the directory can tell the long name from the alias. That test
is `#[ignore]`d, since it needs a host filesystem driver and attaches a disk image.
