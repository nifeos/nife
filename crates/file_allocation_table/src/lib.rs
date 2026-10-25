#![no_std]
//! **Enough of FAT32 to create an EFI system partition holding one boot file**, as pure
//! computation: an installer must put `\EFI\BOOT\BOOTX64.EFI` on a FAT volume, and nothing on the
//! target could write one. **This is a `mkfs`, not a filesystem**: one volume, one shape, one file
//! at one path, then finished; reading is planned and not here yet. No I/O: it says what each
//! sector must contain and the caller writes it. A name that is not 8.3 gets VFAT long-name entries
//! ahead of a `~1` alias (see `long_name.rs`).
//! A volume below [`MIN_CLUSTERS`] is refused, because the specification types a volume by its
//! data clusters, and the data area is rounded up to start on a cluster boundary.

//! # BUGS
//!
//! - A long name's alias is always `~1`, right only while the volume holds one file.
//! - Every timestamp is zero: a `mkfs` with no clock capability writes 1970 rather than inventing one.
//! - The `FSInfo` free-cluster count is written once, which the specification lets a driver tolerate.
//! - Nothing here reads back what it writes; other drivers do (the host's, in
//!   `tests/a_third_party_driver_reads_it.rs`, and UEFI firmware).
//! - Not crash-atomic and no journal, which is FAT.

mod long_name;
mod volume;

pub use volume::{
    BOOT_CLUSTER, CLUSTER, CLUSTER_SECTORS, EFI_CLUSTER, Error, FATS, FILE_CLUSTER, MIN_CLUSTERS,
    RESERVED_SECTORS, ROOT_CLUSTER, SECTOR, Volume,
};
