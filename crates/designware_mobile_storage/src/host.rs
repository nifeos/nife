//! The controller, polled: reset, power, clock, bus width, and one command with or without a data
//! phase. Adapted from OpenBSD's `dwmmc_attach`, `dwmmc_bus_clock`, `dwmmc_bus_width`,
//! `dwmmc_pio_mode`, `dwmmc_exec_command` and `dwmmc_transfer_data` (the crate root carries the
//! notice), with deliberate differences: every wait is a deadline on a clock the caller supplies
//! rather than a count of `delay(100)`s, nothing sleeps, the interrupt line stays masked, and a
//! 64-bit FIFO is refused (radon's is measured by the bench step before anything depends on it).
//! `Host<R>` moves every word through the FIFO by MMIO; `Host<R, D>`, from
//! [`Host::with_internal_direct_memory_access`], hands each data phase to the controller's DMA
//! engine ([`crate::internal_direct_memory_access`]). Commands and identification are the same on
//! both.

use crate::command::{Command, UPDATE_CLOCK};
use crate::internal_direct_memory_access::{
    DirectMemoryAccessRegion, InternalDirectMemoryAccess, NoDirectMemoryAccess, PAGE,
};
use crate::registers;

/// **The register window, and a clock**: everything the driver needs from the machine.
///
/// The kernel implements it over a mapped device window and the architecture's counter; the tests
/// implement it over `sim`. Rule 2 in one trait: the driver is handed a window and knows
/// nothing about where it is.
pub trait Registers {
    /// Read the 32-bit register at `offset` bytes from the base.
    fn read(&mut self, offset: u32) -> u32;
    /// Write the 32-bit register at `offset`.
    fn write(&mut self, offset: u32, value: u32);
    /// Microseconds since some fixed point. Only differences are used.
    fn now_us(&mut self) -> u64;
}

/// What went wrong, with enough to say where.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// `HCON` names a FIFO width this driver does not move: the raw width field.
    #[doc(alias = "fifo")]
    QueueWidth(u32),
    /// A reset bit in `CTRL` or `BMOD` did not clear: the word last read.
    ResetStuck(u32),
    /// The controller did not take a clock update (the start bit stayed set).
    ClockUpdate,
    /// The controller did not take a command: its index.
    CommandNotTaken(u8),
    /// No command-done within the deadline: the index and `RINTSTS`.
    CommandTimeout(u8, u32),
    /// No card answered (`RTO`): the index.
    NoResponse(u8),
    /// The response failed its CRC or was malformed: the index and `RINTSTS`.
    BadResponse(u8, u32),
    /// The data phase failed: the index and `RINTSTS`.
    Data(u8, u32),
    /// The data phase did not finish: the index, `RINTSTS`, and the bytes still owed.
    DataTimeout(u8, u32, u32),
    /// The buffer is not the size the command's data phase says.
    BufferSize,
    /// The card held DAT0 busy past the deadline.
    Busy,
    /// The IDMAC stopped on a bus error or a descriptor it did not own: the index and `IDSTS`.
    DirectMemoryAccess(u8, u32),
}

/// The controller's identity, read before anything is written: what a bench transcript prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    /// `VERID`.
    pub verid: u32,
    /// `HCON`.
    pub hcon: u32,
    /// `USRID`.
    pub usrid: u32,
    /// `FIFOTH` as found, whose receive watermark is `depth - 1` out of reset.
    pub fifoth: u32,
    /// `CDETECT` as found: bit 0 clear means a card is present.
    pub cdetect: u32,
    /// `CTRL`, `PWREN`, `CLKDIV`, `CLKENA` and `CTYPE` as the firmware left them.
    pub firmware: [u32; 5],
}

/// How long a reset, a clock update or a command may take before the driver gives up. Generous:
/// these are fixed costs of a few card clocks at most, and a deadline exists only so a dead
/// controller is a reported error rather than a hung boot.
pub(crate) const COMMAND_DEADLINE_US: u64 = 100_000;
/// How long a data phase may go without progress before it is abandoned. A card may take 250 ms
/// to start a write (the SD spec's write timeout for SDHC); half a second is that with slack.
pub(crate) const DATA_DEADLINE_US: u64 = 500_000;

/// **The card clock divider** for a `target` from a `base`: the smallest `divider` with
/// `base / (2 * divider) <= target`, or 0 (bypass) when `base` itself is low enough. `None` when
/// even 255 is too fast.
///
/// OpenBSD's loop, with its edge closed: OpenBSD exits at `div = 256` and writes that to an
/// eight-bit field, which reads back as 0, the fastest setting there is.
///
/// ```
/// use designware_mobile_storage::host::{clock_divider, card_clock};
///
/// // radon's controllers run from 50 MHz (both trees' `assigned-clock-rates`).
/// let d = clock_divider(50_000_000, 400_000).unwrap();
/// assert_eq!(d, 63);
/// assert!(card_clock(50_000_000, d) <= 400_000);
/// assert_eq!(clock_divider(50_000_000, 50_000_000), Some(0));
/// assert_eq!(clock_divider(50_000_000, 25_000_000), Some(1));
/// ```
#[must_use]
pub const fn clock_divider(base: u32, target: u32) -> Option<u8> {
    if target == 0 {
        return None;
    }
    if base <= target {
        return Some(0);
    }
    // ceil(base / (2 * target)). Saturating, and still exact: a target of 2^31 or more saturates
    // to u32::MAX, and since base > target the quotient is then 1 either way.
    let divider = base.div_ceil(target.saturating_mul(2));
    if divider > 255 {
        None
    } else {
        Some(divider as u8)
    }
}

/// The card clock a divider gives: `base` at 0, `base / (2 * divider)` otherwise.
#[must_use]
pub const fn card_clock(base: u32, divider: u8) -> u32 {
    if divider == 0 {
        base
    } else {
        base / (2 * divider as u32)
    }
}

/// **How many bytes the next FIFO step moves**: the FIFO's filled words (reading) or empty words
/// (writing), in bytes, never more than `remaining`. A count the controller reports above `depth`
/// is clamped to `depth`, so a confused `STATUS` cannot make a write overrun the FIFO.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): `fifo` becomes `queue` in every name this tree
/// authors; was `fifo_step`.
#[doc(alias = "fifo")]
#[must_use]
pub const fn queue_step(status: u32, depth: u32, write: bool, remaining: u32) -> u32 {
    let filled = registers::queue_count(status);
    let filled = if filled > depth { depth } else { filled };
    let words = if write { depth - filled } else { filled };
    let bytes = words.saturating_mul(4);
    if bytes < remaining { bytes } else { remaining }
}

/// The driver: a window, the FIFO's place and depth, the card clock's source rate, and, when the
/// data moves by DMA, the IDMAC's region. `Host<R>` is the polled driver;
/// [`Host::with_internal_direct_memory_access`] makes a `Host<R, D>` whose every data phase goes
/// through the IDMAC.
pub struct Host<R: Registers, D: DirectMemoryAccessRegion = NoDirectMemoryAccess> {
    pub(crate) registers: R,
    queue_offset: u32,
    pub(crate) depth: u32,
    base_hz: u32,
    pub(crate) direct_memory_access: Option<InternalDirectMemoryAccess<D>>,
}

impl<R: Registers> Host<R> {
    /// **Take a controller**, reading its identity and touching nothing.
    ///
    /// `base_hz` is the rate of the clock the controller divides (the tree's `ciu`, 50 MHz on
    /// radon), and `queue_depth` the tree's `fifo-depth` if it has one; without it the depth is
    /// read from `FIFOTH`'s reset value, as OpenBSD does. Refuses a FIFO that is not 32 bits wide.
    ///
    /// # Errors
    ///
    /// [`Error::QueueWidth`] for a 16- or 64-bit FIFO.
    pub fn new(
        mut registers: R,
        base_hz: u32,
        queue_depth: Option<u32>,
    ) -> Result<(Self, Identity), Error> {
        let id = Identity {
            verid: registers.read(registers::VERID),
            hcon: registers.read(registers::HCON),
            usrid: registers.read(registers::USRID),
            fifoth: registers.read(registers::FIFOTH),
            cdetect: registers.read(registers::CDETECT),
            firmware: [
                registers.read(registers::CTRL),
                registers.read(registers::PWREN),
                registers.read(registers::CLKDIV),
                registers.read(registers::CLKENA),
                registers.read(registers::CTYPE),
            ],
        };
        if registers::queue_width(id.hcon) != Some(4) {
            return Err(Error::QueueWidth((id.hcon >> 7) & 7));
        }
        let depth = match queue_depth {
            Some(d) if d > 0 => d,
            _ => registers::fifoth_receive_watermark(id.fifoth) + 1,
        };
        Ok((
            Host {
                registers,
                queue_offset: registers::queue_offset(id.verid),
                depth,
                base_hz,
                direct_memory_access: None,
            },
            id,
        ))
    }

    /// **Move every later data phase through the IDMAC**, over a region
    /// [`InternalDirectMemoryAccess::new`] checked. The controller's state carries over: a card
    /// identified by the polled host stays selected.
    ///
    /// Name: provisional. Ruled 2026-10-26 (UTC): `idmac` and the names built on it spell out
    /// `internal_direct_memory_access`, with `doc(alias = "idmac")`; was `with_idmac`.
    #[doc(alias = "idmac")]
    #[must_use]
    pub fn with_internal_direct_memory_access<D: DirectMemoryAccessRegion>(
        self,
        internal_direct_memory_access: InternalDirectMemoryAccess<D>,
    ) -> Host<R, D> {
        Host {
            registers: self.registers,
            queue_offset: self.queue_offset,
            depth: self.depth,
            base_hz: self.base_hz,
            direct_memory_access: Some(internal_direct_memory_access),
        }
    }
}

impl<R: Registers, D: DirectMemoryAccessRegion> Host<R, D> {
    /// The FIFO depth in words, as decided by [`Host::new`].
    ///
    /// Name: provisional. Ruled 2026-10-26 (UTC): `fifo` becomes `queue` in every name this tree
    /// authors; was `fifo_depth`.
    #[doc(alias = "fifo")]
    #[must_use]
    pub fn queue_depth(&self) -> u32 {
        self.depth
    }

    /// The window, for a caller that needs a register this driver does not name.
    pub fn registers(&mut self) -> &mut R {
        &mut self.registers
    }

    /// Give the window back.
    pub fn into_registers(self) -> R {
        self.registers
    }

    pub(crate) fn wait<F: FnMut(&mut R) -> bool>(&mut self, deadline_us: u64, mut done: F) -> bool {
        let start = self.registers.now_us();
        loop {
            if done(&mut self.registers) {
                return true;
            }
            if self.registers.now_us().wrapping_sub(start) > deadline_us {
                return done(&mut self.registers);
            }
        }
    }

    /// **Reset the controller, the FIFO and the DMA interface**, mask every interrupt, clear the
    /// raw status, and leave the bus 1-bit wide with the FIFO thresholds set for polling. The card
    /// clock is off afterwards; [`Host::set_clock`] turns it on.
    ///
    /// # Errors
    ///
    /// [`Error::ResetStuck`] if a reset bit does not clear.
    pub fn reset(&mut self) -> Result<(), Error> {
        self.registers.write(registers::CLKENA, 0);
        let ctrl = self.registers.read(registers::CTRL)
            & !(registers::CTRL_USE_INTERNAL_DMAC
                | registers::CTRL_DMA_ENABLE
                | registers::CTRL_INT_ENABLE);
        self.registers
            .write(registers::CTRL, ctrl | registers::CTRL_ALL_RESET);
        if !self.wait(COMMAND_DEADLINE_US, |window| {
            window.read(registers::CTRL) & registers::CTRL_ALL_RESET == 0
        }) {
            return Err(Error::ResetStuck(self.registers.read(registers::CTRL)));
        }
        self.registers.write(registers::INTMASK, 0);
        self.registers.write(registers::RINTSTS, 0xffff_ffff);
        // Longest response timeout (255 clocks) and data timeout (2^24 - 1 clocks): the deadlines
        // above are the real limit, and these only keep the controller from giving up first.
        self.registers.write(registers::TMOUT, 0xffff_ffff);
        self.registers.write(registers::CTYPE, 0);
        self.registers
            .write(registers::FIFOTH, registers::fifoth_for_pio(self.depth));
        // The card read threshold stays off. It is for bus modes that cannot stop the card clock
        // (HS200, SDR104, HS400), which this driver does not run, and it may not exceed the FIFO:
        // OpenBSD's one block, 512 bytes, against radon's 128-byte FIFO overran the first read.
        // Written rather than assumed, since firmware may have set it. Before release 2.40a the
        // offset is the data FIFO, and a write there would be a word of data.
        if self.queue_offset != registers::CARDTHRCTL {
            self.registers.write(registers::CARDTHRCTL, 0);
        }
        Ok(())
    }

    /// Power slot 0. The JH7110 ties the card supply on the board, so on radon this bit is what
    /// U-Boot already set; written anyway, because a controller fresh from reset may not have it.
    pub(crate) fn power_on(&mut self) {
        self.registers.write(registers::PWREN, 1);
    }

    fn update_clock(&mut self) -> Result<(), Error> {
        self.registers.write(registers::CMD, UPDATE_CLOCK);
        if self.wait(COMMAND_DEADLINE_US, |window| {
            window.read(registers::CMD) & registers::CMD_START_CMD == 0
        }) {
            Ok(())
        } else {
            Err(Error::ClockUpdate)
        }
    }

    /// **Run the card clock at no more than `hz`**, returning the rate it actually runs at.
    /// OpenBSD's sequence: clock off, divider, update; clock on, update. Low-power gating is on,
    /// since nothing here is an SDIO card.
    ///
    /// # Errors
    ///
    /// [`Error::ClockUpdate`] if the controller does not take either update; `BufferSize` is never
    /// returned. A target no divider reaches runs at the slowest the controller has.
    pub(crate) fn set_clock(&mut self, hz: u32) -> Result<u32, Error> {
        let divider = clock_divider(self.base_hz, hz).unwrap_or(255);
        self.registers.write(registers::CLKENA, 0);
        self.registers.write(registers::CLKSRC, 0);
        self.registers.write(registers::CLKDIV, u32::from(divider));
        self.update_clock()?;
        self.registers.write(
            registers::CLKENA,
            registers::CLKENA_CCLK_ENABLE | registers::CLKENA_CCLK_LOW_POWER,
        );
        self.update_clock()?;
        Ok(card_clock(self.base_hz, divider))
    }

    /// Set the bus width the controller drives: 1, 4 or 8. Anything else is 1.
    pub(crate) fn set_bus_width(&mut self, width: u8) {
        let ctype = match width {
            4 => registers::CTYPE_4BIT,
            8 => registers::CTYPE_8BIT,
            _ => 0,
        };
        self.registers.write(registers::CTYPE, ctype);
    }

    /// **Wait until the card releases DAT0**, after an R1b command or a write.
    ///
    /// # Errors
    ///
    /// [`Error::Busy`] past [`DATA_DEADLINE_US`].
    pub(crate) fn wait_not_busy(&mut self) -> Result<(), Error> {
        if self.wait(DATA_DEADLINE_US, |window| {
            window.read(registers::STATUS) & registers::STATUS_DATA_BUSY == 0
        }) {
            Ok(())
        } else {
            Err(Error::Busy)
        }
    }

    /// Send `command`, its data phase prepared first; `direct_memory_access` arms the IDMAC between
    /// the FIFO reset and the command.
    pub(crate) fn issue(
        &mut self,
        command: &Command,
        direct_memory_access: bool,
    ) -> Result<[u32; 4], Error> {
        self.wait_not_busy()?;
        if let Some(d) = command.data {
            self.registers.write(registers::BYTCNT, d.bytes());
            self.registers.write(registers::BLKSIZ, d.block_size);
            let ctrl = self.registers.read(registers::CTRL);
            self.registers
                .write(registers::CTRL, ctrl | registers::CTRL_FIFO_RESET);
            if !self.wait(COMMAND_DEADLINE_US, |window| {
                window.read(registers::CTRL) & registers::CTRL_FIFO_RESET == 0
            }) {
                return Err(Error::ResetStuck(self.registers.read(registers::CTRL)));
            }
            if direct_memory_access {
                self.internal_direct_memory_access_start()?;
            }
        }
        self.registers
            .write(registers::RINTSTS, registers::RINTSTS_ALL);
        self.registers.write(registers::CMDARG, command.argument);
        self.registers.write(registers::CMD, command.word());
        if !self.wait(COMMAND_DEADLINE_US, |window| {
            window.read(registers::CMD) & registers::CMD_START_CMD == 0
        }) {
            return Err(Error::CommandNotTaken(command.index));
        }
        let mut status = 0;
        if !self.wait(COMMAND_DEADLINE_US, |window| {
            status = window.read(registers::RINTSTS);
            status & registers::RINTSTS_CD != 0
        }) {
            return Err(Error::CommandTimeout(command.index, status));
        }
        if status & registers::RINTSTS_RTO != 0 {
            return Err(Error::NoResponse(command.index));
        }
        if status & (registers::RINTSTS_RCRC | registers::RINTSTS_RE) != 0 {
            return Err(Error::BadResponse(command.index, status));
        }
        Ok(if command.response.is_long() {
            [
                self.registers.read(registers::RESP0),
                self.registers.read(registers::RESP1),
                self.registers.read(registers::RESP2),
                self.registers.read(registers::RESP3),
            ]
        } else {
            [self.registers.read(registers::RESP0), 0, 0, 0]
        })
    }

    /// **Send a command with no data phase** and return its response, short responses in word 0.
    ///
    /// # Errors
    ///
    /// Any of the command-phase errors, and [`Error::BufferSize`] if `command` has a data phase.
    pub fn command(&mut self, command: &Command) -> Result<[u32; 4], Error> {
        if command.data.is_some() {
            return Err(Error::BufferSize);
        }
        self.issue(command, false)
    }

    /// **Send a command whose data phase reads into `buffer`.** `buffer` must be exactly the
    /// command's byte count, and a whole number of words; through the IDMAC, no more than its
    /// capacity.
    ///
    /// # Errors
    ///
    /// [`Error::BufferSize`] on a size mismatch or a write command; any command-phase error;
    /// [`Error::Data`], [`Error::DirectMemoryAccess`] or [`Error::DataTimeout`] from the data
    /// phase.
    pub fn read(&mut self, command: &Command, buffer: &mut [u8]) -> Result<[u32; 4], Error> {
        let Some(d) = command.data else {
            return Err(Error::BufferSize);
        };
        if d.write || buffer.len() as u64 != u64::from(d.bytes()) || !buffer.len().is_multiple_of(4)
        {
            return Err(Error::BufferSize);
        }
        if let Some(capacity) = self
            .direct_memory_access
            .as_ref()
            .map(InternalDirectMemoryAccess::capacity)
        {
            if d.bytes() > capacity {
                return Err(Error::BufferSize);
            }
            let resp = self.internal_direct_memory_access_transfer(command, d.bytes())?;
            if let Some(direct_memory_access) = self.direct_memory_access.as_mut() {
                direct_memory_access.region().read_bytes(PAGE, buffer);
            }
            return Ok(resp);
        }
        let resp = self.issue(command, false)?;
        let mut done = 0usize;
        while done < buffer.len() {
            let (status, rint) = self.wait_queue(
                command.index,
                registers::RINTSTS_RXDR,
                (buffer.len() - done) as u32,
            )?;
            let n = queue_step(status, self.depth, false, (buffer.len() - done) as u32) as usize;
            for chunk in buffer[done..done + n].as_chunks_mut::<4>().0 {
                *chunk = self.registers.read(self.queue_offset).to_le_bytes();
            }
            done += n;
            self.registers
                .write(registers::RINTSTS, rint & registers::RINTSTS_RXDR);
        }
        self.finish(command)?;
        Ok(resp)
    }

    /// **Send a command whose data phase writes `buffer`.** The same size rules as [`Host::read`].
    ///
    /// # Errors
    ///
    /// As [`Host::read`].
    pub fn write(&mut self, command: &Command, buffer: &[u8]) -> Result<[u32; 4], Error> {
        let Some(d) = command.data else {
            return Err(Error::BufferSize);
        };
        if !d.write
            || buffer.len() as u64 != u64::from(d.bytes())
            || !buffer.len().is_multiple_of(4)
        {
            return Err(Error::BufferSize);
        }
        if let Some(direct_memory_access) = self.direct_memory_access.as_mut() {
            if d.bytes() > direct_memory_access.capacity() {
                return Err(Error::BufferSize);
            }
            direct_memory_access.region().write_bytes(PAGE, buffer);
            let resp = self.internal_direct_memory_access_transfer(command, d.bytes())?;
            self.wait_not_busy()?;
            return Ok(resp);
        }
        let resp = self.issue(command, false)?;
        let mut done = 0usize;
        while done < buffer.len() {
            let (status, rint) = self.wait_queue(
                command.index,
                registers::RINTSTS_TXDR,
                (buffer.len() - done) as u32,
            )?;
            let n = queue_step(status, self.depth, true, (buffer.len() - done) as u32) as usize;
            for chunk in buffer[done..done + n].as_chunks::<4>().0 {
                let w = u32::from_le_bytes(*chunk);
                self.registers.write(self.queue_offset, w);
            }
            done += n;
            self.registers
                .write(registers::RINTSTS, rint & registers::RINTSTS_TXDR);
        }
        self.finish(command)?;
        self.wait_not_busy()?;
        Ok(resp)
    }

    /// Wait for the FIFO to want service (`want`) or the transfer to end; a data error ends it.
    /// Returns `STATUS` and `RINTSTS` once there is something to move.
    fn wait_queue(&mut self, index: u8, want: u32, owed: u32) -> Result<(u32, u32), Error> {
        let mut rint = 0;
        let mut status = 0;
        let depth = self.depth;
        let write = want == registers::RINTSTS_TXDR;
        let ok = self.wait(DATA_DEADLINE_US, |window| {
            rint = window.read(registers::RINTSTS);
            if rint & (registers::RINTSTS_DATA_ERR | registers::RINTSTS_DATA_TO) != 0 {
                return true;
            }
            status = window.read(registers::STATUS);
            // Something to move: the controller's request, or (at the tail of a read, below the
            // watermark) words sitting in the FIFO after the transfer is over.
            rint & want != 0 && queue_step(status, depth, write, owed) > 0
                || rint & registers::RINTSTS_DTO != 0 && queue_step(status, depth, write, owed) > 0
        });
        if rint & (registers::RINTSTS_DATA_ERR | registers::RINTSTS_DATA_TO) != 0 {
            return Err(Error::Data(index, rint));
        }
        if !ok {
            return Err(Error::DataTimeout(index, rint, owed));
        }
        Ok((status, rint))
    }

    /// Wait for data-transfer-over, and for the automatic CMD12 after a multi-block transfer.
    fn finish(&mut self, command: &Command) -> Result<(), Error> {
        let mut rint = 0;
        let want = match command.data {
            Some(d) if d.blocks > 1 => registers::RINTSTS_DTO | registers::RINTSTS_ACD,
            _ => registers::RINTSTS_DTO,
        };
        let ok = self.wait(DATA_DEADLINE_US, |window| {
            rint = window.read(registers::RINTSTS);
            rint & want == want
                || rint & (registers::RINTSTS_DATA_ERR | registers::RINTSTS_DATA_TO) != 0
        });
        if rint & (registers::RINTSTS_DATA_ERR | registers::RINTSTS_DATA_TO) != 0 {
            return Err(Error::Data(command.index, rint));
        }
        if !ok {
            return Err(Error::DataTimeout(command.index, rint, 0));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_divider_never_runs_the_card_faster_than_asked() {
        for base in [24_000_000u32, 50_000_000, 100_000_000, 200_000_000] {
            for target in [100_000u32, 400_000, 12_500_000, 25_000_000, 52_000_000] {
                if let Some(d) = clock_divider(base, target) {
                    assert!(card_clock(base, d) <= target, "{base} {target} {d}");
                }
            }
        }
        // Every target the card could be asked for, at the base radon runs: exhaustive rather
        // than a Kani harness, because two symbolic 32-bit divisions ran past twelve minutes
        // without finishing on 2026-10-06 and the domain that matters here is fifty million.
        let base = 50_000_000;
        for target in 1..=base {
            if let Some(d) = clock_divider(base, target) {
                assert!(card_clock(base, d) <= target, "{target} {d}");
                // And it is the fastest that does not exceed: one divider less would, in exact
                // arithmetic (the rate itself is floored, so compare the products).
                if d > 1 {
                    assert!(2 * u64::from(d - 1) * u64::from(target) < u64::from(base));
                }
            }
        }
        // The widest divider the field holds is 255 itself: 51 MHz to 100 kHz needs exactly it.
        assert_eq!(clock_divider(51_000_000, 100_000), Some(255));
        // 50 MHz cannot be divided below 98 kHz.
        assert_eq!(clock_divider(50_000_000, 90_000), None);
        assert_eq!(clock_divider(50_000_000, 0), None);
    }

    #[test]
    fn a_fifo_step_moves_what_is_there_and_never_more_than_is_owed() {
        let filled = |n: u32| n << 17;
        assert_eq!(queue_step(filled(8), 32, false, 4096), 32);
        assert_eq!(queue_step(filled(8), 32, true, 4096), 96);
        assert_eq!(queue_step(filled(8), 32, false, 12), 12);
        // A count above the depth (a confused STATUS) cannot overrun a write.
        assert_eq!(queue_step(filled(0x1fff), 32, true, 4096), 0);
        assert_eq!(queue_step(filled(0x1fff), 32, false, 4096), 128);
    }

    // The sequences below run against `sim`, which models the contract and, through its knobs, the
    // ways silicon can break it: a reset or a clock update that never completes, an answer that
    // takes time on the wire, a card that stalls or ends a transfer early, a confused `STATUS`.

    use std::sync::mpsc;
    use std::time::Duration;
    use std::vec::Vec;

    use crate::command::{Command, Data, Response, SEND_IF_COND, WRITE_BLOCK};
    use crate::sd::{self, IoError};
    use crate::sim::{CardKind, Fault, Sim};

    fn take(sim: Sim) -> Host<Sim> {
        let depth = u32::try_from(sim.depth()).unwrap();
        Host::new(sim, 50_000_000, Some(depth)).unwrap().0
    }

    /// Runs `f` on a worker and fails if it has not returned in ten seconds. A wait whose deadline
    /// never fires spins forever, and this turns that into a named failure rather than a hung test.
    fn within_ten_seconds<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(f());
        });
        receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the driver neither returned nor gave up: a deadline never fired")
    }

    #[test]
    fn reset_clears_the_enables_keeps_other_bits_and_asks_for_all_three_resets() {
        let mut h = take(Sim::new(CardKind::Empty, 1, 32));
        // Firmware left the DMA and interrupt enables on, bit 8, which is not the driver's, and a
        // card read threshold.
        let firmware = registers::CTRL_USE_INTERNAL_DMAC
            | registers::CTRL_DMA_ENABLE
            | registers::CTRL_INT_ENABLE
            | 1 << 8;
        h.registers().write(registers::CTRL, firmware);
        h.registers().write(
            registers::CARDTHRCTL,
            512 << 16 | registers::CARDTHRCTL_RDTHREN,
        );
        h.registers().writes.clear();
        h.reset().unwrap();
        assert_eq!(
            h.registers().writes,
            [
                (registers::CLKENA, 0),
                (registers::CTRL, 1 << 8 | registers::CTRL_ALL_RESET),
                (registers::INTMASK, 0),
                (registers::RINTSTS, 0xffff_ffff),
                (registers::TMOUT, 0xffff_ffff),
                (registers::CTYPE, 0),
                (registers::FIFOTH, registers::fifoth_for_pio(32)),
                (registers::CARDTHRCTL, 0),
            ]
        );
        // A FIFO reset still in flight when the driver starts is asked for again, not toggled off.
        let mut h = take(Sim::new(CardKind::Empty, 1, 32));
        h.registers()
            .write(registers::CTRL, registers::CTRL_FIFO_RESET);
        h.registers().writes.clear();
        h.reset().unwrap();
        assert_eq!(
            h.registers().writes[1],
            (registers::CTRL, registers::CTRL_ALL_RESET)
        );
    }

    #[test]
    fn a_reset_that_never_clears_is_reported_with_the_word_read() {
        let mut sim = Sim::new(CardKind::Empty, 1, 32);
        sim.reset_stuck = true;
        let mut h = take(sim);
        assert_eq!(h.reset(), Err(Error::ResetStuck(registers::CTRL_ALL_RESET)));
    }

    #[test]
    fn power_and_bus_width_land_in_their_registers() {
        let mut h = take(Sim::new(CardKind::Empty, 1, 32));
        h.power_on();
        assert_eq!(h.registers().peek(registers::PWREN), 1);
        for (width, ctype) in [
            (4, registers::CTYPE_4BIT),
            (8, registers::CTYPE_8BIT),
            (1, 0),
            (2, 0),
        ] {
            h.set_bus_width(width);
            assert_eq!(h.registers().peek(registers::CTYPE), ctype, "width {width}");
        }
    }

    #[test]
    fn a_clock_update_the_controller_never_takes_is_an_error() {
        let mut sim = Sim::new(CardKind::Empty, 1, 32);
        sim.clock_stuck = true;
        let mut h = take(sim);
        assert_eq!(h.set_clock(400_000), Err(Error::ClockUpdate));
    }

    /// A clock that advances seven microseconds a read never lands on the deadline exactly, as a
    /// real one need not, so the give-up test has to be a comparison past it.
    #[test]
    fn a_card_busy_forever_is_reported_once_the_deadline_has_passed() {
        let mut sim = Sim::new(CardKind::Empty, 1, 32);
        sim.busy_forever = true;
        sim.clock_step = 7;
        assert_ne!(DATA_DEADLINE_US % 7, 0);
        let mut h = take(sim);
        assert_eq!(
            within_ten_seconds(move || h.wait_not_busy()),
            Err(Error::Busy)
        );
    }

    /// On silicon a command takes time on the wire, so `CD` and the response words land some polls
    /// after the write to `CMD`. The driver must wait for `CD` before it reads either.
    #[test]
    fn the_response_is_read_only_once_the_command_is_done() {
        let mut sim = Sim::new(CardKind::SdHighCapacity, 2048, 32);
        sim.late_answer = 3;
        let mut h = take(sim);
        h.command(&Command::new(0, 0, Response::None)).unwrap();
        let echo = h
            .command(&Command::new(SEND_IF_COND, 0x1aa, Response::R7))
            .unwrap();
        assert_eq!(echo[0] & 0xfff, 0x1aa);
        let mut sim = Sim::new(CardKind::Empty, 1, 32);
        sim.late_answer = 3;
        let mut h = take(sim);
        assert_eq!(
            h.command(&Command::new(SEND_IF_COND, 0x1aa, Response::R7)),
            Err(Error::NoResponse(SEND_IF_COND))
        );
    }

    #[test]
    fn a_response_crc_error_is_a_bad_response() {
        let mut sim = Sim::new(CardKind::SdHighCapacity, 2048, 32);
        sim.faults = vec![(SEND_IF_COND, 0, Fault::BadCrc)];
        let mut h = take(sim);
        h.command(&Command::new(0, 0, Response::None)).unwrap();
        assert!(matches!(
            h.command(&Command::new(SEND_IF_COND, 0x1aa, Response::R7)),
            Err(Error::BadResponse(SEND_IF_COND, s)) if s & registers::RINTSTS_RCRC != 0
        ));
    }

    /// Before a data command the FIFO is reset and the reset waited for, and the card read
    /// threshold is left off: a 512-byte one in radon's 32-word FIFO is what overran its first
    /// read. Bit 8 of `CTRL`, left by firmware, makes a wait that tested anything but the reset
    /// bits never finish.
    #[test]
    fn a_data_command_resets_the_fifo_and_leaves_the_threshold_off() {
        let mut h = take(Sim::new(CardKind::SdHighCapacity, 2048, 32));
        h.registers().write(registers::CTRL, 1 << 8);
        let card = sd::identify(&mut h, 4).unwrap();
        h.registers().writes.clear();
        let mut block = [0u8; 512];
        sd::read_blocks(&mut h, &card, 1, &mut block).unwrap();
        let read = core::mem::take(&mut h.registers().writes);
        assert!(!read.iter().any(|&(o, _)| o == registers::CARDTHRCTL));
        assert!(read.contains(&(registers::CTRL, 1 << 8 | registers::CTRL_FIFO_RESET)));
        sd::write_blocks(&mut h, &card, 1, &block).unwrap();
        let write = &h.registers().writes;
        assert!(!write.iter().any(|&(o, _)| o == registers::CARDTHRCTL));
        assert!(write.contains(&(registers::CTRL, 1 << 8 | registers::CTRL_FIFO_RESET)));
        // A FIFO reset that never clears stops the command before it is sent.
        h.registers().reset_stuck = true;
        assert!(matches!(
            sd::read_blocks(&mut h, &card, 1, &mut block),
            Err(IoError::Host(Error::ResetStuck(_)))
        ));
    }

    /// The size and direction checks come before anything is sent, so an empty slot (which would
    /// answer `NoResponse` to anything sent) shows that nothing was.
    #[test]
    fn a_buffer_or_direction_that_does_not_match_the_command_is_refused_unsent() {
        let mut h = take(Sim::new(CardKind::Empty, 1, 32));
        let one = |write| Data {
            block_size: 512,
            blocks: 1,
            write,
        };
        let read = Command::with_data(17, 0, Response::R1, one(false));
        let write = Command::with_data(WRITE_BLOCK, 0, Response::R1, one(true));
        let mut buffer = [0u8; 1024];
        assert_eq!(h.read(&write, &mut buffer[..512]), Err(Error::BufferSize));
        assert_eq!(h.read(&read, &mut buffer), Err(Error::BufferSize));
        assert_eq!(h.write(&read, &buffer[..512]), Err(Error::BufferSize));
        assert_eq!(h.write(&write, &buffer), Err(Error::BufferSize));
        assert!(h.into_registers().commands.is_empty());
    }

    /// A card that stops moving data is a timeout naming exactly the bytes still owed: the first
    /// 128 moved (a 32-word FIFO's worth), and for a write a second 128 sits in the full FIFO.
    #[test]
    fn a_stalled_transfer_times_out_owing_what_did_not_move() {
        let mut h = take(Sim::new(CardKind::SdHighCapacity, 2048, 32));
        let card = sd::identify(&mut h, 4).unwrap();
        h.registers().stall_after = Some(128);
        let mut block = [0u8; 512];
        assert!(matches!(
            sd::read_blocks(&mut h, &card, 1, &mut block),
            Err(IoError::Host(Error::DataTimeout(17, _, 384)))
        ));
        let mut h = take(Sim::new(CardKind::SdHighCapacity, 2048, 32));
        let card = sd::identify(&mut h, 4).unwrap();
        h.registers().stall_after = Some(128);
        assert!(matches!(
            sd::write_blocks(&mut h, &card, 1, &block),
            Err(IoError::Host(Error::DataTimeout(24, _, 256)))
        ));
    }

    /// A card that raises `DTO` having sent only part of a read leaves nothing to move and nothing
    /// coming: the driver must wait out the deadline, not spin on an end that has no data.
    #[test]
    fn a_read_the_card_ends_early_times_out_rather_than_spinning() {
        let mut h = take(Sim::new(CardKind::SdHighCapacity, 2048, 32));
        let card = sd::identify(&mut h, 4).unwrap();
        h.registers().stall_after = Some(128);
        h.registers().end_early = true;
        let r = within_ten_seconds(move || {
            let mut block = [0u8; 512];
            sd::read_blocks(&mut h, &card, 1, &mut block)
        });
        assert!(matches!(
            r,
            Err(IoError::Host(Error::DataTimeout(17, _, 384)))
        ));
    }

    /// A 48-word FIFO does not divide a 512-byte block, so the last step of a transfer is shorter
    /// than the FIFO: each step must move what is owed, not what fits. The read half is with a
    /// `STATUS` that says the FIFO is full whatever it holds, which `queue_step` exists to survive.
    #[test]
    fn the_last_fifo_step_moves_only_what_is_owed() {
        let mut h = take(Sim::new(CardKind::SdHighCapacity, 2048, 48));
        let card = sd::identify(&mut h, 4).unwrap();
        let data: Vec<u8> = (0..512).map(|i| (i % 253) as u8).collect();
        sd::write_blocks(&mut h, &card, 5, &data).unwrap();
        h.registers().overreport_fill = true;
        let mut back = [0u8; 512];
        sd::read_blocks(&mut h, &card, 5, &mut back).unwrap();
        assert_eq!(back[..], data[..]);
    }

    /// An error that ends a write's data phase is reported, and at once: not after the deadline a
    /// transfer that never ends would wait out.
    #[test]
    fn a_data_error_at_the_end_of_a_write_is_reported_promptly() {
        let mut h = take(Sim::new(CardKind::SdHighCapacity, 2048, 32));
        let card = sd::identify(&mut h, 4).unwrap();
        h.registers().crc_error_at_end = true;
        let start = h.registers().now_us();
        assert!(matches!(
            sd::write_blocks(&mut h, &card, 1, &[0u8; 512]),
            Err(IoError::Host(Error::Data(24, rint))) if rint & registers::RINTSTS_DCRC != 0
        ));
        assert!(h.registers().now_us() - start < DATA_DEADLINE_US);
    }

    /// The same for a read whose data phase fails its CRC: the wait for the FIFO ends on the error.
    #[test]
    fn a_data_error_during_a_read_ends_the_wait_at_once() {
        let mut h = take(Sim::new(CardKind::SdHighCapacity, 2048, 32));
        let card = sd::identify(&mut h, 4).unwrap();
        h.registers().fail_next_read_crc = true;
        let start = h.registers().now_us();
        let mut block = [0u8; 512];
        assert!(matches!(
            sd::read_blocks(&mut h, &card, 1, &mut block),
            Err(IoError::Host(Error::Data(17, _)))
        ));
        assert!(h.registers().now_us() - start < DATA_DEADLINE_US);
    }

    /// A multi-block transfer is over when the automatic CMD12 is, which can land after `DTO`.
    #[test]
    fn a_multi_block_transfer_waits_for_the_automatic_stop() {
        let mut h = take(Sim::new(CardKind::SdHighCapacity, 2048, 32));
        let card = sd::identify(&mut h, 4).unwrap();
        h.registers().late_acd = 5;
        let mut blocks = [0u8; 4 * 512];
        sd::read_blocks(&mut h, &card, 1, &mut blocks).unwrap();
        assert_ne!(
            h.registers().peek(registers::RINTSTS) & registers::RINTSTS_ACD,
            0
        );
    }
}
