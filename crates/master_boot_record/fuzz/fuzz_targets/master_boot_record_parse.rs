//! Fuzz `MasterBootRecord::parse` over any 512 bytes.
//!
//! The first byte is a flag and the rest is the sector, padded with zeros or cut to 512 bytes. When
//! the flag is odd, the signature bytes are set first, so the input need not guess them to reach
//! the accepting path; otherwise the sector is exactly the input. Parsing must not panic, and a
//! table it accepts must keep the rules the type states: the signature is present, each entry is
//! read from its own bytes, and the scratch range `scratch_sector_range` offers lies below every
//! used partition, starts at sector 1 or later, and holds between 1 and the wanted number of
//! sectors.

#![no_main]

use libfuzzer_sys::fuzz_target;
use master_boot_record::MasterBootRecord;

/// The master boot record's own fixed sector, 512 bytes by the format; no logical block size.
const SECTOR_BYTES: usize = 512;
const ENTRY_TABLE_OFFSET: usize = 446;
const ENTRY_BYTES: usize = 16;

fuzz_target!(|data: &[u8]| {
    let Some((&flag, data)) = data.split_first() else {
        return;
    };
    let mut sector = [0u8; SECTOR_BYTES];
    let length = data.len().min(SECTOR_BYTES);
    sector[..length].copy_from_slice(&data[..length]);
    if flag & 1 != 0 {
        sector[510] = 0x55;
        sector[511] = 0xaa;
    }

    let Some(table) = MasterBootRecord::parse(&sector) else {
        return;
    };
    assert_eq!(sector[510..], [0x55, 0xaa]);

    // Each entry comes from its own sixteen bytes: the type at offset 4, the first sector at 8 and
    // the sector count at 12, little-endian.
    for (index, entry) in table.entries.iter().enumerate() {
        let bytes = &sector[ENTRY_TABLE_OFFSET + index * ENTRY_BYTES..][..ENTRY_BYTES];
        assert_eq!(entry.kind, bytes[4]);
        assert_eq!(
            entry.start,
            u32::from_le_bytes(bytes[8..12].try_into().unwrap())
        );
        assert_eq!(
            entry.sectors,
            u32::from_le_bytes(bytes[12..16].try_into().unwrap())
        );
    }

    // The wanted count comes from the bytes past the sector, or from the table's own bytes.
    let wanted = u32::from(data.get(SECTOR_BYTES).copied().unwrap_or(sector[0]));
    if let Some((first, count)) = table.scratch_sector_range(wanted) {
        assert!(first >= 1);
        assert!((1..=wanted).contains(&count));
        let end = first + count;
        for entry in table.entries.iter().filter(|entry| entry.is_used()) {
            assert!(end <= entry.start);
            assert!(!entry.contains(u64::from(first)));
        }
    }
});
