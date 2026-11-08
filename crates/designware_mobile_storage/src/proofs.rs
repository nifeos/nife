//! Name: ratified 2026-11-03 (UTC), #3135. The Kani harnesses, named for what they are.

use crate::card::{CardSpecificData, extended_card_specific_data_sectors, extract_bits};
use crate::command::{Command, Data, Response};
use crate::host_controller::{
    DATA_DEADLINE_MICROSECONDS, Error, HostController, RegisterWindowAndClock, card_clock_hertz,
    clock_divider, queue_step,
};
use crate::internal_direct_memory_access::{
    DESCRIPTOR_BYTES, DirectMemoryAccessRegion, InternalDirectMemoryAccess, MAXIMUM_DESCRIPTORS,
    PAGE_BYTES, descriptor, descriptors_for,
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
    let region_physical_address: u32 = kani::any();
    kani::assume(region_physical_address.is_multiple_of(PAGE_BYTES));
    kani::assume(
        u64::from(region_physical_address) + u64::from((pages + 1) * PAGE_BYTES) <= 1 << 32,
    );
    let bytes: u32 = kani::any();
    kani::assume((1..=pages * PAGE_BYTES).contains(&bytes));
    let count = descriptors_for(bytes);
    assert!((1..=pages).contains(&count));
    let i: u32 = kani::any();
    kani::assume(i < count);
    let [word_0, size, buffer, next] = descriptor(region_physical_address, i, count, bytes);
    assert!((1..=PAGE_BYTES).contains(&size));
    assert!(
        buffer >= region_physical_address + PAGE_BYTES
            && buffer - region_physical_address + size <= (pages + 1) * PAGE_BYTES
    );
    if i + 1 == count {
        assert!(i * PAGE_BYTES + size == bytes);
        assert!(word_0 & crate::internal_direct_memory_access::DESCRIPTOR_LAST != 0);
    } else {
        assert!(size == PAGE_BYTES && next == region_physical_address + (i + 1) * DESCRIPTOR_BYTES);
        assert!(word_0 & crate::internal_direct_memory_access::DESCRIPTOR_LAST == 0);
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
/// 4 GiB, so a chain is one or two descriptors. Each access asserts that the driver asked for
/// bytes inside the region, which the trait leaves the implementation to bound.
struct AdversarialRegion {
    physical_address: u64,
}

impl DirectMemoryAccessRegion for AdversarialRegion {
    fn direct_memory_access_address(&self) -> u64 {
        self.physical_address
    }

    fn bytes(&self) -> u32 {
        3 * PAGE_BYTES
    }

    fn read_u32(&self, offset: u32) -> u32 {
        assert!(u64::from(offset) + 4 <= u64::from(self.bytes()));
        kani::any()
    }

    fn write_u32(&mut self, offset: u32, _value: u32) {
        assert!(u64::from(offset) + 4 <= u64::from(self.bytes()));
    }

    fn read_bytes(&self, offset: u32, out: &mut [u8]) {
        assert!(u64::from(offset) + out.len() as u64 <= u64::from(self.bytes()));
    }

    fn write_bytes(&mut self, offset: u32, source: &[u8]) {
        assert!(u64::from(offset) + source.len() as u64 <= u64::from(self.bytes()));
    }

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
    let mut host = adversarial_host(AdversarialWindowAndClock::new(
        deadline, past_from, past_from,
    ));
    let met = host.is_condition_met_within(deadline, |window| window.read(0) & 1 == 0);
    assert!(host.registers_mut().taken <= past_from + 1);
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
    kani::assume((1..=2 * PAGE_BYTES).contains(&bytes));
    let count = descriptors_for(bytes);
    let window = AdversarialWindowAndClock::new(
        DATA_DEADLINE_MICROSECONDS,
        past_from,
        past_from + count as usize,
    );
    let host = adversarial_host(window);
    let physical_address: u64 = kani::any();
    kani::assume(
        physical_address.is_multiple_of(u64::from(PAGE_BYTES)) && physical_address < 1 << 31,
    );
    let identity = crate::host_controller::Identity {
        version_identification: 0,
        hardware_configuration: 1 << 7,
        user_identification: 0,
        queue_threshold: 0,
        card_detect: 0,
        firmware: [0; 5],
    };
    let Ok(engine) =
        InternalDirectMemoryAccess::new(AdversarialRegion { physical_address }, &identity)
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
            block_bytes: 512,
            blocks,
            is_write: kani::any(),
        },
    );
    let result = host.internal_direct_memory_access_wait(&command, count);
    assert!(host.registers_mut().taken <= past_from + count as usize + 1);
    kani::cover!(matches!(
        result,
        Err(crate::host_controller::Error::DataTimeout(..))
    ));
}

/// **The card clock divider is total**: for any base and target it divides by no zero, overflows
/// nothing and panics nowhere, and the card clock it names is computed the same way. That the card
/// clock stays at or below the target is proved at a 50 MHz base by
/// `reset_and_the_card_clock_are_total_against_any_controller`: for any base the comparison makes
/// the solver build both 32-bit divisions, which ran past five minutes.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.the_clock_divider_is_total.patch`
#[kani::proof]
fn the_clock_divider_is_total() {
    let base: u32 = kani::any();
    let target: u32 = kani::any();
    if let Some(divider) = clock_divider(base, target) {
        assert!((divider == 0) == (base <= target));
        let _ = card_clock_hertz(base, divider);
    }
}

/// A response kind, any of the seven.
fn any_response() -> Response {
    match kani::any::<u8>() % 7 {
        0 => Response::None,
        1 => Response::R1,
        2 => Response::R1b,
        3 => Response::R2,
        4 => Response::R3,
        5 => Response::R6,
        _ => Response::R7,
    }
}

/// **A controller that owes the driver nothing, and a clock that runs.** Every register read is
/// the solver's and every write is dropped, as in [`AdversarialWindowAndClock`], but each clock
/// reading is more than the longest deadline (the one-second power-up) past the one before, from
/// any start, wrapping included. Every wait therefore ends at its second reading. The wait
/// harnesses prove that a wait ends under any clock that passes its deadline; this clock lets the
/// harnesses of whole sequences bound their loops by what the controller says rather than by time.
/// It records the address and byte count of every write command the driver sends.
struct AdversarialWindowAndRunningClock {
    now: u64,
    step: u64,
    argument: u32,
    byte_count: u32,
    write_commands: u32,
    last_write: (u32, u32),
}

impl AdversarialWindowAndRunningClock {
    fn new() -> Self {
        let step: u64 = kani::any();
        kani::assume(step > crate::sd::POWER_UP_DEADLINE_MICROSECONDS && step < 1 << 40);
        AdversarialWindowAndRunningClock {
            now: kani::any(),
            step,
            argument: 0,
            byte_count: 0,
            write_commands: 0,
            last_write: (0, 0),
        }
    }
}

impl RegisterWindowAndClock for AdversarialWindowAndRunningClock {
    fn read(&mut self, _offset: u32) -> u32 {
        kani::any()
    }

    fn write(&mut self, offset: u32, value: u32) {
        use crate::registers::{
            BYTE_COUNT_REGISTER, COMMAND_ARGUMENT_REGISTER, COMMAND_REGISTER, COMMAND_WRITE,
        };
        if offset == COMMAND_ARGUMENT_REGISTER {
            self.argument = value;
        } else if offset == BYTE_COUNT_REGISTER {
            self.byte_count = value;
        } else if offset == COMMAND_REGISTER && value & COMMAND_WRITE != 0 {
            self.write_commands += 1;
            self.last_write = (self.argument, self.byte_count);
        }
    }

    fn now_microseconds(&mut self) -> u64 {
        self.now = self.now.wrapping_add(self.step);
        self.now
    }
}

/// **A command word sends the command asked for and nothing else**: the start bit, the index in
/// bits 5:0, never the clock-update-only bit (which would make the controller skip the command),
/// a data phase exactly when the command has one, and the write direction only on a write.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.a_command_word_sends_only_the_command_asked_for.patch`
#[kani::proof]
fn a_command_word_sends_only_the_command_asked_for() {
    let data = if kani::any() {
        Some(Data {
            block_bytes: kani::any(),
            blocks: kani::any(),
            is_write: kani::any(),
        })
    } else {
        None
    };
    let command = Command {
        index: kani::any(),
        argument: kani::any(),
        response: any_response(),
        data,
    };
    let word = command.register_word();
    assert!(word & crate::registers::COMMAND_START != 0);
    assert!(word & 0x3f == u32::from(command.index) & 0x3f);
    assert!(word & crate::registers::COMMAND_UPDATE_CLOCK_REGISTERS_ONLY == 0);
    assert!((word & crate::registers::COMMAND_DATA_EXPECTED != 0) == data.is_some());
    assert!((word & crate::registers::COMMAND_WRITE != 0) == matches!(data, Some(d) if d.is_write));
}

/// **Sending a command is total against any controller**: whatever the controller answers, the
/// send ends, panics nowhere, and maps what it saw to an error that names this command. A short
/// response leaves words 1 to 3 zero, and a bad response carries the status bit that made it bad.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.sending_a_command_is_total_against_any_controller.patch`
#[kani::proof]
#[kani::unwind(3)]
fn sending_a_command_is_total_against_any_controller() {
    let depth: u32 = kani::any();
    kani::assume(depth > 0);
    let Ok((mut host, _identity)) = HostController::new(
        AdversarialWindowAndRunningClock::new(),
        kani::any(),
        Some(depth),
    ) else {
        return;
    };
    let command = Command::new(kani::any(), kani::any(), any_response());
    match host.send_command(&command) {
        Ok(response) => {
            if command.response != Response::R2 {
                assert!(response[1] == 0 && response[2] == 0 && response[3] == 0);
            }
        }
        Err(Error::Busy) => {}
        Err(Error::CommandNotTaken(index) | Error::NoResponse(index)) => {
            assert!(index == command.index);
        }
        Err(Error::CommandTimeout(index, _)) => assert!(index == command.index),
        Err(Error::BadResponse(index, status)) => {
            assert!(index == command.index);
            assert!(
                status
                    & (crate::registers::RAW_INTERRUPT_RESPONSE_CRC_ERROR
                        | crate::registers::RAW_INTERRUPT_RESPONSE_ERROR)
                    != 0
            );
        }
        Err(_) => panic!("a command with no data phase failed in its data phase"),
    }
}

/// **Reset and the card clock are total against any controller**: a reset and a clock change at
/// any rate end and panic nowhere, whatever the controller answers, and at the VisionFive 2's
/// 50 MHz the card runs no faster than asked, or at the slowest rate the divider has when nothing
/// slower exists.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.reset_and_the_card_clock_are_total_against_any_controller.patch`
#[kani::proof]
#[kani::unwind(3)]
fn reset_and_the_card_clock_are_total_against_any_controller() {
    let Ok((mut host, _identity)) = HostController::new(
        AdversarialWindowAndRunningClock::new(),
        crate::jh7110::CARD_INTERFACE_UNIT_HERTZ,
        None,
    ) else {
        return;
    };
    let _ = host.reset();
    let target: u32 = kani::any();
    if let Ok(hertz) = host.set_clock(target) {
        let slowest = crate::jh7110::CARD_INTERFACE_UNIT_HERTZ / 510;
        assert!(hertz <= target || hertz == slowest);
    }
}

/// The largest FIFO data phase the queue harness moves, in bytes. Its loop runs once per FIFO
/// step and its inner loop once per word, so the bound is this over four, plus one.
const QUEUE_PROOF_BYTES: usize = 16;

/// **A FIFO transfer stays inside its buffer**: whatever the controller reports (any fill, any
/// status, data that ends early or never), `read_data` and `write_data` through a FIFO one or
/// two words deep end, index the buffer only inside it, and panic nowhere. Bounded: data phases
/// of one or two blocks of 4 or 8 bytes. The step arithmetic does not depend on the length, and
/// `a_queue_step_never_overruns_the_queue_or_the_buffer` proves each step for any length; the host
/// tests move whole 512-byte blocks.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.a_queue_transfer_stays_inside_its_buffer.patch`
#[kani::proof]
#[kani::unwind(6)]
fn a_queue_transfer_stays_inside_its_buffer() {
    let depth: u32 = kani::any();
    kani::assume((1..=2).contains(&depth));
    let Ok((mut host, _identity)) = HostController::new(
        AdversarialWindowAndRunningClock::new(),
        kani::any(),
        Some(depth),
    ) else {
        return;
    };
    let words: u32 = kani::any();
    kani::assume((1..=2).contains(&words));
    let blocks: u32 = kani::any();
    kani::assume((1..=2).contains(&blocks));
    let write: bool = kani::any();
    let data = Data {
        block_bytes: 4 * words,
        blocks,
        is_write: write,
    };
    let command = Command::with_data(17, kani::any(), Response::R1, data);
    let mut buffer = [0u8; QUEUE_PROOF_BYTES];
    let length = data.total_bytes() as usize;
    let result = if write {
        host.write_data(&command, &buffer[..length])
    } else {
        host.read_data(&command, &mut buffer[..length])
    };
    assert!(result != Err(Error::BufferSize));
}

/// A host whose data phases go through the IDMAC, over [`AdversarialRegion`].
fn adversarial_direct_memory_access_host()
-> HostController<AdversarialWindowAndRunningClock, AdversarialRegion> {
    let Ok((host, identity)) = HostController::new(
        AdversarialWindowAndRunningClock::new(),
        crate::jh7110::CARD_INTERFACE_UNIT_HERTZ,
        None,
    ) else {
        kani::assume(false);
        unreachable!()
    };
    let physical_address: u64 = kani::any();
    let Ok(engine) =
        InternalDirectMemoryAccess::new(AdversarialRegion { physical_address }, &identity)
    else {
        kani::assume(false);
        unreachable!()
    };
    host.with_internal_direct_memory_access(engine)
}

/// A card of any size and addressing, as `identify` might have left it.
fn any_card() -> crate::sd::Card {
    crate::sd::Card {
        kind: crate::sd::CardKind::SdHighCapacity,
        relative_card_address: kani::any(),
        operation_conditions: 0,
        card_identification_raw: [0; 4],
        card_specific_data_raw: [0; 4],
        card_identification: crate::card::CardIdentification::from_sd_register(&[0; 4]),
        blocks: kani::any(),
        block_addressed: kani::any(),
        bus_width: 4,
        clock_hertz: crate::sd::TRANSFER_HERTZ,
    }
}

/// **Reading a block never writes the card or leaves the region**: whatever the controller and
/// the descriptors say, `read_blocks` of one block through the IDMAC ends, panics nowhere, touches
/// the DMA region only inside it, and sends no write command.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.reading_a_block_never_writes_the_card_or_leaves_the_region.patch`
#[kani::proof]
#[kani::unwind(9)]
fn reading_a_block_never_writes_the_card_or_leaves_the_region() {
    let mut host = adversarial_direct_memory_access_host();
    let card = any_card();
    let mut buffer = [0u8; crate::sd::BLOCK_BYTES];
    let _ = crate::sd::read_blocks(&mut host, &card, kani::any(), &mut buffer);
    assert!(host.registers_mut().write_commands == 0);
}

/// **Writing a block sends only the block asked for**: whatever the controller and the
/// descriptors say, `write_blocks` of one block through the IDMAC ends, panics nowhere, touches
/// the DMA region only inside it, and sends at most one write command. That command addresses the
/// block asked for, is 512 bytes long, and goes out only for a block the card has.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.writing_a_block_sends_only_the_block_asked_for.patch`
#[kani::proof]
#[kani::unwind(9)]
fn writing_a_block_sends_only_the_block_asked_for() {
    let mut host = adversarial_direct_memory_access_host();
    let card = any_card();
    let logical_block_address: u64 = kani::any();
    let buffer = [0u8; crate::sd::BLOCK_BYTES];
    let _ = crate::sd::write_blocks(&mut host, &card, logical_block_address, &buffer);
    let window = host.registers_mut();
    assert!(window.write_commands <= 1);
    if window.write_commands == 1 {
        assert!(crate::sd::can_address_blocks(
            card.blocks,
            card.block_addressed,
            logical_block_address,
            1
        ));
        assert!(window.last_write.0 == card.block_address_argument(logical_block_address));
        assert!(window.last_write.1 == crate::sd::BLOCK_BYTES as u32);
    }
}

/// **Bringing a card up is total against any controller**: whatever the controller and the
/// card answer, the first three steps of `identify` (reset and `CMD0`, `CMD8`, power-up by
/// `ACMD41` or `CMD1`, then `CMD2`, `CMD3`, `CMD9` and `CMD7`) end, panic nowhere and send no write
/// command. `identify` as one harness ran past five minutes; this and
/// `configuring_a_card_is_total_against_any_controller` prove its steps, and what `identify` does
/// between them is to build the `Card` from their results.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.bringing_a_card_up_is_total_against_any_controller.patch`
#[kani::proof]
#[kani::unwind(3)]
fn bringing_a_card_up_is_total_against_any_controller() {
    let Ok((mut host, _identity)) = HostController::new(
        AdversarialWindowAndRunningClock::new(),
        crate::jh7110::CARD_INTERFACE_UNIT_HERTZ,
        None,
    ) else {
        return;
    };
    let mut steps = 0;
    if let Ok(is_version_2_or_later) = crate::sd::go_idle_and_check_interface(&mut host) {
        steps += 1;
        if let Ok((kind, _operation_conditions)) =
            crate::sd::power_up(&mut host, is_version_2_or_later)
        {
            steps += 1;
            if crate::sd::address_and_select(&mut host, kind).is_ok() {
                steps += 1;
            }
        }
    }
    assert!(host.registers_mut().write_commands == 0);
    kani::cover!(steps == 3);
}

/// **Configuring a card is total against any controller**: whatever the controller, the card and
/// its CSD and `EXT_CSD` say, the last steps of `identify` (bus width, block length and clock for
/// an SD card; clock and `EXT_CSD` through the IDMAC for an eMMC) end, panic nowhere, touch the DMA
/// region only inside it, and send no write command. A configured SD card has a capacity and runs
/// no faster than the transfer clock; the CID decodes `identify` uses are total.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.configuring_a_card_is_total_against_any_controller.patch`
#[kani::proof]
#[kani::unwind(9)]
fn configuring_a_card_is_total_against_any_controller() {
    let mut host = adversarial_direct_memory_access_host();
    let mut card = any_card();
    let card_specific_data: [u32; 4] = kani::any();
    let card_identification: [u32; 4] = kani::any();
    let _ = crate::card::CardIdentification::from_sd_register(&card_identification);
    let _ = crate::card::CardIdentification::multimedia_card(&card_identification);
    if kani::any() {
        if crate::sd::configure_sd(&mut host, &mut card, &card_specific_data, kani::any()).is_ok() {
            assert!(card.blocks > 0);
            assert!(card.clock_hertz <= crate::sd::TRANSFER_HERTZ);
        }
    } else {
        let _ = crate::sd::configure_multimedia_card(&mut host, &mut card, &card_specific_data);
    }
    assert!(host.registers_mut().write_commands == 0);
}

/// Where a card stands in bring-up: the states of the SD Physical Layer Simplified
/// Specification's card identification mode (figure 4-1, version 9.10) and JEDEC JESD84's, which
/// `identify` walks in this order, and `Unknown` for a card whose answer the controller garbled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CardState {
    /// After `CMD0`, and while `ACMD41` or `CMD1` reports busy.
    Idle,
    /// Powered up: `ACMD41` or `CMD1` reported the busy bit done.
    Ready,
    /// `CMD2` sent the card identification.
    Identification,
    /// `CMD3` gave the card its relative address.
    Standby,
    /// `CMD7` selected the card.
    Transfer,
    /// The controller garbled an answer, so the card may have moved or not. Only `CMD0` is sure
    /// to reach it.
    Unknown,
}

/// Any state, as an earlier bring-up that failed at any step might leave a card.
fn any_card_state() -> CardState {
    match kani::any::<u8>() % 6 {
        0 => CardState::Idle,
        1 => CardState::Ready,
        2 => CardState::Identification,
        3 => CardState::Standby,
        4 => CardState::Transfer,
        _ => CardState::Unknown,
    }
}

/// How the card met the last command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answer {
    /// It answered, and the controller reports a clean response and, for a data command, the
    /// data transferred.
    Answered,
    /// It did not answer, and the controller reports a response timeout.
    Silent,
    /// The controller reports a response error. Whether the card moved is unknown.
    Garbled,
}

/// The bits of `ModeledCard::passed`: each state bring-up enters after `CMD0`.
const PASSED_READY: u8 = 1 << 0;
/// `CMD3` answered after `CMD2`.
const PASSED_IDENTIFICATION: u8 = 1 << 1;
/// `CMD3` answered.
const PASSED_STANDBY: u8 = 1 << 2;
/// `CMD7` answered.
const PASSED_TRANSFER: u8 = 1 << 3;
/// Every state bring-up passes through.
const PASSED_EVERY_STATE: u8 =
    PASSED_READY | PASSED_IDENTIFICATION | PASSED_STANDBY | PASSED_TRANSFER;

/// What the controller looked like when the first card command went out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FirstCommand {
    index: u8,
    controller_was_reset: bool,
    powered: bool,
    clock_enabled: bool,
    clock_divider: u32,
    controller_bus_width: u8,
}

/// **A card of any history behind a controller that carries out each command, judged against the
/// card state machine.** It starts in any state with any relative address and bus width, and the
/// controller with any bus width and clock, as an earlier bring-up that failed anywhere might leave
/// them. For each command the solver picks whether the card answered, stayed silent or had its
/// answer garbled, and picks the response words; the controller reports that choice and is
/// otherwise idle and error free, so every wait the driver makes on it is met at its first look.
/// That keeps these harnesses on the order of commands. A controller that fails a command another
/// way (never takes it, never finishes it, holds the card busy) reaches the driver as an error from
/// the same call, which the driver meets as it meets a garbled answer, and the harnesses of the
/// totality of each step cover such a controller. The one thing assumed of the card is that an SD
/// card publishes a relative address other than zero, which the SD specification reserves.
///
/// The clock reads two seconds for each power-up command (`ACMD41` or `CMD1`) since the last
/// `CMD0`. Every wait on this controller is met at its first look, so power-up's deadline is the
/// only one that reads the clock twice, and it sees the deadline pass after one try. The wait
/// harnesses prove that a wait ends under any clock that passes its deadline, and
/// `a_card_that_keeps_reporting_busy_is_given_up_on_once_the_deadline_passes` proves the retries.
///
/// Each command is judged legal or not against the state the card is in: `CMD0` from any state;
/// `CMD8` in idle, or with a data phase (an eMMC's `EXT_CSD`) in transfer; `CMD55` addressed to the
/// card's own address in idle, standby or transfer; `ACMD41` or `CMD1` in idle; `CMD2` in ready;
/// `CMD3` in identification; `CMD9` and `CMD7` addressed to the card in standby; `ACMD6` to an SD
/// card and `CMD16` in transfer. An application command is legal only straight after `CMD55`, and
/// nothing else is. A card that answers a legal command moves as the specification says; a garbled
/// answer, or an answer to an illegal command, leaves it unknown.
struct ModeledCard {
    power_up_commands_since_go_idle: u64,
    state: CardState,
    application_command_pending: bool,
    multimedia_card: bool,
    relative_card_address: u16,
    card_bus_width: u8,
    block_bytes_set: bool,
    extended_card_specific_data_read: bool,
    passed: u8,
    ready_operation_conditions: u32,
    answer: Answer,
    response: [u32; 4],
    argument: u32,
    controller_bus_width: u8,
    controller_was_reset: bool,
    powered: bool,
    clock_enabled: bool,
    clock_divider: u32,
    commands: u32,
    illegal_commands: u32,
    write_commands: u32,
    first_command: Option<FirstCommand>,
}

impl ModeledCard {
    fn new() -> Self {
        let controller_bus_width = match kani::any::<u8>() % 3 {
            0 => 1,
            1 => 4,
            _ => 8,
        };
        ModeledCard {
            power_up_commands_since_go_idle: 0,
            state: any_card_state(),
            application_command_pending: kani::any(),
            multimedia_card: kani::any(),
            relative_card_address: kani::any(),
            card_bus_width: if kani::any() { 4 } else { 1 },
            block_bytes_set: kani::any(),
            extended_card_specific_data_read: kani::any(),
            passed: 0,
            ready_operation_conditions: 0,
            answer: Answer::Garbled,
            response: [0; 4],
            argument: 0,
            controller_bus_width,
            controller_was_reset: false,
            powered: false,
            clock_enabled: false,
            clock_divider: kani::any(),
            commands: 0,
            illegal_commands: 0,
            write_commands: 0,
            first_command: None,
        }
    }

    /// Whether the card, as it stands, accepts command `index` with this argument.
    fn accepts(&self, index: u8, has_data: bool, is_write: bool) -> bool {
        use crate::command::{
            ALL_SEND_CARD_IDENTIFICATION, APPLICATION_COMMAND, GO_IDLE_STATE,
            SD_SEND_OPERATION_CONDITIONS, SELECT_CARD, SEND_CARD_SPECIFIC_DATA,
            SEND_INTERFACE_CONDITION, SEND_OPERATION_CONDITIONS, SEND_RELATIVE_ADDRESS,
            SET_BLOCK_LENGTH, SET_BUS_WIDTH,
        };
        let addressed = (self.argument >> 16) as u16 == self.relative_card_address;
        let is_application_command =
            index == SD_SEND_OPERATION_CONDITIONS || index == SET_BUS_WIDTH;
        if index == GO_IDLE_STATE {
            return !has_data;
        }
        if self.state == CardState::Unknown
            || self.application_command_pending != is_application_command
            || is_write
            || (has_data && index != SEND_INTERFACE_CONDITION)
        {
            return false;
        }
        match index {
            SEND_INTERFACE_CONDITION if has_data => {
                self.state == CardState::Transfer && self.multimedia_card
            }
            SEND_INTERFACE_CONDITION => self.state == CardState::Idle,
            APPLICATION_COMMAND => {
                addressed
                    && matches!(
                        self.state,
                        CardState::Idle | CardState::Standby | CardState::Transfer
                    )
            }
            SD_SEND_OPERATION_CONDITIONS | SEND_OPERATION_CONDITIONS => {
                self.state == CardState::Idle
            }
            ALL_SEND_CARD_IDENTIFICATION => self.state == CardState::Ready,
            SEND_RELATIVE_ADDRESS => self.state == CardState::Identification,
            SEND_CARD_SPECIFIC_DATA | SELECT_CARD => addressed && self.state == CardState::Standby,
            SET_BUS_WIDTH => self.state == CardState::Transfer && !self.multimedia_card,
            SET_BLOCK_LENGTH => self.state == CardState::Transfer,
            _ => false,
        }
    }

    /// The card meets a command the driver wrote to `COMMAND_REGISTER`.
    fn meet(&mut self, word: u32) {
        use crate::card::OPERATION_CONDITIONS_BUSY_DONE;
        use crate::command::{
            ALL_SEND_CARD_IDENTIFICATION, APPLICATION_COMMAND, GO_IDLE_STATE,
            SD_SEND_OPERATION_CONDITIONS, SELECT_CARD, SEND_INTERFACE_CONDITION,
            SEND_OPERATION_CONDITIONS, SEND_RELATIVE_ADDRESS, SET_BLOCK_LENGTH, SET_BUS_WIDTH,
        };
        use crate::registers::{COMMAND_DATA_EXPECTED, COMMAND_WRITE};
        let index = (word & 0x3f) as u8;
        let has_data = word & COMMAND_DATA_EXPECTED != 0;
        let is_write = word & COMMAND_WRITE != 0;
        if self.first_command.is_none() {
            self.first_command = Some(FirstCommand {
                index,
                controller_was_reset: self.controller_was_reset,
                powered: self.powered,
                clock_enabled: self.clock_enabled,
                clock_divider: self.clock_divider,
                controller_bus_width: self.controller_bus_width,
            });
        }
        self.commands += 1;
        if index == GO_IDLE_STATE {
            self.power_up_commands_since_go_idle = 0;
        } else if index == SD_SEND_OPERATION_CONDITIONS || index == SEND_OPERATION_CONDITIONS {
            self.power_up_commands_since_go_idle += 1;
        }
        if is_write {
            self.write_commands += 1;
        }
        let legal = self.accepts(index, has_data, is_write);
        if !legal {
            self.illegal_commands += 1;
        }
        self.answer = match kani::any::<u8>() % 3 {
            0 => Answer::Answered,
            1 => Answer::Silent,
            _ => Answer::Garbled,
        };
        self.response = kani::any();
        match self.answer {
            Answer::Silent => {
                self.application_command_pending = false;
                return;
            }
            Answer::Garbled => {
                self.application_command_pending = false;
                self.state = CardState::Unknown;
                return;
            }
            Answer::Answered if !legal => {
                self.application_command_pending = false;
                self.state = CardState::Unknown;
                return;
            }
            Answer::Answered => {}
        }
        self.application_command_pending = false;
        let busy_done = self.response[0] & OPERATION_CONDITIONS_BUSY_DONE != 0;
        match index {
            GO_IDLE_STATE => {
                self.state = CardState::Idle;
                self.relative_card_address = 0;
                self.card_bus_width = 1;
                self.block_bytes_set = false;
                self.extended_card_specific_data_read = false;
                self.passed = 0;
            }
            APPLICATION_COMMAND => self.application_command_pending = true,
            SD_SEND_OPERATION_CONDITIONS | SEND_OPERATION_CONDITIONS if busy_done => {
                self.state = CardState::Ready;
                self.multimedia_card = index == SEND_OPERATION_CONDITIONS;
                self.ready_operation_conditions = self.response[0];
                self.passed |= PASSED_READY;
            }
            ALL_SEND_CARD_IDENTIFICATION => {
                self.state = CardState::Identification;
                self.passed |= PASSED_IDENTIFICATION;
            }
            SEND_RELATIVE_ADDRESS => {
                self.state = CardState::Standby;
                self.relative_card_address = if self.multimedia_card {
                    (self.argument >> 16) as u16
                } else {
                    kani::assume(self.response[0] >> 16 != 0);
                    (self.response[0] >> 16) as u16
                };
                self.passed |= PASSED_STANDBY;
            }
            SELECT_CARD => {
                self.state = CardState::Transfer;
                self.passed |= PASSED_TRANSFER;
            }
            SET_BUS_WIDTH => self.card_bus_width = if self.argument & 3 == 2 { 4 } else { 1 },
            SET_BLOCK_LENGTH => {
                self.block_bytes_set = self.argument == crate::sd::BLOCK_BYTES as u32;
            }
            SEND_INTERFACE_CONDITION if has_data => self.extended_card_specific_data_read = true,
            _ => {}
        }
    }
}

impl RegisterWindowAndClock for ModeledCard {
    fn read(&mut self, offset: u32) -> u32 {
        use crate::registers::{
            BUS_MODE_REGISTER, COMMAND_REGISTER, CONTROL_REGISTER,
            INTERNAL_DIRECT_MEMORY_ACCESS_STATUS_REGISTER, RAW_INTERRUPT_COMMAND_DONE,
            RAW_INTERRUPT_DATA_TRANSFER_OVER, RAW_INTERRUPT_RESPONSE_ERROR,
            RAW_INTERRUPT_RESPONSE_TIMEOUT, RAW_INTERRUPT_STATUS_REGISTER, RESPONSE_0_REGISTER,
            RESPONSE_1_REGISTER, RESPONSE_2_REGISTER, RESPONSE_3_REGISTER, STATUS_REGISTER,
        };
        match offset {
            COMMAND_REGISTER
            | CONTROL_REGISTER
            | STATUS_REGISTER
            | BUS_MODE_REGISTER
            | INTERNAL_DIRECT_MEMORY_ACCESS_STATUS_REGISTER => 0,
            RAW_INTERRUPT_STATUS_REGISTER => match self.answer {
                Answer::Answered => RAW_INTERRUPT_COMMAND_DONE | RAW_INTERRUPT_DATA_TRANSFER_OVER,
                Answer::Silent => RAW_INTERRUPT_COMMAND_DONE | RAW_INTERRUPT_RESPONSE_TIMEOUT,
                Answer::Garbled => RAW_INTERRUPT_COMMAND_DONE | RAW_INTERRUPT_RESPONSE_ERROR,
            },
            RESPONSE_0_REGISTER => self.response[0],
            RESPONSE_1_REGISTER => self.response[1],
            RESPONSE_2_REGISTER => self.response[2],
            RESPONSE_3_REGISTER => self.response[3],
            _ => kani::any(),
        }
    }

    fn write(&mut self, offset: u32, value: u32) {
        use crate::registers::{
            CARD_TYPE_4_BIT_BUS, CARD_TYPE_8_BIT_BUS, CARD_TYPE_REGISTER, CLOCK_DIVIDER_REGISTER,
            CLOCK_ENABLE_CARD_CLOCK, CLOCK_ENABLE_REGISTER, COMMAND_ARGUMENT_REGISTER,
            COMMAND_REGISTER, COMMAND_START, COMMAND_UPDATE_CLOCK_REGISTERS_ONLY,
            CONTROL_ALL_RESETS_MASK, CONTROL_REGISTER, POWER_ENABLE_REGISTER,
        };
        match offset {
            COMMAND_ARGUMENT_REGISTER => self.argument = value,
            COMMAND_REGISTER => {
                if value & COMMAND_START != 0 && value & COMMAND_UPDATE_CLOCK_REGISTERS_ONLY == 0 {
                    self.meet(value);
                }
            }
            CARD_TYPE_REGISTER => {
                self.controller_bus_width = if value & CARD_TYPE_8_BIT_BUS != 0 {
                    8
                } else if value & CARD_TYPE_4_BIT_BUS != 0 {
                    4
                } else {
                    1
                };
            }
            CLOCK_DIVIDER_REGISTER => self.clock_divider = value,
            CLOCK_ENABLE_REGISTER => self.clock_enabled = value & CLOCK_ENABLE_CARD_CLOCK != 0,
            POWER_ENABLE_REGISTER => self.powered = value & 1 != 0,
            CONTROL_REGISTER => {
                if value & CONTROL_ALL_RESETS_MASK == CONTROL_ALL_RESETS_MASK {
                    self.controller_was_reset = true;
                }
            }
            _ => {}
        }
    }

    fn now_microseconds(&mut self) -> u64 {
        self.power_up_commands_since_go_idle * 2 * crate::sd::POWER_UP_DEADLINE_MICROSECONDS
    }
}

/// **A DMA region whose descriptors the IDMAC has closed**: every word reads zero, so a chain is
/// done at the first look, and each access asserts it is inside the region, as
/// [`AdversarialRegion`]'s do. The data the card sends is not what these harnesses are about.
struct ClosedRegion {
    physical_address: u64,
}

impl DirectMemoryAccessRegion for ClosedRegion {
    fn direct_memory_access_address(&self) -> u64 {
        self.physical_address
    }

    fn bytes(&self) -> u32 {
        3 * PAGE_BYTES
    }

    fn read_u32(&self, offset: u32) -> u32 {
        assert!(u64::from(offset) + 4 <= u64::from(self.bytes()));
        0
    }

    fn write_u32(&mut self, offset: u32, _value: u32) {
        assert!(u64::from(offset) + 4 <= u64::from(self.bytes()));
    }

    fn read_bytes(&self, offset: u32, out: &mut [u8]) {
        assert!(u64::from(offset) + out.len() as u64 <= u64::from(self.bytes()));
    }

    fn write_bytes(&mut self, offset: u32, source: &[u8]) {
        assert!(u64::from(offset) + source.len() as u64 <= u64::from(self.bytes()));
    }

    fn barrier(&mut self) {}
}

/// A host over a [`ModeledCard`] whose data phases go through the IDMAC, over a
/// [`ClosedRegion`], so an eMMC's `EXT_CSD` read is one descriptor rather than a FIFO loop.
/// The FIFO is the JH7110's, 32 words deep; bring-up does not depend on its depth.
fn modeled_card_host() -> HostController<ModeledCard, ClosedRegion> {
    let Ok((host, identity)) = HostController::new(
        ModeledCard::new(),
        crate::jh7110::CARD_INTERFACE_UNIT_HERTZ,
        Some(crate::jh7110::QUEUE_DEPTH_WORDS),
    ) else {
        kani::assume(false);
        unreachable!()
    };
    let physical_address: u64 = kani::any();
    let Ok(engine) = InternalDirectMemoryAccess::new(ClosedRegion { physical_address }, &identity)
    else {
        kani::assume(false);
        unreachable!()
    };
    host.with_internal_direct_memory_access(engine)
}

/// **Each bring-up command goes to a card state that accepts it**: whatever state the card starts
/// in and whatever it and the controller answer, every command `identify` sends is legal in the
/// state the card is in, as [`ModeledCard`] judges it, and none is a write. So no step runs before
/// the one it needs, an application command always follows `CMD55`, and `CMD9` and `CMD7` go to the
/// address the card holds.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.each_bring_up_command_goes_to_a_card_state_that_accepts_it.patch`
#[kani::proof]
#[kani::unwind(7)]
fn each_bring_up_command_goes_to_a_card_state_that_accepts_it() {
    let mut host = modeled_card_host();
    let _ = crate::sd::identify(&mut host, kani::any());
    let card = host.registers_mut();
    assert!(card.illegal_commands == 0);
    assert!(card.write_commands == 0);
    kani::cover!(card.state == CardState::Transfer);
}

/// **A card is brought up only through every step**: when `identify` hands back a card, the card
/// went from `CMD0` through ready, identification and standby to transfer, holds the relative
/// address the `Card` names, reported the OCR (operation conditions register) the `Card` holds,
/// and is the kind the `Card` says. An SD card has its block length set to 512 bytes and the bus
/// width the slot allows, an eMMC has had its `EXT_CSD` read on a 1-bit bus, and the controller
/// runs the transfer clock.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.a_card_is_brought_up_only_through_every_step.patch`
#[kani::proof]
#[kani::unwind(7)]
fn a_card_is_brought_up_only_through_every_step() {
    let mut host = modeled_card_host();
    let maximum_bus_width: u8 = kani::any();
    let result = crate::sd::identify(&mut host, maximum_bus_width);
    let model = host.registers_mut();
    if let Ok(card) = result {
        assert!(model.state == CardState::Transfer);
        assert!(model.passed == PASSED_EVERY_STATE);
        assert!(card.relative_card_address == model.relative_card_address);
        assert!(card.operation_conditions == model.ready_operation_conditions);
        assert!((card.kind == crate::sd::CardKind::MultimediaCard) == model.multimedia_card);
        assert!(card.bus_width == model.card_bus_width);
        if model.multimedia_card {
            assert!(model.extended_card_specific_data_read);
            assert!(card.bus_width == 1);
        } else {
            assert!(model.block_bytes_set);
            assert!(card.bus_width == if maximum_bus_width >= 4 { 4 } else { 1 });
        }
        assert!(
            model.clock_divider
                == u32::from(
                    clock_divider(
                        crate::jh7110::CARD_INTERFACE_UNIT_HERTZ,
                        crate::sd::TRANSFER_HERTZ
                    )
                    .unwrap_or(255)
                )
        );
    }
    kani::cover!(result.is_ok());
}

/// **A bring-up starts over from whatever a failed one left**: from any card state and any
/// controller bus width and clock, `identify` resets the controller, powers the slot, drops it to
/// a 1-bit bus at the identification clock and sends `CMD0` before any other command. A card it
/// hands back runs the same bus width on the card, on the controller and in the `Card`, so an error
/// at any step leaves nothing half brought up that the next bring-up inherits.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.a_bring_up_starts_over_from_whatever_a_failed_one_left.patch`
#[kani::proof]
#[kani::unwind(7)]
fn a_bring_up_starts_over_from_whatever_a_failed_one_left() {
    let mut host = modeled_card_host();
    let initial_controller_bus_width = host.registers_mut().controller_bus_width;
    let result = crate::sd::identify(&mut host, kani::any());
    let model = host.registers_mut();
    if let Some(first) = model.first_command {
        assert!(first.index == crate::command::GO_IDLE_STATE);
        assert!(first.controller_was_reset && first.powered && first.clock_enabled);
        assert!(first.controller_bus_width == 1);
        assert!(
            first.clock_divider
                == u32::from(
                    clock_divider(
                        crate::jh7110::CARD_INTERFACE_UNIT_HERTZ,
                        crate::sd::IDENTIFY_HERTZ
                    )
                    .unwrap_or(255)
                )
        );
    }
    if let Ok(card) = result {
        assert!(model.controller_bus_width == card.bus_width);
        assert!(model.card_bus_width == card.bus_width);
    }
    kani::cover!(result.is_ok() && initial_controller_bus_width != 1);
}

/// **A card that keeps reporting busy at a clock that runs forward from any start, at any pace.**
/// It answers every command at once and never holds DAT0 (data line 0) busy. An SD card answers
/// `CMD55` and `ACMD41`; an eMMC is silent to `CMD55` and answers `CMD0` and `CMD1`. Every OCR it
/// reports has the busy bit clear. The clock never goes backward, and from reading `past_from` on
/// each reading is more than the one-second power-up deadline past the one before. It counts the
/// power-up commands (`ACMD41` and `CMD1`) sent after that reading.
struct BusyForeverCard {
    multimedia_card: bool,
    index: u8,
    now: u64,
    past_from: u32,
    taken: u32,
    power_up_commands: u32,
    power_up_commands_after_the_deadline: u32,
}

impl RegisterWindowAndClock for BusyForeverCard {
    fn read(&mut self, offset: u32) -> u32 {
        use crate::registers::{
            COMMAND_REGISTER, RAW_INTERRUPT_COMMAND_DONE, RAW_INTERRUPT_RESPONSE_TIMEOUT,
            RAW_INTERRUPT_STATUS_REGISTER, RESPONSE_0_REGISTER, STATUS_REGISTER,
        };
        match offset {
            COMMAND_REGISTER | STATUS_REGISTER => 0,
            RAW_INTERRUPT_STATUS_REGISTER => {
                if self.multimedia_card && self.index == crate::command::APPLICATION_COMMAND {
                    RAW_INTERRUPT_COMMAND_DONE | RAW_INTERRUPT_RESPONSE_TIMEOUT
                } else {
                    RAW_INTERRUPT_COMMAND_DONE
                }
            }
            RESPONSE_0_REGISTER => {
                kani::any::<u32>() & !crate::card::OPERATION_CONDITIONS_BUSY_DONE
            }
            _ => kani::any(),
        }
    }

    fn write(&mut self, offset: u32, value: u32) {
        use crate::registers::{COMMAND_REGISTER, COMMAND_START};
        if offset == COMMAND_REGISTER && value & COMMAND_START != 0 {
            self.index = (value & 0x3f) as u8;
            if self.index == crate::command::SD_SEND_OPERATION_CONDITIONS
                || self.index == crate::command::SEND_OPERATION_CONDITIONS
            {
                self.power_up_commands += 1;
                if self.taken > self.past_from {
                    self.power_up_commands_after_the_deadline += 1;
                    assert!(
                        self.power_up_commands_after_the_deadline <= 1,
                        "power-up asked the card again after its deadline had passed"
                    );
                }
            }
        }
    }

    fn now_microseconds(&mut self) -> u64 {
        let step: u64 = kani::any();
        if self.taken >= self.past_from {
            kani::assume(step > crate::sd::POWER_UP_DEADLINE_MICROSECONDS);
        }
        kani::assume(step < 1 << 40);
        self.now += step;
        self.taken += 1;
        self.now
    }
}

/// **A card that keeps reporting busy is given up on once the deadline passes**: against a card
/// whose every `ACMD41` or `CMD1` reports busy, power-up repeats the command while the clock is
/// inside one second of the first try and ends with `PowerUp` and the last OCR, whatever the
/// clock did before then. It sends at most one more power-up command once the clock has passed
/// the deadline, so it never loops forever on a busy card. An eMMC is tried only for a card that
/// did not echo `CMD8`.
///
/// Falsification: replayable `crates/designware_mobile_storage/falsifications/proofs.a_card_that_keeps_reporting_busy_is_given_up_on_once_the_deadline_passes.patch`
#[kani::proof]
#[kani::unwind(8)]
fn a_card_that_keeps_reporting_busy_is_given_up_on_once_the_deadline_passes() {
    let past_from: u32 = kani::any();
    kani::assume((1..=16).contains(&past_from));
    let start: u64 = kani::any();
    kani::assume(start < 1 << 62);
    let multimedia_card: bool = kani::any();
    let Ok((mut host, _identity)) = HostController::new(
        BusyForeverCard {
            multimedia_card,
            index: 0,
            now: start,
            past_from,
            taken: 0,
            power_up_commands: 0,
            power_up_commands_after_the_deadline: 0,
        },
        crate::jh7110::CARD_INTERFACE_UNIT_HERTZ,
        Some(32),
    ) else {
        kani::assume(false);
        unreachable!()
    };
    let is_version_2_or_later: bool = kani::any();
    let result = crate::sd::power_up(&mut host, is_version_2_or_later);
    let card = host.registers_mut();
    if multimedia_card && is_version_2_or_later {
        assert!(matches!(
            result,
            Err(crate::sd::InitializationError::NoCard(_))
        ));
    } else {
        assert!(matches!(
            result,
            Err(crate::sd::InitializationError::PowerUp(operation_conditions))
                if operation_conditions & crate::card::OPERATION_CONDITIONS_BUSY_DONE == 0
        ));
        assert!(card.power_up_commands >= 1);
    }
    kani::cover!(card.power_up_commands >= 2);
}
