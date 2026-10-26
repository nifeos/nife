//! **The clusters of the one volume shape the writer lays out**: an EFI system partition whose
//! root holds `EFI`, which holds `BOOT`, which holds one boot file. [`crate::VolumeLayout`] places
//! each directory and the file's data at these clusters, in this order, after the root at
//! [`crate::ROOT_CLUSTER`].
//!
//! Name: ratified 2026-10-25 (#2720). The writer's one volume shape, which holds the three clusters
//! below.

/// The cluster holding the `EFI` directory.
///
/// Name: ratified 2026-10-25 (#2720).
pub const EFI_CLUSTER: u32 = 3;

/// The cluster holding the `EFI\BOOT` directory.
///
/// Name: ratified 2026-10-25 (#2720).
pub const BOOT_CLUSTER: u32 = 4;

/// The first cluster of the boot file's data.
///
/// Name: ratified 2026-10-25 (#2720).
pub const FILE_CLUSTER: u32 = 5;
