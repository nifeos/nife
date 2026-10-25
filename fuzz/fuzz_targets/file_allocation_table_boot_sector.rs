//! Fuzz the FAT reader's boot-sector read with the head of an arbitrary volume.
//!
//! The input is the volume, read through `SectorDevice` the way a caller's device would serve it,
//! once at each of the four sector sizes. So the dosfstools-made boot sectors in
//! `crates/file_allocation_table/tests/fixtures` seed it unmodified. The crate's Kani
//! harnesses cover `BootSector::parse` on every 512-byte sector; this reaches the sizes above 512,
//! the device path in `BootSector::read`, and every accessor a caller reaches for afterwards, and
//! checks the same geometry the harnesses prove (notes/fuzzing/targets.md).

#![no_main]

use file_allocation_table::{BootSector, RootDirectory, SectorDevice, SectorSize};
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

    for index in 0..=boot.tables() {
        match boot.table_first_sector(index) {
            Some(first) => assert!(u64::from(first) + u64::from(boot.table_sectors()) <= total),
            None => assert_eq!(index, boot.tables()),
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
