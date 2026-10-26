//! Fuzz the FAT reader with an arbitrary volume: its boot sector, its table, its chains and its
//! directories.
//!
//! The input is the volume, read through `SectorDevice` the way a caller's device would serve it,
//! once at each of the four sector sizes. So the dosfstools-made boot sectors in
//! `crates/file_allocation_table/tests/fixtures` seed it unmodified. The crate's Kani
//! harnesses cover `BootSector::parse` on every 512-byte sector; this reaches the sizes above 512,
//! the device path in `BootSector::read`, and every accessor a caller reaches for afterwards, and
//! checks the same geometry the harnesses prove. Then it opens the volume and walks its tree,
//! holding every chain to the volume's data clusters and count, which the harnesses prove only on
//! small volumes (notes/fuzzing/targets.md).

#![no_main]

use file_allocation_table::{
    BootSector, ChainError, DirectoryEntry, RootDirectory, SectorDevice, SectorSize, Volume,
};
use libfuzzer_sys::fuzz_target;

/// The volume's bytes, served a sector at a time; a read past the end is the device's error.
struct Image<'a> {
    bytes: &'a [u8],
    size: SectorSize,
}

impl SectorDevice for Image<'_> {
    type Error = ();

    fn sector_size(&self) -> SectorSize {
        self.size
    }

    fn read_sector(&mut self, index: u64, buffer: &mut [u8]) -> Result<(), ()> {
        assert_eq!(buffer.len(), self.size.bytes());
        let start = usize::try_from(index)
            .map_err(|_| ())?
            .checked_mul(buffer.len())
            .ok_or(())?;
        let end = start.checked_add(buffer.len()).ok_or(())?;
        buffer.copy_from_slice(self.bytes.get(start..end).ok_or(())?);
        Ok(())
    }
}

fuzz_target!(|data: &[u8]| {
    for size in [
        SectorSize::Bytes512,
        SectorSize::Bytes1024,
        SectorSize::Bytes2048,
        SectorSize::Bytes4096,
    ] {
        check(data, size);
        walk(data, size);
    }
});

/// Read `bytes` as a volume on a device of `size` sectors and hold what is accepted to the
/// geometry the crate promises.
fn check(bytes: &[u8], size: SectorSize) {
    let mut device = Image { bytes, size };
    let Ok(boot) = BootSector::read(&mut device) else {
        return;
    };
    assert_eq!(boot.sector_size(), size);
    assert_eq!(BootSector::parse(bytes), Ok(boot));

    let total = u64::from(boot.total_sectors());
    let per_cluster = u64::from(boot.sectors_per_cluster());
    let data_end = u64::from(boot.first_data_sector()) + u64::from(boot.clusters()) * per_cluster;
    assert!(boot.clusters() >= 1 && data_end <= total);
    assert_eq!(
        u64::from(boot.bytes_per_cluster()),
        per_cluster * size.bytes() as u64
    );

    for index in 0..=boot.table_count() {
        match boot.table_first_sector(index) {
            Some(first) => assert!(u64::from(first) + u64::from(boot.table_sectors()) <= total),
            None => assert_eq!(index, boot.table_count()),
        }
    }
    if let RootDirectory::Cluster(root) = boot.root_directory() {
        assert!(boot.cluster_first_sector(root).is_some());
    }
    let last = boot.clusters() + 1;
    for cluster in [0, 1, 2, 3, last - 1, last, last + 1, u32::MAX] {
        let is_data = (2..=last).contains(&cluster);
        match boot.cluster_first_sector(cluster) {
            Some(first) => assert!(is_data && u64::from(first) + per_cluster <= total),
            None => assert!(!is_data),
        }
    }
    let _ = (
        boot.fat_type(),
        boot.media(),
        boot.hidden_sectors(),
        boot.active_table(),
        boot.fs_info_sector(),
        boot.backup_boot_sector(),
        boot.volume_id(),
        boot.reserved_sectors(),
    );
}

/// The most chain steps and directory slots one input may cost, so that a hostile volume claiming
/// millions of clusters still runs in milliseconds. The walk's own bound is Kani's to prove.
const BUDGET: u32 = 1 << 14;

/// How deep the walk follows subdirectories.
const DEPTH: u32 = 4;

/// Open the volume and walk its tree: every directory's slots, and every short entry's chain,
/// holding each chain to the volume's clusters.
fn walk(bytes: &[u8], size: SectorSize) {
    let Ok(mut volume) = Volume::open(Image { bytes, size }) else {
        return;
    };
    let clusters = volume.boot_sector().clusters();
    let last = clusters + 1;
    for cluster in [0, 1, 2, last, last + 1, u32::MAX] {
        let is_data = (2..=last).contains(&cluster);
        match volume.entry(cluster) {
            Err(ChainError::NotADataCluster(c)) => assert!(!is_data && c == cluster),
            Ok(_) | Err(ChainError::Device(())) => assert!(is_data),
            Err(other) => panic!("an entry read failed with {other:?}"),
        }
    }
    let mut budget = BUDGET;
    directory(&mut volume, 0, DEPTH, &mut budget);
}

/// Read the directory starting at `first` (0 for the root) and what it names.
fn directory(volume: &mut Volume<Image<'_>>, first: u32, depth: u32, budget: &mut u32) {
    let mut entries = Vec::new();
    let mut read = volume.directory(first);
    loop {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        match read.next() {
            Some(Ok(DirectoryEntry::Short(entry))) => entries.push(entry),
            Some(Ok(DirectoryEntry::LongName(piece))) => {
                let _ = (
                    piece.ordinal(),
                    piece.is_last(),
                    piece.checksum(),
                    piece.units(),
                );
            }
            Some(Err(_)) => {
                assert!(
                    read.next().is_none(),
                    "a directory read went on after an error"
                );
                break;
            }
            None => {
                assert!(
                    read.next().is_none(),
                    "a directory read went on after its end"
                );
                break;
            }
        }
    }
    for entry in entries {
        let name = entry.name();
        if entry.is_volume_label() || name[0] == b'.' || entry.first_cluster() == 0 {
            continue;
        }
        chain(volume, entry.first_cluster(), budget);
        if entry.is_directory() && depth > 0 {
            directory(volume, entry.first_cluster(), depth - 1, budget);
        }
    }
}

/// Walk the chain from `first` and hold it to the volume's data clusters and count.
fn chain(volume: &mut Volume<Image<'_>>, first: u32, budget: &mut u32) {
    let clusters = volume.boot_sector().clusters();
    let mut yielded = 0u32;
    let mut walk = volume.chain(first);
    loop {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        match walk.next() {
            Some(Ok(cluster)) => {
                assert!((2..=clusters + 1).contains(&cluster));
                yielded += 1;
                assert!(yielded <= clusters, "a chain outlasted the volume");
            }
            Some(Err(ChainError::TooLong)) => {
                assert_eq!(yielded, clusters);
                break;
            }
            Some(Err(_)) | None => break,
        }
    }
    assert!(walk.next().is_none(), "a chain went on after it ended");
}
