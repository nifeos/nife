//! **A simulated controller with a card behind it**, for the host tests.
//!
//! Nothing emulates this controller: QEMU has no `DW_mshc` model for any RISC-V machine, and the
//! one `dw_mmc`-shaped device QEMU does have (on some Arm boards) is not reachable from the
//! machines this tree boots. So the tests run the real driver against this, and the bench step is
//! what checks this against silicon. What it models is the contract the driver relies on, as the
//! databook and OpenBSD's driver describe it, and no more:
//!
//! - `CTRL`'s reset bits clear themselves one read after they are set;
//! - a write to `CMD` with the start bit either loads the clock (and clears the bit) or runs the
//!   command against the card, sets the response words and `RINTSTS` (`CD`, plus `RTO` when
//!   nothing answered), and clears the bit;
//! - a data phase moves through a FIFO of `depth` words: reads fill it from the card and raise
//!   `RXDR`, writes drain it to the card and raise `TXDR`, and the end raises `DTO` (and `ACD`
//!   after a multi-block transfer, the automatic CMD12);
//! - `STATUS` reports the FIFO's fill and, for a few reads after a write, data busy.
//! - a read with the card read threshold (`CARDTHRCTL`) enabled above the FIFO's size ends at
//!   once with `FRUN` and `DTO`, the word radon's controller gave for it.
//! - with the IDMAC selected in `CTRL` and enabled in `BMOD`, the FIFO's other end is [`Memory`]:
//!   the walk starts at `DBADDR`, loads each descriptor, suspends with `DU` on one it does not own,
//!   stops with `FBE` on an address outside memory, moves its buffer, closes it (clears `OWN`,
//!   raises `RI` or `TI` unless `DIC`), and stops after `LD` or follows `CH`. A read's last words
//!   reach memory after `DTO`, as on silicon, so the driver must wait for the descriptor too.
//!
//! Time is one microsecond per call to `now_us`, so every deadline is reached in a bounded number
//! of polls and a test that would hang on silicon fails here instead.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use std::vec::Vec;

use crate::host::Registers;
use crate::idmac::{DES0_CH, DES0_DIC, DES0_LD, DES0_OWN, DES1_BS1_MASK, DmaRegion};
use crate::regs;

/// What the bus holds and what has been ordered: the driver's writes since its last barrier, and
/// the device's.
#[derive(Debug, Default)]
pub(crate) struct Bus {
    /// The memory's first byte's bus address.
    pub base: u64,
    /// The memory.
    pub bytes: Vec<u8>,
    cpu_unfenced: bool,
    device_unfenced: bool,
    /// The IDMAC was started with driver writes to memory not yet behind a barrier.
    pub unfenced_start: bool,
    /// The driver read data the IDMAC wrote without a barrier between.
    pub unfenced_read: bool,
    /// Bytes the IDMAC has written to memory.
    pub device_written: usize,
}

/// **Memory the simulated IDMAC reaches**, and the region the driver writes it through: two
/// handles on one [`Bus`].
#[derive(Debug, Clone)]
pub(crate) struct Memory(Arc<Mutex<Bus>>);

impl Memory {
    /// `pages` pages at bus address `base`, zero.
    #[must_use]
    pub fn new(base: u64, pages: usize) -> Memory {
        Memory(Arc::new(Mutex::new(Bus {
            base,
            bytes: vec![0; pages * 4096],
            ..Bus::default()
        })))
    }

    /// The bus, locked.
    pub fn bus(&self) -> MutexGuard<'_, Bus> {
        self.0.lock().unwrap()
    }
}

impl DmaRegion for Memory {
    fn bus_address(&self) -> u64 {
        self.bus().base
    }
    fn bytes(&self) -> u32 {
        u32::try_from(self.bus().bytes.len()).unwrap()
    }
    fn read_u32(&self, off: u32) -> u32 {
        let b = &self.bus().bytes[off as usize..off as usize + 4];
        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
    }
    fn write_u32(&mut self, off: u32, v: u32) {
        let mut bus = self.bus();
        bus.cpu_unfenced = true;
        bus.bytes[off as usize..off as usize + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn read_bytes(&self, off: u32, out: &mut [u8]) {
        let mut bus = self.bus();
        if bus.device_unfenced {
            bus.unfenced_read = true;
        }
        out.copy_from_slice(&bus.bytes[off as usize..off as usize + out.len()]);
    }
    fn write_bytes(&mut self, off: u32, src: &[u8]) {
        let mut bus = self.bus();
        bus.cpu_unfenced = true;
        bus.bytes[off as usize..off as usize + src.len()].copy_from_slice(src);
    }
    fn barrier(&mut self) {
        let mut bus = self.bus();
        bus.cpu_unfenced = false;
        bus.device_unfenced = false;
    }
}

/// Where the simulated IDMAC is in its walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Walk {
    /// Starts at `DBADDR` when next there is work.
    Idle,
    /// On the descriptor at this bus address, this many bytes of its buffer moved.
    At(u32, u32),
    /// Suspended (`DU`) on the descriptor at this address until a poll demand.
    Suspended(u32),
    /// Stopped by a bus error until a reset.
    Halted,
    /// Past a descriptor with `LD`.
    Finished,
}

/// Which card is in the slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CardKind {
    /// An SDHC card: answers CMD8, high capacity, block addressed.
    SdHighCapacity,
    /// An SD 1.x card: ignores CMD8, standard capacity, byte addressed.
    SdV1,
    /// An SD 2.0 card of standard capacity: answers CMD8, byte addressed.
    SdV2,
    /// An eMMC: ignores CMD8 in idle and CMD55, answers CMD1, takes its address from the host.
    Mmc,
    /// An empty slot: nothing answers.
    Empty,
}

/// What an injected fault does to the command it hits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// The card does not answer: `RTO`.
    NoResponse,
    /// The card answers with the R1 `ERROR` bit (19) set.
    ErrorStatus,
    /// The card answers `CMD8` with the wrong check pattern.
    EchoWrong,
    /// The answer arrives with a response CRC error (`RCRC`).
    BadCrc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Ready,
    Ident,
    Standby,
    Transfer,
}

#[derive(Debug)]
struct Transfer {
    write: bool,
    first_lba: u64,
    bytes: usize,
    moved: usize,
    auto_stop: bool,
    buffer: Vec<u8>,
    ext_csd: bool,
}

/// The simulation. Public fields are what a test sets up or checks.
pub struct Sim {
    regs: [u32; 0x220 / 4],
    reset_pending: bool,
    now: u64,
    fifo: VecDeque<u32>,
    depth: usize,
    transfer: Option<Transfer>,
    state: State,
    app: bool,
    power_up_polls: u32,
    rca: u16,
    busy_reads: u32,
    /// The card in the slot.
    pub kind: CardKind,
    /// The card's contents, 512-byte blocks.
    pub storage: Vec<[u8; 512]>,
    /// Every command the card saw: index and argument.
    pub commands: Vec<(u8, u32)>,
    /// Every block the card was asked to program, in order.
    pub written: Vec<u64>,
    /// Make the next read's data phase fail its CRC.
    pub fail_next_read_crc: bool,
    /// How many `ACMD41`s or `CMD1`s report busy before power-up completes.
    pub busy_polls: u32,
    /// Set when a command was sent while the card clock was off.
    pub clockless_command: bool,
    /// The release `VERID` reports, which decides where the FIFO is.
    verid: u32,
    /// Set when the driver wrote the FIFO with no write transfer in progress.
    pub stray_fifo_write: bool,
    /// Faults to inject: the command index, which of its occurrences (from 0), and what happens.
    pub faults: Vec<(u8, usize, Fault)>,
    /// How many times each command index has been sent.
    seen: [usize; 64],
    /// The CSD reports a structure this driver does not size (3), so the capacity reads as 0.
    pub zero_capacity: bool,
    /// Only the first `n` write commands store their data; later ones are acknowledged and lost,
    /// the way a card that has failed into read-only mode can behave.
    pub writes_stored: Option<usize>,
    /// How many write commands have completed.
    write_commands: usize,
    /// Raise `RXDR` only above the receive watermark the driver programmed in `FIFOTH`, as the
    /// databook says, rather than whenever the FIFO holds anything. A read whose tail is shorter
    /// than the watermark then ends with words in the FIFO and only `DTO` to say so.
    pub strict_watermark: bool,
    /// Every register write the driver made, in order, FIFO words excepted.
    pub writes: Vec<(u32, u32)>,
    /// `CTRL`'s reset bits never clear.
    pub reset_stuck: bool,
    /// The controller never takes a clock update: `CMD`'s start bit stays set.
    pub clock_stuck: bool,
    /// The card holds DAT0 low forever.
    pub busy_forever: bool,
    /// How many `RINTSTS` reads pass before a command's answer (`CD`, `RTO`, the response words)
    /// lands, as on silicon, where the command takes time on the wire.
    pub late_answer: u32,
    answer_pending: Option<(u32, Option<[u32; 4]>, u32)>,
    /// The card stops moving data after this many bytes of a transfer.
    pub stall_after: Option<usize>,
    /// When the card stalls, it also raises `DTO`: a transfer the card ended early.
    pub end_early: bool,
    /// `STATUS` reports a full FIFO whatever it holds: a confused controller.
    pub overreport_fill: bool,
    /// A write's data phase ends with a data CRC error and no `DTO`.
    pub crc_error_at_end: bool,
    /// How many `RINTSTS` reads pass after `DTO` before the automatic CMD12's `ACD` lands.
    pub late_acd: u32,
    acd_pending: u32,
    /// An eMMC of 2 GB or less: byte addressed, its OCR without the sector-mode bit.
    pub mmc_byte_addressed: bool,
    /// What `EXT_CSD`'s `SEC_COUNT` reports, if not the card's real size.
    pub ext_csd_sectors: Option<u32>,
    /// Store one block of one write command wrongly: `(write command, from 0; block)`.
    pub mangle_write: Option<(usize, u64)>,
    /// Microseconds per call to `now_us`.
    pub clock_step: u64,
    /// What the IDMAC reaches, if a test gave it any.
    pub memory: Option<Memory>,
    walk: Walk,
    /// The bus refuses the buffer of the descriptor walked at this position (from 0) of a chain.
    pub bus_error_at: Option<u32>,
    walked: u32,
    /// The IDMAC moves at most this many words each time the sim advances: a slow bus, which lets
    /// a read's `DTO` land well before its last words reach memory.
    pub dma_words_per_step: Option<usize>,
    /// The IDMAC flips bit 0 of every word it writes to memory: a bus that corrupts.
    pub dma_corrupts: bool,
    /// The direction of the last data command: the way the IDMAC moves words.
    dma_write: bool,
}

/// The `VERID` the simulation reports: release 2.90a, which puts the FIFO at `0x200`.
pub(crate) const VERID: u32 = 0x5342_290a;
/// The `HCON` it reports: a 32-bit FIFO, 32-bit addresses.
pub(crate) const HCON: u32 = 1 << 7;

impl Sim {
    /// A controller of FIFO `depth` words with `kind` of card holding `blocks` blocks of zero.
    #[must_use]
    pub fn new(kind: CardKind, blocks: usize, depth: usize) -> Sim {
        let mut s = Sim {
            regs: [0; 0x220 / 4],
            reset_pending: false,
            now: 0,
            fifo: VecDeque::new(),
            depth,
            transfer: None,
            state: State::Idle,
            app: false,
            power_up_polls: 0,
            rca: 0,
            busy_reads: 0,
            kind,
            storage: vec![[0u8; 512]; blocks],
            commands: Vec::new(),
            written: Vec::new(),
            fail_next_read_crc: false,
            busy_polls: 3,
            clockless_command: false,
            verid: VERID,
            stray_fifo_write: false,
            faults: Vec::new(),
            seen: [0; 64],
            zero_capacity: false,
            writes_stored: None,
            write_commands: 0,
            strict_watermark: false,
            writes: Vec::new(),
            reset_stuck: false,
            clock_stuck: false,
            busy_forever: false,
            late_answer: 0,
            answer_pending: None,
            stall_after: None,
            end_early: false,
            overreport_fill: false,
            crc_error_at_end: false,
            late_acd: 0,
            acd_pending: 0,
            mmc_byte_addressed: false,
            ext_csd_sectors: None,
            mangle_write: None,
            clock_step: 1,
            memory: None,
            walk: Walk::Idle,
            bus_error_at: None,
            walked: 0,
            dma_words_per_step: None,
            dma_corrupts: false,
            dma_write: false,
        };
        s.regs[(regs::VERID / 4) as usize] = VERID;
        s.regs[(regs::HCON / 4) as usize] = HCON;
        // FIFOTH's reset value carries depth - 1 as the receive watermark.
        s.regs[(regs::FIFOTH / 4) as usize] = ((depth as u32 - 1) & 0xfff) << 16;
        // A card is present (active low).
        s.regs[(regs::CDETECT / 4) as usize] = u32::from(kind == CardKind::Empty);
        s
    }

    /// The same controller, reporting release `verid` instead (an older one puts the FIFO at
    /// `0x100`, where a later one has the card threshold register).
    #[must_use]
    pub(crate) fn with_verid(mut self, verid: u32) -> Sim {
        self.verid = verid;
        self.regs[(regs::VERID / 4) as usize] = verid;
        self
    }

    /// The FIFO depth in words.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// A register's value as the driver last left it, without the side effects of a read.
    #[must_use]
    pub fn peek(&self, offset: u32) -> u32 {
        self.regs[(offset / 4) as usize]
    }

    fn reg(&mut self, offset: u32) -> &mut u32 {
        &mut self.regs[(offset / 4) as usize]
    }

    fn high_capacity(&self) -> bool {
        !matches!(self.kind, CardKind::SdV1 | CardKind::SdV2)
            && !(self.kind == CardKind::Mmc && self.mmc_byte_addressed)
    }

    fn r1(&self) -> u32 {
        let state = match self.state {
            State::Idle => 0,
            State::Ready => 1,
            State::Ident => 2,
            State::Standby => 3,
            State::Transfer => 4,
        };
        (state << 9) | (1 << 8) | if self.app { 1 << 5 } else { 0 }
    }

    fn csd(&self) -> [u32; 4] {
        let blocks = self.storage.len() as u64;
        let mut r = [0u32; 4];
        let mut put = |start: u32, len: u32, v: u64| {
            for i in 0..len {
                let bit = start + i;
                if (v >> i) & 1 == 1 {
                    r[(bit / 32) as usize] |= 1 << (bit % 32);
                }
            }
        };
        if self.zero_capacity {
            put(126, 2, 3);
            return r;
        }
        match self.kind {
            CardKind::SdHighCapacity => {
                put(126, 2, 1);
                put(48, 22, blocks / 1024 - 1);
            }
            // Version 1 layout for both: C_SIZE_MULT 7 and READ_BL_LEN 9 give 512-block units.
            _ => {
                put(80, 4, 9);
                put(47, 3, 7);
                put(62, 12, blocks / 512 - 1);
            }
        }
        r
    }

    fn cid(&self) -> [u32; 4] {
        // Manufacturer 0x1b, product "NIFE1", serial 0x1234_5678.
        let mut r = [0x1234_5678u32 << 24, 0x1234_5678 >> 8, 0, 0x1b00_0000];
        for (i, b) in b"NIFE1".iter().enumerate() {
            let bit = 96 - 8 * i as u32;
            r[(bit / 32) as usize] |= u32::from(*b) << (bit % 32);
        }
        r
    }

    fn lba(&self, arg: u32) -> u64 {
        if self.high_capacity() {
            u64::from(arg)
        } else {
            u64::from(arg) / 512
        }
    }

    /// The card's answer to one command: `None` is no response.
    fn card(&mut self, index: u8, arg: u32) -> Option<[u32; 4]> {
        if self.kind == CardKind::Empty {
            return None;
        }
        let app = core::mem::replace(&mut self.app, false);
        let short = |v: u32| Some([v, 0, 0, 0]);
        match (index, app, self.kind) {
            (0, _, _) => {
                self.state = State::Idle;
                self.power_up_polls = 0;
                None
            }
            (8, false, CardKind::SdHighCapacity | CardKind::SdV2) if self.state == State::Idle => {
                short(arg & 0xfff)
            }
            (55, _, CardKind::Mmc) => None,
            (55, _, _) => {
                self.app = true;
                short(self.r1() | 1 << 5)
            }
            (41, true, _) | (1, false, CardKind::Mmc) => {
                self.power_up_polls += 1;
                let mut ocr = 0x00ff_8000;
                if self.power_up_polls > self.busy_polls {
                    ocr |= 1 << 31;
                    if self.high_capacity() && (index == 1 || arg & (1 << 30) != 0) {
                        ocr |= 1 << 30;
                    }
                    self.state = State::Ready;
                }
                short(ocr)
            }
            (2, false, _) if self.state == State::Ready => {
                self.state = State::Ident;
                Some(self.cid())
            }
            (3, false, CardKind::Mmc) if self.state == State::Ident => {
                self.rca = (arg >> 16) as u16;
                let r = self.r1();
                self.state = State::Standby;
                short(r)
            }
            (3, false, _) if self.state == State::Ident => {
                self.rca = 0xb368;
                self.state = State::Standby;
                short(u32::from(self.rca) << 16 | 0x0500)
            }
            (9, false, _) if self.state == State::Standby && (arg >> 16) as u16 == self.rca => {
                Some(self.csd())
            }
            (7, false, _) if (arg >> 16) as u16 == self.rca => {
                let r = self.r1();
                self.state = State::Transfer;
                short(r)
            }
            (6, true, _) | (16, false, _) | (13, false, _) if self.state == State::Transfer => {
                short(self.r1())
            }
            (8, false, CardKind::Mmc) | (17 | 18 | 24 | 25, false, _)
                if self.state == State::Transfer =>
            {
                short(self.r1())
            }
            _ => None,
        }
    }

    fn command(&mut self, word: u32) {
        let index = (word & 0x3f) as u8;
        let arg = *self.reg(regs::CMDARG);
        self.commands.push((index, arg));
        if *self.reg(regs::CLKENA) & regs::CLKENA_ENABLE == 0 {
            self.clockless_command = true;
        }
        let nth = self.seen[usize::from(index)];
        self.seen[usize::from(index)] += 1;
        let fault = self
            .faults
            .iter()
            .find(|f| f.0 == index && f.1 == nth)
            .map(|f| f.2);
        let mut answer = if fault == Some(Fault::NoResponse) {
            None
        } else {
            self.card(index, arg)
        };
        if let Some(r) = answer.as_mut() {
            match fault {
                Some(Fault::ErrorStatus) => r[0] |= 1 << 19,
                Some(Fault::EchoWrong) => r[0] ^= 0xff,
                _ => {}
            }
        }
        let mut rint = regs::INT_CD;
        match answer {
            None if word & regs::CMD_RESPONSE_EXPECT != 0 => rint |= regs::INT_RTO,
            None => {}
            Some(_) if fault == Some(Fault::BadCrc) => rint |= regs::INT_RCRC,
            Some(_) => {}
        }
        if self.late_answer > 0 {
            self.answer_pending = Some((rint, answer, self.late_answer));
        } else {
            self.land(rint, answer);
        }
        if answer.is_some() && word & regs::CMD_DATA_EXPECTED != 0 {
            let bytes = *self.reg(regs::BYTCNT) as usize;
            let write = word & regs::CMD_WRITE != 0;
            self.dma_write = write;
            if self.idmac_active()
                && let Some(m) = &self.memory
                && m.bus().cpu_unfenced
            {
                m.bus().unfenced_start = true;
            }
            let ext_csd = index == 8;
            let mut t = Transfer {
                write,
                first_lba: if ext_csd { 0 } else { self.lba(arg) },
                bytes,
                moved: 0,
                auto_stop: word & regs::CMD_SEND_AUTO_STOP != 0,
                buffer: Vec::new(),
                ext_csd,
            };
            if !write {
                if ext_csd {
                    let mut e = vec![0u8; 512];
                    let sectors = self.ext_csd_sectors.unwrap_or(self.storage.len() as u32);
                    e[212..216].copy_from_slice(&sectors.to_le_bytes());
                    t.buffer = e;
                } else {
                    for i in 0..bytes / 512 {
                        let lba = t.first_lba as usize + i;
                        t.buffer.extend_from_slice(
                            self.storage.get(lba).map_or(&[0xffu8; 512][..], |b| &b[..]),
                        );
                    }
                }
            }
            self.transfer = Some(t);
        }
    }

    fn land(&mut self, rint: u32, answer: Option<[u32; 4]>) {
        if let Some(r) = answer {
            *self.reg(regs::RESP0) = r[0];
            *self.reg(regs::RESP1) = r[1];
            *self.reg(regs::RESP2) = r[2];
            *self.reg(regs::RESP3) = r[3];
        }
        *self.reg(regs::RINTSTS) |= rint;
    }

    /// Advance the data phase: fill or drain the FIFO, and end it when every byte has moved.
    fn pump(&mut self) {
        self.idmac_step();
        self.pump_card();
        self.idmac_step();
    }

    /// The card's side of the data phase.
    fn pump_card(&mut self) {
        let Some(mut t) = self.transfer.take() else {
            return;
        };
        if t.write {
            let limit = self.stall_after.unwrap_or(usize::MAX);
            while t.buffer.len() < limit
                && let Some(w) = self.fifo.pop_front()
            {
                t.buffer.extend_from_slice(&w.to_le_bytes());
            }
            if t.buffer.len() >= t.bytes {
                let store = self.writes_stored.is_none_or(|n| self.write_commands < n);
                let nth = self.write_commands;
                self.write_commands += 1;
                for (i, block) in t.buffer.as_chunks::<512>().0.iter().enumerate() {
                    let lba = t.first_lba + i as u64;
                    self.written.push(lba);
                    let mangle = self.mangle_write == Some((nth, lba));
                    if let Some(b) = self.storage.get_mut(lba as usize)
                        && store
                    {
                        b.copy_from_slice(block);
                        if mangle {
                            b[0] ^= 0xff;
                        }
                    }
                }
                self.busy_reads = 2;
                self.end(&t);
                return;
            }
            *self.reg(regs::RINTSTS) |= regs::INT_TXDR;
        } else {
            if self.threshold_beyond_fifo() {
                // What radon answered (`RINTSTS 0x080c` with `CD`): the block ends, overrun.
                *self.reg(regs::RINTSTS) |= regs::INT_FRUN | regs::INT_DTO;
                return;
            }
            if self.fail_next_read_crc && !t.ext_csd {
                self.fail_next_read_crc = false;
                *self.reg(regs::RINTSTS) |= regs::INT_DCRC | regs::INT_DTO;
                return;
            }
            let limit = self.stall_after.unwrap_or(usize::MAX);
            while self.fifo.len() < self.depth && t.moved < t.bytes && t.moved < limit {
                let b = &t.buffer[t.moved..t.moved + 4];
                self.fifo
                    .push_back(u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                t.moved += 4;
            }
            let watermark = if self.strict_watermark {
                regs::fifoth_rx_watermark(*self.reg(regs::FIFOTH)) as usize
            } else {
                0
            };
            if self.fifo.len() > watermark {
                *self.reg(regs::RINTSTS) |= regs::INT_RXDR;
            }
            if t.moved >= t.bytes || (self.end_early && t.moved >= limit) {
                self.end(&t);
                return;
            }
        }
        self.transfer = Some(t);
    }

    fn idmac_active(&self) -> bool {
        let both = regs::CTRL_USE_INTERNAL_DMAC | regs::CTRL_DMA_ENABLE;
        self.peek(regs::CTRL) & both == both
            && self.peek(regs::BMOD) & regs::BMOD_DMA_ENABLE != 0
            && self.memory.is_some()
    }

    /// **One stretch of the IDMAC's walk**: move words between the FIFO and memory until the FIFO
    /// is empty (a read) or full (a write), or the walk stops.
    fn idmac_step(&mut self) {
        if !self.idmac_active() {
            return;
        }
        let Some(memory) = self.memory.clone() else {
            return;
        };
        let mut bus = memory.bus();
        let mut budget = self.dma_words_per_step.unwrap_or(usize::MAX);
        loop {
            if budget == 0 {
                return;
            }
            budget -= 1;
            let more = if self.dma_write {
                self.fifo.len() < self.depth
            } else {
                !self.fifo.is_empty()
            };
            if !more {
                return;
            }
            let (desc, done) = match self.walk {
                Walk::Idle => (self.peek(regs::DBADDR), 0),
                Walk::At(d, n) => (d, n),
                Walk::Suspended(_) | Walk::Halted | Walk::Finished => return,
            };
            let Some(at) = self.offset(&bus, desc, 16) else {
                self.halt();
                return;
            };
            let word = |i: usize| {
                let b = &bus.bytes[at + 4 * i..at + 4 * i + 4];
                u32::from_le_bytes([b[0], b[1], b[2], b[3]])
            };
            let (des0, size, buffer, next) = (word(0), word(1) & DES1_BS1_MASK, word(2), word(3));
            if des0 & DES0_OWN == 0 {
                self.walk = Walk::Suspended(desc);
                *self.reg(regs::IDSTS) |= regs::IDSTS_DU | regs::IDSTS_AIS;
                return;
            }
            if done == 0 && self.bus_error_at == Some(self.walked) {
                self.halt();
                return;
            }
            let Some(buf) = self.offset(&bus, buffer.wrapping_add(done), 4) else {
                self.halt();
                return;
            };
            if self.dma_write {
                let b = &bus.bytes[buf..buf + 4];
                self.fifo
                    .push_back(u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
            } else if let Some(w) = self.fifo.pop_front() {
                let w = w ^ u32::from(self.dma_corrupts);
                bus.bytes[buf..buf + 4].copy_from_slice(&w.to_le_bytes());
                bus.device_unfenced = true;
                bus.device_written += 4;
            }
            let done = done + 4;
            if done < size {
                self.walk = Walk::At(desc, done);
                continue;
            }
            bus.bytes[at..at + 4].copy_from_slice(&(des0 & !DES0_OWN).to_le_bytes());
            bus.device_unfenced = true;
            self.walked += 1;
            if des0 & DES0_DIC == 0 {
                let finished = if self.dma_write {
                    regs::IDSTS_TI
                } else {
                    regs::IDSTS_RI
                };
                *self.reg(regs::IDSTS) |= finished | regs::IDSTS_NIS;
            }
            self.walk = if des0 & DES0_LD != 0 {
                Walk::Finished
            } else if des0 & DES0_CH != 0 {
                Walk::At(next, 0)
            } else {
                Walk::At(desc + 16, 0)
            };
        }
    }

    /// The byte offset in memory of `len` bytes at bus address `addr`, if they are all there.
    fn offset(&self, bus: &Bus, addr: u32, len: usize) -> Option<usize> {
        let off = usize::try_from(u64::from(addr).checked_sub(bus.base)?).ok()?;
        (off + len <= bus.bytes.len()).then_some(off)
    }

    fn halt(&mut self) {
        self.walk = Walk::Halted;
        *self.reg(regs::IDSTS) |= regs::IDSTS_FBE | regs::IDSTS_AIS;
    }

    /// A card read threshold enabled above the FIFO's size in bytes, which the databook does not
    /// allow and Linux's `dw_mci_ctrl_thld` refuses to program.
    fn threshold_beyond_fifo(&self) -> bool {
        if regs::fifo_offset(self.verid) == regs::CARDTHRCTL {
            return false;
        }
        let t = self.peek(regs::CARDTHRCTL);
        t & regs::CARDTHRCTL_READ_ENABLE != 0 && (t >> 16) & 0xfff > self.depth as u32 * 4
    }

    fn end(&mut self, t: &Transfer) {
        if t.write && self.crc_error_at_end {
            *self.reg(regs::RINTSTS) |= regs::INT_DCRC;
            return;
        }
        let mut rint = regs::INT_DTO;
        if t.auto_stop {
            self.commands.push((12, 0));
            if self.late_acd > 0 {
                self.acd_pending = self.late_acd;
            } else {
                rint |= regs::INT_ACD;
            }
        }
        *self.reg(regs::RINTSTS) |= rint;
    }
}

impl Registers for Sim {
    fn read(&mut self, offset: u32) -> u32 {
        let fifo = regs::fifo_offset(self.verid);
        if offset == fifo {
            let w = self.fifo.pop_front().unwrap_or(0xdead_dead);
            self.pump();
            return w;
        }
        match offset {
            regs::CTRL if self.reset_pending => {
                // The bits read as set once, then clear: the driver must poll, not assume.
                self.reset_pending = false;
                let v = *self.reg(regs::CTRL);
                *self.reg(regs::CTRL) &= !regs::CTRL_ALL_RESET;
                v
            }
            regs::BMOD if self.peek(regs::BMOD) & regs::BMOD_SOFTWARE_RESET != 0 => {
                let v = self.peek(regs::BMOD);
                if !self.reset_stuck {
                    *self.reg(regs::BMOD) &= !regs::BMOD_SOFTWARE_RESET;
                }
                v
            }
            regs::STATUS => {
                self.pump();
                let filled = if self.overreport_fill {
                    self.depth
                } else {
                    self.fifo.len()
                };
                let mut s = (filled as u32) << 17;
                if self.busy_forever {
                    s |= regs::STATUS_DATA_BUSY;
                } else if self.busy_reads > 0 {
                    self.busy_reads -= 1;
                    s |= regs::STATUS_DATA_BUSY;
                }
                s
            }
            regs::RINTSTS => {
                if let Some((rint, answer, polls)) = self.answer_pending {
                    if polls <= 1 {
                        self.answer_pending = None;
                        self.land(rint, answer);
                    } else {
                        self.answer_pending = Some((rint, answer, polls - 1));
                    }
                }
                if self.acd_pending > 0 {
                    self.acd_pending -= 1;
                    if self.acd_pending == 0 {
                        *self.reg(regs::RINTSTS) |= regs::INT_ACD;
                    }
                }
                self.pump();
                *self.reg(regs::RINTSTS)
            }
            _ => *self.reg(offset),
        }
    }

    fn write(&mut self, offset: u32, value: u32) {
        let fifo = regs::fifo_offset(self.verid);
        if offset == fifo {
            if !self.transfer.as_ref().is_some_and(|t| t.write) {
                self.stray_fifo_write = true;
            }
            assert!(
                self.fifo.len() < self.depth,
                "FIFO overrun: the driver wrote into a full FIFO"
            );
            self.fifo.push_back(value);
            return;
        }
        self.writes.push((offset, value));
        match offset {
            regs::RINTSTS => *self.reg(regs::RINTSTS) &= !value,
            regs::IDSTS => *self.reg(regs::IDSTS) &= !value,
            regs::BMOD => {
                if value & regs::BMOD_SOFTWARE_RESET != 0
                    || value & regs::BMOD_DMA_ENABLE & !self.peek(regs::BMOD) != 0
                {
                    self.walk = Walk::Idle;
                    self.walked = 0;
                }
                *self.reg(regs::BMOD) = value;
            }
            regs::DBADDR => {
                self.walk = Walk::Idle;
                self.walked = 0;
                *self.reg(regs::DBADDR) = value;
            }
            regs::PLDMND => {
                if let Walk::Suspended(d) = self.walk {
                    self.walk = Walk::At(d, 0);
                }
            }
            regs::CTRL => {
                if value & regs::CTRL_DMA_RESET != 0 {
                    self.walk = Walk::Idle;
                }
                if value & regs::CTRL_ALL_RESET != 0 {
                    self.reset_pending = !self.reset_stuck;
                    if value & regs::CTRL_FIFO_RESET != 0 {
                        self.fifo.clear();
                    }
                    if value & regs::CTRL_CONTROLLER_RESET != 0 {
                        self.transfer = None;
                    }
                }
                *self.reg(regs::CTRL) = value;
            }
            regs::CMD => {
                if value & regs::CMD_START != 0 && value & regs::CMD_UPDATE_CLOCK_ONLY == 0 {
                    self.command(value);
                }
                let stuck = self.clock_stuck && value & regs::CMD_UPDATE_CLOCK_ONLY != 0;
                *self.reg(regs::CMD) = if stuck {
                    value
                } else {
                    value & !regs::CMD_START
                };
            }
            regs::VERID | regs::HCON | regs::STATUS => {}
            _ => *self.reg(offset) = value,
        }
    }

    fn now_us(&mut self) -> u64 {
        self.now += self.clock_step;
        self.now
    }
}
