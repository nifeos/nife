//! **rust-fatfs and `fsck` agree with the attributes and labels this crate writes.**
//!
//! On FAT12, FAT16 and FAT32 volumes, this crate sets every combination of the four user bits on
//! files and a directory, and rust-fatfs must read the same bits and `fsck -n` must accept the
//! volume. A read-only file refuses a write and a remove until its bit clears. A label this crate
//! sets is the one rust-fatfs reports, in the root and the boot sector both; a label a format or
//! `mkfs.fat` wrote reads back here.

use std::io::Cursor;
use std::path::Path;
use std::process::Command;

use file_allocation_table::{
    Attributes, FormatOptions, FormatPlan, NoClock, SectorDevice, SectorSize, WritableSectorDevice,
    WritableVolume, WriteError,
};
use lamfat::{FatType, FileAttributes};

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

fn lamfat_attributes(image: &[u8], name: &str) -> FileAttributes {
    let fs = lamfat::FileSystem::new(
        lamfat::StdIoWrapper::from(Cursor::new(image.to_vec())),
        lamfat::FsOptions::new(),
    )
    .unwrap();
    let found = fs
        .root_dir()
        .iter()
        .map(|e| e.unwrap())
        .find(|e| e.file_name() == name)
        .unwrap_or_else(|| panic!("{name} is listed"));
    found.attributes()
}

fn lamfat_label(image: &[u8]) -> String {
    lamfat::FileSystem::new(
        lamfat::StdIoWrapper::from(Cursor::new(image.to_vec())),
        lamfat::FsOptions::new(),
    )
    .unwrap()
    .volume_label()
}

fn attributes_agree(label: &str, mebibytes: usize, fat_type: FatType) {
    let mut volume = WritableVolume::open(formatted(mebibytes, fat_type, 512), NoClock).unwrap();
    let file = volume.create_file(0, "Attribute Test File.txt").unwrap();
    let dir = volume.create_directory(0, "Attribute Dir").unwrap();
    let sets = [
        (Attributes::EMPTY, FileAttributes::empty()),
        (Attributes::READ_ONLY, FileAttributes::READ_ONLY),
        (Attributes::HIDDEN, FileAttributes::HIDDEN),
        (Attributes::SYSTEM, FileAttributes::SYSTEM),
        (Attributes::ARCHIVE, FileAttributes::ARCHIVE),
        (
            Attributes::READ_ONLY
                .union(Attributes::HIDDEN)
                .union(Attributes::SYSTEM)
                .union(Attributes::ARCHIVE),
            FileAttributes::READ_ONLY
                | FileAttributes::HIDDEN
                | FileAttributes::SYSTEM
                | FileAttributes::ARCHIVE,
        ),
    ];
    for (ours, theirs) in sets {
        let file = volume.set_attributes(&file, ours).unwrap();
        assert_eq!(file.user_attributes(), ours, "{label}");
        let dir = volume.set_attributes(&dir, ours).unwrap();
        assert!(dir.is_directory(), "{label}");
        volume.mark_clean().unwrap();
        let image = volume.into_device();
        assert_eq!(
            lamfat_attributes(&image.bytes, "Attribute Test File.txt"),
            theirs,
            "{label} file"
        );
        assert_eq!(
            lamfat_attributes(&image.bytes, "Attribute Dir"),
            theirs | FileAttributes::DIRECTORY,
            "{label} directory"
        );
        check_with_fsck(&image.bytes, &format!("{label}-attributes"));
        volume = WritableVolume::open(image, NoClock).unwrap();
    }
    // A read-only file refuses until the bit clears.
    let current = volume.lookup("Attribute Test File.txt").unwrap();
    let locked = volume
        .set_attributes(&current, Attributes::READ_ONLY)
        .unwrap();
    assert_eq!(
        volume.open_file_writer(&locked).unwrap().write(b"x"),
        Err(WriteError::ReadOnly)
    );
    assert_eq!(volume.remove_file(&locked), Err(WriteError::ReadOnly));
    let open = volume.set_attributes(&locked, Attributes::EMPTY).unwrap();
    volume
        .open_file_writer(&open)
        .unwrap()
        .write(b"written")
        .unwrap();
    volume.mark_clean().unwrap();
    check_with_fsck(&volume.into_device().bytes, &format!("{label}-unlocked"));
}

#[test]
fn attributes_agree_on_fat12() {
    attributes_agree("attr-fat12", 4, FatType::Fat12);
}

#[test]
fn attributes_agree_on_fat16() {
    attributes_agree("attr-fat16", 32, FatType::Fat16);
}

#[test]
fn attributes_agree_on_fat32() {
    attributes_agree("attr-fat32", 64, FatType::Fat32);
}

fn labels_agree(label: &str, mebibytes: usize, fat_type: FatType) {
    let mut volume = WritableVolume::open(formatted(mebibytes, fat_type, 512), NoClock).unwrap();
    assert_eq!(
        volume.volume_label().unwrap(),
        None,
        "{label}: rust-fatfs formats with no label entry"
    );
    volume.set_volume_label("ORACLE LBL").unwrap();
    assert_eq!(
        volume.volume_label().unwrap().unwrap().trimmed(),
        b"ORACLE LBL"
    );
    volume.mark_clean().unwrap();
    let image = volume.into_device();
    assert_eq!(lamfat_label(&image.bytes), "ORACLE LBL", "{label}");
    check_with_fsck(&image.bytes, &format!("{label}-label"));
    // Change it: still one entry, and rust-fatfs and fsck agree.
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    volume.set_volume_label("RENAMED").unwrap();
    let labels = volume
        .root_named_entries()
        .map(|e| e.unwrap())
        .filter(|e| e.is_volume_label())
        .count();
    assert_eq!(labels, 1, "{label}");
    volume.mark_clean().unwrap();
    let image = volume.into_device();
    assert_eq!(lamfat_label(&image.bytes), "RENAMED", "{label}");
    check_with_fsck(&image.bytes, &format!("{label}-relabel"));
    // The boot sector's copy follows, and on FAT32 so does the backup boot sector.
    let mut volume = WritableVolume::open(
        Memory {
            bytes: image.bytes.clone(),
            size: image.size,
        },
        NoClock,
    )
    .unwrap();
    let boot = *volume.boot_sector();
    let at = if fat_type == FatType::Fat32 { 71 } else { 43 };
    assert_eq!(&image.bytes[at..at + 11], b"RENAMED    ", "{label}");
    if let Some(backup) = boot.backup_boot_sector() {
        let sector = usize::from(backup) * 512;
        assert_eq!(
            &image.bytes[sector + at..sector + at + 11],
            b"RENAMED    ",
            "{label} backup"
        );
    }
    assert_eq!(
        volume.set_volume_label("lower"),
        Err(WriteError::LabelRefused)
    );
}

#[test]
fn labels_agree_on_fat12() {
    labels_agree("label-fat12", 4, FatType::Fat12);
}

#[test]
fn labels_agree_on_fat16() {
    labels_agree("label-fat16", 32, FatType::Fat16);
}

#[test]
fn labels_agree_on_fat32() {
    labels_agree("label-fat32", 64, FatType::Fat32);
}

#[test]
fn a_label_the_format_wrote_reads_back_and_changes() {
    for sectors in [8 * 1024u32, 64 * 1024, 200 * 1024] {
        let options = FormatOptions::new(SectorSize::Bytes512, sectors)
            .with_label("FORMATTED")
            .unwrap();
        let format = FormatPlan::new(options, &mut NoClock).unwrap();
        let mut image = Memory {
            bytes: vec![0u8; sectors as usize * 512],
            size: SectorSize::Bytes512,
        };
        format.write_to(&mut image).unwrap();
        assert_eq!(lamfat_label(&image.bytes), "FORMATTED");
        let mut volume = WritableVolume::open(image, NoClock).unwrap();
        assert_eq!(
            volume.volume_label().unwrap().unwrap().trimmed(),
            b"FORMATTED"
        );
        volume.set_volume_label("CHANGED").unwrap();
        volume.mark_clean().unwrap();
        let image = volume.into_device();
        assert_eq!(lamfat_label(&image.bytes), "CHANGED");
        check_with_fsck(&image.bytes, "formatted-label");
    }
}

/// `mkfs.fat`, where this host has it: its label reads back here, and a change passes `fsck`.
#[test]
fn a_label_mkfs_fat_wrote_reads_back_and_changes() {
    let probe = ["mkfs.fat", "/usr/sbin/mkfs.fat", "/sbin/mkfs.fat"]
        .into_iter()
        .find(|tool| Command::new(tool).arg("--help").output().is_ok());
    let Some(tool) = probe else {
        eprintln!("no mkfs.fat on this host; the mkfs.fat label check is skipped");
        return;
    };
    let path = std::env::temp_dir().join(format!("label-mkfs-{}.img", std::process::id()));
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(32 << 20).unwrap();
    drop(file);
    let made = Command::new(tool)
        .args(["-F", "16", "-n", "MKFSLABEL"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(made.status.success());
    let bytes = std::fs::read(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    let mut volume = WritableVolume::open(
        Memory {
            bytes,
            size: SectorSize::Bytes512,
        },
        NoClock,
    )
    .unwrap();
    assert_eq!(
        volume.volume_label().unwrap().unwrap().trimmed(),
        b"MKFSLABEL"
    );
    volume.set_volume_label("AFTER").unwrap();
    volume.mark_clean().unwrap();
    let image = volume.into_device();
    assert_eq!(lamfat_label(&image.bytes), "AFTER");
    check_with_fsck(&image.bytes, "mkfs-label");
}
