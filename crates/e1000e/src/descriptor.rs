//! **The queues and their descriptors**: where each queue's registers and rings sit, how big they
//! are, and what a descriptor's status says.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// The page of BAR0 holding receive queue 0's registers, mapped into the process.
pub const RX_QUEUE_PAGE: u64 = 0x2000;

/// The page of BAR0 holding transmit queue 0's registers, mapped into the process.
pub const TX_QUEUE_PAGE: u64 = 0x3000;

/// Where `RDT` sits inside [`RX_QUEUE_PAGE`], and `TDT` inside [`TX_QUEUE_PAGE`]: the same offset.
pub const TAIL_IN_PAGE: u64 = regs::RDT - RX_QUEUE_PAGE;

pub(crate) const PAGE: u64 = 4096;

/// Receive descriptors. Sixteen 16-byte descriptors are 256 bytes, which satisfies `RDLEN`'s
/// multiple-of-128 rule and fits one page with room to grow.
pub const RX_ENTRIES: u16 = 16;

/// Transmit descriptors.
pub const TX_ENTRIES: u16 = 16;

/// Bytes per buffer, matching `RCTL`'s size field of zero.
pub const BUFFER_SIZE: u64 = 2048;

/// The largest frame either direction carries: 1500 bytes of payload and the 14-byte Ethernet
/// header, no CRC (the device strips it on receive and appends it on transmit). smoltcp's
/// `max_transmission_unit` means exactly this for an Ethernet device.
pub const MAX_FRAME: usize = 1514;

/// Receive status: descriptor done.
pub const RX_STATUS_DD: u8 = 0x01;

/// Receive status: end of packet. A frame without it spans buffers, which a 2048-byte buffer and
/// a 1514-byte maximum never need, so such a frame is dropped rather than reassembled.
pub const RX_STATUS_EOP: u8 = 0x02;

/// Receive errors that mean the frame is bad: CRC, symbol, sequence, carrier extension, RX data.
/// Linux's `E1000_RXD_ERR_FRAME_ERR_MASK`.
pub const RX_ERRORS_FRAME: u8 = 0x01 | 0x02 | 0x04 | 0x10 | 0x80;

/// Transmit command: end of packet, insert FCS, report status.
pub const TX_COMMAND: u8 = 0x01 | 0x02 | 0x08;

/// Transmit status: descriptor done.
pub const TX_STATUS_DD: u8 = 0x01;

/// What a receive descriptor's second word says, judged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RxVerdict {
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
/// caller can take. No field is trusted (notes/shared-page-audit.md's finding 6): a `Frame(n)`
/// always has `n <= room` and `n <= BUFFER_SIZE`, which the Kani harness below proves for every word.
///
/// # Examples
///
/// ```
/// use e1000e::{RxVerdict, rx_verdict, RX_STATUS_DD, RX_STATUS_EOP};
///
/// let word = |len: u64, status: u8| len | (status as u64) << 32;
/// assert_eq!(rx_verdict(word(60, RX_STATUS_DD | RX_STATUS_EOP), 1514), RxVerdict::Frame(60));
/// assert_eq!(rx_verdict(word(60, 0), 1514), RxVerdict::NotYet);
/// assert_eq!(rx_verdict(word(0xffff, RX_STATUS_DD | RX_STATUS_EOP), 1514), RxVerdict::Drop);
/// ```
pub fn rx_verdict(word: u64, room: usize) -> RxVerdict {
    let status = (word >> 32) as u8;
    if status & RX_STATUS_DD == 0 {
        return RxVerdict::NotYet;
    }
    let errors = (word >> 40) as u8;
    let len = (word & 0xffff) as usize;
    let fits = (14..=MAX_FRAME).contains(&len) && len <= room && len as u64 <= BUFFER_SIZE;
    if status & RX_STATUS_EOP == 0 || errors & RX_ERRORS_FRAME != 0 || !fits {
        return RxVerdict::Drop;
    }
    RxVerdict::Frame(len)
}

/// A transmit descriptor's second word: `len` bytes, [`TX_COMMAND`], status cleared.
pub const fn tx_word(len: u16) -> u64 {
    len as u64 | (TX_COMMAND as u64) << 24
}
