//! **An event the controller wrote**, decoded.

use super::*;

/// **One event, decoded.** The fields a driver uses, by event type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// A transfer finished (or failed).
    Transfer {
        /// The physical address of the TRB it was about.
        trb: u64,
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
        trb: u64,
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

impl Event {
    /// Decode an event TRB. Total: every input is some variant.
    pub const fn decode(t: TransferRequestBlock) -> Event {
        let d = t.0;
        let pointer = d[0] as u64 | (d[1] as u64) << 32;
        let code = (d[2] >> 24) as u8;
        let slot = (d[3] >> 24) as u8;
        match t.kind() {
            transfer_request_block_type::TRANSFER_EVENT => Event::Transfer {
                trb: pointer,
                residual: d[2] & 0x00ff_ffff,
                code,
                slot,
                endpoint: ((d[3] >> 16) & 0x1f) as u8,
            },
            transfer_request_block_type::COMMAND_COMPLETION => Event::Command {
                trb: pointer,
                code,
                slot,
            },
            transfer_request_block_type::PORT_STATUS_CHANGE => Event::PortStatus {
                port: (d[0] >> 24) as u8,
            },
            kind => Event::Other { kind },
        }
    }
}
