#![no_std]
//! **Enough of FAT32 to create an EFI system partition holding one boot file**, as pure
//! computation: an installer must put `\EFI\BOOT\BOOTX64.EFI` on a FAT volume, and nothing on the
//! target could write one (notes/installing.md). **This is a `mkfs`, not a filesystem**: one volume,
//! one shape, one file at one path, then finished; reading is milestone 140 (mount a drive this
//! system did not create). No I/O: it says what each sector must contain and the caller writes it.
//! The shape is the most boring FAT32 a firmware could be handed, because the reader is somebody
//! else's driver.
//!
//! | | |
//! |---|---|
//! | Sector size | 512 ([`SECTOR`]); FAT32 permits others and nothing here has met one |
//! | Sectors per cluster | 8, so a 4096-byte cluster, which is also `filesystem_contract::blk`'s transfer unit |
//! | Reserved sectors | 32, the usual FAT32 figure: boot sector, `FSInfo`, and the backup pair at 6 and 7 |
//! | FATs | 2, because every tool writes two and a firmware that reads the second finds it |
//! | Cluster 2 | the root directory: a volume label and `EFI` |
//! | Cluster 3 | `EFI`: the dot entries and `BOOT` |
//! | Cluster 4 | `BOOT`: the dot entries and the boot file, with its long-name run if it has one |
//! | Cluster 5 onward | the boot file's data, contiguous |
//!
//! **A file name that is not 8.3 gets VFAT long-name entries** ahead of a `~1` short alias, so
//! `BOOTRISCV64.EFI` is written as itself (see `long_name.rs`). A name that is 8.3 is written as a
//! short entry alone, the form every FAT driver reads.
//!
//! **The cluster count is load-bearing**: Microsoft's specification types a volume by its data
//! clusters, so under 65525 it *is* FAT16 whatever the boot sector says, and a volume below
//! [`MIN_CLUSTERS`] is refused rather than handed to firmware as a lie.
//!
//! # BUGS
//!
//! - The short alias of a long name is always `~1`, which is right only because the volume holds
//!   one file. A crate that writes a second file has to number aliases against the directory.
//! - Every timestamp is zero: a `mkfs` with no clock capability writes 1970 rather than inventing one.
//! - The `FSInfo` free-cluster count is written once, which the specification lets a driver tolerate.
//! - Nothing here reads back what it writes. Other drivers prove it: OVMF in
//!   `cargo xtask install-boot`, EDK2 riscv64 reading a long name in `cargo xtask stick-boot`, and
//!   the host's in `tests/a_third_party_driver_reads_it.rs`.
//! - Not crash-atomic and no journal, which is FAT.
//!
//! Name: ratified 2026-10-07 (calef, #1806): FAT spelled out as the tree's acronym rule asks;
//! permanent once published.

mod long_name;
mod volume;

pub use volume::{
    BOOT_CLUSTER, CLUSTER, CLUSTER_SECTORS, EFI_CLUSTER, Error, FATS, FILE_CLUSTER, MIN_CLUSTERS,
    RESERVED_SECTORS, ROOT_CLUSTER, SECTOR, Volume,
};
