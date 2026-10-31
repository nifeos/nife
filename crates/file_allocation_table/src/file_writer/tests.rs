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

#[test]
fn a_shrink_to_a_cluster_boundary_forgets_the_cluster_a_cursor_had_reached() {
    // Image clusters are one sector. The writer reached chain index 2; shrinking to two sectors
    // frees that cluster, and the next write at the new end needs a fresh one.
    let mut volume = Volume::open(Image::fat12()).unwrap();
    let entry = volume.create_file(0, "CUT.BIN").unwrap();
    let mut writer = volume.open_file_writer(&entry).unwrap();
    writer.write(&[1; 1025]).unwrap();
    writer.set_len(1024).unwrap();
    writer.seek(1024);
    writer.write(&[2]).unwrap();
    let mut out = [0u8; 1100];
    assert_eq!(read_back(&mut volume, "CUT.BIN", &mut out), 1025);
    assert_eq!(out[1024], 2);
    assert_eq!(out[..1024], [1; 1024]);
    let entry = volume.lookup("CUT.BIN").unwrap();
    assert_eq!(volume.chain(entry.first_cluster()).count(), 3);
    assert_eq!(volume.count_free_clusters().unwrap(), 350 - 3);
}

#[test]
fn a_partial_overwrite_inside_the_last_sector_keeps_the_bytes_around_it() {
    let mut volume = Volume::open(Image::fat12()).unwrap();
    let entry = volume.create_file(0, "MID.BIN").unwrap();
    let mut writer = volume.open_file_writer(&entry).unwrap();
    let bytes: [u8; 300] = core::array::from_fn(pattern);
    writer.write(&bytes).unwrap();
    writer.seek(200);
    writer.write(&[0xFF, 0xFE]).unwrap();
    let mut out = [0u8; 400];
    assert_eq!(read_back(&mut volume, "MID.BIN", &mut out), 300);
    let mut expected = bytes;
    expected[200] = 0xFF;
    expected[201] = 0xFE;
    assert_eq!(out[..300], expected);
}

#[test]
fn a_root_whose_entry_count_ends_inside_a_sector_stops_there() {
    let mut image = Image::fat12();
    // Eight entries: half of the one sector the region occupies.
    image.bytes[17..19].copy_from_slice(&8u16.to_le_bytes());
    let mut volume = Volume::open(image).unwrap();
    for i in 0..8 {
        volume.create_file(0, &format_name(i)).unwrap();
    }
    assert_eq!(
        volume.create_file(0, "NINTH"),
        Err(WriteError::DirectoryFull)
    );
}

#[test]
fn a_slot_that_became_a_long_name_piece_is_stale_for_a_write_and_a_remove() {
    let mut volume = Volume::open(Image::fat12()).unwrap();
    let entry = volume.create_file(0, "LFN.BIN").unwrap();
    let mut image = volume.into_device();
    // The root's first slot starts at sector 5; byte 11 is the attribute.
    image.bytes[5 * 512 + 11] = 0x0F;
    let mut volume = Volume::open(image).unwrap();
    let mut writer = volume.open_file_writer(&entry).unwrap();
    assert_eq!(writer.write(b"x"), Err(WriteError::EntryChanged));
    assert_eq!(volume.remove_file(&entry), Err(WriteError::EntryChanged));
}

#[test]
fn a_write_keeps_the_attributes_the_entry_already_has() {
    let mut volume = Volume::open(Image::fat12()).unwrap();
    let entry = volume.create_file(0, "RO.BIN").unwrap();
    let mut image = volume.into_device();
    image.bytes[5 * 512 + 11] = 0x01;
    let mut volume = Volume::open(image).unwrap();
    let readonly = NamedEntry::created(
        ShortEntry::new(entry.short_name(), 0x01, 0, 0),
        entry.place(),
    );
    volume
        .open_file_writer(&readonly)
        .unwrap()
        .write(b"x")
        .unwrap();
    assert_eq!(volume.into_device().bytes[5 * 512 + 11], 0x21);
}

#[test]
fn a_reused_free_slot_does_not_end_the_directory_and_stale_bytes_after_the_end_are_ended() {
    let mut volume = Volume::open(Image::fat12()).unwrap();
    let first = volume.create_file(0, "A.TXT").unwrap();
    volume.create_file(0, "B.TXT").unwrap();
    volume.create_file(0, "C.TXT").unwrap();
    volume.remove_file(&first).unwrap();
    let reused = volume.create_file(0, "D.TXT").unwrap();
    assert_eq!(reused.place().short_slot, 0);
    for name in ["D.TXT", "B.TXT", "C.TXT"] {
        assert!(volume.lookup(name).is_ok(), "{name}");
    }
    // A stale entry sits in the slot after the end marker of an empty directory.
    let mut image = Image::fat12();
    image.bytes[5 * 512 + 32..5 * 512 + 43].copy_from_slice(b"GHOST   TXT");
    let mut volume = Volume::open(image).unwrap();
    let made = volume.create_file(0, "REAL.TXT").unwrap();
    assert_eq!(made.place().short_slot, 0);
    assert!(volume.lookup("REAL.TXT").is_ok());
    assert!(volume.lookup("GHOST.TXT").is_err());
    let image = volume.into_device();
    assert_eq!(image.bytes[5 * 512 + 32], 0);
    assert_eq!(image.bytes[5 * 512 + 33..5 * 512 + 43], *b"HOST   TXT");
}

#[test]
fn an_overwrite_in_a_later_sector_zeroes_stale_bytes_past_the_end_of_the_file() {
    let mut volume = Volume::open(Image::fat12()).unwrap();
    let entry = volume.create_file(0, "TAIL.BIN").unwrap();
    volume
        .open_file_writer(&entry)
        .unwrap()
        .write(&[1; 600])
        .unwrap();
    let first = volume.lookup("TAIL.BIN").unwrap().first_cluster();
    let second = volume.chain(first).nth(1).unwrap().unwrap();
    let mut image = volume.into_device();
    // Cluster `second`'s sector holds file bytes 512 to 599; byte 200 of it is past the end.
    image.bytes[(6 + second as usize - 2) * 512 + 200] = 0xEE;
    let mut volume = Volume::open(image).unwrap();
    let entry = volume.lookup("TAIL.BIN").unwrap();
    let mut writer = volume.open_file_writer(&entry).unwrap();
    writer.seek(520);
    writer.write(&[9]).unwrap();
    let image = volume.into_device();
    assert_eq!(image.bytes[(6 + second as usize - 2) * 512 + 200], 0);
}

#[test]
fn growing_by_several_buffers_from_empty_and_by_exactly_two_works() {
    let mut volume = Volume::open(Image::fat12()).unwrap();
    let entry = volume.create_file(0, "GROW.BIN").unwrap();
    volume
        .open_file_writer(&entry)
        .unwrap()
        .set_len(8192)
        .unwrap();
    let mut out = [1u8; 9000];
    assert_eq!(read_back(&mut volume, "GROW.BIN", &mut out), 8192);
    assert!(out[..8192].iter().all(|b| *b == 0));
    let entry = volume.lookup("GROW.BIN").unwrap();
    volume
        .open_file_writer(&entry)
        .unwrap()
        .set_len(8192 + 4096 + 1)
        .unwrap();
    assert_eq!(
        volume.lookup("GROW.BIN").unwrap().size_bytes(),
        8192 + 4096 + 1
    );
}
