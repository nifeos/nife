//! Bringing a card up, and reading and writing its blocks. Identification is the SD Physical Layer
//! Simplified Specification's figure 4-2 (version 9.10), with JEDEC JESD84's eMMC sequence beside
//! it, in the order OpenBSD's `sdmmc_mem_enable` and `sdmmc_mem_init` take them
//! (`sys/dev/sdmmc/sdmmc_mem.c`, ISC; the crate root carries the notice). What answers decides the
//! card: an SD 2.0 card echoes `CMD8`'s check pattern, and an eMMC does not answer `CMD55`, which
//! tells it from an SD 1.x card and brings it up with `CMD1`. The clock then goes from 400 kHz to
//! 25 MHz, default speed; high speed and UHS are not attempted, and an eMMC's bus stays 1-bit.
//! Every step's failure names the step, so a bench transcript that stops says where.

use crate::card::{
    self, CardIdentification, CardSpecificData, INTERFACE_CONDITION_ARGUMENT,
    MULTIMEDIA_CARD_OPERATION_CONDITIONS_ARGUMENT, OPERATION_CONDITIONS_BUSY_DONE,
    OPERATION_CONDITIONS_HIGH_CAPACITY, R1_ERRORS, SD_OPERATION_CONDITIONS_ARGUMENT,
};
use crate::command::{self as c, Command, Data, Response};
use crate::host::{Error, Host, Registers};
use crate::internal_direct_memory_access::DirectMemoryAccessRegion;

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
/// The relative card address (RCA) the host gives an eMMC.
#[doc(alias = "rca")]
pub(crate) const MULTIMEDIA_CARD_RELATIVE_CARD_ADDRESS: u16 = 1;

/// What kind of card answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// SD 1.x, standard capacity, byte addressed.
    SdV1,
    /// SD 2.0 or later, standard capacity, byte addressed.
    SdV2,
    /// SDHC, SDXC or SDUC: block addressed.
    SdHighCapacity,
    /// eMMC, an embedded multimedia card (MMC), or an MMC. Block addressed when the OCR says
    /// sector mode.
    #[doc(alias = "mmc")]
    MultimediaCard,
}

/// Which step of identification failed, and how.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): `init` meaning initialize is spelled out wherever
/// this tree authors it; was `InitError`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitializationError {
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
    /// Its relative card address (RCA).
    #[doc(alias = "rca")]
    pub relative_card_address: u16,
    /// The operation conditions register (OCR) it reported at power-up.
    #[doc(alias = "ocr")]
    pub operation_conditions: u32,
    /// The raw card identification register (CID).
    #[doc(alias = "cid_raw")]
    pub card_identification_raw: [u32; 4],
    /// The raw card-specific data register (CSD).
    #[doc(alias = "csd_raw")]
    pub card_specific_data_raw: [u32; 4],
    /// The CID, decoded.
    #[doc(alias = "cid")]
    pub card_identification: CardIdentification,
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
    /// The command argument that addresses block `logical_block_address`.
    #[must_use]
    pub const fn address(&self, logical_block_address: u64) -> u32 {
        if self.block_addressed {
            logical_block_address as u32
        } else {
            (logical_block_address * BLOCK as u64) as u32
        }
    }

    fn relative_card_address_argument(&self) -> u32 {
        u32::from(self.relative_card_address) << 16
    }
}

fn r1_ok(index: u8, r1: u32) -> Result<(), InitializationError> {
    if r1 & R1_ERRORS != 0 {
        Err(InitializationError::Status(index, r1))
    } else {
        Ok(())
    }
}

/// **Bring up whatever card is in the slot** and leave it selected, in the transfer state, at
/// [`TRANSFER_HZ`]. `maximum_bus_width` is the tree's `bus-width` for the slot (4 on radon's SD
/// slot, 8 on its eMMC slot).
///
/// # Errors
///
/// The step that failed; see [`InitializationError`].
pub fn identify<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
    maximum_bus_width: u8,
) -> Result<Card, InitializationError> {
    let v2 = go_idle_and_check_interface(host)?;
    let (kind, operation_conditions) = power_up(host, v2)?;
    let (card_identification_raw, relative_card_address, card_specific_data_raw) =
        address_and_select(host, kind)?;

    let mut card = Card {
        kind,
        relative_card_address,
        operation_conditions,
        card_identification_raw,
        card_specific_data_raw,
        card_identification: if kind == Kind::MultimediaCard {
            CardIdentification::multimedia_card(&card_identification_raw)
        } else {
            CardIdentification::sd(&card_identification_raw)
        },
        blocks: 0,
        block_addressed: match kind {
            Kind::SdHighCapacity => true,
            Kind::MultimediaCard => operation_conditions & OPERATION_CONDITIONS_HIGH_CAPACITY != 0,
            _ => false,
        },
        bus_width: 1,
        clock_hz: 0,
    };

    if kind == Kind::MultimediaCard {
        configure_multimedia_card(host, &mut card, &card_specific_data_raw)?;
    } else {
        configure_sd(host, &mut card, &card_specific_data_raw, maximum_bus_width)?;
    }
    if card.blocks == 0 {
        return Err(InitializationError::Capacity(
            CardSpecificData::sd(&card_specific_data_raw).structure,
        ));
    }
    Ok(card)
}

/// Steps 1 and 2: reset the controller at the identification clock, `CMD0`, then `CMD8`. Returns
/// whether the card echoed the check pattern (SD 2.0 or later).
fn go_idle_and_check_interface<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
) -> Result<bool, InitializationError> {
    host.reset().map_err(InitializationError::Controller)?;
    host.power_on();
    host.set_clock(IDENTIFY_HZ)
        .map_err(InitializationError::Controller)?;
    host.set_bus_width(1);

    host.command(&Command::new(c::GO_IDLE_STATE, 0, Response::None))
        .map_err(InitializationError::GoIdle)?;

    let v2 = match host.command(&Command::new(
        c::SEND_IF_COND,
        INTERFACE_CONDITION_ARGUMENT,
        Response::R7,
    )) {
        Ok(r) if r[0] & 0xfff == INTERFACE_CONDITION_ARGUMENT => true,
        Ok(r) => return Err(InitializationError::InterfaceCondition(r[0])),
        Err(Error::NoResponse(_)) => false,
        Err(e) => return Err(InitializationError::Identify(c::SEND_IF_COND, e)),
    };
    Ok(v2)
}

/// Step 3: `ACMD41` until the card is powered up, or `CMD1` for an eMMC. Returns which kind of card
/// answered and its OCR.
fn power_up<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
    v2: bool,
) -> Result<(Kind, u32), InitializationError> {
    let (kind, operation_conditions) = match sd_power_up(host, v2) {
        Ok(operation_conditions) => {
            let kind = if operation_conditions & OPERATION_CONDITIONS_HIGH_CAPACITY != 0 {
                Kind::SdHighCapacity
            } else if v2 {
                Kind::SdV2
            } else {
                Kind::SdV1
            };
            (kind, operation_conditions)
        }
        // No answer to CMD55: not an SD card. An eMMC, or nothing. CMD0 again first, since a
        // failed ACMD may have left a card confused about whether it is in application mode.
        Err(InitializationError::NoCard(_)) if !v2 => {
            host.command(&Command::new(c::GO_IDLE_STATE, 0, Response::None))
                .map_err(InitializationError::GoIdle)?;
            (Kind::MultimediaCard, multimedia_card_power_up(host)?)
        }
        Err(e) => return Err(e),
    };
    Ok((kind, operation_conditions))
}

/// Step 4: `CMD2` for the CID, `CMD3` for the relative address, `CMD9` for the CSD, `CMD7` to
/// select. Returns the raw CID, the address and the raw CSD.
fn address_and_select<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
    kind: Kind,
) -> Result<([u32; 4], u16, [u32; 4]), InitializationError> {
    let card_identification_raw = host
        .command(&Command::new(c::ALL_SEND_CID, 0, Response::R2))
        .map_err(|e| InitializationError::Identify(c::ALL_SEND_CID, e))?;

    let relative_card_address = if kind == Kind::MultimediaCard {
        let r1 = host
            .command(&Command::new(
                c::SEND_RELATIVE_ADDR,
                u32::from(MULTIMEDIA_CARD_RELATIVE_CARD_ADDRESS) << 16,
                Response::R1,
            ))
            .map_err(|e| InitializationError::Identify(c::SEND_RELATIVE_ADDR, e))?;
        r1_ok(c::SEND_RELATIVE_ADDR, r1[0])?;
        MULTIMEDIA_CARD_RELATIVE_CARD_ADDRESS
    } else {
        let r6 = host
            .command(&Command::new(c::SEND_RELATIVE_ADDR, 0, Response::R6))
            .map_err(|e| InitializationError::Identify(c::SEND_RELATIVE_ADDR, e))?;
        (r6[0] >> 16) as u16
    };

    let card_specific_data_raw = host
        .command(&Command::new(
            c::SEND_CSD,
            u32::from(relative_card_address) << 16,
            Response::R2,
        ))
        .map_err(|e| InitializationError::Identify(c::SEND_CSD, e))?;

    let r1 = host
        .command(&Command::new(
            c::SELECT_CARD,
            u32::from(relative_card_address) << 16,
            Response::R1b,
        ))
        .map_err(|e| InitializationError::Identify(c::SELECT_CARD, e))?;
    r1_ok(c::SELECT_CARD, r1[0])?;
    host.wait_not_busy()
        .map_err(|e| InitializationError::Identify(c::SELECT_CARD, e))?;
    Ok((
        card_identification_raw,
        relative_card_address,
        card_specific_data_raw,
    ))
}

/// Steps 5 and 6 for an eMMC: the transfer clock, then `EXT_CSD` for the true sector count.
fn configure_multimedia_card<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
    card: &mut Card,
    card_specific_data_raw: &[u32; 4],
) -> Result<(), InitializationError> {
    let card_specific_data = CardSpecificData::multimedia_card(card_specific_data_raw);
    card.blocks = card_specific_data.blocks;
    // The clock first: EXT_CSD is 512 bytes, and at 400 kHz on one line that is 10 ms.
    card.clock_hz = host
        .set_clock(TRANSFER_HZ)
        .map_err(|e| InitializationError::Configure(c::SEND_IF_COND, e))?;
    let mut ext = [0u8; BLOCK];
    let command = Command::with_data(
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
        .read(&command, &mut ext)
        .map_err(|e| InitializationError::Configure(c::SEND_IF_COND, e))?;
    r1_ok(c::SEND_IF_COND, r1[0])?;
    // Above 2 GB the CSD's size is a placeholder; EXT_CSD's count is the truth.
    let sectors = card::extended_card_specific_data_sectors(&ext);
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
    card_specific_data_raw: &[u32; 4],
    maximum_bus_width: u8,
) -> Result<(), InitializationError> {
    let card_specific_data = CardSpecificData::sd(card_specific_data_raw);
    if card_specific_data.blocks == 0 {
        return Err(InitializationError::Capacity(card_specific_data.structure));
    }
    card.blocks = card_specific_data.blocks;
    if maximum_bus_width >= 4 {
        app_command(host, card)?;
        let r1 = host
            .command(&Command::new(c::SET_BUS_WIDTH, 2, Response::R1))
            .map_err(|e| InitializationError::Configure(c::SET_BUS_WIDTH, e))?;
        r1_ok(c::SET_BUS_WIDTH, r1[0])?;
        host.set_bus_width(4);
        card.bus_width = 4;
    }
    let r1 = host
        .command(&Command::new(c::SET_BLOCKLEN, BLOCK as u32, Response::R1))
        .map_err(|e| InitializationError::Configure(c::SET_BLOCKLEN, e))?;
    r1_ok(c::SET_BLOCKLEN, r1[0])?;
    card.clock_hz = host
        .set_clock(TRANSFER_HZ)
        .map_err(|e| InitializationError::Configure(c::SET_BLOCKLEN, e))?;
    Ok(())
}

fn app_command<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
    card: &Card,
) -> Result<(), InitializationError> {
    let r1 = host
        .command(&Command::new(
            c::APP_CMD,
            card.relative_card_address_argument(),
            Response::R1,
        ))
        .map_err(|e| InitializationError::Configure(c::APP_CMD, e))?;
    r1_ok(c::APP_CMD, r1[0])
}

fn sd_power_up<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
    v2: bool,
) -> Result<u32, InitializationError> {
    let argument = if v2 {
        SD_OPERATION_CONDITIONS_ARGUMENT
    } else {
        SD_OPERATION_CONDITIONS_ARGUMENT & !OPERATION_CONDITIONS_HIGH_CAPACITY
    };
    let start = host.registers().now_us();
    loop {
        host.command(&Command::new(c::APP_CMD, 0, Response::R1))
            .map_err(InitializationError::NoCard)?;
        let operation_conditions = host
            .command(&Command::new(c::SD_SEND_OP_COND, argument, Response::R3))
            .map_err(InitializationError::NoCard)?[0];
        if operation_conditions & OPERATION_CONDITIONS_BUSY_DONE != 0 {
            return Ok(operation_conditions);
        }
        if host.registers().now_us().wrapping_sub(start) > POWER_UP_DEADLINE_US {
            return Err(InitializationError::PowerUp(operation_conditions));
        }
    }
}

fn multimedia_card_power_up<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
) -> Result<u32, InitializationError> {
    let start = host.registers().now_us();
    loop {
        let operation_conditions = host
            .command(&Command::new(
                c::SEND_OP_COND,
                MULTIMEDIA_CARD_OPERATION_CONDITIONS_ARGUMENT,
                Response::R3,
            ))
            .map_err(InitializationError::NoCard)?[0];
        if operation_conditions & OPERATION_CONDITIONS_BUSY_DONE != 0 {
            return Ok(operation_conditions);
        }
        if host.registers().now_us().wrapping_sub(start) > POWER_UP_DEADLINE_US {
            return Err(InitializationError::PowerUp(operation_conditions));
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

/// **Can `count` blocks from `logical_block_address` be addressed on `card`?** Inside the card, and
/// (on a byte-addressed card) inside the four gigabytes a 32-bit byte address reaches.
#[must_use]
pub const fn in_range(
    card_blocks: u64,
    block_addressed: bool,
    logical_block_address: u64,
    count: u64,
) -> bool {
    let Some(end) = logical_block_address.checked_add(count) else {
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

fn data_command(card: &Card, logical_block_address: u64, blocks: u32, write: bool) -> Command {
    let index = match (write, blocks) {
        (false, 1) => c::READ_SINGLE_BLOCK,
        (false, _) => c::READ_MULTIPLE_BLOCK,
        (true, 1) => c::WRITE_BLOCK,
        (true, _) => c::WRITE_MULTIPLE_BLOCK,
    };
    Command::with_data(
        index,
        card.address(logical_block_address),
        Response::R1,
        Data {
            block_size: BLOCK as u32,
            blocks,
            write,
        },
    )
}

fn check(len: usize, card: &Card, logical_block_address: u64) -> Result<u32, IoError> {
    if len == 0 || !len.is_multiple_of(BLOCK) {
        return Err(IoError::Length);
    }
    let blocks = (len / BLOCK) as u64;
    if blocks > u64::from(u32::MAX) / BLOCK as u64
        || !in_range(
            card.blocks,
            card.block_addressed,
            logical_block_address,
            blocks,
        )
    {
        return Err(IoError::Range);
    }
    Ok(blocks as u32)
}

/// **Read whole blocks from `logical_block_address` into `buffer`**, one command: `CMD17` for one
/// block, `CMD18` with the automatic stop for more.
///
/// # Errors
///
/// [`IoError::Length`] or [`IoError::Range`] before anything is sent; otherwise what the
/// controller or the card reported.
pub fn read_blocks<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
    card: &Card,
    logical_block_address: u64,
    buffer: &mut [u8],
) -> Result<(), IoError> {
    let blocks = check(buffer.len(), card, logical_block_address)?;
    let r1 = host
        .read(
            &data_command(card, logical_block_address, blocks, false),
            buffer,
        )
        .map_err(IoError::Host)?;
    if r1[0] & R1_ERRORS != 0 {
        return Err(IoError::Status(r1[0]));
    }
    Ok(())
}

/// **Write whole blocks from `buffer` at `logical_block_address`**, one command: `CMD24` or
/// `CMD25`, then wait for the card to finish programming.
///
/// # Errors
///
/// As [`read_blocks`].
pub fn write_blocks<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
    card: &Card,
    logical_block_address: u64,
    buffer: &[u8],
) -> Result<(), IoError> {
    let blocks = check(buffer.len(), card, logical_block_address)?;
    let r1 = host
        .write(
            &data_command(card, logical_block_address, blocks, true),
            buffer,
        )
        .map_err(IoError::Host)?;
    if r1[0] & R1_ERRORS != 0 {
        return Err(IoError::Status(r1[0]));
    }
    // The card has the data; ask whether it took it (a write error is reported in the status
    // after the transfer, not in the R1 that started it).
    let status = host
        .command(&Command::new(
            c::SEND_STATUS,
            card.relative_card_address_argument(),
            Response::R1,
        ))
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
            relative_card_address: 1,
            operation_conditions: 0,
            card_identification_raw: [0; 4],
            card_specific_data_raw: [0; 4],
            card_identification: CardIdentification::sd(&[0; 4]),
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
