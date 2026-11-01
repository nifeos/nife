//! Every stamped field, and every operation that stamps or does not, on the small FAT12 volume
//! `reader::tests` builds, with a clock the test sets and counts.
//!
//! Name: provisional.

use core::cell::Cell;

use super::*;
use crate::reader::tests::Image;
use crate::time::{NoClock, Timestamp};

/// A clock the test sets between operations and reads the count of.
struct Shared<'a> {
    time: &'a Cell<Option<Timestamp>>,
    readings: &'a Cell<u32>,
}

impl Clock for Shared<'_> {
    fn now(&mut self) -> Option<Timestamp> {
        self.readings.set(self.readings.get() + 1);
        self.time.get()
    }
}

fn at(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Option<Timestamp> {
    Some(Timestamp {
        year,
        month,
        day,
        hour,
        minute,
        second,
    })
}

/// The root directory's first sector in the test image.
const ROOT: usize = 5 * 512;

/// The six stamped fields of the 32-byte slot at `start`: tenths, creation time and date, access
/// date, write time and date.
fn fields(bytes: &[u8], start: usize) -> (u8, u16, u16, u16, u16, u16) {
    let word = |at: usize| u16::from_le_bytes([bytes[start + at], bytes[start + at + 1]]);
    (
        bytes[start + 13],
        word(14),
        word(16),
        word(18),
        word(22),
        word(24),
    )
}

fn packed(time: Option<Timestamp>) -> (u8, u16, u16, u16, u16, u16) {
    let s = Stamp::pack(time).unwrap();
    (
        s.creation_tenths,
        s.creation_time,
        s.creation_date,
        s.access_date,
        s.write_time,
        s.write_date,
    )
}

#[test]
fn a_new_file_is_created_written_and_accessed_at_the_time_of_its_creation() {
    let (time, readings) = (Cell::new(at(2026, 11, 1, 12, 34, 57)), Cell::new(0));
    let mut volume = WritableVolume::open(
        Image::fat12(),
        Shared {
            time: &time,
            readings: &readings,
        },
    )
    .unwrap();
    volume.create_file(0, "NEW.TXT").unwrap();
    assert_eq!(readings.get(), 1, "one reading for the operation");
    let image = volume.into_device();
    assert_eq!(fields(&image.bytes, ROOT), packed(time.get()));
    assert_ne!(fields(&image.bytes, ROOT).1, 0);
}

#[test]
fn a_write_and_a_resize_set_the_write_and_access_times_and_keep_the_creation_time() {
    let (time, readings) = (Cell::new(at(2026, 11, 1, 12, 34, 57)), Cell::new(0));
    let mut volume = WritableVolume::open(
        Image::fat12(),
        Shared {
            time: &time,
            readings: &readings,
        },
    )
    .unwrap();
    let entry = volume.create_file(0, "W.TXT").unwrap();
    time.set(at(2027, 2, 3, 4, 5, 6));
    let before = readings.get();
    let mut writer = volume.open_file_writer(&entry).unwrap();
    assert_eq!(readings.get(), before, "opening a writer reads no clock");
    writer.write(b"x").unwrap();
    assert_eq!(readings.get(), before + 1);
    time.set(at(2028, 3, 4, 5, 6, 8));
    let entry = volume.lookup("W.TXT").unwrap();
    let mut writer = volume.open_file_writer(&entry).unwrap();
    writer.set_len(0).unwrap();
    let image = volume.into_device();
    let (tenths, created_time, created_date, access, write_time, write_date) =
        fields(&image.bytes, ROOT);
    let created = packed(at(2026, 11, 1, 12, 34, 57));
    let last = packed(at(2028, 3, 4, 5, 6, 8));
    assert_eq!(
        (tenths, created_time, created_date),
        (created.0, created.1, created.2)
    );
    assert_eq!((access, write_time, write_date), (last.3, last.4, last.5));
}

#[test]
fn a_write_of_no_bytes_and_a_resize_to_the_same_length_stamp_nothing() {
    let (time, readings) = (Cell::new(at(2026, 11, 1, 12, 34, 56)), Cell::new(0));
    let mut volume = WritableVolume::open(
        Image::fat12(),
        Shared {
            time: &time,
            readings: &readings,
        },
    )
    .unwrap();
    let entry = volume.create_file(0, "Z.TXT").unwrap();
    let before = fields(&volume.into_device().bytes, ROOT);
    let mut volume = WritableVolume::open(
        Image::fat12(),
        Shared {
            time: &time,
            readings: &readings,
        },
    )
    .unwrap();
    let entry2 = volume.create_file(0, "Z.TXT").unwrap();
    assert_eq!(entry2.place(), entry.place());
    time.set(at(2030, 1, 1, 1, 1, 2));
    let mut writer = volume.open_file_writer(&entry2).unwrap();
    writer.write(b"").unwrap();
    writer.set_len(0).unwrap();
    assert_eq!(fields(&volume.into_device().bytes, ROOT), before);
}

#[test]
fn no_time_stamps_zero_and_a_modify_zeroes_only_the_write_and_access_fields() {
    let (time, readings) = (Cell::new(at(2026, 11, 1, 12, 34, 57)), Cell::new(0));
    let mut volume = WritableVolume::open(
        Image::fat12(),
        Shared {
            time: &time,
            readings: &readings,
        },
    )
    .unwrap();
    let entry = volume.create_file(0, "N.TXT").unwrap();
    time.set(None);
    volume
        .open_file_writer(&entry)
        .unwrap()
        .write(b"x")
        .unwrap();
    let image = volume.into_device();
    let (tenths, created_time, created_date, access, write_time, write_date) =
        fields(&image.bytes, ROOT);
    let created = packed(at(2026, 11, 1, 12, 34, 57));
    assert_eq!(
        (tenths, created_time, created_date),
        (created.0, created.1, created.2)
    );
    assert_eq!((access, write_time, write_date), (0, 0, 0));
    // And NoClock writes the zero timestamp from the start.
    let mut volume = WritableVolume::open(Image::fat12(), NoClock).unwrap();
    volume.create_file(0, "Z.TXT").unwrap();
    assert_eq!(
        fields(&volume.into_device().bytes, ROOT),
        (0, 0, 0, 0, 0, 0)
    );
}

#[test]
fn a_directory_and_its_dot_entries_carry_the_creation_time() {
    let (time, readings) = (Cell::new(at(2026, 11, 1, 12, 34, 57)), Cell::new(0));
    let mut volume = WritableVolume::open(
        Image::fat12(),
        Shared {
            time: &time,
            readings: &readings,
        },
    )
    .unwrap();
    let dir = volume.make_directory(0, "D").unwrap();
    assert_eq!(readings.get(), 1);
    let cluster = dir.first_cluster() as usize;
    let image = volume.into_device();
    let want = packed(time.get());
    assert_eq!(fields(&image.bytes, ROOT), want);
    let dir_sector = (6 + cluster - 2) * 512;
    assert_eq!(fields(&image.bytes, dir_sector), want, "`.`");
    assert_eq!(fields(&image.bytes, dir_sector + 32), want, "`..`");
}

#[test]
fn a_rename_keeps_the_times_and_a_label_takes_the_time_it_was_set() {
    let (time, readings) = (Cell::new(at(2026, 11, 1, 12, 34, 57)), Cell::new(0));
    let mut volume = WritableVolume::open(
        Image::fat12(),
        Shared {
            time: &time,
            readings: &readings,
        },
    )
    .unwrap();
    let entry = volume.create_file(0, "OLD.TXT").unwrap();
    time.set(at(2031, 5, 6, 7, 8, 10));
    let before = readings.get();
    let moved = volume.rename(&entry, 0, "A new longer name.txt").unwrap();
    volume.set_attributes(&moved, Attributes::HIDDEN).unwrap();
    assert_eq!(
        readings.get(),
        before,
        "a rename and an attribute change read no clock"
    );
    volume.set_volume_label("LABEL").unwrap();
    assert_eq!(readings.get(), before + 1);
    let image = volume.into_device();
    // The renamed entry sits after its long-name run (two pieces), at slot 3, and kept its times.
    let short_slot = moved.place().short_slot as usize;
    assert_eq!(
        fields(&image.bytes, ROOT + short_slot * 32),
        packed(at(2026, 11, 1, 12, 34, 57))
    );
    // The label took the new time, in the slot the renamed entry left free.
    assert_eq!(image.bytes[ROOT + 11], 0x08);
    assert_eq!(fields(&image.bytes, ROOT), packed(time.get()));
}

#[test]
fn reads_and_removals_read_no_clock_and_leave_the_bytes_alone() {
    let (time, readings) = (Cell::new(at(2026, 11, 1, 12, 34, 57)), Cell::new(0));
    let mut volume = WritableVolume::open(
        Image::fat12(),
        Shared {
            time: &time,
            readings: &readings,
        },
    )
    .unwrap();
    let entry = volume.create_file(0, "R.TXT").unwrap();
    volume
        .open_file_writer(&entry)
        .unwrap()
        .write(b"data")
        .unwrap();
    let after_writes = readings.get();
    let found = volume.lookup("R.TXT").unwrap();
    let mut out = [0u8; 8];
    assert_eq!(volume.open_file(&found).unwrap().read(&mut out).unwrap(), 4);
    for _ in volume.root_entries() {}
    assert_eq!(volume.volume_label(), Ok(None));
    assert_eq!(readings.get(), after_writes, "reading takes no clock");
    let image = volume.into_volume().into_device();
    let before = fields(&image.bytes, ROOT);
    let mut volume = WritableVolume::open(
        image,
        Shared {
            time: &time,
            readings: &readings,
        },
    )
    .unwrap();
    let found = volume.lookup("R.TXT").unwrap();
    volume.open_file(&found).unwrap().read(&mut out).unwrap();
    assert_eq!(
        fields(&volume.into_device().bytes, ROOT),
        before,
        "a read stamps no access date"
    );
    let mut volume = WritableVolume::open(
        Image::fat12(),
        Shared {
            time: &time,
            readings: &readings,
        },
    )
    .unwrap();
    let doomed = volume.create_file(0, "D.TXT").unwrap();
    let count = readings.get();
    volume.remove_file(&doomed).unwrap();
    assert_eq!(readings.get(), count, "a removal reads no clock");
}

#[test]
fn the_volume_without_a_clock_cannot_write_and_the_clock_is_given_back_with_the_device() {
    let mut volume = Volume::open(Image::fat12()).unwrap();
    assert_eq!(volume.volume_label(), Ok(None));
    let writable = WritableVolume::open(volume.into_device(), NoClock).unwrap();
    let mut read_only = writable.into_volume();
    assert_eq!(read_only.count_free_clusters(), Ok(350));
}

/// The times FAT cannot store that the refusal tests try.
fn bad_times() -> [Option<Timestamp>; 5] {
    [
        at(1979, 12, 31, 23, 59, 58),
        at(2108, 1, 1, 0, 0, 0),
        at(2026, 13, 1, 0, 0, 0),
        at(2026, 1, 1, 24, 0, 0),
        at(2026, 1, 1, 0, 60, 0),
    ]
}

/// Run `operation` on a volume holding one file, with a clock that reports a time FAT cannot
/// store, and check it is refused and left every byte of the image as it was.
fn refused_and_untouched(
    operation: impl Fn(&mut WritableVolume<Image, Shared<'_>>) -> Result<(), WriteError<u64>>,
) {
    let (time, readings) = (Cell::new(at(2026, 11, 1, 12, 34, 56)), Cell::new(0));
    for bad in bad_times() {
        time.set(at(2026, 11, 1, 12, 34, 56));
        let mut volume = WritableVolume::open(
            Image::fat12(),
            Shared {
                time: &time,
                readings: &readings,
            },
        )
        .unwrap();
        let file = volume.create_file(0, "KEEP.TXT").unwrap();
        volume
            .open_file_writer(&file)
            .unwrap()
            .write(b"keep")
            .unwrap();
        volume.sync().unwrap();
        let image = volume.into_device();
        let before = image.bytes;
        let mut volume = WritableVolume::open(
            image,
            Shared {
                time: &time,
                readings: &readings,
            },
        )
        .unwrap();
        time.set(bad);
        assert_eq!(
            operation(&mut volume),
            Err(WriteError::TimeOutOfRange),
            "{bad:?}"
        );
        assert!(
            volume.into_device().bytes == before,
            "an out-of-range time wrote bytes ({bad:?})"
        );
    }
}

#[test]
fn creating_a_file_at_a_time_fat_cannot_store_is_refused_and_writes_nothing() {
    refused_and_untouched(|volume| volume.create_file(0, "NEW.TXT").map(|_| ()));
    refused_and_untouched(|volume| {
        volume
            .create_file(0, "A long new file name.txt")
            .map(|_| ())
    });
}

#[test]
fn making_a_directory_at_a_time_fat_cannot_store_is_refused_and_writes_nothing() {
    refused_and_untouched(|volume| volume.make_directory(0, "NEWDIR").map(|_| ()));
}

#[test]
fn setting_the_label_at_a_time_fat_cannot_store_is_refused_and_writes_nothing() {
    refused_and_untouched(|volume| volume.set_volume_label("LABEL"));
}

#[test]
fn writing_at_a_time_fat_cannot_store_is_refused_and_writes_nothing() {
    refused_and_untouched(|volume| {
        let entry = volume.lookup("KEEP.TXT").unwrap();
        volume
            .open_file_writer(&entry)
            .unwrap()
            .write(b"more bytes")
            .map(|_| ())
    });
    // Even a write of no bytes reads the clock first, and is refused the same way.
    refused_and_untouched(|volume| {
        let entry = volume.lookup("KEEP.TXT").unwrap();
        volume
            .open_file_writer(&entry)
            .unwrap()
            .write(b"")
            .map(|_| ())
    });
}

#[test]
fn resizing_at_a_time_fat_cannot_store_is_refused_and_writes_nothing() {
    refused_and_untouched(|volume| {
        let entry = volume.lookup("KEEP.TXT").unwrap();
        volume.open_file_writer(&entry).unwrap().set_len(2000)
    });
    refused_and_untouched(|volume| {
        let entry = volume.lookup("KEEP.TXT").unwrap();
        volume.open_file_writer(&entry).unwrap().set_len(0)
    });
}

#[test]
fn a_refused_writer_stays_usable_once_the_clock_is_right() {
    let (time, readings) = (Cell::new(at(2026, 11, 1, 12, 34, 56)), Cell::new(0));
    let mut volume = WritableVolume::open(
        Image::fat12(),
        Shared {
            time: &time,
            readings: &readings,
        },
    )
    .unwrap();
    let file = volume.create_file(0, "W.TXT").unwrap();
    let mut writer = volume.open_file_writer(&file).unwrap();
    time.set(at(2108, 1, 1, 0, 0, 0));
    assert_eq!(writer.write(b"x"), Err(WriteError::TimeOutOfRange));
    assert_eq!(writer.set_len(5), Err(WriteError::TimeOutOfRange));
    time.set(at(2026, 11, 2, 0, 0, 0));
    assert_eq!(writer.write(b"x"), Ok(1));
}

#[test]
fn the_boundaries_through_a_volume_refuse_one_second_outside_and_accept_the_edge() {
    let (time, readings) = (Cell::new(None), Cell::new(0));
    let mut volume = WritableVolume::open(
        Image::fat12(),
        Shared {
            time: &time,
            readings: &readings,
        },
    )
    .unwrap();
    for (moment, ok, name) in [
        (at(1979, 12, 31, 23, 59, 58), false, "A.TXT"),
        (at(1980, 1, 1, 0, 0, 0), true, "B.TXT"),
        (at(2107, 12, 31, 23, 59, 58), true, "C.TXT"),
        (at(2108, 1, 1, 0, 0, 0), false, "D.TXT"),
    ] {
        time.set(moment);
        assert_eq!(volume.create_file(0, name).is_ok(), ok, "{moment:?}");
    }
    // A clock with no time is never out of range.
    time.set(None);
    assert!(volume.create_file(0, "E.TXT").is_ok());
}

#[test]
fn the_format_and_the_efi_volume_refuse_a_time_fat_cannot_store() {
    use crate::{Format, FormatError, FormatOptions, LayoutError, SectorSize, VolumeLayout};
    let (time, readings) = (Cell::new(at(2108, 1, 1, 0, 0, 0)), Cell::new(0));
    let mut clock = Shared {
        time: &time,
        readings: &readings,
    };
    let options = FormatOptions::new(SectorSize::Bytes512, 16 * 1024);
    assert_eq!(
        Format::new(options, &mut clock).err(),
        Some(FormatError::TimeOutOfRange)
    );
    assert_eq!(
        VolumeLayout::new(1024 * 1024, 0, 4096, 1, "BOOTX64.EFI", &mut clock).err(),
        Some(LayoutError::TimeOutOfRange)
    );
    time.set(at(2107, 12, 31, 23, 59, 58));
    assert!(Format::new(options, &mut clock).is_ok());
}
