//! **The boot sector and its BIOS parameter block**, parsed from hostile bytes, and the FAT type
//! the volume's cluster count makes it.
//!
//! Every field a later read depends on is checked here, once, so that what [`BootSector::parse`]
//! accepts describes a volume whose regions lie in order inside its own sector count and whose
//! table has an entry for every cluster. The offsets and the type rule are those of Microsoft's
//! "FAT: General Overview of On-Disk Format", version 1.03.
//!
//! Name: provisional.

use crate::sector::{SectorDevice, SectorSize};

/// The cluster count below which a volume is FAT12, by the specification's rule.
const FAT16_MIN_CLUSTERS: u32 = 4085;

/// The cluster count below which a volume is FAT16, and at or above which it is FAT32.
const FAT32_MIN_CLUSTERS: u32 = 65_525;

/// The most clusters a FAT32 volume may hold: its highest cluster number, `clusters + 1`, must stay
/// below the bad-cluster marker `0x0FFF_FFF7` with the specification's margin.
const FAT32_MAX_CLUSTERS: u32 = 0x0FFF_FFF5 - 1;

/// The bytes of a sector the parse reads: the BPB at the start and the signature at 510.
const PARSED: usize = 512;

/// **The FAT type**, decided by the count of data clusters and by nothing else.
///
/// The type string at `BS_FilSysType` is informational and never read. A volume whose BPB layout
/// disagrees with its cluster count is refused (see [`ParseError::LayoutContradictsClusterCount`]).
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FatType {
    /// Fewer than 4085 clusters; 12-bit table entries.
    Fat12,
    /// From 4085 up to 65524 clusters; 16-bit table entries.
    Fat16,
    /// 65525 clusters or more; 32-bit table entries whose top four bits are reserved.
    Fat32,
}

impl FatType {
    /// The type the specification assigns to a volume of `clusters` data clusters.
    ///
    /// Name: provisional.
    pub const fn from_clusters(clusters: u32) -> FatType {
        if clusters < FAT16_MIN_CLUSTERS {
            FatType::Fat12
        } else if clusters < FAT32_MIN_CLUSTERS {
            FatType::Fat16
        } else {
            FatType::Fat32
        }
    }

    /// The width of one table entry in bits: 12, 16 or 32.
    ///
    /// Name: provisional.
    pub const fn entry_bits(self) -> u32 {
        match self {
            FatType::Fat12 => 12,
            FatType::Fat16 => 16,
            FatType::Fat32 => 32,
        }
    }
}

/// **Where the root directory lives**, which is the one structural difference FAT32 makes.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RootDirectory {
    /// FAT12 and FAT16: a fixed region between the last table and the first cluster.
    Region {
        /// The volume-relative sector the region begins at.
        first_sector: u32,
        /// How many sectors it spans.
        sectors: u32,
        /// How many 32-byte entries it holds.
        entries: u16,
    },
    /// FAT32: an ordinary cluster chain starting at this cluster.
    Cluster(u32),
}

/// **Why a sector is not a boot sector this crate will read.** Each names the field it failed on.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParseError {
    /// The buffer holds fewer than 512 bytes.
    TooShort,
    /// Bytes 510 and 511 are not `0x55 0xAA`.
    NoSignature,
    /// `BS_jmpBoot` is neither `0xEB ?? 0x90` nor `0xE9 ?? ??`.
    NoJump,
    /// `BPB_BytsPerSec` is not 512, 1024, 2048 or 4096.
    SectorSize,
    /// `BPB_SecPerClus` is not a power of two from 1 to 128.
    ClusterSize,
    /// `BPB_RsvdSecCnt` is zero, which would put the table over the boot sector.
    NoReservedSectors,
    /// `BPB_NumFATs` is zero.
    NoTables,
    /// `BPB_Media` is not `0xF0` or `0xF8` to `0xFF`.
    Media,
    /// Both `BPB_TotSec16` and `BPB_TotSec32` are zero.
    NoSectors,
    /// Both `BPB_FATSz16` and `BPB_FATSz32` are zero.
    NoTableSectors,
    /// The reserved sectors, the tables and the root directory region end past the volume's last
    /// sector.
    MetadataPastEnd,
    /// The data region holds no whole cluster.
    NoClusters,
    /// The BPB is laid out for one FAT type and the cluster count makes the volume another: a FAT32
    /// layout (`BPB_FATSz16` and `BPB_RootEntCnt` zero) under 65525 clusters, or a FAT12 or FAT16
    /// layout at or over it. Drivers disagree about such a volume, so this crate reads none.
    LayoutContradictsClusterCount,
    /// More clusters than FAT32 can number.
    TooManyClusters,
    /// One table is too short to hold an entry for every cluster.
    TableTooSmall,
    /// FAT32's `BPB_RootClus` is not a data cluster of this volume.
    RootCluster,
    /// FAT32's `BPB_FSVer` is not 0.0, the only version the specification defines.
    Version,
    /// FAT32's `BPB_ExtFlags` names an active table the volume does not have.
    ActiveTable,
}

/// **Why [`BootSector::read`] failed**: the device's error, or the sector it returned.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReadError<E> {
    /// The device refused the read; its own error, unchanged.
    Device(E),
    /// Sector 0 is not a boot sector this crate will read.
    Refused(ParseError),
    /// The volume's `BPB_BytsPerSec` differs from the device's sector size.
    SectorSizeMismatch {
        /// What the device reports.
        device: SectorSize,
        /// What the volume's BPB states.
        volume: SectorSize,
    },
}

/// **A parsed boot sector**: the volume's geometry, checked to be self-consistent.
///
/// Every value here comes from [`BootSector::parse`], which refuses a sector whose regions overlap,
/// run past the volume's last sector, or leave a cluster without a table entry. A caller can index
/// with these numbers without rechecking them against each other. They are not checked against the
/// device's length, which the crate does not know.
///
/// # Examples
///
/// ```
/// use file_allocation_table::{BootSector, FatType, SECTOR, Volume};
///
/// let volume = Volume::new(1024 * 1024, 0, 4096, 1, "BOOTX64.EFI").unwrap();
/// let mut sector = [0u8; SECTOR];
/// volume.sector(0, &mut sector);
///
/// let boot = BootSector::parse(&sector).unwrap();
/// assert_eq!(boot.fat_type(), FatType::Fat32);
/// assert_eq!(boot.clusters(), volume.clusters());
/// ```
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BootSector {
    sector_size: SectorSize,
    cluster_shift: u32,
    reserved_sectors: u16,
    tables: u8,
    table_sectors: u32,
    total_sectors: u32,
    media: u8,
    hidden_sectors: u32,
    fat_type: FatType,
    clusters: u32,
    first_data_sector: u32,
    root: RootDirectory,
    active_table: Option<u8>,
    fs_info_sector: Option<u16>,
    backup_boot_sector: Option<u16>,
    volume_id: Option<u32>,
}

/// A little-endian `u16` at `at`. The callers pass constant offsets under [`PARSED`].
fn get16(sector: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([sector[at], sector[at + 1]])
}

/// A little-endian `u32` at `at`. The callers pass constant offsets under [`PARSED`].
fn get32(sector: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([sector[at], sector[at + 1], sector[at + 2], sector[at + 3]])
}

/// A reserved-region sector number from FAT32's BPB, or `None` when it names no sector the
/// reserved region holds: 0 and `0xFFFF` mean "none", and a number at or past the region is
/// ignored rather than followed.
fn reserved_pointer(value: u16, reserved_sectors: u16) -> Option<u16> {
    (value != 0 && value < reserved_sectors).then_some(value)
}

impl BootSector {
    /// **Parse a boot sector from `sector`**, which holds at least its first 512 bytes.
    ///
    /// Returns the volume's geometry, or the first field that makes it unreadable. Never panics
    /// and never reads past byte 511, whatever `sector` holds.
    ///
    /// Name: provisional.
    pub fn parse(sector: &[u8]) -> Result<BootSector, ParseError> {
        let sector = sector.get(..PARSED).ok_or(ParseError::TooShort)?;
        if sector[510] != 0x55 || sector[511] != 0xAA {
            return Err(ParseError::NoSignature);
        }
        if !((sector[0] == 0xEB && sector[2] == 0x90) || sector[0] == 0xE9) {
            return Err(ParseError::NoJump);
        }
        let sector_size =
            SectorSize::from_bytes(get16(sector, 11)).ok_or(ParseError::SectorSize)?;
        let per_cluster = sector[13];
        if !per_cluster.is_power_of_two() {
            return Err(ParseError::ClusterSize);
        }
        let cluster_shift = per_cluster.trailing_zeros();
        let reserved_sectors = get16(sector, 14);
        if reserved_sectors == 0 {
            return Err(ParseError::NoReservedSectors);
        }
        let tables = sector[16];
        if tables == 0 {
            return Err(ParseError::NoTables);
        }
        let media = sector[21];
        if media != 0xF0 && media < 0xF8 {
            return Err(ParseError::Media);
        }
        let root_entries = get16(sector, 17);
        let total_sectors = match get16(sector, 19) {
            0 => get32(sector, 32),
            small => u32::from(small),
        };
        if total_sectors == 0 {
            return Err(ParseError::NoSectors);
        }
        let table_sectors_16 = get16(sector, 22);
        let table_sectors = match table_sectors_16 {
            0 => get32(sector, 36),
            small => u32::from(small),
        };
        if table_sectors == 0 {
            return Err(ParseError::NoTableSectors);
        }

        // Every sum and product below is taken in u64, where none of them can overflow: the
        // largest is 65535 + 255 * (2^32 - 1) + 2^16 sectors.
        let root_sectors =
            (u64::from(root_entries) * 32 + sector_size.bytes() as u64 - 1) >> sector_size.shift();
        let root_first = u64::from(reserved_sectors) + u64::from(tables) * u64::from(table_sectors);
        let first_data_sector = root_first + root_sectors;
        let data_sectors = u64::from(total_sectors)
            .checked_sub(first_data_sector)
            .ok_or(ParseError::MetadataPastEnd)?;
        // Bounded by `total_sectors`, so the conversions to u32 from here on cannot truncate.
        let clusters = (data_sectors >> cluster_shift) as u32;
        let first_data_sector = first_data_sector as u32;
        if clusters == 0 {
            return Err(ParseError::NoClusters);
        }

        let fat_type = FatType::from_clusters(clusters);
        let layout_is_32 = table_sectors_16 == 0 && root_entries == 0;
        let layout_is_16 = table_sectors_16 != 0 && root_entries != 0;
        match fat_type {
            FatType::Fat32 if !layout_is_32 => {
                return Err(ParseError::LayoutContradictsClusterCount);
            }
            FatType::Fat12 | FatType::Fat16 if !layout_is_16 => {
                return Err(ParseError::LayoutContradictsClusterCount);
            }
            _ => {}
        }
        if clusters > FAT32_MAX_CLUSTERS {
            return Err(ParseError::TooManyClusters);
        }
        // Clusters 0 and 1 have table entries too, which is why the table needs `clusters + 2`.
        let needed_bits = (u64::from(clusters) + 2) * u64::from(fat_type.entry_bits());
        let held_bits = u64::from(table_sectors) << (sector_size.shift() + 3);
        if needed_bits > held_bits {
            return Err(ParseError::TableTooSmall);
        }

        let mut boot = BootSector {
            sector_size,
            cluster_shift,
            reserved_sectors,
            tables,
            table_sectors,
            total_sectors,
            media,
            hidden_sectors: get32(sector, 28),
            fat_type,
            clusters,
            first_data_sector,
            root: RootDirectory::Region {
                first_sector: root_first as u32,
                sectors: root_sectors as u32,
                entries: root_entries,
            },
            active_table: None,
            fs_info_sector: None,
            backup_boot_sector: None,
            volume_id: (sector[38] == 0x29).then(|| get32(sector, 39)),
        };
        if fat_type == FatType::Fat32 {
            boot.parse_fat32(sector)?;
        }
        Ok(boot)
    }

    /// The fields only FAT32's extended BPB carries, at offsets 36 to 89.
    fn parse_fat32(&mut self, sector: &[u8]) -> Result<(), ParseError> {
        if get16(sector, 42) != 0 {
            return Err(ParseError::Version);
        }
        let root = get32(sector, 44);
        if self.cluster_first_sector(root).is_none() {
            return Err(ParseError::RootCluster);
        }
        self.root = RootDirectory::Cluster(root);
        // Bit 7 clear means every table is a mirror and a writer updates them all; set, bits 0 to
        // 3 name the one table that is live.
        let flags = get16(sector, 40);
        if flags & 0x80 != 0 {
            let active = (flags & 0x0F) as u8;
            if active >= self.tables {
                return Err(ParseError::ActiveTable);
            }
            self.active_table = Some(active);
        }
        self.fs_info_sector = reserved_pointer(get16(sector, 48), self.reserved_sectors);
        self.backup_boot_sector = reserved_pointer(get16(sector, 50), self.reserved_sectors);
        self.volume_id = (sector[66] == 0x29).then(|| get32(sector, 67));
        Ok(())
    }

    /// **Read and parse sector 0 of `device`.**
    ///
    /// Refuses a volume whose `BPB_BytsPerSec` is not the device's sector size, since every later
    /// read would land at the wrong offset.
    ///
    /// Name: provisional.
    pub fn read<D: SectorDevice>(device: &mut D) -> Result<BootSector, ReadError<D::Error>> {
        let size = device.sector_size();
        let mut buffer = [0u8; SectorSize::MAX];
        let buffer = &mut buffer[..size.bytes()];
        device.read_sector(0, buffer).map_err(ReadError::Device)?;
        let boot = BootSector::parse(buffer).map_err(ReadError::Refused)?;
        if boot.sector_size != size {
            return Err(ReadError::SectorSizeMismatch {
                device: size,
                volume: boot.sector_size,
            });
        }
        Ok(boot)
    }

    /// The volume's sector size, from `BPB_BytsPerSec`.
    ///
    /// Name: provisional.
    pub const fn sector_size(&self) -> SectorSize {
        self.sector_size
    }

    /// Sectors per cluster: a power of two from 1 to 128.
    ///
    /// Name: provisional.
    pub const fn sectors_per_cluster(&self) -> u32 {
        1 << self.cluster_shift
    }

    /// Bytes per cluster: from 512 to 512 KiB.
    ///
    /// Name: provisional.
    pub const fn bytes_per_cluster(&self) -> u32 {
        (self.sector_size.bytes() as u32) << self.cluster_shift
    }

    /// The FAT type, from the cluster count.
    ///
    /// Name: provisional.
    pub const fn fat_type(&self) -> FatType {
        self.fat_type
    }

    /// How many data clusters the volume has. They are numbered 2 to `clusters() + 1`.
    ///
    /// Name: provisional.
    pub const fn clusters(&self) -> u32 {
        self.clusters
    }

    /// The volume's sector count, from `BPB_TotSec16` or `BPB_TotSec32`.
    ///
    /// Name: provisional.
    pub const fn total_sectors(&self) -> u32 {
        self.total_sectors
    }

    /// Sectors ahead of the first table, the boot sector among them.
    ///
    /// Name: provisional.
    pub const fn reserved_sectors(&self) -> u16 {
        self.reserved_sectors
    }

    /// How many copies of the table the volume holds.
    ///
    /// Name: provisional.
    pub const fn tables(&self) -> u8 {
        self.tables
    }

    /// Sectors in one copy of the table.
    ///
    /// Name: provisional.
    pub const fn table_sectors(&self) -> u32 {
        self.table_sectors
    }

    /// The volume-relative sector table `index` begins at, or `None` when the volume has no such
    /// table.
    ///
    /// Name: provisional.
    pub const fn table_first_sector(&self, index: u8) -> Option<u32> {
        if index >= self.tables {
            return None;
        }
        Some(self.reserved_sectors as u32 + index as u32 * self.table_sectors)
    }

    /// The one table a FAT32 volume keeps live, when `BPB_ExtFlags` turns mirroring off; `None`
    /// when every table is a mirror, which FAT12 and FAT16 always are.
    ///
    /// Name: provisional.
    pub const fn active_table(&self) -> Option<u8> {
        self.active_table
    }

    /// The volume-relative sector cluster 2 begins at.
    ///
    /// Name: provisional.
    pub const fn first_data_sector(&self) -> u32 {
        self.first_data_sector
    }

    /// Where the root directory lives.
    ///
    /// Name: provisional.
    pub const fn root_directory(&self) -> RootDirectory {
        self.root
    }

    /// The media descriptor byte, `BPB_Media`.
    ///
    /// Name: provisional.
    pub const fn media(&self) -> u8 {
        self.media
    }

    /// `BPB_HiddSec`: the sectors ahead of the volume on its disk, as the formatter recorded them.
    /// Informational; nothing in this crate reads through it.
    ///
    /// Name: provisional.
    pub const fn hidden_sectors(&self) -> u32 {
        self.hidden_sectors
    }

    /// FAT32's `FSInfo` sector, or `None` when the BPB names none inside the reserved region.
    ///
    /// Name: provisional.
    pub const fn fs_info_sector(&self) -> Option<u16> {
        self.fs_info_sector
    }

    /// FAT32's backup boot sector, or `None` when the BPB names none inside the reserved region.
    ///
    /// Name: provisional.
    pub const fn backup_boot_sector(&self) -> Option<u16> {
        self.backup_boot_sector
    }

    /// The volume serial number, when the extended boot signature `0x29` says it is present.
    ///
    /// Name: provisional.
    pub const fn volume_id(&self) -> Option<u32> {
        self.volume_id
    }

    /// **The volume-relative sector `cluster` begins at**, or `None` when `cluster` is not one of
    /// this volume's data clusters (2 to `clusters() + 1`). The cluster's
    /// [`BootSector::sectors_per_cluster`] sectors all lie inside the volume.
    ///
    /// Name: provisional.
    pub const fn cluster_first_sector(&self, cluster: u32) -> Option<u32> {
        if cluster < 2 || cluster - 2 >= self.clusters {
            return None;
        }
        Some(self.first_data_sector + ((cluster - 2) << self.cluster_shift))
    }
}

#[cfg(kani)]
mod verification;

#[cfg(test)]
mod tests;
