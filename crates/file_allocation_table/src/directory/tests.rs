//! Slots parsed one by one, and directories read through the FAT12 root region and through
//! cluster chains on the reader tests' volume.

use super::*;
use crate::reader::tests::{FIRST_DATA, Image};
use crate::table::TableEntry;

/// More slots than any directory here holds. Every read is capped at it, so a reader that stops
/// advancing fails an assertion rather than hanging the suite.
const MOST: usize = 1 << 14;

/// A short entry's 32 bytes.
fn short(name: &[u8; 11], attributes: u8, high: u16, low: u16, size: u32) -> [u8; 32] {
    let mut slot = [0u8; 32];
    slot[..11].copy_from_slice(name);
    slot[11] = attributes;
    slot[20..22].copy_from_slice(&high.to_le_bytes());
    slot[26..28].copy_from_slice(&low.to_le_bytes());
    slot[28..32].copy_from_slice(&size.to_le_bytes());
    slot
}

/// A long-name piece holding the units `1..=13` plus `base`.
fn long(order: u8, checksum: u8, base: u16) -> [u8; 32] {
    let mut slot = [0u8; 32];
    slot[0] = order;
    slot[11] = 0x0F;
    slot[13] = checksum;
    let offsets = [1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30];
    for (unit, at) in offsets.into_iter().enumerate() {
        slot[at..at + 2].copy_from_slice(&(base + unit as u16 + 1).to_le_bytes());
    }
    slot
}

fn short_of(slot: Slot) -> ShortEntry {
    match slot {
        Slot::Used(DirectoryEntry::Short(entry)) => entry,
        other => panic!("not a short entry: {other:?}"),
    }
}

#[test]
fn the_first_byte_marks_the_end_and_free_slots() {
    let mut slot = short(b"HELLO   TXT", 0x20, 0, 5, 1);
    slot[0] = 0;
    assert_eq!(Slot::parse(&slot, FatType::Fat16), Slot::End);
    slot[0] = 0xE5;
    assert_eq!(Slot::parse(&slot, FatType::Fat16), Slot::Free);
    let mut piece = long(0x41, 7, 0);
    piece[0] = 0xE5;
    assert_eq!(Slot::parse(&piece, FatType::Fat32), Slot::Free);
}

#[test]
fn a_short_entry_reads_its_fields() {
    let entry = short_of(Slot::parse(
        &short(b"HELLO   TXT", 0x21, 0, 5, 1234),
        FatType::Fat12,
    ));
    assert_eq!(&entry.name(), b"HELLO   TXT");
    assert_eq!(entry.attributes(), 0x21);
    assert!(!entry.is_directory() && !entry.is_volume_label());
    assert_eq!(entry.first_cluster(), 5);
    assert_eq!(entry.size(), 1234);

    let dir = short_of(Slot::parse(
        &short(b"SUB        ", 0x10, 0, 9, 0),
        FatType::Fat12,
    ));
    assert!(dir.is_directory());
    let label = short_of(Slot::parse(
        &short(b"NIFETEST   ", 0x08, 0, 0, 0),
        FatType::Fat12,
    ));
    assert!(label.is_volume_label());
}

#[test]
fn a_stored_0x05_reads_back_as_0xe5() {
    let entry = short_of(Slot::parse(
        &short(b"\x05BC     TXT", 0x20, 0, 5, 1),
        FatType::Fat16,
    ));
    assert_eq!(entry.name()[0], 0xE5);
}

#[test]
fn only_fat32_takes_the_high_cluster_word() {
    let slot = short(b"BIG     BIN", 0x20, 0x0001, 0x0002, 1);
    assert_eq!(
        short_of(Slot::parse(&slot, FatType::Fat32)).first_cluster(),
        0x0001_0002
    );
    assert_eq!(
        short_of(Slot::parse(&slot, FatType::Fat16)).first_cluster(),
        2
    );
    assert_eq!(
        short_of(Slot::parse(&slot, FatType::Fat12)).first_cluster(),
        2
    );
}

#[test]
fn a_long_name_piece_reads_its_units_in_order() {
    let Slot::Used(DirectoryEntry::LongName(piece)) =
        Slot::parse(&long(0x42, 0xAB, 0x40), FatType::Fat32)
    else {
        panic!("not a long-name piece");
    };
    assert_eq!(piece.ordinal(), 2);
    assert!(piece.is_last());
    assert_eq!(piece.checksum(), 0xAB);
    assert_eq!(piece.units(), core::array::from_fn(|i| 0x41 + i as u16));
    let Slot::Used(DirectoryEntry::LongName(piece)) =
        Slot::parse(&long(0x01, 0, 0), FatType::Fat32)
    else {
        panic!("not a long-name piece");
    };
    assert_eq!(piece.ordinal(), 1);
    assert!(!piece.is_last());
}

#[test]
fn the_archive_bit_beside_the_long_name_bits_makes_a_short_entry() {
    let slot = short(b"ODD     BIN", 0x2F, 0, 3, 1);
    assert!(matches!(
        Slot::parse(&slot, FatType::Fat16),
        Slot::Used(DirectoryEntry::Short(_))
    ));
}

/// Write `slots` into the volume from byte offset `at`.
fn put(image: &mut Image, at: usize, slots: &[[u8; 32]]) {
    for (index, slot) in slots.iter().enumerate() {
        image.bytes[at + index * 32..at + index * 32 + 32].copy_from_slice(slot);
    }
}

/// The byte offset of `cluster` in the reader tests' volume.
fn cluster_at(cluster: u32) -> usize {
    (FIRST_DATA + cluster as usize - 2) * 512
}

#[test]
fn the_root_region_yields_used_slots_and_stops_at_the_end_slot() {
    let mut image = Image::fat12();
    let mut free = short(b"GONE    TXT", 0x20, 0, 4, 1);
    free[0] = 0xE5;
    put(
        &mut image,
        5 * 512,
        &[
            long(0x41, 1, 0),
            short(b"HELLO   TXT", 0x20, 0, 10, 5),
            free,
            short(b"SUB        ", 0x10, 0, 20, 0),
            [0; 32],
            short(b"AFTER   END", 0x20, 0, 30, 1),
        ],
    );
    let mut volume = Volume::open(image).unwrap();
    let mut root = volume.root_directory();
    assert!(matches!(root.next(), Some(Ok(DirectoryEntry::LongName(_)))));
    let Some(Ok(DirectoryEntry::Short(hello))) = root.next() else {
        panic!()
    };
    assert_eq!(&hello.name(), b"HELLO   TXT");
    let Some(Ok(DirectoryEntry::Short(sub))) = root.next() else {
        panic!()
    };
    assert_eq!(sub.first_cluster(), 20);
    assert!(root.next().is_none());
    assert!(root.next().is_none());
    assert_eq!(volume.directory(0).take(MOST).count(), 3);
}

#[test]
fn a_full_root_region_ends_at_its_last_slot() {
    let mut image = Image::fat12();
    let slots = [short(b"FILE    TXT", 0x20, 0, 3, 1); 16];
    put(&mut image, 5 * 512, &slots);
    let mut volume = Volume::open(image).unwrap();
    assert_eq!(volume.root_directory().take(MOST).count(), 16);
}

#[test]
fn a_subdirectory_reads_across_its_clusters() {
    let mut image = Image::fat12();
    image.link(&[20, 33]);
    put(
        &mut image,
        cluster_at(20),
        &[short(b"FILE    TXT", 0x20, 0, 3, 1); 16],
    );
    put(
        &mut image,
        cluster_at(33),
        &[short(b"LAST    TXT", 0x20, 0, 4, 1)],
    );
    let mut volume = Volume::open(image).unwrap();
    let mut count = 0;
    for entry in volume.directory(20).take(MOST) {
        assert!(matches!(entry, Ok(DirectoryEntry::Short(_))));
        count += 1;
    }
    assert_eq!(count, 17);
}

#[test]
fn a_subdirectory_without_an_end_slot_ends_with_its_chain() {
    let mut image = Image::fat12();
    image.link(&[20]);
    put(
        &mut image,
        cluster_at(20),
        &[short(b"FILE    TXT", 0x20, 0, 3, 1); 16],
    );
    let mut volume = Volume::open(image).unwrap();
    assert_eq!(volume.directory(20).take(MOST).count(), 16);
}

#[test]
fn a_looping_directory_chain_ends_with_an_error() {
    let mut image = Image::fat12();
    image.set(20, 20);
    put(
        &mut image,
        cluster_at(20),
        &[short(b"FILE    TXT", 0x20, 0, 3, 1); 16],
    );
    let mut volume = Volume::open(image).unwrap();
    let mut entries = 0usize;
    let mut last = None;
    for entry in volume.directory(20).take(MOST) {
        match entry {
            Ok(_) => entries += 1,
            Err(error) => last = Some(error),
        }
    }
    assert_eq!(entries, 350 * 16);
    assert_eq!(last, Some(ChainError::TooLong));
}

#[test]
fn a_broken_directory_chain_and_a_failing_device_end_the_read() {
    let mut image = Image::fat12();
    image.set(20, 0);
    put(
        &mut image,
        cluster_at(20),
        &[short(b"FILE    TXT", 0x20, 0, 3, 1); 16],
    );
    let mut volume = Volume::open(image).unwrap();
    let mut read = volume.directory(20);
    assert_eq!(read.by_ref().take(16).filter(Result::is_ok).count(), 16);
    assert_eq!(
        read.next(),
        Some(Err(ChainError::Broken {
            cluster: 20,
            entry: TableEntry::Free
        }))
    );
    assert_eq!(read.next(), None);

    let mut image = Image::fat12();
    image.refuse = Some(5);
    let mut volume = Volume::open(image).unwrap();
    let mut root = volume.root_directory();
    assert_eq!(root.next(), Some(Err(ChainError::Device(5))));
    assert_eq!(root.next(), None);

    let mut volume = Volume::open(Image::fat12()).unwrap();
    assert_eq!(
        volume.directory(1).next(),
        Some(Err(ChainError::NotADataCluster(1)))
    );
}
