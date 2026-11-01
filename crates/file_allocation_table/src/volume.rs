//! **The volume**: its layout constants, the arithmetic that sizes the table and places each
//! cluster, and the sectors a caller writes. The crate root re-exports what is public here.
//!
//! Name: ratified 2026-10-25 (#2720).

use crate::boot_sector::FileAllocationTableType;
use crate::efi_system_partition::{BOOT_CLUSTER, EFI_CLUSTER, FILE_CLUSTER};
use crate::format::{Geometry, GeometryError, put16, put32, write_fs_info};
use crate::long_name::{self, LongName};
use crate::sector::SectorSize;
use crate::time::{Clock, Stamp};

/// A FAT sector, and the unit every one of this crate's outputs is measured in.
///
/// FAT32 permits 512, 1024, 2048 and 4096; this crate writes 512, which is what every disk this
/// project has met reports and what `globally_unique_identifier_partition_table` counts in.
///
/// Name: ratified 2026-10-25 (#2720). Refused `SECTOR` (it reads as a sector and holds a size).
pub const SECTOR_BYTES: usize = 512;

/// Sectors per cluster, so 4096-byte clusters.
///
/// The same number as `filesystem_contract::blk`'s transfer unit, which is not a coincidence: a
/// caller moving one blk block per request moves exactly one cluster.
///
/// Name: ratified 2026-10-25 (#2720).
pub const CLUSTER_SECTORS: u32 = 8;

/// Bytes per cluster.
///
/// Name: ratified 2026-10-25 (#2720). Refused `CLUSTER` (it reads as a cluster and holds a size).
pub const CLUSTER_BYTES: usize = CLUSTER_SECTORS as usize * SECTOR_BYTES;

/// Reserved sectors ahead of the first FAT: the boot sector, `FSInfo`, and the backup pair the
/// specification puts at sectors 6 and 7, with room left over as every tool leaves it.
///
/// Name: ratified 2026-10-25 (#2720).
pub const RESERVED_SECTORS: u32 = 32;

/// How many copies of the table are written: two, because every tool writes two and a firmware that
/// reads the second finds it.
///
/// Name: ratified 2026-10-25 (#2720). Refused `FATS` (the acronym in a crate that spells it out).
pub const TABLE_COUNT: u32 = 2;

/// The first cluster of the root directory. FAT32 fixes nothing here, but 2 is the first data
/// cluster and every tool uses it.
///
/// Name: ratified 2026-10-25 (#2720).
pub const ROOT_CLUSTER: u32 = 2;

/// **The count of data clusters below which a volume is not FAT32 at all.**
///
/// Microsoft's specification defines the FAT type by this number and nothing else, and drivers in
/// the field follow it. A "FAT32" volume with fewer clusters than this is read as FAT16 by the code
/// that matters, which is the firmware's, so this crate refuses to build one.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `FAT32_MIN_CLUSTERS` (min spelled out, as the
/// tree-wide ruling asks). Refused `MIN_CLUSTERS` (it is FAT32's floor alone).
pub const FAT32_MINIMUM_CLUSTERS: u32 = 65_525;

/// The largest cluster number FAT32 addresses. The top four bits of a 32-bit entry are reserved.
const MAXIMUM_CLUSTER: u32 = 0x0FFF_FFF5;

/// The end-of-chain marker written into the last entry of every chain.
const END_OF_CHAIN: u32 = 0x0FFF_FFFF;

/// `ATTR_DIRECTORY`.
const ATTR_DIRECTORY: u8 = 0x10;
/// `ATTR_VOLUME_ID`.
const ATTR_VOLUME_ID: u8 = 0x08;
/// `ATTR_ARCHIVE`.
const ATTR_ARCHIVE: u8 = 0x20;

/// One directory entry, in bytes.
const DIR_ENTRY: usize = 32;

/// Why a volume could not be laid out. Every one of these is a refusal before any byte is written,
/// which is the only kind of refusal worth making about a disk.
///
/// Name: ratified 2026-10-25 (#2720). Refused `Error` (the writer's refusal, beside the reader's
/// errors).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutError {
    /// **Not FAT32** (the 32-bit file allocation table, "FAT: General Overview of On-Disk Format"
    /// version 1.03, section 3, Boot Sector and BPB, which types a volume by its clusters): the
    /// partition holds fewer than [`FAT32_MINIMUM_CLUSTERS`] data clusters, so it is not a FAT32
    /// volume however its boot sector is filled in. With 4096-byte clusters that is about 256 MiB.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    #[doc(alias = "not a 32-bit file allocation table")]
    NotFat32,
    /// The partition is larger than FAT32 addresses.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    TooLarge,
    /// The file name is neither an 8.3 short name nor a long name a VFAT entry may carry: empty,
    /// over 255 units, outside the Basic Multilingual Plane, holding a control character or one of
    /// `" * / : < > ? \ |`, or ending in a space or a period.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    NameRefused,
    /// The clock reported a time FAT cannot store (see [`crate::Clock`]).
    ///
    /// Name: ratified 2026-11-01 (UTC), as `WriteError::TimeOutOfRange`.
    TimeOutOfRange,
    /// The one file the volume holds is over 4 GiB less one byte, or needs more clusters than the
    /// volume has left after its three directories. It is a refusal to lay out a volume, and not
    /// [`crate::WriteError::FileTooLarge`], which stops a write whose result would pass the 32-bit
    /// size field of a directory entry.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    FileTooBig,
}

/// **A volume, laid out but not written.** Every method is arithmetic; nothing here touches a disk.
///
/// # Examples
///
/// A 512 MiB EFI system partition at LBA 2048 holding a 9 MB boot file, written through anything
/// that can put 512 bytes at an LBA:
///
/// ```
/// use file_allocation_table::{SECTOR_BYTES, VolumeLayout, NoClock};
///
/// let volume = VolumeLayout::new(1024 * 1024, 2048, 9_000_000, 0x1234_5678, "BOOTX64.EFI", &mut NoClock)
///     .expect("512 MiB is a FAT32 volume");
///
/// let mut sector = [0u8; SECTOR_BYTES];
/// for index in 0..volume.metadata_sectors() {
///     assert!(volume.fill_sector(index, &mut sector));
///     // write `sector` at partition-relative LBA `index`
/// }
/// // then the file's own bytes, contiguous, at this LBA:
/// assert_eq!(volume.file_first_sector(), volume.metadata_sectors());
/// ```
///
/// Name: ratified 2026-10-25 (#2720). Refused `Volume` (a volume to be written, not one read).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolumeLayout {
    total_sectors: u32,
    hidden_sectors: u32,
    fat_sectors: u32,
    clusters: u32,
    file_len: u32,
    file_clusters: u32,
    geometry: Geometry,
    stamp: Stamp,
    volume_id: u32,
    name: [u8; 11],
    /// The long name written ahead of `name`, which is then its alias; `None` for an 8.3 name.
    long_name: Option<LongName>,
}

impl VolumeLayout {
    /// **Lay out a FAT32 volume of `total_sectors` holding one file of `file_len` bytes** at
    /// `EFI\BOOT\<file_name>`.
    ///
    /// `hidden_sectors` is the partition's own first LBA on the disk, which goes into
    /// `BPB_HiddSec`. It is a field some boot code reads and nothing here does; passing the wrong
    /// value does not move a single byte this crate places, because every other offset is
    /// partition-relative.
    ///
    /// `volume_id` is the serial number, and a caller with an entropy capability should draw it;
    /// nothing depends on it being unique and this crate will not invent one.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub fn new(
        total_sectors: u64,
        hidden_sectors: u64,
        file_len: u64,
        volume_id: u32,
        file_name: &str,
        clock: &mut impl Clock,
    ) -> Result<VolumeLayout, LayoutError> {
        if total_sectors > u32::MAX as u64 || hidden_sectors > u32::MAX as u64 {
            return Err(LayoutError::TooLarge);
        }
        let (name, long_name) = match short_name(file_name) {
            Some(name) => (name, None),
            None => {
                let long = LongName::new(file_name).ok_or(LayoutError::NameRefused)?;
                (long.alias().ok_or(LayoutError::NameRefused)?, Some(long))
            }
        };
        let total_sectors = total_sectors as u32;

        let stamp = Stamp::pack(clock.now()).map_err(|_| LayoutError::TimeOutOfRange)?;
        let geometry = Geometry::new(
            FileAllocationTableType::Fat32,
            SectorSize::Bytes512,
            CLUSTER_SECTORS,
            total_sectors,
        )
        .map_err(|error| match error {
            GeometryError::WrongType(clusters) if clusters > MAXIMUM_CLUSTER - 2 => {
                LayoutError::TooLarge
            }
            _ => LayoutError::NotFat32,
        })?;
        let (fat_sectors, clusters) = (geometry.table_sectors, geometry.clusters);

        if file_len > u32::MAX as u64 {
            return Err(LayoutError::FileTooBig);
        }
        let file_len = file_len as u32;
        let file_clusters = (file_len as u64).div_ceil(CLUSTER_BYTES as u64) as u32;
        // Clusters are numbered 2..clusters+2, and 2, 3 and 4 are the three directories.
        if file_clusters > clusters.saturating_sub(FILE_CLUSTER - ROOT_CLUSTER) {
            return Err(LayoutError::FileTooBig);
        }

        Ok(VolumeLayout {
            total_sectors,
            hidden_sectors: hidden_sectors as u32,
            fat_sectors,
            clusters,
            file_len,
            file_clusters,
            geometry,
            stamp,
            volume_id,
            name,
            long_name,
        })
    }

    /// How many data clusters the volume has. Below [`FAT32_MINIMUM_CLUSTERS`]
    /// [`VolumeLayout::new`] refuses.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn clusters(&self) -> u32 {
        self.clusters
    }

    /// Sectors in one copy of the table.
    ///
    /// Name: ratified 2026-10-25 (#2720). Refused `fat_sectors` (the reader's spelling for the same
    /// quantity).
    pub const fn table_sectors(&self) -> u32 {
        self.fat_sectors
    }

    /// The partition-relative sector where cluster 2 begins.
    ///
    /// Name: ratified 2026-10-25 (#2720). Refused `data_first_sector` (the specification's order).
    pub const fn first_data_sector(&self) -> u64 {
        (RESERVED_SECTORS + TABLE_COUNT * self.fat_sectors) as u64
    }

    /// The partition-relative sector where `cluster` begins.
    ///
    /// `cluster` must be at least [`ROOT_CLUSTER`] (2); clusters 0 and 1 are the FAT's own
    /// reserved entries and have no sectors, and the subtraction below underflows on them (a panic
    /// in a debug build, a wrap in release). Every caller in this tree passes a constant from 2
    /// up; noted by the 2026-09-24 security audit rather than made checked, because a `const fn`
    /// returning an `Option` would push a `?` into a writer whose inputs are all its own.
    ///
    /// Name: ratified 2026-10-25 (#2720). Refused `cluster_sector` (the reader's spelling for the
    /// same quantity).
    pub const fn cluster_first_sector(&self, cluster: u32) -> u64 {
        self.first_data_sector() + (cluster - ROOT_CLUSTER) as u64 * CLUSTER_SECTORS as u64
    }

    /// **How many leading sectors the caller must write from [`VolumeLayout::fill_sector`]**: everything from
    /// the boot sector through the last directory cluster. The boot file's own bytes go at
    /// [`VolumeLayout::file_first_sector`], which is the next sector after these.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn metadata_sectors(&self) -> u64 {
        self.cluster_first_sector(FILE_CLUSTER)
    }

    /// The partition-relative sector the boot file's first byte lands on. Its data is contiguous
    /// from here.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn file_first_sector(&self) -> u64 {
        self.cluster_first_sector(FILE_CLUSTER)
    }

    /// How many sectors the boot file occupies, rounded up to whole clusters. The tail of the last
    /// cluster is never read, and this crate does not require the caller to zero it.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn file_sectors(&self) -> u64 {
        self.file_clusters as u64 * CLUSTER_SECTORS as u64
    }

    /// **Fill `out` with partition-relative sector `index`.**
    ///
    /// `false` when `index` is at or past [`VolumeLayout::metadata_sectors`] (the caller's own file bytes
    /// begin there) or when `out` is not [`SECTOR_BYTES`] bytes; in neither case is `out` touched.
    ///
    /// Every sector this answers for is written whole, zeros included, so that a volume laid over a
    /// disk that held something before does not inherit a single stale directory entry or table
    /// word.
    ///
    /// Name: ratified 2026-10-25 (#2720). Refused `sector` (it fills a buffer and reads as a
    /// getter).
    pub fn fill_sector(&self, index: u64, out: &mut [u8]) -> bool {
        if out.len() != SECTOR_BYTES || index >= self.metadata_sectors() {
            return false;
        }
        out.fill(0);
        let fat_start = RESERVED_SECTORS as u64;
        let fat_end = fat_start + (TABLE_COUNT * self.fat_sectors) as u64;
        match index {
            0 => self.boot_sector(out),
            1 => self.file_system_information(out),
            // The backup pair, which the specification puts at `BPB_BkBootSec` and the sector
            // after it. A firmware that finds the primary damaged reads these.
            6 => self.boot_sector(out),
            7 => self.file_system_information(out),
            i if (fat_start..fat_end).contains(&i) => {
                // Both copies are the same bytes, so the offset inside one copy is all that matters.
                let within = (i - fat_start) % self.fat_sectors as u64;
                self.fat_sector(within as u32, out);
            }
            i if i == self.cluster_first_sector(ROOT_CLUSTER) => self.root_cluster(out),
            i if i == self.cluster_first_sector(EFI_CLUSTER) => self.efi_cluster(out),
            i if (self.cluster_first_sector(BOOT_CLUSTER)
                ..self.cluster_first_sector(FILE_CLUSTER))
                .contains(&i) =>
            {
                self.boot_dir_sector(i - self.cluster_first_sector(BOOT_CLUSTER), out);
            }
            // Reserved sectors with nothing in them, and the second and later sectors of the root
            // and `EFI`: zero, which is what `out.fill(0)` above already made them.
            _ => {}
        }
        true
    }

    /// The BPB and the boot signature, written by the general format's builder.
    fn boot_sector(&self, out: &mut [u8]) {
        self.geometry
            .write_boot_sector(self.volume_id, b"NIFE       ", self.hidden_sectors, out);
    }

    /// `FSInfo`, whose two counts are hints a driver is allowed to ignore or disbelieve.
    fn file_system_information(&self, out: &mut [u8]) {
        // Three directories and the file. Written once; see this crate's BUGS.
        let used = (FILE_CLUSTER - ROOT_CLUSTER) + self.file_clusters;
        write_fs_info(self.clusters - used, FILE_CLUSTER + self.file_clusters, out);
    }

    /// One sector of the table, `within` sectors into a copy of it.
    ///
    /// The whole table is written, so the free clusters are explicitly zero rather than whatever the
    /// disk held. On a 512 MiB volume that is a megabyte of writes and it is not optional: a stale
    /// word in a table the firmware reads is a chain into somebody's old data.
    fn fat_sector(&self, within: u32, out: &mut [u8]) {
        let per_sector = (SECTOR_BYTES / 4) as u32;
        let first = within * per_sector;
        for slot in 0..per_sector {
            let cluster = first + slot;
            let entry = match cluster {
                // Entry 0 carries the media byte in its low eight bits, entry 1 an end-of-chain
                // marker. Both are conventions rather than allocations; cluster numbering starts
                // at 2 for exactly this reason.
                0 => 0x0FFF_FFF8,
                1 => END_OF_CHAIN,
                // The three directories are one cluster each.
                ROOT_CLUSTER | EFI_CLUSTER | BOOT_CLUSTER => END_OF_CHAIN,
                c if c >= FILE_CLUSTER && c < FILE_CLUSTER + self.file_clusters => {
                    if c + 1 == FILE_CLUSTER + self.file_clusters {
                        END_OF_CHAIN
                    } else {
                        c + 1
                    }
                }
                _ => 0, // free, or past the last cluster this volume has
            };
            put32(out, slot as usize * 4, entry);
        }
    }

    /// The root directory: the volume label, then `EFI`.
    fn root_cluster(&self, out: &mut [u8]) {
        dir_entry(out, 0, b"NIFE       ", ATTR_VOLUME_ID, 0, 0, &self.stamp);
        dir_entry(
            out,
            1,
            b"EFI        ",
            ATTR_DIRECTORY,
            EFI_CLUSTER,
            0,
            &self.stamp,
        );
    }

    /// `EFI`: the two dot entries the specification requires of every non-root directory, then
    /// `BOOT`.
    fn efi_cluster(&self, out: &mut [u8]) {
        dir_entry(
            out,
            0,
            b".          ",
            ATTR_DIRECTORY,
            EFI_CLUSTER,
            0,
            &self.stamp,
        );
        // `..` of a directory whose parent is the root names cluster **zero**, not the root's own
        // cluster number. The specification is explicit and drivers check it.
        dir_entry(out, 1, b"..         ", ATTR_DIRECTORY, 0, 0, &self.stamp);
        dir_entry(
            out,
            2,
            b"BOOT       ",
            ATTR_DIRECTORY,
            BOOT_CLUSTER,
            0,
            &self.stamp,
        );
    }

    /// Sector `within` of `EFI\BOOT`: the dot entries, the long-name run if the file has one,
    /// then the file's short entry.
    ///
    /// A 255-unit name is twenty long-name entries, so with the dot entries and the short entry
    /// the directory is 23 slots and reaches into its second sector. Every slot past those is
    /// zero, which a driver reads as the end of the directory.
    fn boot_dir_sector(&self, within: u64, out: &mut [u8]) {
        let per_sector = SECTOR_BYTES / DIR_ENTRY;
        let first = within as usize * per_sector;
        let run = self.long_name.map_or(0, |n| n.entries());
        for at in 0..per_sector {
            match first + at {
                0 => dir_entry(
                    out,
                    at,
                    b".          ",
                    ATTR_DIRECTORY,
                    BOOT_CLUSTER,
                    0,
                    &self.stamp,
                ),
                1 => dir_entry(
                    out,
                    at,
                    b"..         ",
                    ATTR_DIRECTORY,
                    EFI_CLUSTER,
                    0,
                    &self.stamp,
                ),
                slot if slot < 2 + run => {
                    if let Some(long) = &self.long_name {
                        let entry = &mut out[at * DIR_ENTRY..(at + 1) * DIR_ENTRY];
                        long.entry(slot - 2, long_name::checksum(&self.name), entry);
                    }
                }
                slot if slot == 2 + run => dir_entry(
                    out,
                    at,
                    &self.name,
                    ATTR_ARCHIVE,
                    FILE_CLUSTER,
                    self.file_len,
                    &self.stamp,
                ),
                _ => {}
            }
        }
    }
}

/// Write one 32-byte directory entry at slot `slot` of a directory cluster's first sector.
fn dir_entry(
    out: &mut [u8],
    slot: usize,
    name: &[u8; 11],
    attr: u8,
    cluster: u32,
    size: u32,
    stamp: &Stamp,
) {
    let at = slot * DIR_ENTRY;
    out[at..at + 11].copy_from_slice(name);
    out[at + 11] = attr;
    // 12..20: reserved, creation time and tenths, last access date. All zero; see BUGS.
    put16(out, at + 20, (cluster >> 16) as u16); // DIR_FstClusHI
    // 22..26: write time and date, zero.
    put16(out, at + 26, cluster as u16); // DIR_FstClusLO
    put32(out, at + 28, size); // DIR_FileSize, zero for a directory
    stamp.write_new(&mut out[at..at + DIR_ENTRY]);
}

/// **Turn `name` into the eleven-byte short form**, or `None` when it needs a long name.
///
/// Strict: upper-case letters, digits and the short form's punctuation, at most eight before the
/// dot and three after. Anything else, lower case included, goes to [`LongName`], so the name a
/// driver shows is always the name asked for.
fn short_name(name: &str) -> Option<[u8; 11]> {
    let bytes = name.as_bytes();
    let dot = bytes.iter().position(|b| *b == b'.');
    let (base, ext): (&[u8], &[u8]) = match dot {
        Some(i) => (&bytes[..i], &bytes[i + 1..]),
        None => (bytes, b""),
    };
    if base.is_empty() || base.len() > 8 || ext.len() > 3 {
        return None;
    }
    let ok = |b: &u8| *b != b' ' && long_name::is_short_byte(*b);
    if !base.iter().all(ok) || !ext.iter().all(ok) {
        return None;
    }
    let mut out = [b' '; 11];
    out[..base.len()].copy_from_slice(base);
    out[8..8 + ext.len()].copy_from_slice(ext);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoClock;

    /// 512 MiB in 512-byte sectors, the size the installer lays out.
    const ESP_SECTORS: u64 = 1024 * 1024;

    fn volume() -> VolumeLayout {
        VolumeLayout::new(
            ESP_SECTORS,
            2048,
            9_000_000,
            0xDEAD_BEEF,
            "BOOTX64.EFI",
            &mut NoClock,
        )
        .expect("a FAT32 volume")
    }

    fn sector(v: &VolumeLayout, index: u64) -> [u8; SECTOR_BYTES] {
        let mut out = [0u8; SECTOR_BYTES];
        assert!(v.fill_sector(index, &mut out), "sector {index} is metadata");
        out
    }

    #[test]
    fn a_512_mib_volume_has_enough_clusters_to_be_fat32() {
        let v = volume();
        assert!(
            v.clusters() >= FAT32_MINIMUM_CLUSTERS,
            "{} clusters",
            v.clusters()
        );
        // The table has to hold an entry for every cluster plus the two reserved ones.
        assert!(v.table_sectors() as u64 * (SECTOR_BYTES as u64 / 4) >= v.clusters() as u64 + 2);
    }

    /// The smallest volume this crate will build, and the boundary it refuses below. With 4096-byte
    /// clusters, 65525 of them is a little over 256 MiB, which is why the EFI system partition the
    /// installer lays out is not the 100 MiB a reader might expect.
    #[test]
    fn a_volume_too_small_to_be_fat32_is_refused_rather_than_mislabelled() {
        assert_eq!(
            VolumeLayout::new(200 * 1024 * 2, 2048, 1024, 1, "BOOTX64.EFI", &mut NoClock),
            Err(LayoutError::NotFat32)
        );
        let just_enough =
            VolumeLayout::new(600 * 1024 * 2, 2048, 1024, 1, "BOOTX64.EFI", &mut NoClock)
                .expect("300 MiB clears the cluster floor");
        assert!(just_enough.clusters() >= FAT32_MINIMUM_CLUSTERS);
    }

    #[test]
    fn the_boot_sector_carries_the_fields_at_the_offsets_the_specification_states() {
        let v = volume();
        let s = sector(&v, 0);
        assert_eq!(&s[0..3], &[0xEB, 0x58, 0x90]);
        assert_eq!(u16::from_le_bytes([s[11], s[12]]), 512); // BPB_BytsPerSec
        assert_eq!(s[13], 8); // BPB_SecPerClus
        assert_eq!(u16::from_le_bytes([s[14], s[15]]), 32); // BPB_RsvdSecCnt
        assert_eq!(s[16], 2); // BPB_NumFATs
        assert_eq!(u16::from_le_bytes([s[17], s[18]]), 0); // BPB_RootEntCnt
        assert_eq!(s[21], 0xF8); // BPB_Media
        assert_eq!(u16::from_le_bytes([s[22], s[23]]), 0); // BPB_FATSz16
        assert_eq!(u32::from_le_bytes([s[28], s[29], s[30], s[31]]), 2048); // BPB_HiddSec
        assert_eq!(
            u32::from_le_bytes([s[32], s[33], s[34], s[35]]),
            ESP_SECTORS as u32
        ); // BPB_TotSec32
        assert_eq!(
            u32::from_le_bytes([s[36], s[37], s[38], s[39]]),
            v.table_sectors()
        ); // BPB_FATSz32
        assert_eq!(u32::from_le_bytes([s[44], s[45], s[46], s[47]]), 2); // BPB_RootClus
        assert_eq!(u16::from_le_bytes([s[48], s[49]]), 1); // BPB_FSInfo
        assert_eq!(u16::from_le_bytes([s[50], s[51]]), 6); // BPB_BkBootSec
        assert_eq!(s[66], 0x29); // BS_BootSig
        assert_eq!(&s[82..90], b"FAT32   ");
        assert_eq!([s[510], s[511]], [0x55, 0xAA]);
    }

    /// The backup the specification puts at sector 6 is the same bytes, which is the whole point of
    /// it: a firmware that finds sector 0 damaged reads this one instead.
    #[test]
    fn the_backup_boot_sector_is_a_copy() {
        let v = volume();
        assert_eq!(sector(&v, 0), sector(&v, 6));
        assert_eq!(sector(&v, 1), sector(&v, 7));
    }

    #[test]
    fn fs_info_carries_both_signatures_and_the_trailer() {
        let v = volume();
        let s = sector(&v, 1);
        assert_eq!(u32::from_le_bytes([s[0], s[1], s[2], s[3]]), 0x4161_5252);
        assert_eq!(
            u32::from_le_bytes([s[484], s[485], s[486], s[487]]),
            0x6141_7272
        );
        assert_eq!(
            u32::from_le_bytes([s[508], s[509], s[510], s[511]]),
            0xAA55_0000
        );
    }

    /// The two copies of the table must be byte-identical, because `BPB_ExtFlags` says they are
    /// mirrored and a driver is free to read either.
    #[test]
    fn both_copies_of_the_table_are_the_same_bytes() {
        let v = volume();
        let first = RESERVED_SECTORS as u64;
        let second = first + v.table_sectors() as u64;
        for offset in [0, 1, 7, v.table_sectors() as u64 - 1] {
            assert_eq!(sector(&v, first + offset), sector(&v, second + offset));
        }
    }

    /// The file's chain, read out of the table the way a driver would walk it: start at
    /// [`FILE_CLUSTER`], follow each entry, and stop at the end-of-chain marker. What it proves is
    /// that the chain is exactly as long as the file needs and that it is contiguous, which is what
    /// lets the caller write the file's bytes as one run.
    #[test]
    fn the_files_chain_walks_from_its_first_cluster_to_the_end_of_chain() {
        let v = volume();
        let entry = |cluster: u32| -> u32 {
            let per_sector = (SECTOR_BYTES / 4) as u32;
            let s = sector(&v, RESERVED_SECTORS as u64 + (cluster / per_sector) as u64);
            let at = (cluster % per_sector) as usize * 4;
            u32::from_le_bytes([s[at], s[at + 1], s[at + 2], s[at + 3]])
        };
        assert_eq!(entry(0), 0x0FFF_FFF8);
        assert_eq!(entry(1), END_OF_CHAIN);
        for directory in [ROOT_CLUSTER, EFI_CLUSTER, BOOT_CLUSTER] {
            assert_eq!(entry(directory), END_OF_CHAIN);
        }

        let mut cluster = FILE_CLUSTER;
        let mut walked = 1;
        while entry(cluster) != END_OF_CHAIN {
            assert_eq!(entry(cluster), cluster + 1, "the chain is contiguous");
            cluster = entry(cluster);
            walked += 1;
            assert!(walked <= 10_000, "the chain does not terminate");
        }
        assert_eq!(walked, 9_000_000u64.div_ceil(CLUSTER_BYTES as u64));
        // And the cluster after the chain is free, so nothing reads past the file.
        assert_eq!(entry(cluster + 1), 0);
    }

    /// Walk `\EFI\BOOT\BOOTX64.EFI` the way a firmware does: root cluster, then the directory each
    /// entry names, checking the dot entries on the way because a driver that validates them will
    /// refuse a volume that gets them wrong.
    #[test]
    fn the_boot_file_is_reachable_by_walking_the_directories() {
        let v = volume();
        let entry_at = |cluster: u32, slot: usize| -> ([u8; 11], u8, u32, u32) {
            let s = sector(&v, v.cluster_first_sector(cluster));
            let at = slot * DIR_ENTRY;
            let mut name = [0u8; 11];
            name.copy_from_slice(&s[at..at + 11]);
            let first = u32::from(u16::from_le_bytes([s[at + 26], s[at + 27]]))
                | u32::from(u16::from_le_bytes([s[at + 20], s[at + 21]])) << 16;
            let size = u32::from_le_bytes([s[at + 28], s[at + 29], s[at + 30], s[at + 31]]);
            (name, s[at + 11], first, size)
        };

        let (label, attr, _, _) = entry_at(ROOT_CLUSTER, 0);
        assert_eq!(&label, b"NIFE       ");
        assert_eq!(attr, ATTR_VOLUME_ID);

        let (name, attr, cluster, _) = entry_at(ROOT_CLUSTER, 1);
        assert_eq!(&name, b"EFI        ");
        assert_eq!(attr, ATTR_DIRECTORY);
        assert_eq!(cluster, EFI_CLUSTER);

        assert_eq!(entry_at(EFI_CLUSTER, 0).0, *b".          ");
        assert_eq!(entry_at(EFI_CLUSTER, 1).0, *b"..         ");
        // `..` out of a child of the root is cluster zero, not the root's own number.
        assert_eq!(entry_at(EFI_CLUSTER, 1).2, 0);
        let (name, attr, cluster, _) = entry_at(EFI_CLUSTER, 2);
        assert_eq!(&name, b"BOOT       ");
        assert_eq!(attr, ATTR_DIRECTORY);
        assert_eq!(cluster, BOOT_CLUSTER);

        assert_eq!(entry_at(BOOT_CLUSTER, 1).2, EFI_CLUSTER);
        let (name, attr, cluster, size) = entry_at(BOOT_CLUSTER, 2);
        assert_eq!(&name, b"BOOTX64 EFI");
        assert_eq!(attr, ATTR_ARCHIVE);
        assert_eq!(cluster, FILE_CLUSTER);
        assert_eq!(size, 9_000_000);
    }

    /// **Every cluster begins on a 4096-byte boundary of the partition**, which is what lets a
    /// caller moving one `filesystem_contract::blk` block per request write the volume at all. It
    /// is not free: FAT32's own sizing arithmetic leaves the data area wherever it falls, and the
    /// first installed disk this crate produced had its boot file six sectors early because of it.
    #[test]
    fn the_data_area_begins_on_a_cluster_boundary() {
        // A range of sizes, because the alignment depends on the table's length and the table's
        // length depends on the size.
        for mib in [300u64, 301, 400, 512, 700, 1024, 4096] {
            let sectors = mib * 1024 * 2;
            let v = VolumeLayout::new(sectors, 2048, 9_000_000, 1, "BOOTX64.EFI", &mut NoClock)
                .unwrap_or_else(|e| panic!("{mib} MiB: {e:?}"));
            assert_eq!(
                v.first_data_sector() % u64::from(CLUSTER_SECTORS),
                0,
                "{mib} MiB: data area at sector {}",
                v.first_data_sector()
            );
            assert_eq!(v.file_first_sector() % u64::from(CLUSTER_SECTORS), 0);
            // And the table still covers every cluster the volume has.
            assert!(
                v.table_sectors() as u64 * (SECTOR_BYTES as u64 / 4) >= v.clusters() as u64 + 2
            );
        }
    }

    #[test]
    fn the_files_bytes_begin_where_the_metadata_ends() {
        let v = volume();
        assert_eq!(v.file_first_sector(), v.metadata_sectors());
        assert_eq!(v.file_first_sector(), v.cluster_first_sector(FILE_CLUSTER));
        let mut out = [0u8; SECTOR_BYTES];
        assert!(
            !v.fill_sector(v.file_first_sector(), &mut out),
            "not metadata"
        );
        assert!(
            !v.fill_sector(0, &mut out[..100]),
            "a short buffer is refused"
        );
    }

    #[test]
    fn an_8_dot_3_name_is_short_and_anything_else_is_long() {
        assert_eq!(short_name("BOOTX64.EFI"), Some(*b"BOOTX64 EFI"));
        assert_eq!(short_name("BOOTAA64.EFI"), Some(*b"BOOTAA64EFI"));
        for long in ["BOOTRISCV64.EFI", "bootx64.efi", "", "A.LONGEXT"] {
            assert_eq!(short_name(long), None, "{long}");
        }
        assert_eq!(
            VolumeLayout::new(ESP_SECTORS, 2048, 1024, 1, "a:b", &mut NoClock),
            Err(LayoutError::NameRefused)
        );
        assert_eq!(
            VolumeLayout::new(ESP_SECTORS, 2048, 1024, 1, "", &mut NoClock),
            Err(LayoutError::NameRefused)
        );
    }

    /// The slots of `EFI\BOOT`, as 32-byte entries read out of its first two sectors.
    fn boot_dir_slots(v: &VolumeLayout) -> [[u8; DIR_ENTRY]; 2 * SECTOR_BYTES / DIR_ENTRY] {
        let first = v.cluster_first_sector(BOOT_CLUSTER);
        let mut slots = [[0; DIR_ENTRY]; 2 * SECTOR_BYTES / DIR_ENTRY];
        let both = [sector(v, first), sector(v, first + 1)];
        let entries = both.iter().flat_map(|s| s.as_chunks::<DIR_ENTRY>().0);
        for (slot, entry) in slots.iter_mut().zip(entries) {
            *slot = *entry;
        }
        slots
    }

    /// **The riscv64 removable-media name**: two long-name entries between the dot entries and
    /// the `BOOTRI~1.EFI` short entry, each stamped with the checksum of that short entry's name
    /// bytes as they sit on disk, which is the binding a driver checks.
    #[test]
    fn bootriscv64_efi_is_written_as_a_long_name_run_ahead_of_its_alias() {
        let v = VolumeLayout::new(
            ESP_SECTORS,
            2048,
            9_000_000,
            1,
            "BOOTRISCV64.EFI",
            &mut NoClock,
        )
        .expect("a volume");
        let slots = boot_dir_slots(&v);
        assert_eq!(&slots[0][..11], b".          ");
        assert_eq!(&slots[1][..11], b"..         ");
        assert_eq!([slots[2][0], slots[3][0]], [0x42, 0x01]);
        let short = &slots[4];
        assert_eq!(&short[..11], b"BOOTRI~1EFI");
        assert_eq!(short[11], ATTR_ARCHIVE);
        assert_eq!(
            u32::from_le_bytes(short[28..32].try_into().unwrap()),
            9_000_000
        );
        let sum = long_name::checksum(short[..11].try_into().unwrap());
        for entry in &slots[2..4] {
            assert_eq!(entry[11], long_name::ATTR_LONG_NAME);
            assert_eq!(entry[13], sum);
        }
        assert_eq!(
            slots[5], [0; DIR_ENTRY],
            "the directory ends after the file"
        );
    }

    /// A 255-unit name is twenty entries, so the directory crosses into its second sector and
    /// the file's short entry is slot 22. Only the first two sectors hold anything.
    #[test]
    fn the_longest_name_reaches_the_second_sector_of_the_boot_directory() {
        let name = "n".repeat(251) + ".efi";
        let v =
            VolumeLayout::new(ESP_SECTORS, 2048, 1024, 1, &name, &mut NoClock).expect("a volume");
        let slots = boot_dir_slots(&v);
        assert_eq!(slots[2][0], 0x40 | 20);
        assert_eq!(slots[21][0], 1);
        assert_eq!(&slots[22][..11], b"NNNNNN~1EFI");
        assert_eq!(slots[23], [0; DIR_ENTRY]);
        let third = v.cluster_first_sector(BOOT_CLUSTER) + 2;
        assert_eq!(sector(&v, third), [0; SECTOR_BYTES]);
    }

    #[test]
    fn a_file_larger_than_the_volume_is_refused() {
        assert_eq!(
            VolumeLayout::new(
                ESP_SECTORS,
                2048,
                600 * 1024 * 1024,
                1,
                "BOOTX64.EFI",
                &mut NoClock
            ),
            Err(LayoutError::FileTooBig)
        );
    }

    /// **The sector arithmetic, pinned by hand**: the data area starts after the reserved sectors
    /// and both copies of the table, a cluster is eight sectors, and cluster 2 is the first.
    #[test]
    fn the_data_area_and_its_clusters_sit_where_the_arithmetic_says() {
        let v = volume();
        let first = 32 + 2 * v.table_sectors() as u64;
        assert_eq!(v.first_data_sector(), first);
        assert_eq!(v.cluster_first_sector(2), first);
        assert_eq!(v.cluster_first_sector(3), first + 8);
        assert_eq!(v.cluster_first_sector(5), first + 24);
        assert_eq!(v.file_first_sector(), first + 24);
        assert_eq!(v.metadata_sectors(), first + 24);
        // 9,000,000 bytes is 2,198 clusters of 4,096, each of eight sectors.
        assert_eq!(v.file_sectors(), 2198 * 8);
    }

    /// `FSInfo`'s two hints count the three directories and the file: free is what is left, and the
    /// next free cluster is the one after the file.
    #[test]
    fn fs_info_counts_the_directories_and_the_file() {
        let v = volume();
        let s = sector(&v, 1);
        let word = |at: usize| u32::from_le_bytes([s[at], s[at + 1], s[at + 2], s[at + 3]]);
        assert_eq!(word(488), v.clusters() - (3 + 2198));
        assert_eq!(word(492), 5 + 2198);
    }

    /// Each directory cluster's first sector is its own and the sectors between are zero. A sector
    /// that nothing owns is not handed a directory by default.
    #[test]
    fn only_the_three_directory_clusters_have_contents_and_their_later_sectors_are_zero() {
        let v = volume();
        let at = |c| v.cluster_first_sector(c);
        for c in [2, 3, 4] {
            assert_ne!(sector(&v, at(c)), [0; SECTOR_BYTES], "cluster {c}");
            assert_eq!(sector(&v, at(c) + 1), [0; SECTOR_BYTES], "cluster {c}");
        }
        // The reserved sectors between the backup pair and the table.
        for i in [2, 3, 4, 5, 8, 31] {
            assert_eq!(sector(&v, i), [0; SECTOR_BYTES], "sector {i}");
        }
        // The table is not a directory: its second sector is table words, not the boot directory.
        assert_ne!(sector(&v, 33), sector(&v, at(4)));
    }

    /// A directory entry's first cluster is split into high and low halves.
    #[test]
    fn a_directory_entry_splits_its_first_cluster_into_halves() {
        let mut out = [0u8; SECTOR_BYTES];
        dir_entry(
            &mut out,
            1,
            b"NAME       ",
            ATTR_ARCHIVE,
            0x0001_0002,
            77,
            &Stamp::ZERO,
        );
        assert_eq!(&out[32..43], b"NAME       ");
        assert_eq!(u16::from_le_bytes([out[32 + 20], out[32 + 21]]), 1);
        assert_eq!(u16::from_le_bytes([out[32 + 26], out[32 + 27]]), 2);
        assert_eq!(u32::from_le_bytes(out[60..64].try_into().unwrap()), 77);
    }

    /// Each rule that makes a name not short sends it to a long name alone, and the longest short
    /// name is kept.
    #[test]
    fn a_name_is_long_for_each_reason_and_the_longest_short_name_is_kept() {
        assert_eq!(short_name("ABCDEFGH.XYZ").unwrap(), *b"ABCDEFGHXYZ");
        assert_eq!(short_name("A").unwrap(), *b"A          ");
        for long in [
            ".EFI",
            "ABCDEFGHI.EFI",
            "ABC.WXYZ",
            "abc.efi",
            "AB CD",
            "A+B",
        ] {
            assert_eq!(short_name(long), None, "{long}");
        }
    }
}
