//! **Machine-checked proofs (Kani) of the directory reader**: every 32-byte slot parses as its
//! first byte and attributes say, and the sectors a directory read visits lie inside its region
//! or its clusters and run out. Behind `#[cfg(kani)]`, so only `cargo kani` compiles them.
//!
//! The slot parse is straight-line code, so its harness covers every slot. The sector harness
//! runs on a FAT12 volume of up to three clusters of up to four sectors, over the chain walk's
//! hostile table, the bound Kani needs to unroll it. The slots inside a sector are read at fixed
//! offsets below the sector's size; whole images are `script/fuzz`'s.

use super::*;
use crate::reader::verification::HostileTable;

/// **Every slot parses, as its first byte and its attributes say.** A first byte of 0 is the
/// end and `0xE5` a free slot; otherwise the six defined attribute bits equal to `0x0F` make a
/// long-name piece and anything else a short entry, whose first cluster on FAT12 and FAT16 fits
/// the low word.
/// Falsification: replayable `crates/file_allocation_table/falsifications/directory.verification.every_slot_parses_as_its_bytes_say.patch`
#[kani::proof]
fn every_slot_parses_as_its_bytes_say() {
    let slot: [u8; 32] = kani::any();
    let fat_type = match kani::any::<u8>() % 3 {
        0 => FatType::Fat12,
        1 => FatType::Fat16,
        _ => FatType::Fat32,
    };
    match Slot::parse(&slot, fat_type) {
        Slot::End => assert_eq!(slot[0], 0),
        Slot::Free => assert_eq!(slot[0], 0xE5),
        Slot::Used(entry) => {
            assert!(slot[0] != 0 && slot[0] != 0xE5);
            let long = slot[11] & 0x3F == 0x0F;
            match entry {
                DirectoryEntry::LongName(piece) => {
                    assert!(long);
                    assert_eq!(piece.checksum(), slot[13]);
                    assert_eq!(piece.units()[12], u16::from_le_bytes([slot[30], slot[31]]));
                }
                DirectoryEntry::Short(short) => {
                    assert!(!long);
                    assert_eq!(short.attributes(), slot[11]);
                    if fat_type != FatType::Fat32 {
                        assert!(short.first_cluster() <= 0xFFFF);
                    }
                }
            }
        }
    }
}

/// A FAT12 boot sector: one reserved sector, two one-sector tables, a one-sector root of 16
/// entries, then `clusters` clusters of `per_cluster` sectors.
fn fat12(clusters: u32, per_cluster: u8) -> BootSector {
    let total = 4 + clusters as u16 * u16::from(per_cluster);
    let mut boot = [0u8; 512];
    boot[0..3].copy_from_slice(&[0xEB, 0x3C, 0x90]);
    boot[11..13].copy_from_slice(&512u16.to_le_bytes());
    boot[13] = per_cluster;
    boot[14..16].copy_from_slice(&1u16.to_le_bytes());
    boot[16] = 2;
    boot[17..19].copy_from_slice(&16u16.to_le_bytes());
    boot[19..21].copy_from_slice(&total.to_le_bytes());
    boot[21] = 0xF8;
    boot[22..24].copy_from_slice(&1u16.to_le_bytes());
    boot[510] = 0x55;
    boot[511] = 0xAA;
    let Ok(boot) = BootSector::parse(&boot) else {
        panic!("the harness's boot sector is a valid FAT12 volume");
    };
    boot
}

/// **The sectors a directory read visits lie inside its region or its clusters, and run out.**
/// From the root region, or from any first cluster over any table contents and device failures,
/// each sector is one of the region's or one of a data cluster's, there are no more of them than
/// the region or all the clusters hold, and the sequence has ended by the step after that.
/// Falsification: replayable `crates/file_allocation_table/falsifications/directory.verification.a_directory_reads_only_its_own_sectors.patch`
#[kani::proof]
#[kani::unwind(15)]
fn a_directory_reads_only_its_own_sectors() {
    let clusters: u32 = kani::any();
    kani::assume((1..=3).contains(&clusters));
    let per_cluster: u8 = match kani::any::<u8>() % 3 {
        0 => 1,
        1 => 2,
        _ => 4,
    };
    let boot = fat12(clusters, per_cluster);
    assert_eq!(boot.clusters(), clusters);
    let mut table = HostileTable { clusters };
    let root = kani::any::<bool>();
    let mut source = if root {
        Source::root(&boot)
    } else {
        Source::chain(kani::any())
    };
    let first_data = boot.first_data_sector();
    let data_sectors = clusters * u32::from(per_cluster);
    let most = if root { 1 } else { data_sectors };
    let mut visited = 0;
    let mut ended = false;
    for _ in 0..most + 2 {
        match source.next_sector(&mut table, &boot) {
            Some(Ok(sector)) => {
                if root {
                    assert_eq!(sector, 3);
                } else {
                    assert!(sector >= first_data && sector < first_data + data_sectors);
                }
                visited += 1;
            }
            Some(Err(_)) | None => {
                ended = true;
                break;
            }
        }
    }
    assert!(ended);
    assert!(visited <= most);
}
