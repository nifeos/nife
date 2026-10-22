//! **The internal DMA controller (IDMAC)**: the controller walks a chain of descriptors in memory
//! and moves a data phase between its FIFO and RAM itself, so the CPU copies each byte once in
//! cached memory rather than once a word over MMIO.
//!
//! The region is one page of descriptors followed by the data pages, one descriptor per data page,
//! so a transfer is at most as many pages as the region has. Reads land in the data pages and are
//! copied out; writes are copied in first. The CPU never touches the FIFO. Between transfers the
//! controller is in the polled state [`Host::reset`] leaves: every transfer arms the IDMAC, and
//! every transfer, finished or failed, stops and resets it before returning, so no engine is left
//! walking a chain the caller has already reused. See notes/designware-mobile-storage.md.
//!
//! The sequence follows the databook's IDMAC programming model as OpenBSD's `dwmmc_dma_setup` and
//! Linux's `dw_mci_idmac_start_dma` use it (Linux read for hardware facts only): reset the IDMAC,
//! point `DBADDR` at the chain, select it in `CTRL`, enable it in `BMOD`, then send the command.
//! Completion is polled, like everything here: `RINTSTS` for the card's side and the last
//! descriptor's `OWN` bit for the memory side, since a read's `DTO` can land while the IDMAC
//! still has words to write.
//!
//! Name: provisional (milestone 53's IDMAC lane, 2026-10-22 UTC). The databook's own name for the
//! engine, as `regs` names its registers.

use crate::command::Command;
use crate::host::{COMMAND_DEADLINE_US, DATA_DEADLINE_US, Error, Host, Identity, Registers};
use crate::regs;

/// **The DMA region**, as the driver reads and writes it. Offsets are bytes from the region's
/// first byte; an implementation bounds them. The kernel's is pages in the direct map, the tests'
/// is memory `sim` walks too. `designware_ethernet::Region`'s shape.
pub trait DmaRegion {
    /// The region's first byte as the controller addresses it: on radon, its physical address.
    fn bus_address(&self) -> u64;
    /// The region's length in bytes.
    fn bytes(&self) -> u32;
    /// Read the little-endian `u32` at `off`.
    fn read_u32(&self, off: u32) -> u32;
    /// Write `v` at `off`, little-endian.
    fn write_u32(&mut self, off: u32, v: u32);
    /// Copy `out.len()` bytes starting at `off` into `out`.
    fn read_bytes(&self, off: u32, out: &mut [u8]);
    /// Copy `src` to `off`.
    fn write_bytes(&mut self, off: u32, src: &[u8]);
    /// Order every region access before this against every access after it, register accesses
    /// included, as the controller sees them. Called after a chain is written and before the
    /// register writes that start it, and after its end is seen and before the data is read.
    fn barrier(&mut self);
}

/// **No region**: a [`Host`] that moves its data through the FIFO. Uninhabited, so a host of this
/// type provably never takes the IDMAC path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoDma {}

impl DmaRegion for NoDma {
    fn bus_address(&self) -> u64 {
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
pub const PAGE: u32 = 4096;
/// One 32-bit-address descriptor: four words.
pub const DESCRIPTOR_BYTES: u32 = 16;
/// The most descriptors the chain page holds, and so the most data pages a region may have.
pub const MAX_DESCRIPTORS: u32 = PAGE / DESCRIPTOR_BYTES;

/// Descriptor word 0: the IDMAC owns this descriptor. Cleared by the IDMAC when it closes it.
pub(crate) const DES0_OWN: u32 = 1 << 31;
/// Word 0: word 3 holds the next descriptor's address, not a second buffer.
pub(crate) const DES0_CH: u32 = 1 << 4;
/// Word 0: the first descriptor of a transfer.
pub(crate) const DES0_FS: u32 = 1 << 3;
/// Word 0: the last descriptor of a transfer. The IDMAC stops after it.
pub(crate) const DES0_LD: u32 = 1 << 2;
/// Word 0: do not raise `TI`/`RI` when this descriptor closes.
pub(crate) const DES0_DIC: u32 = 1 << 1;
/// Word 1: the first buffer's size in bytes, bits 12:0.
pub(crate) const DES1_BS1_MASK: u32 = 0x1fff;

/// Why a region was refused before anything was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// `HCON` names an external DMA interface, so there is no IDMAC: the field.
    NoIdmac(u32),
    /// `HCON` says the IDMAC uses 64-bit addresses, whose descriptors this driver does not build.
    Addressing64,
    /// The region does not start on a page.
    Alignment(u64),
    /// The region is not a descriptor page and 1 to [`MAX_DESCRIPTORS`] data pages: its length.
    Size(u32),
    /// Part of the region lies above 4 GiB, where a 32-bit descriptor cannot point: its address.
    Above4GiB(u64),
}

/// **A region the IDMAC can use**, checked against the controller once. Only [`Idmac::new`] makes
/// one, so a [`Host`] holding one has an IDMAC, 32-bit descriptors and a region they can address.
pub struct Idmac<D: DmaRegion> {
    region: D,
    bus: u32,
    capacity: u32,
}

impl<D: DmaRegion> Idmac<D> {
    /// **Check `region` against the controller `id` describes.**
    ///
    /// # Errors
    ///
    /// A [`Refusal`]: no IDMAC, 64-bit addressing, or a region that is unaligned, the wrong size,
    /// or not wholly below 4 GiB.
    pub fn new(region: D, id: &Identity) -> Result<Idmac<D>, Refusal> {
        let interface = regs::dma_interface(id.hcon);
        if interface != 0 {
            return Err(Refusal::NoIdmac(interface));
        }
        if regs::dma_64bit(id.hcon) {
            return Err(Refusal::Addressing64);
        }
        let base = region.bus_address();
        if !base.is_multiple_of(u64::from(PAGE)) {
            return Err(Refusal::Alignment(base));
        }
        let bytes = region.bytes();
        let pages = bytes / PAGE;
        if !bytes.is_multiple_of(PAGE) || pages < 2 || pages - 1 > MAX_DESCRIPTORS {
            return Err(Refusal::Size(bytes));
        }
        let Ok(bus) = u32::try_from(base) else {
            return Err(Refusal::Above4GiB(base));
        };
        if bus.checked_add(bytes - 1).is_none() {
            return Err(Refusal::Above4GiB(base));
        }
        Ok(Idmac {
            region,
            bus,
            capacity: bytes - PAGE,
        })
    }

    /// The most bytes one transfer moves: the region's data pages.
    #[must_use]
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// The region, for a caller that wants to inspect it.
    pub fn region(&mut self) -> &mut D {
        &mut self.region
    }

    /// Give the region back.
    pub fn into_region(self) -> D {
        self.region
    }

    /// Write the chain for `bytes`, every descriptor the IDMAC's, and nothing past its last.
    fn write_chain(&mut self, bytes: u32) {
        let count = descriptors_for(bytes);
        for i in 0..count {
            let d = descriptor(self.bus, i, count, bytes);
            for (w, word) in d.iter().enumerate() {
                self.region
                    .write_u32(i * DESCRIPTOR_BYTES + 4 * w as u32, *word);
            }
        }
    }

    /// Bytes in descriptors the IDMAC still owns, of a chain of `count`: what it has not moved.
    fn owed(&self, count: u32) -> u32 {
        (0..count)
            .filter(|i| self.region.read_u32(i * DESCRIPTOR_BYTES) & DES0_OWN != 0)
            .map(|i| self.region.read_u32(i * DESCRIPTOR_BYTES + 4) & DES1_BS1_MASK)
            .sum()
    }
}

/// How many descriptors a transfer of `bytes` takes: one a page, and at least one.
#[must_use]
pub const fn descriptors_for(bytes: u32) -> u32 {
    if bytes == 0 { 1 } else { bytes.div_ceil(PAGE) }
}

/// **Descriptor `i` of `count`** for a transfer of `bytes`, in a region whose first byte is at bus
/// address `bus`: the chain page first, then the data pages in order. Every descriptor is the
/// IDMAC's and chained; the first carries `FS`, the last `LD` and the only completion status, and
/// each holds a page except the last, which holds what is left.
#[must_use]
pub const fn descriptor(bus: u32, i: u32, count: u32, bytes: u32) -> [u32; 4] {
    let last = i + 1 == count;
    let mut des0 = DES0_OWN | DES0_CH;
    if i == 0 {
        des0 |= DES0_FS;
    }
    if last {
        des0 |= DES0_LD;
    } else {
        des0 |= DES0_DIC;
    }
    let size = if last { bytes - i * PAGE } else { PAGE };
    let next = if last { 0 } else { i + 1 };
    [
        des0,
        size & DES1_BS1_MASK,
        bus + PAGE * (i + 1),
        bus + DESCRIPTOR_BYTES * next,
    ]
}

impl<R: Registers, D: DmaRegion> Host<R, D> {
    /// **Run `cmd`'s data phase through the IDMAC**, the data already in the region for a write.
    /// Stops and resets the IDMAC before returning, whatever happened.
    pub(crate) fn idmac_transfer(&mut self, cmd: &Command, bytes: u32) -> Result<[u32; 4], Error> {
        let count = descriptors_for(bytes);
        if let Some(dma) = self.dma.as_mut() {
            dma.write_chain(bytes);
            dma.region.barrier();
        }
        let result = self
            .issue(cmd, true)
            .and_then(|resp| self.idmac_wait(cmd, count).map(|()| resp));
        let stopped = self.idmac_stop();
        let resp = result?;
        stopped?;
        if let Some(dma) = self.dma.as_mut() {
            dma.region.barrier();
        }
        Ok(resp)
    }

    /// **Arm the IDMAC** on the chain at the region's start: called by `issue` once the FIFO
    /// reset has cleared and before the command is written, so the engine is ready for the first
    /// word. `IDINTEN` stays zero and `CTRL`'s interrupt enable clear; `IDSTS` is raw.
    pub(crate) fn idmac_start(&mut self) -> Result<(), Error> {
        let Some(bus) = self.dma.as_ref().map(|d| d.bus) else {
            return Ok(());
        };
        let bmod = self.regs.read(regs::BMOD);
        self.regs
            .write(regs::BMOD, bmod | regs::BMOD_SOFTWARE_RESET);
        if !self.wait(COMMAND_DEADLINE_US, |r| {
            r.read(regs::BMOD) & regs::BMOD_SOFTWARE_RESET == 0
        }) {
            return Err(Error::ResetStuck(self.regs.read(regs::BMOD)));
        }
        self.regs
            .write(regs::FIFOTH, regs::fifoth_for_dma(self.depth));
        self.regs.write(regs::IDINTEN, 0);
        self.regs.write(regs::IDSTS, regs::IDSTS_ALL);
        self.regs.write(regs::DBADDR, bus);
        let ctrl = self.regs.read(regs::CTRL);
        self.regs.write(
            regs::CTRL,
            ctrl | regs::CTRL_USE_INTERNAL_DMAC | regs::CTRL_DMA_ENABLE,
        );
        let bmod = self.regs.read(regs::BMOD);
        self.regs.write(
            regs::BMOD,
            bmod | regs::BMOD_DMA_ENABLE | regs::BMOD_FIXED_BURST,
        );
        self.regs.write(regs::PLDMND, 1);
        Ok(())
    }

    /// Wait for the card's side (`DTO`, and `ACD` after more than one block) and the memory side
    /// (the last descriptor closed). The deadline restarts whenever a descriptor closes, so a long
    /// transfer is judged on progress, not on its length.
    fn idmac_wait(&mut self, cmd: &Command, count: u32) -> Result<(), Error> {
        let want = match cmd.data {
            Some(d) if d.blocks > 1 => regs::INT_DTO | regs::INT_ACD,
            _ => regs::INT_DTO,
        };
        let Some(dma) = self.dma.as_ref() else {
            return Err(Error::BufferSize);
        };
        let mut owed = dma.owed(count);
        let mut start = self.regs.now_us();
        loop {
            let rint = self.regs.read(regs::RINTSTS);
            if rint & (regs::INT_DATA_ERRORS | regs::INT_DATA_TIMEOUTS) != 0 {
                return Err(Error::Data(cmd.index, rint));
            }
            let idsts = self.regs.read(regs::IDSTS);
            if idsts & regs::IDSTS_ERRORS != 0 {
                return Err(Error::Dma(cmd.index, idsts));
            }
            let Some(dma) = self.dma.as_ref() else {
                return Err(Error::BufferSize);
            };
            let now_owed = dma.owed(count);
            if rint & want == want && now_owed == 0 {
                return Ok(());
            }
            let now = self.regs.now_us();
            if now_owed != owed {
                owed = now_owed;
                start = now;
            } else if now.wrapping_sub(start) > DATA_DEADLINE_US {
                return Err(Error::DataTimeout(cmd.index, rint, owed));
            }
        }
    }

    /// **Stop the IDMAC** and hand the FIFO back to the CPU: disable it, deselect it, reset it and
    /// the FIFO, clear its status, and put back the polled thresholds. After this nothing the
    /// controller does can reach the region.
    fn idmac_stop(&mut self) -> Result<(), Error> {
        let bmod = self.regs.read(regs::BMOD);
        self.regs.write(
            regs::BMOD,
            (bmod & !(regs::BMOD_DMA_ENABLE | regs::BMOD_FIXED_BURST)) | regs::BMOD_SOFTWARE_RESET,
        );
        let ctrl =
            self.regs.read(regs::CTRL) & !(regs::CTRL_USE_INTERNAL_DMAC | regs::CTRL_DMA_ENABLE);
        self.regs.write(
            regs::CTRL,
            ctrl | regs::CTRL_DMA_RESET | regs::CTRL_FIFO_RESET,
        );
        let quiet = self.wait(COMMAND_DEADLINE_US, |r| {
            r.read(regs::CTRL) & (regs::CTRL_DMA_RESET | regs::CTRL_FIFO_RESET) == 0
                && r.read(regs::BMOD) & regs::BMOD_SOFTWARE_RESET == 0
        });
        self.regs.write(regs::IDSTS, regs::IDSTS_ALL);
        self.regs
            .write(regs::FIFOTH, regs::fifoth_for_pio(self.depth));
        if quiet {
            Ok(())
        } else {
            Err(Error::ResetStuck(self.regs.read(regs::CTRL)))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::vec::Vec;

    use super::*;
    use crate::sd::{self, Card, IoError};
    use crate::sim::{CardKind, Memory, Sim};

    /// A bus address with high bits set, so an offset taken as an address shows.
    const BASE: u64 = 0x8000_0000;

    /// An identified SDHC card behind a polled host, and memory of `pages` pages the sim's IDMAC
    /// reaches.
    fn polled(pages: usize) -> (Host<Sim>, Card, Memory) {
        let mut sim = Sim::new(CardKind::SdHighCapacity, 4096, 32);
        let memory = Memory::new(BASE, pages);
        sim.memory = Some(memory.clone());
        let (mut h, _) = Host::new(sim, 50_000_000, Some(32)).unwrap();
        let card = sd::identify(&mut h, 4).unwrap();
        (h, card, memory)
    }

    fn with_idmac(h: Host<Sim>, memory: &Memory) -> Host<Sim, Memory> {
        let mut h = h;
        let hcon = h.registers().peek(regs::HCON);
        let id = Identity {
            verid: 0,
            hcon,
            usrid: 0,
            fifoth: 0,
            cdetect: 0,
            firmware: [0; 5],
        };
        h.with_idmac(Idmac::new(memory.clone(), &id).unwrap())
    }

    fn polled_again(h: Host<Sim, Memory>) -> Host<Sim> {
        Host::new(h.into_registers(), 50_000_000, Some(32))
            .unwrap()
            .0
    }

    fn pattern(seed: u8, len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8 ^ seed).collect()
    }

    /// **Every descriptor bit, as a number, and a chain as the databook lays it out.** The driver
    /// and `sim` share these constants, so a wrong one would agree with itself in every walk; only
    /// the databook's values catch it. 4608 bytes is a full page and a 512-byte tail.
    #[test]
    fn the_descriptor_bits_and_a_two_page_chain_are_the_databooks() {
        assert_eq!(
            [DES0_OWN, DES0_CH, DES0_FS, DES0_LD, DES0_DIC, DES1_BS1_MASK],
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

    /// The 4 GiB edge is the one an 8 GB radon could reach: a region ending exactly there is
    /// usable, one a page later is not, and neither is anything a 32-bit descriptor cannot hold.
    #[test]
    fn a_region_the_idmac_cannot_reach_or_describe_is_refused() {
        let id = |hcon| Identity {
            verid: 0,
            hcon,
            usrid: 0,
            fifoth: 0,
            cdetect: 0,
            firmware: [0; 5],
        };
        let radon = id(0x00c4_3cc1);
        let new = |base, pages| Idmac::new(Memory::new(base, pages), &radon).map(|i| i.capacity());
        assert_eq!(new(0xffff_e000, 2), Ok(PAGE));
        assert_eq!(new(0xffff_f000, 2), Err(Refusal::Above4GiB(0xffff_f000)));
        assert_eq!(
            new(0x1_0000_0000, 2),
            Err(Refusal::Above4GiB(0x1_0000_0000))
        );
        assert_eq!(new(BASE + 8, 2), Err(Refusal::Alignment(BASE + 8)));
        assert_eq!(new(BASE, 1), Err(Refusal::Size(PAGE)));
        assert_eq!(new(BASE, 258), Err(Refusal::Size(258 * PAGE)));
        assert_eq!(new(BASE, 257), Ok(256 * PAGE));
        let m = Memory::new(BASE, 2);
        assert!(matches!(
            Idmac::new(m.clone(), &id(2 << 16)),
            Err(Refusal::NoIdmac(2))
        ));
        assert!(matches!(
            Idmac::new(m, &id(1 << 27)),
            Err(Refusal::Addressing64)
        ));
    }

    /// **Written one way, read the other.** A round trip through one path would pass with both
    /// halves wrong the same way (an offset, a byte order); the FIFO path is tested against the
    /// card on its own, so it is the reference here. Nine blocks leave the last descriptor a
    /// 512-byte tail. Every transfer is fenced on both sides and leaves every descriptor closed.
    #[test]
    fn a_chain_moves_the_cards_bytes_in_both_directions_as_the_fifo_does() {
        let (mut h, card, memory) = polled(4);
        let first = pattern(0x5a, 9 * 512);
        sd::write_blocks(&mut h, &card, 100, &first).unwrap();
        let mut h = with_idmac(h, &memory);
        let mut back = vec![0u8; 9 * 512];
        sd::read_blocks(&mut h, &card, 100, &mut back).unwrap();
        assert_eq!(back, first);
        let second = pattern(0xa5, 9 * 512);
        sd::write_blocks(&mut h, &card, 300, &second).unwrap();
        for i in 0..2 {
            assert_eq!(memory.read_u32(i * DESCRIPTOR_BYTES) & DES0_OWN, 0);
        }
        let bus = memory.bus();
        assert!(!bus.unfenced_start && !bus.unfenced_read);
        assert_eq!(bus.device_written, 9 * 512);
        drop(bus);
        let mut h = polled_again(h);
        sd::read_blocks(&mut h, &card, 300, &mut back).unwrap();
        assert_eq!(back, second);
        assert!(!h.registers().stray_fifo_write);
    }

    /// On a slow bus a read's `DTO` lands while the IDMAC still has words to write. A driver that
    /// stopped at `DTO` would hand back the memory's previous bytes; this one waits for the last
    /// descriptor to close.
    #[test]
    fn a_read_is_over_when_its_last_descriptor_closes_not_at_dto() {
        let (mut h, card, memory) = polled(3);
        let data = pattern(0x33, 16 * 512);
        sd::write_blocks(&mut h, &card, 40, &data).unwrap();
        let mut h = with_idmac(h, &memory);
        h.registers().dma_words_per_step = Some(1);
        let mut back = vec![0u8; 16 * 512];
        sd::read_blocks(&mut h, &card, 40, &mut back).unwrap();
        assert_eq!(back, data);
    }

    /// One more block than the region holds is refused before anything is sent, rather than
    /// written past the region's end.
    #[test]
    fn a_transfer_larger_than_the_region_is_refused_unsent() {
        let (h, card, memory) = polled(3);
        let mut h = with_idmac(h, &memory);
        h.registers().commands.clear();
        let mut big = vec![0u8; 17 * 512];
        assert_eq!(
            sd::read_blocks(&mut h, &card, 0, &mut big),
            Err(IoError::Host(Error::BufferSize))
        );
        assert_eq!(
            sd::write_blocks(&mut h, &card, 0, &big),
            Err(IoError::Host(Error::BufferSize))
        );
        assert!(h.registers().commands.is_empty());
    }

    /// **Every way a transfer fails stops the engine before the driver returns**: a bus error
    /// (`IDSTS`), a data CRC error (`RINTSTS`) and a card that stalls (the deadline, owing the one
    /// descriptor still open). Afterwards the IDMAC is deselected and disabled, the FIFO is the
    /// CPU's again, a card that resumes moves nothing into memory, and the next transfer works.
    #[test]
    fn every_failed_transfer_stops_the_idmac_and_the_next_one_works() {
        type Inject = fn(&mut Sim);
        type Expect = fn(&Error) -> bool;
        let cases: [(Inject, Expect); 3] = [
            (
                |s| s.bus_error_at = Some(1),
                |e| matches!(e, Error::Dma(18, s) if s & regs::IDSTS_FBE != 0),
            ),
            (
                |s| s.fail_next_read_crc = true,
                |e| matches!(e, Error::Data(18, r) if r & regs::INT_DCRC != 0),
            ),
            (
                |s| s.stall_after = Some(4096 + 512),
                |e| matches!(e, Error::DataTimeout(18, _, 4096)),
            ),
        ];
        for (inject, expected) in cases {
            let (mut h, card, memory) = polled(3);
            let data = pattern(0x77, 16 * 512);
            sd::write_blocks(&mut h, &card, 60, &data).unwrap();
            let mut h = with_idmac(h, &memory);
            inject(h.registers());
            let mut back = vec![0u8; 16 * 512];
            match sd::read_blocks(&mut h, &card, 60, &mut back) {
                Err(IoError::Host(e)) => assert!(expected(&e), "{e:?}"),
                other => panic!("{other:?}"),
            }
            let sim = h.registers();
            let dma = regs::CTRL_USE_INTERNAL_DMAC | regs::CTRL_DMA_ENABLE;
            assert_eq!(sim.peek(regs::CTRL) & dma, 0);
            assert_eq!(sim.peek(regs::BMOD) & regs::BMOD_DMA_ENABLE, 0);
            assert_eq!(sim.peek(regs::FIFOTH), regs::fifoth_for_pio(32));
            sim.stall_after = None;
            sim.bus_error_at = None;
            let written = memory.bus().device_written;
            for _ in 0..64 {
                sim.read(regs::STATUS);
            }
            assert_eq!(memory.bus().device_written, written);
            sd::read_blocks(&mut h, &card, 60, &mut back).unwrap();
            assert_eq!(back, data);
        }
    }

    /// The bench compares the same blocks both ways: the IDMAC's read of the partition's first
    /// 64 KiB against the polled read of it, not whatever the timed read leaves in the buffer. A
    /// bus that corrupts is a mismatch, and the timed read still runs.
    #[test]
    fn the_bench_compares_the_idmac_read_with_the_polled_one() {
        let (mut h, card, memory) = polled(17);
        let data = pattern(0x11, 128 * 512);
        sd::write_blocks(&mut h, &card, 2048, &data).unwrap();
        let mut reference = vec![0u8; 128 * 512];
        sd::read_blocks(&mut h, &card, 2048, &mut reference).unwrap();
        let report = crate::bench::ReadReport {
            card,
            mbr: None,
            partition_boot_signature: false,
            fs_type: [0; 8],
            timed_from: 2048,
            timed_blocks: 0,
            timed_us: 0,
        };
        let mut h = with_idmac(h, &memory);
        let mut buf = vec![0u8; 128 * 512];
        let r = crate::bench::idmac_read(&mut h, &report, &reference, &mut buf, 1024).unwrap();
        assert!(r.matches);
        assert_eq!(r.timed_blocks, 1024);
        h.registers().dma_corrupts = true;
        let r = crate::bench::idmac_read(&mut h, &report, &reference, &mut buf, 256).unwrap();
        assert!(!r.matches);
        assert_eq!(r.timed_blocks, 256);
    }
}
