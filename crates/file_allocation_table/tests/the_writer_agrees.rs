//! **rust-fatfs and `fsck` agree with what this crate allocates and frees.**
//!
//! On FAT12, FAT16 and FAT32 volumes rust-fatfs formatted, this crate allocates chains, extends
//! them, cuts them and frees them. After each step rust-fatfs's count of free clusters must equal
//! this crate's, the table must be byte for byte what it was once every chain is freed, and the
//! host's `fsck -n` must accept the volume. On FAT32, the `FSInfo` hints this crate writes must
//! be the true ones.

mod support;

use file_allocation_table::{AllocationError, TableEntry, Volume};
use lamfat::FatType;
use support::{Memory, check_with_fsck, formatted, oracle_free_clusters};

/// The bytes of `image` that hold the boot sectors and the tables, with the `FSInfo` sector
/// blanked: what freeing every chain must put back.
fn metadata(image: &Memory) -> Vec<u8> {
    let volume = Volume::open(Memory {
        bytes: image.bytes.clone(),
        size: image.size,
        writes: 0,
    })
    .unwrap();
    let boot = *volume.boot_sector();
    let sector = image.size.bytes();
    let end = boot.first_data_sector() as usize * sector;
    let mut bytes = image.bytes[..end].to_vec();
    if let Some(info) = boot.fs_info_sector() {
        let at = usize::from(info) * sector;
        bytes[at..at + sector].fill(0);
    }
    bytes
}

/// A small deterministic sequence, so a failure reproduces.
struct Sequence(u64);

impl Sequence {
    fn next(&mut self, below: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % below
    }
}

fn allocate_extend_cut_and_free(label: &str, mebibytes: usize, fat_type: FatType, sector: u16) {
    let image = formatted(mebibytes, fat_type, sector);
    let original = metadata(&image);
    check_with_fsck(&image.bytes, &format!("{label}-empty"));
    let mut volume = Volume::open(image).unwrap();
    let total = volume.count_free_clusters().unwrap();
    assert_eq!(total, oracle_free_clusters(&volume.into_device().bytes));
    let image = formatted(mebibytes, fat_type, sector);
    let mut volume = Volume::open(image).unwrap();

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
    volume.sync().unwrap();
    let image = volume.into_device();
    assert_eq!(oracle_free_clusters(&image.bytes), total - 47, "{label}");

    let mut volume = Volume::open(image).unwrap();
    assert_eq!(volume.count_free_clusters().unwrap(), total - 47);
    let freed = volume.truncate_chain(first[0], 12).unwrap();
    assert_eq!(freed, 28);
    lengths[0] = 12;
    assert_eq!(volume.chain(first[0]).count() as u32, lengths[0]);
    assert_eq!(volume.chain(first[1]).count() as u32, lengths[1]);
    volume.sync().unwrap();
    let image = volume.into_device();
    assert_eq!(oracle_free_clusters(&image.bytes), total - 19, "{label}");
    let hints = Volume::open(Memory {
        bytes: image.bytes.clone(),
        size: image.size,
        writes: 0,
    })
    .unwrap()
    .fs_info()
    .unwrap();
    // Only FAT32 has an FSInfo sector, and the count it holds is the true one.
    assert_eq!(
        hints.is_some(),
        matches!(fat_type, FatType::Fat32),
        "{label}"
    );
    if let Some(hints) = hints {
        assert_eq!(
            hints.free_clusters(),
            Some(total - 19),
            "{label}: FSInfo count"
        );
    }

    let mut volume = Volume::open(image).unwrap();
    assert_eq!(volume.free_chain(first[0]).unwrap(), 12);
    assert_eq!(volume.free_chain(first[1]).unwrap(), 7);
    volume.sync().unwrap();
    let image = volume.into_device();
    assert_eq!(oracle_free_clusters(&image.bytes), total, "{label}");
    assert_eq!(
        metadata(&image),
        original,
        "{label}: the tables are as they were"
    );
    check_with_fsck(&image.bytes, &format!("{label}-freed"));
}

#[test]
fn chains_allocate_extend_cut_and_free_on_fat12() {
    allocate_extend_cut_and_free("alloc-fat12", 4, FatType::Fat12, 512);
}

#[test]
fn chains_allocate_extend_cut_and_free_on_fat16() {
    allocate_extend_cut_and_free("alloc-fat16", 32, FatType::Fat16, 512);
}

#[test]
fn chains_allocate_extend_cut_and_free_on_fat16_with_4096_byte_sectors() {
    allocate_extend_cut_and_free("alloc-fat16-4k", 64, FatType::Fat16, 4096);
}

#[test]
fn chains_allocate_extend_cut_and_free_on_fat32() {
    allocate_extend_cut_and_free("alloc-fat32", 64, FatType::Fat32, 512);
}

/// Many chains grown, cut and freed in a scrambled order, against a model of which clusters are
/// in use: the free count and every chain's length must agree throughout.
fn scrambled(label: &str, mebibytes: usize, fat_type: FatType) {
    let image = formatted(mebibytes, fat_type, 512);
    let original = metadata(&image);
    let mut volume = Volume::open(image).unwrap();
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
    volume.sync().unwrap();
    assert_eq!(volume.count_free_clusters().unwrap(), total - used);
    let image = volume.into_device();
    assert_eq!(oracle_free_clusters(&image.bytes), total - used, "{label}");
    let mut volume = Volume::open(image).unwrap();
    for (first, _) in chains {
        volume.free_chain(first).unwrap();
    }
    volume.sync().unwrap();
    let image = volume.into_device();
    assert_eq!(metadata(&image), original, "{label}");
    check_with_fsck(&image.bytes, label);
}

#[test]
fn a_scrambled_run_keeps_the_count_and_the_chains_on_fat12() {
    scrambled("scramble-fat12", 4, FatType::Fat12);
}

#[test]
fn a_scrambled_run_keeps_the_count_and_the_chains_on_fat16() {
    scrambled("scramble-fat16", 32, FatType::Fat16);
}

#[test]
fn a_scrambled_run_keeps_the_count_and_the_chains_on_fat32() {
    scrambled("scramble-fat32", 64, FatType::Fat32);
}

#[test]
fn a_full_volume_is_refused_and_every_cluster_comes_back() {
    let image = formatted(1, FatType::Fat12, 512);
    let original = metadata(&image);
    let mut volume = Volume::open(image).unwrap();
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
    assert_eq!(volume.free_chain(first).unwrap(), total);
    volume.sync().unwrap();
    assert_eq!(metadata(&volume.into_device()), original);
}
