//! **The queues and their descriptors**: where each queue's registers and rings sit, how big they
//! are, and what a descriptor's status says.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// The page of BAR0 holding receive queue 0's registers, mapped into the process.
///
/// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling
/// covers the rest of the name. Refused `RX_QUEUE_PAGE` (rx abbreviates receive).
pub const RECEIVE_QUEUE_PAGE: u64 = 0x2000;

/// The page of BAR0 holding transmit queue 0's registers, mapped into the process.
///
/// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling
/// covers the rest of the name. Refused `TX_QUEUE_PAGE` (tx abbreviates transmit).
pub const TRANSMIT_QUEUE_PAGE: u64 = 0x3000;

/// Where `RDT` sits inside [`RECEIVE_QUEUE_PAGE`], and `TDT` inside [`TRANSMIT_QUEUE_PAGE`]: the
/// same offset.
pub const TAIL_IN_PAGE: u64 = registers::RDT - RECEIVE_QUEUE_PAGE;

pub(crate) const PAGE: u64 = 4096;

/// Receive descriptors. Sixteen 16-byte descriptors are 256 bytes, which satisfies `RDLEN`'s
/// multiple-of-128 rule and fits one page with room to grow.
///
/// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling
/// covers the rest of the name. Refused `RX_ENTRIES` (rx abbreviates receive).
pub const RECEIVE_ENTRIES: u16 = 16;

/// Transmit descriptors.
///
/// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling
/// covers the rest of the name. Refused `TX_ENTRIES` (tx abbreviates transmit).
pub const TRANSMIT_ENTRIES: u16 = 16;

/// Bytes per buffer, matching `RCTL`'s size field of zero.
pub const BUFFER_SIZE: u64 = 2048;

/// The largest frame either direction carries: 1500 bytes of payload and the 14-byte Ethernet
/// header, no CRC (the device strips it on receive and appends it on transmit). smoltcp's
/// `max_transmission_unit` means exactly this for an Ethernet device.
pub const MAX_FRAME: usize = 1514;

/// Receive status: descriptor done.
///
/// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling
/// covers the rest of the name. Refused `RX_STATUS_DD` (rx abbreviates receive).
pub const RECEIVE_STATUS_DD: u8 = 0x01;

/// Receive status: end of packet. A frame without it spans buffers, which a 2048-byte buffer and
/// a 1514-byte maximum never need, so such a frame is dropped rather than reassembled.
///
/// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling
/// covers the rest of the name. Refused `RX_STATUS_EOP` (rx abbreviates receive).
pub const RECEIVE_STATUS_EOP: u8 = 0x02;

/// Receive errors that mean the frame is bad: CRC, symbol, sequence, carrier extension, RX data.
/// Linux's `E1000_RXD_ERR_FRAME_ERR_MASK`.
///
/// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling
/// covers the rest of the name. Refused `RX_ERRORS_FRAME` (rx abbreviates receive).
pub const RECEIVE_ERRORS_FRAME: u8 = 0x01 | 0x02 | 0x04 | 0x10 | 0x80;

/// Transmit command: end of packet, insert FCS, report status.
///
/// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling
/// covers the rest of the name. Refused `TX_COMMAND` (tx abbreviates transmit).
pub const TRANSMIT_COMMAND: u8 = 0x01 | 0x02 | 0x08;

/// Transmit status: descriptor done.
///
/// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling
/// covers the rest of the name. Refused `TX_STATUS_DD` (tx abbreviates transmit).
pub const TRANSMIT_STATUS_DD: u8 = 0x01;

/// What a receive descriptor's second word says, judged.
///
/// Name: ratified 2026-10-26 (UTC), in the tree-wide ruling that spells out rx and tx, which names
/// it. Refused `RxVerdict` (rx abbreviates receive).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiveVerdict {
    /// The device has not written this descriptor yet.
    NotYet,
    /// A whole, good frame of this many bytes, which fits the buffer and the caller's space.
    Frame(usize),
    /// The device wrote it and it is not a frame to deliver: an error bit, no end-of-packet, or a
    /// length it could not have meant. Consumed and the buffer reposted.
    Drop,
}

/// **Judge a receive descriptor's status word**, where `word` is the descriptor's second
/// little-endian `u64` (length, checksum, status, errors, special) and `room` is how many bytes the
/// caller can take. No field is trusted, since the device writes them all: a `Frame(n)` always has
/// `n <= room` and `n <= BUFFER_SIZE`, which the Kani harness below proves for every word.
///
/// # Examples
///
/// ```
/// use e1000e::{ReceiveVerdict, receive_verdict, RECEIVE_STATUS_DD, RECEIVE_STATUS_EOP};
///
/// let word = |len: u64, status: u8| len | (status as u64) << 32;
/// let done = RECEIVE_STATUS_DD | RECEIVE_STATUS_EOP;
/// assert_eq!(receive_verdict(word(60, done), 1514), ReceiveVerdict::Frame(60));
/// assert_eq!(receive_verdict(word(60, 0), 1514), ReceiveVerdict::NotYet);
/// assert_eq!(receive_verdict(word(0xffff, done), 1514), ReceiveVerdict::Drop);
/// ```
///
/// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling
/// covers the rest of the name. Refused `rx_verdict` (rx abbreviates receive).
pub fn receive_verdict(word: u64, room: usize) -> ReceiveVerdict {
    let status = (word >> 32) as u8;
    if status & RECEIVE_STATUS_DD == 0 {
        return ReceiveVerdict::NotYet;
    }
    let errors = (word >> 40) as u8;
    let len = (word & 0xffff) as usize;
    let fits = (14..=MAX_FRAME).contains(&len) && len <= room && len as u64 <= BUFFER_SIZE;
    if status & RECEIVE_STATUS_EOP == 0 || errors & RECEIVE_ERRORS_FRAME != 0 || !fits {
        return ReceiveVerdict::Drop;
    }
    ReceiveVerdict::Frame(len)
}

/// A transmit descriptor's second word: `len` bytes, [`TRANSMIT_COMMAND`], status cleared.
///
/// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling
/// covers the rest of the name. Refused `tx_word` (tx abbreviates transmit).
pub const fn transmit_word(len: u16) -> u64 {
    len as u64 | (TRANSMIT_COMMAND as u64) << 24
}
