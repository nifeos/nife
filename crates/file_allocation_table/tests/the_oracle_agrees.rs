//! **rust-fatfs and this crate agree about a volume, in both directions.**
//!
//! rust-fatfs, as `lamfat` 0.4.3, formats FAT12, FAT16 and FAT32 volumes in memory and writes the
//! same tree onto each: nested directories with long names, a file spanning many clusters, a
//! directory grown past one cluster, an empty file, a delete and a rename. This crate then reads
//! every directory, every chain and the table's free count back, and must agree with what
//! rust-fatfs lists and reads. In the other direction, rust-fatfs reads the volume this crate's
//! writer lays out.
//!
//! Where `fsck.fat` (dosfstools) or `fsck_msdos` (macOS) is installed, each image must also pass
//! it with `-n`. Neither is linked; each runs as a process, and its absence is reported and
//! skipped.

use std::io::Cursor;
use std::path::Path;
use std::process::Command;

use file_allocation_table::{
    DirectoryEntry, SECTOR_BYTES, SectorDevice, SectorSize, ShortEntry, TableEntry, Volume,
    VolumeLayout,
};
use lamfat::{Read, Write};

/// A volume held in memory, served one sector at a time.
struct Memory<'a> {
    bytes: &'a [u8],
    size: SectorSize,
}

impl SectorDevice for Memory<'_> {
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

/// What both readers must agree on for one directory entry.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Record {
    short: String,
    long: Option<Vec<u16>>,
    directory: bool,
    size: u64,
    /// A file's bytes, or the records of a directory's entries.
    bytes: Vec<u8>,
    children: Vec<Record>,
}

/// The bytes of the many-cluster file: not a round number, and a pattern a cluster out of place
/// cannot reproduce.
fn big() -> Vec<u8> {
    (0..300_000u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect()
}

/// Format `image` as `fat_type` with `sector` bytes per sector and write the tree onto it.
fn write_tree(image: &mut Vec<u8>, fat_type: lamfat::FatType, sector: u16) {
    let mut cursor = Cursor::new(&mut *image);
    lamfat::format_volume(
        &mut lamfat::StdIoWrapper::from(&mut cursor),
        lamfat::FormatVolumeOptions::new()
            .fat_type(fat_type)
            .bytes_per_sector(sector),
    )
    .expect("rust-fatfs formats the volume");
    let fs = lamfat::FileSystem::new(
        lamfat::StdIoWrapper::from(Cursor::new(&mut *image)),
        lamfat::FsOptions::new(),
    )
    .expect("rust-fatfs mounts its own volume");
    {
        let root = fs.root_dir();
        let deep = "A Long Directory Name/nested two/third level deep";
        root.create_dir("A Long Directory Name").unwrap();
        root.create_dir("A Long Directory Name/nested two").unwrap();
        root.create_dir(deep).unwrap();
        let mut file = root
            .create_file(&format!("{deep}/Spans Many Clusters.bin"))
            .unwrap();
        file.write_all(&big()).unwrap();
        file.flush().unwrap();
        drop(file);
        for i in 0..40 {
            let name = format!("A Long Directory Name/file number {i:02} with a long name.txt");
            let mut file = root.create_file(&name).unwrap();
            file.write_all(format!("file {i}\n").as_bytes()).unwrap();
        }
        root.create_file("EMPTY.TXT").unwrap();
        let mut file = root.create_file("short.txt").unwrap();
        file.write_all(b"short\n").unwrap();
        drop(file);
        let mut file = root.create_file("Doomed File.txt").unwrap();
        file.write_all(&[7u8; 9000]).unwrap();
        drop(file);
        root.remove("Doomed File.txt").unwrap();
        root.create_dir("empty dir").unwrap();
        root.rename(
            "short.txt",
            &root,
            "A Long Directory Name/nested two/Renamed From Short.txt",
        )
        .unwrap();
    }
    fs.unmount().expect("rust-fatfs unmounts");
}

/// Everything rust-fatfs lists under `dir`, recursively, `.` and `..` left out.
fn oracle_records<
    IO: lamfat::ReadWriteSeek,
    TP: lamfat::TimeProvider,
    OCC: lamfat::OemCpConverter,
>(
    dir: &lamfat::Dir<'_, IO, TP, OCC>,
) -> Vec<Record> {
    let mut out = Vec::new();
    for entry in dir.iter() {
        let entry = entry.expect("rust-fatfs lists the directory");
        let short = String::from_utf8(entry.short_file_name_as_bytes().to_vec()).unwrap();
        if short == "." || short == ".." {
            continue;
        }
        let mut record = Record {
            short,
            long: entry.long_file_name_as_ucs2_units().map(<[u16]>::to_vec),
            directory: entry.is_dir(),
            size: if entry.is_dir() { 0 } else { entry.len() },
            bytes: Vec::new(),
            children: Vec::new(),
        };
        if entry.is_dir() {
            record.children = oracle_records(&entry.to_dir());
        } else {
            entry
                .to_file()
                .read_to_end_compat(&mut record.bytes)
                .expect("rust-fatfs reads the file");
        }
        out.push(record);
    }
    out.sort();
    out
}

/// The 0.4 line's `Read` has no `read_to_end`.
trait ReadToEnd {
    fn read_to_end_compat(&mut self, out: &mut Vec<u8>) -> Result<(), ()>;
}

impl<R: Read> ReadToEnd for R {
    fn read_to_end_compat(&mut self, out: &mut Vec<u8>) -> Result<(), ()> {
        let mut buffer = [0u8; 4096];
        loop {
            let read = self.read(&mut buffer).map_err(|_| ())?;
            if read == 0 {
                return Ok(());
            }
            out.extend_from_slice(&buffer[..read]);
        }
    }
}

/// A short name as rust-fatfs prints it: base and extension trimmed, joined by a dot when there
/// is an extension.
fn printed(entry: &ShortEntry) -> String {
    let name = entry.name();
    let base = String::from_utf8_lossy(&name[..8]).trim_end().to_string();
    let extension = String::from_utf8_lossy(&name[8..]).trim_end().to_string();
    if extension.is_empty() {
        base
    } else {
        format!("{base}.{extension}")
    }
}

/// The bytes of the chain from `first`, `size` of them, read cluster by cluster; the chain must
/// hold exactly the clusters `size` needs.
fn read_file(volume: &mut Volume<Memory<'_>>, image: &[u8], first: u32, size: u64) -> Vec<u8> {
    let boot = *volume.boot_sector();
    let cluster_bytes = u64::from(boot.bytes_per_cluster());
    if size == 0 {
        assert_eq!(first, 0, "an empty file owns no cluster");
        return Vec::new();
    }
    let clusters: Vec<u32> = volume
        .chain(first)
        .map(|c| c.expect("the chain reads"))
        .collect();
    assert_eq!(clusters.len() as u64, size.div_ceil(cluster_bytes));
    let sector = boot.sector_size().bytes();
    let mut out = Vec::new();
    for cluster in clusters {
        let at = boot.cluster_first_sector(cluster).unwrap() as usize * sector;
        out.extend_from_slice(&image[at..at + cluster_bytes as usize]);
    }
    out.truncate(size as usize);
    out
}

/// Everything this crate reads under the directory starting at `first` (0 for the root).
fn our_records(volume: &mut Volume<Memory<'_>>, image: &[u8], first: u32) -> Vec<Record> {
    // The specification caps a directory at 65,536 slots; a reader that stops advancing then fails
    // the comparison rather than hanging the suite.
    let entries: Vec<DirectoryEntry> = volume
        .directory(first)
        .take(65_536)
        .map(|e| e.expect("the directory reads"))
        .collect();
    let mut out = Vec::new();
    let mut pieces: Vec<[u16; 13]> = Vec::new();
    for entry in entries {
        let short = match entry {
            DirectoryEntry::LongName(piece) => {
                pieces.push(piece.units());
                continue;
            }
            DirectoryEntry::Short(short) => short,
        };
        // The pieces sit last-first on disk.
        let long = (!pieces.is_empty()).then(|| {
            let units: Vec<u16> = pieces.iter().rev().flatten().copied().collect();
            let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
            units[..end].to_vec()
        });
        pieces.clear();
        let name = printed(&short);
        if short.is_volume_label() || name == "." || name == ".." {
            continue;
        }
        let mut record = Record {
            short: name,
            long,
            directory: short.is_directory(),
            size: u64::from(short.size()),
            bytes: Vec::new(),
            children: Vec::new(),
        };
        if short.is_directory() {
            record.children = our_records(volume, image, short.first_cluster());
        } else {
            record.bytes = read_file(volume, image, short.first_cluster(), record.size);
        }
        out.push(record);
    }
    out.sort();
    out
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

/// Write the tree with rust-fatfs and read it back with both readers.
fn agree(label: &str, mebibytes: usize, fat_type: lamfat::FatType, sector: u16) {
    let mut image = vec![0u8; mebibytes << 20];
    write_tree(&mut image, fat_type, sector);
    check_with_fsck(&image, label);

    let fs = lamfat::FileSystem::new(
        lamfat::StdIoWrapper::from(Cursor::new(image.clone())),
        lamfat::FsOptions::new(),
    )
    .unwrap();
    let expected = oracle_records(&fs.root_dir());
    let free = fs.stats().unwrap().free_clusters();

    let size = SectorSize::from_bytes(sector).unwrap();
    let mut volume = Volume::open(Memory {
        bytes: &image,
        size,
    })
    .expect("this crate opens rust-fatfs's volume");
    let ours = our_records(&mut volume, &image, 0);
    assert_eq!(
        ours, expected,
        "{label}: the two readers list different trees"
    );
    assert_eq!(
        expected.len(),
        3,
        "{label}: the root holds the tree's three entries"
    );

    let clusters = volume.boot_sector().clusters();
    let our_free = (2..clusters + 2)
        .filter(|&c| volume.entry(c).unwrap() == TableEntry::Free)
        .count();
    assert_eq!(our_free as u32, free, "{label}: the free counts differ");
}

#[test]
fn this_crate_reads_what_rust_fatfs_wrote_on_fat12() {
    agree("fat12", 4, lamfat::FatType::Fat12, 512);
}

#[test]
fn this_crate_reads_what_rust_fatfs_wrote_on_fat16() {
    agree("fat16", 32, lamfat::FatType::Fat16, 512);
}

#[test]
fn this_crate_reads_what_rust_fatfs_wrote_on_fat16_with_4096_byte_sectors() {
    agree("fat16-4k", 64, lamfat::FatType::Fat16, 4096);
}

#[test]
fn this_crate_reads_what_rust_fatfs_wrote_on_fat32() {
    agree("fat32", 64, lamfat::FatType::Fat32, 512);
}

#[test]
fn rust_fatfs_reads_what_this_crate_wrote() {
    const FILE_LEN: usize = 100_000;
    let total = 70_000 * 8 + 2_000;
    let layout = VolumeLayout::new(total, 0, FILE_LEN as u64, 0x1234_5678, "BOOTRISCV64.EFI")
        .expect("a FAT32 volume");
    let mut image = vec![0u8; total as usize * SECTOR_BYTES];
    for index in 0..layout.metadata_sectors() {
        let at = index as usize * SECTOR_BYTES;
        assert!(layout.fill_sector(index, &mut image[at..at + SECTOR_BYTES]));
    }
    let body: Vec<u8> = (0..FILE_LEN).map(|i| (i % 251) as u8).collect();
    let at = layout.file_first_sector() as usize * SECTOR_BYTES;
    image[at..at + FILE_LEN].copy_from_slice(&body);
    check_with_fsck(&image, "writer");

    let fs = lamfat::FileSystem::new(
        lamfat::StdIoWrapper::from(Cursor::new(image.clone())),
        lamfat::FsOptions::new(),
    )
    .expect("rust-fatfs mounts this crate's volume");
    let mut read = Vec::new();
    fs.root_dir()
        .open_file("EFI/BOOT/BOOTRISCV64.EFI")
        .expect("rust-fatfs finds the long name")
        .read_to_end_compat(&mut read)
        .unwrap();
    assert_eq!(read, body);
    let free = fs.stats().unwrap().free_clusters();

    let mut volume = Volume::open(Memory {
        bytes: &image,
        size: SectorSize::Bytes512,
    })
    .unwrap();
    let records = our_records(&mut volume, &image, 0);
    assert_eq!(records.len(), 1);
    let boot = &records[0].children[0];
    assert_eq!(boot.short, "BOOT");
    let file = &boot.children[0];
    assert_eq!(file.short, "BOOTRI~1.EFI");
    assert_eq!(
        file.long.as_deref(),
        Some(&"BOOTRISCV64.EFI".encode_utf16().collect::<Vec<_>>()[..])
    );
    assert_eq!(file.bytes, body);
    let clusters = volume.boot_sector().clusters();
    let our_free = (2..clusters + 2)
        .filter(|&c| volume.entry(c).unwrap() == TableEntry::Free)
        .count();
    assert_eq!(our_free as u32, free);
}
