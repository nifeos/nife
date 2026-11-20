//! **Where on a card a write cannot hurt: the MBR, and the gap before its first partition.** A
//! master boot record (MBR) is sector 0 of a disk, with four primary partition entries and a
//! two-byte signature. This crate parses it and, from it, names the sectors that belong to no
//! partition. The `VisionFive` 2 boots from its microSD card, one `FAT32` partition in an MBR, so the
//! only sectors a caller names for a write are before the first partition and after sector 0: the
//! alignment gap no filesystem owns and U-Boot does not read when the board boots from its `QSPI`
//! flash, as the `VisionFive` 2 does. A card with no MBR signature, a protective MBR (`GPT`, whose
//! header and table live in that gap), no partitions, or a first partition at sector 1 gets no
//! scratch range, and a caller that wants a scratch range gets none and must refuse to write. A
//! Kani harness (`verification`) proves the range never touches a partition or sector 0, for every
//! table.
//!
//! The [`protective`] module writes and checks the protective MBR a `GPT` disk carries: one record
//! of type `0xEE` that covers the disk.
//!
//! Pure computation: no I/O, no allocation, no `unsafe`, no dependency.
//!
//! Name: provisional, minted 2026-11-19 (UTC) for the module's move out of
//! `designware_mobile_storage`; a name review follows.

#![no_std]
#![forbid(unsafe_code)]

/// The MBR's last two bytes on a valid table.
pub(crate) const MASTER_BOOT_RECORD_SIGNATURE: [u8; 2] = [0x55, 0xaa];

pub mod protective;
mod record;

pub use record::{MasterBootRecord, PartitionEntry};

#[cfg(kani)]
mod verification;
