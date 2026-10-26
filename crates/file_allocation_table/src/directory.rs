//! **Directory entries**: each 32-byte slot of a directory parsed from hostile bytes, and a
//! directory read slot by slot through its region or its cluster chain.
//!
//! A slot is the end of the directory, a free slot, a short (8.3) entry, or one piece of a long
//! name. This module reports each long-name piece as it stands; it does not assemble a long name
//! or check its checksum against the short entry that follows. The layouts are those of
//! Microsoft's "FAT: General Overview of On-Disk Format", version 1.03, sections 6 and 7.

use crate::boot_sector::{FatType, RootDirectory};
use crate::reader::{ChainError, Volume, Walk};
use crate::sector::{SectorDevice, SectorSize};

/// The bytes of one directory slot.
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

/// **One directory slot, parsed.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slot {
    /// The first name byte is 0: this slot and every slot after it are free, and a reader stops.
    End,
    /// The first name byte is `0xE5`: a free slot, deleted or never used, between used ones.
    Free,
    /// A short entry or a long-name piece.
    Used(DirectoryEntry),
}

/// **A used directory slot**: a short entry, or one piece of a long name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DirectoryEntry {
    /// An 8.3 entry: a file, a directory, `.` or `..`, or the volume label.
    Short(ShortEntry),
    /// Thirteen UTF-16 units of a long name, which sits in the slots ahead of its short entry.
    LongName(LongNameEntry),
}

/// **A short (8.3) directory entry.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShortEntry {
    name: [u8; 11],
    attributes: u8,
    first_cluster: u32,
    size: u32,
}

/// **One piece of a long file name.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LongNameEntry {
    order: u8,
    checksum: u8,
    units: [u16; 13],
}

impl Slot {
    /// **Parse one slot of a directory on a volume of `fat_type`.**
    ///
    /// Never panics: every 32 bytes are one of the variants. A short entry's first cluster takes
    /// its high half from `DIR_FstClusHI` only on FAT32; FAT12 and FAT16 leave that word to other
    /// uses, so it is ignored there. The cluster is not checked against any volume here; a reader
    /// that follows it does that.
    pub fn parse(slot: &[u8; SLOT_BYTES], fat_type: FatType) -> Slot {
        match slot[0] {
            0 => return Slot::End,
            FREE => return Slot::Free,
            _ => {}
        }
        let attributes = slot[11];
        if attributes & LONG_NAME_MASK == LONG_NAME {
            let mut units = [0u16; 13];
            for (unit, at) in units.iter_mut().zip(
                (1..11)
                    .step_by(2)
                    .chain((14..26).step_by(2))
                    .chain((28..32).step_by(2)),
            ) {
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
        let high = match fat_type {
            FatType::Fat32 => u32::from(u16::from_le_bytes([slot[20], slot[21]])),
            FatType::Fat12 | FatType::Fat16 => 0,
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
    /// The name as stored: eight bytes of base and three of extension, each padded with spaces,
    /// with a stored `0x05` read back as `0xE5`.
    pub const fn name(&self) -> [u8; 11] {
        self.name
    }

    /// `DIR_Attr`, every bit as stored.
    pub const fn attributes(&self) -> u8 {
        self.attributes
    }

    /// Whether the entry is a directory.
    pub const fn is_directory(&self) -> bool {
        self.attributes & DIRECTORY != 0
    }

    /// Whether the entry is the volume label, which names no file and owns no cluster.
    pub const fn is_volume_label(&self) -> bool {
        self.attributes & VOLUME_ID != 0
    }

    /// The first cluster of the entry's chain, or 0 for an empty file and for a `..` whose parent
    /// is the root. Not checked against the volume.
    pub const fn first_cluster(&self) -> u32 {
        self.first_cluster
    }

    /// The file's length in bytes; 0 for a directory.
    pub const fn size(&self) -> u32 {
        self.size
    }
}

impl LongNameEntry {
    /// The piece's place in its name, counting from 1 at the piece nearest the short entry. The
    /// last-piece bit is cleared; the specification allows 1 to 20, and this value is not checked.
    pub const fn ordinal(&self) -> u8 {
        self.order & !LAST_LONG_ENTRY
    }

    /// Whether this is the name's last piece, which sits first on disk.
    pub const fn is_last(&self) -> bool {
        self.order & LAST_LONG_ENTRY != 0
    }

    /// The checksum of the short name this piece belongs to, as stored.
    pub const fn checksum(&self) -> u8 {
        self.checksum
    }

    /// The piece's thirteen UTF-16 units, as stored: the name ends at a `0x0000` unit, and
    /// `0xFFFF` pads the rest.
    pub const fn units(&self) -> [u16; 13] {
        self.units
    }
}

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

/// **A directory, read slot by slot**: what [`Volume::root_directory`] and [`Volume::directory`]
/// return.
///
/// It yields every used slot in order, skips free ones, and stops at an end slot or at the end
/// of the region or chain. A chain is walked with [`Volume::chain`]'s bound, so the read ends
/// whatever the table holds. After an error it yields nothing more.
#[derive(Debug)]
pub struct Directory<'v, D> {
    volume: &'v mut Volume<D>,
    source: Source,
    buffer: [u8; SectorSize::MAX],
    /// The byte offset of the next slot in `buffer`; one sector or more means a sector is due.
    at: usize,
    done: bool,
}

impl<'v, D: SectorDevice> Directory<'v, D> {
    /// The root directory of `volume`.
    pub(crate) fn root(volume: &'v mut Volume<D>) -> Directory<'v, D> {
        match volume.boot_sector().root_directory() {
            RootDirectory::Region {
                first_sector,
                sectors,
                ..
            } => Directory::new(
                volume,
                Source::Region {
                    next: first_sector,
                    end: first_sector + sectors,
                },
            ),
            RootDirectory::Cluster(first) => Directory::chain(volume, first),
        }
    }

    /// The directory whose chain starts at `first`.
    pub(crate) fn chain(volume: &'v mut Volume<D>, first: u32) -> Directory<'v, D> {
        let source = Source::Chain {
            walk: Walk::new(first),
            next: 0,
            remaining: 0,
        };
        Directory::new(volume, source)
    }

    fn new(volume: &'v mut Volume<D>, source: Source) -> Directory<'v, D> {
        Directory {
            volume,
            source,
            buffer: [0; SectorSize::MAX],
            at: SectorSize::MAX,
            done: false,
        }
    }

    /// The volume-relative sector to read next, `None` at the end of the directory.
    fn next_sector(&mut self) -> Option<Result<u32, ChainError<D::Error>>> {
        match &mut self.source {
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
                    let cluster = match walk.step(self.volume)? {
                        Ok(cluster) => cluster,
                        Err(error) => return Some(Err(error)),
                    };
                    let boot = self.volume.boot_sector();
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

    /// Read the next sector into the buffer: `false` at the end of the directory.
    fn fill(&mut self) -> Result<bool, ChainError<D::Error>> {
        let Some(sector) = self.next_sector() else {
            return Ok(false);
        };
        let size = self.volume.boot_sector().sector_size().bytes();
        self.volume.read_sector(sector?, &mut self.buffer[..size])?;
        self.at = 0;
        Ok(true)
    }

    fn advance(&mut self) -> Option<Result<DirectoryEntry, ChainError<D::Error>>> {
        let size = self.volume.boot_sector().sector_size().bytes();
        let fat_type = self.volume.boot_sector().fat_type();
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
            match Slot::parse(&slot, fat_type) {
                Slot::End => return None,
                Slot::Free => {}
                Slot::Used(entry) => return Some(Ok(entry)),
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

#[cfg(test)]
mod tests;
