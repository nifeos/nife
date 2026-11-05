//! Allocation and release over the small FAT12 volume `reader::tests` builds: the first free
//! cluster from a hint, a wrap, a full volume, a chain freed and cut, every way a chain can break,
//! entries that straddle a table sector, both table copies, and a device that refuses a write.
//!
//! Name: ratified 2026-11-01 (UTC), by the class rule on the `tests` and `verification` modules of this crate.

use super::*;
use crate::reader::tests::{CLUSTERS, Image};
use crate::{NoClock, WritableVolume};

/// The raw 12-bit entry of `cluster` in table copy `copy` (0 or 1) of `image`.
fn raw(image: &Image, copy: usize, cluster: u32) -> u16 {
    let at = (1 + copy * 2) * 512 + cluster as usize * 3 / 2;
    let pair = u16::from_le_bytes([image.bytes[at], image.bytes[at + 1]]);
    if cluster & 1 == 1 {
        pair >> 4
    } else {
        pair & 0xFFF
    }
}

#[test]
fn allocation_takes_the_first_free_cluster_from_the_hint_and_ends_the_chain_there() {
    let mut volume = WritableVolume::open(Image::fat12(), NoClock).unwrap();
    assert_eq!(volume.allocate_cluster(None), Ok(2));
    assert_eq!(volume.allocate_cluster(None), Ok(3));
    assert_eq!(volume.table_entry(2), Ok(TableEntry::EndOfChain));
    assert_eq!(volume.table_entry(4), Ok(TableEntry::Free));
    let image = volume.into_device();
    for copy in 0..2 {
        assert_eq!(raw(&image, copy, 2), 0xFFF);
        assert_eq!(raw(&image, copy, 3), 0xFFF);
        assert_eq!(raw(&image, copy, 4), 0);
    }
}

#[test]
fn extending_a_chain_links_the_old_end_to_the_new_cluster() {
    let mut volume = WritableVolume::open(Image::fat12(), NoClock).unwrap();
    let first = volume.allocate_cluster(None).unwrap();
    let second = volume.allocate_cluster(Some(first)).unwrap();
    let third = volume.allocate_cluster(Some(second)).unwrap();
    let chain: [u32; 3] = [first, second, third];
    let mut walked = [0u32; 3];
    for (slot, cluster) in walked.iter_mut().zip(volume.chain(first)) {
        *slot = cluster.unwrap();
    }
    assert_eq!(walked, chain);
    assert_eq!(
        volume.allocate_cluster(Some(first)),
        Err(AllocationError::NotTheEndOfAChain(first))
    );
}

#[test]
fn the_search_wraps_past_the_last_cluster_to_the_first_free_one() {
    let mut image = Image::fat12();
    image.set(2, 0xFFF);
    image.set(3, 0xFFF);
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    volume.writer.next_free = CLUSTERS + 1;
    assert_eq!(volume.allocate_cluster(None), Ok(CLUSTERS + 1));
    assert_eq!(volume.writer.next_free, 2);
    assert_eq!(volume.allocate_cluster(None), Ok(4));
}

#[test]
fn a_full_volume_is_refused_and_one_freed_cluster_is_found_again() {
    let mut image = Image::fat12();
    for cluster in 2..CLUSTERS + 2 {
        image.set(cluster, 0xFFF);
    }
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    assert_eq!(
        volume.allocate_cluster(None),
        Err(AllocationError::VolumeFull)
    );
    assert_eq!(volume.deallocate_chain(200), Ok(1));
    assert_eq!(volume.allocate_cluster(None), Ok(200));
}

#[test]
fn a_bad_or_reserved_cluster_is_never_allocated() {
    let mut image = Image::fat12();
    for cluster in 2..CLUSTERS + 2 {
        image.set(cluster, if cluster % 2 == 0 { 0xFF7 } else { 1 });
    }
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    assert_eq!(
        volume.allocate_cluster(None),
        Err(AllocationError::VolumeFull)
    );
}

#[test]
fn every_entry_stores_without_disturbing_its_neighbors_across_the_table() {
    let mut volume = WritableVolume::open(Image::fat12(), NoClock).unwrap();
    for cluster in 2..CLUSTERS + 2 {
        let entry = TableEntry::Next(2 + (cluster * 7) % CLUSTERS);
        volume.write_table_entry(cluster, entry).unwrap();
    }
    for cluster in 2..CLUSTERS + 2 {
        let entry = TableEntry::Next(2 + (cluster * 7) % CLUSTERS);
        assert_eq!(volume.table_entry(cluster), Ok(entry), "cluster {cluster}");
    }
    let image = volume.into_device();
    for cluster in 2..CLUSTERS + 2 {
        assert_eq!(raw(&image, 0, cluster), raw(&image, 1, cluster));
    }
    // The two reserved entries and the table's tail are untouched.
    assert_eq!(raw(&image, 0, 0), 0xFF8);
    assert_eq!(raw(&image, 0, 1), 0xFFF);
}

#[test]
fn a_straddling_entry_rewrites_both_of_its_sectors() {
    // Cluster 341's entry starts at byte 511 of the first table sector.
    let mut volume = WritableVolume::open(Image::fat12(), NoClock).unwrap();
    volume
        .write_table_entry(341, TableEntry::Next(0x123))
        .unwrap();
    volume
        .write_table_entry(340, TableEntry::Next(0x456))
        .unwrap();
    volume
        .write_table_entry(342, TableEntry::Next(0x789))
        .unwrap();
    assert_eq!(volume.table_entry(341), Ok(TableEntry::Next(0x123)));
    assert_eq!(volume.table_entry(340), Ok(TableEntry::Next(0x456)));
    assert_eq!(volume.table_entry(342), Ok(TableEntry::Next(0x789)));
}

#[test]
fn only_a_data_cluster_has_an_entry_to_write() {
    let mut volume = WritableVolume::open(Image::fat12(), NoClock).unwrap();
    for cluster in [0, 1, CLUSTERS + 2, u32::MAX] {
        assert_eq!(
            volume.write_table_entry(cluster, TableEntry::Free),
            Err(ChainError::NotADataCluster(cluster))
        );
    }
}

#[test]
fn freeing_a_chain_frees_every_cluster_and_counts_them() {
    let mut image = Image::fat12();
    image.link(&[5, 9, 6, 20]);
    image.link(&[30, 31]);
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    assert_eq!(volume.deallocate_chain(5), Ok(4));
    for cluster in [5, 9, 6, 20] {
        assert_eq!(volume.table_entry(cluster), Ok(TableEntry::Free));
    }
    assert_eq!(volume.table_entry(30), Ok(TableEntry::Next(31)));
    assert_eq!(volume.count_free_clusters(), Ok(CLUSTERS - 2));
}

#[test]
fn a_cut_keeps_the_head_ends_it_and_frees_the_tail() {
    for keep in 1..=4u32 {
        let mut image = Image::fat12();
        image.link(&[5, 9, 6, 20]);
        let mut volume = WritableVolume::open(image, NoClock).unwrap();
        assert_eq!(volume.truncate_chain(5, keep), Ok(4 - keep));
        let mut kept = 0;
        for cluster in volume.chain(5) {
            cluster.unwrap();
            kept += 1;
        }
        assert_eq!(kept, keep);
        assert_eq!(volume.count_free_clusters(), Ok(CLUSTERS - keep));
    }
}

#[test]
fn a_cut_past_the_end_frees_nothing_and_a_cut_to_zero_frees_all() {
    let mut image = Image::fat12();
    image.link(&[5, 9]);
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    assert_eq!(volume.truncate_chain(5, 9), Ok(0));
    assert_eq!(volume.truncate_chain(5, 0), Ok(2));
    assert_eq!(volume.count_free_clusters(), Ok(CLUSTERS));
}

#[test]
fn a_broken_chain_frees_what_precedes_the_break_and_reports_it() {
    let mut image = Image::fat12();
    image.set(5, 9);
    image.set(9, 0xFF7);
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    assert_eq!(
        volume.deallocate_chain(5),
        Err(AllocationError::Chain(ChainError::Broken {
            cluster: 9,
            entry: TableEntry::Bad
        }))
    );
    assert_eq!(volume.table_entry(5), Ok(TableEntry::Free));
    assert_eq!(volume.table_entry(9), Ok(TableEntry::Bad));
    assert_eq!(
        volume.deallocate_chain(1),
        Err(AllocationError::Chain(ChainError::NotADataCluster(1)))
    );
}

#[test]
fn a_looping_chain_is_freed_once_and_ends_at_the_cluster_it_already_freed() {
    let mut image = Image::fat12();
    image.set(5, 9);
    image.set(9, 5);
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    assert_eq!(
        volume.deallocate_chain(5),
        Err(AllocationError::Chain(ChainError::Broken {
            cluster: 5,
            entry: TableEntry::Free
        }))
    );
    assert_eq!(volume.table_entry(9), Ok(TableEntry::Free));
}

#[test]
fn a_refused_write_is_the_devices_error_and_a_failed_link_leaks_nothing() {
    // Cluster 341's entry spans table sectors 1 and 2; cluster 2's entry lies in sector 1 alone.
    let mut image = Image::fat12();
    image.set(341, 0xFFF);
    image.refuse_write = Some(2);
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    assert_eq!(
        volume.allocate_cluster(Some(341)),
        Err(AllocationError::Chain(ChainError::Device(2)))
    );
    // The new cluster was marked and released again, and the chain it would have joined is intact.
    assert_eq!(volume.table_entry(2), Ok(TableEntry::Free));
    assert_eq!(volume.table_entry(341), Ok(TableEntry::EndOfChain));
}

#[test]
fn the_hints_follow_allocation_and_release() {
    let mut volume = WritableVolume::open(Image::fat12(), NoClock).unwrap();
    volume.writer.free_clusters = Some(CLUSTERS);
    volume.writer.loaded = true;
    volume.allocate_cluster(None).unwrap();
    volume.allocate_cluster(None).unwrap();
    assert_eq!(volume.writer.free_clusters, Some(CLUSTERS - 2));
    assert_eq!(volume.writer.next_free, 4);
    volume.deallocate_chain(2).unwrap();
    assert_eq!(volume.writer.free_clusters, Some(CLUSTERS - 1));
    // A volume with no FSInfo has nowhere to write them, and a `mark_clean` is still clean.
    assert_eq!(volume.mark_clean(), Ok(()));
}

#[test]
fn a_chain_that_links_outside_the_volume_is_broken_and_left_alone() {
    let mut image = Image::fat12();
    // 400 is a legal 12-bit link and not one of the volume's 350 clusters.
    image.set(5, 400);
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    let broken = AllocationError::Chain(ChainError::Broken {
        cluster: 5,
        entry: TableEntry::Next(400),
    });
    assert_eq!(volume.deallocate_chain(5), Err(broken));
    assert_eq!(volume.truncate_chain(5, 1), Err(broken));
    assert_eq!(volume.table_entry(5), Ok(TableEntry::Next(400)));
}

#[test]
fn the_hint_wraps_to_the_first_cluster_only_after_the_last() {
    let mut volume = WritableVolume::open(Image::fat12(), NoClock).unwrap();
    volume.writer.next_free = CLUSTERS;
    assert_eq!(volume.allocate_cluster(None), Ok(CLUSTERS));
    assert_eq!(volume.writer.next_free, CLUSTERS + 1);
    assert_eq!(volume.allocate_cluster(None), Ok(CLUSTERS + 1));
    assert_eq!(volume.writer.next_free, 2);
}

#[test]
fn a_release_that_frees_nothing_leaves_the_hints_unchanged() {
    let mut image = Image::fat12();
    image.link(&[5, 9]);
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    assert_eq!(volume.truncate_chain(5, 2), Ok(0));
    assert!(!volume.writer.hints_changed);
    assert_eq!(volume.truncate_chain(5, 1), Ok(1));
    assert!(volume.writer.hints_changed);
}

#[test]
fn a_volume_over_a_borrowed_device_writes_through_it() {
    let mut image = Image::fat12();
    {
        let mut volume = WritableVolume::open(&mut image, NoClock).unwrap();
        assert_eq!(volume.allocate_cluster(None), Ok(2));
    }
    assert_eq!(raw(&image, 0, 2), 0xFFF);
}

// The oracle checks of allocation, release and cuts on volumes rust-fatfs formatted: after each
// step rust-fatfs's count of free clusters must equal this crate's, the table must be byte for
// byte what it was once every chain is freed, and the host's `fsck -n` must accept the volume. On
// FAT32, the `FSInfo` hints this crate writes must be the true ones. Not under Miri: they check
// volumes against the host's `fsck` through temporary files, which Miri's isolation refuses.

extern crate std;
#[cfg(not(miri))]
use std::format;
#[cfg(not(miri))]
use std::io::Cursor;
#[cfg(not(miri))]
use std::path::Path;
#[cfg(not(miri))]
use std::process::Command;
#[cfg(not(miri))]
use std::vec;
#[cfg(not(miri))]
use std::vec::Vec;
#[cfg(not(miri))]
use std::{eprintln, string::String};

#[cfg(not(miri))]
use lamfat::FatType;

#[cfg(not(miri))]
use crate::{SectorDevice, SectorSize, WritableSectorDevice};

#[cfg(not(miri))]
/// A volume held in memory, read and written one sector at a time.
struct Memory {
    bytes: Vec<u8>,
    size: SectorSize,
    /// Writes served so far.
    writes: usize,
}

#[cfg(not(miri))]
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

#[cfg(not(miri))]
impl WritableSectorDevice for Memory {
    fn write_sector(&mut self, index: u64, buffer: &[u8]) -> Result<(), u64> {
        let start = usize::try_from(index).map_err(|_| index)? * buffer.len();
        let bytes = self
            .bytes
            .get_mut(start..start + buffer.len())
            .ok_or(index)?;
        bytes.copy_from_slice(buffer);
        self.writes += 1;
        Ok(())
    }
}

#[cfg(not(miri))]
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
        writes: 0,
    }
}

#[cfg(not(miri))]
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

#[cfg(not(miri))]
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

#[cfg(not(miri))]
/// What rust-fatfs counts as free clusters on `image`.
fn oracle_free_clusters(image: &[u8]) -> u32 {
    let fs = lamfat::FileSystem::new(
        lamfat::StdIoWrapper::from(Cursor::new(image.to_vec())),
        lamfat::FsOptions::new(),
    )
    .expect("rust-fatfs mounts the volume");
    fs.stats().unwrap().free_clusters()
}

#[cfg(not(miri))]
/// The bytes of `image` that hold the boot sectors and the tables, with the `FSInfo` sector
/// blanked: what freeing every chain must put back.
fn metadata(image: &Memory) -> Vec<u8> {
    let volume = WritableVolume::open(
        Memory {
            bytes: image.bytes.clone(),
            size: image.size,
            writes: 0,
        },
        NoClock,
    )
    .unwrap();
    let boot = *volume.boot_sector();
    let sector = image.size.bytes();
    let end = boot.first_data_sector() as usize * sector;
    let mut bytes = image.bytes[..end].to_vec();
    if let Some(info) = boot.file_system_information_sector() {
        let at = usize::from(info) * sector;
        bytes[at..at + sector].fill(0);
    }
    bytes
}

#[cfg(not(miri))]
/// A small deterministic sequence, so a failure reproduces.
struct Sequence(u64);

#[cfg(not(miri))]
impl Sequence {
    fn next(&mut self, below: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % below
    }
}

#[cfg(not(miri))]
fn allocate_extend_cut_and_free(label: &str, mebibytes: usize, fat_type: FatType, sector: u16) {
    let image = formatted(mebibytes, fat_type, sector);
    let original = metadata(&image);
    check_with_fsck(&image.bytes, &format!("{label}-empty"));
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    let total = volume.count_free_clusters().unwrap();
    assert_eq!(total, oracle_free_clusters(&volume.into_device().bytes));
    let image = formatted(mebibytes, fat_type, sector);
    let mut volume = WritableVolume::open(image, NoClock).unwrap();

    // A chain of 40 clusters, then a second of 7, then the first cut to 12.
    let mut first = [0u32; 2];
    let mut lengths = [40u32, 7];
    for (chain, length) in first.iter_mut().zip(lengths) {
        let mut last = None;
        for _ in 0..length {
            let cluster = volume.allocate_cluster(last).unwrap();
            if last.is_none() {
                *chain = cluster;
            }
            last = Some(cluster);
        }
    }
    volume.mark_clean().unwrap();
    let image = volume.into_device();
    assert_eq!(oracle_free_clusters(&image.bytes), total - 47, "{label}");

    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    assert_eq!(volume.count_free_clusters().unwrap(), total - 47);
    let freed = volume.truncate_chain(first[0], 12).unwrap();
    assert_eq!(freed, 28);
    lengths[0] = 12;
    assert_eq!(volume.chain(first[0]).count() as u32, lengths[0]);
    assert_eq!(volume.chain(first[1]).count() as u32, lengths[1]);
    volume.mark_clean().unwrap();
    let image = volume.into_device();
    assert_eq!(oracle_free_clusters(&image.bytes), total - 19, "{label}");
    let hints = WritableVolume::open(
        Memory {
            bytes: image.bytes.clone(),
            size: image.size,
            writes: 0,
        },
        NoClock,
    )
    .unwrap()
    .file_system_information()
    .unwrap();
    // Only FAT32 has an FSInfo sector, and the count it holds is the true one.
    assert_eq!(
        hints.is_some(),
        matches!(fat_type, FatType::Fat32),
        "{label}"
    );
    if let Some(hints) = hints {
        assert_eq!(
            hints.free_cluster_count(),
            Some(total - 19),
            "{label}: FSInfo count"
        );
    }

    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    assert_eq!(volume.deallocate_chain(first[0]).unwrap(), 12);
    assert_eq!(volume.deallocate_chain(first[1]).unwrap(), 7);
    volume.mark_clean().unwrap();
    let image = volume.into_device();
    assert_eq!(oracle_free_clusters(&image.bytes), total, "{label}");
    assert_eq!(
        metadata(&image),
        original,
        "{label}: the tables are as they were"
    );
    check_with_fsck(&image.bytes, &format!("{label}-freed"));
}

#[cfg(not(miri))]
#[test]
fn chains_allocate_extend_cut_and_free_on_fat12() {
    allocate_extend_cut_and_free("alloc-fat12", 4, FatType::Fat12, 512);
}

#[cfg(not(miri))]
#[test]
fn chains_allocate_extend_cut_and_free_on_fat16() {
    allocate_extend_cut_and_free("alloc-fat16", 32, FatType::Fat16, 512);
}

#[cfg(not(miri))]
#[test]
fn chains_allocate_extend_cut_and_free_on_fat16_with_4096_byte_sectors() {
    allocate_extend_cut_and_free("alloc-fat16-4k", 64, FatType::Fat16, 4096);
}

#[cfg(not(miri))]
#[test]
fn chains_allocate_extend_cut_and_free_on_fat32() {
    allocate_extend_cut_and_free("alloc-fat32", 64, FatType::Fat32, 512);
}

#[cfg(not(miri))]
/// Many chains grown, cut and freed in a scrambled order, against a model of which clusters are
/// in use: the free count and every chain's length must agree throughout.
fn scrambled(label: &str, mebibytes: usize, fat_type: FatType) {
    let image = formatted(mebibytes, fat_type, 512);
    let original = metadata(&image);
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    let total = volume.count_free_clusters().unwrap();
    let mut chains: Vec<(u32, u32)> = Vec::new(); // (first cluster, length)
    let mut last: Vec<u32> = Vec::new();
    let mut rng = Sequence(0x5EED);
    let mut used = 0u32;
    for step in 0..400 {
        match rng.next(4) {
            0 | 1 if chains.len() < 12 => {
                let first = volume.allocate_cluster(None).unwrap();
                chains.push((first, 1));
                last.push(first);
                used += 1;
            }
            2 if !chains.is_empty() => {
                let i = rng.next(chains.len() as u64) as usize;
                let grow = 1 + rng.next(20) as u32;
                for _ in 0..grow {
                    let next = volume.allocate_cluster(Some(last[i])).unwrap();
                    last[i] = next;
                    chains[i].1 += 1;
                    used += 1;
                }
            }
            _ if !chains.is_empty() => {
                let i = rng.next(chains.len() as u64) as usize;
                let (first, length) = chains[i];
                let keep = rng.next(u64::from(length) + 1) as u32;
                let freed = volume.truncate_chain(first, keep).unwrap();
                assert_eq!(freed, length - keep.min(length), "{label} step {step}");
                used -= freed;
                if keep == 0 {
                    chains.swap_remove(i);
                    last.swap_remove(i);
                } else {
                    chains[i].1 = keep.min(length);
                    last[i] = volume.chain(first).last().unwrap().unwrap();
                }
            }
            _ => {}
        }
        if step % 50 == 0 {
            assert_eq!(
                volume.count_free_clusters().unwrap(),
                total - used,
                "{label}"
            );
        }
    }
    for (first, length) in &chains {
        assert_eq!(volume.chain(*first).count() as u32, *length, "{label}");
    }
    volume.mark_clean().unwrap();
    assert_eq!(volume.count_free_clusters().unwrap(), total - used);
    let image = volume.into_device();
    assert_eq!(oracle_free_clusters(&image.bytes), total - used, "{label}");
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    for (first, _) in chains {
        volume.deallocate_chain(first).unwrap();
    }
    volume.mark_clean().unwrap();
    let image = volume.into_device();
    assert_eq!(metadata(&image), original, "{label}");
    check_with_fsck(&image.bytes, label);
}

#[cfg(not(miri))]
#[test]
fn a_scrambled_run_keeps_the_count_and_the_chains_on_fat12() {
    scrambled("scramble-fat12", 4, FatType::Fat12);
}

#[cfg(not(miri))]
#[test]
fn a_scrambled_run_keeps_the_count_and_the_chains_on_fat16() {
    scrambled("scramble-fat16", 32, FatType::Fat16);
}

#[cfg(not(miri))]
#[test]
fn a_scrambled_run_keeps_the_count_and_the_chains_on_fat32() {
    scrambled("scramble-fat32", 64, FatType::Fat32);
}

#[cfg(not(miri))]
#[test]
fn a_full_volume_is_refused_and_every_cluster_comes_back() {
    let image = formatted(1, FatType::Fat12, 512);
    let original = metadata(&image);
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    let total = volume.count_free_clusters().unwrap();
    let first = volume.allocate_cluster(None).unwrap();
    let mut last = first;
    for _ in 1..total {
        last = volume.allocate_cluster(Some(last)).unwrap();
    }
    assert_eq!(
        volume.allocate_cluster(Some(last)),
        Err(AllocationError::VolumeFull)
    );
    assert_eq!(
        volume.allocate_cluster(None),
        Err(AllocationError::VolumeFull)
    );
    assert_eq!(volume.count_free_clusters().unwrap(), 0);
    assert_eq!(volume.table_entry(last), Ok(TableEntry::EndOfChain));
    assert_eq!(volume.deallocate_chain(first).unwrap(), total);
    volume.mark_clean().unwrap();
    assert_eq!(metadata(&volume.into_device()), original);
}

#[cfg(not(miri))]
/// A FAT12 entry can straddle two sectors for an even cluster when the sector is 1024 bytes or
/// more: its top nibble then belongs to the next cluster, which lives in the next sector. Freeing
/// such a cluster must leave that neighbor's link alone.
#[test]
fn freeing_a_cluster_whose_entry_straddles_sectors_keeps_its_neighbors_link() {
    let image = formatted(2, FatType::Fat12, 1024);
    let original = metadata(&image);
    let mut volume = WritableVolume::open(image, NoClock).unwrap();
    // Cluster 682's entry starts at byte 1023 of the first table sector.
    let mut last = None;
    let mut first = 0;
    for _ in 2..=682 {
        let cluster = volume.allocate_cluster(last).unwrap();
        first = if last.is_none() { cluster } else { first };
        last = Some(cluster);
    }
    assert_eq!(last, Some(682));
    let second = volume.allocate_cluster(None).unwrap();
    assert_eq!(second, 683);
    let mut tail = second;
    for _ in 0..4 {
        tail = volume.allocate_cluster(Some(tail)).unwrap();
    }
    assert_eq!(volume.truncate_chain(first, 400).unwrap(), 281);
    let walked: Vec<u32> = volume.chain(second).map(Result::unwrap).collect();
    assert_eq!(walked, [683, 684, 685, 686, 687]);
    assert_eq!(volume.chain(first).count(), 400);
    volume.deallocate_chain(first).unwrap();
    volume.deallocate_chain(second).unwrap();
    volume.mark_clean().unwrap();
    assert_eq!(metadata(&volume.into_device()), original);
}

#[cfg(not(miri))]
/// The raw 32-bit entry of `cluster` in table copy `copy` of `image`.
fn raw_fat32_entry(image: &Memory, copy: u8, cluster: u32) -> u32 {
    let volume = WritableVolume::open(
        Memory {
            bytes: image.bytes.clone(),
            size: image.size,
            writes: 0,
        },
        NoClock,
    )
    .unwrap();
    let boot = volume.boot_sector();
    let sector = boot.sector_size().bytes();
    let at = boot.table_first_sector(copy).unwrap() as usize * sector + cluster as usize * 4;
    let bytes = &image.bytes[at..at + 4];
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

#[cfg(not(miri))]
/// With `BPB_ExtFlags` turning mirroring off, only the table it names is live: the other copy
/// must not be written.
#[test]
fn only_the_live_table_is_written_when_mirroring_is_off() {
    for live in [0u8, 1] {
        let mut image = formatted(64, FatType::Fat32, 512);
        image.bytes[40] = 0x80 | live;
        image.bytes[41] = 0;
        let mut volume = WritableVolume::open(image, NoClock).unwrap();
        assert_eq!(volume.boot_sector().active_table(), Some(live));
        let cluster = volume.allocate_cluster(None).unwrap();
        let image = volume.into_device();
        assert_eq!(raw_fat32_entry(&image, live, cluster), 0x0FFF_FFFF);
        assert_eq!(raw_fat32_entry(&image, 1 - live, cluster), 0);
        let mut volume = WritableVolume::open(image, NoClock).unwrap();
        assert_eq!(volume.deallocate_chain(cluster).unwrap(), 1);
        let image = volume.into_device();
        assert_eq!(raw_fat32_entry(&image, live, cluster), 0);
    }
}
