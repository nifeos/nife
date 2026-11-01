//! Labels on the small FAT12 volume `reader::tests` builds.
//!
//! Name: provisional.

use super::*;
use crate::reader::tests::Image;
use crate::{NoClock, WritableVolume};

#[test]
fn a_volume_with_no_label_reads_as_none_and_a_set_label_reads_back() {
    let mut volume = WritableVolume::open(Image::fat12(), NoClock).unwrap();
    assert_eq!(volume.volume_label(), Ok(None));
    volume.set_volume_label("MY DISK").unwrap();
    let label = volume.volume_label().unwrap().unwrap();
    assert_eq!(label.as_bytes(), b"MY DISK    ");
    assert_eq!(label.trimmed(), b"MY DISK");
    // The entry is the first in the root and the root still lists it as the label.
    let first = volume.root_entries().next().unwrap().unwrap();
    assert!(first.is_volume_label());
    volume.set_volume_label("OTHER").unwrap();
    assert_eq!(volume.volume_label().unwrap().unwrap().trimmed(), b"OTHER");
    let labels = volume
        .root_entries()
        .map(|e| e.unwrap())
        .filter(|e| e.is_volume_label())
        .count();
    assert_eq!(labels, 1);
}

#[test]
fn an_illegal_label_is_refused_and_changes_nothing() {
    let mut volume = WritableVolume::open(Image::fat12(), NoClock).unwrap();
    for bad in ["", "lower", "TWELVE CHARS", "A*B"] {
        assert_eq!(
            volume.set_volume_label(bad),
            Err(WriteError::InvalidName),
            "{bad}"
        );
    }
    assert_eq!(volume.volume_label(), Ok(None));
}

#[test]
fn the_boot_sectors_copy_is_read_when_the_root_has_no_entry() {
    let mut image = Image::fat12();
    // Extended boot signature and a label at the FAT12 offsets.
    image.bytes[38] = 0x29;
    image.bytes[43..54].copy_from_slice(b"BOOT LABEL ");
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    assert_eq!(
        volume.volume_label().unwrap().unwrap().trimmed(),
        b"BOOT LABEL"
    );
    // "NO NAME" is the absence of a label, and so is an all-space field.
    let mut image = volume.into_device();
    image.bytes[43..54].copy_from_slice(b"NO NAME    ");
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    assert_eq!(volume.volume_label(), Ok(None));
    let mut image = volume.into_device();
    image.bytes[43..54].copy_from_slice(&[b' '; 11]);
    assert_eq!(
        WritableVolume::open(image, NoClock).unwrap().volume_label(),
        Ok(None)
    );
}

#[test]
fn setting_the_label_writes_the_boot_sector_copy_only_where_it_has_a_field() {
    let mut image = Image::fat12();
    image.bytes[38] = 0x29;
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    volume.set_volume_label("NEW").unwrap();
    let image = volume.into_device();
    assert_eq!(&image.bytes[43..54], b"NEW        ");
    // Without the signature the boot sector is left alone.
    let mut volume = WritableVolume::open(Image::fat12(), NoClock).unwrap();
    volume.set_volume_label("NEW").unwrap();
    assert!(volume.into_device().bytes[43..54].iter().all(|b| *b == 0));
}

#[test]
fn a_full_fixed_root_has_no_room_for_a_first_label() {
    let mut volume = WritableVolume::open(Image::fat12(), NoClock).unwrap();
    for i in 0..16u8 {
        let name = [b'A' + i];
        volume
            .create_file(0, core::str::from_utf8(&name).unwrap())
            .unwrap();
    }
    assert_eq!(volume.set_volume_label("X"), Err(WriteError::DirectoryFull));
}
