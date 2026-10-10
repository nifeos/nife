//! A minimal polled DesignWare I2C master, built for one transaction class, for milestone 592
//! (radon's cold reboot dies in OpenSBI's PMIC write): write bytes to a 7-bit address, optionally
//! read bytes back, one transfer.
//!
//! It exists because the 2026-10-10 radon bench put the board's reset in the outcome table's
//! fourth row: the PMIC's bus provably up, and OpenSBI's own driver still failing to read the PMIC
//! ten times, most plausibly on the controller's reset-default timing, which that firmware never
//! programs. Option B is this kernel programming the timing itself (out of
//! `jh7110_clock_and_reset::standard_mode_100k`) and writing the AXP15060's reset bit directly, so
//! the reset stops depending on OpenSBI's marginal read at all.
//!
//! Every register offset and bit is the vendor U-Boot `drivers/i2c/designware_i2c.{c,h}` of the
//! tree 592 pinned (`starfive-tech/u-boot` `1539c1fb5a49`, fetched 2026-10-10), the driver radon's
//! own firmware was built from. The sequence mirrors its `__dw_i2c_init`, `i2c_xfer_init`,
//! `__dw_i2c_write` and `i2c_xfer_finish`: disable, program, set the target address, enable, wait
//! the bus free, push bytes (the last with STOP), wait STOP_DET, disable.
//!
//! Bounded everywhere, for the drivers' own reason: this runs on the way to a reset, where
//! proceeding to the next route beats hanging on silicon that is not answering. Nothing here takes
//! an interrupt, a lock or a global; the caller passes the controller's base address, per the
//! driver rule.
//!
//! Name: provisional (milestone 592, 2026-10-10): calef names public items.

/// `IC_CON`, the control word.
const CON: usize = 0x00;
/// `IC_TAR`, the target (slave) address.
const TAR: usize = 0x04;
/// `IC_DATA_CMD`: a byte to write, or `READ_CMD` to ask for one.
const DATA_CMD: usize = 0x10;
/// `IC_SS_SCL_HCNT`, standard-mode high count.
const SS_SCL_HCNT: usize = 0x14;
/// `IC_SS_SCL_LCNT`, standard-mode low count.
const SS_SCL_LCNT: usize = 0x18;
/// `IC_RAW_INTR_STAT`: the unmasked status, readable without enabling any interrupt.
const RAW_INTR_STAT: usize = 0x34;
/// `IC_RX_TL`, the receive watermark, set to zero so one byte triggers.
const RX_TL: usize = 0x38;
/// `IC_TX_TL`, the transmit watermark, set to zero so the empty FIFO triggers.
const TX_TL: usize = 0x3c;
/// `IC_CLR_TX_ABRT`: reading clears a transmit abort.
const CLR_TX_ABRT: usize = 0x54;
/// `IC_CLR_STOP_DET`: reading clears the stop-detected bit.
const CLR_STOP_DET: usize = 0x60;
/// `IC_ENABLE`.
const ENABLE: usize = 0x6c;
/// `IC_STATUS`.
const STATUS: usize = 0x70;
/// `IC_SDA_HOLD`.
const SDA_HOLD: usize = 0x7c;
/// `IC_TX_ABRT_SOURCE`, why the last transmit aborted, one bit per cause.
const TX_ABRT_SOURCE: usize = 0x80;
/// `IC_ENABLE_STATUS`, whose bit 0 says whether the controller took the enable word.
const ENABLE_STATUS: usize = 0x9c;
/// `IC_COMP_TYPE`: the IP's own identity, `0x4457_0140` ("DW" plus a number) in every build of it.
const COMP_TYPE: usize = 0xfc;

/// The `IC_DATA_CMD` read bit (`IC_CMD`).
const READ_CMD: u32 = 1 << 8;
/// The `IC_DATA_CMD` stop bit (`IC_STOP`).
const STOP_CMD: u32 = 1 << 9;
/// `IC_ENABLE`'s enable bit (`IC_ENABLE_0B`).
const ENABLE_0B: u32 = 1;
/// `IC_STATUS`'s master-activity bit (`IC_STATUS_MA`).
const STATUS_MA: u32 = 1 << 5;
/// `IC_STATUS`'s receive-not-empty bit (`IC_STATUS_RFNE`).
const STATUS_RFNE: u32 = 1 << 3;
/// `IC_STATUS`'s transmit-empty bit (`IC_STATUS_TFE`).
const STATUS_TFE: u32 = 1 << 2;
/// `IC_STATUS`'s transmit-not-full bit (`IC_STATUS_TFNF`).
const STATUS_TFNF: u32 = 1 << 1;
/// `IC_RAW_INTR_STAT`'s stop-detected bit (`IC_STOP_DET`).
const INTR_STOP_DET: u32 = 1 << 9;
/// `IC_RAW_INTR_STAT`'s transmit-abort bit (`IC_TX_ABRT`).
const INTR_TX_ABRT: u32 = 1 << 6;

/// The value a DesignWare I2C IP answers in `IC_COMP_TYPE` (the vendor header's `DW_I2C_COMP_TYPE`).
pub const DW_I2C_COMP_TYPE: u32 = 0x4457_0140;

/// Every wait's bound, the size of the console drivers' own bounds: far more polls than a 100 kHz
/// transaction of a few bytes needs, and small enough to reach the fallback route promptly.
const BOUND: u32 = 1_000_000;

/// How the transaction ended, when it did not end well. Every field is a register word, because
/// the reset path's answer is a console line naming the state, not a typed recovery: the caller
/// prints this and falls through to the firmware route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// `IC_COMP_TYPE` did not read as a DesignWare I2C IP; the window is not this controller.
    NotADesignWare(u32),
    /// The controller never took an enable or disable word.
    EnableStuck(u32),
    /// The bus never looked free (`IC_STATUS` last read in the field).
    BusBusy(u32),
    /// A transmit FIFO slot never opened (`IC_STATUS` last read).
    TransmitterStuck(u32),
    /// A read byte never arrived (`IC_STATUS` last read).
    ReceiverStuck(u32),
    /// The STOP condition's interrupt status never arrived (`IC_RAW_INTR_STAT` last read).
    StopNeverDetected(u32),
    /// The controller aborted the transfer; the source word names the cause (bit per the IP's
    /// `TX_ABRT_SOURCE`: bit 7 is a target NACK on the address, bit 9 a NACK on data).
    TransmitAborted { source: u32, raw: u32 },
}

/// One DesignWare I2C controller, at a base address the caller owns.
#[derive(Debug)]
pub struct DesignWareI2c {
    base: usize,
}

impl DesignWareI2c {
    /// Wrap the controller at `base`. The caller vouches the window is mapped device memory; this
    /// driver never writes at construction.
    #[must_use]
    pub const fn new(base: usize) -> Self {
        Self { base }
    }

    fn rd(&self, reg: usize) -> u32 {
        // SAFETY: the caller vouched for the window at construction; the register map is 32-bit.
        unsafe { core::ptr::read_volatile((self.base + reg) as *const u32) }
    }

    fn wr(&self, reg: usize, value: u32) {
        // SAFETY: as `rd`; every write here is to a register this sequence owns for the duration.
        unsafe { core::ptr::write_volatile((self.base + reg) as *mut u32, value) }
    }

    /// `IC_COMP_TYPE`, so the first thing the reset path does with a wrong window is say so.
    #[must_use]
    pub fn component_type(&self) -> u32 {
        self.rd(COMP_TYPE)
    }

    fn set_enable(&self, on: bool) -> Result<(), Failure> {
        let want = if on { ENABLE_0B } else { 0 };
        self.wr(ENABLE, want);
        let mut spins = BOUND;
        while spins > 0 {
            if self.rd(ENABLE_STATUS) & ENABLE_0B == want {
                return Ok(());
            }
            core::hint::spin_loop();
            spins -= 1;
        }
        Err(Failure::EnableStuck(self.rd(ENABLE_STATUS)))
    }

    fn wait_bus_free(&self) -> Result<(), Failure> {
        let mut spins = BOUND;
        while spins > 0 {
            let status = self.rd(STATUS);
            if status & STATUS_MA == 0 && status & STATUS_TFE != 0 {
                return Ok(());
            }
            core::hint::spin_loop();
            spins -= 1;
        }
        Err(Failure::BusBusy(self.rd(STATUS)))
    }

    /// **Write `write` then read `read` back, one transaction**, standard mode at the timing
    /// `mode` gives. The vendor's own two halves, `__dw_i2c_write`'s address-then-data and
    /// `__dw_i2c_read`'s read with STOP on the last byte, in one function because this driver's
    /// one caller wants exactly that shape: send a register address, get or set one byte.
    ///
    /// The reads are issued by pushing `READ_CMD` into the transmit FIFO, which is how the IP
    /// works: reads are commands the master transmits. The STOP goes on the last command of all,
    /// whichever half it ends in.
    pub fn write_read(
        &self,
        mode: &jh7110_clock_and_reset::StandardMode,
        target: u8,
        write: &[u8],
        read: &mut [u8],
    ) -> Result<(), Failure> {
        if self.component_type() != DW_I2C_COMP_TYPE {
            return Err(Failure::NotADesignWare(self.component_type()));
        }
        self.set_enable(false)?;
        self.wr(CON, mode.con);
        self.wr(RX_TL, 0);
        self.wr(TX_TL, 0);
        self.wr(SS_SCL_HCNT, mode.hcnt);
        self.wr(SS_SCL_LCNT, mode.lcnt);
        self.wr(SDA_HOLD, mode.sda_hold);
        self.wr(TAR, u32::from(target & 0x7f));
        self.set_enable(true)?;
        self.wait_bus_free()?;

        let last_write = write.len().saturating_sub(1);
        for (n, byte) in write.iter().enumerate() {
            let mut spins = BOUND;
            while self.rd(STATUS) & STATUS_TFNF == 0 {
                if spins == 0 {
                    return Err(Failure::TransmitterStuck(self.rd(STATUS)));
                }
                core::hint::spin_loop();
                spins -= 1;
            }
            let stop = if read.is_empty() && n == last_write {
                STOP_CMD
            } else {
                0
            };
            self.wr(DATA_CMD, u32::from(*byte) | stop);
        }
        for n in 0..read.len() {
            let mut spins = BOUND;
            while self.rd(STATUS) & STATUS_TFNF == 0 {
                if spins == 0 {
                    return Err(Failure::TransmitterStuck(self.rd(STATUS)));
                }
                core::hint::spin_loop();
                spins -= 1;
            }
            let stop = if n + 1 == read.len() { STOP_CMD } else { 0 };
            self.wr(DATA_CMD, READ_CMD | stop);

            let mut spins = BOUND;
            while self.rd(STATUS) & STATUS_RFNE == 0 {
                if spins == 0 {
                    return Err(Failure::ReceiverStuck(self.rd(STATUS)));
                }
                core::hint::spin_loop();
                spins -= 1;
            }
            read[n] = self.rd(DATA_CMD) as u8;
        }

        let mut spins = BOUND;
        loop {
            let raw = self.rd(RAW_INTR_STAT);
            if raw & INTR_TX_ABRT != 0 {
                let source = self.rd(TX_ABRT_SOURCE);
                let _ = self.rd(CLR_TX_ABRT);
                let _ = self.set_enable(false);
                return Err(Failure::TransmitAborted { source, raw });
            }
            if raw & INTR_STOP_DET != 0 {
                let _ = self.rd(CLR_STOP_DET);
                break;
            }
            if spins == 0 {
                let _ = self.set_enable(false);
                return Err(Failure::StopNeverDetected(raw));
            }
            core::hint::spin_loop();
            spins -= 1;
        }
        self.set_enable(false)
    }
}
