//! The proofs: the scratch range stays in the gap, the parse reads each entry from its own bytes,
//! and the protective check accepts only a protective MBR.
//!
//! Name: provisional, minted 2026-11-19 (UTC) for the module's move out of
//! `designware_mobile_storage`.

use super::*;

/// Falsification: replayable `crates/master_boot_record/falsifications/verification.the_scratch_range_never_touches_a_partition_or_sector_zero.patch`
/// No partition table makes the write test's range touch a partition or sector 0.
///
#[kani::proof]
#[kani::unwind(5)]
fn the_scratch_range_never_touches_a_partition_or_sector_zero() {
    let entry = || PartitionEntry {
        kind: kani::any(),
        start: kani::any(),
        sectors: kani::any(),
    };
    let master_boot_record = MasterBootRecord {
        entries: [entry(), entry(), entry(), entry()],
    };
    let want: u32 = kani::any();
    if let Some((first, count)) = master_boot_record.scratch_sector_range(want) {
        assert!(first >= 1);
        assert!((1..=want).contains(&count));
        let k: u32 = kani::any();
        kani::assume(k < count);
        let sector = u64::from(first) + u64::from(k);
        for e in &master_boot_record.entries {
            assert!(!e.contains(sector));
        }
    }
}

/// Falsification: replayable `crates/master_boot_record/falsifications/verification.the_master_boot_record_parse_is_total_and_reads_each_entry_from_its_own_bytes.patch`
/// The MBR parse is total on any sector 0 a card holds. It accepts exactly the sectors that end in
/// the signature, and each entry it returns is the type byte and the two little-endian words at
/// that entry's own offsets, so a partition's extent is what the card wrote and nothing else. An
/// entry contains a sector only inside `[start, start + sectors)`, with the end computed in 64
/// bits.
///
#[kani::proof]
#[kani::unwind(5)]
fn the_master_boot_record_parse_is_total_and_reads_each_entry_from_its_own_bytes() {
    let sector: [u8; 512] = kani::any();
    let parsed = MasterBootRecord::parse(&sector);
    assert!(parsed.is_some() == (sector[510] == 0x55 && sector[511] == 0xaa));
    if let Some(table) = parsed {
        let mut i = 0;
        while i < 4 {
            let at = 446 + 16 * i;
            let entry = table.entries[i];
            assert!(entry.kind == sector[at + 4]);
            assert!(
                entry.start
                    == u32::from_le_bytes([
                        sector[at + 8],
                        sector[at + 9],
                        sector[at + 10],
                        sector[at + 11]
                    ])
            );
            assert!(
                entry.sectors
                    == u32::from_le_bytes([
                        sector[at + 12],
                        sector[at + 13],
                        sector[at + 14],
                        sector[at + 15]
                    ])
            );
            let probe: u64 = kani::any();
            if entry.contains(probe) {
                assert!(entry.is_used());
                assert!(probe >= u64::from(entry.start));
                assert!(probe - u64::from(entry.start) < u64::from(entry.sectors));
            }
            i += 1;
        }
    }
}

/// **The protective MBR check is total**, on every sector and every disk size: no panic, and an
/// acceptance only of a sector whose signature, protective record and size are right.
///
/// Stated against the layout as the specification gives it (the four records at 446, the
/// signature at 510), not against the validator's own helpers, so the proof is not the code
/// compared with itself.
/// Falsification: replayable `crates/master_boot_record/falsifications/verification.the_protective_master_boot_record_check_is_total.patch`
#[kani::proof]
#[kani::unwind(5)]
fn the_protective_master_boot_record_check_is_total() {
    let sector: [u8; 512] = kani::any();
    let blocks: u64 = kani::any();
    if protective::validate(&sector, blocks).is_err() {
        return;
    }
    assert_eq!(sector[510..512], [0x55, 0xAA]);
    let record = |i: usize| &sector[446 + i * 16..446 + (i + 1) * 16];
    let word = |i: usize, at: usize| u32::from_le_bytes(record(i)[at..at + 4].try_into().unwrap());
    let protective = (0..4)
        .find(|&i| record(i)[4] == protective::PROTECTIVE_OPERATING_SYSTEM_TYPE)
        .unwrap();
    assert_eq!(word(protective, 8), 1);
    let whole = u32::try_from(blocks.saturating_sub(1)).unwrap_or(u32::MAX);
    assert!(word(protective, 12) == whole || word(protective, 12) == u32::MAX);
    for other in 0..4 {
        assert!(other == protective || record(other)[4] == 0 || word(other, 12) == 0);
    }
}
