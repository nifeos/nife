//! **The master boot record**: the partition table in sector 0 of a whole-disk image, and a
//! device that shows one partition as if it were the whole device.
//!
//! A USB stick usually arrives with an MBR whose entry says where its FAT volume starts. The table
//! is four 16-byte entries at byte 446 of the first sector, then the signature `0x55 0xAA`. The
//! layout is Microsoft's, in "FAT: General Overview of On-Disk Format", version 1.03, and the
//! conventional one every partitioning tool writes. A parse never panics, and every partition it
//! accepts lies inside the 32-bit sector numbers an entry can hold.
//!
//! A bare FAT volume has no MBR. Its sector 0 is a boot sector whose last two bytes are the same
//! signature, so a caller tries [`BootSector::read`](crate::BootSector::read) first and the
//! partition table second: bytes 446 and up of a FAT boot sector are boot code, which this parse
//! refuses or reads as nonsense.
//!
//! Name: provisional. The standard name of the structure.

use crate::sector::{SectorDevice, SectorSize};

/// The byte offset of the first partition entry in sector 0.
const TABLE_OFFSET: usize = 446;
/// The bytes of one partition entry.
const ENTRY_BYTES: usize = 16;
/// The partition entries in the table.
const ENTRIES: usize = 4;

/// **The most partitions an MBR holds.**
///
/// Name: provisional. The count, spelled like `TABLE_COUNT`.
pub const PARTITION_COUNT: usize = ENTRIES;

/// **Why sector 0 is not a usable partition table.**
///
/// Name: provisional. Why a parse refused, beside `ParseError`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MasterBootRecordError {
    /// The sector is shorter than 512 bytes.
    ///
    /// Name: provisional. The condition.
    TooShort,
    /// The last two bytes are not `0x55 0xAA`.
    ///
    /// Name: provisional. The signature the specification requires.
    NoSignature,
    /// Partition `index` has a status byte other than `0x00` and `0x80`, which the convention
    /// reserves.
    ///
    /// Name: provisional. The status byte's two legal values.
    BadStatus {
        /// The entry's place in the table, 0 to 3.
        index: usize,
    },
    /// Partition `index` has a type but no sectors.
    ///
    /// Name: provisional. A used entry with a zero length.
    EmptyPartition {
        /// The entry's place in the table, 0 to 3.
        index: usize,
    },
    /// Partition `index` ends past sector 2^32.
    ///
    /// Name: provisional. First sector plus length overflows the 32-bit sector numbers.
    PartitionOverflows {
        /// The entry's place in the table, 0 to 3.
        index: usize,
    },
}

/// **One used partition table entry.**
///
/// Name: provisional. The standard name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Partition {
    bootable: bool,
    kind: u8,
    first_sector: u32,
    sectors: u32,
}

impl Partition {
    /// Whether the status byte marks the partition active (`0x80`).
    ///
    /// Name: provisional. The convention's "active" flag, named for what a firmware does with it.
    pub const fn is_bootable(&self) -> bool {
        self.bootable
    }

    /// The partition type byte as stored: `0x0B` for FAT32, for example.
    ///
    /// Name: provisional. The convention's "partition type".
    pub const fn kind(&self) -> u8 {
        self.kind
    }

    /// The partition's first sector, in the device's sectors.
    ///
    /// Name: provisional. `first_sector` as in `VolumeLayout`.
    pub const fn first_sector(&self) -> u32 {
        self.first_sector
    }

    /// The partition's length in sectors; at least 1.
    ///
    /// Name: provisional. A count of sectors.
    pub const fn sector_count(&self) -> u32 {
        self.sectors
    }

    /// Whether the type byte is one a FAT volume uses: `0x01` (FAT12), `0x04`, `0x06` and `0x0E`
    /// (FAT16), `0x0B` and `0x0C` (FAT32) or `0xEF` (an EFI system partition, which is FAT).
    ///
    /// The type is a hint. A volume is a FAT volume if its boot sector says so, and
    /// [`Volume::open`](crate::Volume::open) is what checks that.
    ///
    /// Name: provisional. Names the file system rather than listing the bytes.
    pub const fn holds_file_allocation_table(&self) -> bool {
        matches!(self.kind, 0x01 | 0x04 | 0x06 | 0x0B | 0x0C | 0x0E | 0xEF)
    }

    /// **The device that shows only this partition**, with sector 0 its first sector.
    ///
    /// Name: provisional. Wraps `device` for this partition.
    pub const fn device<D>(self, device: D) -> PartitionDevice<D> {
        PartitionDevice {
            device,
            first_sector: self.first_sector,
            sectors: self.sectors,
        }
    }
}

/// **A partition table read from sector 0.**
///
/// Name: provisional. The standard name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MasterBootRecord {
    partitions: [Option<Partition>; ENTRIES],
}

impl MasterBootRecord {
    /// **Parse the 512 bytes of sector 0.**
    ///
    /// An entry whose type byte is 0 is unused whatever its other bytes say. Every other entry has
    /// a legal status byte, at least one sector, and an end within 2^32. The table is refused whole
    /// when any entry breaks that, since a table with one wrong entry cannot be trusted about the
    /// rest. Partitions that overlap each other, or run past the device, are not detected; the
    /// device refuses a read past its end.
    ///
    /// Name: provisional. The verb `BootSector::parse` uses for hostile bytes.
    pub fn parse(sector: &[u8]) -> Result<MasterBootRecord, MasterBootRecordError> {
        if sector.len() < SectorSize::Bytes512.bytes() {
            return Err(MasterBootRecordError::TooShort);
        }
        if sector[510] != 0x55 || sector[511] != 0xAA {
            return Err(MasterBootRecordError::NoSignature);
        }
        let mut partitions = [None; ENTRIES];
        for (index, slot) in partitions.iter_mut().enumerate() {
            let at = TABLE_OFFSET + index * ENTRY_BYTES;
            let entry = &sector[at..at + ENTRY_BYTES];
            let kind = entry[4];
            if kind == 0 {
                continue;
            }
            let bootable = match entry[0] {
                0x00 => false,
                0x80 => true,
                _ => return Err(MasterBootRecordError::BadStatus { index }),
            };
            let first_sector = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]);
            let sectors = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]);
            if sectors == 0 {
                return Err(MasterBootRecordError::EmptyPartition { index });
            }
            if u64::from(first_sector) + u64::from(sectors) > 1 << 32 {
                return Err(MasterBootRecordError::PartitionOverflows { index });
            }
            *slot = Some(Partition {
                bootable,
                kind,
                first_sector,
                sectors,
            });
        }
        Ok(MasterBootRecord { partitions })
    }

    /// **Read sector 0 of `device` and parse it.**
    ///
    /// Name: provisional. Beside `BootSector::read`.
    pub fn read<D: SectorDevice>(
        device: &mut D,
    ) -> Result<MasterBootRecord, MasterBootRecordReadError<D::Error>> {
        let size = device.sector_size();
        let mut sector = [0u8; SectorSize::MAX];
        let sector = &mut sector[..size.bytes()];
        device
            .read_sector(0, sector)
            .map_err(MasterBootRecordReadError::Device)?;
        MasterBootRecord::parse(sector).map_err(MasterBootRecordReadError::Parse)
    }

    /// The partition at table position `index` (0 to 3), or `None` for an unused entry and for an
    /// index past the table.
    ///
    /// Name: provisional. A partition by its place.
    pub const fn partition(&self, index: usize) -> Option<Partition> {
        if index < ENTRIES {
            self.partitions[index]
        } else {
            None
        }
    }

    /// Every used partition, in table order.
    ///
    /// Name: provisional. The used entries.
    pub fn partitions(&self) -> impl Iterator<Item = Partition> + '_ {
        self.partitions.iter().flatten().copied()
    }
}

/// **Why [`MasterBootRecord::read`] failed.**
///
/// Name: provisional. Beside `ReadError`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MasterBootRecordReadError<E> {
    /// The device refused to read sector 0; its own error, unchanged.
    ///
    /// Name: provisional. The same variant `ReadError` has.
    Device(E),
    /// Sector 0 is not a partition table.
    ///
    /// Name: provisional. The same variant `ReadError` has.
    Parse(MasterBootRecordError),
}

/// **Why a [`PartitionDevice`] read failed.**
///
/// Name: provisional. Beside `ChainError`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PartitionError<E> {
    /// The device refused the read; its own error, unchanged.
    ///
    /// Name: provisional. The same variant `ReadError` has.
    Device(E),
    /// The sector asked for is past the partition's last sector.
    ///
    /// Name: provisional. The condition.
    OutsidePartition(u64),
}

/// **One partition of a device, as a device**: sector `n` is the underlying device's sector
/// `first_sector + n`, and a sector past the partition's length is refused without a read.
///
/// # Examples
///
/// Open the first FAT partition of a whole-disk image:
///
/// ```
/// use file_allocation_table::{
///     MasterBootRecord, SectorDevice, SectorSize, Volume,
/// };
///
/// fn open<D: SectorDevice>(mut disk: D) -> Option<()> {
///     let table = MasterBootRecord::read(&mut disk).ok()?;
///     let partition = table.partitions().find(|p| p.holds_file_allocation_table())?;
///     let volume = Volume::open(partition.device(disk)).ok()?;
///     let _root = volume.boot_sector().root_directory();
///     Some(())
/// }
/// # struct Empty;
/// # impl SectorDevice for Empty {
/// #     type Error = ();
/// #     fn sector_size(&self) -> SectorSize { SectorSize::Bytes512 }
/// #     fn read_sector(&mut self, _: u64, b: &mut [u8]) -> Result<(), ()> { b.fill(0); Ok(()) }
/// # }
/// assert_eq!(open(Empty), None);
/// ```
///
/// Name: provisional. A device that is one partition.
#[derive(Debug, Clone, Copy)]
pub struct PartitionDevice<D> {
    device: D,
    first_sector: u32,
    sectors: u32,
}

impl<D> PartitionDevice<D> {
    /// Give the underlying device back.
    ///
    /// Name: provisional. The `into_` convention, as `Volume::into_device`.
    pub fn into_device(self) -> D {
        self.device
    }
}

impl<D: SectorDevice> SectorDevice for PartitionDevice<D> {
    type Error = PartitionError<D::Error>;

    fn sector_size(&self) -> SectorSize {
        self.device.sector_size()
    }

    fn read_sector(&mut self, index: u64, buffer: &mut [u8]) -> Result<(), Self::Error> {
        if index >= u64::from(self.sectors) {
            return Err(PartitionError::OutsidePartition(index));
        }
        self.device
            .read_sector(u64::from(self.first_sector) + index, buffer)
            .map_err(PartitionError::Device)
    }
}

#[cfg(kani)]
mod verification;

#[cfg(test)]
mod tests;
