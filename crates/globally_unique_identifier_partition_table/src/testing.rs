use crate::guid::types;
use crate::{ENTRY_ARRAY_BYTES, Entry, PartitionTable, Uuid};

/// A 64 MiB, 512-byte-block disk carrying one data partition, as a contiguous
/// buffer: logical block 0 is at offset 0, so slicing by the block number times 512 gets any block.
///
/// Only the first 34 blocks are real; the rest is zeros. Enough for the primary table, which is
/// what a doc example needs.
pub fn sample_disk() -> [u8; 512 * 34] {
    let mut disk = [0u8; 512 * 34];
    let mut array = [0u8; ENTRY_ARRAY_BYTES];
    let part = Entry::new(
        types::LINUX_FILESYSTEM,
        Uuid::from_bytes([0x11; 16]),
        2048,
        131_038,
    )
    .with_name("data")
    .expect("four characters fit");
    let table = PartitionTable::create(
        Uuid::from_bytes([0x22; 16]),
        512,
        131_072,
        &[part],
        &mut array,
    )
    .expect("a 64 MiB disk has room for one partition");

    let (mbr, rest) = disk.split_at_mut(512);
    let (header, entries) = rest.split_at_mut(512);
    table
        .write_protective_master_boot_record(mbr)
        .expect("512 is a block");
    table.write_primary_header(header).expect("512 is a block");
    entries[..ENTRY_ARRAY_BYTES].copy_from_slice(table.entry_array());
    disk
}
