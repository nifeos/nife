//! **The one volume shape [`crate::VolumeLayout`] lays out**: an EFI (Extensible Firmware
//! Interface) system partition (ESP, the FAT volume UEFI 2.10 section 13.3.1, System Partition,
//! boots from) of 512-byte sectors and 4096-byte clusters, whose root holds `EFI`, which holds
//! `BOOT`, which holds one boot file.
//!
//! The constants here describe that shape alone. The reader, [`crate::FormatPlan`] and
//! [`crate::WritableVolume`] take any sector size from 512 to 4096 bytes and any cluster size the
//! specification allows; [`crate::SectorSize`] is their unit. [`crate::VolumeLayout`] places the
//! root at [`ROOT_CLUSTER`] and each directory and the file's data at the three clusters after it,
//! in this order.
//!
//! Name: ratified 2026-10-25 (#2720). The writer's one volume shape.

/// A sector of this shape, and the unit [`crate::VolumeLayout`] measures every output in.
///
/// FAT32 permits 512, 1024, 2048 and 4096; this writer writes 512, which is what nearly every disk
/// reports and the unit a GUID partition table on such a disk counts in.
///
/// Name: ratified 2026-10-25 (#2720). Refused `SECTOR` (it reads as a sector and holds a size).
pub const SECTOR_BYTES: usize = 512;

/// Sectors per cluster, so 4096-byte clusters: a caller moving one 4096-byte block per request
/// moves exactly one cluster.
///
/// Name: ratified 2026-10-25 (#2720); renamed from `CLUSTER_SECTORS` on 2026-11-06 (UTC), the
/// count named by what it counts per what.
pub const SECTORS_PER_CLUSTER: u32 = 8;

/// Bytes per cluster.
///
/// Name: ratified 2026-10-25 (#2720). Refused `CLUSTER` (it reads as a cluster and holds a size).
pub const CLUSTER_BYTES: usize = SECTORS_PER_CLUSTER as usize * SECTOR_BYTES;

/// Reserved sectors ahead of the first FAT: the boot sector, the file system information sector,
/// and the backup pair the specification puts at sectors 6 and 7, with room left over as every tool
/// leaves it.
///
/// Name: ratified 2026-10-25 (#2720).
pub const RESERVED_SECTORS: u32 = 32;

/// How many copies of the table are written: two, because every tool writes two and a firmware that
/// reads the second finds it.
///
/// Name: ratified 2026-10-25 (#2720). Refused `FATS` (the acronym in a crate that spells it out).
pub const TABLE_COUNT: u32 = 2;

/// The first cluster of the root directory. FAT32 fixes nothing here, but 2 is the first data
/// cluster and every tool uses it.
///
/// Name: ratified 2026-10-25 (#2720).
pub const ROOT_CLUSTER: u32 = 2;

/// **The count of data clusters below which a volume is not FAT32 at all**, and so the smallest
/// volume [`crate::VolumeLayout::new`] lays out.
///
/// Microsoft's specification defines the FAT type by this number and nothing else, and drivers in
/// the field follow it. A "FAT32" volume with fewer clusters than this is read as FAT16 by the code
/// that matters, which is the firmware's, so this crate refuses to build one. With this shape's
/// 4096-byte clusters it is about 256 MiB.
///
/// Name: ratified 2026-11-05 (UTC), renamed from `FAT32_MINIMUM_CLUSTERS`: minimum first, then what
/// it is the minimum of. Refused `MIN_CLUSTERS` (it is
/// FAT32's floor alone) and any abbreviated `MIN`.
pub const MINIMUM_FAT32_CLUSTERS: u32 = 65_525;

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
