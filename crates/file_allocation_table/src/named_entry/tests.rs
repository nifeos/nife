//! A root directory with long names, stale pieces and the entries a listing meets, read as named
//! entries.
//!
//! Name: ratified 2026-11-01 (UTC), by the class rule on the `tests` and `verification` modules of this crate.

use super::*;
use crate::long_name::{LongName, checksum};
use crate::reader::Volume;
use crate::reader::tests::Image;

/// The root region's first byte in the reader tests' volume.
const ROOT: usize = 5 * 512;

/// A short entry's 32 bytes.
fn short(name: &[u8; 11], attributes: u8, cluster: u16, size: u32) -> [u8; 32] {
    let mut slot = [0u8; 32];
    slot[..11].copy_from_slice(name);
    slot[11] = attributes;
    slot[26..28].copy_from_slice(&cluster.to_le_bytes());
    slot[28..32].copy_from_slice(&size.to_le_bytes());
    slot
}

/// Write `slots` from the start of the root region.
fn put(image: &mut Image, slots: &[[u8; 32]]) {
    for (index, slot) in slots.iter().enumerate() {
        image.bytes[ROOT + index * 32..ROOT + index * 32 + 32].copy_from_slice(slot);
    }
}

/// The writer's pieces for `name`, bound to `short`, then `short` itself, as slots.
fn file(name: &str, short_name: &[u8; 11], cluster: u16, size: u32) -> ([[u8; 32]; 4], usize) {
    let long = LongName::new(name).expect("a long name");
    let mut out = [[0u8; 32]; 4];
    for (index, slot) in out.iter_mut().enumerate().take(long.entries()) {
        long.entry(index, checksum(short_name), slot);
    }
    out[long.entries()] = short(short_name, 0x20, cluster, size);
    (out, long.entries() + 1)
}

#[test]
fn a_short_entry_carries_the_long_name_ahead_of_it() {
    let mut image = Image::fat12();
    let (slots, count) = file("A Long File Name.txt", b"ALONGF~1TXT", 9, 77);
    let mut all = [[0u8; 32]; 6];
    all[..count].copy_from_slice(&slots[..count]);
    all[count] = short(b"PLAIN   BIN", 0x20, 12, 5);
    all[count + 1] = short(b"SUB        ", 0x10, 14, 0);
    put(&mut image, &all[..count + 2]);
    let mut volume = Volume::open(image).unwrap();
    let mut entries = volume.root_entries();

    let first = entries.next().unwrap().unwrap();
    assert_eq!(&first.short_name(), b"ALONGF~1TXT");
    let name = first.long_name().unwrap();
    assert!(name.characters().eq("A Long File Name.txt".chars()));
    assert_eq!(first.long_name_refusal(), None);
    assert_eq!((first.first_cluster(), first.size_bytes()), (9, 77));
    assert_eq!(first.attributes(), 0x20);
    assert!(!first.is_directory() && !first.is_volume_label() && !first.is_dot_entry());

    let plain = entries.next().unwrap().unwrap();
    assert_eq!(plain.long_name(), None);
    assert_eq!(plain.long_name_refusal(), None);
    assert_eq!(plain.size_bytes(), 5);

    let sub = entries.next().unwrap().unwrap();
    assert!(sub.is_directory());
    assert!(entries.next().is_none());
    assert!(entries.next().is_none());
}

#[test]
fn a_run_for_another_entry_is_refused_and_the_entry_is_still_listed() {
    let mut image = Image::fat12();
    let (mut slots, count) = file("Stale Name.txt", b"STALEN~1TXT", 9, 1);
    // The short entry was renamed after the pieces were written.
    slots[count - 1][..11].copy_from_slice(b"RENAMED TXT");
    put(&mut image, &slots[..count]);
    let mut volume = Volume::open(image).unwrap();
    let entry = volume.root_entries().next().unwrap().unwrap();
    assert_eq!(&entry.short_name(), b"RENAMED TXT");
    assert_eq!(entry.long_name(), None);
    assert_eq!(
        entry.long_name_refusal(),
        Some(LongNameError::ChecksumMismatch)
    );
}

#[test]
fn a_run_does_not_leak_into_the_entry_after_its_own() {
    let mut image = Image::fat12();
    let (slots, count) = file("First Long.txt", b"FIRSTL~1TXT", 9, 1);
    let mut all = [[0u8; 32]; 4];
    all[..count].copy_from_slice(&slots[..count]);
    all[count] = short(b"SECOND  TXT", 0x20, 10, 1);
    put(&mut image, &all[..count + 1]);
    let mut volume = Volume::open(image).unwrap();
    let mut entries = volume.root_entries();
    assert!(entries.next().unwrap().unwrap().long_name().is_some());
    let second = entries.next().unwrap().unwrap();
    assert_eq!(second.long_name(), None);
    assert_eq!(second.long_name_refusal(), None);
}

#[test]
fn a_deleted_files_pieces_are_skipped_with_it() {
    let mut image = Image::fat12();
    let (mut slots, count) = file("Deleted Name.txt", b"DELETE~1TXT", 9, 1);
    for slot in &mut slots[..count] {
        slot[0] = 0xE5;
    }
    let mut all = [[0u8; 32]; 4];
    all[..count].copy_from_slice(&slots[..count]);
    all[count] = short(b"LIVE    TXT", 0x20, 10, 1);
    put(&mut image, &all[..count + 1]);
    let mut volume = Volume::open(image).unwrap();
    let mut entries = volume.root_entries();
    let live = entries.next().unwrap().unwrap();
    assert_eq!(&live.short_name(), b"LIVE    TXT");
    assert_eq!(live.long_name_refusal(), None);
    assert!(entries.next().is_none());
}

#[test]
fn the_dot_entries_and_the_label_are_flagged() {
    let mut image = Image::fat12();
    put(
        &mut image,
        &[
            short(b".          ", 0x10, 20, 0),
            short(b"..         ", 0x10, 0, 0),
            short(b"NIFETEST   ", 0x08, 0, 0),
            short(b".GITKEEP   ", 0x20, 0, 0),
        ],
    );
    let mut volume = Volume::open(image).unwrap();
    let mut entries = volume.root_entries();
    assert!(entries.next().unwrap().unwrap().is_dot_entry());
    assert!(entries.next().unwrap().unwrap().is_dot_entry());
    assert!(entries.next().unwrap().unwrap().is_volume_label());
    let hidden_name = entries.next().unwrap().unwrap();
    assert!(!hidden_name.is_dot_entry() && !hidden_name.is_directory());
}

#[test]
fn a_device_error_ends_the_listing() {
    let mut image = Image::fat12();
    image.refuse = Some(5);
    let mut volume = Volume::open(image).unwrap();
    let mut entries = volume.root_entries();
    assert_eq!(entries.next(), Some(Err(ChainError::Device(5))));
    assert_eq!(entries.next(), None);
}

#[test]
fn a_subdirectory_lists_by_its_cluster() {
    let mut image = Image::fat12();
    image.link(&[20]);
    let at = (crate::reader::tests::FIRST_DATA + 18) * 512;
    image.bytes[at..at + 32].copy_from_slice(&short(b"CHILD   TXT", 0x20, 30, 3));
    let mut volume = Volume::open(image).unwrap();
    let child = volume.entries(20).next().unwrap().unwrap();
    assert_eq!(&child.short_name(), b"CHILD   TXT");
    assert_eq!(volume.entries(0).count(), 0);
}

#[test]
fn a_listing_yields_nothing_after_an_error_however_often_it_is_asked() {
    use crate::reader::tests::Image;
    let mut image = Image::fat12();
    // A directory chain that loops through clusters whose sectors the image does not store.
    image.set(40, 41);
    image.set(41, 40);
    image.bytes[(6 + 40 - 2) * 512..(6 + 41 - 2) * 512].fill(0xE5);
    image.refuse = Some(6 + 41 - 2);
    let mut volume = crate::Volume::open(image).unwrap();
    let mut entries = volume.entries(40);
    let mut errors = 0;
    for _ in 0..20 {
        if let Some(Err(_)) = entries.next() {
            errors += 1;
        }
    }
    assert_eq!(errors, 1);
    assert!(entries.next().is_none());
}
