//! **The file allocation table**: where a cluster's entry lies, and what the entry says.
//!
//! Each data cluster has one entry in the table, 12, 16 or 32 bits wide by the FAT type. An entry
//! names the next cluster of a chain, ends the chain, marks the cluster free or bad, or holds a
//! value no reader may follow. The width and the markers are those of Microsoft's "FAT: General
//! Overview of On-Disk Format", version 1.03, section 4.
//!
//! This module computes and decodes; [`crate::Volume`] does the reading.
//!
//! Name: ratified 2026-10-26 (UTC), on #2760. The file allocation table, spelled out as the crate
//! spells FAT (`table_sectors`, `TABLE_COUNT`).

use crate::boot_sector::{BootSector, FileAllocationTableType};

/// **What one table entry says about the cluster after this one.**
///
/// A value that names a cluster is reported as [`TableEntry::Next`] whether or not the volume has
/// that cluster; the chain walk checks it against the volume, since only the volume knows its
/// cluster count.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. The specification's "FAT entry", spelled out the same
/// way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TableEntry {
    /// The cluster is free: entry value 0.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. Entry value 0, which the specification calls
    /// free.
    Free,
    /// Entry value 1, which the specification reserves and no chain may hold.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. Entry value 1, which the specification reserves.
    Reserved,
    /// The chain continues at this cluster number, at least 2 and below the bad-cluster marker.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. The next cluster of the chain.
    Next(u32),
    /// The cluster is marked bad: `0xFF7`, `0xFFF7` or `0x0FFF_FFF7` by the FAT type.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. The specification's "bad cluster" marker.
    Bad,
    /// The chain ends with this cluster: any value from the bad marker plus one to the type's
    /// maximum.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. The specification's "end of clusterchain" marker.
    EndOfChain,
}

impl TableEntry {
    /// **Decode `raw`, an entry as read from a table of `file_allocation_table_type`.**
    ///
    /// A FAT12 entry keeps its low 12 bits and a FAT16 entry its low 16. A FAT32 entry keeps its
    /// low 28: the top four bits are reserved, and the specification says a reader ignores them.
    ///
    /// Name: ratified 2026-10-26 (UTC), on #2760. Turns a raw entry into a `TableEntry`.
    pub const fn decode(
        file_allocation_table_type: FileAllocationTableType,
        raw: u32,
    ) -> TableEntry {
        let (value, bad) = match file_allocation_table_type {
            FileAllocationTableType::Fat12 => (raw & 0xFFF, 0xFF7),
            FileAllocationTableType::Fat16 => (raw & 0xFFFF, 0xFFF7),
            FileAllocationTableType::Fat32 => (raw & 0x0FFF_FFFF, 0x0FFF_FFF7),
        };
        if value == 0 {
            TableEntry::Free
        } else if value == 1 {
            TableEntry::Reserved
        } else if value < bad {
            TableEntry::Next(value)
        } else if value == bad {
            TableEntry::Bad
        } else {
            TableEntry::EndOfChain
        }
    }
}

/// **Where cluster `cluster`'s entry starts**: the volume-relative sector, and the byte offset
/// inside it. `None` when `cluster` is not a data cluster of the volume.
///
/// The table read is the live one when FAT32 turns mirroring off, and the first otherwise. A FAT12
/// entry starting at the last byte of a sector ends in the next one, which also lies inside the
/// table: `BootSector::parse` refuses a table too short to hold an entry for every cluster.
pub(crate) fn entry_position(boot: &BootSector, cluster: u32) -> Option<(u32, usize)> {
    boot.cluster_first_sector(cluster)?;
    let table = boot.active_table().unwrap_or_default();
    let first = boot.table_first_sector(table)?;
    let cluster = u64::from(cluster);
    let offset = match boot.file_allocation_table_type() {
        FileAllocationTableType::Fat12 => cluster + cluster / 2,
        FileAllocationTableType::Fat16 => cluster * 2,
        FileAllocationTableType::Fat32 => cluster * 4,
    };
    let shift = boot.sector_size().shift();
    // Inside the table, by the parse's check, so the sector number fits a u32.
    let sector = first + (offset >> shift) as u32;
    let within = (offset & ((1 << shift) - 1)) as usize;
    Some((sector, within))
}

/// The entry for `cluster` from the bytes that hold it: `low` the first byte, `high` the rest, as
/// many as the width needs (one for FAT12 and FAT16, three for FAT32). FAT12 packs two entries
/// into three bytes, so an odd cluster's entry is the high 12 bits of its two bytes.
pub(crate) fn assemble(
    file_allocation_table_type: FileAllocationTableType,
    cluster: u32,
    bytes: [u8; 4],
) -> u32 {
    match file_allocation_table_type {
        FileAllocationTableType::Fat12 => {
            let pair = u32::from(u16::from_le_bytes([bytes[0], bytes[1]]));
            if cluster & 1 == 1 {
                pair >> 4
            } else {
                pair & 0xFFF
            }
        }
        FileAllocationTableType::Fat16 => u32::from(u16::from_le_bytes([bytes[0], bytes[1]])),
        FileAllocationTableType::Fat32 => u32::from_le_bytes(bytes),
    }
}

/// How many bytes an entry spans in the table: 2 for FAT12 (one and a half, read as two) and
/// FAT16, 4 for FAT32.
pub(crate) const fn entry_bytes(file_allocation_table_type: FileAllocationTableType) -> usize {
    match file_allocation_table_type {
        FileAllocationTableType::Fat12 | FileAllocationTableType::Fat16 => 2,
        FileAllocationTableType::Fat32 => 4,
    }
}

#[cfg(kani)]
mod verification;

#[cfg(test)]
mod tests;
