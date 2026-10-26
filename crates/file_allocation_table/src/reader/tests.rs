//! Chains walked over a small FAT12 volume built byte by byte: a chain across the sector boundary
//! FAT12 entries straddle, every way a chain can break, a loop, and a device that fails.

use super::*;
use crate::table::TableEntry;

/// Sectors in the test volume: 1 reserved, two tables of 2, a root of 1, and 350 clusters.
pub(crate) const SECTORS: usize = 356;
/// Data clusters in the test volume.
pub(crate) const CLUSTERS: u32 = 350;
/// The first data sector, where cluster 2 begins.
pub(crate) const FIRST_DATA: usize = 6;

/// The sectors the image stores: the metadata and the first 42 clusters, which every test that
/// reads data uses. The device reads the rest as zeros, which keeps the image small enough to move
/// by value on a test thread's stack.
const STORED: usize = 48;

/// A FAT12 volume in memory, and the one sector index its device refuses, if any.
pub(crate) struct Image {
    pub(crate) bytes: [u8; STORED * 512],
    pub(crate) refuse: Option<u64>,
}

impl SectorDevice for Image {
    type Error = u64;

    fn sector_size(&self) -> SectorSize {
        SectorSize::Bytes512
    }

    fn read_sector(&mut self, index: u64, buffer: &mut [u8]) -> Result<(), u64> {
        if self.refuse == Some(index) || index >= SECTORS as u64 {
            return Err(index);
        }
        let start = index as usize * 512;
        match self.bytes.get(start..start + 512) {
            Some(bytes) => buffer.copy_from_slice(bytes),
            None => buffer.fill(0),
        }
        Ok(())
    }
}

impl Image {
    /// An empty FAT12 volume: 512-byte sectors, one per cluster, a 16-entry root region.
    pub(crate) fn fat12() -> Image {
        let mut bytes = [0u8; STORED * 512];
        let boot = &mut bytes[..512];
        boot[0..3].copy_from_slice(&[0xEB, 0x3C, 0x90]);
        boot[11..13].copy_from_slice(&512u16.to_le_bytes());
        boot[13] = 1;
        boot[14..16].copy_from_slice(&1u16.to_le_bytes());
        boot[16] = 2;
        boot[17..19].copy_from_slice(&16u16.to_le_bytes());
        boot[19..21].copy_from_slice(&(SECTORS as u16).to_le_bytes());
        boot[21] = 0xF8;
        boot[22..24].copy_from_slice(&2u16.to_le_bytes());
        boot[510] = 0x55;
        boot[511] = 0xAA;
        let mut image = Image {
            bytes,
            refuse: None,
        };
        image.set(0, 0xFF8);
        image.set(1, 0xFFF);
        image
    }

    /// Set `cluster`'s 12-bit entry to `value` in both tables.
    pub(crate) fn set(&mut self, cluster: u32, value: u16) {
        for table in [512, 3 * 512] {
            let at = table + cluster as usize * 3 / 2;
            let pair = u16::from_le_bytes([self.bytes[at], self.bytes[at + 1]]);
            let pair = if cluster & 1 == 1 {
                (pair & 0x000F) | (value << 4)
            } else {
                (pair & 0xF000) | (value & 0xFFF)
            };
            self.bytes[at..at + 2].copy_from_slice(&pair.to_le_bytes());
        }
    }

    /// Link `clusters` into one chain ending in an end-of-chain marker.
    pub(crate) fn link(&mut self, clusters: &[u32]) {
        for pair in clusters.windows(2) {
            self.set(pair[0], pair[1] as u16);
        }
        self.set(*clusters.last().unwrap(), 0xFFF);
    }
}

/// What a walk yielded, step by step.
type Steps<const N: usize> = [Option<Result<u32, ChainError<u64>>>; N];

/// Walk the chain from `first` on `image`, keeping at most `N` results.
fn walk<const N: usize>(image: Image, first: u32) -> (Steps<N>, usize) {
    let mut volume = Volume::open(image).unwrap();
    let mut out = [None; N];
    let mut count = 0;
    for (slot, result) in out.iter_mut().zip(volume.chain(first)) {
        *slot = Some(result);
        count += 1;
    }
    (out, count)
}

#[test]
fn the_volume_opens_as_fat12_with_its_cluster_count() {
    let volume = Volume::open(Image::fat12()).unwrap();
    assert_eq!(volume.boot_sector().clusters(), CLUSTERS);
    assert_eq!(volume.boot_sector().first_data_sector(), FIRST_DATA as u32);
    assert_eq!(volume.into_device().refuse, None);
}

#[test]
fn a_chain_reads_across_the_entry_that_straddles_two_table_sectors() {
    // Cluster 341's entry starts at byte 511 of the first table sector and ends in the second.
    let mut image = Image::fat12();
    image.link(&[340, 341, 342, 7]);
    let (out, count) = walk::<8>(image, 340);
    assert_eq!(count, 4);
    assert_eq!(
        out[..4],
        [Some(Ok(340)), Some(Ok(341)), Some(Ok(342)), Some(Ok(7))]
    );
}

#[test]
fn a_one_cluster_chain_ends_at_its_marker_and_stays_ended() {
    let mut image = Image::fat12();
    image.set(9, 0xFF8);
    let mut volume = Volume::open(image).unwrap();
    let mut chain = volume.chain(9);
    assert_eq!(chain.next(), Some(Ok(9)));
    assert_eq!(chain.next(), None);
    assert_eq!(chain.next(), None);
}

#[test]
fn entries_decode_by_their_markers() {
    let mut image = Image::fat12();
    image.set(2, 0);
    image.set(3, 1);
    image.set(4, 0xFF7);
    image.set(5, 0xFFF);
    image.set(6, 0x123);
    let mut volume = Volume::open(image).unwrap();
    assert_eq!(volume.entry(2), Ok(TableEntry::Free));
    assert_eq!(volume.entry(3), Ok(TableEntry::Reserved));
    assert_eq!(volume.entry(4), Ok(TableEntry::Bad));
    assert_eq!(volume.entry(5), Ok(TableEntry::EndOfChain));
    assert_eq!(volume.entry(6), Ok(TableEntry::Next(0x123)));
    for cluster in [0, 1, CLUSTERS + 2, u32::MAX] {
        assert_eq!(
            volume.entry(cluster),
            Err(ChainError::NotADataCluster(cluster))
        );
    }
    assert!(volume.entry(CLUSTERS + 1).is_ok());
}

#[test]
fn a_start_that_is_not_a_data_cluster_is_refused_once() {
    for first in [0, 1, CLUSTERS + 2, u32::MAX] {
        let (out, count) = walk::<4>(Image::fat12(), first);
        assert_eq!(count, 1);
        assert_eq!(out[0], Some(Err(ChainError::NotADataCluster(first))));
    }
}

#[test]
fn every_entry_that_cannot_continue_a_chain_breaks_it_there() {
    for (value, entry) in [
        (0, TableEntry::Free),
        (1, TableEntry::Reserved),
        (0xFF7, TableEntry::Bad),
        (CLUSTERS as u16 + 2, TableEntry::Next(CLUSTERS + 2)),
        (0xFF6, TableEntry::Next(0xFF6)),
    ] {
        let mut image = Image::fat12();
        image.set(20, 21);
        image.set(21, value);
        let (out, count) = walk::<4>(image, 20);
        assert_eq!(count, 3, "entry {value:#x}");
        assert_eq!(
            out[..3],
            [
                Some(Ok(20)),
                Some(Ok(21)),
                Some(Err(ChainError::Broken { cluster: 21, entry }))
            ]
        );
    }
}

#[test]
fn a_loop_ends_after_as_many_clusters_as_the_volume_holds() {
    let mut image = Image::fat12();
    image.set(2, 3);
    image.set(3, 2);
    let mut volume = Volume::open(image).unwrap();
    let mut chain = volume.chain(2);
    for step in 0..CLUSTERS {
        assert_eq!(chain.next(), Some(Ok(2 + step % 2)));
    }
    assert_eq!(chain.next(), Some(Err(ChainError::TooLong)));
    assert_eq!(chain.next(), None);
}

#[test]
fn a_chain_through_every_cluster_is_not_too_long() {
    let mut image = Image::fat12();
    let all: [u32; CLUSTERS as usize] = core::array::from_fn(|i| i as u32 + 2);
    image.link(&all);
    let mut volume = Volume::open(image).unwrap();
    let mut count = 0;
    for (expected, result) in (2..).zip(volume.chain(2)) {
        assert_eq!(result, Ok(expected));
        count += 1;
    }
    assert_eq!(count, CLUSTERS);
}

#[test]
fn a_device_error_ends_the_walk_with_the_devices_own_error() {
    let mut image = Image::fat12();
    image.link(&[2, 3]);
    image.refuse = Some(1);
    let (out, count) = walk::<4>(image, 2);
    assert_eq!(count, 2);
    assert_eq!(out[..2], [Some(Ok(2)), Some(Err(ChainError::Device(1)))]);
}

#[test]
fn an_entry_inside_one_sector_reads_only_that_sector() {
    // Cluster 2's entry sits at bytes 3 and 4 of the first table sector; the next sector, the
    // table's second, is refused, so a read that strays into it fails.
    let mut image = Image::fat12();
    image.set(2, 0xFFF);
    image.refuse = Some(2);
    let mut volume = Volume::open(image).unwrap();
    assert_eq!(volume.entry(2), Ok(TableEntry::EndOfChain));
}

#[test]
fn the_cache_reloads_when_a_walk_moves_between_table_sectors() {
    let mut image = Image::fat12();
    image.link(&[345, 3, 346, 4]);
    let (out, count) = walk::<8>(image, 345);
    assert_eq!(count, 4);
    assert_eq!(
        out[..4],
        [Some(Ok(345)), Some(Ok(3)), Some(Ok(346)), Some(Ok(4))]
    );
}

#[test]
fn a_live_fat32_table_is_the_one_read() {
    // A FAT32 boot sector naming table 1 live, its geometry checked by the boot-sector tests.
    let shape = crate::boot_sector::tests::fat32_with_active_table(1);
    let boot = BootSector::parse(&shape).unwrap();
    let (sector, within) = crate::table::entry_position(&boot, 2).unwrap();
    assert_eq!(sector, boot.table_first_sector(1).unwrap());
    assert_eq!(within, 8);
}
