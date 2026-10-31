//! **Machine-checked proof (Kani) that a format reads back as itself**: for any device size, type
//! and cluster size, a geometry the format computes is one the reader accepts, with the same type,
//! cluster count, table size and data start. Behind `#[cfg(kani)]`, so only `cargo kani` compiles
//! it.
//!
//! The sector size is 512 bytes, the one the boot-sector parse proofs also fix; the format's
//! arithmetic takes the sector size as a number, and the oracle tests cross 1024 and 4096.
//!
//! Name: provisional.

use super::*;
use crate::boot_sector::BootSector;

/// **A computed geometry parses back to itself.** For every device size, type and cluster size
/// from 1 to 128 sectors: when [`Geometry::new`] accepts, the boot sector it writes parses, and the
/// parse reports the same type, cluster count, table size, cluster size and data start, with the
/// data area on a cluster boundary and every cluster inside the device.
/// Falsification: replayable `crates/file_allocation_table/falsifications/format.verification.a_geometry_parses_back_to_itself.patch`
#[kani::proof]
#[kani::unwind(130)]
fn a_geometry_parses_back_to_itself() {
    let total: u32 = kani::any();
    let kind = match kani::any::<u8>() % 3 {
        0 => FileAllocationTableType::Fat12,
        1 => FileAllocationTableType::Fat16,
        _ => FileAllocationTableType::Fat32,
    };
    let cluster_sectors: u32 = match kani::any::<u8>() % 8 {
        0 => 1,
        1 => 2,
        2 => 4,
        3 => 8,
        4 => 16,
        5 => 32,
        6 => 64,
        _ => 128,
    };
    let Ok(geometry) = Geometry::new(kind, SectorSize::Bytes512, cluster_sectors, total) else {
        return;
    };
    let mut sector = [0u8; 512];
    geometry.write_boot_sector(kani::any(), &[b' '; 11], kani::any(), &mut sector);
    let parsed = BootSector::parse(&sector).expect("the reader accepts what the format wrote");
    assert_eq!(parsed.file_allocation_table_type(), kind);
    assert_eq!(parsed.clusters(), geometry.clusters);
    assert_eq!(parsed.table_sectors(), geometry.table_sectors);
    assert_eq!(parsed.sectors_per_cluster(), cluster_sectors);
    assert_eq!(parsed.first_data_sector(), geometry.first_data_sector());
    assert_eq!(geometry.first_data_sector() % cluster_sectors, 0);
    let end = u64::from(geometry.first_data_sector())
        + u64::from(geometry.clusters) * u64::from(cluster_sectors);
    assert!(end <= u64::from(total));
}
