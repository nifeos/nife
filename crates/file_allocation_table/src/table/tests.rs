//! The markers of each FAT type at their boundaries, and where entries lie in the table.
//!
//! Name: ratified 2026-10-26 (UTC), on #2760.

use super::*;
use crate::boot_sector::tests::fat32_with_active_table;

#[test]
fn each_types_markers_decode_at_their_boundaries() {
    for (file_allocation_table_type, bad) in [
        (FileAllocationTableType::Fat12, 0xFF7u32),
        (FileAllocationTableType::Fat16, 0xFFF7),
        (FileAllocationTableType::Fat32, 0x0FFF_FFF7),
    ] {
        assert_eq!(
            TableEntry::decode(file_allocation_table_type, 0),
            TableEntry::Free
        );
        assert_eq!(
            TableEntry::decode(file_allocation_table_type, 1),
            TableEntry::Reserved
        );
        assert_eq!(
            TableEntry::decode(file_allocation_table_type, 2),
            TableEntry::Next(2)
        );
        assert_eq!(
            TableEntry::decode(file_allocation_table_type, bad - 1),
            TableEntry::Next(bad - 1)
        );
        assert_eq!(
            TableEntry::decode(file_allocation_table_type, bad),
            TableEntry::Bad
        );
        assert_eq!(
            TableEntry::decode(file_allocation_table_type, bad + 1),
            TableEntry::EndOfChain
        );
        assert_eq!(
            TableEntry::decode(file_allocation_table_type, bad + 8),
            TableEntry::EndOfChain
        );
    }
}

#[test]
fn bits_above_the_width_are_ignored() {
    assert_eq!(
        TableEntry::decode(FileAllocationTableType::Fat32, 0xF000_0002),
        TableEntry::Next(2)
    );
    assert_eq!(
        TableEntry::decode(FileAllocationTableType::Fat32, 0x1FFF_FFF8),
        TableEntry::EndOfChain
    );
    assert_eq!(
        TableEntry::decode(FileAllocationTableType::Fat32, 0x1000_0000),
        TableEntry::Free
    );
    assert_eq!(
        TableEntry::decode(FileAllocationTableType::Fat16, 0x0001_0003),
        TableEntry::Next(3)
    );
    assert_eq!(
        TableEntry::decode(FileAllocationTableType::Fat12, 0xF001),
        TableEntry::Reserved
    );
}

#[test]
fn fat12_packs_two_entries_into_three_bytes() {
    // Entries 2 and 3 hold 0x123 and 0x456: bytes 0x23, 0x61, 0x45.
    let bytes = [0x23, 0x61, 0x45, 0];
    assert_eq!(assemble(FileAllocationTableType::Fat12, 2, bytes), 0x123);
    assert_eq!(
        assemble(
            FileAllocationTableType::Fat12,
            3,
            [bytes[1], bytes[2], 0, 0]
        ),
        0x456
    );
    assert_eq!(
        assemble(FileAllocationTableType::Fat16, 2, [0x34, 0x12, 0xFF, 0xFF]),
        0x1234
    );
    assert_eq!(
        assemble(FileAllocationTableType::Fat32, 2, [0x78, 0x56, 0x34, 0x12]),
        0x1234_5678
    );
}

#[test]
fn entries_lie_at_their_widths_offsets() {
    let boot = BootSector::parse(&fat32_with_active_table(0)).unwrap();
    let first = boot.table_first_sector(0).unwrap();
    assert_eq!(entry_position(&boot, 2), Some((first, 8)));
    assert_eq!(entry_position(&boot, 127), Some((first, 508)));
    assert_eq!(entry_position(&boot, 128), Some((first + 1, 0)));
    assert_eq!(entry_position(&boot, 0), None);
    assert_eq!(entry_position(&boot, boot.clusters() + 2), None);
    assert_eq!(entry_bytes(FileAllocationTableType::Fat12), 2);
    assert_eq!(entry_bytes(FileAllocationTableType::Fat16), 2);
    assert_eq!(entry_bytes(FileAllocationTableType::Fat32), 4);
}

#[test]
fn encode_is_decodes_inverse_for_every_kind_of_entry() {
    for file_allocation_table_type in [
        FileAllocationTableType::Fat12,
        FileAllocationTableType::Fat16,
        FileAllocationTableType::Fat32,
    ] {
        for entry in [
            TableEntry::Free,
            TableEntry::Reserved,
            TableEntry::Next(2),
            TableEntry::Next(0x123),
            TableEntry::Bad,
            TableEntry::EndOfChain,
        ] {
            let raw = entry.encode(file_allocation_table_type);
            assert_eq!(TableEntry::decode(file_allocation_table_type, raw), entry);
        }
    }
    assert_eq!(
        TableEntry::EndOfChain.encode(FileAllocationTableType::Fat12),
        0xFFF
    );
    assert_eq!(
        TableEntry::EndOfChain.encode(FileAllocationTableType::Fat16),
        0xFFFF
    );
    assert_eq!(
        TableEntry::EndOfChain.encode(FileAllocationTableType::Fat32),
        0x0FFF_FFFF
    );
    assert_eq!(
        TableEntry::Bad.encode(FileAllocationTableType::Fat16),
        0xFFF7
    );
}

#[test]
fn a_fat32_store_keeps_the_reserved_top_bits_and_a_fat12_store_keeps_the_neighbor() {
    let mut bytes = [0xAB, 0xCD, 0xEF, 0xF1];
    store(FileAllocationTableType::Fat32, 9, &mut bytes, 0x0123_4567);
    assert_eq!(bytes, [0x67, 0x45, 0x23, 0xF1]);
    let mut bytes = [0x34, 0xA2, 0, 0];
    store(FileAllocationTableType::Fat12, 4, &mut bytes, 0xFFF);
    assert_eq!(bytes[..2], [0xFF, 0xAF]);
    let mut bytes = [0x34, 0xA2, 0, 0];
    store(FileAllocationTableType::Fat12, 5, &mut bytes, 0x123);
    assert_eq!(bytes[..2], [0x34, 0x12]);
    let mut bytes = [0xFF, 0xFF, 7, 7];
    store(FileAllocationTableType::Fat16, 2, &mut bytes, 0x1234);
    assert_eq!(bytes, [0x34, 0x12, 7, 7]);
}
