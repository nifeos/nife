//! Names compared without regard to case, long and short, and paths walked through a root and a
//! subdirectory of a small FAT12 volume.
//!
//! Name: ratified 2026-11-01 (UTC), by the class rule on the `tests` and `verification` modules of this crate.

use super::*;
use crate::long_name::{LongName, checksum};
use crate::reader::tests::{FIRST_DATA, Image};

/// A short entry's 32 bytes.
fn short(name: &[u8; 11], attributes: u8, cluster: u16, size: u32) -> [u8; 32] {
    let mut slot = [0u8; 32];
    slot[..11].copy_from_slice(name);
    slot[11] = attributes;
    slot[26..28].copy_from_slice(&cluster.to_le_bytes());
    slot[28..32].copy_from_slice(&size.to_le_bytes());
    slot
}

/// Write `slots` at byte `at` of `image`.
fn put(image: &mut Image, at: usize, slots: &[[u8; 32]]) {
    for (index, slot) in slots.iter().enumerate() {
        image.bytes[at + index * 32..at + index * 32 + 32].copy_from_slice(slot);
    }
}

/// The writer's pieces for `name`, then the short entry.
fn file(name: &str, short_name: &[u8; 11], cluster: u16, size: u32, out: &mut [[u8; 32]]) -> usize {
    let long = LongName::new(name).expect("a long name");
    for (index, slot) in out.iter_mut().enumerate().take(long.entries()) {
        long.entry(index, checksum(short_name), slot);
    }
    out[long.entries()] = short(short_name, 0x20, cluster, size);
    long.entries() + 1
}

/// A volume whose root holds `Long Name.txt` (short `LONGNA~1.TXT`), `README` and a directory
/// `Sub Dir` (short `SUBDIR`, cluster 20), which holds `Inner File.dat` and a `.` and `..`.
fn volume() -> Volume<Image> {
    let mut image = Image::fat12();
    image.link(&[20]);
    let mut root = [[0u8; 32]; 8];
    let mut count = file("Long Name.txt", b"LONGNA~1TXT", 9, 11, &mut root);
    root[count] = short(b"README     ", 0x20, 10, 1);
    count += 1;
    count += file("Sub Dir", b"SUBDIR     ", 20, 0, &mut root[count..]);
    root[count - 1][11] = 0x10;
    root[count] = short(b"LABEL      ", 0x08, 0, 0);
    count += 1;
    put(&mut image, 5 * 512, &root[..count]);
    let mut inner = [[0u8; 32]; 6];
    inner[0] = short(b".          ", 0x10, 20, 0);
    inner[1] = short(b"..         ", 0x10, 0, 0);
    let count = 2 + file("Inner File.dat", b"INNERF~1DAT", 30, 5, &mut inner[2..]);
    put(&mut image, (FIRST_DATA + 18) * 512, &inner[..count]);
    Volume::open(image).unwrap()
}

#[test]
fn long_names_match_without_regard_to_case() {
    let units = |s: &str| {
        let mut out = [0u16; 32];
        let mut len = 0;
        for unit in s.encode_utf16() {
            out[len] = unit;
            len += 1;
        }
        (out, len)
    };
    let (name, len) = units("Long Name.txt");
    let name = &name[..len];
    for query in ["Long Name.txt", "long name.TXT", "LONG NAME.TXT"] {
        assert!(long_name_matches(name, query), "{query}");
    }
    for query in [
        "Long Name.tx",
        "Long Name.txtt",
        "",
        "Long  Name.txt",
        "Long Name.txt ",
    ] {
        assert!(!long_name_matches(name, query), "{query:?}");
    }
    let (name, len) = units("Éa\u{1F980}");
    assert!(long_name_matches(&name[..len], "éA\u{1F980}"));
    assert!(!long_name_matches(&name[..len], "ea\u{1F980}"));
}

#[test]
fn short_names_match_in_their_dotted_form_without_regard_to_case() {
    assert!(short_name_matches(b"README     ", "readme"));
    assert!(short_name_matches(b"README     ", "README"));
    assert!(!short_name_matches(b"README     ", "readme."));
    assert!(!short_name_matches(b"README     ", "readme.txt"));
    assert!(short_name_matches(b"HELLO   TXT", "hello.txt"));
    assert!(!short_name_matches(b"HELLO   TXT", "hello"));
    assert!(!short_name_matches(b"HELLO   TXT", "hellotxt"));
    assert!(short_name_matches(b"A       B  ", "a.b"));
    assert!(short_name_matches(b".          ", "."));
    assert!(short_name_matches(b"..         ", ".."));
    assert!(!short_name_matches(b".          ", ".."));
    assert!(short_name_matches(b"ABCDEFGHXYZ", "abcdefgh.xyz"));
    assert!(!short_name_matches(b"HELLO   TXT", "h\u{e9}llo.txt"));
}

#[test]
fn a_file_is_found_by_either_name_in_any_case() {
    let mut volume = volume();
    for path in [
        "Long Name.txt",
        "long name.TXT",
        "/LONGNA~1.TXT",
        "longna~1.txt",
        "\\Long Name.txt",
        "//long name.txt//",
    ] {
        let entry = volume
            .lookup(path)
            .unwrap_or_else(|e| panic!("{path}: {e:?}"));
        assert_eq!(entry.first_cluster(), 9, "{path}");
    }
    assert_eq!(volume.lookup("readme").unwrap().first_cluster(), 10);
    assert_eq!(volume.lookup("README").unwrap().size_bytes(), 1);
}

#[test]
fn paths_walk_into_subdirectories_by_long_or_short_name() {
    let mut volume = volume();
    for path in [
        "Sub Dir/Inner File.dat",
        "SUBDIR/innerf~1.dat",
        "sub dir\\INNER FILE.DAT",
        "/subdir/./inner file.dat",
        "subdir/../subdir/inner file.dat",
    ] {
        let entry = volume
            .lookup(path)
            .unwrap_or_else(|e| panic!("{path}: {e:?}"));
        assert_eq!(entry.first_cluster(), 30, "{path}");
    }
    // The subdirectory holds no entry called "Sub Dir".
    assert_eq!(volume.lookup("Sub Dir/Sub Dir"), Err(LookupError::NotFound));
    let dir = volume.lookup("sub dir").unwrap();
    assert!(dir.is_directory());
    assert_eq!(dir.first_cluster(), 20);
    assert_eq!(
        volume
            .find(20, "inner file.dat")
            .unwrap()
            .unwrap()
            .size_bytes(),
        5
    );
    assert_eq!(volume.find(0, "inner file.dat"), Ok(None));
}

#[test]
fn what_is_not_there_is_not_found_and_the_label_is_called_nothing() {
    let mut volume = volume();
    assert_eq!(volume.lookup("missing"), Err(LookupError::NotFound));
    assert_eq!(volume.lookup("Sub Dir/missing"), Err(LookupError::NotFound));
    assert_eq!(volume.lookup("label"), Err(LookupError::NotFound));
    assert_eq!(volume.lookup(".."), Err(LookupError::NotFound));
    assert_eq!(
        volume.lookup("Long Name.txt/x"),
        Err(LookupError::NotADirectory)
    );
    for path in ["", "/", "\\", "///"] {
        assert_eq!(volume.lookup(path), Err(LookupError::NoName), "{path:?}");
    }
    assert_eq!(volume.find(0, ""), Ok(None));
}

#[test]
fn a_refused_long_name_run_still_leaves_the_short_name_findable() {
    let mut image = Image::fat12();
    let mut slots = [[0u8; 32]; 4];
    let count = file("Stale Name.txt", b"STALEN~1TXT", 9, 1, &mut slots);
    slots[count - 1][..11].copy_from_slice(b"RENAMED TXT");
    put(&mut image, 5 * 512, &slots[..count]);
    let mut volume = Volume::open(image).unwrap();
    assert!(volume.lookup("renamed.txt").is_ok());
    assert_eq!(volume.lookup("Stale Name.txt"), Err(LookupError::NotFound));
}

#[test]
fn a_device_failure_is_the_lookups_error() {
    let mut image = Image::fat12();
    image.refuse = Some(5);
    let mut volume = Volume::open(image).unwrap();
    assert_eq!(
        volume.lookup("x"),
        Err(LookupError::Chain(ChainError::Device(5)))
    );
}
