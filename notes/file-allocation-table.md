# The FAT32 writer

*Name: provisional, minted 2026-10-24 (UTC) by comments/929-crates-16, for the crate it describes.*

`crates/file_allocation_table` reads FAT12, FAT16 and FAT32 volumes through `SectorDevice`, and its
writer, `VolumeLayout`, lays out one FAT32 volume holding one boot file. Neither half does any I/O.
The crate's `lib.rs` and `README.md` describe both halves, and milestone 813 (a proven FAT crate,
released on its own) plans the rest of the crate. Why the installer needs the writer, the cluster
count that types the volume, and the alignment bug are in [installing.md](installing.md), section
3. This page holds what the crate cannot say about itself: why the writer's volume has this shape,
and which drivers outside this tree read it back.

## The shape

The shape is the most boring FAT32 a firmware could be handed, because the reader is somebody
else's driver. Each number is a constant in `volume.rs` whose doc gives its reason
(`SECTOR_BYTES`, `CLUSTER_SECTORS`, `RESERVED_SECTORS`, `TABLE_COUNT`, `ROOT_CLUSTER`), and
`efi_system_partition` places `EFI`, `EFI\BOOT` and the boot file's data at clusters 3, 4 and 5
onward, after the root at cluster 2.

A file name that is 8.3 is written as a short entry alone, the form every FAT driver reads. Any
other name gets VFAT long-name entries ahead of a short alias, so `BOOTRISCV64.EFI` is written as
itself.

The alias always ends in `~1`. That is right only because the volume is created holding one file,
so no other alias can collide with it. A crate that writes a second file has to number aliases
against the directory it writes into, which is milestone 813's stage 7.

## Long names

`long_name.rs`'s module doc gives the format: Microsoft's "FAT: General Overview of On-Disk Format"
(version 1.03), section 7, the run's order and its padding. Its `verification.rs` proves the
checksum and the run's layout for every name.

The checksum is the part that fails silently. A driver that finds a run whose checksum does not
match the short entry after it ignores the run and shows the alias. A wrong checksum is therefore
a volume that mounts, lists `BOOTRI~1.EFI`, and boots nothing.

## Who reads it back

A writer tested against its own reader proves almost nothing, and since milestone 813's stage 2
this crate has a reader. What proves the writer is somebody else's FAT driver reading what it
wrote. Four do:

- OVMF, in `cargo xtask install-boot`.
- EDK2 on riscv64, reading a long name, in `cargo xtask stick-boot`.
- rust-fatfs, in `tests/the_oracle_agrees.rs`'s `rust_fatfs_reads_what_this_crate_wrote`, which
  also runs `fsck.fat` or `fsck_msdos` where the host has one. It runs in every `cargo test`.
- The host's own driver, in `tests/a_third_party_driver_reads_it.rs`. It is `#[ignore]`d, since it
  needs a host filesystem driver and attaches a disk image.

Each of the last three writes `\EFI\BOOT\BOOTRISCV64.EFI`, because a wrong checksum or a malformed
run fails silently. Only a reader that lists the directory can tell the long name from the alias.
