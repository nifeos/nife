//! Bringing a card up, and reading and writing its blocks. Identification is the SD Physical Layer
//! Simplified Specification's figure 4-2 (version 9.10), with JEDEC JESD84's eMMC sequence beside
//! it. Only the order of the addressing commands (`CMD2`, `CMD3`, `CMD9`, then `CMD7`) follows
//! OpenBSD's `sdmmc_mem_scan` and `sdmmc_mem_init` (`sys/dev/sdmmc/sdmmc_mem.c`, ISC); no code is
//! adapted from it. The power-up differs: OpenBSD asks the OCR first with an empty `ACMD41` and
//! sends `CMD8` after a second `CMD0`, and this module sends `CMD8` first, as figure 4-2 does. What
//! answers decides the
//! card: an SD 2.0 card echoes `CMD8`'s check pattern, and an eMMC does not answer `CMD55`, which
//! tells it from an SD 1.x card and brings it up with `CMD1`. The clock then goes from 400 kHz to
//! 25 MHz, default speed; high speed and UHS are not attempted, and an eMMC's bus stays 1-bit.
//! Every step's failure names the step, so a board transcript that stops says where.
//!
//! Name: ratified 2026-11-03 (UTC), #3135. `sd` is an accepted acronym, like `MAC` and `GMAC`
//! (#3094): Secure Digital, the card family this module brings up.

use crate::card::{
    self, CardIdentification, CardSpecificData, INTERFACE_CONDITION_ARGUMENT,
    MULTIMEDIA_CARD_OPERATION_CONDITIONS_ARGUMENT, OPERATION_CONDITIONS_BUSY_DONE,
    OPERATION_CONDITIONS_HIGH_CAPACITY, R1_ERROR_MASK, SD_OPERATION_CONDITIONS_ARGUMENT,
};
use crate::command::{self, Command, Data, Response};
use crate::host_controller::{Error, HostController, RegisterWindowAndClock};
use crate::internal_direct_memory_access::DirectMemoryAccessRegion;

/// The identification clock, the spec's ceiling for it, in hertz.
pub(crate) const IDENTIFY_HERTZ: u32 = 400_000;
/// The transfer clock: default speed, the spec's ceiling for it on SD and the legacy eMMC ceiling
/// (26 MHz) rounded down.
///
/// Name: ratified 2026-11-05 (UTC), #3276. `HERTZ` spells the unit out; the symbol is not a valid
/// suffix.
pub const TRANSFER_HERTZ: u32 = 25_000_000;
/// One block, in bytes.
///
/// Name: ratified 2026-11-03 (UTC), #3139. Renamed from `BLOCK`, which
/// did not say its unit; the suffix is the one `TRANSFER_HERTZ` uses.
pub const BLOCK_BYTES: usize = 512;
/// How long `ACMD41` or `CMD1` may report busy before the card is declared dead: the SD spec gives
/// a card one second from the first `ACMD41`.
///
/// Name: ratified 2026-11-02 (UTC), #3092. The unit is spelled out.
pub const POWER_UP_DEADLINE_MICROSECONDS: u64 = 1_000_000;
/// The relative card address the host assigns an eMMC with `CMD3`. An SD card proposes its own
/// instead.
#[doc(alias = "rca")]
pub(crate) const MULTIMEDIA_CARD_RELATIVE_CARD_ADDRESS: u16 = 1;

/// What kind of card answered.
///
/// Name: ratified 2026-11-03 (UTC), #3139. Renamed from `Kind`, which
/// said nothing outside its module; the name now says it is the kind of a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CardKind {
    /// SD 1.x, standard capacity, byte addressed.
    SdV1,
    /// SD 2.0 or later, standard capacity, byte addressed.
    SdV2,
    /// SDHC, SDXC or SDUC: block addressed.
    SdHighCapacity,
    /// An eMMC (embedded multimedia card) or an MMC (multimedia card). Block addressed when the OCR
    /// says sector mode.
    #[doc(alias = "mmc")]
    MultimediaCard,
}

/// Which step of identification failed, and how.
///
/// Name: ratified 2026-11-02 (UTC), #3092. `init` meaning initialize is spelled out wherever this
/// tree authors it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum InitializationError {
    /// Reset, power or the identification clock.
    Controller(Error),
    /// `CMD0` failed.
    GoIdle(Error),
    /// `CMD8` answered with the wrong echo: what it said.
    InterfaceCondition(u32),
    /// Neither `ACMD41` nor `CMD1` was answered: no card, or a card that is not memory.
    NoCard(Error),
    /// The card stayed busy past [`POWER_UP_DEADLINE_MICROSECONDS`]: the last OCR.
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

impl core::fmt::Display for InitializationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Controller(e) => write!(f, "the controller would not start: {e}"),
            Self::GoIdle(e) => write!(f, "CMD0 failed: {e}"),
            Self::InterfaceCondition(echo) => write!(f, "CMD8 echoed {echo:#x}"),
            Self::NoCard(e) => write!(f, "no memory card answered: {e}"),
            Self::PowerUp(ocr) => write!(f, "the card stayed busy at power up (OCR {ocr:#x})"),
            Self::Identify(index, e) => write!(f, "CMD{index} failed during identification: {e}"),
            Self::Capacity(structure) => {
                write!(
                    f,
                    "the card specific data gives no capacity (structure {structure})"
                )
            }
            Self::Configure(index, e) => write!(f, "CMD{index} failed during configuration: {e}"),
            Self::Status(index, status) => {
                write!(f, "CMD{index} reported a card error (status {status:#x})")
            }
        }
    }
}

impl core::error::Error for InitializationError {}

/// A card that is selected and in the transfer state.
///
/// Name: ratified 2026-11-03 (UTC), #3139.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Card {
    /// Which kind.
    pub kind: CardKind,
    /// Its relative card address (RCA).
    #[doc(alias = "rca")]
    pub relative_card_address: u16,
    /// The operation conditions register (OCR) it reported at power-up.
    #[doc(alias = "ocr")]
    pub operation_conditions: u32,
    /// The raw card identification register (CID).
    pub card_identification_raw: [u32; 4],
    /// The raw card-specific data register (CSD).
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
    /// The card clock in use, in hertz.
    ///
    /// Name: ratified 2026-11-05 (UTC), #3276. The unit is spelled out.
    pub clock_hertz: u32,
}

impl Card {
    /// The command argument that addresses block `logical_block_address`.
    ///
    /// Name: ratified 2026-11-03 (UTC), #3139. Renamed from
    /// `address`, which did not say it is the argument of a command that addresses a block.
    #[must_use]
    pub const fn block_address_argument(&self, logical_block_address: u64) -> u32 {
        if self.block_addressed {
            logical_block_address as u32
        } else {
            (logical_block_address * BLOCK_BYTES as u64) as u32
        }
    }

    fn relative_card_address_argument(&self) -> u32 {
        u32::from(self.relative_card_address) << 16
    }
}

fn r1_ok(index: u8, r1: u32) -> Result<(), InitializationError> {
    if r1 & R1_ERROR_MASK != 0 {
        Err(InitializationError::Status(index, r1))
    } else {
        Ok(())
    }
}

/// **Bring up whatever card is in the slot** and leave it selected, in the transfer state, at
/// [`TRANSFER_HERTZ`]. `maximum_bus_width` is the tree's `bus-width` for the slot (4 on the VisionFive 2's SD
/// slot, 8 on its eMMC slot).
///
/// Name: ratified 2026-11-03 (UTC), #3139.
///
/// # Errors
///
/// The step that failed; see [`InitializationError`].
pub fn identify<R: RegisterWindowAndClock, D: DirectMemoryAccessRegion>(
    host: &mut HostController<R, D>,
    maximum_bus_width: u8,
) -> Result<Card, InitializationError> {
    let is_version_2_or_later = go_idle_and_check_interface(host)?;
    let (kind, operation_conditions) = power_up(host, is_version_2_or_later)?;
    let (card_identification_raw, relative_card_address, card_specific_data_raw) =
        address_and_select(host, kind)?;

    let mut card = Card {
        kind,
        relative_card_address,
        operation_conditions,
        card_identification_raw,
        card_specific_data_raw,
        card_identification: if kind == CardKind::MultimediaCard {
            CardIdentification::multimedia_card(&card_identification_raw)
        } else {
            CardIdentification::from_sd_register(&card_identification_raw)
        },
        blocks: 0,
        block_addressed: match kind {
            CardKind::SdHighCapacity => true,
            CardKind::MultimediaCard => {
                operation_conditions & OPERATION_CONDITIONS_HIGH_CAPACITY != 0
            }
            _ => false,
        },
        bus_width: 1,
        clock_hertz: 0,
    };

    if kind == CardKind::MultimediaCard {
        configure_multimedia_card(host, &mut card, &card_specific_data_raw)?;
    } else {
        configure_sd(host, &mut card, &card_specific_data_raw, maximum_bus_width)?;
    }
    if card.blocks == 0 {
        return Err(InitializationError::Capacity(
            CardSpecificData::from_sd_register(&card_specific_data_raw).structure,
        ));
    }
    Ok(card)
}

/// Steps 1 and 2: reset the controller at the identification clock, `CMD0`, then `CMD8`. Returns
/// whether the card echoed the check pattern (SD 2.0 or later).
pub(crate) fn go_idle_and_check_interface<
    R: RegisterWindowAndClock,
    D: DirectMemoryAccessRegion,
>(
    host: &mut HostController<R, D>,
) -> Result<bool, InitializationError> {
    host.reset().map_err(InitializationError::Controller)?;
    host.power_on();
    host.set_clock(IDENTIFY_HERTZ)
        .map_err(InitializationError::Controller)?;
    host.set_bus_width(1);

    host.send_command(&Command::new(command::GO_IDLE_STATE, 0, Response::None))
        .map_err(InitializationError::GoIdle)?;

    let is_version_2_or_later = match host.send_command(&Command::new(
        command::SEND_INTERFACE_CONDITION,
        INTERFACE_CONDITION_ARGUMENT,
        Response::R7,
    )) {
        Ok(r) if r[0] & 0xfff == INTERFACE_CONDITION_ARGUMENT => true,
        Ok(r) => return Err(InitializationError::InterfaceCondition(r[0])),
        Err(Error::NoResponse(_)) => false,
        Err(e) => {
            return Err(InitializationError::Identify(
                command::SEND_INTERFACE_CONDITION,
                e,
            ));
        }
    };
    Ok(is_version_2_or_later)
}

/// Step 3: `ACMD41` until the card is powered up, or `CMD1` for an eMMC. Returns which kind of card
/// answered and its OCR.
pub(crate) fn power_up<R: RegisterWindowAndClock, D: DirectMemoryAccessRegion>(
    host: &mut HostController<R, D>,
    is_version_2_or_later: bool,
) -> Result<(CardKind, u32), InitializationError> {
    let (kind, operation_conditions) = match sd_power_up(host, is_version_2_or_later) {
        Ok(operation_conditions) => {
            let kind = if operation_conditions & OPERATION_CONDITIONS_HIGH_CAPACITY != 0 {
                CardKind::SdHighCapacity
            } else if is_version_2_or_later {
                CardKind::SdV2
            } else {
                CardKind::SdV1
            };
            (kind, operation_conditions)
        }
        // No answer to CMD55: not an SD card. An eMMC, or nothing. CMD0 again first, since a
        // failed ACMD may have left a card confused about whether it is in application mode.
        Err(InitializationError::NoCard(_)) if !is_version_2_or_later => {
            host.send_command(&Command::new(command::GO_IDLE_STATE, 0, Response::None))
                .map_err(InitializationError::GoIdle)?;
            (CardKind::MultimediaCard, multimedia_card_power_up(host)?)
        }
        Err(e) => return Err(e),
    };
    Ok((kind, operation_conditions))
}

/// Step 4: `CMD2` for the CID, `CMD3` for the relative address, `CMD9` for the CSD, `CMD7` to
/// select. Returns the raw CID, the address and the raw CSD.
pub(crate) fn address_and_select<R: RegisterWindowAndClock, D: DirectMemoryAccessRegion>(
    host: &mut HostController<R, D>,
    kind: CardKind,
) -> Result<([u32; 4], u16, [u32; 4]), InitializationError> {
    let card_identification_raw = host
        .send_command(&Command::new(
            command::ALL_SEND_CARD_IDENTIFICATION,
            0,
            Response::R2,
        ))
        .map_err(|e| InitializationError::Identify(command::ALL_SEND_CARD_IDENTIFICATION, e))?;

    let relative_card_address = if kind == CardKind::MultimediaCard {
        let r1 = host
            .send_command(&Command::new(
                command::SEND_RELATIVE_ADDRESS,
                u32::from(MULTIMEDIA_CARD_RELATIVE_CARD_ADDRESS) << 16,
                Response::R1,
            ))
            .map_err(|e| InitializationError::Identify(command::SEND_RELATIVE_ADDRESS, e))?;
        r1_ok(command::SEND_RELATIVE_ADDRESS, r1[0])?;
        MULTIMEDIA_CARD_RELATIVE_CARD_ADDRESS
    } else {
        let r6 = host
            .send_command(&Command::new(
                command::SEND_RELATIVE_ADDRESS,
                0,
                Response::R6,
            ))
            .map_err(|e| InitializationError::Identify(command::SEND_RELATIVE_ADDRESS, e))?;
        (r6[0] >> 16) as u16
    };

    let card_specific_data_raw = host
        .send_command(&Command::new(
            command::SEND_CARD_SPECIFIC_DATA,
            u32::from(relative_card_address) << 16,
            Response::R2,
        ))
        .map_err(|e| InitializationError::Identify(command::SEND_CARD_SPECIFIC_DATA, e))?;

    let r1 = host
        .send_command(&Command::new(
            command::SELECT_CARD,
            u32::from(relative_card_address) << 16,
            Response::R1b,
        ))
        .map_err(|e| InitializationError::Identify(command::SELECT_CARD, e))?;
    r1_ok(command::SELECT_CARD, r1[0])?;
    host.wait_not_busy()
        .map_err(|e| InitializationError::Identify(command::SELECT_CARD, e))?;
    Ok((
        card_identification_raw,
        relative_card_address,
        card_specific_data_raw,
    ))
}

/// Steps 5 and 6 for an eMMC: the transfer clock, then `EXT_CSD` for the true sector count.
pub(crate) fn configure_multimedia_card<R: RegisterWindowAndClock, D: DirectMemoryAccessRegion>(
    host: &mut HostController<R, D>,
    card: &mut Card,
    card_specific_data_raw: &[u32; 4],
) -> Result<(), InitializationError> {
    let card_specific_data = CardSpecificData::multimedia_card(card_specific_data_raw);
    card.blocks = card_specific_data.blocks;
    // The clock first: EXT_CSD is 512 bytes, and at 400 kHz on one line that is 10 ms.
    card.clock_hertz = host
        .set_clock(TRANSFER_HERTZ)
        .map_err(|e| InitializationError::Configure(command::SEND_INTERFACE_CONDITION, e))?;
    let mut extended_card_specific_data = [0u8; BLOCK_BYTES];
    let command = Command::with_data(
        command::SEND_INTERFACE_CONDITION,
        0,
        Response::R1,
        Data {
            block_size: BLOCK_BYTES as u32,
            blocks: 1,
            is_write: false,
        },
    );
    let r1 = host
        .read_data(&command, &mut extended_card_specific_data)
        .map_err(|e| InitializationError::Configure(command::SEND_INTERFACE_CONDITION, e))?;
    r1_ok(command::SEND_INTERFACE_CONDITION, r1[0])?;
    // Above 2 GB the CSD's size is a placeholder; EXT_CSD's count is the truth.
    let sectors = card::extended_card_specific_data_sectors(&extended_card_specific_data);
    if card.block_addressed && sectors != 0 {
        card.blocks = sectors;
    }
    Ok(())
}

/// Steps 5 and 6 for an SD card: the capacity from the CSD, `ACMD6` to 4-bit if the slot allows,
/// `CMD16` to 512 bytes, then the transfer clock.
pub(crate) fn configure_sd<R: RegisterWindowAndClock, D: DirectMemoryAccessRegion>(
    host: &mut HostController<R, D>,
    card: &mut Card,
    card_specific_data_raw: &[u32; 4],
    maximum_bus_width: u8,
) -> Result<(), InitializationError> {
    let card_specific_data = CardSpecificData::from_sd_register(card_specific_data_raw);
    if card_specific_data.blocks == 0 {
        return Err(InitializationError::Capacity(card_specific_data.structure));
    }
    card.blocks = card_specific_data.blocks;
    if maximum_bus_width >= 4 {
        application_command(host, card)?;
        let r1 = host
            .send_command(&Command::new(command::SET_BUS_WIDTH, 2, Response::R1))
            .map_err(|e| InitializationError::Configure(command::SET_BUS_WIDTH, e))?;
        r1_ok(command::SET_BUS_WIDTH, r1[0])?;
        host.set_bus_width(4);
        card.bus_width = 4;
    }
    let r1 = host
        .send_command(&Command::new(
            command::SET_BLOCK_LENGTH,
            BLOCK_BYTES as u32,
            Response::R1,
        ))
        .map_err(|e| InitializationError::Configure(command::SET_BLOCK_LENGTH, e))?;
    r1_ok(command::SET_BLOCK_LENGTH, r1[0])?;
    card.clock_hertz = host
        .set_clock(TRANSFER_HERTZ)
        .map_err(|e| InitializationError::Configure(command::SET_BLOCK_LENGTH, e))?;
    Ok(())
}

fn application_command<R: RegisterWindowAndClock, D: DirectMemoryAccessRegion>(
    host: &mut HostController<R, D>,
    card: &Card,
) -> Result<(), InitializationError> {
    let r1 = host
        .send_command(&Command::new(
            command::APPLICATION_COMMAND,
            card.relative_card_address_argument(),
            Response::R1,
        ))
        .map_err(|e| InitializationError::Configure(command::APPLICATION_COMMAND, e))?;
    r1_ok(command::APPLICATION_COMMAND, r1[0])
}

fn sd_power_up<R: RegisterWindowAndClock, D: DirectMemoryAccessRegion>(
    host: &mut HostController<R, D>,
    is_version_2_or_later: bool,
) -> Result<u32, InitializationError> {
    let argument = if is_version_2_or_later {
        SD_OPERATION_CONDITIONS_ARGUMENT
    } else {
        SD_OPERATION_CONDITIONS_ARGUMENT & !OPERATION_CONDITIONS_HIGH_CAPACITY
    };
    let start = host.registers_mut().now_microseconds();
    loop {
        host.send_command(&Command::new(command::APPLICATION_COMMAND, 0, Response::R1))
            .map_err(InitializationError::NoCard)?;
        let operation_conditions = host
            .send_command(&Command::new(
                command::SD_SEND_OPERATION_CONDITIONS,
                argument,
                Response::R3,
            ))
            .map_err(InitializationError::NoCard)?[0];
        if operation_conditions & OPERATION_CONDITIONS_BUSY_DONE != 0 {
            return Ok(operation_conditions);
        }
        if host.registers_mut().now_microseconds().wrapping_sub(start)
            > POWER_UP_DEADLINE_MICROSECONDS
        {
            return Err(InitializationError::PowerUp(operation_conditions));
        }
    }
}

fn multimedia_card_power_up<R: RegisterWindowAndClock, D: DirectMemoryAccessRegion>(
    host: &mut HostController<R, D>,
) -> Result<u32, InitializationError> {
    let start = host.registers_mut().now_microseconds();
    loop {
        let operation_conditions = host
            .send_command(&Command::new(
                command::SEND_OPERATION_CONDITIONS,
                MULTIMEDIA_CARD_OPERATION_CONDITIONS_ARGUMENT,
                Response::R3,
            ))
            .map_err(InitializationError::NoCard)?[0];
        if operation_conditions & OPERATION_CONDITIONS_BUSY_DONE != 0 {
            return Ok(operation_conditions);
        }
        if host.registers_mut().now_microseconds().wrapping_sub(start)
            > POWER_UP_DEADLINE_MICROSECONDS
        {
            return Err(InitializationError::PowerUp(operation_conditions));
        }
    }
}

/// Why a block transfer was refused or failed.
///
/// Name: ratified 2026-11-03 (UTC), #3139. Renamed from `IoError`, which
/// covers only block transfers and not the other input and output a card does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BlockTransferError {
    /// The buffer is empty or not a whole number of blocks.
    Length,
    /// The range runs past the card's end (or past what a 32-bit address reaches).
    Range,
    /// The controller or the card failed.
    Host(Error),
    /// The card's R1 status reported an error.
    Status(u32),
}

impl core::fmt::Display for BlockTransferError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Length => f.write_str("the buffer is empty or not a whole number of blocks"),
            Self::Range => f.write_str("the block range runs past what the card can address"),
            Self::Host(e) => write!(f, "the transfer failed: {e}"),
            Self::Status(status) => write!(f, "the card reported an error (status {status:#x})"),
        }
    }
}

impl core::error::Error for BlockTransferError {}

/// **Can `count` blocks from `logical_block_address` be addressed on `card`?** Inside the card, and
/// (on a byte-addressed card) inside the four gigabytes a 32-bit byte address reaches.
///
/// Name: ratified 2026-11-03 (UTC), #3139. Renamed from `in_range`, to
/// state the yes or no question it answers.
#[must_use]
pub const fn can_address_blocks(
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
        end <= (1 << 32) / BLOCK_BYTES as u64
    }
}

fn data_command(card: &Card, logical_block_address: u64, blocks: u32, write: bool) -> Command {
    let index = match (write, blocks) {
        (false, 1) => command::READ_SINGLE_BLOCK,
        (false, _) => command::READ_MULTIPLE_BLOCK,
        (true, 1) => command::WRITE_BLOCK,
        (true, _) => command::WRITE_MULTIPLE_BLOCK,
    };
    Command::with_data(
        index,
        card.block_address_argument(logical_block_address),
        Response::R1,
        Data {
            block_size: BLOCK_BYTES as u32,
            blocks,
            is_write: write,
        },
    )
}

fn check(len: usize, card: &Card, logical_block_address: u64) -> Result<u32, BlockTransferError> {
    if len == 0 || !len.is_multiple_of(BLOCK_BYTES) {
        return Err(BlockTransferError::Length);
    }
    let blocks = (len / BLOCK_BYTES) as u64;
    if blocks > u64::from(u32::MAX) / BLOCK_BYTES as u64
        || !can_address_blocks(
            card.blocks,
            card.block_addressed,
            logical_block_address,
            blocks,
        )
    {
        return Err(BlockTransferError::Range);
    }
    Ok(blocks as u32)
}

/// **Read whole blocks from `logical_block_address` into `buffer`**, one command: `CMD17` for one
/// block, `CMD18` with the automatic stop for more.
///
/// Name: ratified 2026-11-03 (UTC), #3139.
///
/// # Errors
///
/// [`BlockTransferError::Length`] or [`BlockTransferError::Range`] before anything is sent; otherwise what the
/// controller or the card reported.
pub fn read_blocks<R: RegisterWindowAndClock, D: DirectMemoryAccessRegion>(
    host: &mut HostController<R, D>,
    card: &Card,
    logical_block_address: u64,
    buffer: &mut [u8],
) -> Result<(), BlockTransferError> {
    let blocks = check(buffer.len(), card, logical_block_address)?;
    let r1 = host
        .read_data(
            &data_command(card, logical_block_address, blocks, false),
            buffer,
        )
        .map_err(BlockTransferError::Host)?;
    if r1[0] & R1_ERROR_MASK != 0 {
        return Err(BlockTransferError::Status(r1[0]));
    }
    Ok(())
}

/// **Write whole blocks from `buffer` at `logical_block_address`**, one command: `CMD24` or
/// `CMD25`, then wait for the card to finish programming.
///
/// Name: ratified 2026-11-03 (UTC), #3139.
///
/// # Errors
///
/// As [`read_blocks`].
pub fn write_blocks<R: RegisterWindowAndClock, D: DirectMemoryAccessRegion>(
    host: &mut HostController<R, D>,
    card: &Card,
    logical_block_address: u64,
    buffer: &[u8],
) -> Result<(), BlockTransferError> {
    let blocks = check(buffer.len(), card, logical_block_address)?;
    let r1 = host
        .write_data(
            &data_command(card, logical_block_address, blocks, true),
            buffer,
        )
        .map_err(BlockTransferError::Host)?;
    if r1[0] & R1_ERROR_MASK != 0 {
        return Err(BlockTransferError::Status(r1[0]));
    }
    // The card has the data; ask whether it took it (a write error is reported in the status
    // after the transfer, not in the R1 that started it).
    let status = host
        .send_command(&Command::new(
            command::SEND_STATUS,
            card.relative_card_address_argument(),
            Response::R1,
        ))
        .map_err(BlockTransferError::Host)?[0];
    if status & R1_ERROR_MASK != 0 {
        return Err(BlockTransferError::Status(status));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(blocks: u64, block_addressed: bool) -> Card {
        Card {
            kind: CardKind::SdHighCapacity,
            relative_card_address: 1,
            operation_conditions: 0,
            card_identification_raw: [0; 4],
            card_specific_data_raw: [0; 4],
            card_identification: CardIdentification::from_sd_register(&[0; 4]),
            blocks,
            block_addressed,
            bus_width: 4,
            clock_hertz: TRANSFER_HERTZ,
        }
    }

    #[test]
    fn the_last_block_is_in_range_and_the_one_after_it_is_not() {
        assert!(can_address_blocks(10, true, 9, 1));
        assert!(!can_address_blocks(10, true, 9, 2));
        assert!(!can_address_blocks(10, true, 10, 1));
    }

    /// One command moves at most `u32::MAX / 512` blocks, because `BYTCNT` is a 32-bit byte count.
    /// `check` is given lengths it is never handed a buffer of, so nothing is allocated.
    #[test]
    fn one_command_moves_no_more_blocks_than_a_32_bit_byte_count_holds() {
        let huge = card(1 << 40, true);
        let most = (u32::MAX / BLOCK_BYTES as u32) as usize;
        assert_eq!(most, 8_388_607);
        assert_eq!(check(most * BLOCK_BYTES, &huge, 0), Ok(most as u32));
        assert_eq!(
            check((most + 1) * BLOCK_BYTES, &huge, 0),
            Err(BlockTransferError::Range)
        );
        assert_eq!(
            check((most + 2) * BLOCK_BYTES, &huge, 0),
            Err(BlockTransferError::Range)
        );
        assert_eq!(check(512 * BLOCK_BYTES, &huge, 0), Ok(512));
    }
}
