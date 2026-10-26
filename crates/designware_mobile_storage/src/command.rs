//! **Commands, and the `CMD` register word that sends each one.**
//!
//! The flag choices are OpenBSD's `dwmmc_exec_command`, adapted: the hold register on every command,
//! wait-for-previous-data on everything but CMD12 and CMD13, the initialization clocks on CMD0, and
//! an automatic CMD12 after a multi-block transfer. The response kinds are the SD Physical Layer
//! Simplified Specification's (section 4.9) and JEDEC JESD84's for eMMC; which of them carry a
//! CRC is the spec's statement, and the R3 exception (the OCR register has no CRC, its CRC field
//! reads as all ones) is why [`Response::checks_crc`] exists.
//!
//! The command constants keep the specifications' names: the SD specification's section 4.7.4 and
//! JESD84-B51's section 6.10.4. Each doc gives the command's number and what it does.

use crate::registers;

/// **CMD0, go idle state** (`GO_IDLE_STATE` in both specifications): every card back to idle. No
/// response.
#[doc(alias = "reset to idle")]
pub const GO_IDLE_STATE: u8 = 0;
/// **CMD1, send operation conditions** (JESD84-B51's `SEND_OP_COND`, eMMC only): negotiate the
/// voltage and wait for power-up. The response is R3.
#[doc(alias = "multimedia card operation conditions")]
pub(crate) const SEND_OP_COND: u8 = 1;
/// **CMD2, all send card identification** (`ALL_SEND_CID` in both specifications): the card's
/// identity, 128 bits, as R2.
#[doc(alias = "send card identification")]
pub(crate) const ALL_SEND_CID: u8 = 2;
/// **CMD3, send relative address** (the SD specification's `SEND_RELATIVE_ADDR`; JESD84-B51 calls
/// it `SET_RELATIVE_ADDR`): on SD the card picks its address and answers R6; on eMMC the host sets
/// it and the card answers R1.
#[doc(alias = "relative card address")]
pub(crate) const SEND_RELATIVE_ADDR: u8 = 3;
/// **CMD6, switch** (JESD84-B51's `SWITCH`; the SD specification's `SWITCH_FUNC`, unused here):
/// write one byte of `EXT_CSD` on an eMMC. The response is R1b.
#[doc(alias = "switch mode")]
pub const SWITCH: u8 = 6;
/// **CMD7, select or deselect card** (the specifications' `SELECT/DESELECT_CARD`, as OpenBSD's
/// `MMC_SELECT_CARD` writes it): move the addressed card into the transfer state. The response is
/// R1b.
#[doc(alias = "select card")]
pub(crate) const SELECT_CARD: u8 = 7;
/// **CMD8, send interface condition** (the SD specification's `SEND_IF_COND`; JESD84-B51's
/// `SEND_EXT_CSD`): on SD 2.0 and later, R7 echoing the voltage and check pattern; on eMMC, R1 and
/// the 512-byte `EXT_CSD`. The same index with two meanings is how an eMMC is told from an SD card.
#[doc(alias = "interface condition")]
pub(crate) const SEND_IF_COND: u8 = 8;
/// **CMD9, send card-specific data** (`SEND_CSD` in both specifications): the card's capacity and
/// timing, 128 bits, as R2.
#[doc(alias = "send card specific data")]
pub(crate) const SEND_CSD: u8 = 9;
/// **CMD12, stop transmission** (`STOP_TRANSMISSION` in both specifications): end a multiple-block
/// transfer. The response is R1b.
#[doc(alias = "stop transfer")]
pub(crate) const STOP_TRANSMISSION: u8 = 12;
/// **CMD13, send status** (`SEND_STATUS` in both specifications): the card status word, as R1.
#[doc(alias = "card status")]
pub const SEND_STATUS: u8 = 13;
/// **CMD16, set block length** (`SET_BLOCKLEN` in both specifications): 512, for a
/// standard-capacity card; high-capacity cards ignore it. The response is R1.
#[doc(alias = "set block length")]
pub(crate) const SET_BLOCKLEN: u8 = 16;
/// **CMD17, read single block** (`READ_SINGLE_BLOCK` in both specifications): one block from the
/// card. The response is R1.
#[doc(alias = "read one block")]
pub(crate) const READ_SINGLE_BLOCK: u8 = 17;
/// **CMD18, read multiple block** (`READ_MULTIPLE_BLOCK` in both specifications): blocks from the
/// card until CMD12. The response is R1.
#[doc(alias = "read blocks")]
pub(crate) const READ_MULTIPLE_BLOCK: u8 = 18;
/// **CMD24, write block** (`WRITE_BLOCK` in both specifications): one block to the card. The
/// response is R1.
#[doc(alias = "write one block")]
pub(crate) const WRITE_BLOCK: u8 = 24;
/// **CMD25, write multiple block** (`WRITE_MULTIPLE_BLOCK` in both specifications): blocks to the
/// card until CMD12. The response is R1.
#[doc(alias = "write blocks")]
pub(crate) const WRITE_MULTIPLE_BLOCK: u8 = 25;
/// **CMD55, application command** (the SD specification's `APP_CMD`): the next command is
/// application-specific (`ACMD`). The response is R1.
#[doc(alias = "application command")]
pub(crate) const APP_CMD: u8 = 55;
/// **ACMD6, set bus width** (the SD specification's `SET_BUS_WIDTH`): the data bus width, 1 or 4
/// bits. The response is R1.
#[doc(alias = "bus width")]
pub(crate) const SET_BUS_WIDTH: u8 = 6;
/// **ACMD41, SD send operation conditions** (the SD specification's `SD_SEND_OP_COND`): negotiate
/// the voltage and capacity support and wait for power-up. The response is R3.
#[doc(alias = "SD operation conditions")]
pub(crate) const SD_SEND_OP_COND: u8 = 41;

/// What comes back from the card, and so how the controller must be told to listen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Response {
    /// No response (CMD0): nothing comes back.
    None,
    /// **R1, normal response** (SD section 4.9.1, JESD84-B51 section 6.12): 48 bits carrying the
    /// card status word, CRC checked.
    #[doc(alias = "normal response")]
    R1,
    /// **R1b, normal response with busy** (SD section 4.9.2, JESD84-B51 section 6.12): R1, then the
    /// card holds DAT0 low while it is busy.
    #[doc(alias = "response with busy")]
    R1b,
    /// **R2, card identification or card-specific data** (SD section 4.9.3, JESD84-B51 section
    /// 6.12): 136 bits carrying the CID or the CSD register.
    #[doc(alias = "card register response")]
    R2,
    /// **R3, operation conditions** (SD section 4.9.4, JESD84-B51 section 6.12): 48 bits carrying
    /// the OCR register, with **no CRC**.
    #[doc(alias = "operation conditions response")]
    R3,
    /// **R6, published relative address** (SD section 4.9.5, SD only): 48 bits carrying the card's
    /// new relative address and some status.
    #[doc(alias = "relative address response")]
    R6,
    /// **R7, card interface condition** (SD section 4.9.6, SD only): 48 bits echoing the voltage
    /// and check pattern of CMD8.
    #[doc(alias = "interface condition response")]
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
    /// assert_ne!(w & CMD_START_CMD, 0);
    /// assert_ne!(w & CMD_SEND_INITIALIZATION, 0);
    /// assert_eq!(w & CMD_RESPONSE_EXPECT, 0);
    /// ```
    #[must_use]
    pub const fn word(&self) -> u32 {
        let mut w =
            registers::CMD_START_CMD | registers::CMD_USE_HOLD_REG | (self.index as u32 & 0x3f);
        if self.index == STOP_TRANSMISSION {
            w |= registers::CMD_STOP_ABORT_CMD;
        } else if self.index != SEND_STATUS {
            w |= registers::CMD_WAIT_PRVDATA_COMPLETE;
        }
        if self.index == GO_IDLE_STATE {
            w |= registers::CMD_SEND_INITIALIZATION;
        }
        if !matches!(self.response, Response::None) {
            w |= registers::CMD_RESPONSE_EXPECT;
        }
        if self.response.is_long() {
            w |= registers::CMD_RESPONSE_LENGTH;
        }
        if self.response.checks_crc() {
            w |= registers::CMD_CHECK_RESPONSE_CRC;
        }
        if let Some(d) = self.data {
            w |= registers::CMD_DATA_EXPECTED;
            if d.write {
                w |= registers::CMD_WR;
            }
            if d.blocks > 1 {
                w |= registers::CMD_SEND_AUTO_STOP;
            }
        }
        w
    }
}

/// The word that loads new clock settings into the card clock domain and sends nothing.
pub(crate) const UPDATE_CLOCK: u32 = registers::CMD_START_CMD
    | registers::CMD_WAIT_PRVDATA_COMPLETE
    | registers::CMD_UPDATE_CLOCK_REGISTERS_ONLY;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registers::*;

    #[test]
    fn a_long_response_with_crc_and_no_data() {
        let w = Command::new(ALL_SEND_CID, 0, Response::R2).word();
        assert_eq!(
            w,
            CMD_START_CMD
                | CMD_USE_HOLD_REG
                | CMD_WAIT_PRVDATA_COMPLETE
                | CMD_RESPONSE_EXPECT
                | CMD_RESPONSE_LENGTH
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
        assert_eq!(status & CMD_WAIT_PRVDATA_COMPLETE, 0);
        let stop = Command::new(STOP_TRANSMISSION, 0, Response::R1b).word();
        assert_eq!(stop & CMD_WAIT_PRVDATA_COMPLETE, 0);
        assert_ne!(stop & CMD_STOP_ABORT_CMD, 0);
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
        assert_eq!(r8 & CMD_WR, 0);
        let w =
            Command::with_data(WRITE_BLOCK, 0, Response::R1, Data { write: true, ..one }).word();
        assert_ne!(w & (CMD_WR | CMD_DATA_EXPECTED), 0);
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
                Command::new(SEND_STATUS, 0, r).word() & CMD_RESPONSE_LENGTH,
                0
            );
        }
        // The clock-update word, as a number: start, wait for previous data, update only.
        assert_eq!(UPDATE_CLOCK, 0x8020_2000);
    }
}
