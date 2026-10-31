//! **Machine-checked proofs (Kani) of the table's arithmetic**, over a boot sector, a cluster
//! number and an entry the solver chooses. Behind `#[cfg(kani)]`, so only `cargo kani` compiles
//! them.
//!
//! Both functions are straight-line code, so each harness covers every accepted boot sector of
//! 512 bytes and every value, not a bounded family. The markers are written as the
//! specification's literals rather than through the crate's match, so a wrong marker cannot prove
//! itself right.
//!
//! Name: ratified 2026-10-26 (UTC), on #2760. The Kani proofs, named as `boot_sector` and
//! `long_name` name theirs.

use super::*;

/// **Every data cluster's entry lies inside the table read, and nothing else has an entry.** The
/// entry starts inside its sector, and an entry that runs past the sector's end is a FAT12 entry
/// whose second byte is still a sector of the same table. So [`crate::Volume::table_entry`] reads
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
    if within + entry_bytes(boot.file_allocation_table_type()) > size {
        assert!(boot.file_allocation_table_type() == FileAllocationTableType::Fat12);
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
    let (file_allocation_table_type, value, bad) = match kani::any::<u8>() % 3 {
        0 => (FileAllocationTableType::Fat12, raw & 0x0FFF, 0x0FF7),
        1 => (FileAllocationTableType::Fat16, raw & 0xFFFF, 0xFFF7),
        _ => (
            FileAllocationTableType::Fat32,
            raw & 0x0FFF_FFFF,
            0x0FFF_FFF7,
        ),
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
    assert_eq!(
        TableEntry::decode(file_allocation_table_type, raw),
        expected
    );
}

/// The type under test and its mask and bad-cluster marker, chosen by the solver.
fn any_type() -> (FileAllocationTableType, u32, u32) {
    match kani::any::<u8>() % 3 {
        0 => (FileAllocationTableType::Fat12, 0x0FFF, 0x0FF7),
        1 => (FileAllocationTableType::Fat16, 0xFFFF, 0xFFF7),
        _ => (FileAllocationTableType::Fat32, 0x0FFF_FFFF, 0x0FFF_FFF7),
    }
}

/// **A stored entry reads back, and nothing else in its bytes changes.** For every type, cluster,
/// window of table bytes and value: [`assemble`] over the stored window returns the value's low
/// bits; a FAT12 entry leaves its neighbor's nibble alone, a FAT32 entry leaves its reserved top
/// four bits alone, and the bytes past the entry's width are untouched.
/// Falsification: replayable `crates/file_allocation_table/falsifications/table.verification.a_stored_entry_reads_back_and_disturbs_nothing_else.patch`
#[kani::proof]
fn a_stored_entry_reads_back_and_disturbs_nothing_else() {
    let (file_allocation_table_type, mask, _) = any_type();
    let cluster: u32 = kani::any();
    let before: [u8; 4] = kani::any();
    let value: u32 = kani::any();
    let mut after = before;
    store(file_allocation_table_type, cluster, &mut after, value);
    // `decode` drops FAT32's reserved top bits, so the comparison drops them too.
    assert_eq!(
        assemble(file_allocation_table_type, cluster, after) & mask,
        value & mask
    );
    match file_allocation_table_type {
        FileAllocationTableType::Fat12 => {
            assert_eq!(after[2..], before[2..]);
            if cluster & 1 == 1 {
                assert_eq!(after[0] & 0x0F, before[0] & 0x0F);
                assert_eq!(after[1], (value >> 4) as u8);
            } else {
                assert_eq!(after[0], value as u8);
                assert_eq!(after[1] & 0xF0, before[1] & 0xF0);
            }
        }
        FileAllocationTableType::Fat16 => assert_eq!(after[2..], before[2..]),
        FileAllocationTableType::Fat32 => assert_eq!(after[3] & 0xF0, before[3] & 0xF0),
    }
}

/// **Encoding inverts decoding.** For every type, every kind of entry and every cluster number a
/// chain may name, `decode(encode(entry))` is the entry, and `encode` of a free, reserved, bad or
/// end-of-chain entry is the specification's literal.
/// Falsification: replayable `crates/file_allocation_table/falsifications/table.verification.encoding_inverts_decoding.patch`
#[kani::proof]
fn encoding_inverts_decoding() {
    let (file_allocation_table_type, mask, bad) = any_type();
    let next: u32 = kani::any();
    kani::assume(next >= 2 && next < bad);
    let entry = match kani::any::<u8>() % 5 {
        0 => TableEntry::Free,
        1 => TableEntry::Reserved,
        2 => TableEntry::Next(next),
        3 => TableEntry::Bad,
        _ => TableEntry::EndOfChain,
    };
    let raw = entry.encode(file_allocation_table_type);
    assert_eq!(TableEntry::decode(file_allocation_table_type, raw), entry);
    match entry {
        TableEntry::Free => assert_eq!(raw, 0),
        TableEntry::Reserved => assert_eq!(raw, 1),
        TableEntry::Next(n) => assert_eq!(raw, n),
        TableEntry::Bad => assert_eq!(raw, bad),
        TableEntry::EndOfChain => assert_eq!(raw, mask),
    }
}
