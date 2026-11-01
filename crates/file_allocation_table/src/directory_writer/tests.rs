//! Run placement, alias numbering, directories and renames on the small FAT12 volume
//! `reader::tests` builds.
//!
//! Name: ratified 2026-11-01 (UTC), by the class rule on the `tests` and `verification` modules of this crate.

use super::*;
use crate::reader::tests::Image;
use crate::{NoClock, WritableVolume};

fn feed(count: u32, kinds: &[SlotKind]) -> Option<u32> {
    let mut finder = RunFinder::new(count);
    kinds.iter().find_map(|kind| finder.push(*kind))
}

#[test]
fn a_run_starts_at_the_first_gap_that_is_long_enough_and_an_end_marker_completes_it() {
    use SlotKind::{End, Free, Used};
    assert_eq!(feed(1, &[Used, Free]), Some(1));
    assert_eq!(feed(2, &[Free, Used, Free, Free]), Some(2));
    assert_eq!(feed(3, &[Free, Free, Used, Free, Free]), None);
    assert_eq!(feed(3, &[Used, Free, Free, End]), Some(1));
    assert_eq!(feed(4, &[Used, End]), Some(1));
    assert_eq!(feed(2, &[End]), Some(0));
}

#[test]
fn aliases_count_up_and_give_the_base_room_for_the_tail() {
    let name = LongName::new("A Long Name Number 7.txt").unwrap();
    assert_eq!(name.alias_numbered(1), Some(*b"ALONGN~1TXT"));
    assert_eq!(name.alias_numbered(9), Some(*b"ALONGN~9TXT"));
    assert_eq!(name.alias_numbered(10), Some(*b"ALONG~10TXT"));
    assert_eq!(name.alias_numbered(99), Some(*b"ALONG~99TXT"));
    assert_eq!(name.alias_numbered(100), Some(*b"ALON~100TXT"));
    assert_eq!(name.alias_numbered(999_999), Some(*b"A~999999TXT"));
    assert_eq!(name.alias_numbered(0), None);
    assert_eq!(name.alias_numbered(1_000_000), None);
}

#[test]
fn colliding_long_names_take_successive_aliases_and_a_freed_alias_is_reused() {
    let mut volume = WritableVolume::open(Image::fat12(), NoClock).unwrap();
    let first = volume.create_file(0, "Long File One.txt").unwrap();
    let second = volume.create_file(0, "Long File Two.txt").unwrap();
    assert_eq!(first.short_name(), *b"LONGFI~1TXT");
    assert_eq!(second.short_name(), *b"LONGFI~2TXT");
    volume.remove_file(&first).unwrap();
    let third = volume.create_file(0, "Long File Three.txt").unwrap();
    assert_eq!(third.short_name(), *b"LONGFI~1TXT");
    assert!(volume.lookup("long file three.TXT").is_ok());
    assert_eq!(
        volume.create_file(0, "LONG FILE TWO.txt"),
        Err(WriteError::AlreadyExists)
    );
}

#[test]
fn a_long_run_fits_a_gap_of_deleted_slots_and_not_a_shorter_one() {
    let mut volume = WritableVolume::open(Image::fat12(), NoClock).unwrap();
    let a = volume.create_file(0, "A.TXT").unwrap();
    volume.create_file(0, "B.TXT").unwrap();
    let c = volume.create_file(0, "C.TXT").unwrap();
    volume.create_file(0, "D.TXT").unwrap();
    volume.remove_file(&a).unwrap();
    volume.remove_file(&c).unwrap();
    // One slot gaps at 0 and 2; a two-piece name needs three slots, so it goes after D.
    let long = volume
        .create_file(0, "A name that needs two long pieces.txt")
        .unwrap();
    assert_eq!(long.place().first_slot, 4);
    assert_eq!(long.place().short_slot, 7);
    // A short name takes the first gap.
    assert_eq!(
        volume.create_file(0, "E.TXT").unwrap().place().short_slot,
        0
    );
}

#[test]
fn a_directory_holds_dot_entries_and_is_removed_only_when_empty() {
    let mut volume = WritableVolume::open(Image::fat12(), NoClock).unwrap();
    let free = volume.count_free_clusters().unwrap();
    let dir = volume.create_directory(0, "Sub Directory").unwrap();
    assert!(dir.is_directory());
    assert_eq!(volume.count_free_clusters().unwrap(), free - 1);
    let names: usize = volume
        .named_entries(dir.first_cluster())
        .map(|e| e.unwrap())
        .filter(|e| e.is_dot_entry())
        .count();
    assert_eq!(names, 2);
    let inner = volume
        .create_file(dir.first_cluster(), "inner.txt")
        .unwrap();
    assert_eq!(
        volume.remove_directory(&dir),
        Err(WriteError::DirectoryNotEmpty)
    );
    volume.remove_file(&inner).unwrap();
    volume.remove_directory(&dir).unwrap();
    assert_eq!(volume.count_free_clusters().unwrap(), free);
    assert!(volume.lookup("Sub Directory").is_err());
    let file = volume.create_file(0, "F.TXT").unwrap();
    assert_eq!(
        volume.remove_directory(&file),
        Err(WriteError::NotADirectory)
    );
}

#[test]
fn a_failed_create_directory_gives_its_cluster_back() {
    let mut volume = WritableVolume::open(Image::fat12(), NoClock).unwrap();
    volume.create_directory(0, "D").unwrap();
    let free = volume.count_free_clusters().unwrap();
    assert_eq!(
        volume.create_directory(0, "d"),
        Err(WriteError::AlreadyExists)
    );
    assert_eq!(
        volume.create_directory(0, "bad*name"),
        Err(WriteError::NameRefused)
    );
    assert_eq!(volume.count_free_clusters().unwrap(), free);
}

#[test]
fn a_rename_keeps_the_data_and_a_moved_directory_gets_a_new_dotdot() {
    let mut volume = WritableVolume::open(Image::fat12(), NoClock).unwrap();
    let a = volume.create_directory(0, "A").unwrap();
    let b = volume.create_directory(0, "B").unwrap();
    let file = volume.create_file(a.first_cluster(), "data.bin").unwrap();
    volume
        .open_file_writer(&file)
        .unwrap()
        .write(&[7; 600])
        .unwrap();
    let file = volume.lookup("A/data.bin").unwrap();
    let moved = volume
        .rename(&file, b.first_cluster(), "Data Renamed.bin")
        .unwrap();
    assert_eq!(
        (moved.first_cluster(), moved.size_bytes()),
        (file.first_cluster(), 600)
    );
    assert!(volume.lookup("A/data.bin").is_err());
    assert!(volume.lookup("B/data renamed.BIN").is_ok());
    // The moved file's bytes are untouched (a move rewrites entries, never clusters).
    let entry = volume.lookup("B/data renamed.BIN").unwrap();
    let mut reader = volume.open_file_reader(&entry).unwrap();
    let mut out = [0u8; 700];
    assert_eq!(reader.read(&mut out).unwrap(), 600);
    assert!(out[..600].iter().all(|b| *b == 7));
    let a = volume.lookup("A").unwrap();
    volume.rename(&a, b.first_cluster(), "A Inside B").unwrap();
    let dotdot = volume.read_slot(a.first_cluster(), 1).unwrap().unwrap();
    assert_eq!(
        u16::from_le_bytes([dotdot[26], dotdot[27]]) as u32,
        b.first_cluster()
    );
    assert!(volume.lookup("B/A Inside B").is_ok());
    // And back to the root, where `..` is 0.
    let inside = volume.lookup("B/A Inside B").unwrap();
    volume.rename(&inside, 0, "A Again").unwrap();
    let dotdot = volume.read_slot(a.first_cluster(), 1).unwrap().unwrap();
    assert_eq!(u16::from_le_bytes([dotdot[26], dotdot[27]]), 0);
}

#[test]
fn a_directory_cannot_move_into_itself_or_below_and_dots_cannot_be_renamed() {
    let mut volume = WritableVolume::open(Image::fat12(), NoClock).unwrap();
    let a = volume.create_directory(0, "A").unwrap();
    let inner = volume.create_directory(a.first_cluster(), "Inner").unwrap();
    let innermost = volume
        .create_directory(inner.first_cluster(), "Innermost")
        .unwrap();
    for target in [
        a.first_cluster(),
        inner.first_cluster(),
        innermost.first_cluster(),
    ] {
        assert_eq!(
            volume.rename(&a, target, "X"),
            Err(WriteError::MovedIntoItself)
        );
    }
    assert!(volume.rename(&inner, 0, "Out").is_ok());
    let dot = volume
        .named_entries(a.first_cluster())
        .map(|e| e.unwrap())
        .find(|e| e.is_dot_entry())
        .unwrap();
    assert_eq!(volume.rename(&dot, 0, "Dot"), Err(WriteError::NotAFile));
    assert_eq!(
        volume.remove_directory(&dot),
        Err(WriteError::NotADirectory)
    );
}

#[test]
fn a_rename_that_only_changes_case_keeps_one_entry() {
    let mut volume = WritableVolume::open(Image::fat12(), NoClock).unwrap();
    let file = volume.create_file(0, "Mixed Case Name.txt").unwrap();
    let again = volume.rename(&file, 0, "MIXED case NAME.txt").unwrap();
    assert_ne!(again.place().short_slot, file.place().short_slot);
    let listed = volume
        .root_named_entries()
        .map(|e| e.unwrap())
        .filter(|e| !e.is_volume_label())
        .count();
    assert_eq!(listed, 1);
}

#[test]
fn a_stale_entry_is_refused_by_a_directory_remove_and_a_rename() {
    let mut volume = WritableVolume::open(Image::fat12(), NoClock).unwrap();
    let dir = volume.create_directory(0, "D").unwrap();
    let file = volume.create_file(0, "F.TXT").unwrap();
    // The directory's slot becomes a long-name piece; the file's slot is freed and reused.
    let slot = dir.place().short_slot as usize;
    volume.remove_file(&file).unwrap();
    assert_eq!(
        volume.rename(&file, 0, "G.TXT"),
        Err(WriteError::EntryChanged)
    );
    let mut image = volume.into_device();
    image.bytes[5 * 512 + slot * 32 + 11] = 0x0F;
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    assert_eq!(volume.remove_directory(&dir), Err(WriteError::EntryChanged));
    assert_eq!(volume.rename(&dir, 0, "E"), Err(WriteError::EntryChanged));
}

#[test]
fn a_volume_label_cannot_be_renamed() {
    let mut volume = WritableVolume::open(Image::fat12(), NoClock).unwrap();
    let file = volume.create_file(0, "L.TXT").unwrap();
    let mut image = volume.into_device();
    image.bytes[5 * 512 + 11] = 0x08;
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    let label = volume
        .root_named_entries()
        .map(|e| e.unwrap())
        .next()
        .unwrap();
    assert!(label.is_volume_label());
    assert_eq!(volume.rename(&label, 0, "NEW"), Err(WriteError::NotAFile));
    let _ = file;
}
