//! Name: ratified 2026-11-03 (UTC), #3135. The Kani harnesses, named for what they are.

use crate::card::{CardSpecificData, extended_card_specific_data_sectors, extract_bits};
use crate::host_controller::queue_step;
use crate::internal_direct_memory_access::{
    DESCRIPTOR_BYTES, MAXIMUM_DESCRIPTORS, PAGE, descriptor, descriptors_for,
};
use crate::partition::{MasterBootRecord, PartitionEntry};
use crate::sd::can_address_blocks;

/// No partition table makes the write test's range touch a partition or sector 0.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.the_scratch_range_never_touches_a_partition_or_sector_zero.patch`
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

/// A FIFO step never moves more than is owed, and a write step never more than the FIFO has
/// room for, whatever `STATUS` says.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.a_queue_step_never_overruns_the_queue_or_the_buffer.patch`
#[kani::proof]
fn a_queue_step_never_overruns_the_queue_or_the_buffer() {
    let status: u32 = kani::any();
    let depth: u32 = kani::any();
    kani::assume((1..=4096).contains(&depth));
    let write: bool = kani::any();
    let remaining: u32 = kani::any();
    let n = queue_step(status, depth, write, remaining);
    assert!(n <= remaining);
    assert!(n <= depth * 4);
    let filled = crate::registers::queue_count(status);
    if write && filled <= depth {
        assert!(n <= (depth - filled) * 4);
    }
}

/// A range the driver admits is inside the card and inside what the command's 32-bit address
/// can name.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.an_admitted_range_is_inside_the_card_and_the_address.patch`
#[kani::proof]
fn an_admitted_range_is_inside_the_card_and_the_address() {
    let blocks: u64 = kani::any();
    let block_addressed: bool = kani::any();
    let logical_block_address: u64 = kani::any();
    let count: u64 = kani::any();
    if can_address_blocks(blocks, block_addressed, logical_block_address, count) {
        assert!(count >= 1);
        assert!(logical_block_address + count <= blocks);
        let last = logical_block_address + count - 1;
        if block_addressed {
            assert!(last <= u64::from(u32::MAX));
        } else {
            assert!((last + 1) * 512 <= 1 << 32);
        }
    }
}

/// A chain for any transfer a region admits describes exactly its bytes, each descriptor a page
/// or less inside the region's data pages, and points only at descriptors in the chain page.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.a_chain_describes_its_bytes_inside_the_region.patch`
#[kani::proof]
fn a_chain_describes_its_bytes_inside_the_region() {
    let pages: u32 = kani::any();
    kani::assume((1..=MAXIMUM_DESCRIPTORS).contains(&pages));
    let bus: u32 = kani::any();
    kani::assume(bus.is_multiple_of(PAGE));
    kani::assume(u64::from(bus) + u64::from((pages + 1) * PAGE) <= 1 << 32);
    let bytes: u32 = kani::any();
    kani::assume((1..=pages * PAGE).contains(&bytes));
    let count = descriptors_for(bytes);
    assert!((1..=pages).contains(&count));
    let i: u32 = kani::any();
    kani::assume(i < count);
    let [des0, size, buffer, next] = descriptor(bus, i, count, bytes);
    assert!((1..=PAGE).contains(&size));
    assert!(buffer >= bus + PAGE && buffer - bus + size <= (pages + 1) * PAGE);
    if i + 1 == count {
        assert!(i * PAGE + size == bytes);
        assert!(des0 & crate::internal_direct_memory_access::DES0_LD != 0);
    } else {
        assert!(size == PAGE && next == bus + (i + 1) * DESCRIPTOR_BYTES);
        assert!(des0 & crate::internal_direct_memory_access::DES0_LD == 0);
    }
}

/// The MBR parse is total on any sector 0 a card holds. It accepts exactly the sectors that end in
/// the signature, and each entry it returns is the type byte and the two little-endian words at
/// that entry's own offsets, so a partition's extent is what the card wrote and nothing else. An
/// entry contains a sector only inside `[start, start + sectors)`, with the end computed in 64
/// bits.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.the_master_boot_record_parse_is_total_and_reads_each_entry_from_its_own_bytes.patch`
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

/// A field read out of a 128-bit register never exceeds its width, reads zero from past bit 127,
/// and never panics, whatever the start, the length and the register.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.a_register_field_never_exceeds_its_width.patch`
#[kani::proof]
fn a_register_field_never_exceeds_its_width() {
    let register: [u32; 4] = kani::any();
    let start: u32 = kani::any();
    let len: u32 = kani::any();
    let field = extract_bits(&register, start, len);
    if len < 32 {
        assert!(u64::from(field) < 1u64 << len);
    }
    if start >= 128 {
        assert!(field == 0);
    }
}

/// Decoding any CSD, as SD or as eMMC, is total and sizes the card without overflow: the
/// structure is bits 127:126, an unknown structure or a read block length under 512 bytes sizes
/// nothing, and no structure sizes more blocks than its fields can name.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.a_card_specific_data_decode_is_total_and_never_overflows.patch`
#[kani::proof]
fn a_card_specific_data_decode_is_total_and_never_overflows() {
    let register: [u32; 4] = kani::any();
    let sd = CardSpecificData::from_sd_register(&register);
    assert!(sd.structure == register[3] >> 30);
    match sd.structure {
        0 => assert!(sd.blocks <= 1 << 27),
        1 => assert!(sd.blocks <= 1 << 32),
        2 => assert!(sd.blocks <= 1 << 38),
        _ => assert!(sd.blocks == 0),
    }
    let multimedia_card = CardSpecificData::multimedia_card(&register);
    assert!(multimedia_card.structure == sd.structure);
    assert!(multimedia_card.blocks <= 1 << 27);
    if extract_bits(&register, 80, 4) < 9 {
        assert!(multimedia_card.blocks == 0);
    }
}

/// An eMMC's `SEC_COUNT` is read from bytes 212 to 215 of any buffer, as a little-endian word, and
/// a buffer too short to hold it gives zero rather than a panic.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.an_extended_card_specific_data_sector_count_is_total.patch`
#[kani::proof]
fn an_extended_card_specific_data_sector_count_is_total() {
    let bytes: [u8; 216] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= 216);
    let sectors = extended_card_specific_data_sectors(&bytes[..len]);
    if len < 216 {
        assert!(sectors == 0);
    } else {
        assert!(
            sectors
                == u64::from(u32::from_le_bytes([
                    bytes[212], bytes[213], bytes[214], bytes[215]
                ]))
        );
    }
    assert!(sectors <= u64::from(u32::MAX));
}
