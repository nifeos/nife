//! Files created, written, resized and deleted on the small FAT12 volume `reader::tests` builds:
//! odd chunk sizes across sector boundaries, slack zeroed, a position past the end clamped, a full
//! root, a stale entry, and a device that refuses a write.
//!
//! Name: provisional.

use super::*;
use crate::reader::tests::Image;

/// A byte that tells its file offset.
fn pattern(offset: usize) -> u8 {
    (offset * 31 + 7) as u8
}

/// Read the whole file `name` from the root of `volume`.
fn read_back(volume: &mut Volume<Image>, name: &str, out: &mut [u8]) -> usize {
    let entry = volume.lookup(name).expect("the file is listed");
    let mut reader = volume.open_file(&entry).unwrap();
    let mut total = 0;
    loop {
        let got = reader.read(&mut out[total..]).expect("the file reads");
        if got == 0 {
            return total;
        }
        total += got;
    }
}

#[test]
fn a_file_written_in_odd_chunks_reads_back_and_its_entry_is_true() {
    for chunk in [1usize, 7, 100, 511, 512, 513, 1500] {
        let mut volume = Volume::open(Image::fat12()).unwrap();
        let entry = volume.create_file(0, "DATA.BIN").unwrap();
        let mut writer = volume.open_file_writer(&entry).unwrap();
        let bytes: [u8; 1500] = core::array::from_fn(pattern);
        for piece in bytes.chunks(chunk) {
            assert_eq!(writer.write(piece), Ok(piece.len()));
        }
        assert_eq!((writer.size_bytes(), writer.position()), (1500, 1500));
        let mut out = [0u8; 2000];
        assert_eq!(
            read_back(&mut volume, "DATA.BIN", &mut out),
            1500,
            "chunk {chunk}"
        );
        assert_eq!(out[..1500], bytes);
        let entry = volume.lookup("DATA.BIN").unwrap();
        assert_eq!(entry.size_bytes(), 1500);
        assert_eq!(volume.chain(entry.first_cluster()).count(), 3);
        assert_eq!(entry.attributes() & 0x20, 0x20);
    }
}

#[test]
fn an_overwrite_keeps_the_rest_and_a_partial_sector_zeroes_the_slack() {
    let mut image = Image::fat12();
    // Stale bytes where the new file's first cluster will be.
    image.bytes[6 * 512..7 * 512].fill(0xEE);
    let mut volume = Volume::open(image).unwrap();
    let entry = volume.create_file(0, "A.BIN").unwrap();
    let mut writer = volume.open_file_writer(&entry).unwrap();
    writer.write(&[1; 10]).unwrap();
    writer.seek(4);
    writer.write(&[2, 2]).unwrap();
    assert_eq!((writer.size_bytes(), writer.position()), (10, 6));
    let mut image_bytes = [0u8; 16];
    assert_eq!(read_back(&mut volume, "A.BIN", &mut image_bytes), 10);
    assert_eq!(image_bytes[..10], [1, 1, 1, 1, 2, 2, 1, 1, 1, 1]);
    let image = volume.into_device();
    assert!(image.bytes[6 * 512 + 10..7 * 512].iter().all(|b| *b == 0));
}

#[test]
fn a_seek_past_the_end_stops_at_the_end() {
    let mut volume = Volume::open(Image::fat12()).unwrap();
    let entry = volume.create_file(0, "S.BIN").unwrap();
    let mut writer = volume.open_file_writer(&entry).unwrap();
    writer.seek(99);
    assert_eq!(writer.position(), 0);
    writer.write(b"abc").unwrap();
    writer.seek(99);
    assert_eq!(writer.position(), 3);
}

#[test]
fn set_len_grows_with_zeros_and_shrinks_by_freeing_clusters() {
    let mut volume = Volume::open(Image::fat12()).unwrap();
    let entry = volume.create_file(0, "L.BIN").unwrap();
    let mut writer = volume.open_file_writer(&entry).unwrap();
    writer.write(&[9; 600]).unwrap();
    writer.set_len(1200).unwrap();
    assert_eq!((writer.size_bytes(), writer.position()), (1200, 600));
    let mut out = [0u8; 1300];
    assert_eq!(read_back(&mut volume, "L.BIN", &mut out), 1200);
    assert!(out[..600].iter().all(|b| *b == 9));
    assert!(out[600..1200].iter().all(|b| *b == 0));
    let free = volume.count_free_clusters().unwrap();
    let entry = volume.lookup("L.BIN").unwrap();
    let mut writer = volume.open_file_writer(&entry).unwrap();
    writer.seek(1200);
    writer.set_len(513).unwrap();
    assert_eq!((writer.size_bytes(), writer.position()), (513, 513));
    writer.set_len(0).unwrap();
    assert_eq!(volume.count_free_clusters().unwrap(), free + 3);
    let entry = volume.lookup("L.BIN").unwrap();
    assert_eq!((entry.first_cluster(), entry.size_bytes()), (0, 0));
    // The emptied file takes a new chain.
    let mut writer = volume.open_file_writer(&entry).unwrap();
    writer.write(b"again").unwrap();
    assert_ne!(volume.lookup("L.BIN").unwrap().first_cluster(), 0);
}

#[test]
fn a_removed_file_frees_its_slots_and_clusters_and_a_new_file_takes_the_slot() {
    let mut volume = Volume::open(Image::fat12()).unwrap();
    let free = volume.count_free_clusters().unwrap();
    let entry = volume.create_file(0, "GONE.BIN").unwrap();
    volume
        .open_file_writer(&entry)
        .unwrap()
        .write(&[1; 1000])
        .unwrap();
    let entry = volume.lookup("GONE.BIN").unwrap();
    volume.remove_file(&entry).unwrap();
    assert_eq!(volume.count_free_clusters().unwrap(), free);
    assert!(volume.lookup("GONE.BIN").is_err());
    let again = volume.create_file(0, "NEW.BIN").unwrap();
    assert_eq!(again.place().short_slot, 0);
}

#[test]
fn a_directory_or_label_is_not_a_file_to_write_or_remove() {
    let mut volume = Volume::open(Image::fat12()).unwrap();
    let entry = volume.create_file(0, "X.BIN").unwrap();
    let dir = NamedEntry::created(ShortEntry::new(*b"DIR        ", 0x10, 0, 0), entry.place());
    assert_eq!(volume.open_file_writer(&dir).err(), Some(NotAFile));
    assert_eq!(volume.remove_file(&dir), Err(WriteError::NotAFile));
}

#[test]
fn the_fixed_root_fills_and_a_refused_write_is_the_devices_error() {
    let mut volume = Volume::open(Image::fat12()).unwrap();
    for i in 0..16 {
        volume.create_file(0, &format_name(i)).unwrap();
    }
    assert_eq!(
        volume.create_file(0, "ONEMORE"),
        Err(WriteError::DirectoryFull)
    );

    let mut image = Image::fat12();
    image.refuse_write = Some(6);
    let mut volume = Volume::open(image).unwrap();
    let entry = volume.create_file(0, "E.BIN").unwrap();
    let mut writer = volume.open_file_writer(&entry).unwrap();
    assert_eq!(
        writer.write(b"x"),
        Err(WriteError::Chain(ChainError::Device(6)))
    );
    assert_eq!(writer.write(b"x"), Err(WriteError::AfterAFailure));
    assert_eq!(writer.set_len(5), Err(WriteError::AfterAFailure));
}

/// `F0` to `F15` as short names.
fn format_name(i: u32) -> heapless_name::Name {
    heapless_name::Name::new(i)
}

mod heapless_name {
    /// A short name built without an allocator.
    pub(super) struct Name {
        bytes: [u8; 4],
        len: usize,
    }

    impl Name {
        pub(super) fn new(i: u32) -> Name {
            let mut bytes = [b'F', 0, 0, 0];
            let len = if i < 10 {
                bytes[1] = b'0' + i as u8;
                2
            } else {
                bytes[1] = b'0' + (i / 10) as u8;
                bytes[2] = b'0' + (i % 10) as u8;
                3
            };
            Name { bytes, len }
        }
    }

    impl core::ops::Deref for Name {
        type Target = str;

        fn deref(&self) -> &str {
            core::str::from_utf8(&self.bytes[..self.len]).unwrap()
        }
    }
}

#[test]
fn a_damaged_chain_is_reported_not_extended() {
    let mut image = Image::fat12();
    // A file claiming 1000 bytes with a one-cluster chain.
    image.set(5, 0xFFF);
    let mut volume = Volume::open(image).unwrap();
    let entry = volume.create_file(0, "D.BIN").unwrap();
    let damaged = NamedEntry::created(
        ShortEntry::new(entry.short_name(), 0x20, 5, 1000),
        entry.place(),
    );
    let mut writer = volume.open_file_writer(&damaged).unwrap();
    writer.seek(0);
    assert_eq!(writer.write(b"x"), Ok(1));
    writer.seek(u32::MAX);
    assert_eq!(writer.write(b"y"), Err(WriteError::ShorterThanItsSize));
}

#[test]
fn the_dirty_bit_is_set_by_the_first_change_and_the_sync_restores_it() {
    let mut volume = Volume::open(Image::fat12()).unwrap();
    // FAT12 keeps no bit.
    assert_eq!(volume.is_clean(), Ok(None));
    volume.allocate_cluster(None).unwrap();
    assert_eq!(volume.sync(), Ok(()));
}

#[test]
fn a_write_after_seeking_back_and_forth_lands_where_it_says() {
    let mut volume = Volume::open(Image::fat12()).unwrap();
    let entry = volume.create_file(0, "SEEK.BIN").unwrap();
    let mut writer = volume.open_file_writer(&entry).unwrap();
    writer.write(&[0; 1400]).unwrap();
    for (at, byte) in [
        (1300u32, 1u8),
        (3, 2),
        (700, 3),
        (1399, 4),
        (512, 5),
        (511, 6),
    ] {
        writer.seek(at);
        writer.write(&[byte]).unwrap();
    }
    let mut out = [0u8; 1500];
    assert_eq!(read_back(&mut volume, "SEEK.BIN", &mut out), 1400);
    for (at, byte) in [
        (1300usize, 1u8),
        (3, 2),
        (700, 3),
        (1399, 4),
        (512, 5),
        (511, 6),
    ] {
        assert_eq!(out[at], byte, "offset {at}");
    }
    assert_eq!(out.iter().filter(|b| **b != 0).count(), 6);
    // A second writer starts with no cursor and follows the chain from its head.
    let entry = volume.lookup("SEEK.BIN").unwrap();
    let mut writer = volume.open_file_writer(&entry).unwrap();
    writer.seek(1300);
    writer.write(&[9]).unwrap();
    assert_eq!(read_back(&mut volume, "SEEK.BIN", &mut out), 1400);
    assert_eq!(out[1300], 9);
}

#[test]
fn a_chain_that_loops_or_leaves_the_volume_is_an_error_not_a_hang() {
    let mut image = Image::fat12();
    image.set(5, 9);
    image.set(9, 5);
    image.set(20, 400);
    let mut volume = Volume::open(image).unwrap();
    let entry = volume.create_file(0, "L.BIN").unwrap();
    for (first, error) in [
        (5, ChainError::TooLong),
        (
            20,
            ChainError::Broken {
                cluster: 20,
                entry: crate::TableEntry::Next(400),
            },
        ),
    ] {
        let looped = NamedEntry::created(
            ShortEntry::new(entry.short_name(), 0x20, first, 1_000_000),
            entry.place(),
        );
        let mut writer = volume.open_file_writer(&looped).unwrap();
        writer.seek(u32::MAX);
        assert_eq!(writer.write(b"x"), Err(WriteError::Chain(error)));
    }
}

#[test]
fn a_write_at_the_four_gibibyte_limit_is_refused_and_nothing_is_written() {
    let mut volume = Volume::open(Image::fat12()).unwrap();
    let boot = *volume.boot_sector();
    let mut state = WriteState::new(5, u32::MAX);
    state.seek(u32::MAX);
    let mut scratch = [0u8; 512];
    assert_eq!(
        state.write(&mut volume, &boot, b"x", &mut scratch),
        Err(WriteError::FileTooLarge)
    );
    assert_eq!(state.write(&mut volume, &boot, b"", &mut scratch), Ok(0));
}

#[test]
fn the_longest_and_shortest_short_names_are_legal_and_the_next_longer_is_not() {
    let mut volume = Volume::open(Image::fat12()).unwrap();
    for good in ["A", "ABCDEFGH", "A.B", "ABCDEFGH.XYZ", "$A-B_C~1.!#&"] {
        volume.create_file(0, good).expect(good);
    }
    for bad in ["ABCDEFGHI", "ABCDEFGH.XYZW"] {
        assert_eq!(
            volume.create_file(0, bad),
            Err(WriteError::InvalidName),
            "{bad}"
        );
    }
    assert_eq!(
        volume.lookup("ABCDEFGH.XYZ").unwrap().short_name(),
        *b"ABCDEFGHXYZ"
    );
}

#[test]
fn setting_the_length_it_already_has_changes_nothing() {
    let mut volume = Volume::open(Image::fat12()).unwrap();
    let entry = volume.create_file(0, "SAME.BIN").unwrap();
    let mut writer = volume.open_file_writer(&entry).unwrap();
    writer.write(&[3; 700]).unwrap();
    let free = volume.count_free_clusters().unwrap();
    let same = volume.lookup("SAME.BIN").unwrap();
    let mut writer = volume.open_file_writer(&same).unwrap();
    writer.set_len(700).unwrap();
    assert_eq!((writer.size_bytes(), writer.position()), (700, 0));
    assert_eq!(volume.count_free_clusters().unwrap(), free);
}

#[test]
fn an_entry_that_no_longer_matches_its_slot_is_stale_for_every_operation() {
    let mut volume = Volume::open(Image::fat12()).unwrap();
    let entry = volume.create_file(0, "OLD.BIN").unwrap();
    volume.remove_file(&entry).unwrap();
    let other = volume.create_file(0, "NEW.BIN").unwrap();
    assert_eq!(other.place(), entry.place());
    let mut writer = volume.open_file_writer(&entry).unwrap();
    assert_eq!(writer.write(b"x"), Err(WriteError::EntryChanged));
    assert_eq!(volume.remove_file(&entry), Err(WriteError::EntryChanged));
    // The live entry was not touched.
    assert_eq!(volume.lookup("NEW.BIN").unwrap().size_bytes(), 0);
}
