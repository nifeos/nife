//! **The I219 bring-up, in FreeBSD's order.**
//!
//! A driver calls three passes in this order: [`initialize_physical_layer_device_workarounds`] at
//! attach, [`global_reset`] after its descriptor-ring flush, and [`initialize_hardware`] before the
//! rings are programmed, with [`transmit_errata`] once transmit is on. Only FreeBSD's
//! `e1000_pch_spt` branches are carried ([`super::is_pch`]), and each function names the FreeBSD
//! function it follows.

//! ```text
//! SPDX-License-Identifier: BSD-3-Clause
//!
//! Copyright (c) 2001-2020, Intel Corporation
//! All rights reserved.
//!
//! Redistribution and use in source and binary forms, with or without
//! modification, are permitted provided that the following conditions are met:
//!
//!  1. Redistributions of source code must retain the above copyright notice,
//!     this list of conditions and the following disclaimer.
//!
//!  2. Redistributions in binary form must reproduce the above copyright
//!     notice, this list of conditions and the following disclaimer in the
//!     documentation and/or other materials provided with the distribution.
//!
//!  3. Neither the name of the Intel Corporation nor the names of its
//!     contributors may be used to endorse or promote products derived from
//!     this software without specific prior written permission.
//!
//! THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
//! AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//! IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
//! ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE
//! LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
//! CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
//! SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
//! INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
//! CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//! ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
//! POSSIBILITY OF SUCH DAMAGE.
//! ```

use super::physical_layer_device::{self, Swflag, r};
use super::{Hardware, delay_ms, flush_writes, non_volatile_memory, regs, reset, ulp};
use crate::regs as mac;

/// Why the bring-up stopped. Each is a line on the bench boot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// A PHY access failed.
    ///
    /// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `phy` (the chip) is spelled out
    /// `physical_layer_device`; no ruling covers the rest of the name. Refused `Phy` (an
    /// abbreviation).
    PhysicalLayerDevice(physical_layer_device::Error),
    /// An NVM read failed, or no bank is valid ("EEPROM not present" in FreeBSD).
    ///
    /// Name: ratified 2026-10-26 (UTC), in the ruling on e1000e's names. Refused
    /// `Nvm` (an abbreviation, as the module's `nvm` was).
    NonVolatileMemory(non_volatile_memory::Error),
    /// The PHY did not answer after the `SMBus` force, the `LANPHYPC` power cycle and the unforce.
    ///
    /// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `phy` (the chip) is spelled out
    /// `physical_layer_device`; no ruling covers the rest of the name. Refused `PhyInaccessible`
    /// (an abbreviation).
    PhysicalLayerDeviceInaccessible,
    /// The PHY did not answer and firmware blocks the `LANPHYPC` toggle that would have reset it.
    LanphypcBlocked,
    /// Firmware began blocking PHY access after the PHY reset.
    ///
    /// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `phy` (the chip) is spelled out
    /// `physical_layer_device`; no ruling covers the rest of the name. Refused
    /// `BlockedAfterPhyReset` (an abbreviation).
    BlockedAfterPhysicalLayerDeviceReset,
    /// `CTRL.RST` did not self-clear.
    ResetTimeout,
}

impl From<physical_layer_device::Error> for Error {
    fn from(e: physical_layer_device::Error) -> Self {
        Error::PhysicalLayerDevice(e)
    }
}

impl From<non_volatile_memory::Error> for Error {
    fn from(e: non_volatile_memory::Error) -> Self {
        Error::NonVolatileMemory(e)
    }
}

/// Where the PHY was first seen to answer, in [`initialize_physical_layer_device_workarounds`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reached {
    /// At once: the interconnect was already PCIe.
    Directly,
    /// After forcing the MAC into System Management Bus (`SMBus`) mode.
    ///
    /// Name: ratified 2026-10-26 (UTC), in the ruling on e1000e's names. Refused
    /// `InSmbus` (an abbreviation of System Management Bus).
    #[doc(alias = "smbus")]
    InSystemManagementBus,
    /// After the `LANPHYPC` power cycle.
    AfterLanphypc,
    /// After the power cycle and unforcing `SMBus` in the MAC.
    AfterUnforce,
}

/// What the bring-up observed that the bench boot should print. None of it changes what runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Note {
    /// The Management Engine was asked to leave ULP; `cleared` says whether it finished in time.
    UlpByMe {
        /// `FWSM.ULP_CFG_DONE` cleared before [`ulp::WAIT_MS`].
        cleared: bool,
    },
    /// There is no ME; ULP was left by PHY registers, with this outcome.
    UlpBySoftware(Result<(), Error>),
    /// The PHY answered, with this identifier (revision masked off).
    ///
    /// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `phy` (the chip) is spelled out
    /// `physical_layer_device`; no ruling covers the rest of the name. Refused `PhyReached` (an
    /// abbreviation).
    PhysicalLayerDeviceReached {
        /// Where.
        at: Reached,
        /// `PHY_ID1:PHY_ID2`, revision nibble cleared.
        id: u32,
    },
    /// Firmware blocks PHY resets, so FreeBSD's PHY reset was skipped.
    ///
    /// Name: ratified 2026-10-26 (UTC), in the tree-wide ruling that spells out `phy` (the chip) as
    /// `physical_layer_device`, which names it. Refused `PhyResetBlocked` (an abbreviation).
    PhysicalLayerDeviceResetBlocked,
    /// `STATUS.LAN_INIT_DONE` did not rise in 150 ms after a reset.
    LanInitTimeout,
    /// `STATUS.PHYRA` was not set after a PHY reset (FreeBSD: "needs delay").
    ///
    /// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `phy` (the chip) is spelled out
    /// `physical_layer_device`; no ruling covers the rest of the name. Refused
    /// `PhyResetNotAsserted` (an abbreviation).
    PhysicalLayerDeviceResetNotAsserted,
    /// The semaphore could not be taken before the global reset; FreeBSD resets anyway.
    SwflagBeforeReset(physical_layer_device::Error),
    /// Master requests were still pending when the reset went ahead.
    MasterRequestsPending,
    /// The NVM's extended configuration region was replayed into the LAN connected device (LCD,
    /// Intel's name for the PHY beside a PCH): this many words.
    ///
    /// Name: ratified 2026-10-26 (UTC), in the ruling on e1000e's names. Refused
    /// `LcdConfigured` (`Lcd` reads as a display).
    #[doc(alias = "lcd")]
    LanConnectedDeviceConfigured {
        /// (value, register) pairs written.
        pairs: u32,
    },
    /// No extended configuration to replay into the LAN connected device (`FEXTNVM.SW_CONFIG`
    /// clear, or a zero size).
    ///
    /// Name: ratified 2026-10-26 (UTC), in the ruling on e1000e's names. Refused
    /// `LcdNotConfigured` (as `LcdConfigured`).
    #[doc(alias = "lcd")]
    LanConnectedDeviceNotConfigured,
    /// `STRAP` names a System Management Bus (`SMBus`) frequency of zero, which FreeBSD declines to
    /// program.
    ///
    /// Name: ratified 2026-10-26 (UTC), in the ruling on e1000e's names. Refused
    /// `SmbusFrequencyUnsupported` (an abbreviation of System Management Bus).
    #[doc(alias = "smbus")]
    SystemManagementBusFrequencyUnsupported,
    /// The host wakeup bit could not be read after the reset, so it was left as it was.
    HostWakeupUnread(physical_layer_device::Error),
}

// `CTRL` bits.
const CTRL_LANPHYPC_OVERRIDE: u32 = 0x0001_0000;
const CTRL_LANPHYPC_VALUE: u32 = 0x0002_0000;
const CTRL_PHY_RST: u32 = 0x8000_0000;
const CTRL_FRCSPD: u32 = 0x0000_0800;
const CTRL_FRCDPX: u32 = 0x0000_1000;
const CTRL_MEHE: u32 = 0x0008_0000;
// `CTRL_EXT` bits.
const CTRL_EXT_LPCD: u32 = 0x0000_0004;
const CTRL_EXT_FORCE_SMBUS: u32 = 0x0000_0800;
const CTRL_EXT_RO_DIS: u32 = 0x0002_0000;
const CTRL_EXT_PHYPDEN: u32 = 0x0010_0000;
// `STATUS` bits.
const STATUS_LAN_INIT_DONE: u32 = 0x0000_0200;
const STATUS_PHYRA: u32 = 0x0000_0400;
// Others, each named for its FreeBSD define.
const EXTCNF_CTRL_GATE_PHY_CFG: u32 = 0x0000_0080;
const EXTCNF_CTRL_EXT_CNF_POINTER_MASK: u32 = 0x0fff_0000;
const EXTCNF_SIZE_EXT_PCIE_LENGTH_MASK: u32 = 0x00ff_0000;
const FEXTNVM_SW_CONFIG_ICH8M: u32 = 1 << 27;
const FEXTNVM3_PHY_CFG_COUNTER_MASK: u32 = 0x0c00_0000;
const FEXTNVM3_PHY_CFG_COUNTER_50MSEC: u32 = 0x0800_0000;
const FEXTNVM7_DISABLE_SMB_PERST: u32 = 0x0000_0020;
const PHY_CTRL_GBE_DISABLE: u32 = 0x0000_0040;
const PHY_CTRL_D0A_LPLU: u32 = 0x0000_0002;
const STRAP_SYSTEM_MANAGEMENT_BUS_ADDRESS_MASK: u32 = 0x00fe_0000;
const STRAP_SYSTEM_MANAGEMENT_BUS_ADDRESS_SHIFT: u32 = 17;
const STRAP_SMT_FREQ_MASK: u32 = 0x0000_3000;
const STRAP_SMT_FREQ_SHIFT: u32 = 12;
const TCTL_PSP: u32 = 0x0000_0008;
const TCTL_MULR: u32 = 0x1000_0000;
const RFCTL_NFSW_DIS: u32 = 0x0000_0040;
const RFCTL_NFSR_DIS: u32 = 0x0000_0080;
const PBECCSTS_ECC_ENABLE: u32 = 0x0001_0000;
const PBA_26K: u32 = 0x001a;
const TXDCTL_WTHRESH: u32 = 0x003f_0000;
const TXDCTL_PTHRESH: u32 = 0x0000_003f;
const TXDCTL_FULL_TX_DESC_WB: u32 = 0x0101_0000;
const TXDCTL_MAX_TX_DESC_PREFETCH: u32 = 0x0100_001f;
const RCTL_RDMTS_HEX: u32 = 0x0001_0000;
const TARC0_CB_MULTIQ_3_REQ: u32 = 0x3000_0000;
const TARC0_CB_MULTIQ_2_REQ: u32 = 0x2000_0000;
// PHY register bits.
const SMB_CTRL_FORCE_SMBUS: u16 = 0x0001;
const PM_CTRL_K1_ENABLE: u16 = 0x4000;
const ULP_CONFIG1_START: u16 = 0x0001;
const ULP_CONFIG1_CLEARED: u16 = 0x0004 // IND
    | 0x0010 // STICKY_ULP
    | 0x0100 // RESET_TO_SMBUS
    | 0x0040 // WOL_HOST
    | 0x0020 // INBAND_EXIT
    | 0x0400 // EN_ULP_LANPHYPC
    | 0x0800 // DIS_CLR_STICKY_ON_PERST
    | 0x1000; // DISABLE_SMB_PERST
const PORT_GEN_CFG_HOST_WU: u16 = 1 << 4;
const SMB_ADDR_MASK: u16 = 0x007f;
const SMB_ADDR_PEC_EN: u16 = 0x0200;
const SMB_ADDR_VALID: u16 = 0x0080;
const SMB_ADDR_FREQ_MASK: u16 = 0x1100;
const SMB_ADDR_FREQ_LOW_SHIFT: u16 = 8;
const SMB_ADDR_FREQ_HIGH_SHIFT: u16 = 12;
const OEM_BITS_GBE_DIS: u16 = 0x0040;
const OEM_BITS_LPLU: u16 = 0x0004;
const OEM_BITS_RESTART_AN: u16 = 0x0400;
const I82577_CFG_ASSERT_CRS_ON_TX: u16 = 1 << 15;
const I82577_CFG_ENABLE_DOWNSHIFT: u16 = 3 << 10;
const I82577_PHY_CTRL2_MDIX_CFG_MASK: u16 = 0x0600;
const I82577_PHY_CTRL2_AUTO_MDI_MDIX: u16 = 0x0400;
const MII_CR_AUTO_NEG_EN: u16 = 0x1000;
const MII_CR_RESTART_AUTO_NEG: u16 = 0x0200;
const NWAY_AR_10T_HD_CAPS: u16 = 0x0020;
const NWAY_AR_10T_FD_CAPS: u16 = 0x0040;
const NWAY_AR_100TX_HD_CAPS: u16 = 0x0080;
const NWAY_AR_100TX_FD_CAPS: u16 = 0x0100;
const NWAY_AR_PAUSE: u16 = 0x0400;
const NWAY_AR_ASM_DIR: u16 = 0x0800;
const CR_1000T_HD_CAPS: u16 = 0x0100;
const CR_1000T_FD_CAPS: u16 = 0x0200;

fn set(hardware: &mut impl Hardware, off: u64, bits: u32) {
    let v = hardware.read(off);
    hardware.write(off, v | bits);
}

fn clear(hardware: &mut impl Hardware, off: u64, bits: u32) {
    let v = hardware.read(off);
    hardware.write(off, v & !bits);
}

fn has_management_engine(hardware: &mut impl Hardware) -> bool {
    ulp::has_management_engine(hardware.read(regs::FWSM))
}

// ---------------------------------------------------------------------------------------------
// Pass 1: at attach.
// ---------------------------------------------------------------------------------------------

/// **`e1000_init_phy_workarounds_pchlan`**, the SPT path. Returns the PHY's identifier.
///
/// Name: ratified 2026-10-26 (UTC), in the tree-wide rulings that spell out `phy` (the chip) as
/// `physical_layer_device` and `init` as `initialize`, the second of which names it. Refused
/// `init_phy_workarounds` (both abbreviations).
pub fn initialize_physical_layer_device_workarounds(
    hardware: &mut impl Hardware,
) -> Result<u32, Error> {
    // Gate automatic PHY configuration by hardware; this driver configures it in `global_reset`.
    set(hardware, regs::EXTCNF_CTRL, EXTCNF_CTRL_GATE_PHY_CFG);

    // "It is not possible to be certain of the current state of ULP so forcibly disable it."
    // FreeBSD reports a failure here and carries on; so does this.
    leave_ulp(hardware);

    let f = Swflag::acquire(hardware)?;
    let mut id = 0;
    let reached = (|| {
        if accessible(hardware, &f, &mut id) {
            return Ok(Reached::Directly);
        }
        // Before toggling LANPHYPC, see whether forcing the MAC into SMBus mode is enough.
        set(hardware, mac::CTRL_EXT, CTRL_EXT_FORCE_SMBUS);
        // "Wait 50 milliseconds for MAC to finish any retries that it might be trying to
        // perform from previous attempts to acknowledge any phy read requests."
        delay_ms(hardware, 50);
        if accessible(hardware, &f, &mut id) {
            return Ok(Reached::InSystemManagementBus);
        }
        if physical_layer_device::reset_blocked(hardware) {
            return Err(Error::LanphypcBlocked);
        }
        toggle_lanphypc(hardware);
        if accessible(hardware, &f, &mut id) {
            return Ok(Reached::AfterLanphypc);
        }
        // Toggling LANPHYPC brings the PHY out of SMBus mode, so take the MAC out too.
        clear(hardware, mac::CTRL_EXT, CTRL_EXT_FORCE_SMBUS);
        if accessible(hardware, &f, &mut id) {
            return Ok(Reached::AfterUnforce);
        }
        Err(Error::PhysicalLayerDeviceInaccessible)
    })();
    f.release(hardware);
    let at = reached?;
    hardware.note(Note::PhysicalLayerDeviceReached { at, id });

    if physical_layer_device::reset_blocked(hardware) {
        // FreeBSD: ERROR_REPORT("Reset blocked by ME") and success.
        hardware.note(Note::PhysicalLayerDeviceResetBlocked);
        return Ok(id);
    }
    // "Reset the PHY before any access to it. Doing so, ensures that the PHY is in a known good
    // state before we read/write PHY registers."
    physical_layer_device_reset_generic(hardware)?;
    if physical_layer_device::reset_blocked(hardware) {
        return Err(Error::BlockedAfterPhysicalLayerDeviceReset);
    }
    Ok(id)
}

/// **`e1000_disable_ulp_lpt_lp` with `force`.** Through the ME when there is one; by PHY registers
/// when not. Reports rather than returns, as its caller does.
fn leave_ulp(hardware: &mut impl Hardware) {
    if has_management_engine(hardware) {
        let h = hardware.read(regs::H2ME);
        hardware.write(regs::H2ME, ulp::request(h));
        let mut cleared = false;
        for i in 0..=ulp::WAIT_MS / 10 {
            if ulp::done(hardware.read(regs::FWSM)) {
                cleared = true;
                break;
            }
            if i < ulp::WAIT_MS / 10 {
                delay_ms(hardware, 10);
            }
        }
        if cleared {
            let h = hardware.read(regs::H2ME);
            hardware.write(regs::H2ME, ulp::release(h));
        }
        hardware.note(Note::UlpByMe { cleared });
        return;
    }
    let outcome = leave_ulp_by_physical_layer_device(hardware);
    hardware.note(Note::UlpBySoftware(outcome));
}

/// The software branch of `e1000_disable_ulp_lpt_lp`.
fn leave_ulp_by_physical_layer_device(hardware: &mut impl Hardware) -> Result<(), Error> {
    let f = Swflag::acquire(hardware)?;
    toggle_lanphypc(hardware);
    let done = (|| -> Result<(), physical_layer_device::Error> {
        // Unforce SMBus mode in the PHY. If the MAC is in PCIe mode the read fails, so force the
        // MAC to SMBus long enough to reach the PHY.
        let v = match physical_layer_device::read(hardware, &f, r::SMB_CTRL) {
            Ok(v) => v,
            Err(_) => {
                set(hardware, mac::CTRL_EXT, CTRL_EXT_FORCE_SMBUS);
                delay_ms(hardware, 50);
                physical_layer_device::read(hardware, &f, r::SMB_CTRL)?
            }
        };
        // FreeBSD ignores the result of these writes; so does this.
        let _ = physical_layer_device::write(hardware, &f, r::SMB_CTRL, v & !SMB_CTRL_FORCE_SMBUS);
        clear(hardware, mac::CTRL_EXT, CTRL_EXT_FORCE_SMBUS);
        // "When ULP mode was previously entered, K1 was disabled by the hardware. Re-Enable K1
        // in the PHY when exiting ULP."
        let pm = physical_layer_device::read(hardware, &f, r::PM_CTRL)?;
        let _ = physical_layer_device::write(hardware, &f, r::PM_CTRL, pm | PM_CTRL_K1_ENABLE);
        // Clear the ULP configuration, then commit it by starting auto ULP configuration.
        let u = physical_layer_device::read(hardware, &f, r::ULP_CONFIG1)? & !ULP_CONFIG1_CLEARED;
        let _ = physical_layer_device::write(hardware, &f, r::ULP_CONFIG1, u);
        let _ = physical_layer_device::write(hardware, &f, r::ULP_CONFIG1, u | ULP_CONFIG1_START);
        clear(hardware, regs::FEXTNVM7, FEXTNVM7_DISABLE_SMB_PERST);
        Ok(())
    })();
    f.release(hardware);
    // With `force`, FreeBSD resets the PHY whatever happened above, and waits 50 ms.
    let reset = physical_layer_device_reset_ich8lan(hardware);
    delay_ms(hardware, 50);
    done?;
    reset
}

/// **`e1000_phy_is_accessible_pchlan`**, for parts at Lynx Point and later. Two tries at the
/// identifier; on success, and only without an ME, unforce `SMBus` in the PHY and the MAC.
///
/// FreeBSD's quirk is kept: an identifier that reads as `0xffff` without an `MDIC` error counts as
/// accessible on a first probe (its `ret_val` is zero), so does this.
fn accessible(hardware: &mut impl Hardware, f: &Swflag, id: &mut u32) -> bool {
    let mut seen = 0u32;
    let mut last = Ok(0);
    for _ in 0..2 {
        last = physical_layer_device::read(hardware, f, r::ID1);
        let Ok(hi) = last else { continue };
        if hi == 0xffff {
            continue;
        }
        last = physical_layer_device::read(hardware, f, r::ID2);
        let Ok(lo) = last else { continue };
        if lo == 0xffff {
            continue;
        }
        seen = (u32::from(hi) << 16 | u32::from(lo)) & physical_layer_device::ID_MASK;
        break;
    }
    let known = *id != 0;
    let ok = if known {
        *id == seen || last.is_ok()
    } else if seen != 0 {
        *id = seen;
        true
    } else {
        last.is_ok()
    };
    if ok && !has_management_engine(hardware) {
        if let Ok(v) = physical_layer_device::read(hardware, f, r::SMB_CTRL) {
            let _ =
                physical_layer_device::write(hardware, f, r::SMB_CTRL, v & !SMB_CTRL_FORCE_SMBUS);
        }
        clear(hardware, mac::CTRL_EXT, CTRL_EXT_FORCE_SMBUS);
    }
    ok
}

/// **`e1000_toggle_lanphypc_pch_lpt`**: fully power-cycle the PHY by driving the `LANPHYPC` pin
/// low for a millisecond, then wait for the hardware's "LCD power cycle done".
fn toggle_lanphypc(hardware: &mut impl Hardware) {
    let n3 = hardware.read(regs::FEXTNVM3);
    hardware.write(
        regs::FEXTNVM3,
        (n3 & !FEXTNVM3_PHY_CFG_COUNTER_MASK) | FEXTNVM3_PHY_CFG_COUNTER_50MSEC,
    );
    let ctrl = (hardware.read(mac::CTRL) | CTRL_LANPHYPC_OVERRIDE) & !CTRL_LANPHYPC_VALUE;
    hardware.write(mac::CTRL, ctrl);
    flush_writes(hardware);
    delay_ms(hardware, 1);
    hardware.write(mac::CTRL, ctrl & !CTRL_LANPHYPC_OVERRIDE);
    flush_writes(hardware);
    // FreeBSD: up to 21 polls of 5 ms for CTRL_EXT.LPCD, then 30 ms whatever it said.
    for _ in 0..=20 {
        delay_ms(hardware, 5);
        if hardware.read(mac::CTRL_EXT) & CTRL_EXT_LPCD != 0 {
            break;
        }
    }
    delay_ms(hardware, 30);
}

/// **`e1000_phy_hw_reset_generic`**: pulse `CTRL.PHY_RST` under the semaphore, then wait for the
/// configuration to finish.
fn physical_layer_device_reset_generic(hardware: &mut impl Hardware) -> Result<(), Error> {
    if physical_layer_device::reset_blocked(hardware) {
        return Ok(());
    }
    let f = Swflag::acquire(hardware)?;
    let ctrl = hardware.read(mac::CTRL);
    hardware.write(mac::CTRL, ctrl | CTRL_PHY_RST);
    flush_writes(hardware);
    // `phy->reset_delay_us`, 100 for every PCH part.
    hardware.delay_us(100);
    hardware.write(mac::CTRL, ctrl);
    flush_writes(hardware);
    hardware.delay_us(150);
    f.release(hardware);
    config_done(hardware)
}

/// **`e1000_phy_hw_reset_ich8lan`**: the generic reset and then the post-reset configuration (the
/// 82579 gating it begins with is not this class's).
fn physical_layer_device_reset_ich8lan(hardware: &mut impl Hardware) -> Result<(), Error> {
    physical_layer_device_reset_generic(hardware)?;
    post_physical_layer_device_reset(hardware)
}

/// **`e1000_get_cfg_done_ich8lan`**: 10 ms, then `LAN_INIT_DONE`, then clear `PHYRA`, then check
/// that an NVM bank is valid.
fn config_done(hardware: &mut impl Hardware) -> Result<(), Error> {
    delay_ms(hardware, 10);
    // `e1000_lan_init_done_ich8lan`: up to 1500 polls of 100 µs.
    let mut done = false;
    for _ in 0..1500 {
        let s = hardware.read(mac::STATUS);
        hardware.delay_us(100);
        if s & STATUS_LAN_INIT_DONE != 0 {
            done = true;
            break;
        }
    }
    if !done {
        hardware.note(Note::LanInitTimeout);
    }
    clear(hardware, mac::STATUS, STATUS_LAN_INIT_DONE);
    let s = hardware.read(mac::STATUS);
    if s & STATUS_PHYRA != 0 {
        hardware.write(mac::STATUS, s & !STATUS_PHYRA);
    } else {
        hardware.note(Note::PhysicalLayerDeviceResetNotAsserted);
    }
    let g = non_volatile_memory::Geometry::read(hardware);
    non_volatile_memory::valid_bank(hardware, g)?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Pass 2: the global reset.
// ---------------------------------------------------------------------------------------------

/// **`e1000_reset_hw_ich8lan`** for the SPT class, preceded by `em_reset`'s packet-buffer
/// allocation and followed by nothing the kernel does not already do for the 82574L.
///
/// One addition: FreeBSD does not wait for `CTRL.RST` to self-clear (its configuration-done wait
/// covers it); this waits up to `reset_wait_ms` first, so a reset that never completes is reported
/// as that rather than as a PHY timeout.
pub fn global_reset(hardware: &mut impl Hardware, reset_wait_ms: u64) -> Result<(), Error> {
    hardware.write(regs::PBA, PBA_26K);

    // `e1000_disable_pcie_master_generic`: stop new bus-master requests, wait for pending ones.
    set(hardware, mac::CTRL, reset::CTRL_GIO_MASTER_DISABLE);
    let mut idle = false;
    for _ in 0..reset::MASTER_WAIT_US / 100 {
        if hardware.read(mac::STATUS) & reset::STATUS_GIO_MASTER_ENABLE == 0 {
            idle = true;
            break;
        }
        hardware.delay_us(100);
    }
    if !idle {
        hardware.note(Note::MasterRequestsPending);
    }

    hardware.write(mac::IMC, u32::MAX);
    hardware.write(mac::RCTL, 0);
    hardware.write(mac::TCTL, TCTL_PSP);
    flush_writes(hardware);
    delay_ms(hardware, 10);

    // "Full-chip reset requires MAC and PHY reset at the same time to make sure the interface
    // between MAC and the external PHY is reset."
    let mut ctrl = hardware.read(mac::CTRL);
    let with_physical_layer_device = !physical_layer_device::reset_blocked(hardware);
    if with_physical_layer_device {
        ctrl |= CTRL_PHY_RST;
    }
    match Swflag::acquire(hardware) {
        Ok(f) => f.lost_to_reset(),
        Err(e) => hardware.note(Note::SwflagBeforeReset(e)),
    }
    // "Configuration space access creates the needed delay." Written to the read-only STRAP so
    // the read happens before the reset, and again before any MAC access after it.
    let v = hardware.pci_vendor_id();
    hardware.write(regs::STRAP, u32::from(v));
    hardware.write(mac::CTRL, ctrl | crate::CTRL_RST);
    // "cannot issue a flush here because it hangs the hardware"
    delay_ms(hardware, 20);
    let v = hardware.pci_vendor_id();
    hardware.write(regs::STRAP, u32::from(v));

    let mut cleared = false;
    for _ in 0..reset_wait_ms {
        if hardware.read(mac::CTRL) & crate::CTRL_RST == 0 {
            cleared = true;
            break;
        }
        delay_ms(hardware, 1);
    }
    if !cleared {
        return Err(Error::ResetTimeout);
    }

    if with_physical_layer_device {
        config_done(hardware)?;
        post_physical_layer_device_reset(hardware)?;
    }
    hardware.write(mac::IMC, u32::MAX);
    let _ = hardware.read(mac::ICR);
    set(hardware, regs::KABGTXD, reset::KABGTXD_BGSQLBIAS);
    Ok(())
}

/// **`e1000_post_phy_reset_ich8lan`**, SPT: clear the host wakeup bit, replay the NVM's PHY
/// configuration, and apply the OEM bits.
fn post_physical_layer_device_reset(hardware: &mut impl Hardware) -> Result<(), Error> {
    if physical_layer_device::reset_blocked(hardware) {
        return Ok(());
    }
    // "Allow time for h/w to get to quiescent state after reset"
    delay_ms(hardware, 10);
    {
        let f = Swflag::acquire(hardware)?;
        match physical_layer_device::read(hardware, &f, r::PORT_GEN_CFG) {
            Ok(v) => {
                let _ = physical_layer_device::write(
                    hardware,
                    &f,
                    r::PORT_GEN_CFG,
                    v & !PORT_GEN_CFG_HOST_WU,
                );
            }
            Err(e) => hardware.note(Note::HostWakeupUnread(e)),
        }
        f.release(hardware);
    }
    lan_connected_device_configuration(hardware)?;
    oem_bits(hardware)
}

/// **`e1000_sw_lcd_config_ich8lan`**, the LAN connected device (LCD) configuration: when the NVM
/// says software configures the PHY ("an issue
/// where the NVM configuration is not properly autoloaded after power transitions"), write the
/// `SMBus` address and LED configuration, then replay the extended configuration region's
/// (value, register) pairs into the PHY.
#[doc(alias = "lcd")]
fn lan_connected_device_configuration(hardware: &mut impl Hardware) -> Result<(), Error> {
    let f = Swflag::acquire(hardware)?;
    let out = (|| {
        if hardware.read(regs::FEXTNVM) & FEXTNVM_SW_CONFIG_ICH8M == 0 {
            return Ok(None);
        }
        let ext = hardware.read(regs::EXTCNF_CTRL);
        let size = (hardware.read(regs::EXTCNF_SIZE) & EXTCNF_SIZE_EXT_PCIE_LENGTH_MASK) >> 16;
        if size == 0 {
            return Ok(None);
        }
        let base = (ext & EXTCNF_CTRL_EXT_CNF_POINTER_MASK) >> 16;
        // Past the 82578 generation the hardware leaves these to software.
        write_system_management_bus_address(hardware, &f)?;
        let led = hardware.read(regs::LEDCTL) as u16;
        physical_layer_device::write(hardware, &f, r::LED_CONFIG, led)?;
        let g = non_volatile_memory::Geometry::read(hardware);
        let word = base << 1;
        let mut page: u32 = 0;
        for i in 0..size {
            let value = non_volatile_memory::read_word(hardware, g, word + i * 2)?;
            let at = u32::from(non_volatile_memory::read_word(
                hardware,
                g,
                word + i * 2 + 1,
            )?);
            if at == physical_layer_device::PAGE_SELECT {
                // Save the page for the writes that follow; it is already `page << 5`.
                page = u32::from(value);
                continue;
            }
            physical_layer_device::write(
                hardware,
                &f,
                (at & physical_layer_device::MAX_REG) | page,
                value,
            )?;
        }
        Ok(Some(size))
    })();
    f.release(hardware);
    match out {
        Ok(Some(pairs)) => hardware.note(Note::LanConnectedDeviceConfigured { pairs }),
        Ok(None) => hardware.note(Note::LanConnectedDeviceNotConfigured),
        Err(e) => return Err(e),
    }
    Ok(())
}

/// **`e1000_write_smbus_addr`**: the PHY's System Management Bus (`SMBus`) address, for the
/// I217-family PHY every I219 carries.
#[doc(alias = "smbus")]
fn write_system_management_bus_address(
    hardware: &mut impl Hardware,
    f: &Swflag,
) -> Result<(), Error> {
    let strap = hardware.read(regs::STRAP);
    let freq = ((strap & STRAP_SMT_FREQ_MASK) >> STRAP_SMT_FREQ_SHIFT) as u16;
    let address = ((strap & STRAP_SYSTEM_MANAGEMENT_BUS_ADDRESS_MASK)
        >> STRAP_SYSTEM_MANAGEMENT_BUS_ADDRESS_SHIFT) as u16;
    let mut v = physical_layer_device::read(hardware, f, r::SMB_ADDR)?;
    v = (v & !SMB_ADDR_MASK) | address | SMB_ADDR_PEC_EN | SMB_ADDR_VALID;
    if freq != 0 {
        let fq = freq - 1;
        v &= !SMB_ADDR_FREQ_MASK;
        v |= (fq & 1) << SMB_ADDR_FREQ_LOW_SHIFT;
        v |= (fq & 2) << (SMB_ADDR_FREQ_HIGH_SHIFT - 1);
    } else {
        hardware.note(Note::SystemManagementBusFrequencyUnsupported);
    }
    physical_layer_device::write(hardware, f, r::SMB_ADDR, v)?;
    Ok(())
}

/// **`e1000_oem_bits_config_ich8lan`** for D0: carry the MAC's gigabit-disable and LPLU settings
/// into the PHY's OEM bits and restart autonegotiation to apply them.
fn oem_bits(hardware: &mut impl Hardware) -> Result<(), Error> {
    let f = Swflag::acquire(hardware)?;
    let out = (|| {
        if hardware.read(regs::FEXTNVM) & FEXTNVM_SW_CONFIG_ICH8M == 0 {
            return Ok(());
        }
        let pc = hardware.read(regs::PHY_CTRL);
        let mut v = physical_layer_device::read(hardware, &f, r::OEM_BITS)?
            & !(OEM_BITS_GBE_DIS | OEM_BITS_LPLU);
        if pc & PHY_CTRL_GBE_DISABLE != 0 {
            v |= OEM_BITS_GBE_DIS;
        }
        if pc & PHY_CTRL_D0A_LPLU != 0 {
            v |= OEM_BITS_LPLU;
        }
        if !physical_layer_device::reset_blocked(hardware) {
            v |= OEM_BITS_RESTART_AN;
        }
        physical_layer_device::write(hardware, &f, r::OEM_BITS, v)
    })();
    f.release(hardware);
    Ok(out?)
}

// ---------------------------------------------------------------------------------------------
// Pass 3: hardware bits and the copper link.
// ---------------------------------------------------------------------------------------------

/// **`e1000_init_hw_ich8lan`**, the parts that are not already the kernel's (it programs the
/// receive address and clears the multicast table itself): `e1000_initialize_hw_bits_ich8lan`, the
/// link setup, the transmit write-back policy and relaxed-ordering disable. Statistics are not
/// cleared; nothing reads them.
///
/// Name: ratified 2026-10-26 (UTC), in the ruling on e1000e's names and the tree-wide ruling that
/// spells out `init` as `initialize`, which names it. Refused `init_hw` (`hw` abbreviates
/// hardware), `init_hardware` (`init` abbreviates initialize).
pub fn initialize_hardware(hardware: &mut impl Hardware) -> Result<(), Error> {
    // `e1000_initialize_hw_bits_ich8lan`. Bit 22 of CTRL_EXT, TXDCTL and the TARC bits are
    // unnamed in FreeBSD too; they are Intel's.
    set(hardware, mac::CTRL_EXT, 1 << 22 | CTRL_EXT_PHYPDEN);
    set(hardware, regs::TXDCTL0, 1 << 22);
    set(hardware, regs::TXDCTL1, 1 << 22);
    set(hardware, regs::TARC0, 1 << 23 | 1 << 24 | 1 << 26 | 1 << 27);
    let mut t1 = hardware.read(regs::TARC1);
    if hardware.read(mac::TCTL) & TCTL_MULR != 0 {
        t1 &= !(1 << 28);
    } else {
        t1 |= 1 << 28;
    }
    hardware.write(regs::TARC1, t1 | 1 << 24 | 1 << 26 | 1 << 30);
    // "work-around descriptor data corruption issue during nfs v2 udp traffic"
    set(hardware, mac::RFCTL, RFCTL_NFSW_DIS | RFCTL_NFSR_DIS);
    // "Enable ECC on Lynxpoint"
    set(hardware, regs::PBECCSTS, PBECCSTS_ECC_ENABLE);
    set(hardware, mac::CTRL, CTRL_MEHE);

    // `e1000_setup_link_ich8lan`: the copper link, unless firmware blocks the PHY.
    if !physical_layer_device::reset_blocked(hardware) {
        setup_copper_link(hardware)?;
    }

    for q in [regs::TXDCTL0, regs::TXDCTL1] {
        let t = hardware.read(q);
        let t = (t & !TXDCTL_WTHRESH) | TXDCTL_FULL_TX_DESC_WB;
        hardware.write(q, (t & !TXDCTL_PTHRESH) | TXDCTL_MAX_TX_DESC_PREFETCH);
    }
    set(hardware, mac::CTRL_EXT, CTRL_EXT_RO_DIS);
    Ok(())
}

/// **`e1000_setup_copper_link_pch_lpt`**: set link up with speed and duplex unforced, configure the
/// 82577-family PHY (`e1000_copper_link_setup_82577`: carrier sense on transmit, downshift, auto
/// MDI/MDI-X), advertise every speed, and restart autonegotiation
/// (`e1000_copper_link_autoneg`, `e1000_phy_setup_autoneg`).
///
/// Two departures. The pause bits are advertised clear, because this driver programs no flow
/// control watermarks (the module's `BUGS`); FreeBSD advertises full flow control. And
/// `e1000_set_master_slave_mode` is left out: FreeBSD's `em` runs it with `e1000_ms_hw_default`,
/// which writes `PHY_1000T_CTRL` back unchanged. FreeBSD's short wait for link afterwards is the
/// kernel's own `STATUS.LU` wait.
pub(crate) fn setup_copper_link(hardware: &mut impl Hardware) -> Result<(), Error> {
    let c = (hardware.read(mac::CTRL) | crate::CTRL_SLU) & !(CTRL_FRCSPD | CTRL_FRCDPX);
    hardware.write(mac::CTRL, c);
    let f = Swflag::acquire(hardware)?;
    let out = (|| {
        physical_layer_device::update(hardware, &f, r::I82577_CFG, |v| {
            v | I82577_CFG_ASSERT_CRS_ON_TX | I82577_CFG_ENABLE_DOWNSHIFT
        })?;
        physical_layer_device::update(hardware, &f, r::I82577_CTRL_2, |v| {
            (v & !I82577_PHY_CTRL2_MDIX_CFG_MASK) | I82577_PHY_CTRL2_AUTO_MDI_MDIX
        })?;
        physical_layer_device::update(hardware, &f, r::AUTONEG_ADV, |v| {
            (v & !(NWAY_AR_ASM_DIR | NWAY_AR_PAUSE))
                | NWAY_AR_10T_HD_CAPS
                | NWAY_AR_10T_FD_CAPS
                | NWAY_AR_100TX_HD_CAPS
                | NWAY_AR_100TX_FD_CAPS
        })?;
        physical_layer_device::update(hardware, &f, r::CTRL_1000T, |v| {
            (v & !CR_1000T_HD_CAPS) | CR_1000T_FD_CAPS
        })?;
        physical_layer_device::update(hardware, &f, r::CONTROL, |v| {
            v | MII_CR_AUTO_NEG_EN | MII_CR_RESTART_AUTO_NEG
        })
    })();
    f.release(hardware);
    Ok(out?)
}

/// **`em_initialize_transmit_unit`'s "SPT and KBL errata workarounds"**, after `TCTL` is written:
/// multiple outstanding requests on, `IOSFPC`'s receive threshold, and `TARC0`'s request count
/// (Intel's "i218-i219 Specification Update 1.5.4.5", cited by FreeBSD, not read here).
pub fn transmit_errata(hardware: &mut impl Hardware) {
    set(hardware, mac::TCTL, TCTL_MULR);
    set(hardware, regs::IOSFPC, RCTL_RDMTS_HEX);
    let t = hardware.read(regs::TARC0);
    hardware.write(
        regs::TARC0,
        (t & !TARC0_CB_MULTIQ_3_REQ) | TARC0_CB_MULTIQ_2_REQ,
    );
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::vec::Vec;

    use super::super::sim::{Sim, Start};
    use super::*;

    /// **Registers that hold what is written and do nothing else**: no self-clearing reset, no PHY
    /// behind `MDIC`, no flash, no `LCD power cycle done`. What each step does when the device
    /// never answers, which the simulated I219 (always answering) cannot show.
    #[derive(Default)]
    struct Inert {
        regs: HashMap<u64, u32>,
        reads: Vec<u64>,
        writes: Vec<(u64, u32)>,
        elapsed_us: u64,
        notes: Vec<Note>,
    }

    impl Hardware for Inert {
        fn read(&mut self, off: u64) -> u32 {
            self.reads.push(off);
            *self.regs.get(&off).unwrap_or(&0)
        }
        fn write(&mut self, off: u64, v: u32) {
            self.writes.push((off, v));
            self.regs.insert(off, v);
        }
        fn read16(&mut self, off: u64) -> u16 {
            self.read(off) as u16
        }
        fn delay_us(&mut self, us: u64) {
            self.elapsed_us += us;
        }
        fn pci_vendor_id(&mut self) -> u16 {
            0x8086
        }
        fn note(&mut self, n: Note) {
            self.notes.push(n);
        }
    }

    impl Inert {
        fn written(&self, off: u64) -> Vec<u32> {
            self.writes
                .iter()
                .filter(|w| w.0 == off)
                .map(|w| w.1)
                .collect()
        }
    }

    fn mac(s: &Sim, off: u64) -> u32 {
        *s.mac.get(&off).unwrap_or(&0)
    }

    /// **An ME that never finishes is given FreeBSD's whole 2.5 s** (250 waits of 10 ms over 251
    /// polls, none after the last, and no polls past them), and the request is left standing. One
    /// that is already done costs nothing and has its request released.
    #[test]
    fn the_me_gets_two_and_a_half_seconds_to_leave_ulp() {
        let mut hardware = Inert::default();
        hardware
            .regs
            .insert(regs::FWSM, ulp::FWSM_FW_VALID | ulp::FWSM_ULP_CFG_DONE);
        leave_ulp(&mut hardware);
        assert_eq!(hardware.elapsed_us, 2_500_000);
        // One read to ask whether there is an ME, then the 251 polls.
        assert_eq!(
            hardware.reads.iter().filter(|&&o| o == regs::FWSM).count(),
            1 + 251
        );
        assert_eq!(hardware.notes, [Note::UlpByMe { cleared: false }]);
        assert_eq!(hardware.regs[&regs::H2ME], ulp::H2ME_ENFORCE_SETTINGS);

        let mut hardware = Inert::default();
        hardware.regs.insert(regs::FWSM, ulp::FWSM_FW_VALID);
        leave_ulp(&mut hardware);
        assert_eq!(hardware.elapsed_us, 0);
        assert_eq!(hardware.notes, [Note::UlpByMe { cleared: true }]);
        assert_eq!(
            hardware.written(regs::H2ME),
            [ulp::H2ME_ENFORCE_SETTINGS, 0]
        );
    }

    /// **`LANPHYPC` is driven low and released, and nothing else in `CTRL` moves**: the override
    /// set with the value bit low, then the override cleared, from whatever `CTRL` held. The PHY
    /// configuration counter is set to 50 ms in `FEXTNVM3` with the register's other bits kept.
    /// The wait is 1 ms low, then 5 ms polls for `LPCD`, then 30 ms: 36 ms when the power cycle
    /// is reported at once, 136 ms when it never is.
    #[test]
    fn the_lanphypc_toggle_moves_only_its_own_bits_and_waits_for_lpcd() {
        let mut hardware = Inert::default();
        hardware.regs.insert(regs::FEXTNVM3, 0x0c00_00ff);
        // SLU, with the override and the value bit both left set by someone else.
        hardware.regs.insert(mac::CTRL, 0x0003_0040);
        toggle_lanphypc(&mut hardware);
        assert_eq!(hardware.regs[&regs::FEXTNVM3], 0x0800_00ff);
        assert_eq!(hardware.written(mac::CTRL), [0x0001_0040, 0x0000_0040]);
        assert_eq!(hardware.elapsed_us, 136_000, "LPCD never rose");

        let mut s = Sim::new(Start::Dead, true);
        toggle_lanphypc(&mut s);
        assert_eq!(s.lanphypc_toggles, 1);
        assert_eq!(s.elapsed_us, 36_000, "LPCD rose at the first poll");
    }

    /// **A configuration that never completes is waited for 150 ms after the first 10 and
    /// reported**, and so is a PHY reset that `PHYRA` does not confirm.
    #[test]
    fn a_configuration_that_never_completes_is_reported_after_its_wait() {
        let mut hardware = Inert::default();
        assert_eq!(
            config_done(&mut hardware),
            Err(Error::NonVolatileMemory(
                non_volatile_memory::Error::NoDescriptor
            ))
        );
        assert_eq!(
            hardware.notes,
            [
                Note::LanInitTimeout,
                Note::PhysicalLayerDeviceResetNotAsserted
            ]
        );
        // 10 ms, 1500 polls of 100 us, and the bank probe's one microsecond before its flash
        // cycle (which fails at once: there is no flash descriptor).
        assert_eq!(hardware.elapsed_us, 10_000 + 1500 * 100 + 1);

        // The simulated reset raises both, and nothing is reported.
        let mut s = Sim::new(Start::Healthy, true);
        global_reset(&mut s, 1000).unwrap();
        assert!(!s.notes.contains(&Note::LanInitTimeout));
        assert!(!s.notes.contains(&Note::PhysicalLayerDeviceResetNotAsserted));
        assert!(!s.notes.contains(&Note::MasterRequestsPending));
    }

    /// **Pending bus-master requests are waited for 80 ms, then reported, and the reset goes
    /// ahead**; a reset that never self-clears is `ResetTimeout`, and a retry after one still asks
    /// for the reset.
    #[test]
    fn stuck_master_requests_are_waited_for_and_the_reset_still_goes_ahead() {
        let mut hardware = Inert::default();
        hardware
            .regs
            .insert(regs::FWSM, physical_layer_device::FWSM_RSPCIPHY);
        hardware
            .regs
            .insert(mac::STATUS, reset::STATUS_GIO_MASTER_ENABLE);
        // The last reset never cleared.
        hardware.regs.insert(mac::CTRL, crate::CTRL_RST);
        assert_eq!(global_reset(&mut hardware, 0), Err(Error::ResetTimeout));
        assert_eq!(hardware.notes, [Note::MasterRequestsPending]);
        // 80 ms for the master requests, 10 ms after disabling receive and transmit, and 20 ms
        // after the reset.
        assert_eq!(hardware.elapsed_us, 80_000 + 10_000 + 20_000);
        let last = *hardware.written(mac::CTRL).last().unwrap();
        assert_ne!(last & crate::CTRL_RST, 0);
        assert_ne!(last & CTRL_PHY_RST, 0, "the PHY is reset with the MAC");
    }

    /// **After a PHY reset the host wakeup bit is cleared and nothing else in the port
    /// configuration**, and no extended configuration is replayed unless the NVM says software
    /// does it, whatever size the hardware reports.
    #[test]
    fn the_post_reset_clears_host_wakeup_and_replays_only_when_asked() {
        let mut s = Sim::new(Start::Healthy, true);
        s.physical_layer_device_registers
            .insert(r::PORT_GEN_CFG, 0x0011);
        s.mac.insert(regs::EXTCNF_CTRL, 0x40 << 16);
        s.mac.insert(regs::EXTCNF_SIZE, 3 << 16);
        global_reset(&mut s, 1000).unwrap();
        assert_eq!(s.physical_layer_device_registers[&r::PORT_GEN_CFG], 0x0001);
        assert!(s.notes.contains(&Note::LanConnectedDeviceNotConfigured));
        assert!(
            !s.notes
                .iter()
                .any(|n| matches!(n, Note::LanConnectedDeviceConfigured { .. }))
        );
    }

    /// **The `SMBus` address word takes its address and frequency from `STRAP`** and keeps only
    /// the bits it does not own: the old address is replaced, PEC and valid are set even if they
    /// already were, and the frequency (minus one) lands in bits 8 and 12. A frequency of zero is
    /// left alone and reported, as FreeBSD declines to program it.
    #[test]
    fn the_system_management_bus_address_comes_from_strap() {
        let strap = |address: u32, freq: u32| address << 17 | freq << 12;
        for (freq, preset, want) in [
            (2, 0x02d5, 0x0382),
            (3, 0x02d5, 0x1282),
            (0, 0x1155, 0x1382),
        ] {
            let mut s = Sim::new(Start::Healthy, true);
            s.mac.insert(regs::STRAP, strap(2, freq));
            s.physical_layer_device_registers
                .insert(r::SMB_ADDR, preset);
            let f = Swflag::acquire(&mut s).unwrap();
            write_system_management_bus_address(&mut s, &f).unwrap();
            f.release(&mut s);
            assert_eq!(
                s.physical_layer_device_registers[&r::SMB_ADDR],
                want,
                "frequency field {freq}"
            );
            assert_eq!(
                s.notes
                    .contains(&Note::SystemManagementBusFrequencyUnsupported),
                freq == 0
            );
        }
    }

    /// **The OEM bits carry the MAC's gigabit-disable and LPLU into the PHY**, clear them when the
    /// MAC does not ask, keep the PHY's other bits, and restart autonegotiation unless firmware
    /// blocks PHY resets. Without `SW_CONFIG` the PHY is not touched.
    #[test]
    fn the_oem_bits_follow_the_mac() {
        let sw = FEXTNVM_SW_CONFIG_ICH8M;
        let both = PHY_CTRL_GBE_DISABLE | PHY_CTRL_D0A_LPLU;
        // (FEXTNVM, PHY_CTRL, OEM_BITS before, firmware blocks resets, OEM_BITS after)
        for (fextnvm, pc, before, blocked, after) in [
            (sw, both, 0x0000, false, 0x0444),
            (sw | 1, 0, 0x8044, false, 0x8400),
            (0, both, 0x8044, false, 0x8044),
            (sw, 0, 0x0000, true, 0x0000),
        ] {
            let mut s = Sim::new(Start::Healthy, true);
            s.mac.insert(regs::FEXTNVM, fextnvm);
            s.mac.insert(regs::PHY_CTRL, pc);
            if blocked {
                s.mac.insert(regs::FWSM, 0);
            }
            s.physical_layer_device_registers
                .insert(r::OEM_BITS, before);
            oem_bits(&mut s).unwrap();
            assert_eq!(
                s.physical_layer_device_registers[&r::OEM_BITS],
                after,
                "{fextnvm:#x} {pc:#x} {before:#x}"
            );
        }
    }

    /// **`initialize_hardware` writes FreeBSD's values**, from a device that holds nothing and from
    /// one whose registers already hold some of the bits. The second case is what tells setting a
    /// bit from toggling it, and keeping a register's other bits from clearing them.
    #[test]
    fn initialize_hardware_writes_freebsds_values_from_any_starting_state() {
        let mut s = Sim::new(Start::Healthy, true);
        s.physical_layer_device_registers
            .insert(r::AUTONEG_ADV, 0x0c01);
        initialize_hardware(&mut s).unwrap();
        assert_eq!(
            mac(&s, mac::CTRL_EXT),
            0x0052_0000,
            "bit 22, PHYPDEN, RO_DIS"
        );
        assert_eq!(mac(&s, regs::TXDCTL0), 0x0141_001f);
        assert_eq!(mac(&s, regs::TXDCTL1), 0x0141_001f);
        assert_eq!(mac(&s, regs::TARC0), 0x0d80_0000);
        assert_eq!(mac(&s, regs::TARC1), 0x5500_0000, "MULR clear: bit 28 set");
        assert_eq!(mac(&s, mac::RFCTL), 0x00c0);
        assert_eq!(mac(&s, regs::PBECCSTS), 0x0001_0000);
        assert_eq!(mac(&s, mac::CTRL), 0x0008_0040, "MEHE and SLU");
        assert_eq!(s.physical_layer_device_registers[&r::I82577_CFG], 0x8c00);
        assert_eq!(s.physical_layer_device_registers[&r::I82577_CTRL_2], 0x0400);
        assert_eq!(s.physical_layer_device_registers[&r::AUTONEG_ADV], 0x01e1);
        assert_eq!(s.physical_layer_device_registers[&r::CTRL_1000T], 0x0200);
        assert_eq!(s.physical_layer_device_registers[&r::CONTROL], 0x1200);

        let mut s = Sim::new(Start::Healthy, true);
        s.mac.insert(mac::TCTL, TCTL_MULR);
        s.mac.insert(regs::TARC1, 0x5500_00ff);
        s.mac.insert(regs::TXDCTL0, 0x013f_ff3f);
        s.mac.insert(mac::RFCTL, 0x00c0);
        s.mac
            .insert(mac::CTRL, crate::CTRL_SLU | CTRL_FRCSPD | CTRL_FRCDPX);
        s.physical_layer_device_registers
            .insert(r::I82577_CFG, 0x8c01);
        s.physical_layer_device_registers
            .insert(r::I82577_CTRL_2, 0x0601);
        s.physical_layer_device_registers
            .insert(r::AUTONEG_ADV, 0x0de1);
        s.physical_layer_device_registers
            .insert(r::CTRL_1000T, 0x0300);
        s.physical_layer_device_registers.insert(r::CONTROL, 0x1200);
        initialize_hardware(&mut s).unwrap();
        assert_eq!(
            mac(&s, regs::TARC1),
            0x4500_00ff,
            "MULR set: bit 28 cleared"
        );
        assert_eq!(mac(&s, regs::TXDCTL0), 0x0141_ff1f);
        assert_eq!(mac(&s, mac::RFCTL), 0x00c0);
        assert_eq!(mac(&s, mac::CTRL), 0x0008_0040, "speed and duplex unforced");
        assert_eq!(s.physical_layer_device_registers[&r::I82577_CFG], 0x8c01);
        assert_eq!(s.physical_layer_device_registers[&r::I82577_CTRL_2], 0x0401);
        assert_eq!(s.physical_layer_device_registers[&r::AUTONEG_ADV], 0x01e1);
        assert_eq!(s.physical_layer_device_registers[&r::CTRL_1000T], 0x0200);
        assert_eq!(s.physical_layer_device_registers[&r::CONTROL], 0x1200);
    }

    /// **The SPT transmit errata**: `MULR` on, `IOSFPC`'s receive threshold, and `TARC0`'s
    /// request count set to two with the register's other bits kept.
    #[test]
    fn the_transmit_errata_set_freebsds_three_values() {
        let mut hardware = Inert::default();
        hardware.regs.insert(regs::TARC0, 0x3000_00ff);
        transmit_errata(&mut hardware);
        assert_eq!(hardware.regs[&mac::TCTL], TCTL_MULR);
        assert_eq!(hardware.regs[&regs::IOSFPC], 0x0001_0000);
        assert_eq!(hardware.regs[&regs::TARC0], 0x2000_00ff);
    }

    /// **Without an ME, every write to `CV_SMB_CTRL` clears only the force bit, and K1 and the
    /// ULP start bit are set even when already set.** The PHY is reset twice: once by the ULP exit
    /// and once by the attach that follows it.
    #[test]
    fn leaving_ulp_without_an_me_changes_only_the_bits_it_names() {
        let mut s = Sim::new(Start::InSystemManagementBus, false);
        s.physical_layer_device_registers
            .insert(r::SMB_CTRL, 0x0101);
        s.physical_layer_device_registers.insert(r::PM_CTRL, 0x4001);
        s.physical_layer_device_registers
            .insert(r::ULP_CONFIG1, 0x1ff5);
        assert!(initialize_physical_layer_device_workarounds(&mut s).is_ok());
        let to = |reg: u32| -> Vec<u16> {
            s.physical_layer_device_writes
                .iter()
                .filter(|w| w.0 == reg)
                .map(|w| w.1)
                .collect()
        };
        let smb = to(r::SMB_CTRL);
        assert!(
            !smb.is_empty() && smb.iter().all(|&v| v == 0x0100),
            "{smb:x?}"
        );
        assert_eq!(to(r::PM_CTRL), [0x4001]);
        assert_eq!(to(r::ULP_CONFIG1), [0x0281, 0x0281]);
        assert_eq!(s.physical_layer_device_resets, 2);
    }
}
