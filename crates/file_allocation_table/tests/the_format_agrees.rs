//! **The general format against rust-fatfs, `mkfs.fat` and `fsck`.**
//!
//! Volumes this crate formats (FAT12, FAT16 and FAT32, at 512 and 4096-byte sectors and several
//! cluster sizes) must parse, mount in rust-fatfs with the cluster count this crate computed, pass
//! `fsck -n`, and take files from this crate's writer. Where `mkfs.fat` is installed it formats the
//! same sizes, and this crate reads and writes those FAT12 and FAT16 images too.

use std::io::{Cursor, Read};
use std::path::Path;
use std::process::Command;

use file_allocation_table::{
    BootSector, FileAllocationTableType, Format, FormatError, FormatOptions, SectorDevice,
    SectorSize, Volume, WritableSectorDevice,
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

/// Format `sectors` sectors of `size` with `options`, in memory.
fn formatted_by_this_crate(
    size: SectorSize,
    sectors: u32,
    build: impl Fn(FormatOptions) -> FormatOptions,
) -> (Format, Memory) {
    let options = build(FormatOptions::new(size, sectors));
    let format = Format::new(options).expect("a legal format");
    let mut image = Memory {
        bytes: vec![0xEEu8; sectors as usize * size.bytes()],
        size,
    };
    format.write_to(&mut image).unwrap();
    (format, image)
}

/// The volume parses, rust-fatfs mounts it with our cluster count, `fsck` accepts it, and this
/// crate's writer fills it and rust-fatfs reads the files back.
fn check(label: &str, format: &Format, image: Memory) {
    let boot =
        BootSector::parse(&image.bytes[..image.size.bytes()]).expect("our boot sector parses");
    assert_eq!(
        boot.file_allocation_table_type(),
        format.file_allocation_table_type(),
        "{label}"
    );
    assert_eq!(boot.clusters(), format.clusters(), "{label}");
    assert_eq!(
        boot.sectors_per_cluster(),
        format.sectors_per_cluster(),
        "{label}"
    );
    assert_eq!(
        oracle_free_clusters(&image.bytes),
        format.clusters()
            - u32::from(format.file_allocation_table_type() == FileAllocationTableType::Fat32),
        "{label}"
    );
    check_with_fsck(&image.bytes, &format!("{label}-empty"));
    let mut volume = Volume::open(image).unwrap();
    assert_eq!(
        volume.count_free_clusters().unwrap(),
        format.clusters()
            - u32::from(format.file_allocation_table_type() == FileAllocationTableType::Fat32)
    );
    let bytes: Vec<u8> = (0..20_000u32).map(|i| (i * 7) as u8).collect();
    let entry = volume.make_directory(0, "Made By Us").unwrap();
    let file = volume
        .create_file(entry.first_cluster(), "payload.bin")
        .unwrap();
    volume
        .open_file_writer(&file)
        .unwrap()
        .write(&bytes)
        .unwrap();
    volume.sync().unwrap();
    let image = volume.into_device();
    let fs = lamfat::FileSystem::new(
        lamfat::StdIoWrapper::from(Cursor::new(image.bytes.clone())),
        lamfat::FsOptions::new(),
    )
    .unwrap();
    let mut read = Vec::new();
    fs.root_dir()
        .open_file("Made By Us/payload.bin")
        .unwrap()
        .read_to_end(&mut read)
        .unwrap();
    assert_eq!(read, bytes, "{label}");
    drop(fs);
    check_with_fsck(&image.bytes, &format!("{label}-filled"));
}

#[test]
fn every_type_at_every_sector_size_formats_to_a_volume_the_oracles_accept() {
    use FileAllocationTableType::{Fat12, Fat16, Fat32};
    let cases: [(
        &str,
        SectorSize,
        u32,
        Option<FileAllocationTableType>,
        Option<u32>,
    ); 9] = [
        ("fat12-1m", SectorSize::Bytes512, 2 * 1024, None, None),
        (
            "fat12-4m",
            SectorSize::Bytes512,
            8 * 1024,
            Some(Fat12),
            None,
        ),
        (
            "fat12-1k",
            SectorSize::Bytes1024,
            4 * 1024,
            Some(Fat12),
            None,
        ),
        ("fat16-32m", SectorSize::Bytes512, 64 * 1024, None, None),
        (
            "fat16-cluster4",
            SectorSize::Bytes512,
            128 * 1024,
            Some(Fat16),
            Some(4),
        ),
        (
            "fat16-4k",
            SectorSize::Bytes4096,
            16 * 1024,
            Some(Fat16),
            None,
        ),
        (
            "fat32-64m",
            SectorSize::Bytes512,
            128 * 1024,
            Some(Fat32),
            None,
        ),
        (
            "fat32-cluster2",
            SectorSize::Bytes512,
            256 * 1024,
            Some(Fat32),
            Some(2),
        ),
        ("fat32-auto", SectorSize::Bytes512, 200 * 1024, None, None),
    ];
    for (label, size, sectors, kind, cluster) in cases {
        let (format, image) = formatted_by_this_crate(size, sectors, |o| {
            let o = kind.map_or(o, |k| o.with_type(k));
            let o = cluster.map_or(o, |c| o.with_cluster_sectors(c));
            o.with_volume_id(0xC0FF_EE11)
                .with_label("TEST VOLUME")
                .unwrap()
        });
        if let Some(kind) = kind {
            assert_eq!(format.file_allocation_table_type(), kind, "{label}");
        }
        check(label, &format, image);
    }
}

#[test]
fn a_size_no_cluster_size_makes_legal_is_refused_with_the_reason() {
    // 100 sectors cannot hold the reserved sectors, a root, two tables and a cluster as FAT16.
    let tiny =
        FormatOptions::new(SectorSize::Bytes512, 40).with_type(FileAllocationTableType::Fat16);
    assert_eq!(Format::new(tiny), Err(FormatError::TooSmall));
    // A 20 MiB volume cannot be FAT32: even single-sector clusters give 40,000 clusters.
    let small = FormatOptions::new(SectorSize::Bytes512, 40 * 1024)
        .with_type(FileAllocationTableType::Fat32);
    assert_eq!(Format::new(small), Err(FormatError::NoSuchGeometry));
    let odd = FormatOptions::new(SectorSize::Bytes512, 64 * 1024).with_cluster_sectors(3);
    assert_eq!(Format::new(odd), Err(FormatError::ClusterSizeRefused));
    for label in ["", "lower case", "TWELVE CHARS", "A*B"] {
        assert_eq!(
            FormatOptions::new(SectorSize::Bytes512, 64 * 1024)
                .with_label(label)
                .err(),
            Some(FormatError::LabelRefused),
            "{label}"
        );
    }
}

/// `mkfs.fat`, where this host has it.
fn mkfs_fat() -> Option<&'static str> {
    ["mkfs.fat", "/usr/sbin/mkfs.fat", "/sbin/mkfs.fat"]
        .into_iter()
        .find(|tool| Command::new(tool).arg("--help").output().is_ok())
}

/// FAT12 and FAT16 volumes `mkfs.fat` made: this crate opens them, writes a tree, and both
/// rust-fatfs and `fsck` accept the result. Skipped, and said so, where `mkfs.fat` is absent.
#[test]
fn volumes_mkfs_fat_made_take_this_crates_writer() {
    let Some(tool) = mkfs_fat() else {
        eprintln!("no mkfs.fat on this host; the mkfs.fat oracle images are skipped");
        return;
    };
    for (label, fat, mebibytes, cluster) in [
        ("mkfs-fat12", "12", 1u64, "1"),
        ("mkfs-fat16", "16", 32, "4"),
        ("mkfs-fat16-big-cluster", "16", 128, "32"),
    ] {
        let path = std::env::temp_dir().join(format!("{label}-{}.img", std::process::id()));
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(mebibytes << 20).unwrap();
        drop(file);
        let status = Command::new(tool)
            .args(["-F", fat, "-s", cluster, "-n", "ORACLE", "-i", "DEADBEEF"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            status.status.success(),
            "{label}: {}",
            String::from_utf8_lossy(&status.stderr)
        );
        let bytes = std::fs::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        // Their geometry is a valid one for the type; ours for the same size is another valid one.
        let boot = BootSector::parse(&bytes[..512]).unwrap();
        let ours = Format::new(
            FormatOptions::new(SectorSize::Bytes512, (mebibytes << 11) as u32)
                .with_type(boot.file_allocation_table_type())
                .with_cluster_sectors(boot.sectors_per_cluster()),
        )
        .unwrap();
        assert_eq!(
            ours.file_allocation_table_type(),
            boot.file_allocation_table_type(),
            "{label}"
        );
        assert!(
            ours.clusters().abs_diff(boot.clusters()) <= 8,
            "{label}: {} against {}",
            ours.clusters(),
            boot.clusters()
        );
        let image = Memory {
            bytes,
            size: SectorSize::Bytes512,
        };
        let mut volume = Volume::open(image).unwrap();
        let entry = volume.make_directory(0, "Written By Us").unwrap();
        let file = volume
            .create_file(entry.first_cluster(), "a rather long file name.txt")
            .unwrap();
        volume
            .open_file_writer(&file)
            .unwrap()
            .write(&[9u8; 50_000])
            .unwrap();
        volume.sync().unwrap();
        let image = volume.into_device();
        let fs = lamfat::FileSystem::new(
            lamfat::StdIoWrapper::from(Cursor::new(image.bytes.clone())),
            lamfat::FsOptions::new(),
        )
        .unwrap();
        let mut read = Vec::new();
        fs.root_dir()
            .open_file("Written By Us/a rather long file name.txt")
            .unwrap()
            .read_to_end(&mut read)
            .unwrap();
        assert_eq!(read, vec![9u8; 50_000], "{label}");
        drop(fs);
        check_with_fsck(&image.bytes, label);
    }
}
