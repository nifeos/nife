//! **rust-fatfs and `fsck` agree with the files this crate writes.**
//!
//! On FAT12, FAT16 and FAT32 volumes rust-fatfs formatted, this crate creates files, writes them
//! in odd chunks across clusters, overwrites and appends, sets lengths both ways and deletes. After
//! each stage rust-fatfs must list the same names and read the same bytes, count the same free
//! clusters, and the host's `fsck -n` must accept the volume. The dirty bit is set while the crate
//! changes the volume and clear after `sync`.

use std::io::{Cursor, Read};
use std::path::Path;
use std::process::Command;

use file_allocation_table::{
    NoClock, SectorDevice, SectorSize, WritableSectorDevice, WritableVolume, WriteError,
};
use lamfat::FatType;

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

/// What rust-fatfs counts as free clusters on `image`.
fn oracle_free_clusters(image: &[u8]) -> u32 {
    let fs = lamfat::FileSystem::new(
        lamfat::StdIoWrapper::from(Cursor::new(image.to_vec())),
        lamfat::FsOptions::new(),
    )
    .expect("rust-fatfs mounts the volume");
    fs.stats().unwrap().free_clusters()
}

/// Bytes that tell which offset of which file they are.
fn pattern(seed: u8, length: usize) -> Vec<u8> {
    (0..length)
        .map(|i| (i as u32).wrapping_mul(2_654_435_761).wrapping_shr(11) as u8 ^ seed)
        .collect()
}

/// Every file in the root of `image` according to rust-fatfs, as (name, bytes).
fn oracle_files(image: &[u8]) -> Vec<(String, Vec<u8>)> {
    let fs = lamfat::FileSystem::new(
        lamfat::StdIoWrapper::from(Cursor::new(image.to_vec())),
        lamfat::FsOptions::new(),
    )
    .expect("rust-fatfs mounts the volume");
    let mut out = Vec::new();
    for entry in fs.root_dir().iter() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if entry.is_file() {
            let mut bytes = Vec::new();
            let mut file = entry.to_file();
            file.read_to_end(&mut bytes).unwrap();
            out.push((name, bytes));
        }
    }
    out.sort();
    out
}

/// Write `bytes` through `volume` in chunks of `chunk`.
fn write_in_chunks(
    volume: &mut WritableVolume<Memory, NoClock>,
    name: &str,
    bytes: &[u8],
    chunk: usize,
) {
    let entry = volume.create_file(0, name).expect("create");
    let mut writer = volume.open_file_writer(&entry).unwrap();
    for piece in bytes.chunks(chunk.max(1)) {
        assert_eq!(writer.write(piece).unwrap(), piece.len());
    }
    assert_eq!(writer.size_bytes() as usize, bytes.len());
}

/// What a stage of the test shares: the files the volume must hold, and the sizes the cluster
/// arithmetic needs.
struct Plan {
    label: String,
    expected: Vec<(String, Vec<u8>)>,
    total: u32,
    cluster_bytes: usize,
}

impl Plan {
    /// rust-fatfs lists these files, counts the free clusters they leave, and `fsck` accepts it.
    fn check(&mut self, image: &Memory, stage: &str) {
        self.expected.sort();
        assert_eq!(
            oracle_files(&image.bytes),
            self.expected,
            "{}: {stage}",
            self.label
        );
        check_with_fsck(&image.bytes, &format!("{}-{stage}", self.label));
        let used: usize = self
            .expected
            .iter()
            .map(|(_, b)| b.len().div_ceil(self.cluster_bytes))
            .sum();
        assert_eq!(
            oracle_free_clusters(&image.bytes) as usize,
            self.total as usize - used,
            "{}: {stage}",
            self.label
        );
    }

    fn bytes_of(&mut self, name: &str) -> &mut Vec<u8> {
        &mut self.expected.iter_mut().find(|(n, _)| n == name).unwrap().1
    }
}

/// Create ten files of sizes around the sector and cluster boundaries.
fn create_files(volume: &mut WritableVolume<Memory, NoClock>, plan: &mut Plan, fat_type: FatType) {
    let c = plan.cluster_bytes;
    let sizes = [0, 1, 511, 512, 513, c - 1, c, c + 1, 3 * c + 17, 100_000];
    for (i, size) in sizes.iter().enumerate() {
        let name = format!("FILE{i}.BIN");
        let bytes = pattern(i as u8, *size);
        write_in_chunks(volume, &name, &bytes, 1 + i * 97);
        plan.expected.push((name, bytes));
    }
    if fat_type != FatType::Fat12 {
        assert_eq!(
            volume.is_clean().unwrap(),
            Some(false),
            "{}: dirty",
            plan.label
        );
    }
    volume.sync().unwrap();
    if fat_type != FatType::Fat12 {
        assert_eq!(
            volume.is_clean().unwrap(),
            Some(true),
            "{}: clean",
            plan.label
        );
    }
}

/// Overwrite across a cluster boundary, append, shrink, grow, empty and delete.
fn change_files(volume: &mut WritableVolume<Memory, NoClock>, plan: &mut Plan) {
    let c = plan.cluster_bytes;
    let entry = volume.lookup("FILE9.BIN").unwrap();
    {
        let mut writer = volume.open_file_writer(&entry).unwrap();
        writer.seek(c as u32 - 3);
        writer.write(b"SEAM-OVER-BOUNDARY").unwrap();
        writer.seek(u32::MAX);
        writer.write(b"appended").unwrap();
    }
    let nine = plan.bytes_of("FILE9.BIN");
    nine[c - 3..c + 15].copy_from_slice(b"SEAM-OVER-BOUNDARY");
    nine.extend_from_slice(b"appended");
    for (name, length) in [
        ("FILE8.BIN", c + 5),
        ("FILE1.BIN", 2 * c + 9),
        ("FILE5.BIN", 0),
    ] {
        let entry = volume.lookup(name).unwrap();
        volume
            .open_file_writer(&entry)
            .unwrap()
            .set_len(length as u32)
            .unwrap();
        plan.bytes_of(name).resize(length, 0);
    }
    let entry = volume.lookup("FILE7.BIN").unwrap();
    volume.remove_file(&entry).unwrap();
    plan.expected.retain(|(n, _)| n != "FILE7.BIN");
    assert!(volume.lookup("FILE7.BIN").is_err());
    volume.sync().unwrap();
}

/// Read every file back through this crate's reader.
fn read_files(volume: &mut WritableVolume<Memory, NoClock>, plan: &Plan) {
    for (name, bytes) in &plan.expected {
        let entry = volume.lookup(name).unwrap();
        let mut reader = volume.open_file(&entry).unwrap();
        let mut read = vec![0u8; bytes.len() + 1];
        assert_eq!(reader.read(&mut read).unwrap(), bytes.len(), "{name}");
        assert_eq!(&read[..bytes.len()], &bytes[..], "{name}");
    }
}

fn the_writer_agrees(label: &str, mebibytes: usize, fat_type: FatType, sector: u16) {
    let mut volume = WritableVolume::open(formatted(mebibytes, fat_type, sector), NoClock).unwrap();
    let mut plan = Plan {
        label: label.to_string(),
        expected: Vec::new(),
        total: volume.count_free_clusters().unwrap(),
        cluster_bytes: volume.boot_sector().bytes_per_cluster() as usize,
    };
    create_files(&mut volume, &mut plan, fat_type);
    let image = volume.into_device();
    plan.check(&image, "created");

    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    change_files(&mut volume, &mut plan);
    let image = volume.into_device();
    plan.check(&image, "changed");

    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    read_files(&mut volume, &plan);
    for (name, _) in std::mem::take(&mut plan.expected) {
        let entry = volume.lookup(&name).unwrap();
        volume.remove_file(&entry).unwrap();
    }
    volume.sync().unwrap();
    plan.check(&volume.into_device(), "emptied");
}

#[test]
fn files_write_and_agree_on_fat12() {
    the_writer_agrees("files-fat12", 4, FatType::Fat12, 512);
}

#[test]
fn files_write_and_agree_on_fat16() {
    the_writer_agrees("files-fat16", 32, FatType::Fat16, 512);
}

#[test]
fn files_write_and_agree_on_fat16_with_4096_byte_sectors() {
    the_writer_agrees("files-fat16-4k", 64, FatType::Fat16, 4096);
}

#[test]
fn files_write_and_agree_on_fat32() {
    the_writer_agrees("files-fat32", 64, FatType::Fat32, 512);
}

#[test]
fn a_fat32_root_grows_past_one_cluster_and_a_fat16_root_refuses() {
    let mut volume = WritableVolume::open(formatted(64, FatType::Fat32, 512), NoClock).unwrap();
    let per_cluster = volume.boot_sector().bytes_per_cluster() as usize / 32;
    for i in 0..per_cluster + 3 {
        volume.create_file(0, &format!("N{i}.TXT")).unwrap();
    }
    volume.sync().unwrap();
    let image = volume.into_device();
    assert_eq!(oracle_files(&image.bytes).len(), per_cluster + 3);
    check_with_fsck(&image.bytes, "root-grown");

    let mut volume = WritableVolume::open(formatted(32, FatType::Fat16, 512), NoClock).unwrap();
    let mut made = 0;
    let refused = loop {
        match volume.create_file(0, &format!("M{made}.TXT")) {
            Ok(_) => made += 1,
            Err(error) => break error,
        }
    };
    assert_eq!(refused, WriteError::DirectoryFull);
    assert_eq!(made, 512);
}

#[test]
fn names_and_stale_entries_are_refused() {
    let mut volume = WritableVolume::open(formatted(4, FatType::Fat12, 512), NoClock).unwrap();
    for bad in ["", "a/b", "ends.", "has*star"] {
        assert_eq!(
            volume.create_file(0, bad),
            Err(WriteError::InvalidName),
            "{bad}"
        );
    }
    let entry = volume.create_file(0, "ONE.TXT").unwrap();
    assert_eq!(
        volume.create_file(0, "ONE.TXT"),
        Err(WriteError::AlreadyExists)
    );
    volume.remove_file(&entry).unwrap();
    assert_eq!(volume.remove_file(&entry), Err(WriteError::EntryChanged));
    let again = volume.create_file(0, "TWO.TXT").unwrap();
    assert!(volume.open_file_writer(&again).is_ok());
    // The freed slot is reused.
    assert!(volume.lookup("ONE.TXT").is_err());
}

#[test]
fn a_full_volume_writes_what_fits_then_refuses_and_deleting_restores_the_space() {
    let mut volume = WritableVolume::open(formatted(1, FatType::Fat12, 512), NoClock).unwrap();
    let total = volume.count_free_clusters().unwrap() as usize;
    let cluster = volume.boot_sector().bytes_per_cluster() as usize;
    let entry = volume.create_file(0, "BIG.BIN").unwrap();
    let mut writer = volume.open_file_writer(&entry).unwrap();
    let data = vec![0xA5u8; (total + 5) * cluster];
    let written = writer.write(&data).unwrap();
    assert_eq!(written, total * cluster);
    assert_eq!(writer.write(&data[written..]), Err(WriteError::VolumeFull));
    assert_eq!(writer.write(b"x"), Err(WriteError::AfterAFailure));
    let entry = volume.lookup("BIG.BIN").unwrap();
    assert_eq!(entry.size_bytes() as usize, total * cluster);
    volume.remove_file(&entry).unwrap();
    assert_eq!(volume.count_free_clusters().unwrap() as usize, total);
    volume.sync().unwrap();
    check_with_fsck(&volume.into_device().bytes, "full");
}

#[test]
fn a_volume_found_dirty_stays_dirty_after_a_sync() {
    for (mebibytes, fat_type, at, bit) in [
        (32, FatType::Fat16, 2usize, 0x80u8),
        (64, FatType::Fat32, 4, 0x08),
    ] {
        let mut image = formatted(mebibytes, fat_type, 512);
        // Table entry 1 is at byte 2 (FAT16) or 4 (FAT32) of the first table; clear its clean bit.
        let boot = WritableVolume::open(
            Memory {
                bytes: image.bytes.clone(),
                size: image.size,
            },
            NoClock,
        )
        .unwrap();
        let table = boot.boot_sector().table_first_sector(0).unwrap() as usize * 512;
        let top = table + at + if fat_type == FatType::Fat16 { 1 } else { 3 };
        image.bytes[top] &= !bit;
        let mut volume = WritableVolume::open(image, NoClock).unwrap();
        assert_eq!(volume.is_clean().unwrap(), Some(false));
        volume.create_file(0, "A.TXT").unwrap();
        volume.sync().unwrap();
        assert_eq!(volume.is_clean().unwrap(), Some(false), "{fat_type:?}");
    }
}

/// Files made inside a subdirectory rust-fatfs created, enough of them to grow it.
#[test]
fn files_in_a_subdirectory_write_and_the_subdirectory_grows() {
    for (mebibytes, fat_type) in [
        (4, FatType::Fat12),
        (32, FatType::Fat16),
        (64, FatType::Fat32),
    ] {
        let mut image = formatted(mebibytes, fat_type, 512);
        {
            let fs = lamfat::FileSystem::new(
                lamfat::StdIoWrapper::from(Cursor::new(&mut image.bytes)),
                lamfat::FsOptions::new(),
            )
            .unwrap();
            fs.root_dir().create_dir("Sub Directory").unwrap();
            fs.unmount().unwrap();
        }
        let mut volume = WritableVolume::open(image, NoClock).unwrap();
        let dir = volume.lookup("Sub Directory").unwrap().first_cluster();
        let cluster = volume.boot_sector().bytes_per_cluster() as usize;
        // Slot 0 and 1 are `.` and `..`; this many files fill the first cluster and spill over.
        let count = cluster / 32 + 5;
        for i in 0..count {
            let entry = volume.create_file(dir, &format!("F{i}.DAT")).unwrap();
            let mut writer = volume.open_file_writer(&entry).unwrap();
            writer.write(format!("file number {i}").as_bytes()).unwrap();
        }
        // Entries found by path carry their directory: shrink one, delete another.
        let found = volume.lookup("Sub Directory/F3.DAT").unwrap();
        volume.open_file_writer(&found).unwrap().set_len(4).unwrap();
        let doomed = volume.lookup("Sub Directory/F4.DAT").unwrap();
        volume.remove_file(&doomed).unwrap();
        volume.sync().unwrap();
        let image = volume.into_device();
        let fs = lamfat::FileSystem::new(
            lamfat::StdIoWrapper::from(Cursor::new(image.bytes.clone())),
            lamfat::FsOptions::new(),
        )
        .unwrap();
        let sub = fs.root_dir().open_dir("Sub Directory").unwrap();
        let mut seen = 0;
        for entry in sub.iter() {
            let entry = entry.unwrap();
            if entry.is_file() {
                let mut text = String::new();
                entry.to_file().read_to_string(&mut text).unwrap();
                let number: usize = entry.file_name()[1..entry.file_name().len() - 4]
                    .parse()
                    .unwrap();
                if number == 3 {
                    assert_eq!(text, "file");
                } else {
                    assert_eq!(text, format!("file number {number}"));
                }
                seen += 1;
            }
        }
        assert_eq!(seen, count - 1, "{fat_type:?}");
        check_with_fsck(&image.bytes, &format!("subdir-{fat_type:?}"));
    }
}

/// A FAT16 root filled to its 512 entries holds every one of them, in the sectors the region has.
#[test]
fn a_full_fixed_root_reads_back_every_entry() {
    let mut volume = WritableVolume::open(formatted(32, FatType::Fat16, 512), NoClock).unwrap();
    for i in 0..512 {
        let entry = volume.create_file(0, &format!("R{i}.TXT")).unwrap();
        volume
            .open_file_writer(&entry)
            .unwrap()
            .write(format!("{i}").as_bytes())
            .unwrap();
    }
    volume.sync().unwrap();
    let image = volume.into_device();
    let files = oracle_files(&image.bytes);
    assert_eq!(files.len(), 512);
    assert!(files.iter().all(|(name, bytes)| {
        name.strip_prefix('R')
            .and_then(|n| n.strip_suffix(".TXT"))
            .is_some_and(|n| n.as_bytes() == &bytes[..])
    }));
    check_with_fsck(&image.bytes, "fixed-root-full");
}

/// A FAT32 volume with four-sector clusters over stale data: the root grows into a cluster whose
/// later sectors must come back zeroed, and a directory slot lands in each of a cluster's sectors.
#[test]
fn a_directory_that_grows_into_a_multi_sector_cluster_zeroes_all_of_it() {
    // FAT32 needs 65,525 clusters, which at 2048 bytes each is 134 MB.
    let mut image = Memory {
        bytes: vec![0u8; 150 << 20],
        size: SectorSize::Bytes512,
    };
    let mut cursor = Cursor::new(&mut image.bytes);
    lamfat::format_volume(
        &mut lamfat::StdIoWrapper::from(&mut cursor),
        lamfat::FormatVolumeOptions::new()
            .fat_type(FatType::Fat32)
            .bytes_per_cluster(2048),
    )
    .unwrap();
    let probe = WritableVolume::open(
        Memory {
            bytes: image.bytes.clone(),
            size: image.size,
        },
        NoClock,
    )
    .unwrap();
    let boot = *probe.boot_sector();
    assert_eq!(boot.sectors_per_cluster(), 4);
    let data = boot.first_data_sector() as usize * 512;
    // Everything after the root's first cluster is stale.
    image.bytes[data + 2048..].fill(0xAA);
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    let slots = 2048 / 32;
    for i in 0..slots + 40 {
        volume.create_file(0, &format!("G{i}.TXT")).unwrap();
    }
    volume.sync().unwrap();
    let image = volume.into_device();
    assert_eq!(oracle_files(&image.bytes).len(), slots + 40);
    check_with_fsck(&image.bytes, "multi-sector-cluster");
}

/// A file whose first cluster is above 65535 needs the high half of the cluster number, which
/// only FAT32 stores in a directory entry.
#[test]
fn a_fat32_file_beyond_cluster_65535_keeps_its_high_cluster_bits() {
    let mut image = formatted(64, FatType::Fat32, 512);
    let mut cursor = Cursor::new(&mut image.bytes);
    lamfat::format_volume(
        &mut lamfat::StdIoWrapper::from(&mut cursor),
        lamfat::FormatVolumeOptions::new()
            .fat_type(FatType::Fat32)
            .bytes_per_cluster(512),
    )
    .unwrap();
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    let filler = vec![0x5Au8; 1 << 20];
    let entry = volume.create_file(0, "FILL.BIN").unwrap();
    let mut writer = volume.open_file_writer(&entry).unwrap();
    for _ in 0..34 {
        writer.write(&filler).unwrap();
    }
    let late = volume.create_file(0, "LATE.TXT").unwrap();
    volume
        .open_file_writer(&late)
        .unwrap()
        .write(b"late")
        .unwrap();
    assert!(volume.lookup("LATE.TXT").unwrap().first_cluster() > 65_535);
    volume.sync().unwrap();
    let image = volume.into_device();
    let files = oracle_files(&image.bytes);
    assert_eq!(
        files.iter().find(|(n, _)| n == "LATE.TXT").unwrap().1,
        b"late"
    );
    check_with_fsck(&image.bytes, "high-cluster");
}
