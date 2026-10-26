//! **The clusters of the one volume shape the writer lays out**: an EFI system partition whose
//! root holds `EFI`, which holds `BOOT`, which holds one boot file. [`crate::VolumeLayout`] places
//! each directory and the file's data at these clusters, in this order, after the root at
//! [`crate::ROOT_CLUSTER`].

/// The cluster holding the `EFI` directory.
pub const EFI_CLUSTER: u32 = 3;

/// The cluster holding the `EFI\BOOT` directory.
pub const BOOT_CLUSTER: u32 = 4;

/// The first cluster of the boot file's data.
pub const FILE_CLUSTER: u32 = 5;
