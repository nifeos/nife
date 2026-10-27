//! A simulated controller with a card behind it, for the host tests. QEMU has no `DW_mshc` model
//! for any RISC-V machine (its one `dw_mmc`-shaped device, on some Arm boards, is unreachable from
//! the machines this tree boots), so the tests run the real driver against this and the bench step
//! checks this against silicon. It models only the contract the driver relies on, as the databook
//! and OpenBSD describe it: self-clearing `CTRL` resets, the `CMD` start bit, a FIFO raising
//! `RXDR`, `TXDR`, `DTO` and `ACD`, `STATUS` fill and busy, `CARDTHRCTL` above the FIFO ending in
//! `FRUN` and `DTO` as on radon, and the IDMAC walk, whose last read words reach memory after
//! `DTO`. Time is one microsecond per `now_microseconds` call, so a test that would hang on silicon fails
//! here.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use std::vec::Vec;

use crate::host::Registers;
use crate::internal_direct_memory_access::{
    DES0_CH, DES0_DIC, DES0_LD, DES0_OWN, DES1_BS1_MASK, DirectMemoryAccessRegion,
};
use crate::registers;

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
    pub(crate) fn new(base: u64, pages: usize) -> Memory {
        Memory(Arc::new(Mutex::new(Bus {
            base,
            bytes: vec![0; pages * 4096],
            ..Bus::default()
        })))
    }

    /// The bus, locked.
    pub(crate) fn bus(&self) -> MutexGuard<'_, Bus> {
        self.0.lock().unwrap()
    }
}

impl DirectMemoryAccessRegion for Memory {
    fn bus_address(&self) -> u64 {
        self.bus().base
    }
    fn bytes(&self) -> u32 {
        u32::try_from(self.bus().bytes.len()).unwrap()
    }
    fn read_u32(&self, offset: u32) -> u32 {
        let b = &self.bus().bytes[offset as usize..offset as usize + 4];
        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
    }
    fn write_u32(&mut self, offset: u32, value: u32) {
        let mut bus = self.bus();
        bus.cpu_unfenced = true;
        bus.bytes[offset as usize..offset as usize + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn read_bytes(&self, offset: u32, out: &mut [u8]) {
        let mut bus = self.bus();
        if bus.device_unfenced {
            bus.unfenced_read = true;
        }
        out.copy_from_slice(&bus.bytes[offset as usize..offset as usize + out.len()]);
    }
    fn write_bytes(&mut self, offset: u32, source: &[u8]) {
        let mut bus = self.bus();
        bus.cpu_unfenced = true;
        bus.bytes[offset as usize..offset as usize + source.len()].copy_from_slice(source);
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
    MultimediaCard,
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
    first_logical_block_address: u64,
    bytes: usize,
    moved: usize,
    auto_stop: bool,
    buffer: Vec<u8>,
    extended_card_specific_data: bool,
}

/// The simulation. Public fields are what a test sets up or checks.
pub struct Sim {
    registers: [u32; 0x220 / 4],
    reset_pending: bool,
    now: u64,
    queue: VecDeque<u32>,
    depth: usize,
    transfer: Option<Transfer>,
    state: State,
    app: bool,
    power_up_polls: u32,
    relative_card_address: u16,
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
    pub stray_queue_write: bool,
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
    pub multimedia_card_byte_addressed: bool,
    /// What `EXT_CSD`'s `SEC_COUNT` reports, if not the card's real size.
    pub extended_card_specific_data_sectors: Option<u32>,
    /// Store one block of one write command wrongly: `(write command, from 0; block)`.
    pub mangle_write: Option<(usize, u64)>,
    /// Microseconds per call to `now_microseconds`.
    pub clock_step: u64,
    /// What the IDMAC reaches, if a test gave it any.
    pub memory: Option<Memory>,
    walk: Walk,
    /// The bus refuses the buffer of the descriptor walked at this position (from 0) of a chain.
    pub bus_error_at: Option<u32>,
    /// The descriptor walked at this position (from 0) of a chain reads as the CPU's: a chain the
    /// driver did not finish handing over.
    pub unowned_at: Option<u32>,
    /// This many `CTRL` reset requests clear; every later one holds, as `reset_stuck` does.
    pub control_resets_cleared: Option<u32>,
    /// This many `BMOD` soft resets clear; every later one holds.
    pub bus_mode_resets_cleared: Option<u32>,
    walked: u32,
    /// The IDMAC moves at most this many words each time the sim advances: a slow bus, which lets
    /// a read's `DTO` land well before its last words reach memory.
    pub direct_memory_access_words_per_step: Option<usize>,
    /// The IDMAC flips bit 0 of every word it writes to memory: a bus that corrupts.
    pub direct_memory_access_corrupts: bool,
    /// The direction of the last data command: the way the IDMAC moves words.
    direct_memory_access_write: bool,
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
            registers: [0; 0x220 / 4],
            reset_pending: false,
            now: 0,
            queue: VecDeque::new(),
            depth,
            transfer: None,
            state: State::Idle,
            app: false,
            power_up_polls: 0,
            relative_card_address: 0,
            busy_reads: 0,
            kind,
            storage: vec![[0u8; 512]; blocks],
            commands: Vec::new(),
            written: Vec::new(),
            fail_next_read_crc: false,
            busy_polls: 3,
            clockless_command: false,
            verid: VERID,
            stray_queue_write: false,
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
            multimedia_card_byte_addressed: false,
            extended_card_specific_data_sectors: None,
            mangle_write: None,
            clock_step: 1,
            memory: None,
            walk: Walk::Idle,
            bus_error_at: None,
            unowned_at: None,
            control_resets_cleared: None,
            bus_mode_resets_cleared: None,
            walked: 0,
            direct_memory_access_words_per_step: None,
            direct_memory_access_corrupts: false,
            direct_memory_access_write: false,
        };
        s.registers[(registers::VERID / 4) as usize] = VERID;
        s.registers[(registers::HCON / 4) as usize] = HCON;
        // FIFOTH's reset value carries depth - 1 as the receive watermark.
        s.registers[(registers::FIFOTH / 4) as usize] = ((depth as u32 - 1) & 0xfff) << 16;
        // A card is present (active low).
        s.registers[(registers::CDETECT / 4) as usize] = u32::from(kind == CardKind::Empty);
        s
    }

    /// The same controller, reporting release `verid` instead (an older one puts the FIFO at
    /// `0x100`, where a later one has the card threshold register).
    #[must_use]
    pub(crate) fn with_verid(mut self, verid: u32) -> Sim {
        self.verid = verid;
        self.registers[(registers::VERID / 4) as usize] = verid;
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
        self.registers[(offset / 4) as usize]
    }

    fn register(&mut self, offset: u32) -> &mut u32 {
        &mut self.registers[(offset / 4) as usize]
    }

    fn high_capacity(&self) -> bool {
        !matches!(self.kind, CardKind::SdV1 | CardKind::SdV2)
            && !(self.kind == CardKind::MultimediaCard && self.multimedia_card_byte_addressed)
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

    fn card_specific_data(&self) -> [u32; 4] {
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

    fn card_identification(&self) -> [u32; 4] {
        // Manufacturer 0x1b, product "NIFE1", serial 0x1234_5678.
        let mut r = [0x1234_5678u32 << 24, 0x1234_5678 >> 8, 0, 0x1b00_0000];
        for (i, b) in b"NIFE1".iter().enumerate() {
            let bit = 96 - 8 * i as u32;
            r[(bit / 32) as usize] |= u32::from(*b) << (bit % 32);
        }
        r
    }

    fn logical_block_address(&self, argument: u32) -> u64 {
        if self.high_capacity() {
            u64::from(argument)
        } else {
            u64::from(argument) / 512
        }
    }

    /// The card's answer to one command: `None` is no response.
    fn card(&mut self, index: u8, argument: u32) -> Option<[u32; 4]> {
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
                short(argument & 0xfff)
            }
            (55, _, CardKind::MultimediaCard) => None,
            (55, _, _) => {
                self.app = true;
                short(self.r1() | 1 << 5)
            }
            (41, true, _) | (1, false, CardKind::MultimediaCard) => {
                self.power_up_polls += 1;
                let mut operation_conditions = 0x00ff_8000;
                if self.power_up_polls > self.busy_polls {
                    operation_conditions |= 1 << 31;
                    if self.high_capacity() && (index == 1 || argument & (1 << 30) != 0) {
                        operation_conditions |= 1 << 30;
                    }
                    self.state = State::Ready;
                }
                short(operation_conditions)
            }
            (2, false, _) if self.state == State::Ready => {
                self.state = State::Ident;
                Some(self.card_identification())
            }
            (3, false, CardKind::MultimediaCard) if self.state == State::Ident => {
                self.relative_card_address = (argument >> 16) as u16;
                let r = self.r1();
                self.state = State::Standby;
                short(r)
            }
            (3, false, _) if self.state == State::Ident => {
                self.relative_card_address = 0xb368;
                self.state = State::Standby;
                short(u32::from(self.relative_card_address) << 16 | 0x0500)
            }
            (9, false, _)
                if self.state == State::Standby
                    && (argument >> 16) as u16 == self.relative_card_address =>
            {
                Some(self.card_specific_data())
            }
            (7, false, _) if (argument >> 16) as u16 == self.relative_card_address => {
                let r = self.r1();
                self.state = State::Transfer;
                short(r)
            }
            (6, true, _) | (16, false, _) | (13, false, _) if self.state == State::Transfer => {
                short(self.r1())
            }
            (8, false, CardKind::MultimediaCard) | (17 | 18 | 24 | 25, false, _)
                if self.state == State::Transfer =>
            {
                short(self.r1())
            }
            _ => None,
        }
    }

    fn command(&mut self, word: u32) {
        let index = (word & 0x3f) as u8;
        let argument = *self.register(registers::CMDARG);
        self.commands.push((index, argument));
        if *self.register(registers::CLKENA) & registers::CLKENA_CCLK_ENABLE == 0 {
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
            self.card(index, argument)
        };
        if let Some(r) = answer.as_mut() {
            match fault {
                Some(Fault::ErrorStatus) => r[0] |= 1 << 19,
                Some(Fault::EchoWrong) => r[0] ^= 0xff,
                _ => {}
            }
        }
        let mut rint = registers::RINTSTS_CD;
        match answer {
            None if word & registers::CMD_RESPONSE_EXPECT != 0 => rint |= registers::RINTSTS_RTO,
            None => {}
            Some(_) if fault == Some(Fault::BadCrc) => rint |= registers::RINTSTS_RCRC,
            Some(_) => {}
        }
        if self.late_answer > 0 {
            self.answer_pending = Some((rint, answer, self.late_answer));
        } else {
            self.land(rint, answer);
        }
        if answer.is_some() && word & registers::CMD_DATA_EXPECTED != 0 {
            let bytes = *self.register(registers::BYTCNT) as usize;
            let write = word & registers::CMD_WR != 0;
            self.direct_memory_access_write = write;
            if self.internal_direct_memory_access_active()
                && let Some(m) = &self.memory
                && m.bus().cpu_unfenced
            {
                m.bus().unfenced_start = true;
            }
            let extended_card_specific_data = index == 8;
            let mut t = Transfer {
                write,
                first_logical_block_address: if extended_card_specific_data {
                    0
                } else {
                    self.logical_block_address(argument)
                },
                bytes,
                moved: 0,
                auto_stop: word & registers::CMD_SEND_AUTO_STOP != 0,
                buffer: Vec::new(),
                extended_card_specific_data,
            };
            if !write {
                if extended_card_specific_data {
                    let mut e = vec![0u8; 512];
                    let sectors = self
                        .extended_card_specific_data_sectors
                        .unwrap_or(self.storage.len() as u32);
                    e[212..216].copy_from_slice(&sectors.to_le_bytes());
                    t.buffer = e;
                } else {
                    for i in 0..bytes / 512 {
                        let logical_block_address = t.first_logical_block_address as usize + i;
                        t.buffer.extend_from_slice(
                            self.storage
                                .get(logical_block_address)
                                .map_or(&[0xffu8; 512][..], |b| &b[..]),
                        );
                    }
                }
            }
            self.transfer = Some(t);
        }
    }

    fn land(&mut self, rint: u32, answer: Option<[u32; 4]>) {
        if let Some(r) = answer {
            *self.register(registers::RESP0) = r[0];
            *self.register(registers::RESP1) = r[1];
            *self.register(registers::RESP2) = r[2];
            *self.register(registers::RESP3) = r[3];
        }
        *self.register(registers::RINTSTS) |= rint;
    }

    /// Advance the data phase: fill or drain the FIFO, and end it when every byte has moved.
    fn pump(&mut self) {
        self.internal_direct_memory_access_step();
        self.pump_card();
        self.internal_direct_memory_access_step();
    }

    /// The card's side of the data phase.
    fn pump_card(&mut self) {
        let Some(mut t) = self.transfer.take() else {
            return;
        };
        if t.write {
            let limit = self.stall_after.unwrap_or(usize::MAX);
            while t.buffer.len() < limit
                && let Some(w) = self.queue.pop_front()
            {
                t.buffer.extend_from_slice(&w.to_le_bytes());
            }
            if t.buffer.len() >= t.bytes {
                let store = self.writes_stored.is_none_or(|n| self.write_commands < n);
                let nth = self.write_commands;
                self.write_commands += 1;
                for (i, block) in t.buffer.as_chunks::<512>().0.iter().enumerate() {
                    let logical_block_address = t.first_logical_block_address + i as u64;
                    self.written.push(logical_block_address);
                    let mangle = self.mangle_write == Some((nth, logical_block_address));
                    if let Some(b) = self.storage.get_mut(logical_block_address as usize)
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
            *self.register(registers::RINTSTS) |= registers::RINTSTS_TXDR;
        } else {
            if self.threshold_beyond_queue() {
                // What radon answered (`RINTSTS 0x080c` with `CD`): the block ends, overrun.
                *self.register(registers::RINTSTS) |=
                    registers::RINTSTS_FRUN | registers::RINTSTS_DTO;
                return;
            }
            if self.fail_next_read_crc && !t.extended_card_specific_data {
                self.fail_next_read_crc = false;
                *self.register(registers::RINTSTS) |=
                    registers::RINTSTS_DCRC | registers::RINTSTS_DTO;
                return;
            }
            let limit = self.stall_after.unwrap_or(usize::MAX);
            while self.queue.len() < self.depth && t.moved < t.bytes && t.moved < limit {
                let b = &t.buffer[t.moved..t.moved + 4];
                self.queue
                    .push_back(u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                t.moved += 4;
            }
            let watermark = if self.strict_watermark {
                registers::fifoth_receive_watermark(*self.register(registers::FIFOTH)) as usize
            } else {
                0
            };
            if self.queue.len() > watermark {
                *self.register(registers::RINTSTS) |= registers::RINTSTS_RXDR;
            }
            if t.moved >= t.bytes || (self.end_early && t.moved >= limit) {
                self.end(&t);
                return;
            }
        }
        self.transfer = Some(t);
    }

    fn internal_direct_memory_access_active(&self) -> bool {
        let both = registers::CTRL_USE_INTERNAL_DMAC | registers::CTRL_DMA_ENABLE;
        self.peek(registers::CTRL) & both == both
            && self.peek(registers::BMOD) & registers::BMOD_DE != 0
            && self.memory.is_some()
    }

    /// **One stretch of the IDMAC's walk**: move words between the FIFO and memory until the FIFO
    /// is empty (a read) or full (a write), or the walk stops.
    fn internal_direct_memory_access_step(&mut self) {
        if !self.internal_direct_memory_access_active() {
            return;
        }
        let Some(memory) = self.memory.clone() else {
            return;
        };
        let mut bus = memory.bus();
        let mut budget = self
            .direct_memory_access_words_per_step
            .unwrap_or(usize::MAX);
        loop {
            if budget == 0 {
                return;
            }
            budget -= 1;
            let more = if self.direct_memory_access_write {
                self.queue.len() < self.depth
            } else {
                !self.queue.is_empty()
            };
            if !more {
                return;
            }
            let (desc, done) = match self.walk {
                Walk::Idle => (self.peek(registers::DBADDR), 0),
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
            if des0 & DES0_OWN == 0 || done == 0 && self.unowned_at == Some(self.walked) {
                self.walk = Walk::Suspended(desc);
                *self.register(registers::IDSTS) |= registers::IDSTS_DU | registers::IDSTS_AIS;
                return;
            }
            if done == 0 && self.bus_error_at == Some(self.walked) {
                self.halt();
                return;
            }
            let Some(buffer) = self.offset(&bus, buffer.wrapping_add(done), 4) else {
                self.halt();
                return;
            };
            if self.direct_memory_access_write {
                let b = &bus.bytes[buffer..buffer + 4];
                self.queue
                    .push_back(u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
            } else if let Some(w) = self.queue.pop_front() {
                let w = w ^ u32::from(self.direct_memory_access_corrupts);
                bus.bytes[buffer..buffer + 4].copy_from_slice(&w.to_le_bytes());
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
                let finished = if self.direct_memory_access_write {
                    registers::IDSTS_TI
                } else {
                    registers::IDSTS_RI
                };
                *self.register(registers::IDSTS) |= finished | registers::IDSTS_NIS;
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

    /// The byte offset in memory of `len` bytes at bus address `address`, if they are all there.
    fn offset(&self, bus: &Bus, address: u32, len: usize) -> Option<usize> {
        let off = usize::try_from(u64::from(address).checked_sub(bus.base)?).ok()?;
        (off + len <= bus.bytes.len()).then_some(off)
    }

    /// Whether a reset request holds, given how many more `left` will clear.
    fn holds(left: &mut Option<u32>) -> bool {
        match left {
            Some(0) => true,
            Some(n) => {
                *n -= 1;
                false
            }
            None => false,
        }
    }

    fn halt(&mut self) {
        self.walk = Walk::Halted;
        *self.register(registers::IDSTS) |= registers::IDSTS_FBE | registers::IDSTS_AIS;
    }

    /// A card read threshold enabled above the FIFO's size in bytes, which the databook does not
    /// allow and Linux's `dw_mci_ctrl_thld` refuses to program.
    fn threshold_beyond_queue(&self) -> bool {
        if registers::queue_offset(self.verid) == registers::CARDTHRCTL {
            return false;
        }
        let t = self.peek(registers::CARDTHRCTL);
        t & registers::CARDTHRCTL_RDTHREN != 0 && (t >> 16) & 0xfff > self.depth as u32 * 4
    }

    fn end(&mut self, t: &Transfer) {
        if t.write && self.crc_error_at_end {
            *self.register(registers::RINTSTS) |= registers::RINTSTS_DCRC;
            return;
        }
        let mut rint = registers::RINTSTS_DTO;
        if t.auto_stop {
            self.commands.push((12, 0));
            if self.late_acd > 0 {
                self.acd_pending = self.late_acd;
            } else {
                rint |= registers::RINTSTS_ACD;
            }
        }
        *self.register(registers::RINTSTS) |= rint;
    }
}

impl Registers for Sim {
    fn read(&mut self, offset: u32) -> u32 {
        let queue_offset = registers::queue_offset(self.verid);
        if offset == queue_offset {
            let w = self.queue.pop_front().unwrap_or(0xdead_dead);
            self.pump();
            return w;
        }
        match offset {
            registers::CTRL if self.reset_pending => {
                // The bits read as set once, then clear: the driver must poll, not assume.
                self.reset_pending = false;
                let v = *self.register(registers::CTRL);
                *self.register(registers::CTRL) &= !registers::CTRL_ALL_RESET;
                v
            }
            registers::BMOD if self.peek(registers::BMOD) & registers::BMOD_SWR != 0 => {
                let v = self.peek(registers::BMOD);
                if !self.reset_stuck && !Self::holds(&mut self.bus_mode_resets_cleared) {
                    *self.register(registers::BMOD) &= !registers::BMOD_SWR;
                }
                v
            }
            registers::STATUS => {
                self.pump();
                let filled = if self.overreport_fill {
                    self.depth
                } else {
                    self.queue.len()
                };
                let mut s = (filled as u32) << 17;
                if self.busy_forever {
                    s |= registers::STATUS_DATA_BUSY;
                } else if self.busy_reads > 0 {
                    self.busy_reads -= 1;
                    s |= registers::STATUS_DATA_BUSY;
                }
                s
            }
            registers::RINTSTS => {
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
                        *self.register(registers::RINTSTS) |= registers::RINTSTS_ACD;
                    }
                }
                self.pump();
                *self.register(registers::RINTSTS)
            }
            _ => *self.register(offset),
        }
    }

    fn write(&mut self, offset: u32, value: u32) {
        let queue_offset = registers::queue_offset(self.verid);
        if offset == queue_offset {
            if !self.transfer.as_ref().is_some_and(|t| t.write) {
                self.stray_queue_write = true;
            }
            assert!(
                self.queue.len() < self.depth,
                "FIFO overrun: the driver wrote into a full FIFO"
            );
            self.queue.push_back(value);
            return;
        }
        self.writes.push((offset, value));
        match offset {
            registers::RINTSTS => *self.register(registers::RINTSTS) &= !value,
            registers::IDSTS => *self.register(registers::IDSTS) &= !value,
            registers::BMOD => {
                if value & registers::BMOD_SWR != 0
                    || value & registers::BMOD_DE & !self.peek(registers::BMOD) != 0
                {
                    self.walk = Walk::Idle;
                    self.walked = 0;
                }
                *self.register(registers::BMOD) = value;
            }
            registers::DBADDR => {
                self.walk = Walk::Idle;
                self.walked = 0;
                *self.register(registers::DBADDR) = value;
            }
            registers::PLDMND => {
                if let Walk::Suspended(d) = self.walk {
                    self.walk = Walk::At(d, 0);
                }
            }
            registers::CTRL => {
                if value & registers::CTRL_DMA_RESET != 0 {
                    self.walk = Walk::Idle;
                }
                if value & registers::CTRL_ALL_RESET != 0 {
                    self.reset_pending =
                        !self.reset_stuck && !Self::holds(&mut self.control_resets_cleared);
                    if value & registers::CTRL_FIFO_RESET != 0 {
                        self.queue.clear();
                    }
                    if value & registers::CTRL_CONTROLLER_RESET != 0 {
                        self.transfer = None;
                    }
                }
                *self.register(registers::CTRL) = value;
            }
            registers::CMD => {
                if value & registers::CMD_START_CMD != 0
                    && value & registers::CMD_UPDATE_CLOCK_REGISTERS_ONLY == 0
                {
                    self.command(value);
                }
                let stuck =
                    self.clock_stuck && value & registers::CMD_UPDATE_CLOCK_REGISTERS_ONLY != 0;
                *self.register(registers::CMD) = if stuck {
                    value
                } else {
                    value & !registers::CMD_START_CMD
                };
            }
            registers::VERID | registers::HCON | registers::STATUS => {}
            _ => *self.register(offset) = value,
        }
    }

    fn now_microseconds(&mut self) -> u64 {
        self.now += self.clock_step;
        self.now
    }
}
