//! Name: ratified 2026-11-03 (UTC), #3135. The Kani harnesses, named for what they are.

use crate::card::{CardSpecificData, extended_card_specific_data_sectors, extract_bits};
use crate::command::{Command, Data, Response};
use crate::host_controller::{
    DATA_DEADLINE_MICROSECONDS, HostController, RegisterWindowAndClock, card_clock, clock_divider,
    queue_step,
};
use crate::internal_direct_memory_access::{
    DESCRIPTOR_BYTES, DirectMemoryAccessRegion, InternalDirectMemoryAccess, MAXIMUM_DESCRIPTORS,
    PAGE, descriptor, descriptors_for,
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
/// room for, whatever `STATUS_REGISTER` says.
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

/// The most clock readings one harness takes. Each harness bounds its own count below this.
const CLOCK_READINGS: usize = 8;

/// **A controller and a clock that owe the driver nothing.** Every register read is whatever the
/// solver picks, every write is dropped, and the clock is any 64-bit value, wrapping, standing
/// still or jumping either way, up to reading `past_from`. From that reading on, each reading is
/// more than `deadline` microseconds past every earlier one, in wrapping arithmetic: the one thing
/// the trait's "microseconds since some fixed point" promises a wait, that time passes. The
/// assertion is the property each wait harness proves: the driver takes no reading after
/// `last`.
struct AdversarialWindowAndClock {
    deadline: u64,
    past_from: usize,
    last: usize,
    readings: [u64; CLOCK_READINGS],
    taken: usize,
}

impl AdversarialWindowAndClock {
    fn new(deadline: u64, past_from: usize, last: usize) -> Self {
        assert!(last < CLOCK_READINGS);
        AdversarialWindowAndClock {
            deadline,
            past_from,
            last,
            readings: [0; CLOCK_READINGS],
            taken: 0,
        }
    }
}

impl RegisterWindowAndClock for AdversarialWindowAndClock {
    fn read(&mut self, _offset: u32) -> u32 {
        kani::any()
    }

    fn write(&mut self, _offset: u32, _value: u32) {}

    fn now_microseconds(&mut self) -> u64 {
        assert!(
            self.taken <= self.last,
            "the driver read the clock again after its deadline had passed"
        );
        let now: u64 = kani::any();
        if self.taken >= self.past_from {
            let mut i = 0;
            while i < self.taken {
                kani::assume(now.wrapping_sub(self.readings[i]) > self.deadline);
                i += 1;
            }
        }
        self.readings[self.taken] = now;
        self.taken += 1;
        now
    }
}

/// **A DMA region whose descriptors the controller rewrites at will**: every word read is the
/// solver's, every write and copy is dropped. Three pages at a page-aligned bus address below
/// 4 GiB, so a chain is one or two descriptors.
struct AdversarialRegion {
    bus_address: u64,
}

impl DirectMemoryAccessRegion for AdversarialRegion {
    fn bus_address(&self) -> u64 {
        self.bus_address
    }

    fn bytes(&self) -> u32 {
        3 * PAGE
    }

    fn read_u32(&self, _offset: u32) -> u32 {
        kani::any()
    }

    fn write_u32(&mut self, _offset: u32, _value: u32) {}

    fn read_bytes(&self, _offset: u32, _out: &mut [u8]) {}

    fn write_bytes(&mut self, _offset: u32, _source: &[u8]) {}

    fn barrier(&mut self) {}
}

/// A polled host over `window`, taken as `HostController::new` takes any controller whose
/// `HCON` names a 32-bit FIFO.
fn adversarial_host(
    window: AdversarialWindowAndClock,
) -> HostController<AdversarialWindowAndClock> {
    let depth: u32 = kani::any();
    kani::assume(depth > 0);
    let Ok((host, _identity)) = HostController::new(window, kani::any(), Some(depth)) else {
        kani::assume(false);
        unreachable!()
    };
    host
}

/// **A wait returns at the first clock reading past its deadline**, whatever the controller
/// answers and whatever the clock did before: wrapped past 2^64, stood still or jumped either way.
/// The reading past the deadline is the wait's last.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.a_wait_returns_at_the_first_reading_past_its_deadline.patch`
#[kani::proof]
#[kani::unwind(6)]
fn a_wait_returns_at_the_first_reading_past_its_deadline() {
    let deadline: u64 = kani::any();
    kani::assume(deadline < 1 << 60);
    let past_from: usize = kani::any();
    kani::assume((1..=3).contains(&past_from));
    let mut host = adversarial_host(AdversarialWindowAndClock::new(deadline, past_from, past_from));
    let met = host.is_condition_met_within(deadline, |window| window.read(0) & 1 == 0);
    kani::cover!(!met && host.registers_mut().taken == past_from + 1);
}

/// **The IDMAC wait returns once the clock has passed its deadline**, whatever `RINTSTS`, `IDSTS`
/// and the descriptors say. The deadline restarts only when a descriptor closes that was the
/// IDMAC's at every earlier look, so a controller that toggles a descriptor's `OWN` bit or
/// rewrites its size cannot hold the wait open: past the deadline it takes at most one reading
/// per descriptor in the chain.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.the_direct_memory_access_wait_returns_once_its_deadline_has_passed.patch`
#[kani::proof]
#[kani::unwind(7)]
fn the_direct_memory_access_wait_returns_once_its_deadline_has_passed() {
    let past_from: usize = kani::any();
    kani::assume((1..=2).contains(&past_from));
    let bytes: u32 = kani::any();
    kani::assume((1..=2 * PAGE).contains(&bytes));
    let count = descriptors_for(bytes);
    let window = AdversarialWindowAndClock::new(
        DATA_DEADLINE_MICROSECONDS,
        past_from,
        past_from + count as usize,
    );
    let host = adversarial_host(window);
    let bus_address: u64 = kani::any();
    kani::assume(bus_address.is_multiple_of(u64::from(PAGE)) && bus_address < 1 << 31);
    let identity = crate::host_controller::Identity {
        version_identification: 0,
        hardware_configuration: 1 << 7,
        user_identification: 0,
        queue_threshold: 0,
        card_detect: 0,
        firmware: [0; 5],
    };
    let Ok(engine) = InternalDirectMemoryAccess::new(AdversarialRegion { bus_address }, &identity)
    else {
        panic!("a three-page region below 2 GiB is refused");
    };
    let mut host = host.with_internal_direct_memory_access(engine);
    let blocks: u32 = kani::any();
    kani::assume((1..=16).contains(&blocks));
    let command = Command::with_data(
        18,
        kani::any(),
        Response::R1,
        Data {
            block_size: 512,
            blocks,
            is_write: kani::any(),
        },
    );
    let result = host.internal_direct_memory_access_wait(&command, count);
    kani::cover!(matches!(result, Err(crate::host_controller::Error::DataTimeout(..))));
}

/// **The card clock divider is total**: for any base and target it divides by no zero, overflows
/// nothing and panics nowhere, and the card clock it names is computed the same way. That the card
/// clock stays at or below the target is the host test's, exhaustively at the VisionFive 2's
/// 50 MHz: the comparison makes the solver build both 32-bit divisions, which ran past five minutes.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.the_clock_divider_is_total.patch`
#[kani::proof]
fn the_clock_divider_is_total() {
    let base: u32 = kani::any();
    let target: u32 = kani::any();
    if let Some(divider) = clock_divider(base, target) {
        assert!((divider == 0) == (base <= target));
        let _ = card_clock(base, divider);
    }
}
