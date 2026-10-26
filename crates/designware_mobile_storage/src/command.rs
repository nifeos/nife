//! **Commands, and the `CMD` register word that sends each one.**
//!
//! The flag choices are OpenBSD's `dwmmc_exec_command`, adapted: the hold register on every command,
//! wait-for-previous-data on everything but CMD12 and CMD13, the initialization clocks on CMD0, and
//! an automatic CMD12 after a multi-block transfer. The response kinds are the SD Physical Layer
//! Simplified Specification's (section 4.9) and JEDEC JESD84's for eMMC; which of them carry a
//! CRC is the spec's statement, and the R3 exception (the OCR register has no CRC, its CRC field
//! reads as all ones) is why [`Response::checks_crc`] exists.

use crate::registers;

/// `GO_IDLE_STATE`: every card back to idle.
pub const GO_IDLE_STATE: u8 = 0;
/// `SEND_OP_COND` (eMMC): negotiate voltage and wait for power-up.
pub(crate) const MMC_SEND_OP_COND: u8 = 1;
/// `ALL_SEND_CID`: the card's identity, 128 bits.
pub(crate) const ALL_SEND_CID: u8 = 2;
/// `SEND_RELATIVE_ADDR` (SD: the card picks one) or `SET_RELATIVE_ADDR` (eMMC: the host does).
pub(crate) const SEND_RELATIVE_ADDR: u8 = 3;
/// `SWITCH` (eMMC, R1b): write one byte of `EXT_CSD`. Also SD's `SWITCH_FUNC`, unused here.
pub const SWITCH: u8 = 6;
/// `SELECT_CARD`: into the transfer state.
pub(crate) const SELECT_CARD: u8 = 7;
/// `SEND_IF_COND` (SD 2.0 and later, R7) or `SEND_EXT_CSD` (eMMC, R1 and 512 bytes): the same
/// index, two meanings, which is how an eMMC is told from an SD card.
pub(crate) const SEND_IF_COND: u8 = 8;
/// `SEND_CSD`: the card's capacity and timing, 128 bits.
pub(crate) const SEND_CSD: u8 = 9;
/// `STOP_TRANSMISSION` (R1b).
pub(crate) const STOP_TRANSMISSION: u8 = 12;
/// `SEND_STATUS`: the card status word.
pub const SEND_STATUS: u8 = 13;
/// `SET_BLOCKLEN`: 512, for a standard-capacity card. High-capacity cards ignore it.
pub(crate) const SET_BLOCKLEN: u8 = 16;
/// `READ_SINGLE_BLOCK`.
pub(crate) const READ_SINGLE_BLOCK: u8 = 17;
/// `READ_MULTIPLE_BLOCK`.
pub(crate) const READ_MULTIPLE_BLOCK: u8 = 18;
/// `WRITE_BLOCK`.
pub(crate) const WRITE_BLOCK: u8 = 24;
/// `WRITE_MULTIPLE_BLOCK`.
pub(crate) const WRITE_MULTIPLE_BLOCK: u8 = 25;
/// `APP_CMD`: the next command is application-specific (`ACMD`).
pub(crate) const APP_CMD: u8 = 55;
/// `ACMD6`, `SET_BUS_WIDTH`.
pub(crate) const SD_SET_BUS_WIDTH: u8 = 6;
/// `ACMD41`, `SD_SEND_OP_COND`.
pub(crate) const SD_SEND_OP_COND: u8 = 41;

/// What comes back from the card, and so how the controller must be told to listen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Response {
    /// Nothing (CMD0).
    None,
    /// 48 bits, the card status, CRC checked.
    R1,
    /// R1, then the card holds DAT0 low while it is busy.
    R1b,
    /// 136 bits: the CID or the CSD.
    R2,
    /// 48 bits, the OCR, **no CRC**.
    R3,
    /// 48 bits, the new relative card address and some status.
    R6,
    /// 48 bits, the interface condition echo.
    R7,
}

impl Response {
    /// Does the card send a CRC the controller can check? All but R3 and no response.
    #[must_use]
    pub(crate) const fn checks_crc(self) -> bool {
        !matches!(self, Response::None | Response::R3)
    }

    /// Is it the 136-bit kind?
    #[must_use]
    pub(crate) const fn is_long(self) -> bool {
        matches!(self, Response::R2)
    }
}

/// A data phase: how many blocks of how many bytes, and which way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Data {
    /// Bytes per block. 512 for every block transfer this driver makes.
    pub block_size: u32,
    /// How many blocks. Above one, the controller sends CMD12 by itself at the end.
    pub blocks: u32,
    /// True for a write to the card.
    pub write: bool,
}

impl Data {
    /// Total bytes, the `BYTCNT` value. Saturating, so a hostile count cannot wrap into a small
    /// transfer; a caller asking for four gigabytes in one command is refused by [`crate::host`]'s
    /// buffer check before this is ever written.
    #[must_use]
    pub const fn bytes(&self) -> u32 {
        self.block_size.saturating_mul(self.blocks)
    }
}

/// One command to the card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    /// The command index, 0 to 63.
    pub index: u8,
    /// The 32-bit argument.
    pub argument: u32,
    /// What the card answers with.
    pub response: Response,
    /// The data phase, if there is one.
    pub data: Option<Data>,
}

impl Command {
    /// A command with no data phase.
    #[must_use]
    pub const fn new(index: u8, argument: u32, response: Response) -> Self {
        Command {
            index,
            argument,
            response,
            data: None,
        }
    }

    /// A command with a data phase.
    #[must_use]
    pub(crate) const fn with_data(
        index: u8,
        argument: u32,
        response: Response,
        data: Data,
    ) -> Self {
        Command {
            index,
            argument,
            response,
            data: Some(data),
        }
    }

    /// **The `CMD` register word that sends this command.**
    ///
    /// ```
    /// use designware_mobile_storage::command::{Command, Response, GO_IDLE_STATE};
    /// use designware_mobile_storage::registers::*;
    ///
    /// let w = Command::new(GO_IDLE_STATE, 0, Response::None).word();
    /// assert_eq!(w & 0x3f, 0);
    /// assert_ne!(w & CMD_START, 0);
    /// assert_ne!(w & CMD_SEND_INIT, 0);
    /// assert_eq!(w & CMD_RESPONSE_EXPECT, 0);
    /// ```
    #[must_use]
    pub const fn word(&self) -> u32 {
        let mut w = registers::CMD_START | registers::CMD_USE_HOLD_REG | (self.index as u32 & 0x3f);
        if self.index == STOP_TRANSMISSION {
            w |= registers::CMD_STOP_ABORT;
        } else if self.index != SEND_STATUS {
            w |= registers::CMD_WAIT_PREVIOUS_DATA;
        }
        if self.index == GO_IDLE_STATE {
            w |= registers::CMD_SEND_INIT;
        }
        if !matches!(self.response, Response::None) {
            w |= registers::CMD_RESPONSE_EXPECT;
        }
        if self.response.is_long() {
            w |= registers::CMD_LONG_RESPONSE;
        }
        if self.response.checks_crc() {
            w |= registers::CMD_CHECK_RESPONSE_CRC;
        }
        if let Some(d) = self.data {
            w |= registers::CMD_DATA_EXPECTED;
            if d.write {
                w |= registers::CMD_WRITE;
            }
            if d.blocks > 1 {
                w |= registers::CMD_SEND_AUTO_STOP;
            }
        }
        w
    }
}

/// The word that loads new clock settings into the card clock domain and sends nothing.
pub(crate) const UPDATE_CLOCK: u32 =
    registers::CMD_START | registers::CMD_WAIT_PREVIOUS_DATA | registers::CMD_UPDATE_CLOCK_ONLY;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registers::*;

    #[test]
    fn a_long_response_with_crc_and_no_data() {
        let w = Command::new(ALL_SEND_CID, 0, Response::R2).word();
        assert_eq!(
            w,
            CMD_START
                | CMD_USE_HOLD_REG
                | CMD_WAIT_PREVIOUS_DATA
                | CMD_RESPONSE_EXPECT
                | CMD_LONG_RESPONSE
                | CMD_CHECK_RESPONSE_CRC
                | 2
        );
    }

    #[test]
    fn the_ocr_is_not_crc_checked_because_it_has_none() {
        let w = Command::new(SD_SEND_OP_COND, 0x4030_0000, Response::R3).word();
        assert_ne!(w & CMD_RESPONSE_EXPECT, 0);
        assert_eq!(w & CMD_CHECK_RESPONSE_CRC, 0);
    }

    #[test]
    fn status_and_stop_do_not_wait_behind_the_data_they_report_on() {
        let status = Command::new(SEND_STATUS, 1 << 16, Response::R1).word();
        assert_eq!(status & CMD_WAIT_PREVIOUS_DATA, 0);
        let stop = Command::new(STOP_TRANSMISSION, 0, Response::R1b).word();
        assert_eq!(stop & CMD_WAIT_PREVIOUS_DATA, 0);
        assert_ne!(stop & CMD_STOP_ABORT, 0);
    }

    #[test]
    fn only_a_multi_block_transfer_asks_for_the_automatic_stop() {
        let one = Data {
            block_size: 512,
            blocks: 1,
            write: false,
        };
        let many = Data { blocks: 8, ..one };
        let r1 = Command::with_data(READ_SINGLE_BLOCK, 0, Response::R1, one).word();
        let r8 = Command::with_data(READ_MULTIPLE_BLOCK, 0, Response::R1, many).word();
        assert_eq!(r1 & CMD_SEND_AUTO_STOP, 0);
        assert_ne!(r8 & CMD_SEND_AUTO_STOP, 0);
        assert_eq!(r8 & CMD_WRITE, 0);
        let w =
            Command::with_data(WRITE_BLOCK, 0, Response::R1, Data { write: true, ..one }).word();
        assert_ne!(w & (CMD_WRITE | CMD_DATA_EXPECTED), 0);
    }

    /// Only R2 is 136 bits: a 48-bit response asked for as a long one would never complete.
    #[test]
    fn a_short_response_is_not_asked_for_as_a_long_one() {
        for r in [
            Response::R1,
            Response::R1b,
            Response::R3,
            Response::R6,
            Response::R7,
        ] {
            assert!(!r.is_long(), "{r:?}");
            assert_eq!(
                Command::new(SEND_STATUS, 0, r).word() & CMD_LONG_RESPONSE,
                0
            );
        }
        // The clock-update word, as a number: start, wait for previous data, update only.
        assert_eq!(UPDATE_CLOCK, 0x8020_2000);
    }
}
