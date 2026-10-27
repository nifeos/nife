//! A partition table built byte by byte, every way it can be wrong, and a device that shows one
//! partition.
//!
//! Name: provisional.

use super::*;

/// A signed sector with no partitions.
fn empty() -> [u8; 512] {
    let mut sector = [0u8; 512];
    sector[510] = 0x55;
    sector[511] = 0xAA;
    sector
}

/// Write partition `index` into `sector`.
fn put(sector: &mut [u8; 512], index: usize, status: u8, kind: u8, first: u32, count: u32) {
    let at = 446 + index * 16;
    sector[at] = status;
    sector[at + 4] = kind;
    sector[at + 8..at + 12].copy_from_slice(&first.to_le_bytes());
    sector[at + 12..at + 16].copy_from_slice(&count.to_le_bytes());
}

#[test]
fn an_empty_table_has_no_partitions() {
    let table = MasterBootRecord::parse(&empty()).unwrap();
    assert_eq!(table.partitions().count(), 0);
    assert_eq!(table.partition(0), None);
    assert_eq!(table.partition(4), None);
    assert_eq!(table.partition(9), None);
}

#[test]
fn used_entries_read_their_fields_in_table_order() {
    let mut sector = empty();
    put(&mut sector, 1, 0x80, 0x0B, 63, 0x1FFA1);
    put(&mut sector, 3, 0x00, 0x83, 1 << 20, 4096);
    let table = MasterBootRecord::parse(&sector).unwrap();
    assert_eq!(table.partition(0), None);
    let fat = table.partition(1).unwrap();
    assert!(fat.is_bootable() && fat.holds_file_allocation_table());
    assert_eq!(
        (fat.kind(), fat.first_sector(), fat.sector_count()),
        (0x0B, 63, 0x1FFA1)
    );
    let linux = table.partition(3).unwrap();
    assert!(!linux.is_bootable() && !linux.holds_file_allocation_table());
    let mut all = table.partitions();
    assert_eq!(all.next(), Some(fat));
    assert_eq!(all.next(), Some(linux));
    assert_eq!(all.next(), None);
}

#[test]
fn every_fat_type_byte_is_recognized_and_others_are_not() {
    for kind in 0..=255u8 {
        let mut sector = empty();
        put(&mut sector, 0, 0, kind, 1, 1);
        let table = MasterBootRecord::parse(&sector).unwrap();
        let expected = matches!(kind, 0x01 | 0x04 | 0x06 | 0x0B | 0x0C | 0x0E | 0xEF);
        assert_eq!(
            table
                .partition(0)
                .is_some_and(|p| p.holds_file_allocation_table()),
            expected,
            "{kind:#x}"
        );
        assert_eq!(table.partition(0).is_some(), kind != 0);
    }
}

#[test]
fn an_unused_entry_is_ignored_whatever_else_it_holds() {
    let mut sector = empty();
    put(&mut sector, 0, 0x55, 0, u32::MAX, u32::MAX);
    assert_eq!(
        MasterBootRecord::parse(&sector)
            .unwrap()
            .partitions()
            .count(),
        0
    );
}

#[test]
fn a_sector_that_is_not_a_table_is_refused() {
    assert_eq!(
        MasterBootRecord::parse(&[0u8; 511]),
        Err(MasterBootRecordError::TooShort)
    );
    let mut sector = empty();
    sector[511] = 0;
    assert_eq!(
        MasterBootRecord::parse(&sector),
        Err(MasterBootRecordError::NoSignature)
    );
    let mut sector = empty();
    put(&mut sector, 2, 0x01, 0x0B, 1, 1);
    assert_eq!(
        MasterBootRecord::parse(&sector),
        Err(MasterBootRecordError::BadStatus { index: 2 })
    );
    let mut sector = empty();
    put(&mut sector, 1, 0, 0x0B, 1, 0);
    assert_eq!(
        MasterBootRecord::parse(&sector),
        Err(MasterBootRecordError::EmptyPartition { index: 1 })
    );
}

#[test]
fn a_large_partition_that_fits_is_accepted() {
    // Both numbers are small enough to add but their product passes 2^32.
    let mut sector = empty();
    put(&mut sector, 0, 0, 0x0B, 100_000, 100_000);
    let table = MasterBootRecord::parse(&sector).unwrap();
    assert_eq!(table.partition(0).unwrap().sector_count(), 100_000);
}

#[test]
fn a_partition_may_end_exactly_at_sector_2_to_the_32_and_not_a_sector_past_it() {
    let mut sector = empty();
    put(&mut sector, 0, 0, 0x0B, 1, u32::MAX);
    assert!(MasterBootRecord::parse(&sector).is_ok());
    put(&mut sector, 0, 0, 0x0B, 2, u32::MAX);
    assert_eq!(
        MasterBootRecord::parse(&sector),
        Err(MasterBootRecordError::PartitionOverflows { index: 0 })
    );
}

/// A device of numbered sectors that records the last sector asked for.
struct Numbered {
    last: Option<u64>,
    fail: bool,
}

impl SectorDevice for Numbered {
    type Error = u8;

    fn sector_size(&self) -> SectorSize {
        SectorSize::Bytes512
    }

    fn read_sector(&mut self, index: u64, buffer: &mut [u8]) -> Result<(), u8> {
        self.last = Some(index);
        if self.fail {
            return Err(7);
        }
        buffer.fill(index as u8);
        Ok(())
    }
}

#[test]
fn a_partition_device_adds_the_first_sector_and_stops_at_the_end() {
    let mut sector = empty();
    put(&mut sector, 0, 0, 0x0B, 100, 3);
    let partition = MasterBootRecord::parse(&sector)
        .unwrap()
        .partition(0)
        .unwrap();
    let mut device = partition.device(Numbered {
        last: None,
        fail: false,
    });
    let mut buffer = [0u8; 512];
    device.read_sector(0, &mut buffer).unwrap();
    assert_eq!(buffer[0], 100);
    device.read_sector(2, &mut buffer).unwrap();
    assert_eq!(buffer[0], 102);
    assert_eq!(
        device.read_sector(3, &mut buffer),
        Err(PartitionError::OutsidePartition(3))
    );
    assert_eq!(
        device.read_sector(u64::MAX, &mut buffer),
        Err(PartitionError::OutsidePartition(u64::MAX))
    );
    let mut inner = device.into_device();
    assert_eq!(
        inner.last,
        Some(102),
        "a refused read never reached the device"
    );
    inner.fail = true;
    let mut again = partition.device(inner);
    assert_eq!(
        again.read_sector(0, &mut buffer),
        Err(PartitionError::Device(7))
    );
}

#[test]
fn reading_sector_0_of_a_device_parses_it_and_reports_both_failures() {
    struct Disk([u8; 512], bool);
    impl SectorDevice for Disk {
        type Error = ();
        fn sector_size(&self) -> SectorSize {
            SectorSize::Bytes512
        }
        fn read_sector(&mut self, index: u64, buffer: &mut [u8]) -> Result<(), ()> {
            assert_eq!(index, 0);
            if self.1 {
                return Err(());
            }
            buffer.copy_from_slice(&self.0);
            Ok(())
        }
    }
    let mut sector = empty();
    put(&mut sector, 0, 0, 0x0C, 8, 8);
    assert!(MasterBootRecord::read(&mut Disk(sector, false)).is_ok());
    assert_eq!(
        MasterBootRecord::read(&mut Disk(sector, true)),
        Err(MasterBootRecordReadError::Device(()))
    );
    assert_eq!(
        MasterBootRecord::read(&mut Disk([0; 512], false)),
        Err(MasterBootRecordReadError::Parse(
            MasterBootRecordError::NoSignature
        ))
    );
}
