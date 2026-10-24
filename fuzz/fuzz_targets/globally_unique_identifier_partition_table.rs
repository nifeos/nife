//! Fuzz the GUID partition table parser with the head of an arbitrary disk.
//!
//! The input is a disk prefix: `data[0..512]` is LBA 0, the protective MBR; `data[512..1024]` is
//! LBA 1, the primary header (its length is the block size, hence 512); `data[1024..]` is the entry
//! array. So the real `sgdisk` and Disk Utility disks in
//! `crates/globally_unique_identifier_partition_table/tests/fixtures/*.head` seed it directly. The
//! crate's Kani harnesses and mutation tests explore one field at a time; this reaches tables whose
//! `entry_size`, `entry_count`, `entry_array_lba`, `first_usable_lba` and `alternate_lba` are
//! hostile together and survive the CRC (notes/fuzzing/targets.md).

#![no_main]

use globally_unique_identifier_partition_table::GloballyUniqueIdentifierPartitionTable;
use libfuzzer_sys::fuzz_target;

/// The block size the fixtures use, and the one every disk this project has met uses. 4K-native
/// disks exist and `GloballyUniqueIdentifierPartitionTable::parse` takes the block size from the
/// header block's length, so a second split is worth having eventually; one is enough to keep the
/// seeds meaningful.
const BLOCK: usize = 512;

fuzz_target!(|data: &[u8]| {
    if data.len() < BLOCK * 2 {
        return;
    }
    let mbr = &data[..BLOCK];
    let header_block = &data[BLOCK..BLOCK * 2];
    let entry_array = &data[BLOCK * 2..];

    let Ok(table) = GloballyUniqueIdentifierPartitionTable::parse(header_block, entry_array) else {
        return;
    };

    // Every accessor a caller reaches for once `parse` has said yes. Several are bare arithmetic on
    // header fields (`alternate_lba + 1`, `alternate_lba - entry_array_blocks()`) whose freedom from
    // overflow is an invariant `parse` is supposed to have established, not something the accessor
    // rechecks. This is the target that holds it to that.
    let _ = table.header();
    let _ = table.disk_guid();
    let _ = table.block_size();
    let _ = table.block_count();
    let _ = table.first_usable_lba();
    let _ = table.last_usable_lba();
    let _ = table.backup_header_lba();
    let _ = table.primary_entry_lba();
    let _ = table.backup_entry_lba();
    let _ = table.entry_array_blocks();
    let _ = table.entry_count();
    let _ = table.entry_array();

    let mut name = [0u8; 4 * globally_unique_identifier_partition_table::entry::NAME_UNITS];
    for (index, entry) in table.partitions() {
        let _ = index;
        let _ = entry.is_used();
        // `blocks()` is `last_lba - first_lba + 1` on two fields the disk chose, and `name_utf8`
        // decodes 36 UTF-16 code units that may be lone surrogates.
        let _ = entry.blocks();
        let _ = entry.name_utf8(&mut name);
        let _ = entry.encode();
    }
    // Past the end of the array as well as inside it: `entry` is documented to answer `None` there
    // rather than to index, and `entry_count` is a header field a hostile table sets to 2^32-1.
    let _ = table.entry(0);
    let _ = table.entry(table.entry_count() as usize);
    let _ = table.entry(usize::MAX);

    // The two checks a caller runs against blocks it read separately. Feeding them the same bytes
    // will almost always be a mismatch, which is fine: the property under test is that a mismatch is
    // an `Err`, never a panic.
    let _ = table.check_protective_mbr(mbr);
    let _ = table.check_backup(header_block, entry_array);
});
