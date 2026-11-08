//! The internal DMA controller (IDMAC): it moves a data phase between the FIFO and RAM by walking a
//! descriptor chain, so the CPU copies each byte once in cached memory, not word by word over MMIO.
//! The region is a page of descriptors, then data pages, one descriptor each, so a transfer is at
//! most the region's pages; reads are copied out, writes in. Every transfer arms the IDMAC from the
//! polled state [`HostController::reset`] leaves and, finished or failed, stops and resets it, so no engine
//! walks a chain the caller reused. The databook's sequence, as OpenBSD's `dwmmc_dma_setup` and
//! Linux's `dw_mci_idmac_start_dma` use it: reset the IDMAC, point `DBADDR` at the chain, select it
//! in `CONTROL_REGISTER`, enable it in `BUS_MODE_REGISTER`, send the command. Polled completion: `RINTSTS` for the card, the
//! last descriptor's `OWN` for memory, since a read's `DTO` can beat its last words to memory.
//!
//! Name: ratified 2026-11-02 (UTC), #3092. `idmac` and the names built on it spell out
//! `internal_direct_memory_access`; the databook's `idmac` is kept as `doc(alias = "idmac")`.

use crate::command::Command;
use crate::host_controller::{
    COMMAND_DEADLINE_MICROSECONDS, DATA_DEADLINE_MICROSECONDS, Error, HostController, Identity,
    RegisterWindowAndClock,
};
use crate::registers;

/// **The DMA region**, as the driver reads and writes it. Offsets are bytes from the region's
/// first byte; an implementation bounds them. A kernel's might be pages in its direct map; the tests'
/// is memory `simulator` walks too. `designware_ethernet::DirectMemoryAccessRegion`'s shape.
///
/// Name: ratified 2026-11-02 (UTC), #3092. The region the descriptors and data pages live in,
/// spelled out.
pub trait DirectMemoryAccessRegion {
    /// The region's first byte as the controller's direct memory access engine addresses it: on the
    /// VisionFive 2, its physical address. Behind an address translation unit or a bus offset it
    /// differs from the processor's physical address, so it does not take that name.
    ///
    /// Name: provisional (the maintainer), a two-way door before the first release.
    /// Renamed from `bus_address`, which a processor-physical `physical_address` would misname. The
    /// aliases are the plain term and `dma_addr_t`, the Linux type for the same thing.
    #[doc(alias = "bus address")]
    #[doc(alias = "dma_addr_t")]
    fn direct_memory_access_address(&self) -> u64;
    /// The region's length in bytes.
    fn bytes(&self) -> u32;
    /// Read the little-endian `u32` at `offset`.
    fn read_u32(&self, offset: u32) -> u32;
    /// Write `value` at `offset`, little-endian.
    fn write_u32(&mut self, offset: u32, value: u32);
    /// Copy `out.len()` bytes starting at `offset` into `out`.
    fn read_bytes(&self, offset: u32, out: &mut [u8]);
    /// Copy `source` to `offset`.
    fn write_bytes(&mut self, offset: u32, source: &[u8]);
    /// Order every region access before this against every access after it, register accesses
    /// included, as the controller sees them. Called after a chain is written and before the
    /// register writes that start it, and after its end is seen and before the data is read.
    fn barrier(&mut self);
}

/// **No region**: a [`HostController`] that moves its data through the FIFO. Uninhabited, so a host of this
/// type provably never takes the IDMAC path.
///
/// Name: ratified 2026-11-02 (UTC), #3092. The type for a host with no direct memory access,
/// spelled out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoDirectMemoryAccess {}

impl DirectMemoryAccessRegion for NoDirectMemoryAccess {
    fn direct_memory_access_address(&self) -> u64 {
        match *self {}
    }
    fn bytes(&self) -> u32 {
        match *self {}
    }
    fn read_u32(&self, _: u32) -> u32 {
        match *self {}
    }
    fn write_u32(&mut self, _: u32, _: u32) {
        match *self {}
    }
    fn read_bytes(&self, _: u32, _: &mut [u8]) {
        match *self {}
    }
    fn write_bytes(&mut self, _: u32, _: &[u8]) {
        match *self {}
    }
    fn barrier(&mut self) {
        match *self {}
    }
}

/// A page: the descriptor chain's size, and the most one descriptor's buffer holds here. The
/// databook's field holds 8191 bytes; a page keeps each buffer inside one page of the region.
pub(crate) const PAGE_BYTES: u32 = 4096;
/// One 32-bit-address descriptor: four words.
pub(crate) const DESCRIPTOR_BYTES: u32 = 16;
/// The most descriptors the chain page holds, and so the most data pages a region may have.
///
/// Name: ratified 2026-11-02 (UTC), #3092. The maximum is spelled out.
pub const MAXIMUM_DESCRIPTORS: u32 = PAGE_BYTES / DESCRIPTOR_BYTES;

/// **Own, descriptor word 0** (OpenBSD's `DES0_OWN`): the IDMAC owns this descriptor. Cleared by
/// the IDMAC when it closes it.
#[doc(alias = "owned by the controller")]
#[doc(alias = "DES0_OWN")]
pub(crate) const DESCRIPTOR_OWNED_BY_CONTROLLER: u32 = 1 << 31;
/// **Second address chained, descriptor word 0** (OpenBSD's `DES0_CH`): word 3 holds the next
/// descriptor's address, not a second buffer.
#[doc(alias = "chained")]
#[doc(alias = "DES0_CH")]
pub(crate) const DESCRIPTOR_CHAINED: u32 = 1 << 4;
/// **First descriptor, descriptor word 0** (OpenBSD's `DES0_FS`): the first descriptor of a
/// transfer.
#[doc(alias = "first descriptor")]
#[doc(alias = "DES0_FS")]
pub(crate) const DESCRIPTOR_FIRST: u32 = 1 << 3;
/// **Last descriptor, descriptor word 0** (OpenBSD's `DES0_LD`): the last descriptor of a transfer.
/// The IDMAC stops after it.
#[doc(alias = "last descriptor")]
#[doc(alias = "DES0_LD")]
pub(crate) const DESCRIPTOR_LAST: u32 = 1 << 2;
/// **Disable interrupt on completion, descriptor word 0** (OpenBSD's `DES0_DIC`): do not raise
/// `TI`/`RI` when this descriptor closes.
#[doc(alias = "no completion interrupt")]
#[doc(alias = "DES0_DIC")]
pub(crate) const DESCRIPTOR_DISABLE_INTERRUPT_ON_COMPLETION: u32 = 1 << 1;
/// Word 1: the first buffer's size in bytes, bits 12:0.
pub(crate) const DESCRIPTOR_BUFFER_1_BYTES_MASK: u32 = 0x1fff;
// The two word-0 bits every descriptor is written with share no bit (31 and 4), which the compiler
// checks here; the mutation exclusion for `descriptor` names this check. The constant-assertion
// lint is off on purpose: the assertion is constant by construction, and the compiler checking it
// is its job.
#[allow(clippy::assertions_on_constants, clippy::eq_op)]
const _: () = assert!(DESCRIPTOR_OWNED_BY_CONTROLLER + DESCRIPTOR_CHAINED == 0x8000_0010);

/// Why a region was refused before anything was written.
///
/// Name: ratified 2026-11-02 (UTC), #3092. Why a region is refused for the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refusal {
    /// `HCON` names an external DMA interface, so there is no IDMAC: the field.
    #[doc(alias = "idmac")]
    NoInternalDirectMemoryAccess(u32),
    /// `HCON` says the IDMAC uses 64-bit addresses, whose descriptors this driver does not build.
    Addressing64,
    /// The region does not start on a page.
    Alignment(u64),
    /// The region is not a descriptor page and 1 to [`MAXIMUM_DESCRIPTORS`] data pages: its length.
    Size(u32),
    /// Part of the region lies above 4 GiB, where a 32-bit descriptor cannot point: its address.
    ///
    /// Name: ratified 2026-11-07 (UTC). Considered: `Above4GiB`, which put a unit symbol inside a
    /// name; units are spelled out in any position, so the symbol fell to that rule.
    AboveFourGibibytes(u64),
}

impl core::fmt::Display for Refusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoInternalDirectMemoryAccess(field) => {
                write!(
                    f,
                    "the controller has no internal DMA controller (field {field})"
                )
            }
            Self::Addressing64 => f.write_str("64-bit descriptors are not supported"),
            Self::Alignment(base) => write!(f, "the region at {base:#x} does not start on a page"),
            Self::Size(length) => {
                write!(
                    f,
                    "a region of {length} bytes is not a descriptor page and data pages"
                )
            }
            Self::AboveFourGibibytes(base) => {
                write!(f, "the region at {base:#x} reaches above 4 GiB")
            }
        }
    }
}

impl core::error::Error for Refusal {}

/// **A region the IDMAC can use**, checked against the controller once. Only
/// [`InternalDirectMemoryAccess::new`] makes one, so a [`HostController`] holding one has an IDMAC, 32-bit
/// descriptors and a region they can address.
///
/// Name: ratified 2026-11-02 (UTC), #3092. `idmac` and the names built on it spell out
/// `internal_direct_memory_access`; the databook's `idmac` is kept as `doc(alias = "idmac")`.
#[doc(alias = "idmac")]
pub struct InternalDirectMemoryAccess<D: DirectMemoryAccessRegion> {
    region: D,
    region_physical_address: u32,
    capacity: u32,
}

impl<D: DirectMemoryAccessRegion> InternalDirectMemoryAccess<D> {
    /// **Check `region` against the controller `id` describes.**
    ///
    /// # Errors
    ///
    /// A [`Refusal`]: no IDMAC, 64-bit addressing, or a region that is unaligned, the wrong size,
    /// or not wholly below 4 GiB.
    ///
    /// Name: ratified 2026-11-02 (UTC), #3092. The constructor, which checks the region against the
    /// identity.
    pub fn new(region: D, id: &Identity) -> Result<InternalDirectMemoryAccess<D>, Refusal> {
        let interface = registers::direct_memory_access_interface(id.hardware_configuration);
        if interface != 0 {
            return Err(Refusal::NoInternalDirectMemoryAccess(interface));
        }
        if registers::uses_64_bit_direct_memory_access(id.hardware_configuration) {
            return Err(Refusal::Addressing64);
        }
        let base = region.direct_memory_access_address();
        if !base.is_multiple_of(u64::from(PAGE_BYTES)) {
            return Err(Refusal::Alignment(base));
        }
        let bytes = region.bytes();
        let pages = bytes / PAGE_BYTES;
        if !bytes.is_multiple_of(PAGE_BYTES) || pages < 2 || pages - 1 > MAXIMUM_DESCRIPTORS {
            return Err(Refusal::Size(bytes));
        }
        let Ok(region_physical_address) = u32::try_from(base) else {
            return Err(Refusal::AboveFourGibibytes(base));
        };
        if region_physical_address.checked_add(bytes - 1).is_none() {
            return Err(Refusal::AboveFourGibibytes(base));
        }
        Ok(InternalDirectMemoryAccess {
            region,
            region_physical_address,
            capacity: bytes - PAGE_BYTES,
        })
    }

    /// The most bytes one transfer moves: the region's data pages.
    #[must_use]
    pub(crate) fn capacity(&self) -> u32 {
        self.capacity
    }

    /// The region, for a caller that wants to inspect it.
    pub(crate) fn region(&mut self) -> &mut D {
        &mut self.region
    }

    /// Write the chain for `bytes`, every descriptor the IDMAC's, and nothing past its last.
    fn write_chain(&mut self, bytes: u32) {
        let count = descriptors_for(bytes);
        for i in 0..count {
            let d = descriptor(self.region_physical_address, i, count, bytes);
            for (w, word) in d.iter().enumerate() {
                self.region
                    .write_u32(i * DESCRIPTOR_BYTES + 4 * w as u32, *word);
            }
        }
    }

    /// How many descriptors of a chain of `count` the IDMAC still owns.
    fn owned(&self, count: u32) -> u32 {
        (0..count)
            .filter(|i| {
                self.region.read_u32(i * DESCRIPTOR_BYTES) & DESCRIPTOR_OWNED_BY_CONTROLLER != 0
            })
            .count() as u32
    }

    /// Bytes in descriptors the IDMAC still owns, of a chain of `count`: what it has not moved.
    fn owed(&self, count: u32) -> u32 {
        (0..count)
            .filter(|i| {
                self.region.read_u32(i * DESCRIPTOR_BYTES) & DESCRIPTOR_OWNED_BY_CONTROLLER != 0
            })
            .map(|i| {
                self.region.read_u32(i * DESCRIPTOR_BYTES + 4) & DESCRIPTOR_BUFFER_1_BYTES_MASK
            })
            .sum()
    }
}

/// How many descriptors a transfer of `bytes` takes: one a page, and at least one.
#[must_use]
pub(crate) const fn descriptors_for(bytes: u32) -> u32 {
    if bytes == 0 {
        1
    } else {
        bytes.div_ceil(PAGE_BYTES)
    }
}

/// **Descriptor `i` of `count`** for a transfer of `bytes`, in a region whose first byte is at
/// `region_physical_address` as the bus sees it: the chain page first, then the data pages in order. Every descriptor is the
/// IDMAC's and chained; the first carries `FS`, the last `LD` and the only completion status, and
/// each holds a page except the last, which holds what is left.
#[must_use]
pub(crate) const fn descriptor(
    region_physical_address: u32,
    i: u32,
    count: u32,
    bytes: u32,
) -> [u32; 4] {
    let last = i + 1 == count;
    let mut word_0 = DESCRIPTOR_OWNED_BY_CONTROLLER | DESCRIPTOR_CHAINED;
    if i == 0 {
        word_0 |= DESCRIPTOR_FIRST;
    }
    if last {
        word_0 |= DESCRIPTOR_LAST;
    } else {
        word_0 |= DESCRIPTOR_DISABLE_INTERRUPT_ON_COMPLETION;
    }
    let size = if last {
        bytes - i * PAGE_BYTES
    } else {
        PAGE_BYTES
    };
    let next = if last { 0 } else { i + 1 };
    [
        word_0,
        size,
        region_physical_address + PAGE_BYTES * (i + 1),
        region_physical_address + DESCRIPTOR_BYTES * next,
    ]
}

impl<R: RegisterWindowAndClock, D: DirectMemoryAccessRegion> HostController<R, D> {
    /// **Run `command`'s data phase through the IDMAC**, the data already in the region for a
    /// write. Stops and resets the IDMAC before returning, whatever happened.
    pub(crate) fn internal_direct_memory_access_transfer(
        &mut self,
        command: &Command,
        bytes: u32,
    ) -> Result<[u32; 4], Error> {
        let count = descriptors_for(bytes);
        if let Some(direct_memory_access) = self.direct_memory_access.as_mut() {
            direct_memory_access.write_chain(bytes);
            direct_memory_access.region.barrier();
        }
        let result = self.issue(command, true).and_then(|response| {
            self.internal_direct_memory_access_wait(command, count)
                .map(|()| response)
        });
        let stopped = self.internal_direct_memory_access_stop();
        let response = result?;
        stopped?;
        if let Some(direct_memory_access) = self.direct_memory_access.as_mut() {
            direct_memory_access.region.barrier();
        }
        Ok(response)
    }

    /// **Arm the IDMAC** on the chain at the region's start: called by `issue` once the FIFO
    /// reset has cleared and before the command is written, so the engine is ready for the first
    /// word. `IDINTEN` stays zero and `CONTROL_REGISTER`'s interrupt enable clear; `IDSTS` is raw.
    pub(crate) fn internal_direct_memory_access_start(&mut self) -> Result<(), Error> {
        let Some(region_physical_address) = self
            .direct_memory_access
            .as_ref()
            .map(|d| d.region_physical_address)
        else {
            return Ok(());
        };
        self.registers.write(
            registers::BUS_MODE_REGISTER,
            registers::BUS_MODE_SOFTWARE_RESET,
        );
        if !self.is_condition_met_within(COMMAND_DEADLINE_MICROSECONDS, |window| {
            window.read(registers::BUS_MODE_REGISTER) & registers::BUS_MODE_SOFTWARE_RESET == 0
        }) {
            return Err(Error::ResetStuck(
                self.registers.read(registers::BUS_MODE_REGISTER),
            ));
        }
        self.registers.write(
            registers::QUEUE_THRESHOLD_REGISTER,
            registers::queue_threshold_for_direct_memory_access(self.depth),
        );
        self.registers.write(
            registers::INTERNAL_DIRECT_MEMORY_ACCESS_INTERRUPT_ENABLE_REGISTER,
            0,
        );
        self.registers.write(
            registers::INTERNAL_DIRECT_MEMORY_ACCESS_STATUS_REGISTER,
            registers::INTERNAL_DIRECT_MEMORY_ACCESS_STATUS_ALL_MASK,
        );
        self.registers.write(
            registers::DESCRIPTOR_LIST_BASE_PHYSICAL_ADDRESS_REGISTER,
            region_physical_address,
        );
        let control = self.registers.read(registers::CONTROL_REGISTER);
        self.registers.write(
            registers::CONTROL_REGISTER,
            control
                | registers::CONTROL_USE_INTERNAL_DIRECT_MEMORY_ACCESS_CONTROLLER
                | registers::CONTROL_DIRECT_MEMORY_ACCESS_ENABLE,
        );
        self.registers.write(
            registers::BUS_MODE_REGISTER,
            registers::BUS_MODE_INTERNAL_DIRECT_MEMORY_ACCESS_ENABLE
                | registers::BUS_MODE_FIXED_BURST,
        );
        self.registers.write(registers::POLL_DEMAND_REGISTER, 1);
        Ok(())
    }

    /// Wait for the card's side (`DTO`, and `ACD` after more than one block) and the memory side
    /// (every descriptor closed). The deadline restarts whenever a descriptor closes, so a long
    /// transfer is judged on progress, not on its length. Progress is fewer owned descriptors than
    /// at any earlier look, so the deadline restarts at most `count` times: a controller that
    /// reopens a descriptor or rewrites its size cannot hold the wait open.
    pub(crate) fn internal_direct_memory_access_wait(
        &mut self,
        command: &Command,
        count: u32,
    ) -> Result<(), Error> {
        let want = match command.data {
            Some(d) if d.blocks > 1 => {
                registers::RAW_INTERRUPT_DATA_TRANSFER_OVER
                    | registers::RAW_INTERRUPT_AUTOMATIC_COMMAND_DONE
            }
            _ => registers::RAW_INTERRUPT_DATA_TRANSFER_OVER,
        };
        let Some(direct_memory_access) = self.direct_memory_access.as_ref() else {
            return Err(Error::BufferSize);
        };
        let mut owned = direct_memory_access.owned(count);
        let mut start = self.registers.now_microseconds();
        loop {
            let raw_interrupt_status = self
                .registers
                .read(registers::RAW_INTERRUPT_STATUS_REGISTER);
            if raw_interrupt_status
                & (registers::RAW_INTERRUPT_DATA_ERROR_MASK
                    | registers::RAW_INTERRUPT_DATA_TIMEOUT_MASK)
                != 0
            {
                return Err(Error::Data(command.index, raw_interrupt_status));
            }
            let internal_direct_memory_access_status = self
                .registers
                .read(registers::INTERNAL_DIRECT_MEMORY_ACCESS_STATUS_REGISTER);
            if internal_direct_memory_access_status
                & registers::INTERNAL_DIRECT_MEMORY_ACCESS_STATUS_ERROR_MASK
                != 0
            {
                return Err(Error::DirectMemoryAccess(
                    command.index,
                    internal_direct_memory_access_status,
                ));
            }
            let Some(direct_memory_access) = self.direct_memory_access.as_ref() else {
                return Err(Error::BufferSize);
            };
            let now_owned = direct_memory_access.owned(count);
            if raw_interrupt_status & want == want && now_owned == 0 {
                return Ok(());
            }
            let now = self.registers.now_microseconds();
            if now_owned < owned {
                owned = now_owned;
                start = now;
            } else if now.wrapping_sub(start) > DATA_DEADLINE_MICROSECONDS {
                let owed = direct_memory_access.owed(count);
                return Err(Error::DataTimeout(
                    command.index,
                    raw_interrupt_status,
                    owed,
                ));
            }
        }
    }

    /// **Stop the IDMAC** and hand the FIFO back to the CPU: disable it, deselect it, reset it and
    /// the FIFO, clear its status, and put back the polled thresholds. After this nothing the
    /// controller does can reach the region. `BUS_MODE_REGISTER` is written whole, as
    /// [`HostController::internal_direct_memory_access_start`] writes it: none of its other bits is the
    /// firmware's to keep. A reset that does not clear is reported with the `CONTROL_REGISTER` word if that is
    /// the one held, else the `BUS_MODE_REGISTER` word.
    fn internal_direct_memory_access_stop(&mut self) -> Result<(), Error> {
        self.registers.write(
            registers::BUS_MODE_REGISTER,
            registers::BUS_MODE_SOFTWARE_RESET,
        );
        let control = self.registers.read(registers::CONTROL_REGISTER)
            & !(registers::CONTROL_USE_INTERNAL_DIRECT_MEMORY_ACCESS_CONTROLLER
                | registers::CONTROL_DIRECT_MEMORY_ACCESS_ENABLE);
        self.registers.write(
            registers::CONTROL_REGISTER,
            control
                | registers::CONTROL_DIRECT_MEMORY_ACCESS_RESET
                | registers::CONTROL_QUEUE_RESET,
        );
        let quiet = self.is_condition_met_within(COMMAND_DEADLINE_MICROSECONDS, |window| {
            window.read(registers::CONTROL_REGISTER)
                & (registers::CONTROL_DIRECT_MEMORY_ACCESS_RESET | registers::CONTROL_QUEUE_RESET)
                == 0
                && window.read(registers::BUS_MODE_REGISTER) & registers::BUS_MODE_SOFTWARE_RESET
                    == 0
        });
        self.registers.write(
            registers::INTERNAL_DIRECT_MEMORY_ACCESS_STATUS_REGISTER,
            registers::INTERNAL_DIRECT_MEMORY_ACCESS_STATUS_ALL_MASK,
        );
        self.registers.write(
            registers::QUEUE_THRESHOLD_REGISTER,
            registers::queue_threshold_for_programmed_input_output(self.depth),
        );
        let control = self.registers.read(registers::CONTROL_REGISTER);
        if quiet {
            Ok(())
        } else if control
            & (registers::CONTROL_DIRECT_MEMORY_ACCESS_RESET | registers::CONTROL_QUEUE_RESET)
            != 0
        {
            Err(Error::ResetStuck(control))
        } else {
            Err(Error::ResetStuck(
                self.registers.read(registers::BUS_MODE_REGISTER),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::vec::Vec;

    use super::*;
    use crate::sd::{self, BlockTransferError, Card};
    use crate::simulator::{CardKind, Memory, SimulatedHostController};

    /// A bus address with high bits set, so an offset taken as an address shows.
    const BASE: u64 = 0x8000_0000;

    /// An identified SDHC card behind a polled host, and memory of `pages` pages the sim's IDMAC
    /// reaches.
    fn polled(pages: usize) -> (HostController<SimulatedHostController>, Card, Memory) {
        let mut simulator = SimulatedHostController::new(CardKind::SdHighCapacity, 4096, 32);
        let memory = Memory::new(BASE, pages);
        simulator.memory = Some(memory.clone());
        let (mut h, _) = HostController::new(simulator, 50_000_000, Some(32)).unwrap();
        let card = sd::identify(&mut h, 4).unwrap();
        (h, card, memory)
    }

    fn with_internal_direct_memory_access(
        h: HostController<SimulatedHostController>,
        memory: &Memory,
    ) -> HostController<SimulatedHostController, Memory> {
        let mut h = h;
        let hardware_configuration = h
            .registers_mut()
            .peek(registers::HARDWARE_CONFIGURATION_REGISTER);
        let id = Identity {
            version_identification: 0,
            hardware_configuration,
            user_identification: 0,
            queue_threshold: 0,
            card_detect: 0,
            firmware: [0; 5],
        };
        h.with_internal_direct_memory_access(
            InternalDirectMemoryAccess::new(memory.clone(), &id).unwrap(),
        )
    }

    fn polled_again(
        h: HostController<SimulatedHostController, Memory>,
    ) -> HostController<SimulatedHostController> {
        HostController::new(h.into_registers(), 50_000_000, Some(32))
            .unwrap()
            .0
    }

    fn pattern(seed: u8, len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8 ^ seed).collect()
    }

    /// **Every descriptor bit, as a number, and a chain as the databook lays it out.** The driver
    /// and `simulator` share these constants, so a wrong one would agree with itself in every walk; only
    /// the databook's values catch it. 4608 bytes is a full page and a 512-byte tail.
    #[test]
    fn the_descriptor_bits_and_a_two_page_chain_are_the_databooks() {
        assert_eq!(
            [
                DESCRIPTOR_OWNED_BY_CONTROLLER,
                DESCRIPTOR_CHAINED,
                DESCRIPTOR_FIRST,
                DESCRIPTOR_LAST,
                DESCRIPTOR_DISABLE_INTERRUPT_ON_COMPLETION,
                DESCRIPTOR_BUFFER_1_BYTES_MASK
            ],
            [0x8000_0000, 0x10, 0x8, 0x4, 0x2, 0x1fff]
        );
        let b = BASE as u32;
        assert_eq!(descriptors_for(4608), 2);
        assert_eq!(
            descriptor(b, 0, 2, 4608),
            [0x8000_001a, 4096, b + 0x1000, b + 0x10]
        );
        assert_eq!(descriptor(b, 1, 2, 4608), [0x8000_0014, 512, b + 0x2000, b]);
        // One block: first and last at once, and the only one that reports.
        assert_eq!(descriptor(b, 0, 1, 512), [0x8000_001c, 512, b + 0x1000, b]);
    }

    /// The 4 GiB edge is the one an 8 GB VisionFive 2 could reach: a region ending exactly there is
    /// usable, one a page later is not, and neither is anything a 32-bit descriptor cannot hold.
    #[test]
    fn a_region_the_internal_direct_memory_access_engine_cannot_reach_or_describe_is_refused() {
        let id = |hardware_configuration| Identity {
            version_identification: 0,
            hardware_configuration,
            user_identification: 0,
            queue_threshold: 0,
            card_detect: 0,
            firmware: [0; 5],
        };
        let visionfive_2 = id(0x00c4_3cc1);
        let new = |base, pages| {
            InternalDirectMemoryAccess::new(Memory::new(base, pages), &visionfive_2)
                .map(|i| i.capacity())
        };
        assert_eq!(new(0xffff_e000, 2), Ok(PAGE_BYTES));
        assert_eq!(
            new(0xffff_f000, 2),
            Err(Refusal::AboveFourGibibytes(0xffff_f000))
        );
        assert_eq!(
            new(0x1_0000_0000, 2),
            Err(Refusal::AboveFourGibibytes(0x1_0000_0000))
        );
        assert_eq!(new(BASE + 8, 2), Err(Refusal::Alignment(BASE + 8)));
        assert_eq!(new(BASE, 1), Err(Refusal::Size(PAGE_BYTES)));
        assert_eq!(new(BASE, 258), Err(Refusal::Size(258 * PAGE_BYTES)));
        assert_eq!(new(BASE, 257), Ok(256 * PAGE_BYTES));
        let m = Memory::new(BASE, 2);
        assert!(matches!(
            InternalDirectMemoryAccess::new(m.clone(), &id(2 << 16)),
            Err(Refusal::NoInternalDirectMemoryAccess(2))
        ));
        assert!(matches!(
            InternalDirectMemoryAccess::new(m, &id(1 << 27)),
            Err(Refusal::Addressing64)
        ));
    }

    /// **Written one way, read the other.** A round trip through one path would pass with both
    /// halves wrong the same way (an offset, a byte order); the FIFO path is tested against the
    /// card on its own, so it is the reference here. Nine blocks leave the last descriptor a
    /// 512-byte tail. Every transfer is fenced on both sides and leaves every descriptor closed.
    #[test]
    fn a_chain_moves_the_cards_bytes_in_both_directions_as_the_queue_does() {
        let (mut h, card, memory) = polled(4);
        let first = pattern(0x5a, 9 * 512);
        sd::write_blocks(&mut h, &card, 100, &first).unwrap();
        let mut h = with_internal_direct_memory_access(h, &memory);
        let mut back = vec![0u8; 9 * 512];
        sd::read_blocks(&mut h, &card, 100, &mut back).unwrap();
        assert_eq!(back, first);
        let second = pattern(0xa5, 9 * 512);
        sd::write_blocks(&mut h, &card, 300, &second).unwrap();
        for i in 0..2 {
            assert_eq!(
                memory.read_u32(i * DESCRIPTOR_BYTES) & DESCRIPTOR_OWNED_BY_CONTROLLER,
                0
            );
        }
        let bus = memory.bus();
        assert!(!bus.unfenced_start && !bus.unfenced_read);
        assert_eq!(bus.device_written, 9 * 512);
        drop(bus);
        let mut h = polled_again(h);
        sd::read_blocks(&mut h, &card, 300, &mut back).unwrap();
        assert_eq!(back, second);
        assert!(!h.registers_mut().stray_queue_write);
    }

    /// On a slow bus a read's `DTO` lands while the IDMAC still has words to write. A driver that
    /// stopped at `DTO` would hand back the memory's previous bytes; this one waits for the last
    /// descriptor to close.
    #[test]
    fn a_read_is_over_when_its_last_descriptor_closes_not_at_data_transfer_over() {
        let (mut h, card, memory) = polled(3);
        let data = pattern(0x33, 16 * 512);
        sd::write_blocks(&mut h, &card, 40, &data).unwrap();
        let mut h = with_internal_direct_memory_access(h, &memory);
        h.registers_mut().direct_memory_access_words_per_step = Some(1);
        let mut back = vec![0u8; 16 * 512];
        sd::read_blocks(&mut h, &card, 40, &mut back).unwrap();
        assert_eq!(back, data);
    }

    /// **A transfer longer than the deadline is not given up on while it progresses**: the
    /// deadline restarts each time a descriptor closes. Moving one word per IDMAC step, with the
    /// clock advanced 700 microseconds a reading, each of the two pages closes inside the
    /// half-second deadline while the whole transfer takes longer than it. The simulator puts the
    /// working range at about 500 to 900 microseconds: below it the whole transfer fits the
    /// deadline and proves nothing, above it a single page does not.
    #[test]
    fn a_transfer_that_keeps_closing_descriptors_outlasts_the_deadline() {
        let (mut h, card, memory) = polled(3);
        let data = pattern(0x5a, 16 * 512);
        sd::write_blocks(&mut h, &card, 80, &data).unwrap();
        let mut h = with_internal_direct_memory_access(h, &memory);
        h.registers_mut().direct_memory_access_words_per_step = Some(1);
        h.registers_mut().clock_step = 700;
        let start = h.registers_mut().now_microseconds();
        let mut back = vec![0u8; 16 * 512];
        sd::read_blocks(&mut h, &card, 80, &mut back).unwrap();
        assert_eq!(back, data);
        assert!(h.registers_mut().now_microseconds() - start > DATA_DEADLINE_MICROSECONDS);
    }

    /// One more block than the region holds is refused before anything is sent, rather than
    /// written past the region's end; exactly what it holds is moved.
    #[test]
    fn a_transfer_larger_than_the_region_is_refused_unsent() {
        let (h, card, memory) = polled(3);
        let mut h = with_internal_direct_memory_access(h, &memory);
        let full = pattern(0x42, 16 * 512);
        sd::write_blocks(&mut h, &card, 0, &full).unwrap();
        let mut back = vec![0u8; 16 * 512];
        sd::read_blocks(&mut h, &card, 0, &mut back).unwrap();
        assert_eq!(back, full);
        h.registers_mut().commands.clear();
        let mut big = vec![0u8; 17 * 512];
        assert_eq!(
            sd::read_blocks(&mut h, &card, 0, &mut big),
            Err(BlockTransferError::Host(Error::BufferSize))
        );
        assert_eq!(
            sd::write_blocks(&mut h, &card, 0, &big),
            Err(BlockTransferError::Host(Error::BufferSize))
        );
        assert!(h.registers_mut().commands.is_empty());
    }

    /// **Every way a transfer fails stops the engine before the driver returns**: a bus error and a
    /// descriptor the IDMAC does not own (`IDSTS`), and a data CRC error (`RINTSTS`); the stall is
    /// the next test's. Afterwards the IDMAC is deselected and disabled, the FIFO is the CPU's
    /// again, a card that resumes moves nothing into memory, and the next transfer works.
    #[test]
    fn every_failed_transfer_stops_the_internal_direct_memory_access_engine_and_the_next_one_works()
    {
        type Inject = fn(&mut SimulatedHostController);
        type Expect = fn(&Error) -> bool;
        let cases: [(Inject, Expect); 3] = [
            (
                |s| s.bus_error_at = Some(1),
                |e| matches!(e, Error::DirectMemoryAccess(18, s) if s & registers::INTERNAL_DIRECT_MEMORY_ACCESS_STATUS_FATAL_BUS_ERROR != 0),
            ),
            (
                |s| s.unowned_at = Some(1),
                |e| matches!(e, Error::DirectMemoryAccess(18, s) if s & registers::INTERNAL_DIRECT_MEMORY_ACCESS_STATUS_DESCRIPTOR_UNAVAILABLE != 0),
            ),
            (
                |s| s.fail_next_read_crc = true,
                |e| matches!(e, Error::Data(18, r) if r & registers::RAW_INTERRUPT_DATA_CRC_ERROR != 0),
            ),
        ];
        for (inject, expected) in cases {
            let (mut h, card, memory) = polled(3);
            let data = pattern(0x77, 9 * 512);
            sd::write_blocks(&mut h, &card, 60, &data).unwrap();
            let mut h = with_internal_direct_memory_access(h, &memory);
            inject(h.registers_mut());
            let mut back = vec![0u8; 9 * 512];
            match sd::read_blocks(&mut h, &card, 60, &mut back) {
                Err(BlockTransferError::Host(e)) => assert!(expected(&e), "{e:?}"),
                other => panic!("{other:?}"),
            }
            let simulator = h.registers_mut();
            let direct_memory_access =
                registers::CONTROL_USE_INTERNAL_DIRECT_MEMORY_ACCESS_CONTROLLER
                    | registers::CONTROL_DIRECT_MEMORY_ACCESS_ENABLE;
            assert_eq!(
                simulator.peek(registers::CONTROL_REGISTER) & direct_memory_access,
                0
            );
            assert_eq!(
                simulator.peek(registers::BUS_MODE_REGISTER)
                    & registers::BUS_MODE_INTERNAL_DIRECT_MEMORY_ACCESS_ENABLE,
                0
            );
            assert_eq!(
                simulator.peek(registers::QUEUE_THRESHOLD_REGISTER),
                registers::queue_threshold_for_programmed_input_output(32)
            );
            simulator.stall_after = None;
            simulator.bus_error_at = None;
            simulator.unowned_at = None;
            let written = memory.bus().device_written;
            for _ in 0..64 {
                simulator.read(registers::STATUS_REGISTER);
            }
            assert_eq!(memory.bus().device_written, written);
            sd::read_blocks(&mut h, &card, 60, &mut back).unwrap();
            assert_eq!(back, data);
        }
    }

    /// **A card that stalls is given up on once the deadline has passed**, owing what the
    /// descriptors it still owns describe, with the engine stopped. A nine-block read is a page and
    /// a 512-byte tail: a stall inside the page owes both (4608 bytes); a stall once the page is
    /// done owes the tail alone (512), so only the descriptors still owned are counted, each by its
    /// own size. A clock that advances seven microseconds a read never lands on the deadline
    /// exactly, so the give-up test has to be a comparison past it. On a worker, so a wait that
    /// never gives up is a named failure rather than a hung run.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "a ten-second wall-clock watchdog; the interpreted spin to the simulated deadline outlasts it"
    )]
    fn a_stalled_transfer_gives_up_once_the_deadline_has_passed() {
        assert_ne!(DATA_DEADLINE_MICROSECONDS % 7, 0);
        for (stall_after, owed) in [(512, 4608), (4096, 512)] {
            let (h, card, memory) = polled(3);
            let mut h = with_internal_direct_memory_access(h, &memory);
            h.registers_mut().stall_after = Some(stall_after);
            h.registers_mut().clock_step = 7;
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let mut back = vec![0u8; 9 * 512];
                let r = sd::read_blocks(&mut h, &card, 60, &mut back);
                let _ = sender.send((r, h.registers_mut().peek(registers::BUS_MODE_REGISTER)));
            });
            let (r, bus_mode) = receiver
                .recv_timeout(std::time::Duration::from_secs(10))
                .expect("the deadline never fired");
            assert!(
                matches!(r, Err(BlockTransferError::Host(Error::DataTimeout(18, _, o))) if o == owed),
                "stalled after {stall_after}: {r:?}"
            );
            assert_eq!(
                bus_mode & registers::BUS_MODE_INTERNAL_DIRECT_MEMORY_ACCESS_ENABLE,
                0
            );
        }
    }

    /// **The register contract of one transfer**, write by write, on a controller whose firmware
    /// left `CONTROL_REGISTER` bit 8 and the IDMAC selected (the VisionFive 2's U-Boot hands over `CONTROL_REGISTER` bit 25 set).
    /// Arming keeps the firmware's bit and sets both DMA bits whatever they were; `BUS_MODE_REGISTER` is
    /// written whole; stopping clears both DMA bits, resets the DMA interface and the FIFO, and
    /// puts the polled thresholds back. A single block finishes on `DTO` alone.
    #[test]
    fn one_transfer_arms_and_stops_the_internal_direct_memory_access_register_by_register() {
        let (mut h, card, memory) = polled(2);
        let firmware = 1 << 8
            | registers::CONTROL_USE_INTERNAL_DIRECT_MEMORY_ACCESS_CONTROLLER
            | registers::CONTROL_DIRECT_MEMORY_ACCESS_ENABLE;
        h.registers_mut()
            .write(registers::CONTROL_REGISTER, firmware);
        let mut h = with_internal_direct_memory_access(h, &memory);
        h.registers_mut().writes.clear();
        let mut block = [0u8; 512];
        sd::read_blocks(&mut h, &card, 7, &mut block).unwrap();
        let writes = &h.registers_mut().writes;
        let from = |w: (u32, u32)| writes.iter().position(|&x| x == w).unwrap();
        let armed = from((
            registers::BUS_MODE_REGISTER,
            registers::BUS_MODE_SOFTWARE_RESET,
        ));
        assert_eq!(
            writes[armed..armed + 8],
            [
                (
                    registers::BUS_MODE_REGISTER,
                    registers::BUS_MODE_SOFTWARE_RESET
                ),
                (
                    registers::QUEUE_THRESHOLD_REGISTER,
                    registers::queue_threshold_for_direct_memory_access(32)
                ),
                (
                    registers::INTERNAL_DIRECT_MEMORY_ACCESS_INTERRUPT_ENABLE_REGISTER,
                    0
                ),
                (
                    registers::INTERNAL_DIRECT_MEMORY_ACCESS_STATUS_REGISTER,
                    registers::INTERNAL_DIRECT_MEMORY_ACCESS_STATUS_ALL_MASK
                ),
                (
                    registers::DESCRIPTOR_LIST_BASE_PHYSICAL_ADDRESS_REGISTER,
                    BASE as u32
                ),
                (registers::CONTROL_REGISTER, firmware),
                (
                    registers::BUS_MODE_REGISTER,
                    registers::BUS_MODE_INTERNAL_DIRECT_MEMORY_ACCESS_ENABLE
                        | registers::BUS_MODE_FIXED_BURST
                ),
                (registers::POLL_DEMAND_REGISTER, 1),
            ]
        );
        let stopped = armed
            + 1
            + writes[armed + 1..]
                .iter()
                .position(|&x| {
                    x == (
                        registers::BUS_MODE_REGISTER,
                        registers::BUS_MODE_SOFTWARE_RESET,
                    )
                })
                .unwrap();
        assert_eq!(
            writes[stopped..stopped + 4],
            [
                (
                    registers::BUS_MODE_REGISTER,
                    registers::BUS_MODE_SOFTWARE_RESET
                ),
                (
                    registers::CONTROL_REGISTER,
                    1 << 8
                        | registers::CONTROL_DIRECT_MEMORY_ACCESS_RESET
                        | registers::CONTROL_QUEUE_RESET
                ),
                (
                    registers::INTERNAL_DIRECT_MEMORY_ACCESS_STATUS_REGISTER,
                    registers::INTERNAL_DIRECT_MEMORY_ACCESS_STATUS_ALL_MASK
                ),
                (
                    registers::QUEUE_THRESHOLD_REGISTER,
                    registers::queue_threshold_for_programmed_input_output(32)
                ),
            ]
        );
    }

    /// More than one block is over only when the automatic `CMD12` is, which can land after `DTO`
    /// and after the last descriptor closes.
    #[test]
    fn a_multi_block_transfer_waits_for_the_automatic_stop() {
        let (h, card, memory) = polled(2);
        let mut h = with_internal_direct_memory_access(h, &memory);
        h.registers_mut().late_automatic_command_done = 5;
        let mut blocks = [0u8; 2 * 512];
        sd::read_blocks(&mut h, &card, 7, &mut blocks).unwrap();
        assert_ne!(
            h.registers_mut()
                .peek(registers::RAW_INTERRUPT_STATUS_REGISTER)
                & registers::RAW_INTERRUPT_AUTOMATIC_COMMAND_DONE,
            0
        );
    }

    /// A reset that never clears is reported, not assumed done, with the word that holds it:
    /// `BUS_MODE_REGISTER` when arming, and when stopping `BUS_MODE_REGISTER` or `CONTROL_REGISTER`, whichever is held.
    #[test]
    fn an_internal_direct_memory_access_reset_that_never_clears_is_reported_with_its_register() {
        type Hold = fn(&mut SimulatedHostController);
        type Word = fn(u32) -> bool;
        // Whether the read command reached the card: a stuck arming reset stops before it.
        let sent = [false, true, true];
        let cases: [(Hold, Word); 3] = [
            (
                |s| s.bus_mode_resets_cleared = Some(0),
                |w| w == registers::BUS_MODE_SOFTWARE_RESET,
            ),
            (
                |s| s.bus_mode_resets_cleared = Some(1),
                |w| w == registers::BUS_MODE_SOFTWARE_RESET,
            ),
            (
                |s| s.control_resets_cleared = Some(1),
                |w| {
                    w & (registers::CONTROL_DIRECT_MEMORY_ACCESS_RESET
                        | registers::CONTROL_QUEUE_RESET)
                        != 0
                },
            ),
        ];
        for ((hold, word), sent) in cases.into_iter().zip(sent) {
            let (h, card, memory) = polled(2);
            let mut h = with_internal_direct_memory_access(h, &memory);
            hold(h.registers_mut());
            h.registers_mut().commands.clear();
            let mut block = [0u8; 512];
            match sd::read_blocks(&mut h, &card, 7, &mut block) {
                Err(BlockTransferError::Host(Error::ResetStuck(w))) => assert!(word(w), "{w:#x}"),
                other => panic!("{other:?}"),
            }
            assert_eq!(h.registers_mut().commands.iter().any(|c| c.0 == 17), sent);
        }
    }

    /// The same blocks read through the IDMAC and by polling are the same bytes, a read of a long
    /// run keeps them so, and a bus that corrupts is seen as a difference.
    #[test]
    fn the_internal_direct_memory_access_read_matches_the_polled_one_until_the_bus_corrupts() {
        let (mut h, card, memory) = polled(17);
        let data = pattern(0x11, 128 * 512);
        sd::write_blocks(&mut h, &card, 2048, &data).unwrap();
        let mut reference = vec![0u8; 128 * 512];
        sd::read_blocks(&mut h, &card, 2048, &mut reference).unwrap();
        // Under Miri each 128-block command costs minutes, so a run reads fewer commands.
        let commands: u64 = if cfg!(miri) { 2 } else { 8 };
        let mut h = with_internal_direct_memory_access(h, &memory);
        let mut buffer = vec![0u8; 128 * 512];
        sd::read_blocks(&mut h, &card, 2048, &mut buffer).unwrap();
        assert_eq!(buffer, reference);
        for k in 1..commands {
            sd::read_blocks(&mut h, &card, 2048 + 128 * k, &mut buffer).unwrap();
        }
        h.registers_mut().direct_memory_access_corrupts = true;
        sd::read_blocks(&mut h, &card, 2048, &mut buffer).unwrap();
        assert_ne!(buffer, reference);
    }
}
