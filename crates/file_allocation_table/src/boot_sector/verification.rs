//! **Machine-checked proofs (Kani) of the boot-sector parse**, each over a sector of 512 bytes
//! the solver chooses. Behind `#[cfg(kani)]`, so only `cargo kani` compiles them.
//!
//! The parse is straight-line code with no loop, so every harness covers every sector, not a
//! bounded family of them. The thresholds below are written as the specification's literals
//! rather than the crate's constants, so a wrong constant cannot prove itself right.
//!
//! Name: provisional.

use super::*;

/// **No sector panics the parse, and what it accepts has its regions in order inside the
/// volume.** The reserved sectors, the tables, the root region and the data clusters follow one
/// another without overlap and end at or before the last sector, and at least one cluster exists.
/// Falsification: replayable `crates/file_allocation_table/falsifications/boot_sector.verification.what_the_parse_accepts_lies_inside_the_volume.patch`
#[kani::proof]
fn what_the_parse_accepts_lies_inside_the_volume() {
    let sector: [u8; 512] = kani::any();
    let Ok(boot) = BootSector::parse(&sector) else {
        return;
    };
    let total = u64::from(boot.total_sectors());
    let tables_end = u64::from(boot.reserved_sectors())
        + u64::from(boot.tables()) * u64::from(boot.table_sectors());
    assert!(boot.reserved_sectors() >= 1 && boot.tables() >= 1 && boot.table_sectors() >= 1);
    let data_start = match boot.root_directory() {
        RootDirectory::Region {
            first_sector,
            sectors,
            ..
        } => {
            assert_eq!(u64::from(first_sector), tables_end);
            u64::from(first_sector) + u64::from(sectors)
        }
        RootDirectory::Cluster(_) => tables_end,
    };
    assert_eq!(u64::from(boot.first_data_sector()), data_start);
    assert!(boot.clusters() >= 1);
    let data_end = data_start + u64::from(boot.clusters()) * u64::from(boot.sectors_per_cluster());
    assert!(data_end <= total);
}

/// **The FAT type is the cluster count's, by the specification's two thresholds**, and the BPB
/// layout agrees with it: FAT32 has a cluster root and FAT12 and FAT16 a fixed region.
/// Falsification: replayable `crates/file_allocation_table/falsifications/boot_sector.verification.the_fat_type_is_the_cluster_counts.patch`
#[kani::proof]
fn the_fat_type_is_the_cluster_counts() {
    let sector: [u8; 512] = kani::any();
    let Ok(boot) = BootSector::parse(&sector) else {
        return;
    };
    let clusters = boot.clusters();
    match boot.fat_type() {
        FatType::Fat12 => assert!(clusters < 4085),
        FatType::Fat16 => assert!((4085..65525).contains(&clusters)),
        FatType::Fat32 => assert!((65525..=0x0FFF_FFF4).contains(&clusters)),
    }
    let cluster_root = matches!(boot.root_directory(), RootDirectory::Cluster(_));
    assert_eq!(cluster_root, boot.fat_type() == FatType::Fat32);
}

/// **One table holds an entry for every cluster**, the two reserved entries included, at the
/// width the FAT type gives it. A chain walk indexes the table by cluster number, so this is what
/// keeps every such index inside the table.
/// Falsification: replayable `crates/file_allocation_table/falsifications/boot_sector.verification.the_table_has_an_entry_for_every_cluster.patch`
#[kani::proof]
fn the_table_has_an_entry_for_every_cluster() {
    let sector: [u8; 512] = kani::any();
    let Ok(boot) = BootSector::parse(&sector) else {
        return;
    };
    let bits = match boot.fat_type() {
        FatType::Fat12 => 12u64,
        FatType::Fat16 => 16,
        FatType::Fat32 => 32,
    };
    let held = u64::from(boot.table_sectors()) * boot.sector_size().bytes() as u64 * 8;
    assert!((u64::from(boot.clusters()) + 2) * bits <= held);
}

/// **Every data cluster maps to sectors inside the data region, and no other number maps at
/// all.** For any accepted sector and any cluster number, [`BootSector::cluster_first_sector`]
/// answers exactly for 2 to `clusters + 1`, and the whole cluster it names ends at or before the
/// volume's last sector.
/// Falsification: replayable `crates/file_allocation_table/falsifications/boot_sector.verification.only_a_data_cluster_has_sectors.patch`
#[kani::proof]
fn only_a_data_cluster_has_sectors() {
    let sector: [u8; 512] = kani::any();
    let Ok(boot) = BootSector::parse(&sector) else {
        return;
    };
    let cluster: u32 = kani::any();
    let is_data = cluster >= 2 && u64::from(cluster) < u64::from(boot.clusters()) + 2;
    match boot.cluster_first_sector(cluster) {
        Some(first) => {
            assert!(is_data);
            assert!(first >= boot.first_data_sector());
            let end = u64::from(first) + u64::from(boot.sectors_per_cluster());
            assert!(end <= u64::from(boot.total_sectors()));
        }
        None => assert!(!is_data),
    }
}
