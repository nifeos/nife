//! **An open volume**: a device, its checked boot sector, and the reads every later structure
//! makes through them. It reads the table entry by entry and walks cluster chains with a bound
//! no table contents can defeat.

use crate::boot_sector::{BootSector, ReadError};
use crate::directory::Directory;
use crate::sector::{SectorDevice, SectorSize};
use crate::table::{self, TableEntry};

/// **Why a table read or a chain walk stopped.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChainError<E> {
    /// The device refused a read; its own error, unchanged.
    Device(E),
    /// The cluster asked for, or a chain's first cluster, is not one of this volume's data
    /// clusters (2 to `clusters() + 1`).
    NotADataCluster(u32),
    /// `cluster`'s table entry cannot continue a chain: it is free, reserved or bad, or names a
    /// cluster the volume does not have.
    Broken {
        /// The cluster whose entry was read.
        cluster: u32,
        /// What the entry said.
        entry: TableEntry,
    },
    /// The chain has more clusters than the volume holds, so it revisits one: the table loops.
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
/// use file_allocation_table::{SECTOR_BYTES, SectorDevice, SectorSize, Volume, VolumeLayout};
///
/// // A volume this crate's writer lays out, held in memory.
/// let layout = VolumeLayout::new(70_000 * 8 + 2_000, 0, 5_000, 1, "BOOTX64.EFI").unwrap();
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
#[derive(Debug)]
pub struct Volume<D> {
    device: D,
    boot: BootSector,
    cached: Option<u32>,
    buffer: [u8; SectorSize::MAX],
}

impl<D: SectorDevice> Volume<D> {
    /// **Open the volume on `device`**: read and check its boot sector.
    ///
    /// Refuses what [`BootSector::read`] refuses, including a volume whose sector size is not the
    /// device's.
    pub fn open(mut device: D) -> Result<Volume<D>, ReadError<D::Error>> {
        let boot = BootSector::read(&mut device)?;
        Ok(Volume {
            device,
            boot,
            cached: None,
            buffer: [0; SectorSize::MAX],
        })
    }

    /// The volume's boot sector, as [`Volume::open`] checked it.
    pub const fn boot_sector(&self) -> &BootSector {
        &self.boot
    }

    /// Give the device back.
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
    fn load(&mut self, sector: u32) -> Result<(), ChainError<D::Error>> {
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
    pub fn entry(&mut self, cluster: u32) -> Result<TableEntry, ChainError<D::Error>> {
        let (sector, within) = table::entry_position(&self.boot, cluster)
            .ok_or(ChainError::NotADataCluster(cluster))?;
        let fat_type = self.boot.fat_type();
        let size = self.boot.sector_size().bytes();
        let mut bytes = [0u8; 4];
        let wanted = table::entry_bytes(fat_type);
        self.load(sector)?;
        // FAT16 and FAT32 entries are aligned to their width, so only FAT12's split.
        let here = wanted.min(size - within);
        bytes[..here].copy_from_slice(&self.buffer[within..within + here]);
        if here < wanted {
            self.load(sector + 1)?;
            bytes[here..wanted].copy_from_slice(&self.buffer[..wanted - here]);
        }
        let raw = table::assemble(fat_type, cluster, bytes);
        Ok(TableEntry::decode(fat_type, raw))
    }

    /// **The chain that starts at `first`**, cluster by cluster.
    ///
    /// The walk yields `first`, then each cluster the table names, and ends at an end-of-chain
    /// entry. It ends with an error at an entry that cannot continue a chain, at a cluster the
    /// volume does not have, or once it would yield more clusters than the volume holds, which
    /// only a loop can make. After an error it yields nothing more.
    pub fn chain(&mut self, first: u32) -> Chain<'_, D> {
        Chain {
            volume: self,
            walk: Walk::new(first),
        }
    }

    /// **The root directory**: a region of FAT12 and FAT16, a chain of FAT32.
    pub fn root_directory(&mut self) -> Directory<'_, D> {
        Directory::root(self)
    }

    /// **The directory whose first cluster is `first`**, as a short entry's
    /// `first_cluster` names it. Cluster 0 is the root, the value a `..` entry holds in a
    /// directory whose parent is the root.
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
    fn entry(&mut self, cluster: u32) -> Result<TableEntry, ChainError<Self::Error>>;
}

impl<D: SectorDevice> Links for Volume<D> {
    type Error = D::Error;

    fn clusters(&self) -> u32 {
        self.boot.clusters()
    }

    fn entry(&mut self, cluster: u32) -> Result<TableEntry, ChainError<D::Error>> {
        Volume::entry(self, cluster)
    }
}

/// Whether `cluster` is one of `clusters` data clusters, the same test
/// [`BootSector::cluster_first_sector`] makes.
const fn is_data(clusters: u32, cluster: u32) -> bool {
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
            (None, Some(last)) => match links.entry(last) {
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
