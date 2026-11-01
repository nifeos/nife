//! **Allocation**: finding a free cluster, linking it into a chain, and giving a chain back, over
//! a table the crate writes through every copy.
//!
//! The chain operations are written against [`LinksMut`], the table's two verbs, so the proofs in
//! `verification.rs` run them over a model table whose size Kani can unroll; [`Volume`] is the
//! one implementation that touches a device. Every operation moves a chain between two states the
//! specification allows, in an order a crash leaves at worst a leaked cluster and never a cluster
//! two chains share:
//!
//! - A cluster is marked end-of-chain before anything links to it.
//! - A chain is cut by marking its new last cluster end-of-chain first, then freeing the tail.
//! - A chain is freed from its first cluster on, one entry at a time.
//!
//! The FAT32 `FSInfo` hints are kept in memory as the operations change them and written by
//! [`crate::WritableVolume::sync`]. A volume that is dropped without a sync leaves hints the specification
//! already calls untrustworthy.
//!
//! Name: provisional. The module that finds free clusters and owns the table's writes.

use crate::boot_sector::FileAllocationTableType;
use crate::fs_info::FsInfo;
use crate::reader::{ChainError, Links, Volume, Walk, is_data};
use crate::sector::WritableSectorDevice;
use crate::table::{self, TableEntry};

/// **Why an allocation or a release stopped.**
///
/// Name: provisional. Beside `ChainError`, which it wraps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AllocationError<E> {
    /// The table or a sector could not be read or written; the chain error, unchanged.
    ///
    /// Name: provisional. The same variant `FileReadError` has.
    Chain(ChainError<E>),
    /// Every data cluster is in use.
    ///
    /// Name: provisional. The condition.
    VolumeFull,
    /// The cluster to extend is not the last of its chain: its entry is not end-of-chain.
    ///
    /// Name: provisional. The condition, in the table's terms.
    NotTheEndOfAChain(u32),
}

impl<E> From<ChainError<E>> for AllocationError<E> {
    fn from(error: ChainError<E>) -> Self {
        AllocationError::Chain(error)
    }
}

/// **A table the chain operations can read and change**: [`Links`] plus the one write.
pub(crate) trait LinksMut: Links {
    /// Store `entry` as data cluster `cluster`'s entry, in every copy of the table the volume
    /// keeps live. Refuses a cluster that is not a data cluster.
    fn set_table_entry(
        &mut self,
        cluster: u32,
        entry: TableEntry,
    ) -> Result<(), ChainError<Self::Error>>;

    /// Take a free cluster for the end of a chain: linked after `last` when given, else the first
    /// of a new chain. `Ok(None)` means the volume has no free cluster.
    fn append(&mut self, last: Option<u32>) -> Result<Option<u32>, ChainError<Self::Error>>;
}

/// **Find a free cluster, mark it end-of-chain and return it.** The search starts at `hint`
/// (cluster 2 when `hint` is not a data cluster), wraps once and reads each data cluster's entry
/// at most once. `Ok(None)` means no cluster is free.
pub(crate) fn allocate<L: LinksMut>(
    links: &mut L,
    hint: u32,
) -> Result<Option<u32>, ChainError<L::Error>> {
    let clusters = links.clusters();
    let start = if is_data(clusters, hint) { hint - 2 } else { 0 };
    for step in 0..clusters {
        // `start < clusters` and `step < clusters`, so the sum cannot reach 2 * clusters + 2.
        let cluster = 2 + (start + step) % clusters;
        if links.table_entry(cluster)? == TableEntry::Free {
            links.set_table_entry(cluster, TableEntry::EndOfChain)?;
            return Ok(Some(cluster));
        }
    }
    Ok(None)
}

/// **Free every cluster of the chain that starts at `first`**, in chain order, and return how many.
///
/// Ends at the end-of-chain entry. A broken entry, or a chain with more clusters than the volume
/// holds, ends it with the error after the clusters before it are free. A chain that loops reaches
/// a cluster it has already freed, whose entry reads free, so it ends there.
pub(crate) fn free_chain<L: LinksMut>(
    links: &mut L,
    first: u32,
) -> Result<u32, ChainError<L::Error>> {
    let clusters = links.clusters();
    if !is_data(clusters, first) {
        return Err(ChainError::NotADataCluster(first));
    }
    let mut cluster = first;
    let mut freed = 0;
    loop {
        let entry = links.table_entry(cluster)?;
        let next = match entry {
            TableEntry::EndOfChain => None,
            TableEntry::Next(next) if is_data(clusters, next) => Some(next),
            entry => return Err(ChainError::Broken { cluster, entry }),
        };
        links.set_table_entry(cluster, TableEntry::Free)?;
        freed += 1;
        let Some(next) = next else {
            return Ok(freed);
        };
        if freed >= clusters {
            return Err(ChainError::TooLong);
        }
        cluster = next;
    }
}

/// **Cut the chain that starts at `first` to its first `keep` clusters**, and return how many
/// clusters it freed. A chain of `keep` clusters or fewer is left alone. `keep` of 0 frees the
/// whole chain, and the caller must forget `first`.
pub(crate) fn truncate_chain<L: LinksMut>(
    links: &mut L,
    first: u32,
    keep: u32,
) -> Result<u32, ChainError<L::Error>> {
    if keep == 0 {
        return free_chain(links, first);
    }
    let mut walk = Walk::new(first);
    let mut last = first;
    for _ in 0..keep {
        match walk.step(links) {
            Some(cluster) => last = cluster?,
            None => return Ok(0),
        }
    }
    match links.table_entry(last)? {
        TableEntry::EndOfChain => Ok(0),
        TableEntry::Next(tail) if is_data(links.clusters(), tail) => {
            // The cut goes in before the tail is freed: a crash between the two leaks the tail.
            links.set_table_entry(last, TableEntry::EndOfChain)?;
            free_chain(links, tail)
        }
        entry => Err(ChainError::Broken {
            cluster: last,
            entry,
        }),
    }
}

/// **What the writer remembers between calls**: where to start looking for a free cluster, the
/// free count if it is known, and whether the `FSInfo` sector is out of date.
#[derive(Debug, Clone, Copy)]
pub(crate) struct WriterState {
    /// Where the next search for a free cluster starts.
    pub(crate) next_free: u32,
    /// The free-cluster count, when `FSInfo` or an earlier scan gave one.
    pub(crate) free_clusters: Option<u32>,
    /// Whether the hints were read from the device yet.
    loaded: bool,
    /// Whether the hints differ from what the device holds.
    pub(crate) hints_changed: bool,
    /// Whether this volume cleared the clean-shutdown bit and has not set it back.
    marked_dirty: bool,
    /// Whether the clean-shutdown bit was set before this volume cleared it.
    was_clean: bool,
}

impl WriterState {
    pub(crate) const fn new() -> WriterState {
        WriterState {
            next_free: 2,
            free_clusters: None,
            loaded: false,
            hints_changed: false,
            marked_dirty: false,
            was_clean: false,
        }
    }
}

impl<D: WritableSectorDevice> LinksMut for Volume<D> {
    fn set_table_entry(
        &mut self,
        cluster: u32,
        entry: TableEntry,
    ) -> Result<(), ChainError<D::Error>> {
        self.write_table_entry(cluster, entry)
    }

    fn append(&mut self, last: Option<u32>) -> Result<Option<u32>, ChainError<D::Error>> {
        match self.allocate_cluster(last) {
            Ok(cluster) => Ok(Some(cluster)),
            Err(AllocationError::VolumeFull) => Ok(None),
            Err(AllocationError::Chain(error)) => Err(error),
            Err(AllocationError::NotTheEndOfAChain(cluster)) => Err(ChainError::Broken {
                cluster,
                entry: TableEntry::Reserved,
            }),
        }
    }
}

impl<D: WritableSectorDevice> Volume<D> {
    /// Read volume-relative `sector` into the volume's own buffer, which the cache no longer
    /// describes.
    fn read_into_buffer(&mut self, sector: u32) -> Result<(), ChainError<D::Error>> {
        let size = self.boot.sector_size().bytes();
        self.cached = None;
        self.device
            .read_sector(u64::from(sector), &mut self.buffer[..size])
            .map_err(ChainError::Device)
    }

    /// Write the volume's own buffer to volume-relative `sector`.
    fn write_buffer(&mut self, sector: u32) -> Result<(), ChainError<D::Error>> {
        let size = self.boot.sector_size().bytes();
        self.cached = None;
        self.device
            .write_sector(u64::from(sector), &self.buffer[..size])
            .map_err(ChainError::Device)
    }

    /// Store `entry` for `cluster` in every live copy of the table, read-modify-write one sector
    /// at a time. A FAT12 entry that straddles two sectors rewrites both, the later one first.
    pub(crate) fn write_table_entry(
        &mut self,
        cluster: u32,
        entry: TableEntry,
    ) -> Result<(), ChainError<D::Error>> {
        self.boot
            .cluster_first_sector(cluster)
            .ok_or(ChainError::NotADataCluster(cluster))?;
        let raw = entry.encode(self.boot.file_allocation_table_type());
        self.write_raw_entry(cluster, raw)
    }

    /// Store `raw` as table entry `cluster` (any index below `clusters() + 2`) in every live copy.
    fn write_raw_entry(&mut self, cluster: u32, raw: u32) -> Result<(), ChainError<D::Error>> {
        let boot = self.boot;
        let file_allocation_table_type = boot.file_allocation_table_type();
        let wanted = table::entry_bytes(file_allocation_table_type);
        let size = boot.sector_size().bytes();
        let live = match boot.active_table() {
            Some(only) => only..only + 1,
            None => 0..boot.table_count(),
        };
        for copy in live {
            let (sector, within) = table::raw_position_in(&boot, cluster, copy)
                .ok_or(ChainError::NotADataCluster(cluster))?;
            let here = wanted.min(size - within);
            let mut bytes = [0u8; 4];
            self.read_into_buffer(sector)?;
            bytes[..here].copy_from_slice(&self.buffer[within..within + here]);
            if here < wanted {
                self.read_into_buffer(sector + 1)?;
                bytes[here..wanted].copy_from_slice(&self.buffer[..wanted - here]);
            }
            table::store(file_allocation_table_type, cluster, &mut bytes, raw);
            if here < wanted {
                self.buffer[..wanted - here].copy_from_slice(&bytes[here..wanted]);
                self.write_buffer(sector + 1)?;
                self.read_into_buffer(sector)?;
            }
            self.buffer[within..within + here].copy_from_slice(&bytes[..here]);
            self.write_buffer(sector)?;
        }
        Ok(())
    }

    /// Clear the clean-shutdown bit before the first change after open or after a sync, and
    /// remember whether it was set. A volume found dirty stays dirty at the next sync, since this
    /// crate cannot know the earlier writer left it consistent.
    pub(crate) fn begin_change(&mut self) -> Result<(), ChainError<D::Error>> {
        if self.writer.marked_dirty {
            return Ok(());
        }
        if let Some(bit) = self.clean_bit() {
            let entry = self.read_raw_entry(1)?;
            self.writer.was_clean = entry & bit != 0;
            if self.writer.was_clean {
                self.write_raw_entry(1, entry & !bit)?;
            }
        }
        self.writer.marked_dirty = true;
        Ok(())
    }

    /// Read the `FSInfo` hints from the device the first time a change needs them.
    fn load_hints(&mut self) -> Result<(), ChainError<D::Error>> {
        if self.writer.loaded {
            return Ok(());
        }
        self.writer.loaded = true;
        if let Some(sector) = self.boot.fs_info_sector() {
            self.read_into_buffer(u32::from(sector))?;
            let size = self.boot.sector_size().bytes();
            if let Ok(info) = FsInfo::parse(&self.buffer[..size], self.boot.clusters()) {
                self.writer.free_clusters = info.free_clusters();
                self.writer.next_free = info.next_free().unwrap_or(2);
            }
        }
        Ok(())
    }

    /// **Take one free cluster, mark it the end of a chain, and return it.** With `extending` set
    /// to the last cluster of a chain, the new cluster is linked after it, so the chain grows by
    /// one. The new cluster's bytes are whatever the device held; a caller that needs zeros writes
    /// them.
    ///
    /// The search starts at the cluster after the last one this volume handed out, or at
    /// `FSInfo`'s hint when the volume has not allocated yet. Refuses an `extending` cluster whose
    /// entry is not end-of-chain, and a full volume.
    ///
    /// Name: provisional. What a caller asks of a volume that has to grow a file or a directory.
    pub(crate) fn allocate_cluster(
        &mut self,
        extending: Option<u32>,
    ) -> Result<u32, AllocationError<D::Error>> {
        self.begin_change()?;
        self.load_hints()?;
        if let Some(last) = extending {
            let entry = self.table_entry(last)?;
            if entry != TableEntry::EndOfChain {
                return Err(AllocationError::NotTheEndOfAChain(last));
            }
        }
        let clusters = self.boot.clusters();
        let hint = self.writer.next_free;
        let cluster = allocate(self, hint)?.ok_or(AllocationError::VolumeFull)?;
        if let Some(last) = extending
            && let Err(error) = self.write_table_entry(last, TableEntry::Next(cluster))
        {
            // Give the cluster back, so a failed link does not leak it. If this write fails too,
            // the cluster stays marked and the error that matters is the first.
            let _ = self.write_table_entry(cluster, TableEntry::Free);
            return Err(error.into());
        }
        self.writer.next_free = if cluster - 2 + 1 < clusters {
            cluster + 1
        } else {
            2
        };
        self.writer.free_clusters = self.writer.free_clusters.map(|n| n.saturating_sub(1));
        self.writer.hints_changed = true;
        Ok(cluster)
    }

    /// **Free every cluster of the chain that starts at `first`**, and return how many.
    ///
    /// Name: provisional. Beside `allocate_cluster`.
    pub(crate) fn free_chain(&mut self, first: u32) -> Result<u32, AllocationError<D::Error>> {
        self.begin_change()?;
        self.load_hints()?;
        let result = free_chain(self, first);
        self.note_released(&result);
        Ok(result?)
    }

    /// **Cut the chain that starts at `first` to its first `keep` clusters**, and return how many
    /// clusters it freed. A `keep` of 0 frees the whole chain.
    ///
    /// Name: provisional. Beside `free_chain`.
    pub(crate) fn truncate_chain(
        &mut self,
        first: u32,
        keep: u32,
    ) -> Result<u32, AllocationError<D::Error>> {
        self.begin_change()?;
        self.load_hints()?;
        let result = truncate_chain(self, first, keep);
        self.note_released(&result);
        Ok(result?)
    }

    /// Account for clusters a release freed, including the ones freed before it failed.
    fn note_released(&mut self, result: &Result<u32, ChainError<D::Error>>) {
        if let Ok(freed) = result {
            let clusters = self.boot.clusters();
            self.writer.free_clusters = self
                .writer
                .free_clusters
                .map(|n| n.saturating_add(*freed).min(clusters));
            self.writer.hints_changed |= *freed > 0;
        } else {
            // A failed release freed an unknown number: the count is no longer known.
            self.writer.free_clusters = None;
            self.writer.hints_changed = true;
        }
    }

    /// **Write the `FSInfo` hints to the device**, when an operation changed them. FAT12 and FAT16
    /// volumes have no `FSInfo`, and a FAT32 volume whose `FSInfo` sector is missing or has a bad
    /// signature is left alone: the crate never makes a sector into `FSInfo`.
    ///
    /// Name: provisional. The verb of the standard library's `Write::flush`, for the volume's own
    /// state.
    pub(crate) fn sync(&mut self) -> Result<(), ChainError<D::Error>> {
        if self.writer.hints_changed {
            self.write_hints()?;
        }
        if self.writer.marked_dirty {
            if let (Some(bit), true) = (self.clean_bit(), self.writer.was_clean) {
                let entry = self.read_raw_entry(1)?;
                self.write_raw_entry(1, entry + bit)?;
            }
            self.writer.marked_dirty = false;
        }
        Ok(())
    }

    /// Write the `FSInfo` hints.
    fn write_hints(&mut self) -> Result<(), ChainError<D::Error>> {
        if let Some(sector) = self.boot.fs_info_sector() {
            self.read_into_buffer(u32::from(sector))?;
            let size = self.boot.sector_size().bytes();
            let free = self.writer.free_clusters;
            let next = self.writer.next_free;
            if FsInfo::store(&mut self.buffer[..size], free, Some(next)) {
                self.write_buffer(u32::from(sector))?;
            }
        }
        self.writer.hints_changed = false;
        Ok(())
    }
}

impl<D: crate::sector::SectorDevice> Volume<D> {
    /// The clean-shutdown bit of table entry 1, or `None` for FAT12, which has none.
    fn clean_bit(&self) -> Option<u32> {
        match self.boot.file_allocation_table_type() {
            FileAllocationTableType::Fat12 => None,
            FileAllocationTableType::Fat16 => Some(0x8000),
            FileAllocationTableType::Fat32 => Some(0x0800_0000),
        }
    }

    /// **Whether the volume was shut down cleanly**, from the bit in table entry 1: `Ok(None)` on
    /// FAT12, which keeps no such bit. A volume this crate is changing reads as not clean until
    /// [`crate::WritableVolume::sync`].
    ///
    /// Name: provisional. The specification's `ClnShutBitMask`, asked as a question.
    pub fn is_clean(&mut self) -> Result<Option<bool>, ChainError<D::Error>> {
        let Some(bit) = self.clean_bit() else {
            return Ok(None);
        };
        Ok(Some(self.read_raw_entry(1)? & bit != 0))
    }

    /// **Count the free clusters** by reading the whole table: exact where `FSInfo` is a hint.
    ///
    /// Name: provisional. What `FSI_Free_Count` claims, counted.
    pub fn count_free_clusters(&mut self) -> Result<u32, ChainError<D::Error>> {
        let clusters = self.boot.clusters();
        let mut free = 0;
        for cluster in 2..clusters + 2 {
            if self.table_entry(cluster)? == TableEntry::Free {
                free += 1;
            }
        }
        Ok(free)
    }

    /// **The `FSInfo` hints the device holds**, or `Ok(None)` for a volume with no `FSInfo` sector
    /// or one whose signatures are wrong.
    ///
    /// Name: provisional. Reads what `Volume::sync` writes.
    pub fn fs_info(&mut self) -> Result<Option<FsInfo>, ChainError<D::Error>> {
        let Some(sector) = self.boot.fs_info_sector() else {
            return Ok(None);
        };
        let size = self.boot.sector_size().bytes();
        let mut buffer = [0u8; crate::sector::SectorSize::MAX];
        self.device
            .read_sector(u64::from(sector), &mut buffer[..size])
            .map_err(ChainError::Device)?;
        self.cached = None;
        Ok(FsInfo::parse(&buffer[..size], self.boot.clusters()).ok())
    }
}

#[cfg(kani)]
pub(crate) mod verification;

#[cfg(test)]
mod tests;
