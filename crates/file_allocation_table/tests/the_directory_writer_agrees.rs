//! **rust-fatfs and `fsck` agree with the directories, long names and renames this crate writes.**
//!
//! On FAT12, FAT16 and FAT32 volumes rust-fatfs formatted, this crate makes directories, creates
//! files whose names need long-name slots (and whose aliases collide, so the numbering shows),
//! renames within and across directories, and removes. After each stage rust-fatfs must list the
//! same tree and `fsck -n` must accept the volume.

use std::io::{Cursor, Read};
use std::path::Path;
use std::process::Command;

use file_allocation_table::{
    RootDirectory, SectorDevice, SectorSize, Volume, WritableSectorDevice, WriteError,
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

/// Every entry under `dir` according to rust-fatfs: (path, is directory, bytes of a file).
fn walk(
    dir: &lamfat::Dir<
        '_,
        lamfat::StdIoWrapper<Cursor<Vec<u8>>>,
        lamfat::DefaultTimeProvider,
        lamfat::LossyOemCpConverter,
    >,
    prefix: &str,
    out: &mut Vec<(String, bool, Vec<u8>)>,
) {
    for entry in dir.iter() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if name == "." || name == ".." {
            continue;
        }
        let path = format!("{prefix}/{name}");
        if entry.is_dir() {
            out.push((path.clone(), true, Vec::new()));
            walk(&entry.to_dir(), &path, out);
        } else {
            let mut bytes = Vec::new();
            entry.to_file().read_to_end(&mut bytes).unwrap();
            out.push((path, false, bytes));
        }
    }
}

fn oracle_tree(image: &[u8]) -> Vec<(String, bool, Vec<u8>)> {
    let fs = lamfat::FileSystem::new(
        lamfat::StdIoWrapper::from(Cursor::new(image.to_vec())),
        lamfat::FsOptions::new(),
    )
    .expect("rust-fatfs mounts the volume");
    let mut out = Vec::new();
    walk(&fs.root_dir(), "", &mut out);
    out.sort();
    out
}

fn put(volume: &mut Volume<Memory>, directory: u32, name: &str, bytes: &[u8]) {
    let entry = volume.create_file(directory, name).expect(name);
    volume
        .open_file_writer(&entry)
        .unwrap()
        .write(bytes)
        .unwrap();
}

fn tree_of(entries: &[(&str, bool, &[u8])]) -> Vec<(String, bool, Vec<u8>)> {
    let mut v: Vec<_> = entries
        .iter()
        .map(|(p, d, b)| (p.to_string(), *d, b.to_vec()))
        .collect();
    v.sort();
    v
}

fn scenario(label: &str, mebibytes: usize, fat_type: FatType) {
    let mut volume = Volume::open(formatted(mebibytes, fat_type, 512)).unwrap();
    let total = volume.count_free_clusters().unwrap();
    let long_255 = "x".repeat(251) + ".txt";
    let names = [
        "Hello World.txt",
        "lower.txt",
        "NINECHARS.TXT",
        "two.dots.in.name.md",
        "\u{dc}n\u{ef}code \u{2603}.txt",
        long_255.as_str(),
    ];
    for (i, name) in names.iter().enumerate() {
        put(&mut volume, 0, name, format!("body {i}").as_bytes());
    }
    // Fifteen names with one alias base: ALONGN~1 to ALONGN~9, then ALONG~10 and up.
    for i in 0..15 {
        put(&mut volume, 0, &format!("A Long Name Number {i}.txt"), b"n");
    }
    assert_eq!(
        volume.create_file(0, "HELLO world.TXT"),
        Err(WriteError::AlreadyExists)
    );
    volume.sync().unwrap();
    let image = volume.into_device();
    let tree = oracle_tree(&image.bytes);
    for (i, name) in names.iter().enumerate() {
        let found = tree
            .iter()
            .find(|(p, _, _)| p == &format!("/{name}"))
            .unwrap_or_else(|| panic!("{label}: {name}"));
        assert_eq!(found.2, format!("body {i}").as_bytes());
    }
    assert_eq!(tree.len(), names.len() + 15, "{label}");
    check_with_fsck(&image.bytes, &format!("{label}-names"));

    // Directories, a file inside, renames across them, and the refusals.
    let mut volume = Volume::open(image).unwrap();
    let docs = volume.make_directory(0, "Documents And Settings").unwrap();
    let docs_cluster = docs.first_cluster();
    let sub = volume.make_directory(docs_cluster, "Sub").unwrap();
    put(&mut volume, docs_cluster, "notes.txt", b"notes");
    put(&mut volume, sub.first_cluster(), "deep file.txt", b"deep");
    assert_eq!(
        volume.make_directory(0, "documents and settings"),
        Err(WriteError::AlreadyExists)
    );
    assert_eq!(
        volume.remove_directory(&docs),
        Err(WriteError::DirectoryNotEmpty)
    );
    assert_eq!(
        volume.rename(&docs, sub.first_cluster(), "Moved"),
        Err(WriteError::MovedIntoItself)
    );
    assert_eq!(
        volume.rename(&docs, docs_cluster, "Moved"),
        Err(WriteError::MovedIntoItself)
    );
    let notes = volume.lookup("Documents And Settings/notes.txt").unwrap();
    let moved = volume.rename(&notes, 0, "Renamed Notes.txt").unwrap();
    assert_eq!(moved.size_bytes(), 5);
    // The directory with its contents moves under another parent, and its `..` follows.
    let sub = volume.lookup("Documents And Settings/Sub").unwrap();
    volume.rename(&sub, 0, "Sub Moved").unwrap();
    let case_only = volume.lookup("lower.txt").unwrap();
    volume.rename(&case_only, 0, "LOWER.txt").unwrap();
    assert_eq!(
        volume.rename(&moved, 0, "Hello World.txt"),
        Err(WriteError::AlreadyExists)
    );
    volume.sync().unwrap();
    let image = volume.into_device();
    let tree = oracle_tree(&image.bytes);
    for (path, directory, bytes) in tree_of(&[
        ("/Documents And Settings", true, b""),
        ("/Sub Moved", true, b""),
        ("/Sub Moved/deep file.txt", false, b"deep"),
        ("/Renamed Notes.txt", false, b"notes"),
        ("/LOWER.txt", false, b"body 1"),
    ]) {
        assert!(
            tree.contains(&(path.clone(), directory, bytes)),
            "{label}: {path}"
        );
    }
    assert!(
        !tree
            .iter()
            .any(|(p, _, _)| p == "/lower.txt" || p.starts_with("/Documents And Settings/"))
    );
    check_with_fsck(&image.bytes, &format!("{label}-moved"));

    // Remove everything, directories last, and the volume is empty.
    let mut volume = Volume::open(image).unwrap();
    for path in ["Sub Moved/deep file.txt", "Renamed Notes.txt"] {
        let entry = volume.lookup(path).unwrap();
        volume.remove_file(&entry).unwrap();
    }
    for path in ["Sub Moved", "Documents And Settings"] {
        let entry = volume.lookup(path).unwrap();
        volume.remove_directory(&entry).unwrap();
    }
    let rest: Vec<String> = volume
        .root_entries()
        .map(|e| e.unwrap())
        .filter(|e| !e.is_volume_label())
        .map(|e| e.long_name().map_or(String::new(), |_| "x".to_string()))
        .collect();
    assert_eq!(rest.len(), names.len() + 15, "{label}");
    volume.sync().unwrap();
    let image = volume.into_device();
    check_with_fsck(&image.bytes, &format!("{label}-removed"));
    assert_eq!(oracle_tree(&image.bytes).len(), names.len() + 15, "{label}");
    let _ = total;
}

#[test]
fn directories_long_names_and_renames_agree_on_fat12() {
    scenario("dirs-fat12", 8, FatType::Fat12);
}

#[test]
fn directories_long_names_and_renames_agree_on_fat16() {
    scenario("dirs-fat16", 32, FatType::Fat16);
}

#[test]
fn directories_long_names_and_renames_agree_on_fat32() {
    scenario("dirs-fat32", 64, FatType::Fat32);
}

/// A FAT32 volume with 512-byte clusters and enough fill that every new cluster number is above
/// 65535: directories made there need the high half of the cluster number in their entry, in
/// their `.`, and in a child's `..`.
fn high_volume() -> Volume<Memory> {
    let mut image = formatted(64, FatType::Fat32, 512);
    let mut cursor = Cursor::new(&mut image.bytes);
    lamfat::format_volume(
        &mut lamfat::StdIoWrapper::from(&mut cursor),
        lamfat::FormatVolumeOptions::new()
            .fat_type(FatType::Fat32)
            .bytes_per_cluster(512),
    )
    .unwrap();
    let mut volume = Volume::open(image).unwrap();
    let filler = vec![0x5Au8; 1 << 20];
    let entry = volume.create_file(0, "FILL.BIN").unwrap();
    let mut writer = volume.open_file_writer(&entry).unwrap();
    for _ in 0..34 {
        writer.write(&filler).unwrap();
    }
    volume
}

#[test]
fn directories_above_cluster_65535_keep_their_high_bits_through_a_move() {
    let mut volume = high_volume();
    let RootDirectory::Cluster(root_cluster) = volume.boot_sector().root_directory() else {
        panic!("FAT32 keeps its root in a chain");
    };
    // The root named by its cluster number is the root.
    let a = volume.make_directory(root_cluster, "High A").unwrap();
    assert!(a.first_cluster() > 65_535);
    let b = volume.make_directory(a.first_cluster(), "High B").unwrap();
    let c = volume.make_directory(0, "High C").unwrap();
    put(&mut volume, b.first_cluster(), "inside b.txt", b"b");
    // A move below itself is refused even when every `..` on the way needs its high half.
    assert_eq!(
        volume.rename(&a, b.first_cluster(), "x"),
        Err(WriteError::MovedIntoItself)
    );
    // And a move to a high parent rewrites `..` with its high half.
    let moved = volume
        .rename(&b, c.first_cluster(), "High B Moved")
        .unwrap();
    assert_eq!(moved.first_cluster(), b.first_cluster());
    put(&mut volume, moved.first_cluster(), "second.txt", b"2");
    // High C sat under the root (its `..` is 0); moving it under High A gives it a `..` that needs
    // its high half, and High A is then an ancestor of everything below.
    let c_moved = volume
        .rename(&c, a.first_cluster(), "High C Moved")
        .unwrap();
    assert_eq!(
        volume.rename(&a, moved.first_cluster(), "y"),
        Err(WriteError::MovedIntoItself)
    );
    let _ = c_moved;
    volume.sync().unwrap();
    let image = volume.into_device();
    let tree = oracle_tree(&image.bytes);
    for (path, directory) in [
        ("/High A", true),
        ("/High A/High C Moved", true),
        ("/High A/High C Moved/High B Moved", true),
        ("/High A/High C Moved/High B Moved/inside b.txt", false),
        ("/High A/High C Moved/High B Moved/second.txt", false),
    ] {
        assert!(
            tree.iter().any(|(p, d, _)| p == path && *d == directory),
            "{path}"
        );
    }
    check_with_fsck(&image.bytes, "high-directories");
}

#[test]
fn a_new_directory_in_a_multi_sector_cluster_is_zeroed_past_its_dots() {
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
    let boot = *Volume::open(Memory {
        bytes: image.bytes.clone(),
        size: image.size,
    })
    .unwrap()
    .boot_sector();
    let data = boot.first_data_sector() as usize * 512;
    image.bytes[data + 2048..].fill(0xAA);
    let mut volume = Volume::open(image).unwrap();
    volume.make_directory(0, "Fresh").unwrap();
    volume.sync().unwrap();
    let image = volume.into_device();
    let tree = oracle_tree(&image.bytes);
    assert_eq!(tree, tree_of(&[("/Fresh", true, b"")]));
    check_with_fsck(&image.bytes, "fresh-dir");
}
