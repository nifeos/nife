//! **Machine-checked proofs (Kani) of the partition table and the partition device**: every sector
//! parses to a table or an error, and a partition device never reads outside its partition.
//! Behind `#[cfg(kani)]`, so only `cargo kani` compiles them.
//!
//! The parse is straight-line code over fixed offsets, so its harness covers every 512-byte
//! sector. The device harness covers every partition and every sector index.
//!
//! Name: ratified 2026-11-01 (UTC), by the class rule on the `tests` and `verification` modules of this crate.

use super::*;

/// **Every sector parses, and what the parse accepts is sound.** For any 512 bytes the parse
/// returns a table or an error. When it returns a table, the signature is there, and every
/// partition has a status byte of 0 or `0x80`, at least one sector, and an end within 2^32 that
/// the fields in the entry state. An unused entry is exactly one whose type byte is 0.
/// Falsification: replayable `crates/file_allocation_table/falsifications/master_boot_record.verification.every_sector_parses_to_a_sound_table_or_an_error.patch`
#[kani::proof]
fn every_sector_parses_to_a_sound_table_or_an_error() {
    let sector: [u8; 512] = kani::any();
    let Ok(table) = MasterBootRecord::parse(&sector) else {
        return;
    };
    assert!(sector[510] == 0x55 && sector[511] == 0xAA);
    let index: usize = kani::any();
    kani::assume(index < PARTITION_COUNT);
    let at = 446 + index * 16;
    match table.partition(index) {
        None => assert_eq!(sector[at + 4], 0),
        Some(partition) => {
            assert!(sector[at + 4] != 0);
            assert!(sector[at] == 0x00 || sector[at] == 0x80);
            assert_eq!(partition.is_bootable(), sector[at] == 0x80);
            assert_eq!(partition.partition_type(), sector[at + 4]);
            assert!(partition.sector_count() >= 1);
            assert!(
                u64::from(partition.first_sector()) + u64::from(partition.sector_count())
                    <= u64::from(u32::MAX) + 1
            );
        }
    }
}

/// A device that records every index it is asked for.
struct Recorder {
    asked: Option<u64>,
}

impl SectorDevice for Recorder {
    type Error = ();

    fn sector_size(&self) -> SectorSize {
        SectorSize::Bytes512
    }

    fn read_sector(&mut self, index: u64, _: &mut [u8]) -> Result<(), ()> {
        self.asked = Some(index);
        Ok(())
    }
}

/// **A partition device reads only its partition's sectors.** For any partition the parse accepts
/// and any sector index, the read either fails without reaching the device, or reaches it at the
/// partition's first sector plus the index, which is inside `first_sector..first_sector + count`.
/// Falsification: replayable `crates/file_allocation_table/falsifications/master_boot_record.verification.a_partition_device_reads_only_its_partition.patch`
#[kani::proof]
fn a_partition_device_reads_only_its_partition() {
    let first_sector: u32 = kani::any();
    let sectors: u32 = kani::any();
    kani::assume(sectors >= 1 && u64::from(first_sector) + u64::from(sectors) <= 1 << 32);
    let partition = Partition {
        bootable: kani::any(),
        kind: kani::any(),
        first_sector,
        sectors,
    };
    let mut device = partition.restrict_device(Recorder { asked: None });
    let index: u64 = kani::any();
    let mut buffer = [0u8; 512];
    match device.read_sector(index, &mut buffer) {
        Ok(()) => {
            let asked = device.into_device().asked.expect("the device was reached");
            assert!(asked >= u64::from(first_sector));
            assert!(asked < u64::from(first_sector) + u64::from(sectors));
            assert_eq!(asked, u64::from(first_sector) + index);
        }
        Err(PartitionError::OutsidePartition(i)) => {
            assert_eq!(i, index);
            assert!(index >= u64::from(sectors));
            assert_eq!(device.into_device().asked, None);
        }
        Err(PartitionError::Device(())) => panic!("the recorder never fails"),
    }
}
