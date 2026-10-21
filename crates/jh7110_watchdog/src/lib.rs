#![cfg_attr(not(test), no_std)]
//! **The `StarFive` JH7110's watchdog, as pure logic** (milestone 593 (a wedged kernel resets
//! itself), provisional number): its register offsets, the arm, pet and disarm sequences, the
//! timing arithmetic, and the device-tree query that finds it and its clocks. The volatile shell is
//! `kernel/src/drivers/jh7110_watchdog.rs`, which walks these sequences and does nothing else.
//!
//! The block is an Arm SP805 with `StarFive`'s layout. Software loads a count of core-clock ticks
//! and the counter runs down. At zero it raises its interrupt and reloads; at zero again with the
//! interrupt still raised, it asserts reset. Clearing the interrupt reloads the counter, which is
//! the pet. So the reset lands `2 x LOAD / f` after the last pet, and [`load_for`] halves the time
//! a caller asks for, the way Linux's `double_timeout` variant does, so the number a caller writes
//! is the time to the reset. Every register write is refused until `0x1ACCE551` is written to the
//! lock word, and each sequence here unlocks first and locks last.
//!
//! The core clock is `wdt_core`, a gate of the 24 MHz oscillator, so the longest reset this can
//! arm is about 357 s ([`max_reset_after_seconds`]).
//!
//! Sources, read 2026-10-20 (UTC): Linux `drivers/watchdog/starfive-wdt.c` (the JH7110 variant:
//! offsets, `RST_EN` at bit 1 of the control word, the unlock key, `double_timeout`, and its
//! `start`, `keepalive` and `stop`); Linux `arch/riscv/boot/dts/starfive/jh7110.dtsi` and
//! `drivers/clk/starfive/clk-starfive-jh7110-sys.c` (the node, and `wdt_core`'s parent `osc`);
//! `StarFive`'s U-Boot `JH7110_VisionFive2_devel` `arch/riscv/dts/jh7110.dtsi` and its clock and
//! reset headers (the node radon's firmware hands the kernel, `starfive,dskit-wdt`, clocks 123 and
//! 122, resets 109 and 110).
//!
//! # BUGS
//!
//! - **Nothing here has run on a JH7110.** QEMU has no model of this block, so CI exercises the
//!   arithmetic, the sequences and the tree query. The first test is a boot on radon; see
//!   `design/roadmap/0593-a-wedged-kernel-resets-itself.md`.
//! - **No evidence of a watchdog reset survives it** that this crate knows of. The x86_64 TCO has
//!   `SECOND_TO_STS`; no JH7110 source read here names a reset-cause register. So a second boot
//!   cannot say the watchdog ended the first, only that it came back in the expected window.
//! - **The core clock's rate is the oscillator's, taken from the clock driver, not from the
//!   tree.** A gate's rate is its parent's, and the tree names only the gate. The kernel measures
//!   the counter against its own timer before it trusts this number ([`rate_agrees`]).
//!
//! Name: provisional (milestone 593's lane, 2026-10-20 UTC). calef names crates.

use jh7110_clock_and_reset::DevicePlan;

/// `WDOGLOAD`: the count the timer starts from, and reloads from at each zero.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
pub const LOAD: usize = 0x000;
/// `WDOGVALUE`: the count now. Read only.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
pub const VALUE: usize = 0x004;
/// `WDOGCONTROL`: [`CONTROL_ENABLE`] and [`CONTROL_RESET_ENABLE`].
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
pub const CONTROL: usize = 0x008;
/// `WDOGINTCLR`: any write clears the interrupt and reloads the counter from [`LOAD`].
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
pub const INTERRUPT_CLEAR: usize = 0x00c;
/// `WDOGIMS`: nonzero while the first expiry's interrupt is raised, so the next zero resets.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
pub const INTERRUPT_STATUS: usize = 0x014;
/// `WDOGLOCK`: [`UNLOCK_KEY`] opens every other register to writes; anything else closes them.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
pub const LOCK: usize = 0xc00;
/// The SP805's own unlock key.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
pub const UNLOCK_KEY: u32 = 0x1acc_e551;
/// Bit 0 of [`CONTROL`]: the counter runs and its interrupt is live.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
pub const CONTROL_ENABLE: u32 = 0b01;
/// Bit 1 of [`CONTROL`]: the second expiry asserts reset. Linux's `STARFIVE_WDT_JH7110_RST_EN_SHIFT`.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
pub const CONTROL_RESET_ENABLE: u32 = 1 << 1;

/// The part of the 64 KiB window the driver touches: every register above, [`LOCK`] last, is in
/// the first page.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
pub const WINDOW_USED: u64 = 0x1000;

/// `wdt_core`'s rate: the 24 MHz oscillator it gates (Linux's `JH71X0_GATE(JH7110_SYSCLK_WDT_CORE,
/// "wdt_core", 0, JH7110_SYSCLK_OSC)`).
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
pub const CORE_CLOCK_HZ: u64 = 24_000_000;

/// Mainline's binding, `watchdog@13070000`.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
pub const COMPATIBLE_MAINLINE: &[u8] = b"starfive,jh7110-wdt";
/// The binding `StarFive`'s U-Boot uses, `wdog@13070000`, which is the tree radon's firmware hands
/// the kernel. Tried second, for the reason `jh7110_clock_and_reset::discover` gives.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
pub const COMPATIBLE_VENDOR: &[u8] = b"starfive,dskit-wdt";

/// One register access in a sequence the kernel walks in order.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Store `value` at `offset`.
    Write {
        /// Byte offset in the window.
        offset: usize,
        /// The word stored.
        value: u32,
    },
    /// Read the word at `offset`, set `mask`, store it back.
    Set {
        /// Byte offset in the window.
        offset: usize,
        /// The bits set.
        mask: u32,
    },
    /// Read the word at `offset`, clear `mask`, store it back.
    Clear {
        /// Byte offset in the window.
        offset: usize,
        /// The bits cleared.
        mask: u32,
    },
}

const UNLOCK: Access = Access::Write {
    offset: LOCK,
    value: UNLOCK_KEY,
};
const RELOCK: Access = Access::Write {
    offset: LOCK,
    value: !UNLOCK_KEY,
};
const CLEAR_INTERRUPT: Access = Access::Write {
    offset: INTERRUPT_CLEAR,
    value: 1,
};

/// **Arm the watchdog with `load`**, Linux's `starfive_wdt_start` with the count set inside it:
/// stop it first so a timer firmware left running cannot expire mid-sequence, enable the reset,
/// clear a stale interrupt (which would otherwise make the next zero the resetting one), load the
/// count, and only then start it.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
#[must_use]
pub const fn arm_sequence(load: u32) -> [Access; 7] {
    [
        UNLOCK,
        Access::Clear {
            offset: CONTROL,
            mask: CONTROL_ENABLE,
        },
        Access::Set {
            offset: CONTROL,
            mask: CONTROL_RESET_ENABLE,
        },
        CLEAR_INTERRUPT,
        Access::Write {
            offset: LOAD,
            value: load,
        },
        Access::Set {
            offset: CONTROL,
            mask: CONTROL_ENABLE,
        },
        RELOCK,
    ]
}

/// **Pet it**, Linux's `starfive_wdt_keepalive`: clear the interrupt, which reloads the counter
/// and drops a first expiry, then write the count back as Linux does.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
#[must_use]
pub const fn pet_sequence(load: u32) -> [Access; 4] {
    [
        UNLOCK,
        CLEAR_INTERRUPT,
        Access::Write {
            offset: LOAD,
            value: load,
        },
        RELOCK,
    ]
}

/// **Stop it**, Linux's `starfive_wdt_stop`: the counter halts and neither expiry can happen.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
#[must_use]
pub const fn disarm_sequence() -> [Access; 3] {
    [
        UNLOCK,
        Access::Clear {
            offset: CONTROL,
            mask: CONTROL_ENABLE,
        },
        RELOCK,
    ]
}

/// **The count that resets `reset_after_seconds` after the last pet**, at `hz`: half the ticks,
/// because the block counts twice. `None` when that is zero or does not fit the 32-bit register.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
#[must_use]
pub const fn load_for(reset_after_seconds: u64, hz: u64) -> Option<u32> {
    let Some(ticks) = reset_after_seconds.checked_mul(hz) else {
        return None;
    };
    let load = ticks / 2;
    if load == 0 || load > u32::MAX as u64 {
        None
    } else {
        Some(load as u32)
    }
}

/// The time from a pet to the reset for `load` at `hz`, in tenths of a second, for the banner.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
#[must_use]
pub const fn reset_after_tenths(load: u32, hz: u64) -> u64 {
    if hz == 0 {
        return 0;
    }
    2 * load as u64 * 10 / hz
}

/// The longest reset this block can arm at `hz`, in whole seconds.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
#[must_use]
pub const fn max_reset_after_seconds(hz: u64) -> u64 {
    if hz == 0 {
        return 0;
    }
    2 * u32::MAX as u64 / hz
}

/// **The counter's rate as measured**: it fell from `before` to `after` while the kernel's own
/// timer advanced `elapsed` ticks of `timer_hz`. A counter that did not move, or moved up (it
/// reloaded), measures zero.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
#[must_use]
pub const fn measured_hz(before: u32, after: u32, elapsed: u64, timer_hz: u64) -> u64 {
    if elapsed == 0 || after >= before {
        return 0;
    }
    (before - after) as u64 * timer_hz / elapsed
}

/// **Does the measured rate agree with the rate the count was computed for?** Within 5 %: wide
/// enough for a measurement over a fraction of a second, narrow enough that a counter on the wrong
/// clock (a divided bus clock, or a halted one) is refused rather than armed at the wrong time.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
#[must_use]
pub const fn rate_agrees(measured: u64, expected: u64) -> bool {
    let slack = expected / 20;
    measured >= expected - slack && measured <= expected + slack
}

/// What [`discover`] found.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Discovered {
    /// Physical base of the watchdog's window, from the node's `reg`.
    pub base: u64,
    /// The window's size as the tree states it.
    pub size: u64,
    /// Which binding matched.
    pub compatible: &'static [u8],
    /// The node's own clocks and resets, as SYS-domain steps, clocks first.
    pub clocks_and_resets: DevicePlan,
}

/// **Find the JH7110 watchdog in `tree`**, mainline's binding first, then the vendor's. `None`
/// when the tree names neither, or names one with no `reg`: there is no constant to fall back on,
/// because a watchdog nobody described is not one to arm.
///
/// # Errors
///
/// Propagates [`device_tree_blob::Error`] if the blob is malformed.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
pub fn discover(
    tree: &device_tree_blob::DeviceTreeBlob<'_>,
) -> Result<Option<Discovered>, device_tree_blob::Error> {
    for compatible in [COMPATIBLE_MAINLINE, COMPATIBLE_VENDOR] {
        let mut reg = [device_tree_blob::Region { start: 0, size: 0 }; 1];
        if tree.node_reg_compatible(compatible, &mut reg)? == 0 {
            continue;
        }
        return Ok(Some(Discovered {
            base: reg[0].start,
            size: reg[0].size,
            compatible,
            clocks_and_resets: jh7110_clock_and_reset::device_plan(tree, compatible)?,
        }));
    }
    Ok(None)
}

#[cfg(test)]
mod tests;
