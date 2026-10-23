//! **The Intel TCO watchdog**, the one piece of an Intel PC that resets the machine when the kernel
//! stops talking to it (milestone 593 (a wedged kernel resets itself), provisional number).
//!
//! A second timeout with no reload in between asserts `PLTRST#`, a platform reset that needs
//! nothing from the CPU, so the reset lands `2 x TMR x 0.6` seconds after the last pet;
//! [`Tco::arm`] takes the time to the reset and does the halving. `SECOND_TO_STS` survives that
//! reset, so the next boot reports how the last one ended. [`find`] returns a [`Tco`] value, and
//! nothing here keeps a static. See notes/x86-port/tco-watchdog.md, which holds the register map,
//! its sources and the BUGS.

use core::fmt;

use paging::PAGE_SIZE;

use super::mmu;
use super::port::{in16, in32, out16, out32};

// --- The legacy PCI configuration mechanism, bus 0 only. -------------------------------------

/// The configuration-address and data ports (`machine.rs` and `reset.rs` use the same pair).
const CONFIG_ADDRESS: u16 = 0xcf8;
const CONFIG_DATA: u16 = 0xcfc;

/// Read a dword of bus 0's configuration space.
fn config_read(device: u8, function: u8, offset: u8) -> u32 {
    // SAFETY: the legacy configuration mechanism, present on every PC, addressing an aligned dword
    // of a bus-0 function. A read of an absent function returns all ones and has no side effect.
    unsafe {
        out32(CONFIG_ADDRESS, config_address(device, function, offset));
        in32(CONFIG_DATA)
    }
}

/// Write a dword of bus 0's configuration space.
///
/// # Safety
/// The caller must know what the register at `offset` does; a configuration write is a device
/// command.
unsafe fn config_write(device: u8, function: u8, offset: u8, value: u32) {
    // SAFETY: the caller's contract, over the same mechanism as `config_read`.
    unsafe {
        out32(CONFIG_ADDRESS, config_address(device, function, offset));
        out32(CONFIG_DATA, value);
    }
}

fn config_address(device: u8, function: u8, offset: u8) -> u32 {
    0x8000_0000 | u32::from(device) << 11 | u32::from(function) << 8 | u32::from(offset & 0xfc)
}

// --- Where the chipset keeps things. ---------------------------------------------------------

/// Device 31 carries the chipset's own functions on every Intel PC since the ICH.
const CHIPSET_DEVICE: u8 = 31;
const LPC_FUNCTION: u8 = 0;
const P2SB_FUNCTION: u8 = 1;
const SMBUS_FUNCTION: u8 = 4;

const INTEL: u32 = 0x8086;
/// The ICH9 LPC bridge QEMU's `q35` presents (`lpc_ich.c`: `LPC_ICH9`, TCO version 2).
const ICH9_LPC: u32 = 0x2918;
/// The `SMBus` controllers whose TCO `i2c-i801.c` registers as version 4 (`FEATURE_TCO_SPT`) and
/// that this kernel has a machine for or a sibling of: Kaby Lake PCH-H (xenon's Q270), Sunrise
/// Point H and LP.
const SPT_SMBUS: [u32; 3] = [0xa2a3, 0xa123, 0x9d23];

/// ICH9 LPC config: `PMBASE`, the ACPI power-management I/O base (bits 15:7).
const PMBASE: u8 = 0x40;
const PMBASE_MASK: u32 = 0xff80;
/// ICH9 LPC config: `ACPI_CNTL`, whose bit 7 turns the `PMBASE` decode on.
const ACPI_CNTL: u8 = 0x44;
const ACPI_EN: u32 = 1 << 7;
/// The TCO block's offset inside the ICH9's power-management I/O space.
const PMBASE_TCO: u16 = 0x60;
/// ICH9 LPC config: `RCBA`, the root complex register block (bits 31:14, enable in bit 0).
const RCBA: u8 = 0xf0;
const RCBA_MASK: u32 = 0xffff_c000;
const RCBA_ENABLE: u32 = 1;
/// `GCS`, General Control and Status, inside the RCBA block. Bit 5 is `NO_REBOOT`.
const RCBA_GCS: u64 = 0x3410;
const GCS_NO_REBOOT: u32 = 1 << 5;

/// PCH `SMBus` config: `TCOBASE` (bits 15:5) and `TCOCTL` (bit 8 enables the decode).
const TCOBASE: u8 = 0x50;
const TCOBASE_MASK: u32 = 0xffe0;
const TCOCTL: u8 = 0x54;
const TCO_BASE_EN: u32 = 1 << 8;
/// P2SB config: `SBREG_BAR` (low and high dwords) and `P2SBC`, whose bit 8 hides the device.
const SBREG_BAR: u8 = 0x10;
const SBREG_BARH: u8 = 0x14;
const P2SBC: u8 = 0xe0;
const P2SBC_HIDE: u32 = 1 << 8;
/// The `SMBus` `GC` register in private config space: port id `0xc6`, offset `0xc`. Bit 1 is `NR`.
const SMBUS_GC: u64 = 0xc6_000c;
const GC_NO_REBOOT: u32 = 1 << 1;

// --- The TCO block itself. -------------------------------------------------------------------

const TCO_RLD: u16 = 0x00;
const TCO1_STS: u16 = 0x04;
const TCO2_STS: u16 = 0x06;
const TCO1_CNT: u16 = 0x08;
const TCO_TMR: u16 = 0x12;
/// `TCO1_STS` bit 3: the first timeout happened. Write 1 to clear.
const TIMEOUT: u16 = 1 << 3;
/// `TCO2_STS` bit 1: the second timeout happened, and with `NO_REBOOT` clear the chipset reset.
const SECOND_TO_STS: u16 = 1 << 1;
/// `TCO2_STS` bit 2, version 2 only: set with `SECOND_TO_STS`. Linux clears it on version 2.
const BOOT_STS: u16 = 1 << 2;
/// `TCO1_CNT` bit 11: 1 halts the timer, 0 lets it count.
const TCO_TMR_HALT: u16 = 1 << 11;
/// `TCO_TMR` bits 9:0.
const TMR_MASK: u16 = 0x3ff;
/// "Values of 0000h or 0001h will be ignored" (datasheet); Linux refuses below 4.
const TMR_MIN: u16 = 4;

/// Tenths of a second per tick, so the arithmetic stays in integers.
const TENTHS_PER_TICK: u64 = 6;

/// Which of the two layouts above this chipset uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Generation {
    /// ICH9, TCO version 2: QEMU's `q35`.
    Ich9,
    /// 100/200-series PCH, TCO version 4: xenon.
    Pch,
}

/// Where `NO_REBOOT` lives, as a physical address and a bit.
#[derive(Debug, Clone, Copy)]
struct NoReboot {
    phys: u64,
    bit: u32,
}

/// **A TCO watchdog found on this machine**, not yet armed. Name provisional: calef names public
/// items.
#[derive(Debug)]
pub(crate) struct Tco {
    generation: Generation,
    base: u16,
    no_reboot: NoReboot,
}

/// Why [`find`] found nothing it would drive. Each says which register said no, because the first
/// reader of this is a bench log.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Absent {
    /// Neither `00:1f.0` nor `00:1f.4` is a chipset this module knows.
    UnknownChipset { lpc: u32, smbus: u32 },
    /// ICH9 with `PMBASE` unprogrammed: firmware never gave the ACPI registers an address.
    NoPmBase,
    /// ICH9 with `RCBA` disabled, so `NO_REBOOT` cannot be reached.
    RcbaDisabled,
    /// PCH with the TCO decode off in `TCOCTL`, or a zero `TCOBASE`.
    TcoDecodeOff { tcobase: u32, tcoctl: u32 },
    /// PCH whose P2SB answered a zero `SBREG_BAR`, so `NO_REBOOT` cannot be reached.
    NoSidebandBar,
    /// The page holding `NO_REBOOT` could not be mapped.
    Unmappable { phys: u64 },
}

impl fmt::Display for Absent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::UnknownChipset { lpc, smbus } => write!(
                f,
                "no known TCO chipset: 00:1f.0 reads {lpc:#010x}, 00:1f.4 reads {smbus:#010x} \
                 (vendor in the low half)"
            ),
            Self::NoPmBase => write!(f, "ICH9 found but PMBASE (00:1f.0 config 0x40) is zero"),
            Self::RcbaDisabled => {
                write!(f, "ICH9 found but RCBA (00:1f.0 config 0xf0) is disabled")
            }
            Self::TcoDecodeOff { tcobase, tcoctl } => write!(
                f,
                "PCH found but its TCO decode is off: TCOBASE {tcobase:#x}, TCOCTL {tcoctl:#x} \
                 (bit 8 enables)"
            ),
            Self::NoSidebandBar => write!(f, "PCH found but P2SB's SBREG_BAR reads zero"),
            Self::Unmappable { phys } => {
                write!(f, "the NO_REBOOT register at {phys:#x} could not be mapped")
            }
        }
    }
}

/// Why [`Tco::arm`] did not arm.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ArmError {
    /// The requested time to reset is outside what ten bits of 0.6-second ticks, counted twice,
    /// can express: about 4.8 s to 1,227 s.
    OutOfRange { seconds: u64 },
    /// `TCO_TMR` did not read back what was written.
    TimerRefused { wrote: u16, read: u16 },
    /// `NO_REBOOT` stayed set after being cleared: a strap or firmware holds it. The timer would
    /// count and never reset.
    NoRebootLocked { phys: u64, read: u32 },
    /// `TCO_TMR_HALT` stayed set after being cleared.
    WouldNotStart { cnt: u16 },
}

impl fmt::Display for ArmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::OutOfRange { seconds } => write!(
                f,
                "{seconds}s to reset is outside the TCO's range (about 5s to 1227s)"
            ),
            Self::TimerRefused { wrote, read } => {
                write!(f, "TCO_TMR refused {wrote:#x} and reads {read:#x}")
            }
            Self::NoRebootLocked { phys, read } => write!(
                f,
                "NO_REBOOT is still set after clearing it ({phys:#x} reads {read:#010x}): a strap or \
                 firmware holds it, so the timer would count and never reset"
            ),
            Self::WouldNotStart { cnt } => {
                write!(
                    f,
                    "TCO1_CNT still reads the halt bit after clearing it ({cnt:#x})"
                )
            }
        }
    }
}

/// What [`Tco::arm`] did, for the banner.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Armed {
    /// The reload value written to `TCO_TMR`, in 0.6-second ticks.
    pub ticks: u16,
    /// Tenths of a second from a pet to the reset: two full counts.
    pub reset_after_tenths: u64,
    /// `SECOND_TO_STS` as this boot found it: the previous boot was ended by this watchdog.
    pub previous_boot_reset_by_watchdog: bool,
}

/// **Look for a TCO watchdog on this machine**, through the chipset's configuration space.
///
/// Reads only, except in two places Linux also writes: it turns the ICH9's ACPI I/O decode on if
/// firmware left it off (`lpc_ich_enable_acpi_space`), and it unhides the PCH's P2SB bridge for
/// the two reads that find `SBREG_BAR`, then hides it again (`p2sb_bar`).
///
/// Name: provisional (milestone 593), 2026-09-25. calef names public items.
pub fn find() -> Result<Tco, Absent> {
    let lpc = config_read(CHIPSET_DEVICE, LPC_FUNCTION, 0);
    if lpc == ICH9_LPC << 16 | INTEL {
        return find_ich9();
    }
    let smbus = config_read(CHIPSET_DEVICE, SMBUS_FUNCTION, 0);
    if smbus & 0xffff == INTEL && SPT_SMBUS.contains(&(smbus >> 16)) {
        return find_pch();
    }
    Err(Absent::UnknownChipset { lpc, smbus })
}

fn find_ich9() -> Result<Tco, Absent> {
    let pmbase = config_read(CHIPSET_DEVICE, LPC_FUNCTION, PMBASE) & PMBASE_MASK;
    if pmbase == 0 {
        return Err(Absent::NoPmBase);
    }
    let cntl = config_read(CHIPSET_DEVICE, LPC_FUNCTION, ACPI_CNTL);
    if cntl & ACPI_EN == 0 {
        // SAFETY: setting ACPI_EN turns on the decode of the PMBASE firmware already assigned;
        // Linux's `lpc_ich_enable_acpi_space` does exactly this, preserving the other bits.
        unsafe { config_write(CHIPSET_DEVICE, LPC_FUNCTION, ACPI_CNTL, cntl | ACPI_EN) };
    }
    let rcba = config_read(CHIPSET_DEVICE, LPC_FUNCTION, RCBA);
    if rcba & RCBA_ENABLE == 0 {
        return Err(Absent::RcbaDisabled);
    }
    let no_reboot = NoReboot {
        phys: u64::from(rcba & RCBA_MASK) + RCBA_GCS,
        bit: GCS_NO_REBOOT,
    };
    map(no_reboot.phys)?;
    Ok(Tco {
        generation: Generation::Ich9,
        base: pmbase as u16 + PMBASE_TCO,
        no_reboot,
    })
}

fn find_pch() -> Result<Tco, Absent> {
    let tcobase = config_read(CHIPSET_DEVICE, SMBUS_FUNCTION, TCOBASE);
    let tcoctl = config_read(CHIPSET_DEVICE, SMBUS_FUNCTION, TCOCTL);
    if tcoctl & TCO_BASE_EN == 0 || tcobase & TCOBASE_MASK == 0 {
        return Err(Absent::TcoDecodeOff { tcobase, tcoctl });
    }
    // The P2SB bridge is hidden by firmware on most boards, and a hidden device answers all ones
    // to everything except `P2SBC` itself. Unhide, read, and put it back as it was (Linux
    // `p2sb.c`, which writes the whole dword the same way).
    let p2sbc = config_read(CHIPSET_DEVICE, P2SB_FUNCTION, P2SBC);
    let hidden = p2sbc & P2SBC_HIDE != 0;
    if hidden {
        // SAFETY: `P2SBC`'s only writable bit is `HIDE`; clearing it exposes the bridge's config space.
        unsafe { config_write(CHIPSET_DEVICE, P2SB_FUNCTION, P2SBC, p2sbc & !P2SBC_HIDE) };
    }
    let low = config_read(CHIPSET_DEVICE, P2SB_FUNCTION, SBREG_BAR);
    let high = config_read(CHIPSET_DEVICE, P2SB_FUNCTION, SBREG_BARH);
    if hidden {
        // SAFETY: as above, restoring the value firmware chose.
        unsafe { config_write(CHIPSET_DEVICE, P2SB_FUNCTION, P2SBC, p2sbc) };
    }
    let bar = u64::from(high) << 32 | u64::from(low & !0xf);
    if bar == 0 || low == u32::MAX {
        return Err(Absent::NoSidebandBar);
    }
    let no_reboot = NoReboot {
        phys: bar + SMBUS_GC,
        bit: GC_NO_REBOOT,
    };
    map(no_reboot.phys)?;
    Ok(Tco {
        generation: Generation::Pch,
        base: (tcobase & TCOBASE_MASK) as u16,
        no_reboot,
    })
}

/// Map the page holding `phys` device-typed in the direct map. `AlreadyMapped` is success for
/// `pci::adopt`'s reason: the same page, the same flags.
fn map(phys: u64) -> Result<(), Absent> {
    let page = phys & !(PAGE_SIZE - 1);
    match mmu::map_page(mmu::phys_to_virt(page), page, paging::Flags::device()) {
        Ok(()) | Err(paging::MapError::AlreadyMapped) => Ok(()),
        Err(_) => Err(Absent::Unmappable { phys }),
    }
}

impl Tco {
    /// Which chipset generation this is.
    pub(crate) fn generation(&self) -> Generation {
        self.generation
    }

    /// The TCO block's I/O base.
    pub fn base(&self) -> u16 {
        self.base
    }

    fn read16(&self, offset: u16) -> u16 {
        // SAFETY: a register of the TCO block `find` located and whose decode it checked.
        unsafe { in16(self.base + offset) }
    }

    fn write16(&self, offset: u16, value: u16) {
        // SAFETY: as `read16`. Every write here is one the datasheet describes for this register.
        unsafe { out16(self.base + offset, value) }
    }

    fn no_reboot_ptr(&self) -> *mut u32 {
        mmu::phys_to_virt(self.no_reboot.phys) as *mut u32
    }

    /// Set or clear `NO_REBOOT`, returning the register as read back.
    fn set_no_reboot(&self, set: bool) -> u32 {
        let ptr = self.no_reboot_ptr();
        // SAFETY: the dword `find` located and mapped device-typed. Read-modify-write, so the other
        // bits (the SMBus function-disable bit among them) keep what firmware set.
        unsafe {
            let now = core::ptr::read_volatile(ptr);
            let want = if set {
                now | self.no_reboot.bit
            } else {
                now & !self.no_reboot.bit
            };
            core::ptr::write_volatile(ptr, want);
            core::ptr::read_volatile(ptr)
        }
    }

    /// **Arm the watchdog so the machine resets `reset_after_seconds` after the last [`pet`].**
    ///
    /// Linux's `iTCO_wdt_start` order, with the status read added in front: note whether the last
    /// boot ended in this watchdog's reset, clear the status, load the count, clear `NO_REBOOT`
    /// and check it stayed clear, reload, then release the halt and check it released.
    ///
    /// [`pet`]: Tco::pet
    pub(crate) fn arm(&self, reset_after_seconds: u64) -> Result<Armed, ArmError> {
        // Two counts to a reset, so each count is half the time.
        let ticks = reset_after_seconds * 10 / 2 / TENTHS_PER_TICK;
        if ticks < u64::from(TMR_MIN) || ticks > u64::from(TMR_MASK) {
            return Err(ArmError::OutOfRange {
                seconds: reset_after_seconds,
            });
        }
        let ticks = ticks as u16;

        let previous = self.read16(TCO2_STS) & SECOND_TO_STS != 0;

        // Halt while it is being set up, so a half-programmed timer cannot fire.
        self.write16(TCO1_CNT, self.read16(TCO1_CNT) | TCO_TMR_HALT);
        self.write16(TCO1_STS, TIMEOUT);
        self.write16(TCO2_STS, SECOND_TO_STS);
        if self.generation == Generation::Ich9 {
            self.write16(TCO2_STS, BOOT_STS);
        }

        let tmr = (self.read16(TCO_TMR) & !TMR_MASK) | ticks;
        self.write16(TCO_TMR, tmr);
        let read = self.read16(TCO_TMR);
        if read & TMR_MASK != ticks {
            return Err(ArmError::TimerRefused { wrote: tmr, read });
        }

        let read = self.set_no_reboot(false);
        if read & self.no_reboot.bit != 0 {
            return Err(ArmError::NoRebootLocked {
                phys: self.no_reboot.phys,
                read,
            });
        }

        self.pet();
        self.write16(TCO1_CNT, self.read16(TCO1_CNT) & !TCO_TMR_HALT);
        let cnt = self.read16(TCO1_CNT);
        if cnt & TCO_TMR_HALT != 0 {
            return Err(ArmError::WouldNotStart { cnt });
        }

        Ok(Armed {
            ticks,
            reset_after_tenths: 2 * u64::from(ticks) * TENTHS_PER_TICK,
            previous_boot_reset_by_watchdog: previous,
        })
    }

    /// **Pet the watchdog**: reload the count. Any write to `TCO_RLD` does it on versions 2 and 4,
    /// and it also forgets a first timeout: the datasheet's `SECOND_TO_STS` needs a second timeout
    /// "before the `TCO_RLD` register was written". Linux's `iTCO_wdt_ping` is this one write.
    pub(crate) fn pet(&self) {
        self.write16(TCO_RLD, 1);
    }

    /// Ticks left in the current count, read from `TCO_RLD`. A number below the reload value is
    /// the evidence that the timer is really counting.
    pub(crate) fn ticks_left(&self) -> u16 {
        self.read16(TCO_RLD) & TMR_MASK
    }

    /// The reload value `TCO_TMR` holds, in ticks: what a pet restarts the count from.
    pub(crate) fn reload(&self) -> u16 {
        self.read16(TCO_TMR) & TMR_MASK
    }

    /// **Stop it**: halt the timer and set `NO_REBOOT` again, Linux's `iTCO_wdt_stop`. Returns
    /// whether the halt took.
    pub(crate) fn disarm(&self) -> bool {
        self.write16(TCO1_CNT, self.read16(TCO1_CNT) | TCO_TMR_HALT);
        self.set_no_reboot(true);
        self.read16(TCO1_CNT) & TCO_TMR_HALT != 0
    }
}
