//! **A polled Synopsys DesignWare I2C master** for one transaction class: write bytes to a 7-bit
//! target, optionally read bytes back, one transfer, every wait bounded.
//!
//! The register sequence lives here and the register file does not: the caller supplies a
//! [`Registers`], which a userspace driver implements over its mapped controller page and a test
//! implements over a simulated controller. Every register offset and bit is vendor U-Boot's
//! `drivers/i2c/designware_i2c.{c,h}` at `starfive-tech/u-boot` `1539c1fb5a49`, and the sequence
//! follows its `__dw_i2c_init`, `i2c_xfer_init`, `__dw_i2c_write`, `__dw_i2c_read` and
//! `i2c_xfer_finish`. It takes no interrupt, lock or global. See notes/board-reboot.md.
//!
//! # BUGS
//!
//! - Standard mode only, from a [`Timing`] the caller computed; there is no fast mode.
//! - Every bound is a count of register reads, not a duration.
//! - One controller has run it: the JH7110's I2C5, on radon, as the kernel's own copy of this
//!   sequence before this crate held it.
//!
//! Name: provisional. The `DesignWare` IP's vendor name and the bus spelled out, as
//! `designware_mobile_storage` spells its bus.

#![cfg_attr(not(test), no_std)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

#[cfg(test)]
mod tests;

/// `IC_CON`, the control word.
///
/// Name: provisional.
#[doc(alias = "IC_CON")]
pub const CONTROL: u32 = 0x00;
/// `IC_TAR`, the target address.
///
/// Name: provisional.
#[doc(alias = "IC_TAR")]
pub const TARGET_ADDRESS: u32 = 0x04;
/// `IC_DATA_CMD`: a byte to write, or [`READ_COMMAND`] to ask for one.
///
/// Name: provisional.
#[doc(alias = "IC_DATA_CMD")]
pub const DATA_COMMAND: u32 = 0x10;
/// `IC_SS_SCL_HCNT`, the standard-speed clock's high count.
///
/// Name: provisional.
#[doc(alias = "IC_SS_SCL_HCNT")]
pub const STANDARD_SPEED_CLOCK_HIGH_COUNT: u32 = 0x14;
/// `IC_SS_SCL_LCNT`, the standard-speed clock's low count.
///
/// Name: provisional.
#[doc(alias = "IC_SS_SCL_LCNT")]
pub const STANDARD_SPEED_CLOCK_LOW_COUNT: u32 = 0x18;
/// `IC_RAW_INTR_STAT`: the unmasked interrupt status, readable with every interrupt disabled.
///
/// Name: provisional.
#[doc(alias = "IC_RAW_INTR_STAT")]
pub const RAW_INTERRUPT_STATUS: u32 = 0x34;
/// `IC_RX_TL`, the receive watermark, set to zero so one byte triggers.
///
/// Name: provisional.
#[doc(alias = "IC_RX_TL")]
pub const RECEIVE_THRESHOLD: u32 = 0x38;
/// `IC_TX_TL`, the transmit watermark, set to zero so the empty queue triggers.
///
/// Name: provisional.
#[doc(alias = "IC_TX_TL")]
pub const TRANSMIT_THRESHOLD: u32 = 0x3c;
/// `IC_CLR_TX_ABRT`: a read clears a transmit abort.
///
/// Name: provisional.
#[doc(alias = "IC_CLR_TX_ABRT")]
pub const CLEAR_TRANSMIT_ABORT: u32 = 0x54;
/// `IC_CLR_STOP_DET`: a read clears the stop-detected bit.
///
/// Name: provisional.
#[doc(alias = "IC_CLR_STOP_DET")]
pub const CLEAR_STOP_DETECTED: u32 = 0x60;
/// `IC_ENABLE`.
///
/// Name: provisional.
#[doc(alias = "IC_ENABLE")]
pub const ENABLE: u32 = 0x6c;
/// `IC_STATUS`.
///
/// Name: provisional.
#[doc(alias = "IC_STATUS")]
pub const STATUS: u32 = 0x70;
/// `IC_SDA_HOLD`, the data line's hold time in input-clock counts.
///
/// Name: provisional.
#[doc(alias = "IC_SDA_HOLD")]
pub const DATA_HOLD: u32 = 0x7c;
/// `IC_TX_ABRT_SOURCE`: why the last transmit aborted, one bit per cause.
///
/// Name: provisional.
#[doc(alias = "IC_TX_ABRT_SOURCE")]
pub const TRANSMIT_ABORT_SOURCE: u32 = 0x80;
/// `IC_ENABLE_STATUS`, whose bit 0 says whether the controller took the enable word.
///
/// Name: provisional.
#[doc(alias = "IC_ENABLE_STATUS")]
pub const ENABLE_STATUS: u32 = 0x9c;
/// `IC_COMP_TYPE`: the IP's own identity, [`COMPONENT_TYPE_DESIGNWARE`] in every build of it.
///
/// Name: provisional.
#[doc(alias = "IC_COMP_TYPE")]
pub const COMPONENT_TYPE: u32 = 0xfc;

/// What every `DesignWare` I2C IP answers in [`COMPONENT_TYPE`] (`DW_I2C_COMP_TYPE`, "DW" and a
/// number).
///
/// Name: provisional.
#[doc(alias = "DW_I2C_COMP_TYPE")]
pub const COMPONENT_TYPE_DESIGNWARE: u32 = 0x4457_0140;

/// [`DATA_COMMAND`]'s read bit (`IC_CMD`).
///
/// Name: provisional.
#[doc(alias = "IC_CMD")]
pub const READ_COMMAND: u32 = 1 << 8;
/// [`DATA_COMMAND`]'s stop bit (`IC_STOP`).
///
/// Name: provisional.
#[doc(alias = "IC_STOP")]
pub const STOP_COMMAND: u32 = 1 << 9;
/// [`ENABLE`]'s enable bit (`IC_ENABLE_0B`).
const ENABLE_BIT: u32 = 1;
/// [`STATUS`]'s master-activity bit (`IC_STATUS_MA`).
const STATUS_MASTER_ACTIVITY: u32 = 1 << 5;
/// [`STATUS`]'s receive-not-empty bit (`IC_STATUS_RFNE`).
const STATUS_RECEIVE_NOT_EMPTY: u32 = 1 << 3;
/// [`STATUS`]'s transmit-empty bit (`IC_STATUS_TFE`).
const STATUS_TRANSMIT_EMPTY: u32 = 1 << 2;
/// [`STATUS`]'s transmit-not-full bit (`IC_STATUS_TFNF`).
const STATUS_TRANSMIT_NOT_FULL: u32 = 1 << 1;
/// [`RAW_INTERRUPT_STATUS`]'s stop-detected bit (`IC_STOP_DET`).
const INTERRUPT_STOP_DETECTED: u32 = 1 << 9;
/// [`RAW_INTERRUPT_STATUS`]'s transmit-abort bit (`IC_TX_ABRT`).
const INTERRUPT_TRANSMIT_ABORT: u32 = 1 << 6;

/// The controller's register file, 32-bit words at byte offsets from its base.
///
/// A driver implements it over its mapped controller page with volatile accesses; a test
/// implements it over a simulated controller.
///
/// Name: provisional.
pub trait Registers {
    /// Read the word at `offset`.
    fn read(&mut self, offset: u32) -> u32;
    /// Write `value` to the word at `offset`.
    fn write(&mut self, offset: u32, value: u32);
}

/// The standard-mode timing a caller computed from the controller's input clock: the four words
/// the sequence writes before it enables the controller.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Timing {
    /// The [`CONTROL`] word: master mode, restart enable, target mode disabled, standard speed.
    pub control: u32,
    /// [`STANDARD_SPEED_CLOCK_HIGH_COUNT`].
    pub high_count: u32,
    /// [`STANDARD_SPEED_CLOCK_LOW_COUNT`].
    pub low_count: u32,
    /// [`DATA_HOLD`].
    pub data_hold: u32,
}

/// How a transfer ended, when it did not end well. Every field is a register word, because the
/// caller's answer is to report the state and fall back, not to recover.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// [`COMPONENT_TYPE`] did not read as a `DesignWare` I2C IP: the page is not this controller.
    NotADesignWare(u32),
    /// The controller never took an enable or disable word ([`ENABLE_STATUS`] last read).
    EnableStuck(u32),
    /// The bus never looked free ([`STATUS`] last read).
    BusBusy(u32),
    /// A transmit queue slot never opened ([`STATUS`] last read).
    TransmitterStuck(u32),
    /// A read byte never arrived ([`STATUS`] last read).
    ReceiverStuck(u32),
    /// The stop condition never arrived ([`RAW_INTERRUPT_STATUS`] last read).
    StopNeverDetected(u32),
    /// The controller aborted the transfer. Bit 7 of `source` is a target that did not answer its
    /// address, bit 9 one that refused a data byte.
    TransmitAborted {
        /// [`TRANSMIT_ABORT_SOURCE`].
        source: u32,
        /// [`RAW_INTERRUPT_STATUS`].
        raw: u32,
    },
}

impl Failure {
    /// **A small number naming the failure's kind**, 1 to 7 in declaration order, for a reply word
    /// that carries it across a process boundary.
    ///
    /// Name: provisional.
    #[must_use]
    pub const fn kind(&self) -> u8 {
        match self {
            Failure::NotADesignWare(_) => 1,
            Failure::EnableStuck(_) => 2,
            Failure::BusBusy(_) => 3,
            Failure::TransmitterStuck(_) => 4,
            Failure::ReceiverStuck(_) => 5,
            Failure::StopNeverDetected(_) => 6,
            Failure::TransmitAborted { .. } => 7,
        }
    }

    /// **The register word the failure last read**: the abort source for an aborted transfer.
    ///
    /// Name: provisional.
    #[must_use]
    pub const fn word(&self) -> u32 {
        match *self {
            Failure::NotADesignWare(w)
            | Failure::EnableStuck(w)
            | Failure::BusBusy(w)
            | Failure::TransmitterStuck(w)
            | Failure::ReceiverStuck(w)
            | Failure::StopNeverDetected(w) => w,
            Failure::TransmitAborted { source, .. } => source,
        }
    }
}

/// **One `DesignWare` I2C controller**, over registers the caller owns, with every wait bounded by
/// `bound` register reads.
///
/// Name: provisional.
#[derive(Debug)]
pub struct Controller<R: Registers> {
    registers: R,
    bound: u32,
}

impl<R: Registers> Controller<R> {
    /// Wrap `registers`. Writes nothing. `bound` is how many reads each wait makes before it gives
    /// up; a million is far more than a 100 kHz transfer of a few bytes needs.
    ///
    /// Name: provisional.
    pub const fn new(registers: R, bound: u32) -> Self {
        Self { registers, bound }
    }

    /// The register file, back, for a test that inspects what was written.
    ///
    /// Name: provisional.
    pub fn into_registers(self) -> R {
        self.registers
    }

    /// [`COMPONENT_TYPE`], so a wrong page says so before anything is written.
    ///
    /// Name: provisional.
    pub fn component_type(&mut self) -> u32 {
        self.registers.read(COMPONENT_TYPE)
    }

    /// Wait, at most `bound` reads, until `done` holds of the word at `offset`; the last word read
    /// on failure.
    fn wait(&mut self, offset: u32, done: impl Fn(u32) -> bool) -> Result<(), u32> {
        let mut last = 0;
        for _ in 0..self.bound {
            last = self.registers.read(offset);
            if done(last) {
                return Ok(());
            }
            core::hint::spin_loop();
        }
        Err(last)
    }

    fn set_enable(&mut self, on: bool) -> Result<(), Failure> {
        let want = if on { ENABLE_BIT } else { 0 };
        self.registers.write(ENABLE, want);
        self.wait(ENABLE_STATUS, |w| w & ENABLE_BIT == want)
            .map_err(Failure::EnableStuck)
    }

    fn push(&mut self, command: u32) -> Result<(), Failure> {
        self.wait(STATUS, |w| w & STATUS_TRANSMIT_NOT_FULL != 0)
            .map_err(Failure::TransmitterStuck)?;
        self.registers.write(DATA_COMMAND, command);
        Ok(())
    }

    /// **Write `write`, then read `read` back, from the 7-bit `target`**, one transfer at
    /// `timing`; the [`Failure`] if it did not end well.
    ///
    /// The steps: check the identity, disable, program the timing and the target, enable, wait
    /// for a free bus, queue the bytes and then one [`READ_COMMAND`] per byte to read, with
    /// [`STOP_COMMAND`] on the last command of all, wait for the stop, disable.
    ///
    /// Name: provisional.
    pub fn write_read(
        &mut self,
        timing: &Timing,
        target: u8,
        write: &[u8],
        read: &mut [u8],
    ) -> Result<(), Failure> {
        let identity = self.component_type();
        if identity != COMPONENT_TYPE_DESIGNWARE {
            return Err(Failure::NotADesignWare(identity));
        }
        self.set_enable(false)?;
        self.registers.write(CONTROL, timing.control);
        self.registers.write(RECEIVE_THRESHOLD, 0);
        self.registers.write(TRANSMIT_THRESHOLD, 0);
        self.registers
            .write(STANDARD_SPEED_CLOCK_HIGH_COUNT, timing.high_count);
        self.registers
            .write(STANDARD_SPEED_CLOCK_LOW_COUNT, timing.low_count);
        self.registers.write(DATA_HOLD, timing.data_hold);
        self.registers
            .write(TARGET_ADDRESS, u32::from(target & 0x7f));
        self.set_enable(true)?;
        self.wait(STATUS, |w| {
            w & STATUS_MASTER_ACTIVITY == 0 && w & STATUS_TRANSMIT_EMPTY != 0
        })
        .map_err(Failure::BusBusy)?;

        let last_write = write.len().saturating_sub(1);
        for (n, byte) in write.iter().enumerate() {
            let stop = if read.is_empty() && n == last_write {
                STOP_COMMAND
            } else {
                0
            };
            self.push(u32::from(*byte) | stop)?;
        }
        let reads = read.len();
        for (n, byte) in read.iter_mut().enumerate() {
            let stop = if n + 1 == reads { STOP_COMMAND } else { 0 };
            self.push(READ_COMMAND | stop)?;
            self.wait(STATUS, |w| w & STATUS_RECEIVE_NOT_EMPTY != 0)
                .map_err(Failure::ReceiverStuck)?;
            *byte = self.registers.read(DATA_COMMAND) as u8;
        }

        let mut raw = 0;
        for _ in 0..self.bound {
            raw = self.registers.read(RAW_INTERRUPT_STATUS);
            if raw & INTERRUPT_TRANSMIT_ABORT != 0 {
                let source = self.registers.read(TRANSMIT_ABORT_SOURCE);
                let _ = self.registers.read(CLEAR_TRANSMIT_ABORT);
                let _ = self.set_enable(false);
                return Err(Failure::TransmitAborted { source, raw });
            }
            if raw & INTERRUPT_STOP_DETECTED != 0 {
                let _ = self.registers.read(CLEAR_STOP_DETECTED);
                return self.set_enable(false);
            }
            core::hint::spin_loop();
        }
        let _ = self.set_enable(false);
        Err(Failure::StopNeverDetected(raw))
    }

    /// **Read the target's one-byte register `register`**, a write of its number then a read.
    ///
    /// Name: provisional.
    pub fn read_register(
        &mut self,
        timing: &Timing,
        target: u8,
        register: u8,
    ) -> Result<u8, Failure> {
        let mut value = [0u8; 1];
        self.write_read(timing, target, &[register], &mut value)?;
        Ok(value[0])
    }

    /// **Write `value` to the target's one-byte register `register`**, in one transfer.
    ///
    /// Name: provisional.
    pub fn write_register(
        &mut self,
        timing: &Timing,
        target: u8,
        register: u8,
        value: u8,
    ) -> Result<(), Failure> {
        self.write_read(timing, target, &[register, value], &mut [])
    }
}

/// **`value` with bit `bit` set and every other bit as found**, the write a read-modify-write of
/// one register makes.
///
/// Name: provisional.
#[must_use]
pub const fn with_bit(value: u8, bit: u8) -> u8 {
    value | (1 << (bit & 7))
}
