//! Attribute sets, the stored byte, and the change on the small FAT12 volume `reader::tests` builds.
//!
//! Name: provisional.

use super::*;
use crate::reader::tests::Image;

#[test]
fn a_set_is_a_union_a_difference_and_a_containment() {
    let both = Attributes::HIDDEN.union(Attributes::SYSTEM);
    assert_eq!(both.bits(), 0x06);
    assert!(both.contains(Attributes::HIDDEN) && both.contains(Attributes::SYSTEM));
    assert!(!both.contains(Attributes::READ_ONLY));
    assert!(both.contains(Attributes::NONE));
    assert!(!Attributes::HIDDEN.contains(both));
    assert_eq!(both.without(Attributes::HIDDEN), Attributes::SYSTEM);
    assert_eq!(
        Attributes::HIDDEN.union(Attributes::HIDDEN),
        Attributes::HIDDEN
    );
    assert_eq!(Attributes::default(), Attributes::NONE);
    assert_eq!(Attributes::ARCHIVE.bits(), 0x20);
    assert_eq!(Attributes::READ_ONLY.bits(), 0x01);
}

#[test]
fn setting_changes_the_user_bits_and_keeps_the_directory_bit() {
    assert_eq!(apply(0x20, Attributes::READ_ONLY), 0x01);
    assert_eq!(apply(0x10, Attributes::HIDDEN), 0x12);
    assert_eq!(apply(0x37, Attributes::NONE), 0x10);
    assert_eq!(apply(0xC0, Attributes::ARCHIVE), 0xE0);
}

#[test]
fn a_file_and_a_directory_take_attributes_and_the_entry_reads_them_back() {
    let mut volume = Volume::open(Image::fat12()).unwrap();
    let file = volume.create_file(0, "F.TXT").unwrap();
    assert_eq!(file.file_attributes(), Attributes::ARCHIVE);
    let hidden = volume
        .set_attributes(&file, Attributes::HIDDEN.union(Attributes::SYSTEM))
        .unwrap();
    assert_eq!(hidden.file_attributes().bits(), 0x06);
    assert_eq!(hidden.place(), file.place());
    let dir = volume.make_directory(0, "D").unwrap();
    let dir = volume.set_attributes(&dir, Attributes::HIDDEN).unwrap();
    assert!(dir.is_directory());
    assert_eq!(dir.attributes(), 0x12);
    assert_eq!(
        volume.lookup("F.TXT").unwrap().file_attributes().bits(),
        0x06
    );
}

#[test]
fn a_read_only_file_refuses_a_write_a_resize_and_a_remove_until_the_bit_clears() {
    let mut volume = Volume::open(Image::fat12()).unwrap();
    let file = volume.create_file(0, "RO.TXT").unwrap();
    volume
        .open_file_writer(&file)
        .unwrap()
        .write(b"keep")
        .unwrap();
    let file = volume.lookup("RO.TXT").unwrap();
    let locked = volume.set_attributes(&file, Attributes::READ_ONLY).unwrap();
    let mut writer = volume.open_file_writer(&locked).unwrap();
    assert_eq!(writer.write(b"x"), Err(WriteError::ReadOnly));
    assert_eq!(writer.set_len(0), Err(WriteError::ReadOnly));
    assert_eq!(volume.remove_file(&locked), Err(WriteError::ReadOnly));
    let open = volume.set_attributes(&locked, Attributes::ARCHIVE).unwrap();
    volume.open_file_writer(&open).unwrap().write(b"!").unwrap();
    let current = volume.lookup("RO.TXT").unwrap();
    volume.remove_file(&current).unwrap();
}

#[test]
fn the_label_dots_and_a_stale_entry_are_refused() {
    let mut volume = Volume::open(Image::fat12()).unwrap();
    let dir = volume.make_directory(0, "D").unwrap();
    let dot = volume
        .entries(dir.first_cluster())
        .map(|e| e.unwrap())
        .find(NamedEntry::is_dot_entry)
        .unwrap();
    assert_eq!(
        volume.set_attributes(&dot, Attributes::HIDDEN),
        Err(WriteError::NotAFile)
    );
    let file = volume.create_file(0, "S.TXT").unwrap();
    volume.remove_file(&file).unwrap();
    assert_eq!(
        volume.set_attributes(&file, Attributes::HIDDEN),
        Err(WriteError::EntryChanged)
    );
}
