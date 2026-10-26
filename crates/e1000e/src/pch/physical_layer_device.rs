//! **Reaching the PHY: MDIO through `MDIC`, under the software/firmware semaphore.**
//!
//! [`read_mdic`] and [`write_mdic`] are the MDIO primitive the 82574L shares, so QEMU proves this
//! layer too. [`read()`] and [`write()`] are the paged access every PCH PHY register above page 0
//! needs, and each takes a [`Swflag`], which only [`Swflag::acquire`] makes. A page this port does
//! not carry (800, or 1 to 767) is [`Error::PageNotPorted`]. The FreeBSD sources are `e1000_phy.c`
//! (the MDIC and paged access) and `e1000_ich8lan.c` (the semaphore and the reset block). Two
//! divergences are deliberate. There is no MDIC retry, since FreeBSD retries only from Meteor Lake
//! on and every part this crate claims is Sunrise Point. Page 800 (host wakeup, with its own
//! protocol) and pages 1 to 767 (82577 and 82578 debug registers) are refused rather than ported;
//! only an NVM-supplied LCD configuration word could address them.
//!
//! Name: ratified 2026-10-26 (UTC), in the tree-wide ruling that spells out `phy` (the chip) as
//! `physical_layer_device`, IEEE 802.3's phrase. Refused `phy` (an abbreviation), `physical_layer`
//! (blurs the chip with the layer).

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

use super::Hardware;

/// Why a PHY access failed. FreeBSD collapses these into `-E1000_ERR_PHY`; they are kept apart
/// here because the bench boot prints them, and each points somewhere different.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// `MDIC.READY` never rose: nothing answered on the management bus.
    NotReady,
    /// `MDIC.ERROR`: the PHY, or the MAC on its behalf, refused the transaction.
    ///
    /// Name: ratified 2026-10-26 (UTC), in the ruling on e1000e's names. Refused
    /// `Mdi` (an abbreviation of management data interface, the MDI in `MDIC`).
    ManagementDataInterface,
    /// `MDIC` completed for a register other than the one asked for.
    WrongRegister,
    /// The register number is wider than MDIO's five bits.
    OutOfRange,
    /// Someone else's `EXTCNF_CTRL.SWFLAG` stayed set for [`SWFLAG_FREE_MS`].
    SwflagHeld,
    /// The flag would not stick for [`SWFLAG_TAKE_MS`]: firmware or hardware has the PHY.
    SwflagRefused,
    /// A page this port refuses (the module header).
    PageNotPorted,
}

/// `MDIC` register bits, from FreeBSD's `e1000_defines.h`.
pub mod mdic {
    /// The register field's shift.
    pub(crate) const REG_SHIFT: u32 = 16;
    /// The register field.
    pub(crate) const REG_MASK: u32 = 0x001f_0000;
    /// The PHY address field's shift.
    pub(crate) const PHY_SHIFT: u32 = 21;
    /// Read opcode.
    pub(crate) const OP_READ: u32 = 0x0800_0000;
    /// Write opcode.
    pub(crate) const OP_WRITE: u32 = 0x0400_0000;
    /// The transaction completed.
    pub const READY: u32 = 0x1000_0000;
    /// The transaction failed.
    pub const ERROR: u32 = 0x4000_0000;
    /// Polls before giving up: `E1000_GEN_POLL_TIMEOUT * 3`.
    pub(crate) const POLLS: u32 = 640 * 3;
    /// Between polls, in microseconds.
    pub const POLL_US: u64 = 50;
}

/// The highest MDIO register number (`MAX_PHY_REG_ADDRESS`).
pub(crate) const MAX_PHY_REG_ADDRESS: u32 = 0x1f;
/// Registers above this one sit behind a page select (`MAX_PHY_MULTI_PAGE_REG`).
pub(crate) const MAX_PHY_MULTI_PAGE_REG: u32 = 0xf;
/// The page-select register (`IGP01E1000_PHY_PAGE_SELECT`).
pub(crate) const IGP01E1000_PHY_PAGE_SELECT: u32 = 0x1f;
/// How a page and a register pack into one offset (`PHY_PAGE_SHIFT`).
pub(crate) const PHY_PAGE_SHIFT: u32 = 5;
/// The first page reached at PHY address 1 rather than 2 (`HV_INTC_FC_PAGE_START`).
pub(crate) const HV_INTC_FC_PAGE_START: u32 = 768;
/// The host wakeup page (`BM_WUC_PAGE`), refused.
pub(crate) const BM_WUC_PAGE: u32 = 800;

/// The highest page whose select value (`page << 5`) fits the 16-bit `MDIC` data field. FreeBSD
/// passes `page << IGP_PAGE_SHIFT` to `e1000_set_page_igp` as a `u16` and so silently truncates a
/// higher one; this port refuses it instead. Found while writing this module's Kani harness.
pub(crate) const MAXIMUM_PAGE: u32 = 0x7ff;

/// A page and a register packed into the one offset [`read()`] and [`write()`] take: FreeBSD's
/// `PHY_REG(page, reg)`.
///
/// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `regs` and `r` are spelled out
/// `registers`; no ruling covers this function. Refused `reg` (an abbreviation, as in FreeBSD's
/// `PHY_REG`).
#[doc(alias = "PHY_REG")]
pub const fn paged_offset(page: u32, register: u32) -> u32 {
    page << PHY_PAGE_SHIFT | (register & MAX_PHY_REG_ADDRESS)
}

/// The PHY's registers this crate addresses, from FreeBSD's `e1000_defines.h`, `e1000_phy.h` and
/// `e1000_ich8lan.h`.
///
/// Name: ratified 2026-10-26 (UTC), in the tree-wide ruling that turns every `regs` module into
/// `registers`, which names e1000e's one-letter `r`. The constants inside keep their sources'
/// names. Refused `r` (a letter, not a name).
pub mod registers {
    use super::paged_offset;
    /// MII control.
    ///
    /// Name: ratified 2026-10-26 (UTC), in the ruling that keeps a register or bit name copied from
    /// a cited source, strictly: FreeBSD's `PHY_CONTROL` (`e1000_defines.h`). Refused `CONTROL` (a
    /// shortened copy).
    pub const PHY_CONTROL: u32 = 0x00;
    /// PHY identifier, high word.
    pub(crate) const PHY_ID1: u32 = 0x02;
    /// PHY identifier, low word and revision.
    pub(crate) const PHY_ID2: u32 = 0x03;
    /// Autonegotiation advertisement.
    pub(crate) const PHY_AUTONEG_ADV: u32 = 0x04;
    /// 1000BASE-T control.
    pub(crate) const PHY_1000T_CTRL: u32 = 0x09;
    /// 82577-family configuration (`I82577_CFG_REG`).
    pub(crate) const I82577_CFG_REG: u32 = 22;
    /// 82577-family control 2 (`I82577_PHY_CTRL_2`).
    pub(crate) const I82577_PHY_CTRL_2: u32 = 18;
    /// Kumeran mode control (`HV_KMRN_MODE_CTRL`).
    ///
    /// Name: ratified 2026-10-26 (UTC), in the ruling that keeps a register or bit name copied from
    /// a cited source, strictly: FreeBSD's `HV_KMRN_MODE_CTRL` (`e1000_ich8lan.h`). Refused
    /// `KMRN_MODE_CTRL` (a shortened copy).
    pub const HV_KMRN_MODE_CTRL: u32 = paged_offset(769, 16);
    /// Port general configuration (`BM_PORT_GEN_CFG`).
    pub(crate) const BM_PORT_GEN_CFG: u32 = paged_offset(769, 17);
    /// `SMBus` control (`CV_SMB_CTRL`).
    pub(crate) const CV_SMB_CTRL: u32 = paged_offset(769, 23);
    /// Power management control (`HV_PM_CTRL`).
    pub(crate) const HV_PM_CTRL: u32 = paged_offset(770, 17);
    /// Ultra-low-power configuration 1 (`I218_ULP_CONFIG1`).
    pub(crate) const I218_ULP_CONFIG1: u32 = paged_offset(779, 16);
    /// OEM bits: LPLU, gigabit disable, restart autonegotiation (`HV_OEM_BITS`).
    pub(crate) const HV_OEM_BITS: u32 = paged_offset(768, 25);
    /// `SMBus` address (`HV_SMB_ADDR`).
    pub(crate) const HV_SMB_ADDR: u32 = paged_offset(768, 26);
    /// LED configuration (`HV_LED_CONFIG`).
    pub(crate) const HV_LED_CONFIG: u32 = paged_offset(768, 30);
}

/// `PHY_REVISION_MASK`: the identifier without its revision nibble.
pub(crate) const PHY_REVISION_MASK: u32 = 0xffff_fff0;

/// How long to wait for someone else's `SWFLAG` to clear, in ms (`PHY_CFG_TIMEOUT`).
pub(crate) const SWFLAG_FREE_MS: u32 = 100;
/// How long to wait for ours to stick, in ms (`SW_FLAG_TIMEOUT`).
pub(crate) const SWFLAG_TAKE_MS: u32 = 1000;
/// `EXTCNF_CTRL.SWFLAG`.
pub(crate) const EXTCNF_CTRL_SWFLAG: u32 = 0x0000_0020;
/// `FWSM.RSPCIPHY`: clear while firmware blocks a PHY reset.
pub(crate) const ICH_FWSM_RSPCIPHY: u32 = 0x0000_0040;

/// One MDIO read of `register` at PHY address `physical_layer_device_address`,
/// `e1000_read_phy_reg_mdic`.
pub fn read_mdic(
    hardware: &mut impl Hardware,
    physical_layer_device_address: u32,
    register: u32,
) -> Result<u16, Error> {
    if register > MAX_PHY_REG_ADDRESS {
        return Err(Error::OutOfRange);
    }
    hardware.write(
        super::registers::MDIC,
        register << mdic::REG_SHIFT
            | physical_layer_device_address << mdic::PHY_SHIFT
            | mdic::OP_READ,
    );
    complete(hardware, register).map(|v| v as u16)
}

/// One MDIO write, `e1000_write_phy_reg_mdic`.
pub(crate) fn write_mdic(
    hardware: &mut impl Hardware,
    physical_layer_device_address: u32,
    register: u32,
    data: u16,
) -> Result<(), Error> {
    if register > MAX_PHY_REG_ADDRESS {
        return Err(Error::OutOfRange);
    }
    hardware.write(
        super::registers::MDIC,
        u32::from(data)
            | register << mdic::REG_SHIFT
            | physical_layer_device_address << mdic::PHY_SHIFT
            | mdic::OP_WRITE,
    );
    complete(hardware, register).map(|_| ())
}

/// Poll `MDIC` for the transaction's end and judge it, the half both FreeBSD functions share.
fn complete(hardware: &mut impl Hardware, register: u32) -> Result<u32, Error> {
    let mut v = 0;
    for _ in 0..mdic::POLLS {
        hardware.delay_us(mdic::POLL_US);
        v = hardware.read(super::registers::MDIC);
        if v & mdic::READY != 0 {
            break;
        }
    }
    if v & mdic::READY == 0 {
        Err(Error::NotReady)
    } else if v & mdic::ERROR != 0 {
        Err(Error::ManagementDataInterface)
    } else if (v & mdic::REG_MASK) >> mdic::REG_SHIFT != register {
        Err(Error::WrongRegister)
    } else {
        Ok(v)
    }
}

/// **Proof of holding the software/firmware semaphore** over the PHY and some MAC registers.
/// Deliberately not `Clone`: [`Swflag::release`] consumes it.
#[derive(Debug)]
#[must_use = "a held SWFLAG locks the Management Engine out of the PHY until it is released"]
pub struct Swflag(());

impl Swflag {
    /// `e1000_acquire_swflag_ich8lan`: wait for the flag to be free, set it, and wait for it to
    /// read back set. On a refusal it clears the bit it wrote, as FreeBSD does.
    pub(crate) fn acquire(hardware: &mut impl Hardware) -> Result<Self, Error> {
        let mut ext = 0;
        let mut free = false;
        for _ in 0..SWFLAG_FREE_MS {
            ext = hardware.read(super::registers::EXTCNF_CTRL);
            if ext & EXTCNF_CTRL_SWFLAG == 0 {
                free = true;
                break;
            }
            hardware.delay_us(1000);
        }
        if !free {
            return Err(Error::SwflagHeld);
        }
        ext |= EXTCNF_CTRL_SWFLAG;
        hardware.write(super::registers::EXTCNF_CTRL, ext);
        for _ in 0..SWFLAG_TAKE_MS {
            ext = hardware.read(super::registers::EXTCNF_CTRL);
            if ext & EXTCNF_CTRL_SWFLAG != 0 {
                return Ok(Swflag(()));
            }
            hardware.delay_us(1000);
        }
        hardware.write(super::registers::EXTCNF_CTRL, ext & !EXTCNF_CTRL_SWFLAG);
        Err(Error::SwflagRefused)
    }

    /// `e1000_release_swflag_ich8lan`. Clears the bit only if it is still set; FreeBSD logs the
    /// other case ("Semaphore unexpectedly released") and so does nothing here.
    pub fn release(self, hardware: &mut impl Hardware) {
        let ext = hardware.read(super::registers::EXTCNF_CTRL);
        if ext & EXTCNF_CTRL_SWFLAG != 0 {
            hardware.write(super::registers::EXTCNF_CTRL, ext & !EXTCNF_CTRL_SWFLAG);
        }
    }

    /// **The flag is given up to a global reset**, which clears `EXTCNF_CTRL`. FreeBSD's
    /// `e1000_reset_hw_ich8lan` takes it before `CTRL.RST` and never releases it; this names that.
    pub(crate) fn lost_to_reset(self) {}
}

/// Which PHY address a page lives at (`e1000_get_phy_addr_for_hv_page`).
pub(crate) const fn address_for(page: u32) -> u32 {
    if page >= HV_INTC_FC_PAGE_START { 1 } else { 2 }
}

/// What one paged access does on the bus: the page-select write, if any, then the register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    /// `Some(value)` when the page-select register is written first (at PHY address 1).
    pub select: Option<u16>,
    /// The PHY address of the access itself.
    ///
    /// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `phy` (the chip) is spelled out
    /// `physical_layer_device`; no ruling covers the rest of the name. Refused `phy` (an
    /// abbreviation, and the field holds the PHY's address, not the PHY).
    pub physical_layer_device_address: u32,
    /// The five-bit register.
    ///
    /// Name: ratified 2026-10-26 (UTC), in the ruling on e1000e's names. Refused
    /// `reg` (an abbreviation).
    pub register: u32,
}

/// The decomposition `__e1000_read_phy_reg_hv` and `__e1000_write_phy_reg_hv` share, for the pages
/// this port carries. Pure, so it is host-tested and model-checked.
pub const fn plan(offset: u32) -> Result<Plan, Error> {
    let page = offset >> PHY_PAGE_SHIFT;
    let register = offset & MAX_PHY_REG_ADDRESS;
    if page == BM_WUC_PAGE || (page > 0 && page < HV_INTC_FC_PAGE_START) || page > MAXIMUM_PAGE {
        return Err(Error::PageNotPorted);
    }
    let physical_layer_device_address = address_for(page);
    // Page 768 is selected as page 0 (FreeBSD: "if (page == HV_INTC_FC_PAGE_START) page = 0").
    let shown = if page == HV_INTC_FC_PAGE_START {
        0
    } else {
        page
    };
    let select = if register > MAX_PHY_MULTI_PAGE_REG {
        Some((shown << PHY_PAGE_SHIFT) as u16)
    } else {
        None
    };
    Ok(Plan {
        select,
        physical_layer_device_address,
        register,
    })
}

/// Read a PCH PHY register by its paged offset ([`paged_offset`]).
pub fn read(hardware: &mut impl Hardware, _held: &Swflag, offset: u32) -> Result<u16, Error> {
    let p = plan(offset)?;
    if let Some(sel) = p.select {
        write_mdic(hardware, 1, IGP01E1000_PHY_PAGE_SELECT, sel)?;
    }
    read_mdic(hardware, p.physical_layer_device_address, p.register)
}

/// Write a PCH PHY register by its paged offset. FreeBSD's 82578 power-down workaround in the
/// write path is not carried: no part this crate claims has that PHY.
pub fn write(
    hardware: &mut impl Hardware,
    _held: &Swflag,
    offset: u32,
    data: u16,
) -> Result<(), Error> {
    let p = plan(offset)?;
    if let Some(sel) = p.select {
        write_mdic(hardware, 1, IGP01E1000_PHY_PAGE_SELECT, sel)?;
    }
    write_mdic(hardware, p.physical_layer_device_address, p.register, data)
}

/// Read-modify-write under a held flag.
pub fn update(
    hardware: &mut impl Hardware,
    held: &Swflag,
    offset: u32,
    change: impl FnOnce(u16) -> u16,
) -> Result<(), Error> {
    let v = read(hardware, held, offset)?;
    write(hardware, held, offset, change(v))
}

/// `e1000_check_reset_block_ich8lan`: is firmware blocking a PHY reset? Polls `FWSM.RSPCIPHY` up to
/// 31 times 10 ms apart, as FreeBSD does.
pub(crate) fn reset_blocked(hardware: &mut impl Hardware) -> bool {
    for i in 0..=30 {
        if hardware.read(super::registers::FWSM) & ICH_FWSM_RSPCIPHY != 0 {
            return false;
        }
        if i < 30 {
            hardware.delay_us(10_000);
        }
    }
    true
}

/// Read the PHY identifier at the 82574L's address, the generic MDIO path with no pages and no
/// semaphore (the 82574L has neither). What QEMU's gates prove of this module.
pub fn id_at(
    hardware: &mut impl Hardware,
    physical_layer_device_address: u32,
) -> Result<u32, Error> {
    let hi = read_mdic(hardware, physical_layer_device_address, registers::PHY_ID1)?;
    let lo = read_mdic(hardware, physical_layer_device_address, registers::PHY_ID2)?;
    Ok((u32::from(hi) << 16 | u32::from(lo)) & PHY_REVISION_MASK)
}

#[cfg(kani)]
mod proofs {
    use super::*;

    /// **Every paged access this port will make names a PHY address that exists (1 or 2), a
    /// five-bit register that is the offset's own, and a page select that is the offset's page**
    /// (768 shown as 0), written only when the register is above the unpaged range.
    ///
    /// Falsification: replayable `crates/e1000e/falsifications/pch.physical_layer_device.proofs.a_paged_access_selects_the_offsets_own_page_and_register.patch`
    #[kani::proof]
    fn a_paged_access_selects_the_offsets_own_page_and_register() {
        let off: u32 = kani::any();
        if let Ok(p) = plan(off) {
            let page = off >> PHY_PAGE_SHIFT;
            assert!(p.physical_layer_device_address == 1 || p.physical_layer_device_address == 2);
            assert!(p.register == off & MAX_PHY_REG_ADDRESS);
            assert!(page != BM_WUC_PAGE);
            match p.select {
                Some(sel) => {
                    assert!(p.register > MAX_PHY_MULTI_PAGE_REG);
                    let shown = if page == HV_INTC_FC_PAGE_START {
                        0
                    } else {
                        page
                    };
                    assert!(u32::from(sel) >> PHY_PAGE_SHIFT == shown);
                }
                None => assert!(p.register <= MAX_PHY_MULTI_PAGE_REG),
            }
        }
    }
}
