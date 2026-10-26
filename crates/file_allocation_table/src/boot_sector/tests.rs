//! Boot sectors that another tool formatted, the `mkfs` in this crate read back, and one refusal
//! per checked field with the boundary on each side of it.

use super::*;
use crate::{SECTOR_BYTES, VolumeLayout};

const FAT12_FLOPPY: &[u8; 512] = include_bytes!("../../tests/fixtures/fat12-floppy.boot");
const FAT16_32M: &[u8; 512] = include_bytes!("../../tests/fixtures/fat16-32m.boot");
const FAT16_4K: &[u8; 512] = include_bytes!("../../tests/fixtures/fat16-4k.boot");
const FAT32_64M: &[u8; 512] = include_bytes!("../../tests/fixtures/fat32-64m.boot");
const FAT32_4K: &[u8; 512] = include_bytes!("../../tests/fixtures/fat32-4k.boot");
const FAT32_UNDER_FLOOR: &[u8; 512] = include_bytes!("../../tests/fixtures/fat32-under-floor.boot");

/// The shape of a synthetic boot sector, field by field.
#[derive(Clone, Copy)]
struct Shape {
    bytes_per_sector: u16,
    per_cluster: u8,
    reserved: u16,
    tables: u8,
    root_entries: u16,
    total: u32,
    table_sectors: u32,
    fat32: bool,
}

impl Shape {
    /// A FAT16 layout: one reserved sector, two tables, a 512-entry root.
    const fn fat16(total: u32, table_sectors: u32) -> Shape {
        Shape {
            bytes_per_sector: 512,
            per_cluster: 1,
            reserved: 1,
            tables: 2,
            root_entries: 512,
            total,
            table_sectors,
            fat32: false,
        }
    }

    /// A FAT32 layout: 32 reserved sectors, two tables, the root at cluster 2.
    const fn fat32(total: u32, table_sectors: u32) -> Shape {
        Shape {
            bytes_per_sector: 512,
            per_cluster: 1,
            reserved: 32,
            tables: 2,
            root_entries: 0,
            total,
            table_sectors,
            fat32: true,
        }
    }

    fn bytes(self) -> [u8; 512] {
        let mut s = [0u8; 512];
        s[0..3].copy_from_slice(&[0xEB, 0x3C, 0x90]);
        s[11..13].copy_from_slice(&self.bytes_per_sector.to_le_bytes());
        s[13] = self.per_cluster;
        s[14..16].copy_from_slice(&self.reserved.to_le_bytes());
        s[16] = self.tables;
        s[17..19].copy_from_slice(&self.root_entries.to_le_bytes());
        s[21] = 0xF8;
        s[32..36].copy_from_slice(&self.total.to_le_bytes());
        if self.fat32 {
            s[36..40].copy_from_slice(&self.table_sectors.to_le_bytes());
            s[44..48].copy_from_slice(&2u32.to_le_bytes());
            s[48..50].copy_from_slice(&1u16.to_le_bytes());
            s[50..52].copy_from_slice(&6u16.to_le_bytes());
        } else {
            let small = u16::try_from(self.table_sectors).expect("a FAT16 table size");
            s[22..24].copy_from_slice(&small.to_le_bytes());
        }
        s[510] = 0x55;
        s[511] = 0xAA;
        s
    }
}

fn parse(sector: &[u8]) -> Result<BootSector, ParseError> {
    BootSector::parse(sector)
}

/// `sector` with the bytes at `at` replaced by `with`.
fn with(sector: &[u8; 512], at: usize, bytes: &[u8]) -> [u8; 512] {
    let mut out = *sector;
    out[at..at + bytes.len()].copy_from_slice(bytes);
    out
}

#[test]
fn a_floppy_mkfs_fat_formatted_is_fat12_with_its_root_region() {
    let boot = parse(FAT12_FLOPPY).unwrap();
    assert_eq!(boot.fat_type(), FatType::Fat12);
    assert_eq!(boot.sector_size(), SectorSize::Bytes512);
    assert_eq!(boot.clusters(), 2847);
    assert_eq!(boot.total_sectors(), 2880);
    assert_eq!(boot.reserved_sectors(), 1);
    assert_eq!(boot.table_count(), 2);
    assert_eq!(boot.table_sectors(), 9);
    assert_eq!(boot.media(), 0xF0);
    assert_eq!(
        boot.root_directory(),
        RootDirectory::Region {
            first_sector: 19,
            sectors: 14,
            entries: 224
        }
    );
    assert_eq!(boot.first_data_sector(), 33);
    assert_eq!(boot.volume_id(), Some(0x1234_ABCD));
    assert_eq!(boot.active_table(), None);
    assert_eq!(boot.fs_info_sector(), None);
    assert_eq!(boot.backup_boot_sector(), None);
}

#[test]
fn a_32_mib_volume_mkfs_fat_formatted_is_fat16() {
    let boot = parse(FAT16_32M).unwrap();
    assert_eq!(boot.fat_type(), FatType::Fat16);
    assert_eq!(boot.clusters(), 16343);
    assert_eq!(boot.sectors_per_cluster(), 4);
    assert_eq!(boot.bytes_per_cluster(), 2048);
    assert_eq!(boot.table_first_sector(0), Some(4));
    assert_eq!(boot.table_first_sector(1), Some(68));
    assert_eq!(boot.table_first_sector(2), None);
    assert_eq!(
        boot.root_directory(),
        RootDirectory::Region {
            first_sector: 132,
            sectors: 32,
            entries: 512
        }
    );
    assert_eq!(boot.first_data_sector(), 164);
}

#[test]
fn a_64_mib_volume_mkfs_fat_formatted_is_fat32_with_its_reserved_pointers() {
    let boot = parse(FAT32_64M).unwrap();
    assert_eq!(boot.fat_type(), FatType::Fat32);
    assert_eq!(boot.clusters(), 129_022);
    assert_eq!(boot.reserved_sectors(), 32);
    assert_eq!(boot.root_directory(), RootDirectory::Cluster(2));
    assert_eq!(boot.first_data_sector(), 32 + 2 * 1009);
    assert_eq!(boot.fs_info_sector(), Some(1));
    assert_eq!(boot.backup_boot_sector(), Some(6));
    assert_eq!(boot.volume_id(), Some(0x1234_ABCD));
    assert_eq!(boot.active_table(), None);
}

#[test]
fn four_kib_sectors_parse_at_their_own_size() {
    let fat16 = parse(FAT16_4K).unwrap();
    assert_eq!(fat16.sector_size(), SectorSize::Bytes4096);
    assert_eq!(fat16.fat_type(), FatType::Fat16);
    assert_eq!(fat16.clusters(), 4092);
    // 512 entries of 32 bytes is four 4096-byte sectors.
    assert_eq!(
        fat16.root_directory(),
        RootDirectory::Region {
            first_sector: 12,
            sectors: 4,
            entries: 512
        }
    );
    assert_eq!(fat16.bytes_per_cluster(), 16384);

    let fat32 = parse(FAT32_4K).unwrap();
    assert_eq!(fat32.sector_size(), SectorSize::Bytes4096);
    assert_eq!(fat32.fat_type(), FatType::Fat32);
    assert_eq!(fat32.clusters(), 130_784);
}

#[test]
fn a_fat32_layout_under_the_cluster_floor_is_refused() {
    assert_eq!(
        parse(FAT32_UNDER_FLOOR),
        Err(ParseError::LayoutContradictsClusterCount)
    );
}

#[test]
fn what_this_crates_mkfs_writes_parses_to_the_same_geometry() {
    let volume =
        VolumeLayout::new(1024 * 1024, 2048, 9_000_000, 0x1234_5678, "BOOTX64.EFI").unwrap();
    let mut sector = [0u8; SECTOR_BYTES];
    for index in [0, 6] {
        assert!(volume.fill_sector(index, &mut sector));
        let boot = parse(&sector).unwrap();
        assert_eq!(boot.fat_type(), FatType::Fat32);
        assert_eq!(boot.clusters(), volume.clusters());
        assert_eq!(boot.table_sectors(), volume.table_sectors());
        assert_eq!(
            u64::from(boot.first_data_sector()),
            volume.first_data_sector()
        );
        assert_eq!(
            u64::from(boot.cluster_first_sector(5).unwrap()),
            volume.cluster_first_sector(5)
        );
        assert_eq!(boot.hidden_sectors(), 2048);
        assert_eq!(boot.volume_id(), Some(0x1234_5678));
        assert_eq!(boot.fs_info_sector(), Some(1));
        assert_eq!(boot.backup_boot_sector(), Some(6));
    }
}

#[test]
fn the_type_follows_the_cluster_count_at_both_thresholds() {
    // One reserved sector, two 16-sector tables and a 32-sector root put cluster 2 at sector 65.
    assert_eq!(
        parse(&Shape::fat16(65 + 4084, 16).bytes())
            .unwrap()
            .fat_type(),
        FatType::Fat12
    );
    let at = parse(&Shape::fat16(65 + 4085, 16).bytes()).unwrap();
    assert_eq!((at.fat_type(), at.clusters()), (FatType::Fat16, 4085));

    // Two 256-sector tables: cluster 2 at sector 545.
    let below = parse(&Shape::fat16(545 + 65_524, 256).bytes()).unwrap();
    assert_eq!(
        (below.fat_type(), below.clusters()),
        (FatType::Fat16, 65_524)
    );
    assert_eq!(
        parse(&Shape::fat16(545 + 65_525, 256).bytes()),
        Err(ParseError::LayoutContradictsClusterCount)
    );

    // Two 512-sector tables after 32 reserved sectors: cluster 2 at sector 1056.
    let at = parse(&Shape::fat32(1056 + 65_525, 512).bytes()).unwrap();
    assert_eq!((at.fat_type(), at.clusters()), (FatType::Fat32, 65_525));
    assert_eq!(
        parse(&Shape::fat32(1056 + 65_524, 512).bytes()),
        Err(ParseError::LayoutContradictsClusterCount)
    );
    assert_eq!(FatType::from_clusters(0), FatType::Fat12);
    assert_eq!(FatType::from_clusters(u32::MAX), FatType::Fat32);
}

#[test]
fn a_layout_half_one_type_and_half_another_is_refused() {
    // A FAT16 count with a zero root entry count, and a FAT32 count with a 16-bit table size.
    let mut no_root = Shape::fat16(73 + 5000, 20);
    no_root.root_entries = 0;
    assert_eq!(
        parse(&no_root.bytes()),
        Err(ParseError::LayoutContradictsClusterCount)
    );
    let mut root_on_32 = Shape::fat32(1232 + 70_000, 600);
    root_on_32.root_entries = 16;
    let mut sector = root_on_32.bytes();
    sector[22] = 1;
    assert_eq!(
        parse(&sector),
        Err(ParseError::LayoutContradictsClusterCount)
    );
    // FAT32 count, a root entry count, and the 32-bit table size: still a contradiction.
    assert_eq!(
        parse(&root_on_32.bytes()),
        Err(ParseError::LayoutContradictsClusterCount)
    );
    // FAT32 count, no root entries, but a 16-bit table size.
    let mut sector = Shape::fat32(1232 + 70_000, 600).bytes();
    sector[22..24].copy_from_slice(&512u16.to_le_bytes());
    assert_eq!(
        parse(&sector),
        Err(ParseError::LayoutContradictsClusterCount)
    );
}

#[test]
fn the_signature_and_the_jump_are_each_required() {
    assert_eq!(parse(&FAT16_32M[..511]), Err(ParseError::TooShort));
    assert_eq!(
        parse(&with(FAT16_32M, 510, &[0x00])),
        Err(ParseError::NoSignature)
    );
    assert_eq!(
        parse(&with(FAT16_32M, 511, &[0x00])),
        Err(ParseError::NoSignature)
    );
    assert_eq!(parse(&with(FAT16_32M, 0, &[0x00])), Err(ParseError::NoJump));
    assert_eq!(
        parse(&with(FAT16_32M, 0, &[0xEB, 0x3C, 0x00])),
        Err(ParseError::NoJump)
    );
    assert_eq!(
        parse(&with(FAT16_32M, 0, &[0x00, 0x3C, 0x90])),
        Err(ParseError::NoJump)
    );
    assert!(parse(&with(FAT16_32M, 0, &[0xE9, 0x00, 0x00])).is_ok());
    // A longer buffer, as a 4096-byte device sector arrives, reads the same 512 bytes.
    let mut long = [0u8; 4096];
    long[..512].copy_from_slice(FAT16_32M);
    assert_eq!(parse(&long), parse(FAT16_32M));
}

#[test]
fn every_sector_and_cluster_size_the_specification_refuses_is_refused() {
    for bad in [0u16, 256, 513, 8192] {
        assert_eq!(
            parse(&with(FAT16_32M, 11, &bad.to_le_bytes())),
            Err(ParseError::SectorSize)
        );
    }
    for bad in [0u8, 3, 6, 255] {
        assert_eq!(
            parse(&with(FAT16_32M, 13, &[bad])),
            Err(ParseError::ClusterSize)
        );
    }
    let mut big = Shape::fat32(1232 + 128 * 70_000, 600);
    big.per_cluster = 128;
    let boot = parse(&big.bytes()).unwrap();
    assert_eq!((boot.sectors_per_cluster(), boot.clusters()), (128, 70_000));
    assert_eq!(boot.bytes_per_cluster(), 65_536);
}

#[test]
fn zero_counts_and_a_bad_media_byte_are_refused() {
    assert_eq!(
        parse(&with(FAT16_32M, 14, &[0, 0])),
        Err(ParseError::NoReservedSectors)
    );
    assert_eq!(parse(&with(FAT16_32M, 16, &[0])), Err(ParseError::NoTables));
    for bad in [0x00u8, 0xEF, 0xF1, 0xF7] {
        assert_eq!(parse(&with(FAT16_32M, 21, &[bad])), Err(ParseError::Media));
    }
    for good in [0xF0u8, 0xF8, 0xFF] {
        assert!(parse(&with(FAT16_32M, 21, &[good])).is_ok());
    }
    let no_total = with(&with(FAT16_32M, 19, &[0, 0]), 32, &[0, 0, 0, 0]);
    assert_eq!(parse(&no_total), Err(ParseError::NoSectors));
    let no_table = with(&with(FAT16_32M, 22, &[0, 0]), 36, &[0, 0, 0, 0]);
    assert_eq!(parse(&no_table), Err(ParseError::NoTableSectors));
}

#[test]
fn the_16_bit_total_wins_over_the_32_bit_one() {
    let sector = with(
        &Shape::fat16(73 + 5000, 20).bytes(),
        19,
        &(73u16 + 4500).to_le_bytes(),
    );
    assert_eq!(parse(&sector).unwrap().clusters(), 4500);
}

#[test]
fn metadata_reaching_the_last_sector_leaves_no_cluster_and_past_it_is_refused() {
    let mut shape = Shape::fat16(65, 16);
    assert_eq!(parse(&shape.bytes()), Err(ParseError::NoClusters));
    shape.total = 64;
    assert_eq!(parse(&shape.bytes()), Err(ParseError::MetadataPastEnd));
    shape.total = 66;
    assert_eq!(parse(&shape.bytes()).unwrap().clusters(), 1);
    // Fewer data sectors than one cluster holds.
    shape.per_cluster = 4;
    shape.total = 68;
    assert_eq!(parse(&shape.bytes()), Err(ParseError::NoClusters));
    shape.total = 69;
    assert_eq!(parse(&shape.bytes()).unwrap().clusters(), 1);
}

#[test]
fn a_table_one_sector_short_of_every_cluster_is_refused() {
    // 4094 FAT16 clusters plus the two reserved entries is exactly 16 sectors of 16-bit entries.
    assert!(parse(&Shape::fat16(65 + 4094, 16).bytes()).is_ok());
    assert_eq!(
        parse(&Shape::fat16(65 + 4095, 16).bytes()),
        Err(ParseError::TableTooSmall)
    );
    // FAT12 packs 1.5 bytes per entry: 3 sectors hold 1024 entries, so 1022 clusters.
    let mut fat12 = Shape::fat16(1 + 6 + 32 + 1022, 3);
    assert!(parse(&fat12.bytes()).is_ok());
    fat12.total += 1;
    assert_eq!(parse(&fat12.bytes()), Err(ParseError::TableTooSmall));
    // FAT32 at 512 bytes holds 128 entries a sector.
    let exact = 512 * 128 - 2;
    assert!(parse(&Shape::fat32(1056 + exact, 512).bytes()).is_ok());
    assert_eq!(
        parse(&Shape::fat32(1056 + exact + 1, 512).bytes()),
        Err(ParseError::TableTooSmall)
    );
}

#[test]
fn more_clusters_than_fat32_numbers_are_refused() {
    let max = 0x0FFF_FFF4u32;
    let table = (max + 2).div_ceil(128);
    let mut shape = Shape::fat32(32 + table + max, table);
    shape.tables = 1;
    assert_eq!(parse(&shape.bytes()).unwrap().clusters(), max);
    shape.total += 1;
    assert_eq!(parse(&shape.bytes()), Err(ParseError::TooManyClusters));
}

#[test]
fn fat32s_own_fields_are_checked() {
    let base = Shape::fat32(1232 + 70_000, 600).bytes();
    assert_eq!(parse(&with(&base, 42, &[1, 0])), Err(ParseError::Version));
    assert_eq!(parse(&with(&base, 42, &[0, 1])), Err(ParseError::Version));
    for bad in [0u32, 1, 70_002, u32::MAX] {
        assert_eq!(
            parse(&with(&base, 44, &bad.to_le_bytes())),
            Err(ParseError::RootCluster)
        );
    }
    assert_eq!(
        parse(&with(&base, 44, &70_001u32.to_le_bytes()))
            .unwrap()
            .root_directory(),
        RootDirectory::Cluster(70_001)
    );
    // Mirroring on: the low bits mean nothing.
    assert_eq!(
        parse(&with(&base, 40, &[0x0F, 0])).unwrap().active_table(),
        None
    );
    assert_eq!(
        parse(&with(&base, 40, &[0x81, 0])).unwrap().active_table(),
        Some(1)
    );
    assert_eq!(
        parse(&with(&base, 40, &[0x80, 0])).unwrap().active_table(),
        Some(0)
    );
    assert_eq!(
        parse(&with(&base, 40, &[0x82, 0])),
        Err(ParseError::ActiveTable)
    );
    // A reserved-region pointer of 0, 0xFFFF or past the region names nothing.
    for (value, expected) in [
        (0u16, None),
        (0xFFFF, None),
        (32, None),
        (31, Some(31)),
        (1, Some(1)),
    ] {
        let sector = with(&base, 48, &value.to_le_bytes());
        assert_eq!(parse(&sector).unwrap().fs_info_sector(), expected);
        let sector = with(&base, 50, &value.to_le_bytes());
        assert_eq!(parse(&sector).unwrap().backup_boot_sector(), expected);
    }
}

#[test]
fn the_volume_id_is_read_only_under_the_extended_signature() {
    let fat16 = with(FAT16_32M, 38, &[0x28]);
    assert_eq!(parse(&fat16).unwrap().volume_id(), None);
    let fat32 = with(FAT32_64M, 66, &[0x28]);
    assert_eq!(parse(&fat32).unwrap().volume_id(), None);
    // FAT32 keeps its signature at 66; byte 38 there belongs to `BPB_FATSz32`.
    let fat32 = with(
        &Shape::fat32(1232 + 70_000, 600).bytes(),
        66,
        &[0x29, 1, 2, 3, 4],
    );
    assert_eq!(parse(&fat32).unwrap().volume_id(), Some(0x0403_0201));
}

#[test]
fn every_data_cluster_has_a_sector_and_no_other_number_does() {
    let boot = parse(FAT16_32M).unwrap();
    let last = boot.clusters() + 1;
    assert_eq!(boot.cluster_first_sector(0), None);
    assert_eq!(boot.cluster_first_sector(1), None);
    assert_eq!(boot.cluster_first_sector(2), Some(164));
    assert_eq!(boot.cluster_first_sector(3), Some(168));
    let sector = boot.cluster_first_sector(last).unwrap();
    assert_eq!(sector + boot.sectors_per_cluster(), boot.total_sectors());
    assert_eq!(boot.cluster_first_sector(last + 1), None);
    assert_eq!(boot.cluster_first_sector(u32::MAX), None);
}

/// A device over a byte slice, with a sector size of its own and a count of reads.
struct Image<'a> {
    bytes: &'a [u8],
    size: SectorSize,
    reads: u32,
}

impl SectorDevice for Image<'_> {
    type Error = u64;

    fn sector_size(&self) -> SectorSize {
        self.size
    }

    fn read_sector(&mut self, index: u64, buffer: &mut [u8]) -> Result<(), u64> {
        assert_eq!(buffer.len(), self.size.bytes());
        self.reads += 1;
        let start = index as usize * buffer.len();
        let bytes = self.bytes.get(start..start + buffer.len()).ok_or(index)?;
        buffer.copy_from_slice(bytes);
        Ok(())
    }
}

#[test]
fn reading_through_a_device_checks_its_sector_size() {
    let mut device = Image {
        bytes: FAT16_32M,
        size: SectorSize::Bytes512,
        reads: 0,
    };
    assert_eq!(
        BootSector::read(&mut device),
        parse(FAT16_32M).map_err(ReadError::Refused)
    );
    assert_eq!(device.reads, 1);

    let mut four_k = [0u8; 4096];
    four_k[..512].copy_from_slice(FAT32_4K);
    let mut device = Image {
        bytes: &four_k,
        size: SectorSize::Bytes4096,
        reads: 0,
    };
    let mut by_reference = &mut device;
    assert_eq!(
        BootSector::read(&mut by_reference).unwrap().fat_type(),
        FatType::Fat32
    );

    let mut device = Image {
        bytes: FAT16_4K,
        size: SectorSize::Bytes512,
        reads: 0,
    };
    assert_eq!(
        BootSector::read(&mut device),
        Err(ReadError::SectorSizeMismatch {
            device: SectorSize::Bytes512,
            volume: SectorSize::Bytes4096
        })
    );
}

#[test]
fn a_device_error_and_a_refusal_come_back_as_themselves() {
    let mut device = Image {
        bytes: &FAT16_32M[..100],
        size: SectorSize::Bytes512,
        reads: 0,
    };
    assert_eq!(BootSector::read(&mut device), Err(ReadError::Device(0)));
    let blank = [0u8; 512];
    let mut device = Image {
        bytes: &blank,
        size: SectorSize::Bytes512,
        reads: 0,
    };
    assert_eq!(
        BootSector::read(&mut device),
        Err(ReadError::Refused(ParseError::NoSignature))
    );
}
