#![no_std]
//! **A FAT12, FAT16 and FAT32 filesystem, sans I/O, growing in stages.** Two halves exist today.
//!
//! The reader opens a volume through [`SectorDevice`], a trait the caller implements over any
//! device that reads whole sectors. [`BootSector::read`] reads and checks the boot sector and its
//! BPB, and types the volume by its cluster count as the specification does (see
//! `boot_sector.rs`). Every field is treated as hostile: a parse never panics and never reads out
//! of bounds, and what it accepts has its regions in order inside the volume.
//!
//! The writer is a `mkfs` for one shape: a FAT32 EFI system partition holding one boot file at
//! `\EFI\BOOT\<name>`, as pure computation (see [`VolumeLayout`]).
//! A name that is not 8.3 gets VFAT long-name entries ahead of a `~1` alias (see `long_name.rs`).
//! A volume below [`FAT32_MIN_CLUSTERS`] is refused, because the specification types a volume by
//! its data clusters, and the data area is rounded up to start on a cluster boundary.

//! # BUGS
//!
//! - A long name's alias is always `~1`, right only while the volume holds one file.
//! - Every timestamp is zero: a `mkfs` with no clock capability writes 1970 rather than inventing one.
//! - The `FSInfo` free-cluster count is written once, which the specification lets a driver tolerate.
//! - The reader stops at the boot sector: no table, directory or file is read yet.
//! - A boot sector is checked against itself, never against the device's length, which the
//!   [`SectorDevice`] trait does not report; a read past the end is the device's error to return.
//! - A FAT32 layout under 65525 clusters (which `mkfs.fat -F 32` makes with a warning) is refused,
//!   because Linux reads it as FAT32 and the specification as FAT16.
//! - Not crash-atomic and no journal, which is FAT.

mod boot_sector;
pub mod efi_system_partition;
mod long_name;
mod sector;
mod volume;

pub use boot_sector::{BootSector, FatType, ParseError, ReadError, RootDirectory};
pub use sector::{SectorDevice, SectorSize};
pub use volume::{
    CLUSTER_BYTES, CLUSTER_SECTORS, FAT32_MIN_CLUSTERS, LayoutError, RESERVED_SECTORS,
    ROOT_CLUSTER, SECTOR_BYTES, TABLE_COUNT, VolumeLayout,
};
