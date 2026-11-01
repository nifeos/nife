//! Allocation and release over the small FAT12 volume `reader::tests` builds: the first free
//! cluster from a hint, a wrap, a full volume, a chain freed and cut, every way a chain can break,
//! entries that straddle a table sector, both table copies, and a device that refuses a write.
//!
//! Name: provisional.

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
    assert_eq!(volume.free_chain(200), Ok(1));
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
    assert_eq!(volume.free_chain(5), Ok(4));
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
        volume.free_chain(5),
        Err(AllocationError::Chain(ChainError::Broken {
            cluster: 9,
            entry: TableEntry::Bad
        }))
    );
    assert_eq!(volume.table_entry(5), Ok(TableEntry::Free));
    assert_eq!(volume.table_entry(9), Ok(TableEntry::Bad));
    assert_eq!(
        volume.free_chain(1),
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
        volume.free_chain(5),
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
    volume.free_chain(2).unwrap();
    assert_eq!(volume.writer.free_clusters, Some(CLUSTERS - 1));
    // A volume with no FSInfo has nowhere to write them, and a sync is still clean.
    assert_eq!(volume.sync(), Ok(()));
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
    assert_eq!(volume.free_chain(5), Err(broken));
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
