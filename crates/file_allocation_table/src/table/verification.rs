//! **Machine-checked proofs (Kani) of the table's arithmetic**, over a boot sector, a cluster
//! number and an entry the solver chooses. Behind `#[cfg(kani)]`, so only `cargo kani` compiles
//! them.
//!
//! Both functions are straight-line code, so each harness covers every accepted boot sector of
//! 512 bytes and every value, not a bounded family. The markers are written as the
//! specification's literals rather than through the crate's match, so a wrong marker cannot prove
//! itself right.

use super::*;

/// **Every data cluster's entry lies inside the table read, and nothing else has an entry.** The
/// entry starts inside its sector, and an entry that runs past the sector's end is a FAT12 entry
/// whose second byte is still a sector of the same table. So [`crate::Volume::entry`] reads
/// only the table.
/// Falsification: replayable `crates/file_allocation_table/falsifications/table.verification.every_entry_lies_inside_the_table.patch`
#[kani::proof]
fn every_entry_lies_inside_the_table() {
    let sector: [u8; 512] = kani::any();
    let Ok(boot) = BootSector::parse(&sector) else {
        return;
    };
    let cluster: u32 = kani::any();
    let is_data = cluster >= 2 && u64::from(cluster) < u64::from(boot.clusters()) + 2;
    let Some((at, within)) = entry_position(&boot, cluster) else {
        assert!(!is_data);
        return;
    };
    assert!(is_data);
    let table = boot.active_table().unwrap_or(0);
    let first = u64::from(boot.table_first_sector(table).unwrap());
    let end = first + u64::from(boot.table_sectors());
    let size = boot.sector_size().bytes();
    assert!(u64::from(at) >= first && u64::from(at) < end);
    assert!(within < size);
    if within + entry_bytes(boot.fat_type()) > size {
        assert!(boot.fat_type() == FatType::Fat12);
        assert!(u64::from(at) + 1 < end);
    }
}

/// **An entry decodes by the specification's markers, for every type and every raw value.** Only
/// the type's width counts; `Next` names a cluster from 2 to one below the bad marker, and every
/// value above the bad marker ends the chain.
/// Falsification: replayable `crates/file_allocation_table/falsifications/table.verification.an_entry_decodes_by_its_markers.patch`
#[kani::proof]
fn an_entry_decodes_by_its_markers() {
    let raw: u32 = kani::any();
    let (fat_type, value, bad) = match kani::any::<u8>() % 3 {
        0 => (FatType::Fat12, raw & 0x0FFF, 0x0FF7),
        1 => (FatType::Fat16, raw & 0xFFFF, 0xFFF7),
        _ => (FatType::Fat32, raw & 0x0FFF_FFFF, 0x0FFF_FFF7),
    };
    let expected = if value == 0 {
        TableEntry::Free
    } else if value == 1 {
        TableEntry::Reserved
    } else if value < bad {
        TableEntry::Next(value)
    } else if value == bad {
        TableEntry::Bad
    } else {
        TableEntry::EndOfChain
    };
    assert_eq!(TableEntry::decode(fat_type, raw), expected);
}
