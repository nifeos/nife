//! **Reading the I219's NVM on Sunrise Point**, where the `GbE` flash region is reached through
//! registers in BAR0 rather than the chipset's SPI controller.
//!
//! [`super::sequence`] needs it for the post-reset LCD configuration and the NVM-present check.
//! Read only: nothing here can write or erase the flash. Each item names the SPT branch of
//! FreeBSD's `e1000_ich8lan.c` it follows.
//!
//! Name: ratified 2026-10-26 (UTC), in the ruling on e1000e's names. Refused `nvm`
//! (an abbreviation).

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

/// Why an NVM read failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// `HSFSTS.FLDESVALID` is clear: no flash descriptor, so hardware sequencing is unavailable.
    NoDescriptor,
    /// A previous flash cycle never finished.
    Busy,
    /// The cycle did not complete, or completed with `FLCERR`, after every retry.
    Cycle,
    /// The word is outside the shadow RAM's 2048 words.
    OutOfRange,
    /// Neither bank carries the valid signature.
    NoValidBank,
}

/// Where the flash registers sit in BAR0 on SPT (`E1000_FLASH_BASE_ADDR`).
pub(crate) const FLASH_BASE: u64 = 0xe000;
/// Hardware sequencing flash status, low half; control, high half (`ICH_FLASH_HSFSTS`).
pub(crate) const HSFSTS: u64 = 0x04;
/// Flash linear address (`ICH_FLASH_FADDR`).
pub(crate) const FADDR: u64 = 0x08;
/// Flash data 0 (`ICH_FLASH_FDATA0`).
pub(crate) const FDATA0: u64 = 0x10;

/// `HSFSTS` bits (`union ich8_hws_flash_status`).
pub mod status {
    /// Flash cycle done.
    pub const DONE: u16 = 1 << 0;
    /// Flash cycle error.
    pub const ERROR: u16 = 1 << 1;
    /// Direct access error log.
    pub(crate) const DIRECT_ACCESS_ERROR: u16 = 1 << 2;
    /// A cycle is in progress.
    pub(crate) const IN_PROGRESS: u16 = 1 << 5;
    /// The flash descriptor is valid.
    pub(crate) const DESCRIPTOR_VALID: u16 = 1 << 14;
}

/// `HSFCTL` bits (`union ich8_hws_flash_ctrl`), in the high half of the `u32` at [`HSFSTS`].
pub mod control {
    /// Start the cycle.
    pub const GO: u16 = 1 << 0;
    /// The cycle-type field; zero is a read (`ICH_CYCLE_READ`).
    pub(crate) const CYCLE_MASK: u16 = 0b11 << 1;
    /// The byte-count field, bits 9:8, holding the count minus one.
    pub(crate) const BYTE_COUNT_SHIFT: u16 = 8;
    /// Its mask.
    pub(crate) const BYTE_COUNT_MASK: u16 = 0b11 << 8;
}

/// `ICH_FLASH_LINEAR_ADDR_MASK`.
pub(crate) const LINEAR_ADDRESS_MASK: u32 = 0x00ff_ffff;
/// `ICH_FLASH_READ_COMMAND_TIMEOUT`, in 1 µs polls: ten seconds, FreeBSD's bound.
pub(crate) const COMMAND_TIMEOUT: u32 = 10_000_000;
/// `ICH_FLASH_CYCLE_REPEAT_COUNT`.
pub(crate) const CYCLE_REPEATS: u32 = 10;
/// The signature word (`E1000_ICH_NVM_SIG_WORD`).
pub(crate) const SIGNATURE_WORD: u32 = 0x13;
/// `E1000_ICH_NVM_VALID_SIG_MASK`.
pub(crate) const SIGNATURE_MASK: u8 = 0xc0;
/// `E1000_ICH_NVM_SIG_VALUE`.
pub(crate) const SIGNATURE_VALUE: u8 = 0x80;
/// `E1000_SHADOW_RAM_WORDS`.
pub const WORDS: u32 = 2048;
/// `NVM_SIZE_MULTIPLIER`.
pub(crate) const SIZE_MULTIPLIER: u32 = 4096;

/// The NVM's size, from `STRAP` on SPT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Geometry {
    /// One bank's size in 16-bit words.
    pub bank_words: u32,
}

impl Geometry {
    /// `e1000_init_nvm_params_ich8lan`'s SPT branch: `STRAP` bits 5:1 hold the size in 4 KiB
    /// units, minus one; the NVM is two banks.
    pub(crate) const fn from_strap(strap: u32) -> Self {
        let bytes = ((strap >> 1 & 0x1f) + 1) * SIZE_MULTIPLIER;
        Geometry {
            bank_words: bytes / 2 / 2,
        }
    }
    /// Read it from the device.
    pub fn read(hardware: &mut impl Hardware) -> Self {
        Self::from_strap(hardware.read(super::regs::STRAP))
    }
}

fn status_16_bit(hardware: &mut impl Hardware) -> u16 {
    hardware.read16(FLASH_BASE + HSFSTS)
}

/// `e1000_flash_cycle_init_ich8lan`, SPT's 32-bit writes.
fn initialize_cycle(hardware: &mut impl Hardware) -> Result<(), Error> {
    let s = status_16_bit(hardware);
    if s & status::DESCRIPTOR_VALID == 0 {
        return Err(Error::NoDescriptor);
    }
    // Clear the cycle and direct-access errors by writing them as one.
    let s = s | status::ERROR | status::DIRECT_ACCESS_ERROR;
    hardware.write(FLASH_BASE + HSFSTS, u32::from(s));
    if s & status::IN_PROGRESS == 0 {
        hardware.write(FLASH_BASE + HSFSTS, u32::from(s | status::DONE));
        return Ok(());
    }
    for _ in 0..COMMAND_TIMEOUT {
        let now = status_16_bit(hardware);
        if now & status::IN_PROGRESS == 0 {
            hardware.write(FLASH_BASE + HSFSTS, u32::from(now | status::DONE));
            return Ok(());
        }
        hardware.delay_us(1);
    }
    Err(Error::Busy)
}

/// `e1000_flash_cycle_ich8lan`: set GO and wait for DONE.
fn cycle(hardware: &mut impl Hardware) -> Result<(), Error> {
    let ctl = (hardware.read(FLASH_BASE + HSFSTS) >> 16) as u16 | control::GO;
    hardware.write(FLASH_BASE + HSFSTS, u32::from(ctl) << 16);
    let mut s = 0;
    for _ in 0..=COMMAND_TIMEOUT {
        s = status_16_bit(hardware);
        if s & status::DONE != 0 {
            break;
        }
        hardware.delay_us(1);
    }
    if s & status::DONE != 0 && s & status::ERROR == 0 {
        Ok(())
    } else {
        Err(Error::Cycle)
    }
}

/// `e1000_read_flash_data32_ich8lan`: one `u32` (a DWORD, in Intel's word sizes) at byte offset
/// `byte`.
#[doc(alias = "dword")]
pub(crate) fn read_u32_at_byte(hardware: &mut impl Hardware, byte: u32) -> Result<u32, Error> {
    if byte > LINEAR_ADDRESS_MASK {
        return Err(Error::OutOfRange);
    }
    let mut last = Err(Error::Cycle);
    for _ in 0..=CYCLE_REPEATS {
        hardware.delay_us(1);
        initialize_cycle(hardware)?;
        let mut ctl = (hardware.read(FLASH_BASE + HSFSTS) >> 16) as u16;
        ctl = (ctl & !control::BYTE_COUNT_MASK) | (3 << control::BYTE_COUNT_SHIFT);
        ctl &= !control::CYCLE_MASK;
        hardware.write(FLASH_BASE + HSFSTS, u32::from(ctl) << 16);
        hardware.write(FLASH_BASE + FADDR, byte & LINEAR_ADDRESS_MASK);
        last = cycle(hardware);
        if last.is_ok() {
            return Ok(hardware.read(FLASH_BASE + FDATA0));
        }
        let s = status_16_bit(hardware);
        if s & status::ERROR != 0 {
            continue;
        }
        if s & status::DONE == 0 {
            break;
        }
    }
    last.map(|()| 0)
}

/// `e1000_read_flash_dword_ich8lan`: the `u32` (Intel's DWORD) at 16-bit word offset `word`.
#[doc(alias = "dword")]
pub(crate) fn read_u32(hardware: &mut impl Hardware, word: u32) -> Result<u32, Error> {
    read_u32_at_byte(hardware, word << 1)
}

/// `e1000_valid_nvm_bank_detect_ich8lan`'s SPT branch: which bank carries the signature.
pub(crate) fn valid_bank(hardware: &mut impl Hardware, g: Geometry) -> Result<u32, Error> {
    for bank in 0..2 {
        let d = read_u32(hardware, SIGNATURE_WORD + bank * g.bank_words)?;
        if (d >> 8) as u8 & SIGNATURE_MASK == SIGNATURE_VALUE {
            return Ok(bank);
        }
    }
    Err(Error::NoValidBank)
}

/// `e1000_read_nvm_spt` for one word: find the valid bank (bank 0 if none, as FreeBSD assumes),
/// read the `u32` holding the word, and take its half.
pub(crate) fn read_word(
    hardware: &mut impl Hardware,
    g: Geometry,
    word: u32,
) -> Result<u16, Error> {
    if word >= WORDS {
        return Err(Error::OutOfRange);
    }
    let bank = valid_bank(hardware, g).unwrap_or(0);
    let at = bank * g.bank_words + word;
    let d = read_u32(hardware, at - at % 2)?;
    Ok(if at.is_multiple_of(2) {
        d as u16
    } else {
        (d >> 16) as u16
    })
}
