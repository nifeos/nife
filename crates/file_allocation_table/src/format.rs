//! **The general format**: lay out an empty FAT12, FAT16 or FAT32 volume of any size and sector
//! size, as the sectors a caller writes, with no I/O.
//!
//! One piece of arithmetic sizes every volume, [`Geometry`], and one pair of builders writes its
//! boot sector and `FSInfo`; the one-file EFI volume of [`crate::VolumeLayout`] uses both, so the
//! BPB is written in one place. Sizing is Microsoft's own (`fatgen103`, "Determining FAT type"),
//! which over-sizes the table slightly rather than solve the circular dependency, so the number it
//! gives is the number every other tool gives; the data area is then rounded up to start on a
//! cluster boundary.
//!
//! Name: provisional. The `mkfs` of the crate, beside `volume`, which keeps the EFI volume's shape.

use crate::boot_sector::FileAllocationTableType;
use crate::long_name::is_short_byte;
use crate::sector::{SectorSize, WritableSectorDevice};
use crate::time::{Clock, Stamp};

/// Reserved sectors of a FAT32 volume: the boot sector, `FSInfo`, their backups at 6 and 7, and
/// room left over as every tool leaves it.
const FAT32_RESERVED: u32 = 32;
/// Root directory entries of a FAT12 or FAT16 volume, the count most tools write.
const ROOT_ENTRIES: u32 = 512;
/// The cluster FAT32's root directory starts at.
const ROOT_CLUSTER: u32 = 2;
/// The most clusters FAT32 addresses.
const FAT32_MOST_CLUSTERS: u32 = 0x0FFF_FFF5 - 2;
/// `ATTR_VOLUME_ID`.
const ATTR_VOLUME_ID: u8 = 0x08;

/// **Why a volume cannot be laid out.** Each is a refusal before any byte is written.
///
/// Name: provisional. Beside `LayoutError`, which says the same of the one-file volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormatError {
    /// The device is too small for the reserved sectors, a table and one cluster.
    ///
    /// Name: provisional. The condition.
    TooSmall,
    /// No cluster size gives the requested type a legal cluster count: the volume is too small or
    /// too large for it. A FAT32 volume needs 65,525 clusters and a FAT16 volume 4,085.
    ///
    /// Name: provisional. The condition, in the specification's terms: the type follows the count.
    NoSuchGeometry,
    /// The label is longer than eleven bytes or holds a byte a short name refuses.
    ///
    /// Name: provisional. The condition.
    LabelRefused,
    /// The clock reported a time FAT cannot store (see [`crate::Clock`]).
    ///
    /// Name: provisional. As `WriteError::TimeOutOfRange`.
    TimeOutOfRange,
    /// The cluster size is not a power of two from 1 to 128 sectors.
    ///
    /// Name: provisional. The condition.
    ClusterSizeRefused,
}

/// **What to format**: the device's size and sector size, and optionally the type, the cluster
/// size, the serial number, the label and the sectors ahead of the volume on its disk.
///
/// Without a type or a cluster size the smallest cluster that gives a legal count wins, and the
/// type follows from the count, as the specification has it.
///
/// Name: provisional. The `newfs_msdos` and `mkfs.fat` options this crate takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatOptions {
    sector_size: SectorSize,
    total_sectors: u32,
    file_allocation_table_type: Option<FileAllocationTableType>,
    cluster_sectors: Option<u32>,
    volume_id: u32,
    label: [u8; 11],
    has_label: bool,
    hidden_sectors: u32,
}

impl FormatOptions {
    /// Options for a device of `total_sectors` sectors of `sector_size`, with every choice left to
    /// the format: serial number 0, the label `NO NAME`, no hidden sectors.
    ///
    /// Name: provisional. The two things every format needs.
    pub const fn new(sector_size: SectorSize, total_sectors: u32) -> FormatOptions {
        FormatOptions {
            sector_size,
            total_sectors,
            file_allocation_table_type: None,
            cluster_sectors: None,
            volume_id: 0,
            label: *b"NO NAME    ",
            has_label: false,
            hidden_sectors: 0,
        }
    }

    /// Require this type, or a refusal.
    ///
    /// Name: provisional. A builder method, as the standard library's own.
    #[must_use]
    pub const fn with_type(mut self, kind: FileAllocationTableType) -> FormatOptions {
        self.file_allocation_table_type = Some(kind);
        self
    }

    /// Require this many sectors to the cluster: a power of two from 1 to 128.
    ///
    /// Name: provisional. As `with_type`.
    #[must_use]
    pub const fn with_cluster_sectors(mut self, sectors: u32) -> FormatOptions {
        self.cluster_sectors = Some(sectors);
        self
    }

    /// The serial number; a caller with an entropy capability draws it, and nothing here invents one.
    ///
    /// Name: provisional. As `with_type`.
    #[must_use]
    pub const fn with_volume_id(mut self, volume_id: u32) -> FormatOptions {
        self.volume_id = volume_id;
        self
    }

    /// The sectors ahead of the volume on its disk, which only `BPB_HiddSec` records.
    ///
    /// Name: provisional. As `with_type`.
    #[must_use]
    pub const fn with_hidden_sectors(mut self, hidden_sectors: u32) -> FormatOptions {
        self.hidden_sectors = hidden_sectors;
        self
    }

    /// The volume label: up to eleven upper-case letters, digits, spaces and the punctuation a
    /// short name admits. A label also becomes the volume-label entry of the root directory.
    ///
    /// Name: provisional. As `with_type`.
    pub fn with_label(mut self, label: &str) -> Result<FormatOptions, FormatError> {
        self.label = label_bytes(label).ok_or(FormatError::LabelRefused)?;
        self.has_label = true;
        Ok(self)
    }
}

/// The eleven bytes of `BS_VolLab` for `label`, or `None` when it is empty, over eleven bytes, or
/// holds a byte a short name refuses. The text is padded with spaces.
pub(crate) fn label_bytes(label: &str) -> Option<[u8; 11]> {
    label_from_bytes(label.as_bytes())
}

/// [`label_bytes`] over the text's bytes, which the proof reaches without a text to validate.
pub(crate) fn label_from_bytes(bytes: &[u8]) -> Option<[u8; 11]> {
    if bytes.is_empty() || bytes.len() > 11 || !bytes.iter().all(|b| is_short_byte(*b)) {
        return None;
    }
    let mut out = [b' '; 11];
    out[..bytes.len()].copy_from_slice(bytes);
    Some(out)
}

/// **A volume's measurements**, from its size alone. Every field is derived by [`Geometry::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Geometry {
    pub(crate) kind: FileAllocationTableType,
    pub(crate) sector_size: SectorSize,
    pub(crate) cluster_sectors: u32,
    pub(crate) reserved_sectors: u32,
    pub(crate) root_entries: u32,
    pub(crate) root_sectors: u32,
    pub(crate) table_sectors: u32,
    pub(crate) total_sectors: u32,
    pub(crate) clusters: u32,
}

/// Why a geometry does not exist for the choices made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeometryError {
    /// The device cannot hold the reserved sectors, a table and a cluster.
    TooSmall,
    /// The cluster count, carried here, falls in another type's range or past what the type
    /// addresses.
    WrongType(u32),
}

impl Geometry {
    /// **Size a volume of `kind` with `cluster_sectors` sectors to the cluster** on a device of
    /// `total_sectors`, by Microsoft's sizing with the data area rounded to a cluster boundary.
    pub(crate) fn new(
        kind: FileAllocationTableType,
        sector_size: SectorSize,
        cluster_sectors: u32,
        total_sectors: u32,
    ) -> Result<Geometry, GeometryError> {
        let fat32 = kind == FileAllocationTableType::Fat32;
        let bytes = sector_size.bytes() as u32;
        let reserved_sectors = if fat32 { FAT32_RESERVED } else { 1 };
        let root_entries = if fat32 { 0 } else { ROOT_ENTRIES };
        let root_sectors = (root_entries * 32).div_ceil(bytes);
        let after_reserved = total_sectors
            .checked_sub(reserved_sectors + root_sectors)
            .ok_or(GeometryError::TooSmall)?;
        let divisor = (bytes / 2 * cluster_sectors + 2) / if fat32 { 2 } else { 1 };
        let mut table_sectors = after_reserved.div_ceil(divisor);
        // Start the data area on a cluster boundary, so a cluster begins where a device block does.
        // FAT32 grows its tables a sector at a time (two tables move the start by two, and its 32
        // reserved sectors are even, so an even cluster size always meets). A FAT12 or FAT16 start
        // can be odd, so those pad their reserved sectors instead. Either way the table must hold
        // an entry for every cluster and the two reserved ones: Microsoft's formula, which the
        // number above is, falls two entries short when the volume divides it exactly, so a table
        // that is too small grows by a sector and the sizing runs again.
        let bits = u64::from(kind.entry_bits());
        let mut reserved_sectors = reserved_sectors;
        let mut clusters = 0;
        let mut settled = false;
        for _ in 0..cluster_sectors + 8 {
            let padding = if fat32 {
                0
            } else {
                let start = reserved_sectors + root_sectors + 2 * table_sectors;
                (cluster_sectors - start % cluster_sectors) % cluster_sectors
            };
            let first_data = reserved_sectors + padding + root_sectors + 2 * table_sectors;
            if fat32 && first_data % cluster_sectors != 0 {
                table_sectors += 1;
                continue;
            }
            let data_sectors = total_sectors
                .checked_sub(first_data)
                .ok_or(GeometryError::TooSmall)?;
            clusters = data_sectors / cluster_sectors;
            let capacity = u64::from(table_sectors) * u64::from(bytes) * 8 / bits;
            if capacity < u64::from(clusters) + 2 {
                table_sectors += 1;
                continue;
            }
            reserved_sectors += padding;
            settled = true;
            break;
        }
        if !settled || clusters == 0 {
            return Err(GeometryError::TooSmall);
        }
        let in_range = FileAllocationTableType::from_clusters(clusters) == kind
            && (!fat32 || clusters <= FAT32_MOST_CLUSTERS);
        if !in_range {
            return Err(GeometryError::WrongType(clusters));
        }
        Ok(Geometry {
            kind,
            sector_size,
            cluster_sectors,
            reserved_sectors,
            root_entries,
            root_sectors,
            table_sectors,
            total_sectors,
            clusters,
        })
    }

    /// The sector cluster 2 begins at.
    pub(crate) const fn first_data_sector(&self) -> u32 {
        self.reserved_sectors + self.root_sectors + 2 * self.table_sectors
    }

    /// Write the boot sector into `out`, which is one sector long and zeroed. `label` is the
    /// eleven bytes of `BS_VolLab`.
    pub(crate) fn write_boot_sector(
        &self,
        volume_id: u32,
        label: &[u8; 11],
        hidden_sectors: u32,
        out: &mut [u8],
    ) {
        let fat32 = self.kind == FileAllocationTableType::Fat32;
        // A jump every FAT driver expects to find, over the BPB. Nothing executes it here.
        out[0..3].copy_from_slice(&[0xEB, if fat32 { 0x58 } else { 0x3C }, 0x90]);
        out[3..11].copy_from_slice(b"MSWIN4.1"); // BS_OEMName; the spec's own recommendation
        put16(out, 11, self.sector_size.bytes() as u16); // BPB_BytsPerSec
        out[13] = self.cluster_sectors as u8; // BPB_SecPerClus
        put16(out, 14, self.reserved_sectors as u16); // BPB_RsvdSecCnt
        out[16] = 2; // BPB_NumFATs
        put16(out, 17, self.root_entries as u16); // BPB_RootEntCnt, zero on FAT32
        let small = !fat32 && self.total_sectors < 0x1_0000;
        put16(out, 19, if small { self.total_sectors as u16 } else { 0 }); // BPB_TotSec16
        out[21] = 0xF8; // BPB_Media, "fixed disk"
        put16(out, 22, if fat32 { 0 } else { self.table_sectors as u16 }); // BPB_FATSz16
        put16(out, 24, 63); // BPB_SecPerTrk, a geometry nothing has had since 2005
        put16(out, 26, 255); // BPB_NumHeads, likewise
        put32(out, 28, hidden_sectors); // BPB_HiddSec
        put32(out, 32, if small { 0 } else { self.total_sectors }); // BPB_TotSec32
        let name: &[u8; 8] = match self.kind {
            FileAllocationTableType::Fat12 => b"FAT12   ",
            FileAllocationTableType::Fat16 => b"FAT16   ",
            FileAllocationTableType::Fat32 => b"FAT32   ",
        };
        if fat32 {
            put32(out, 36, self.table_sectors); // BPB_FATSz32
            put16(out, 40, 0); // BPB_ExtFlags: both tables live, mirrored
            put16(out, 42, 0); // BPB_FSVer
            put32(out, 44, ROOT_CLUSTER); // BPB_RootClus
            put16(out, 48, 1); // BPB_FSInfo
            put16(out, 50, 6); // BPB_BkBootSec
            out[64] = 0x80; // BS_DrvNum
            out[66] = 0x29; // BS_BootSig, which makes the three fields below meaningful
            put32(out, 67, volume_id); // BS_VolID
            out[71..82].copy_from_slice(label); // BS_VolLab
            out[82..90].copy_from_slice(name); // BS_FilSysType, informational and still checked
        } else {
            out[36] = 0x80; // BS_DrvNum
            out[38] = 0x29; // BS_BootSig
            put32(out, 39, volume_id); // BS_VolID
            out[43..54].copy_from_slice(label); // BS_VolLab
            out[54..62].copy_from_slice(name); // BS_FilSysType
        }
        out[510] = 0x55;
        out[511] = 0xAA;
    }
}

/// Write `FSInfo`'s signatures and its two hints into `out`, which is one zeroed sector.
pub(crate) fn write_fs_info(free_clusters: u32, next_free: u32, out: &mut [u8]) {
    put32(out, 0, 0x4161_5252); // FSI_LeadSig, "RRaA"
    put32(out, 484, 0x6141_7272); // FSI_StrucSig, "rrAa"
    put32(out, 488, free_clusters); // FSI_Free_Count
    put32(out, 492, next_free); // FSI_Nxt_Free
    put32(out, 508, 0xAA55_0000); // FSI_TrailSig
}

/// A little-endian `u16` at `at`.
pub(crate) fn put16(out: &mut [u8], at: usize, value: u16) {
    out[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

/// A little-endian `u32` at `at`.
pub(crate) fn put32(out: &mut [u8], at: usize, value: u32) {
    out[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// **An empty volume, laid out but not written.** Every method is arithmetic; nothing here touches
/// a device until [`Format::write_to`].
///
/// # Examples
///
/// ```
/// use file_allocation_table::{Format, FormatOptions, SectorSize, NoClock, Volume};
///
/// // 8 MiB of 512-byte sectors: small enough to be FAT12.
/// let format = Format::new(FormatOptions::new(SectorSize::Bytes512, 16 * 1024), &mut NoClock).unwrap();
/// let mut boot = [0u8; 512];
/// assert!(format.fill_sector(0, &mut boot));
/// let parsed = file_allocation_table::BootSector::parse(&boot).unwrap();
/// assert_eq!(parsed.file_allocation_table_type(), format.file_allocation_table_type());
/// ```
///
/// Name: provisional. The general format beside `VolumeLayout`, which lays out one shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Format {
    geometry: Geometry,
    volume_id: u32,
    label: [u8; 11],
    has_label: bool,
    hidden_sectors: u32,
    stamp: Stamp,
}

impl Format {
    /// **Lay out the volume `options` describe**, or say why none exists. The label entry, when
    /// there is one, carries the time `clock` reports now (read once); `NoClock` gives the zero
    /// timestamp and a byte-for-byte reproducible image.
    ///
    /// Name: provisional. The constructor, as `VolumeLayout::new`.
    pub fn new(options: FormatOptions, clock: &mut impl Clock) -> Result<Format, FormatError> {
        const KINDS: [FileAllocationTableType; 3] = [
            FileAllocationTableType::Fat12,
            FileAllocationTableType::Fat16,
            FileAllocationTableType::Fat32,
        ];
        const CLUSTERS: [u32; 8] = [1, 2, 4, 8, 16, 32, 64, 128];
        if let Some(sectors) = options.cluster_sectors
            && !CLUSTERS.contains(&sectors)
        {
            return Err(FormatError::ClusterSizeRefused);
        }
        let stamp = Stamp::pack(clock.now()).map_err(|_| FormatError::TimeOutOfRange)?;
        let mut seen_small = false;
        let mut found = None;
        'search: for sectors in CLUSTERS {
            if options.cluster_sectors.is_some_and(|only| only != sectors) {
                continue;
            }
            for kind in KINDS {
                if options
                    .file_allocation_table_type
                    .is_some_and(|only| only != kind)
                {
                    continue;
                }
                match Geometry::new(kind, options.sector_size, sectors, options.total_sectors) {
                    Ok(geometry) => {
                        found = Some(geometry);
                        break 'search;
                    }
                    Err(GeometryError::TooSmall) => seen_small = true,
                    Err(GeometryError::WrongType(_)) => {}
                }
            }
        }
        let geometry = found.ok_or(if seen_small {
            FormatError::TooSmall
        } else {
            FormatError::NoSuchGeometry
        })?;
        Ok(Format {
            geometry,
            volume_id: options.volume_id,
            label: options.label,
            has_label: options.has_label,
            hidden_sectors: options.hidden_sectors,
            stamp,
        })
    }

    /// The type the volume is.
    ///
    /// Name: provisional. As `BootSector::file_allocation_table_type`.
    pub const fn file_allocation_table_type(&self) -> FileAllocationTableType {
        self.geometry.kind
    }

    /// The data clusters the volume has.
    ///
    /// Name: provisional. As `BootSector::clusters`.
    pub const fn clusters(&self) -> u32 {
        self.geometry.clusters
    }

    /// Sectors to the cluster.
    ///
    /// Name: provisional. As `BootSector::sectors_per_cluster`.
    pub const fn sectors_per_cluster(&self) -> u32 {
        self.geometry.cluster_sectors
    }

    /// Sectors in one copy of the table.
    ///
    /// Name: provisional. As `BootSector::table_sectors`.
    pub const fn table_sectors(&self) -> u32 {
        self.geometry.table_sectors
    }

    /// How many leading sectors [`Format::fill_sector`] answers for: the reserved sectors, both
    /// tables, the fixed root of FAT12 and FAT16, and FAT32's root cluster.
    ///
    /// Name: provisional. As `VolumeLayout::metadata_sectors`.
    pub const fn metadata_sectors(&self) -> u32 {
        let first = self.geometry.first_data_sector();
        match self.geometry.kind {
            FileAllocationTableType::Fat32 => first + self.geometry.cluster_sectors,
            _ => first,
        }
    }

    /// **Fill `out` with volume-relative sector `index`.** `false`, with `out` untouched, when
    /// `index` is at or past [`Format::metadata_sectors`] or `out` is not one sector long.
    ///
    /// Every sector this answers for is written whole, zeros included, so a volume laid over a
    /// disk that held something before inherits no stale table word or directory entry. The data
    /// clusters beyond are not touched.
    ///
    /// Name: provisional. As `VolumeLayout::fill_sector`.
    pub fn fill_sector(&self, index: u32, out: &mut [u8]) -> bool {
        let g = &self.geometry;
        if out.len() != g.sector_size.bytes() || index >= self.metadata_sectors() {
            return false;
        }
        out.fill(0);
        let fat32 = g.kind == FileAllocationTableType::Fat32;
        let tables = g.reserved_sectors..g.reserved_sectors + 2 * g.table_sectors;
        if index == 0 || (fat32 && index == 6) {
            g.write_boot_sector(self.volume_id, &self.label, self.hidden_sectors, out);
        } else if fat32 && (index == 1 || index == 7) {
            write_fs_info(g.clusters - 1, ROOT_CLUSTER + 1, out);
        } else if tables.contains(&index) {
            if index == g.reserved_sectors || index == g.reserved_sectors + g.table_sectors {
                self.first_table_sector(out);
            }
        } else if index == g.reserved_sectors + 2 * g.table_sectors && self.has_label {
            // The first sector of the root directory, FAT12 and FAT16's region or FAT32's cluster.
            out[..11].copy_from_slice(&self.label);
            out[11] = ATTR_VOLUME_ID;
            self.stamp.write_new(&mut out[..32]);
        }
        true
    }

    /// The first sector of a table: the media byte, the end-of-chain marker of entry 1 (which also
    /// says the volume was shut down cleanly), and on FAT32 the end of the root directory's chain.
    fn first_table_sector(&self, out: &mut [u8]) {
        match self.geometry.kind {
            FileAllocationTableType::Fat12 => out[..3].copy_from_slice(&[0xF8, 0xFF, 0xFF]),
            FileAllocationTableType::Fat16 => out[..4].copy_from_slice(&[0xF8, 0xFF, 0xFF, 0xFF]),
            FileAllocationTableType::Fat32 => {
                put32(out, 0, 0x0FFF_FFF8);
                put32(out, 4, 0x0FFF_FFFF);
                put32(out, 8, 0x0FFF_FFFF);
            }
        }
    }

    /// **Write the whole format to `device`**, sector by sector, and return the device's first
    /// error. The device must have the sector size the options named.
    ///
    /// Name: provisional. The verb of `Volume::sync`, for a whole volume.
    pub fn write_to<D: WritableSectorDevice>(&self, device: &mut D) -> Result<(), D::Error> {
        let mut sector = [0u8; SectorSize::MAX];
        let size = self.geometry.sector_size.bytes();
        for index in 0..self.metadata_sectors() {
            if self.fill_sector(index, &mut sector[..size]) {
                device.write_sector(u64::from(index), &sector[..size])?;
            }
        }
        Ok(())
    }
}

#[cfg(kani)]
mod verification;

#[cfg(test)]
mod tests;
