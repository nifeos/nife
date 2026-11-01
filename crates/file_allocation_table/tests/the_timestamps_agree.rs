//! **rust-fatfs and `fsck` agree with the times this crate stamps.**
//!
//! A fixed clock stamps files and directories on FAT12, FAT16 and FAT32 volumes rust-fatfs
//! formatted; rust-fatfs must read the creation, write and access times back as the clock gave
//! them, the odd second included, and `fsck -n` must accept every volume, including the edges. The format's label entry and the EFI volume's
//! directories carry the clock's time, and a volume made with `NoClock` carries none.

use std::cell::Cell;
use std::io::Cursor;
use std::path::Path;
use std::process::Command;

use file_allocation_table::{
    Clock, FormatOptions, FormatPlan, NoClock, SectorDevice, SectorSize, Timestamp, VolumeLayout,
    WritableSectorDevice, WritableVolume, WriteError,
};
use lamfat::FatType;

/// A clock the test sets between operations.
struct Shared<'a>(&'a Cell<Option<Timestamp>>);

impl Clock for Shared<'_> {
    fn now(&mut self) -> Option<Timestamp> {
        self.0.get()
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

/// A volume held in memory, read and written one sector at a time.
struct Memory {
    bytes: Vec<u8>,
    size: SectorSize,
}

impl SectorDevice for Memory {
    type Error = u64;

    fn sector_size(&self) -> SectorSize {
        self.size
    }

    fn read_sector(&mut self, index: u64, buffer: &mut [u8]) -> Result<(), u64> {
        let start = usize::try_from(index).map_err(|_| index)? * buffer.len();
        let bytes = self.bytes.get(start..start + buffer.len()).ok_or(index)?;
        buffer.copy_from_slice(bytes);
        Ok(())
    }
}

impl WritableSectorDevice for Memory {
    fn write_sector(&mut self, index: u64, buffer: &[u8]) -> Result<(), u64> {
        let start = usize::try_from(index).map_err(|_| index)? * buffer.len();
        let bytes = self
            .bytes
            .get_mut(start..start + buffer.len())
            .ok_or(index)?;
        bytes.copy_from_slice(buffer);
        Ok(())
    }
}

/// An empty volume of `mebibytes` that rust-fatfs formatted as `fat_type`.
fn formatted(mebibytes: usize, fat_type: lamfat::FatType, sector: u16) -> Memory {
    let mut image = vec![0u8; mebibytes << 20];
    lamfat::format_volume(
        &mut lamfat::StdIoWrapper::from(Cursor::new(&mut image)),
        lamfat::FormatVolumeOptions::new()
            .fat_type(fat_type)
            .bytes_per_sector(sector),
    )
    .expect("rust-fatfs formats the volume");
    Memory {
        bytes: image,
        size: SectorSize::from_bytes(sector).expect("a permitted sector size"),
    }
}

/// The `fsck` this host has, if any, and its name.
fn fsck() -> Option<&'static str> {
    [
        "fsck.fat",
        "/usr/sbin/fsck.fat",
        "/sbin/fsck.fat",
        "fsck_msdos",
    ]
    .into_iter()
    .find(|tool| Command::new(tool).arg("--help").output().is_ok())
}

/// Run the host's `fsck -n` on `image`, which must exit 0; report and skip when there is none.
fn check_with_fsck(image: &[u8], label: &str) {
    let Some(tool) = fsck() else {
        eprintln!("{label}: no fsck.fat or fsck_msdos on this host; the fsck check is skipped");
        return;
    };
    let path = std::env::temp_dir().join(format!(
        "file-allocation-table-{label}-{}.img",
        std::process::id()
    ));
    std::fs::write(&path, image).expect("write the image for fsck");
    let output = Command::new(tool)
        .arg("-n")
        .arg(&path)
        .output()
        .expect("run fsck");
    let _ = std::fs::remove_file(Path::new(&path));
    assert!(
        output.status.success(),
        "{label}: {tool} -n refused the image:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// (year, month, day, hour, minute, second as milliseconds) as rust-fatfs reads a creation time.
type Read = (u16, u16, u16, u16, u16, u32);

/// The numbers in rust-fatfs's `Debug` text of a time, in field order. Its time type names its
/// seconds and milliseconds fields with unit symbols this tree's naming ruling does not allow in
/// our own code, so the test reads the values from the text and not by field name.
fn numbers(text: String) -> Vec<u32> {
    text.split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .map(|part| part.parse().unwrap())
        .collect()
}

fn entry_of(image: &[u8], name: &str) -> (String, String, String) {
    let fs = lamfat::FileSystem::new(
        lamfat::StdIoWrapper::from(Cursor::new(image.to_vec())),
        lamfat::FsOptions::new(),
    )
    .unwrap();
    let entry = fs
        .root_dir()
        .iter()
        .map(|e| e.unwrap())
        .find(|e| e.file_name() == name)
        .unwrap();
    (
        format!("{:?}", entry.created()),
        format!("{:?}", entry.modified()),
        format!("{:?}", entry.accessed()),
    )
}

/// (year, month, day, hour, minute, second as milliseconds) as rust-fatfs reads a date and time.
fn read_time(text: String) -> Read {
    let n = numbers(text);
    (
        n[0] as u16,
        n[1] as u16,
        n[2] as u16,
        n[3] as u16,
        n[4] as u16,
        n[5] * 1000 + n[6],
    )
}

fn creation(image: &[u8], name: &str) -> Read {
    read_time(entry_of(image, name).0)
}

fn modified(image: &[u8], name: &str) -> Read {
    read_time(entry_of(image, name).1)
}

fn accessed(image: &[u8], name: &str) -> (u16, u16, u16) {
    let n = numbers(entry_of(image, name).2);
    (n[0] as u16, n[1] as u16, n[2] as u16)
}

fn times_agree(label: &str, mebibytes: usize, fat_type: FatType) {
    let now = Cell::new(at(2026, 11, 1, 12, 34, 57));
    let mut volume =
        WritableVolume::open(formatted(mebibytes, fat_type, 512), Shared(&now)).unwrap();
    volume
        .create_file(0, "Stamped File With A Long Name.txt")
        .unwrap();
    volume.create_directory(0, "Stamped Dir").unwrap();
    // A later write: the write and access times move, the creation time stays.
    now.set(at(2027, 2, 3, 4, 5, 6));
    let entry = volume.lookup("Stamped File With A Long Name.txt").unwrap();
    volume
        .open_file_writer(&entry)
        .unwrap()
        .write(b"later")
        .unwrap();
    volume.mark_clean().unwrap();
    let image = volume.into_device();
    let file = "Stamped File With A Long Name.txt";
    assert_eq!(
        creation(&image.bytes, file),
        (2026, 11, 1, 12, 34, 57_000),
        "{label}"
    );
    assert_eq!(
        modified(&image.bytes, file),
        (2027, 2, 3, 4, 5, 6_000),
        "{label}"
    );
    assert_eq!(accessed(&image.bytes, file), (2027, 2, 3), "{label}");
    assert_eq!(
        creation(&image.bytes, "Stamped Dir"),
        (2026, 11, 1, 12, 34, 57_000),
        "{label}"
    );
    assert_eq!(
        accessed(&image.bytes, "Stamped Dir"),
        (2026, 11, 1),
        "{label}"
    );
    check_with_fsck(&image.bytes, &format!("{label}-stamped"));
}

#[test]
fn times_agree_on_fat12() {
    times_agree("time-fat12", 4, FatType::Fat12);
}

#[test]
fn times_agree_on_fat16() {
    times_agree("time-fat16", 32, FatType::Fat16);
}

#[test]
fn times_agree_on_fat32() {
    times_agree("time-fat32", 64, FatType::Fat32);
}

#[test]
fn a_time_fat_cannot_store_is_refused_and_the_edges_pass_fsck() {
    for (label, time) in [
        ("before-1980", at(1979, 12, 31, 23, 59, 58)),
        ("after-2107", at(2108, 1, 1, 0, 0, 0)),
        ("month-13", at(2000, 13, 1, 0, 0, 0)),
        ("hour-99", at(2000, 1, 1, 99, 0, 0)),
    ] {
        let now = Cell::new(time);
        let volume = WritableVolume::open(formatted(4, FatType::Fat12, 512), Shared(&now)).unwrap();
        let before = volume.into_device();
        let mut volume = WritableVolume::open(
            Memory {
                bytes: before.bytes.clone(),
                size: before.size,
            },
            Shared(&now),
        )
        .unwrap();
        assert_eq!(
            volume.create_file(0, "REFUSED.TXT"),
            Err(WriteError::TimeOutOfRange),
            "{label}"
        );
        assert!(
            volume.into_device().bytes == before.bytes,
            "{label}: bytes were written"
        );
    }
    // The first and last moments FAT stores are written, read back and accepted by fsck.
    for (label, time, expected) in [
        ("first", at(1980, 1, 1, 0, 0, 0), (1980, 1, 1, 0, 0, 0)),
        (
            "last",
            at(2107, 12, 31, 23, 59, 58),
            (2107, 12, 31, 23, 59, 58_000),
        ),
    ] {
        let now = Cell::new(time);
        let mut volume =
            WritableVolume::open(formatted(4, FatType::Fat12, 512), Shared(&now)).unwrap();
        volume.create_file(0, "EDGE.TXT").unwrap();
        let image = volume.into_device();
        assert_eq!(creation(&image.bytes, "EDGE.TXT"), expected, "{label}");
        check_with_fsck(&image.bytes, &format!("edge-{label}"));
    }
}

#[test]
fn no_clock_and_a_clock_with_no_time_write_the_zero_timestamp() {
    let none = Cell::new(None);
    let mut with_empty =
        WritableVolume::open(formatted(4, FatType::Fat12, 512), Shared(&none)).unwrap();
    with_empty.create_file(0, "ZERO.TXT").unwrap();
    let mut with_no_clock =
        WritableVolume::open(formatted(4, FatType::Fat12, 512), NoClock).unwrap();
    with_no_clock.create_file(0, "ZERO.TXT").unwrap();
    let (a, b) = (with_empty.into_device(), with_no_clock.into_device());
    assert_eq!(a.bytes, b.bytes, "a clock with no time is NoClock");
    check_with_fsck(&a.bytes, "zero-timestamp");
}

#[test]
fn the_formats_label_and_the_efi_volumes_directories_carry_the_clocks_time() {
    let now = Cell::new(at(2026, 11, 1, 12, 34, 57));
    let options = FormatOptions::new(SectorSize::Bytes512, 16 * 1024)
        .with_label("STAMPED")
        .unwrap();
    let format = FormatPlan::new(options, &mut Shared(&now)).unwrap();
    let mut image = Memory {
        bytes: vec![0u8; 16 * 1024 * 512],
        size: SectorSize::Bytes512,
    };
    format.write_to(&mut image).unwrap();
    let root = (1 + 2 * format.table_sectors() as usize) * 512;
    let word = |at: usize| u16::from_le_bytes([image.bytes[root + at], image.bytes[root + at + 1]]);
    assert_eq!(image.bytes[root + 11], 0x08);
    assert_eq!(
        word(24),
        (46 << 9) + (11 << 5) + 1,
        "the label's write date"
    );
    assert_eq!(
        word(22),
        (12 << 11) + (34 << 5) + 28,
        "the label's write time"
    );
    check_with_fsck(&image.bytes, "stamped-label");

    // The EFI volume: the label, EFI, BOOT and the file all carry the time.
    let efi = VolumeLayout::new(1024 * 1024, 0, 4096, 1, "BOOTX64.EFI", &mut Shared(&now)).unwrap();
    let mut sector = [0u8; 512];
    assert!(efi.fill_sector(efi.cluster_first_sector(2), &mut sector));
    for slot in 0..2 {
        let at = slot * 32;
        assert_eq!(
            u16::from_le_bytes([sector[at + 24], sector[at + 25]]),
            (46 << 9) + (11 << 5) + 1
        );
    }
    let zero = VolumeLayout::new(1024 * 1024, 0, 4096, 1, "BOOTX64.EFI", &mut NoClock).unwrap();
    assert!(zero.fill_sector(zero.cluster_first_sector(2), &mut sector));
    assert_eq!(sector[22..26], [0, 0, 0, 0]);
}
