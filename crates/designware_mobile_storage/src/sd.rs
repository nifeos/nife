//! **Bringing a card up, and reading and writing its blocks.**
//!
//! The identification sequence is the SD Physical Layer Simplified Specification's figure 4-2
//! (version 9.10), with JEDEC JESD84's eMMC sequence beside it, in the order OpenBSD's
//! `sdmmc_mem_enable` and `sdmmc_mem_init` take them (`sys/dev/sdmmc/sdmmc_mem.c`, ISC; the crate
//! root carries the notice). Which kind of card is in the slot is decided by what answers:
//!
//! 1. `CMD0`, everything to idle, at 400 kHz.
//! 2. `CMD8` with the check pattern. An SD 2.0 card echoes it; an older SD card and an eMMC do
//!    not answer.
//! 3. `ACMD41` (`CMD55`, then `CMD41`) until the card reports power-up complete. An eMMC does not
//!    answer `CMD55`, which is how it is told from an SD 1.x card; it is then brought up with
//!    `CMD1` instead.
//! 4. `CMD2` for the CID, `CMD3` for the relative address (the card picks it on SD; the host
//!    assigns it on eMMC), `CMD9` for the CSD, `CMD7` to select.
//! 5. SD: `ACMD6` to 4-bit if the slot is wired for it, and `CMD16` to 512 bytes. eMMC: `CMD8`
//!    reads the 512-byte `EXT_CSD` for the true sector count; the bus stays 1-bit (BUGS).
//! 6. The clock goes up to 25 MHz, default speed. High speed and UHS modes are not attempted.
//!
//! Every step's failure names the step, so a bench transcript that stops says where.

use crate::card::{
    self, Cid, Csd, IF_COND_ARG, MMC_OP_COND_ARG, OCR_BUSY_DONE, OCR_HIGH_CAPACITY, R1_ERRORS,
    SD_OP_COND_ARG,
};
use crate::command::{self as c, Command, Data, Response};
use crate::host::{Error, Host, Registers};
use crate::idmac::DirectMemoryAccessRegion;

/// The identification clock, the spec's ceiling for it.
pub(crate) const IDENTIFY_HZ: u32 = 400_000;
/// The transfer clock: default speed, the spec's ceiling for it on SD and the legacy eMMC ceiling
/// (26 MHz) rounded down.
pub(crate) const TRANSFER_HZ: u32 = 25_000_000;
/// One block.
pub const BLOCK: usize = 512;
/// How long `ACMD41` or `CMD1` may report busy before the card is declared dead: the SD spec gives
/// a card one second from the first `ACMD41`.
pub(crate) const POWER_UP_DEADLINE_US: u64 = 1_000_000;
/// The relative address the host gives an eMMC.
pub(crate) const MMC_RCA: u16 = 1;

/// What kind of card answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// SD 1.x, standard capacity, byte addressed.
    SdV1,
    /// SD 2.0 or later, standard capacity, byte addressed.
    SdV2,
    /// SDHC, SDXC or SDUC: block addressed.
    SdHighCapacity,
    /// eMMC (or MMC). Block addressed when the OCR says sector mode.
    Mmc,
}

/// Which step of identification failed, and how.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitError {
    /// Reset, power or the identification clock.
    Controller(Error),
    /// `CMD0` failed.
    GoIdle(Error),
    /// `CMD8` answered with the wrong echo: what it said.
    InterfaceCondition(u32),
    /// Neither `ACMD41` nor `CMD1` was answered: no card, or a card that is not memory.
    NoCard(Error),
    /// The card stayed busy past [`POWER_UP_DEADLINE_US`]: the last OCR.
    PowerUp(u32),
    /// `CMD2`, `CMD3`, `CMD9` or `CMD7` failed: the command and the error.
    Identify(u8, Error),
    /// The CSD decoded to no capacity: its structure field.
    Capacity(u32),
    /// Bus width, block length, `EXT_CSD` or the transfer clock failed.
    Configure(u8, Error),
    /// A command's R1 status reported an error: the command and the status.
    Status(u8, u32),
}

/// A card that is selected and in the transfer state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Card {
    /// Which kind.
    pub kind: Kind,
    /// Its relative address.
    pub rca: u16,
    /// The OCR it reported at power-up.
    pub ocr: u32,
    /// The raw CID.
    pub cid_raw: [u32; 4],
    /// The raw CSD.
    pub csd_raw: [u32; 4],
    /// The CID, decoded.
    pub cid: Cid,
    /// Capacity in 512-byte blocks.
    pub blocks: u64,
    /// Addressed in blocks (true) or bytes (false).
    pub block_addressed: bool,
    /// The bus width in use.
    pub bus_width: u8,
    /// The card clock in use, in Hz.
    pub clock_hz: u32,
}

impl Card {
    /// The command argument that addresses block `lba`.
    #[must_use]
    pub const fn address(&self, lba: u64) -> u32 {
        if self.block_addressed {
            lba as u32
        } else {
            (lba * BLOCK as u64) as u32
        }
    }

    fn rca_arg(&self) -> u32 {
        u32::from(self.rca) << 16
    }
}

fn r1_ok(index: u8, r1: u32) -> Result<(), InitError> {
    if r1 & R1_ERRORS != 0 {
        Err(InitError::Status(index, r1))
    } else {
        Ok(())
    }
}

/// **Bring up whatever card is in the slot** and leave it selected, in the transfer state, at
/// [`TRANSFER_HZ`]. `max_bus_width` is the tree's `bus-width` for the slot (4 on radon's SD slot,
/// 8 on its eMMC slot).
///
/// # Errors
///
/// The step that failed; see [`InitError`].
pub fn identify<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
    max_bus_width: u8,
) -> Result<Card, InitError> {
    let v2 = go_idle_and_check_interface(host)?;
    let (kind, ocr) = power_up(host, v2)?;
    let (cid_raw, rca, csd_raw) = address_and_select(host, kind)?;

    let mut card = Card {
        kind,
        rca,
        ocr,
        cid_raw,
        csd_raw,
        cid: if kind == Kind::Mmc {
            Cid::mmc(&cid_raw)
        } else {
            Cid::sd(&cid_raw)
        },
        blocks: 0,
        block_addressed: match kind {
            Kind::SdHighCapacity => true,
            Kind::Mmc => ocr & OCR_HIGH_CAPACITY != 0,
            _ => false,
        },
        bus_width: 1,
        clock_hz: 0,
    };

    if kind == Kind::Mmc {
        configure_mmc(host, &mut card, &csd_raw)?;
    } else {
        configure_sd(host, &mut card, &csd_raw, max_bus_width)?;
    }
    if card.blocks == 0 {
        return Err(InitError::Capacity(Csd::sd(&csd_raw).structure));
    }
    Ok(card)
}

/// Steps 1 and 2: reset the controller at the identification clock, `CMD0`, then `CMD8`. Returns
/// whether the card echoed the check pattern (SD 2.0 or later).
fn go_idle_and_check_interface<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
) -> Result<bool, InitError> {
    host.reset().map_err(InitError::Controller)?;
    host.power_on();
    host.set_clock(IDENTIFY_HZ).map_err(InitError::Controller)?;
    host.set_bus_width(1);

    host.command(&Command::new(c::GO_IDLE_STATE, 0, Response::None))
        .map_err(InitError::GoIdle)?;

    let v2 = match host.command(&Command::new(c::SEND_IF_COND, IF_COND_ARG, Response::R7)) {
        Ok(r) if r[0] & 0xfff == IF_COND_ARG => true,
        Ok(r) => return Err(InitError::InterfaceCondition(r[0])),
        Err(Error::NoResponse(_)) => false,
        Err(e) => return Err(InitError::Identify(c::SEND_IF_COND, e)),
    };
    Ok(v2)
}

/// Step 3: `ACMD41` until the card is powered up, or `CMD1` for an eMMC. Returns which kind of card
/// answered and its OCR.
fn power_up<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
    v2: bool,
) -> Result<(Kind, u32), InitError> {
    let (kind, ocr) = match sd_power_up(host, v2) {
        Ok(ocr) => {
            let kind = if ocr & OCR_HIGH_CAPACITY != 0 {
                Kind::SdHighCapacity
            } else if v2 {
                Kind::SdV2
            } else {
                Kind::SdV1
            };
            (kind, ocr)
        }
        // No answer to CMD55: not an SD card. An eMMC, or nothing. CMD0 again first, since a
        // failed ACMD may have left a card confused about whether it is in application mode.
        Err(InitError::NoCard(_)) if !v2 => {
            host.command(&Command::new(c::GO_IDLE_STATE, 0, Response::None))
                .map_err(InitError::GoIdle)?;
            (Kind::Mmc, mmc_power_up(host)?)
        }
        Err(e) => return Err(e),
    };
    Ok((kind, ocr))
}

/// Step 4: `CMD2` for the CID, `CMD3` for the relative address, `CMD9` for the CSD, `CMD7` to
/// select. Returns the raw CID, the address and the raw CSD.
fn address_and_select<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
    kind: Kind,
) -> Result<([u32; 4], u16, [u32; 4]), InitError> {
    let cid_raw = host
        .command(&Command::new(c::ALL_SEND_CID, 0, Response::R2))
        .map_err(|e| InitError::Identify(c::ALL_SEND_CID, e))?;

    let rca = if kind == Kind::Mmc {
        let r1 = host
            .command(&Command::new(
                c::SEND_RELATIVE_ADDR,
                u32::from(MMC_RCA) << 16,
                Response::R1,
            ))
            .map_err(|e| InitError::Identify(c::SEND_RELATIVE_ADDR, e))?;
        r1_ok(c::SEND_RELATIVE_ADDR, r1[0])?;
        MMC_RCA
    } else {
        let r6 = host
            .command(&Command::new(c::SEND_RELATIVE_ADDR, 0, Response::R6))
            .map_err(|e| InitError::Identify(c::SEND_RELATIVE_ADDR, e))?;
        (r6[0] >> 16) as u16
    };

    let csd_raw = host
        .command(&Command::new(
            c::SEND_CSD,
            u32::from(rca) << 16,
            Response::R2,
        ))
        .map_err(|e| InitError::Identify(c::SEND_CSD, e))?;

    let r1 = host
        .command(&Command::new(
            c::SELECT_CARD,
            u32::from(rca) << 16,
            Response::R1b,
        ))
        .map_err(|e| InitError::Identify(c::SELECT_CARD, e))?;
    r1_ok(c::SELECT_CARD, r1[0])?;
    host.wait_not_busy()
        .map_err(|e| InitError::Identify(c::SELECT_CARD, e))?;
    Ok((cid_raw, rca, csd_raw))
}

/// Steps 5 and 6 for an eMMC: the transfer clock, then `EXT_CSD` for the true sector count.
fn configure_mmc<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
    card: &mut Card,
    csd_raw: &[u32; 4],
) -> Result<(), InitError> {
    let csd = Csd::mmc(csd_raw);
    card.blocks = csd.blocks;
    // The clock first: EXT_CSD is 512 bytes, and at 400 kHz on one line that is 10 ms.
    card.clock_hz = host
        .set_clock(TRANSFER_HZ)
        .map_err(|e| InitError::Configure(c::SEND_IF_COND, e))?;
    let mut ext = [0u8; BLOCK];
    let cmd = Command::with_data(
        c::SEND_IF_COND,
        0,
        Response::R1,
        Data {
            block_size: BLOCK as u32,
            blocks: 1,
            write: false,
        },
    );
    let r1 = host
        .read(&cmd, &mut ext)
        .map_err(|e| InitError::Configure(c::SEND_IF_COND, e))?;
    r1_ok(c::SEND_IF_COND, r1[0])?;
    // Above 2 GB the CSD's size is a placeholder; EXT_CSD's count is the truth.
    let sectors = card::ext_csd_sectors(&ext);
    if card.block_addressed && sectors != 0 {
        card.blocks = sectors;
    }
    Ok(())
}

/// Steps 5 and 6 for an SD card: the capacity from the CSD, `ACMD6` to 4-bit if the slot allows,
/// `CMD16` to 512 bytes, then the transfer clock.
fn configure_sd<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
    card: &mut Card,
    csd_raw: &[u32; 4],
    max_bus_width: u8,
) -> Result<(), InitError> {
    let csd = Csd::sd(csd_raw);
    if csd.blocks == 0 {
        return Err(InitError::Capacity(csd.structure));
    }
    card.blocks = csd.blocks;
    if max_bus_width >= 4 {
        app_command(host, card)?;
        let r1 = host
            .command(&Command::new(c::SD_SET_BUS_WIDTH, 2, Response::R1))
            .map_err(|e| InitError::Configure(c::SD_SET_BUS_WIDTH, e))?;
        r1_ok(c::SD_SET_BUS_WIDTH, r1[0])?;
        host.set_bus_width(4);
        card.bus_width = 4;
    }
    let r1 = host
        .command(&Command::new(c::SET_BLOCKLEN, BLOCK as u32, Response::R1))
        .map_err(|e| InitError::Configure(c::SET_BLOCKLEN, e))?;
    r1_ok(c::SET_BLOCKLEN, r1[0])?;
    card.clock_hz = host
        .set_clock(TRANSFER_HZ)
        .map_err(|e| InitError::Configure(c::SET_BLOCKLEN, e))?;
    Ok(())
}

fn app_command<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
    card: &Card,
) -> Result<(), InitError> {
    let r1 = host
        .command(&Command::new(c::APP_CMD, card.rca_arg(), Response::R1))
        .map_err(|e| InitError::Configure(c::APP_CMD, e))?;
    r1_ok(c::APP_CMD, r1[0])
}

fn sd_power_up<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
    v2: bool,
) -> Result<u32, InitError> {
    let arg = if v2 {
        SD_OP_COND_ARG
    } else {
        SD_OP_COND_ARG & !OCR_HIGH_CAPACITY
    };
    let start = host.registers().now_us();
    loop {
        host.command(&Command::new(c::APP_CMD, 0, Response::R1))
            .map_err(InitError::NoCard)?;
        let ocr = host
            .command(&Command::new(c::SD_SEND_OP_COND, arg, Response::R3))
            .map_err(InitError::NoCard)?[0];
        if ocr & OCR_BUSY_DONE != 0 {
            return Ok(ocr);
        }
        if host.registers().now_us().wrapping_sub(start) > POWER_UP_DEADLINE_US {
            return Err(InitError::PowerUp(ocr));
        }
    }
}

fn mmc_power_up<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
) -> Result<u32, InitError> {
    let start = host.registers().now_us();
    loop {
        let ocr = host
            .command(&Command::new(
                c::MMC_SEND_OP_COND,
                MMC_OP_COND_ARG,
                Response::R3,
            ))
            .map_err(InitError::NoCard)?[0];
        if ocr & OCR_BUSY_DONE != 0 {
            return Ok(ocr);
        }
        if host.registers().now_us().wrapping_sub(start) > POWER_UP_DEADLINE_US {
            return Err(InitError::PowerUp(ocr));
        }
    }
}

/// Why a block transfer was refused or failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoError {
    /// The buffer is empty or not a whole number of blocks.
    Length,
    /// The range runs past the card's end (or past what a 32-bit address reaches).
    Range,
    /// The controller or the card failed.
    Host(Error),
    /// The card's R1 status reported an error.
    Status(u32),
}

/// **Can `count` blocks from `lba` be addressed on `card`?** Inside the card, and (on a
/// byte-addressed card) inside the four gigabytes a 32-bit byte address reaches.
#[must_use]
pub const fn in_range(card_blocks: u64, block_addressed: bool, lba: u64, count: u64) -> bool {
    let Some(end) = lba.checked_add(count) else {
        return false;
    };
    if count == 0 || end > card_blocks {
        return false;
    }
    if block_addressed {
        end <= 1 << 32
    } else {
        end <= (1 << 32) / BLOCK as u64
    }
}

fn data_command(card: &Card, lba: u64, blocks: u32, write: bool) -> Command {
    let index = match (write, blocks) {
        (false, 1) => c::READ_SINGLE_BLOCK,
        (false, _) => c::READ_MULTIPLE_BLOCK,
        (true, 1) => c::WRITE_BLOCK,
        (true, _) => c::WRITE_MULTIPLE_BLOCK,
    };
    Command::with_data(
        index,
        card.address(lba),
        Response::R1,
        Data {
            block_size: BLOCK as u32,
            blocks,
            write,
        },
    )
}

fn check(len: usize, card: &Card, lba: u64) -> Result<u32, IoError> {
    if len == 0 || !len.is_multiple_of(BLOCK) {
        return Err(IoError::Length);
    }
    let blocks = (len / BLOCK) as u64;
    if blocks > u64::from(u32::MAX) / BLOCK as u64
        || !in_range(card.blocks, card.block_addressed, lba, blocks)
    {
        return Err(IoError::Range);
    }
    Ok(blocks as u32)
}

/// **Read whole blocks from `lba` into `buf`**, one command: `CMD17` for one block, `CMD18` with
/// the automatic stop for more.
///
/// # Errors
///
/// [`IoError::Length`] or [`IoError::Range`] before anything is sent; otherwise what the
/// controller or the card reported.
pub fn read_blocks<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
    card: &Card,
    lba: u64,
    buf: &mut [u8],
) -> Result<(), IoError> {
    let blocks = check(buf.len(), card, lba)?;
    let r1 = host
        .read(&data_command(card, lba, blocks, false), buf)
        .map_err(IoError::Host)?;
    if r1[0] & R1_ERRORS != 0 {
        return Err(IoError::Status(r1[0]));
    }
    Ok(())
}

/// **Write whole blocks from `buf` at `lba`**, one command: `CMD24` or `CMD25`, then wait for the
/// card to finish programming.
///
/// # Errors
///
/// As [`read_blocks`].
pub fn write_blocks<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
    card: &Card,
    lba: u64,
    buf: &[u8],
) -> Result<(), IoError> {
    let blocks = check(buf.len(), card, lba)?;
    let r1 = host
        .write(&data_command(card, lba, blocks, true), buf)
        .map_err(IoError::Host)?;
    if r1[0] & R1_ERRORS != 0 {
        return Err(IoError::Status(r1[0]));
    }
    // The card has the data; ask whether it took it (a write error is reported in the status
    // after the transfer, not in the R1 that started it).
    let status = host
        .command(&Command::new(c::SEND_STATUS, card.rca_arg(), Response::R1))
        .map_err(IoError::Host)?[0];
    if status & R1_ERRORS != 0 {
        return Err(IoError::Status(status));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(blocks: u64, block_addressed: bool) -> Card {
        Card {
            kind: Kind::SdHighCapacity,
            rca: 1,
            ocr: 0,
            cid_raw: [0; 4],
            csd_raw: [0; 4],
            cid: Cid::sd(&[0; 4]),
            blocks,
            block_addressed,
            bus_width: 4,
            clock_hz: TRANSFER_HZ,
        }
    }

    #[test]
    fn the_last_block_is_in_range_and_the_one_after_it_is_not() {
        assert!(in_range(10, true, 9, 1));
        assert!(!in_range(10, true, 9, 2));
        assert!(!in_range(10, true, 10, 1));
    }

    /// One command moves at most `u32::MAX / 512` blocks, because `BYTCNT` is a 32-bit byte count.
    /// `check` is given lengths it is never handed a buffer of, so nothing is allocated.
    #[test]
    fn one_command_moves_no_more_blocks_than_a_32_bit_byte_count_holds() {
        let huge = card(1 << 40, true);
        let most = (u32::MAX / BLOCK as u32) as usize;
        assert_eq!(most, 8_388_607);
        assert_eq!(check(most * BLOCK, &huge, 0), Ok(most as u32));
        assert_eq!(check((most + 1) * BLOCK, &huge, 0), Err(IoError::Range));
        assert_eq!(check((most + 2) * BLOCK, &huge, 0), Err(IoError::Range));
        assert_eq!(check(512 * BLOCK, &huge, 0), Ok(512));
    }
}
