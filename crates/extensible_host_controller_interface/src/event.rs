//! **An event the controller wrote**, decoded.
//!
//! Name: ratified 2026-10-25 (#2720).

use super::*;

/// **One event, decoded.** The fields a driver uses, by event type.
///
/// Name: ratified 2026-10-25 (#2720).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// A transfer finished (or failed).
    Transfer {
        /// The physical address of the TRB it was about.
        transfer_request_block: u64,
        /// Bytes *not* transferred: zero for a full buffer, positive for a short packet.
        residual: u32,
        /// The completion code.
        code: u8,
        /// The device slot.
        slot: u8,
        /// The endpoint's device context index (1 for endpoint 0).
        endpoint: u8,
    },
    /// A command finished.
    Command {
        /// The physical address of the command TRB.
        transfer_request_block: u64,
        /// The completion code.
        code: u8,
        /// The slot it concerns: for Enable Slot, the slot the controller chose.
        slot: u8,
    },
    /// A port's status changed: the driver reads `PORTSC` to learn how.
    PortStatus {
        /// The port, numbered from 1.
        port: u8,
    },
    /// Any other event type, which this driver does not act on.
    Other {
        /// Its type code.
        kind: u8,
    },
}

/// An event's pointer halves share no bit (dword 0 below, dword 1 shifted above bit 32), which
/// the compiler checks here; the mutation exclusion for [`Event::decode`] names this constant.
/// The constant-assertion lint is off on purpose: the assertion is constant by construction, and
/// the compiler checking it is its job.
///
/// Name: ratified 2026-11-03 (UTC), #3117. The compile-time check that the named bit groups do
/// not overlap.
#[allow(clippy::assertions_on_constants, clippy::eq_op)]
// This module is private, so nothing in the crate's code reads the constant; the mutation-testing
// exclusion for `Event::decode` refers to it by name, which is why it stays despite the dead-code
// allowance. Removing it leaves that exclusion without its justification.
#[allow(dead_code)]
pub const EVENT_POINTER_HALVES_DO_NOT_OVERLAP: () =
    assert!((((u32::MAX as u64) << 32) & u32::MAX as u64) == 0);

impl Event {
    /// Decode an event TRB. Total: every input is some variant.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn decode(transfer_request_block: TransferRequestBlock) -> Event {
        let words = transfer_request_block.0;
        let pointer = words[0] as u64 | (words[1] as u64) << 32;
        let code = (words[2] >> 24) as u8;
        let slot = (words[3] >> 24) as u8;
        match transfer_request_block.kind() {
            transfer_request_block_type::TRANSFER_EVENT => Event::Transfer {
                transfer_request_block: pointer,
                residual: words[2] & 0x00ff_ffff,
                code,
                slot,
                endpoint: ((words[3] >> 16) & 0x1f) as u8,
            },
            transfer_request_block_type::COMMAND_COMPLETION => Event::Command {
                transfer_request_block: pointer,
                code,
                slot,
            },
            transfer_request_block_type::PORT_STATUS_CHANGE => Event::PortStatus {
                port: (words[0] >> 24) as u8,
            },
            kind => Event::Other { kind },
        }
    }
}
