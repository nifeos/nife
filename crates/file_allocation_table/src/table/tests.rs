//! The markers of each FAT type at their boundaries, and where entries lie in the table.

use super::*;
use crate::boot_sector::tests::fat32_with_active_table;

#[test]
fn each_types_markers_decode_at_their_boundaries() {
    for (fat_type, bad) in [
        (FatType::Fat12, 0xFF7u32),
        (FatType::Fat16, 0xFFF7),
        (FatType::Fat32, 0x0FFF_FFF7),
    ] {
        assert_eq!(TableEntry::decode(fat_type, 0), TableEntry::Free);
        assert_eq!(TableEntry::decode(fat_type, 1), TableEntry::Reserved);
        assert_eq!(TableEntry::decode(fat_type, 2), TableEntry::Next(2));
        assert_eq!(
            TableEntry::decode(fat_type, bad - 1),
            TableEntry::Next(bad - 1)
        );
        assert_eq!(TableEntry::decode(fat_type, bad), TableEntry::Bad);
        assert_eq!(
            TableEntry::decode(fat_type, bad + 1),
            TableEntry::EndOfChain
        );
        assert_eq!(
            TableEntry::decode(fat_type, bad + 8),
            TableEntry::EndOfChain
        );
    }
}

#[test]
fn bits_above_the_width_are_ignored() {
    assert_eq!(
        TableEntry::decode(FatType::Fat32, 0xF000_0002),
        TableEntry::Next(2)
    );
    assert_eq!(
        TableEntry::decode(FatType::Fat32, 0x1FFF_FFF8),
        TableEntry::EndOfChain
    );
    assert_eq!(
        TableEntry::decode(FatType::Fat32, 0x1000_0000),
        TableEntry::Free
    );
    assert_eq!(
        TableEntry::decode(FatType::Fat16, 0x0001_0003),
        TableEntry::Next(3)
    );
    assert_eq!(
        TableEntry::decode(FatType::Fat12, 0xF001),
        TableEntry::Reserved
    );
}

#[test]
fn fat12_packs_two_entries_into_three_bytes() {
    // Entries 2 and 3 hold 0x123 and 0x456: bytes 0x23, 0x61, 0x45.
    let bytes = [0x23, 0x61, 0x45, 0];
    assert_eq!(assemble(FatType::Fat12, 2, bytes), 0x123);
    assert_eq!(
        assemble(FatType::Fat12, 3, [bytes[1], bytes[2], 0, 0]),
        0x456
    );
    assert_eq!(
        assemble(FatType::Fat16, 2, [0x34, 0x12, 0xFF, 0xFF]),
        0x1234
    );
    assert_eq!(
        assemble(FatType::Fat32, 2, [0x78, 0x56, 0x34, 0x12]),
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
    assert_eq!(entry_bytes(FatType::Fat12), 2);
    assert_eq!(entry_bytes(FatType::Fat16), 2);
    assert_eq!(entry_bytes(FatType::Fat32), 4);
}
