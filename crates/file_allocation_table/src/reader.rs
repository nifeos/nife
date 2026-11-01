//! **An open volume**: a device, its checked boot sector, and the reads every later structure
//! makes through them. It reads the table entry by entry and walks cluster chains with a bound
//! no table contents can defeat.
//!
//! Name: ratified 2026-10-26 (UTC), on #2760. The module that reads through a device, beside
//! `volume`, which lays one out.

use crate::allocation::WriterState;
use crate::boot_sector::{BootSector, ReadError};
use crate::directory::Directory;
use crate::named_entry::NamedEntries;
use crate::sector::{SectorDevice, SectorSize};
use crate::table::{self, TableEntry};
use crate::time::Stamp;

/// **Why a table read or a chain walk stopped.**
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Why a chain walk stopped, beside `ParseError` and
/// `ReadError`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChainError<E> {
    /// The device refused a read; its own error, unchanged.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. The same variant `ReadError` has.
    Device(E),
    /// The cluster asked for, or a chain's first cluster, is not one of this volume's data
    /// clusters (2 to `clusters() + 1`).
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. The condition, in the words of
    /// `cluster_first_sector`'s doc.
    NotADataCluster(u32),
    /// `cluster`'s table entry cannot continue a chain: it is free, reserved or bad, or names a
    /// cluster the volume does not have.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. A chain that cannot continue.
    Broken {
        /// The cluster whose entry was read.
        cluster: u32,
        /// What the entry said.
        entry: TableEntry,
    },
    /// The chain has more clusters than the volume holds, so it revisits one: the table loops.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. What the bound detects: more clusters than the
    /// volume holds. Not `Loop`, since the walk never compares clusters.
    TooLong,
}

/// **A FAT volume open for reading**, over a device the caller supplies.
///
/// It owns the device (pass `&mut device` to keep it) and caches the last table sector it read,
/// so a chain walk through one sector of the table costs one device read.
///
/// # Examples
///
/// ```
/// use file_allocation_table::{SECTOR_BYTES, SectorDevice, SectorSize, Volume, VolumeLayout, NoClock};
///
/// // A volume this crate's writer lays out, held in memory.
/// let layout = VolumeLayout::new(70_000 * 8 + 2_000, 0, 5_000, 1, "BOOTX64.EFI", &mut NoClock).unwrap();
/// struct Image(VolumeLayout);
/// impl SectorDevice for Image {
///     type Error = ();
///     fn sector_size(&self) -> SectorSize {
///         SectorSize::Bytes512
///     }
///     fn read_sector(&mut self, index: u64, buffer: &mut [u8]) -> Result<(), ()> {
///         if !self.0.fill_sector(index, buffer) {
///             buffer.fill(0);
///         }
///         Ok(())
///     }
/// }
///
/// let mut volume = Volume::open(Image(layout)).unwrap();
/// let clusters: Vec<u32> = volume.chain(5).map(Result::unwrap).collect();
/// assert_eq!(clusters, [5, 6]);
/// ```
///
/// Name: ratified 2026-10-26 (UTC), on #2760. An open volume to read. The ruling of 2026-10-25
/// refused it for the writer as "a volume to be written, not one read", which leaves it to the
/// reader.
#[derive(Debug)]
pub struct Volume<D> {
    pub(crate) device: D,
    pub(crate) boot: BootSector,
    pub(crate) cached: Option<u32>,
    pub(crate) buffer: [u8; SectorSize::MAX],
    /// What the writer keeps between calls; a reader never touches it.
    pub(crate) writer: WriterState,
    /// The time the operation under way stamps: packed from the clock once per operation.
    pub(crate) stamp: Stamp,
}

impl<D: SectorDevice> Volume<D> {
    /// **Open the volume on `device`**: read and check its boot sector.
    ///
    /// Refuses what [`BootSector::read`] refuses, including a volume whose sector size is not the
    /// device's.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. What a caller does to a volume before reading it.
    pub fn open(mut device: D) -> Result<Volume<D>, ReadError<D::Error>> {
        let boot = BootSector::read(&mut device)?;
        Ok(Volume {
            device,
            boot,
            cached: None,
            buffer: [0; SectorSize::MAX],
            writer: WriterState::new(),
            stamp: Stamp::ZERO,
        })
    }

    /// The volume's boot sector, as [`Volume::open`] checked it.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. Returns the `BootSector` the volume holds.
    pub const fn boot_sector(&self) -> &BootSector {
        &self.boot
    }

    /// Give the device back.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. The `into_` convention for giving back what a
    /// value owns.
    pub fn into_device(self) -> D {
        self.device
    }

    /// Read volume-relative sector `sector` into `buffer`, which is one sector long. Every caller
    /// passes a sector `BootSector::parse` placed inside the volume.
    pub(crate) fn read_sector(
        &mut self,
        sector: u32,
        buffer: &mut [u8],
    ) -> Result<(), ChainError<D::Error>> {
        self.device
            .read_sector(u64::from(sector), buffer)
            .map_err(ChainError::Device)
    }

    /// Make the cache hold table sector `sector`.
    pub(crate) fn load(&mut self, sector: u32) -> Result<(), ChainError<D::Error>> {
        if self.cached != Some(sector) {
            self.cached = None;
            let size = self.boot.sector_size().bytes();
            self.device
                .read_sector(u64::from(sector), &mut self.buffer[..size])
                .map_err(ChainError::Device)?;
            self.cached = Some(sector);
        }
        Ok(())
    }

    /// **Read data cluster `cluster`'s table entry.**
    ///
    /// Refuses a cluster that is not a data cluster of this volume, and passes on the device's
    /// error. A FAT12 entry that straddles two sectors costs two reads.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. Refused `entry` (it reads a file allocation table
    /// entry, and the crate has three other kinds of entry). Reads one `TableEntry`.
    pub fn table_entry(&mut self, cluster: u32) -> Result<TableEntry, ChainError<D::Error>> {
        self.boot
            .cluster_first_sector(cluster)
            .ok_or(ChainError::NotADataCluster(cluster))?;
        let raw = self.read_raw_entry(cluster)?;
        Ok(TableEntry::decode(
            self.boot.file_allocation_table_type(),
            raw,
        ))
    }

    /// The raw value of table entry `index`, for an index below `clusters() + 2`: the two reserved
    /// entries are readable here and not through [`Volume::table_entry`].
    pub(crate) fn read_raw_entry(&mut self, index: u32) -> Result<u32, ChainError<D::Error>> {
        let (sector, within) = table::raw_position_in(
            &self.boot,
            index,
            self.boot.active_table().unwrap_or_default(),
        )
        .ok_or(ChainError::NotADataCluster(index))?;
        let file_allocation_table_type = self.boot.file_allocation_table_type();
        let size = self.boot.sector_size().bytes();
        let mut bytes = [0u8; 4];
        let wanted = table::entry_bytes(file_allocation_table_type);
        self.load(sector)?;
        // FAT16 and FAT32 entries are aligned to their width, so only FAT12's split.
        let here = wanted.min(size - within);
        bytes[..here].copy_from_slice(&self.buffer[within..within + here]);
        if here < wanted {
            self.load(sector + 1)?;
            bytes[here..wanted].copy_from_slice(&self.buffer[..wanted - here]);
        }
        Ok(table::assemble(file_allocation_table_type, index, bytes))
    }

    /// **The chain that starts at `first`**, cluster by cluster.
    ///
    /// The walk yields `first`, then each cluster the table names, and ends at an end-of-chain
    /// entry. It ends with an error at an entry that cannot continue a chain, at a cluster the
    /// volume does not have, or once it would yield more clusters than the volume holds, which
    /// only a loop can make. After an error it yields nothing more.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. The specification's "cluster chain".
    pub fn chain(&mut self, first: u32) -> Chain<'_, D> {
        Chain {
            volume: self,
            walk: Walk::new(first),
        }
    }

    /// **The root directory**: a region of FAT12 and FAT16, a chain of FAT32.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. The same word as `BootSector::root_directory`,
    /// which says where it lives.
    pub fn root_directory(&mut self) -> Directory<'_, D> {
        Directory::root(self)
    }

    /// **The root directory as named entries**: each short entry with its checked long name.
    ///
    /// Name: ratified 2026-11-01 (UTC), renamed from `root_entries`: `Volume::root_directory` with
    /// the names joined.
    pub fn root_named_entries(&mut self) -> NamedEntries<'_, D> {
        NamedEntries::new(self.root_directory())
    }

    /// **The directory whose first cluster is `first`, as named entries.** Cluster 0 is the root.
    ///
    /// Name: ratified 2026-11-01 (UTC), renamed from `entries`: `Volume::directory` with the names
    /// joined.
    pub fn named_entries(&mut self, first: u32) -> NamedEntries<'_, D> {
        NamedEntries::new(self.directory(first))
    }

    /// **The directory whose first cluster is `first`**, as a short entry's
    /// `first_cluster` names it. Cluster 0 is the root, the value a `..` entry holds in a
    /// directory whose parent is the root.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. A directory by its first cluster.
    pub fn directory(&mut self, first: u32) -> Directory<'_, D> {
        if first == 0 {
            Directory::root(self)
        } else {
            Directory::chain(self, first)
        }
    }
}

/// **What a chain walk reads**: how many data clusters the volume has, and one table entry at a
/// time. [`Volume`] is the one implementation; the proofs supply a hostile one.
pub(crate) trait Links {
    /// The device's error.
    type Error;

    /// The volume's data clusters, numbered 2 to `clusters() + 1`.
    fn clusters(&self) -> u32;

    /// Data cluster `cluster`'s entry.
    fn table_entry(&mut self, cluster: u32) -> Result<TableEntry, ChainError<Self::Error>>;
}

impl<D: SectorDevice> Links for Volume<D> {
    type Error = D::Error;

    fn clusters(&self) -> u32 {
        self.boot.clusters()
    }

    fn table_entry(&mut self, cluster: u32) -> Result<TableEntry, ChainError<D::Error>> {
        Volume::table_entry(self, cluster)
    }
}

/// Whether `cluster` is one of `clusters` data clusters, the same test
/// [`BootSector::cluster_first_sector`] makes.
pub(crate) const fn is_data(clusters: u32, cluster: u32) -> bool {
    cluster >= 2 && cluster - 2 < clusters
}

/// **Where a chain walk stands**, apart from the volume it reads, so a directory can walk a
/// chain while it reads the clusters.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Walk {
    /// The cluster to yield next, or `None` once the walk has ended.
    next: Option<u32>,
    /// The cluster yielded last, whose entry names the next one.
    last: Option<u32>,
    /// How many clusters the walk has yielded.
    yielded: u32,
}

impl Walk {
    /// A walk from `first`, which the first step checks.
    pub(crate) const fn new(first: u32) -> Walk {
        Walk {
            next: Some(first),
            last: None,
            yielded: 0,
        }
    }

    /// The next cluster, `None` at the end, or the error that ends the walk.
    pub(crate) fn step<L: Links>(
        &mut self,
        links: &mut L,
    ) -> Option<Result<u32, ChainError<L::Error>>> {
        let result = self.advance(links);
        if !matches!(result, Some(Ok(_))) {
            self.next = None;
            self.last = None;
        }
        result
    }

    fn advance<L: Links>(&mut self, links: &mut L) -> Option<Result<u32, ChainError<L::Error>>> {
        let clusters = links.clusters();
        let cluster = match (self.next.take(), self.last) {
            (Some(first), None) => {
                if !is_data(clusters, first) {
                    return Some(Err(ChainError::NotADataCluster(first)));
                }
                first
            }
            (None, Some(last)) => match links.table_entry(last) {
                Err(error) => return Some(Err(error)),
                Ok(TableEntry::EndOfChain) => return None,
                Ok(TableEntry::Next(next)) if is_data(clusters, next) => next,
                Ok(entry) => {
                    return Some(Err(ChainError::Broken {
                        cluster: last,
                        entry,
                    }));
                }
            },
            _ => return None,
        };
        if self.yielded >= clusters {
            return Some(Err(ChainError::TooLong));
        }
        self.yielded += 1;
        self.last = Some(cluster);
        Some(Ok(cluster))
    }
}

/// **A cluster chain, walked one cluster at a time**: what [`Volume::chain`] returns.
///
/// It yields at most `clusters()` clusters and then ends, whatever the table holds.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. The iterator `chain` returns.
#[derive(Debug)]
pub struct Chain<'v, D> {
    volume: &'v mut Volume<D>,
    walk: Walk,
}

impl<D: SectorDevice> Iterator for Chain<'_, D> {
    type Item = Result<u32, ChainError<D::Error>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.walk.step(self.volume)
    }
}

#[cfg(kani)]
pub(crate) mod verification;

#[cfg(test)]
pub(crate) mod tests;
