//! **The clusters of the one volume shape the writer lays out**: an EFI system partition (ESP, the
//! FAT volume UEFI 2.10 section 13.3.1, System Partition, boots from) whose root holds `EFI`, which
//! holds `BOOT`, which holds one boot file. [`crate::VolumeLayout`] places
//! each directory and the file's data at these clusters, in this order, after the root at
//! [`crate::ROOT_CLUSTER`].
//!
//! Name: ratified 2026-10-25 (#2720). The writer's one volume shape, which holds the three clusters
//! below.

/// **The `EFI` directory's cluster** (`\EFI`, the directory UEFI 2.10 section 13.3.1.3, Directory
/// Structure, roots every boot path at).
///
/// Name: ratified 2026-10-25 (#2720).
#[doc(alias = "extensible firmware interface directory")]
pub const EFI_CLUSTER: u32 = 3;

/// The cluster holding the `EFI\BOOT` directory.
///
/// Name: ratified 2026-10-25 (#2720).
pub const BOOT_CLUSTER: u32 = 4;

/// The first cluster of the boot file's data.
///
/// Name: ratified 2026-10-25 (#2720).
pub const FILE_CLUSTER: u32 = 5;
