//! **Directory entries**: each 32-byte slot of a directory parsed from hostile bytes, and a
//! directory read slot by slot through its region or its cluster chain.
//!
//! A slot is the end of the directory, a free slot, a short (8.3) entry, or one piece of a long
//! name. This module reports each long-name piece as it stands; `named_entry.rs` assembles the
//! pieces and checks their checksum against the short entry that follows. The layouts are those of
//! Microsoft's "FAT: General Overview of On-Disk Format", version 1.03, sections 6 and 7.
//!
//! Name: ratified 2026-10-26 (UTC), on #2760. The specification's word for the structure the module
//! parses.

use crate::boot_sector::{BootSector, FileAllocationTableType, RootDirectory};
use crate::reader::{ChainError, Links, Volume, Walk};
use crate::sector::{SectorDevice, SectorSize};

/// The bytes of one directory slot.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. The size of one slot, spelled like `SECTOR_BYTES`.
pub const SLOT_BYTES: usize = 32;

/// `DIR_Attr`'s read-only bit.
const READ_ONLY: u8 = 0x01;
/// `DIR_Attr`'s hidden bit.
const HIDDEN: u8 = 0x02;
/// `DIR_Attr`'s system bit.
const SYSTEM: u8 = 0x04;
/// `DIR_Attr`'s volume-label bit.
const VOLUME_ID: u8 = 0x08;
/// `DIR_Attr`'s directory bit.
const DIRECTORY: u8 = 0x10;
/// The low four attribute bits together, which mark a long-name slot.
const LONG_NAME: u8 = READ_ONLY | HIDDEN | SYSTEM | VOLUME_ID;
/// The attribute bits a long-name test looks at: the six the specification defines.
const LONG_NAME_MASK: u8 = 0x3F;
/// The first name byte of a free slot.
const FREE: u8 = 0xE5;
/// The ordinal bit that marks a long name's last piece, which comes first on disk.
const LAST_LONG_ENTRY: u8 = 0x40;

/// Where a long-name piece keeps its thirteen units: five in `LDIR_Name1`, six in `LDIR_Name2`
/// and two in `LDIR_Name3`.
const UNIT_OFFSETS: [usize; 13] = [1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30];

/// **One directory slot, parsed.**
///
/// Name: ratified 2026-10-26 (UTC), on #2760. One 32-byte position in a directory, used or not; the
/// specification calls only a used one an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slot {
    /// The first name byte is 0: this slot and every slot after it are free, and a reader stops.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. The first-byte-zero marker the specification
    /// describes as the end of the directory.
    End,
    /// The first name byte is `0xE5`: a free slot, deleted or never used, between used ones.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. The specification's term for a slot whose first
    /// byte is `0xE5`.
    Free,
    /// A short entry or a long-name piece.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. The opposite of `Free`.
    Used(DirectoryEntry),
}

/// **A used directory slot**: a short entry, or one piece of a long name.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. The specification's name for a used 32-byte slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DirectoryEntry {
    /// An 8.3 entry: a file, a directory, `.` or `..`, or the volume label.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. The specification's "short directory entry".
    Short(ShortEntry),
    /// Thirteen UTF-16 units of a long name, which sits in the slots ahead of its short entry.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. The specification's "long name directory entry".
    LongName(LongNameEntry),
}

/// **A short (8.3) directory entry.**
///
/// Name: ratified 2026-10-26 (UTC), on #2760. The same, as a type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShortEntry {
    name: [u8; 11],
    attributes: u8,
    first_cluster: u32,
    size: u32,
}

/// **One piece of a long file name.**
///
/// Name: ratified 2026-10-26 (UTC), on #2760. One piece of a long name, as the specification's
/// section 7 names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LongNameEntry {
    order: u8,
    checksum: u8,
    units: [u16; 13],
}

impl Slot {
    /// **Parse one slot of a directory on a volume of `file_allocation_table_type`.**
    ///
    /// Never panics: every 32 bytes are one of the variants. A short entry's first cluster takes
    /// its high half from `DIR_FstClusHI` only on FAT32; FAT12 and FAT16 leave that word to other
    /// uses, so it is ignored there. The cluster is not checked against any volume here; a reader
    /// that follows it does that.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. The verb `BootSector::parse` already uses for
    /// hostile bytes.
    pub fn parse(
        slot: &[u8; SLOT_BYTES],
        file_allocation_table_type: FileAllocationTableType,
    ) -> Slot {
        match slot[0] {
            0 => return Slot::End,
            FREE => return Slot::Free,
            _ => {}
        }
        let attributes = slot[11];
        if attributes & LONG_NAME_MASK == LONG_NAME {
            let mut units = [0u16; 13];
            for (unit, at) in units.iter_mut().zip(UNIT_OFFSETS) {
                *unit = u16::from_le_bytes([slot[at], slot[at + 1]]);
            }
            return Slot::Used(DirectoryEntry::LongName(LongNameEntry {
                order: slot[0],
                checksum: slot[13],
                units,
            }));
        }
        let mut name = [0u8; 11];
        name.copy_from_slice(&slot[..11]);
        // `0x05` stands in for a first byte of `0xE5`, which would read as a free slot.
        if name[0] == 0x05 {
            name[0] = FREE;
        }
        let high = match file_allocation_table_type {
            FileAllocationTableType::Fat32 => u32::from(u16::from_le_bytes([slot[20], slot[21]])),
            FileAllocationTableType::Fat12 | FileAllocationTableType::Fat16 => 0,
        };
        let low = u32::from(u16::from_le_bytes([slot[26], slot[27]]));
        Slot::Used(DirectoryEntry::Short(ShortEntry {
            name,
            attributes,
            first_cluster: (high << 16) | low,
            size: u32::from_le_bytes([slot[28], slot[29], slot[30], slot[31]]),
        }))
    }
}

impl ShortEntry {
    /// A short entry from its parts, as a writer lays one down.
    pub(crate) const fn new(
        name: [u8; 11],
        attributes: u8,
        first_cluster: u32,
        size: u32,
    ) -> ShortEntry {
        ShortEntry {
            name,
            attributes,
            first_cluster,
            size,
        }
    }

    /// The name as stored: eight bytes of base and three of extension, each padded with spaces,
    /// with a stored `0x05` read back as `0xE5`.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. `DIR_Name`.
    pub const fn name(&self) -> [u8; 11] {
        self.name
    }

    /// `DIR_Attr`, every bit as stored.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. `DIR_Attr`.
    pub const fn attributes(&self) -> u8 {
        self.attributes
    }

    /// Whether the entry is a directory.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. `ATTR_DIRECTORY`.
    pub const fn is_directory(&self) -> bool {
        self.attributes & DIRECTORY != 0
    }

    /// Whether the entry is the volume label, which names no file and owns no cluster.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. `ATTR_VOLUME_ID`, named for what it marks rather
    /// than for the bit.
    pub const fn is_volume_label(&self) -> bool {
        self.attributes & VOLUME_ID != 0
    }

    /// The first cluster of the entry's chain, or 0 for an empty file and for a `..` whose parent
    /// is the root. Not checked against the volume.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. `DIR_FstClusHI` and `DIR_FstClusLO` joined.
    pub const fn first_cluster(&self) -> u32 {
        self.first_cluster
    }

    /// The file's length in bytes; 0 for a directory.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. Refused `size` (the unit goes in the name, as in
    /// `SECTOR_BYTES` and `SLOT_BYTES`). `DIR_FileSize`.
    pub const fn size_bytes(&self) -> u32 {
        self.size
    }
}

impl LongNameEntry {
    /// A piece from its parts, for the long-name assembly proofs and tests.
    #[cfg(any(kani, test))]
    pub(crate) const fn from_parts(order: u8, checksum: u8, units: [u16; 13]) -> LongNameEntry {
        LongNameEntry {
            order,
            checksum,
            units,
        }
    }

    /// The piece's place in its name, counting from 1 at the piece nearest the short entry. The
    /// last-piece bit is cleared; the specification allows 1 to 20, and this value is not checked.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. `LDIR_Ord` with the last-piece bit cleared.
    pub const fn ordinal(&self) -> u8 {
        self.order & !LAST_LONG_ENTRY
    }

    /// Whether this is the name's last piece, which sits first on disk.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. `LAST_LONG_ENTRY`, the bit it reads.
    pub const fn is_last(&self) -> bool {
        self.order & LAST_LONG_ENTRY != 0
    }

    /// The checksum of the short name this piece belongs to, as stored.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. `LDIR_Chksum`.
    pub const fn checksum(&self) -> u8 {
        self.checksum
    }

    /// The piece's thirteen UTF-16 code units, as stored: the name ends at a `0x0000` unit, and
    /// `0xFFFF` pads the rest.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. Refused `units` (UTF-16 code units, Unicode's
    /// term). The UTF-16 code units it holds, which are not characters.
    pub const fn code_units(&self) -> [u16; 13] {
        self.units
    }
}

/// A used slot's index and entry, or the error that ended the read.
pub(crate) type Indexed<E> = Result<(u32, DirectoryEntry), ChainError<E>>;

/// What a directory read walks: the fixed root region, or a chain.
#[derive(Debug, Clone, Copy)]
enum Source {
    /// The next sector to read and the sector the region ends before.
    Region { next: u32, end: u32 },
    /// The chain, the next sector of the cluster at hand, and how many of its sectors are left.
    Chain {
        walk: Walk,
        next: u32,
        remaining: u32,
    },
}

impl Source {
    /// The root directory's sectors: its region on FAT12 and FAT16, its chain on FAT32.
    fn root(boot: &BootSector) -> Source {
        match boot.root_directory() {
            RootDirectory::Region {
                first_sector,
                sectors,
                ..
            } => Source::Region {
                next: first_sector,
                end: first_sector + sectors,
            },
            RootDirectory::Cluster(first) => Source::chain(first),
        }
    }

    /// The sectors of the chain that starts at `first`.
    const fn chain(first: u32) -> Source {
        Source::Chain {
            walk: Walk::new(first),
            next: 0,
            remaining: 0,
        }
    }

    /// **The volume-relative sector to read next**, `None` at the end of the directory, or the
    /// error that ends a chain. A region yields its own sectors; a chain yields each cluster's
    /// sectors in turn, for the clusters the bounded walk yields.
    fn next_sector<L: Links>(
        &mut self,
        links: &mut L,
        boot: &BootSector,
    ) -> Option<Result<u32, ChainError<L::Error>>> {
        match self {
            Source::Region { next, end } => {
                if *next >= *end {
                    return None;
                }
                *next += 1;
                Some(Ok(*next - 1))
            }
            Source::Chain {
                walk,
                next,
                remaining,
            } => {
                if *remaining == 0 {
                    let cluster = match walk.step(links)? {
                        Ok(cluster) => cluster,
                        Err(error) => return Some(Err(error)),
                    };
                    // The walk yields only data clusters, which always have sectors.
                    *next = boot.cluster_first_sector(cluster)?;
                    *remaining = boot.sectors_per_cluster();
                }
                *remaining -= 1;
                *next += 1;
                Some(Ok(*next - 1))
            }
        }
    }
}

/// **A directory, read slot by slot**: what [`Volume::root_directory`] and [`Volume::directory`]
/// return.
///
/// It yields every used slot in order, skips free ones, and stops at an end slot or at the end
/// of the region or chain. A chain is walked with [`Volume::chain`]'s bound, so the read ends
/// whatever the table holds. After an error it yields nothing more.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. A directory being read; it reads, so it is named for
/// what it reads.
#[derive(Debug)]
pub struct Directory<'v, D> {
    volume: &'v mut Volume<D>,
    source: Source,
    buffer: [u8; SectorSize::MAX],
    /// The byte offset of the next slot in `buffer`; one sector or more means a sector is due.
    at: usize,
    done: bool,
    /// The directory's first cluster, 0 for the root.
    first: u32,
    /// The index, from the directory's start, of the slot at `at`.
    index: u32,
}

impl<'v, D: SectorDevice> Directory<'v, D> {
    /// The root directory of `volume`.
    pub(crate) fn root(volume: &'v mut Volume<D>) -> Directory<'v, D> {
        let source = Source::root(volume.boot_sector());
        Directory::new(volume, source, 0)
    }

    /// The directory whose chain starts at `first`.
    pub(crate) fn chain(volume: &'v mut Volume<D>, first: u32) -> Directory<'v, D> {
        Directory::new(volume, Source::chain(first), first)
    }

    fn new(volume: &'v mut Volume<D>, source: Source, first: u32) -> Directory<'v, D> {
        Directory {
            volume,
            source,
            buffer: [0; SectorSize::MAX],
            at: SectorSize::MAX,
            done: false,
            first,
            index: 0,
        }
    }

    /// The first cluster of the directory being read, 0 for the root.
    pub(crate) const fn first_cluster(&self) -> u32 {
        self.first
    }

    /// Read the next sector into the buffer: `false` at the end of the directory.
    fn fill(&mut self) -> Result<bool, ChainError<D::Error>> {
        let boot = *self.volume.boot_sector();
        let Some(sector) = self.source.next_sector(self.volume, &boot) else {
            return Ok(false);
        };
        let size = boot.sector_size().bytes();
        self.volume.read_sector(sector?, &mut self.buffer[..size])?;
        self.at = 0;
        Ok(true)
    }

    fn advance(&mut self) -> Option<Result<DirectoryEntry, ChainError<D::Error>>> {
        Some(self.advance_indexed()?.map(|(_, entry)| entry))
    }

    /// The next used slot with its index from the directory's start, counting free slots.
    pub(crate) fn advance_indexed(&mut self) -> Option<Indexed<D::Error>> {
        let size = self.volume.boot_sector().sector_size().bytes();
        let file_allocation_table_type = self.volume.boot_sector().file_allocation_table_type();
        loop {
            if self.at >= size {
                match self.fill() {
                    Ok(true) => {}
                    Ok(false) => return None,
                    Err(error) => return Some(Err(error)),
                }
            }
            let mut slot = [0u8; SLOT_BYTES];
            slot.copy_from_slice(&self.buffer[self.at..self.at + SLOT_BYTES]);
            self.at += SLOT_BYTES;
            let index = self.index;
            self.index = self.index.saturating_add(1);
            match Slot::parse(&slot, file_allocation_table_type) {
                Slot::End => return None,
                Slot::Free => {}
                Slot::Used(entry) => return Some(Ok((index, entry))),
            }
        }
    }
}

impl<D: SectorDevice> Iterator for Directory<'_, D> {
    type Item = Result<DirectoryEntry, ChainError<D::Error>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let result = self.advance();
        if !matches!(result, Some(Ok(_))) {
            self.done = true;
        }
        result
    }
}

#[cfg(kani)]
pub(crate) mod verification;

#[cfg(test)]
mod tests;
