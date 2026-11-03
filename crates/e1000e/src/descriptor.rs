//! **The queues and their descriptors**: where each queue's registers and rings sit, how big they
//! are, and what a descriptor's status says.
//!
//! Name: ratified 2026-11-03 (UTC), #3145. Ruled in the group E1 ruling on e1000e's module names;
//! kept as it stands.

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
pub const TAIL_IN_PAGE: u64 = registers::RECEIVE_DESCRIPTOR_TAIL_REGISTER - RECEIVE_QUEUE_PAGE;

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
///
/// Name: ratified 2026-10-26 (#2761). Ruled in the tree-wide ruling that spells out `max` as
/// `maximum`, which names `MAXIMUM_FRAME` in `designware_ethernet`. Refused `MAX_FRAME` (an
/// abbreviation).
pub const MAXIMUM_FRAME: usize = 1514;

/// Receive status: descriptor done. OpenBSD's `E1000_RXD_STAT_DD` (`sys/dev/pci/if_em_hw.h`),
/// which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same.
///
/// Name: ratified 2026-11-03 (UTC), #3147. Renamed from `RECEIVE_STATUS_DESCRIPTOR_DONE`: the abbreviation is
/// spelled out, and the source spelling is the alias. Refused `RX_STATUS_DD` (rx abbreviates
/// receive).
#[doc(alias = "E1000_RXD_STAT_DD")]
pub const RECEIVE_STATUS_DESCRIPTOR_DONE: u8 = 0x01;

/// Receive status: end of packet. A frame without it spans buffers, which a 2048-byte buffer and
/// a 1514-byte maximum never need, so such a frame is dropped rather than reassembled. OpenBSD's
/// `E1000_RXD_STAT_EOP` (`sys/dev/pci/if_em_hw.h`), spelled the same by FreeBSD and Linux.
///
/// Name: ratified 2026-11-03 (UTC), #3147. Renamed from `RECEIVE_STATUS_END_OF_PACKET`: the abbreviation is
/// spelled out, and the source spelling is the alias. Refused `RX_STATUS_EOP` (rx abbreviates
/// receive).
#[doc(alias = "E1000_RXD_STAT_EOP")]
pub const RECEIVE_STATUS_END_OF_PACKET: u8 = 0x02;

/// Receive errors that mean the frame is bad: CRC, symbol, sequence, carrier extension, RX data
/// (bits 0, 1, 2, 4 and 7, a mask of `0x97`). OpenBSD's `E1000_RXD_ERR_FRAME_ERR_MASK`
/// (`sys/dev/pci/if_em_hw.h`), spelled the same by FreeBSD and Linux.
///
/// Name: ratified 2026-11-03 (UTC), #3147. Renamed from `RECEIVE_FRAME_ERROR_MASK`: it is a mask of
/// several bits, and the name now says so. Refused `RX_ERRORS_FRAME` (rx abbreviates receive).
#[doc(alias = "E1000_RXD_ERR_FRAME_ERR_MASK")]
pub const RECEIVE_FRAME_ERROR_MASK: u8 = 0x01 | 0x02 | 0x04 | 0x10 | 0x80;

/// Transmit command: end of packet, insert FCS, report status. OpenBSD's `E1000_TXD_CMD_EOP`,
/// `E1000_TXD_CMD_IFCS` and `E1000_TXD_CMD_RS` (`sys/dev/pci/if_em_hw.h`; FreeBSD and Linux agree)
/// are `0x01000000`, `0x02000000` and `0x08000000`, the bits here at bit 24 of the second word. No
/// single BSD name exists for the OR of the three.
///
/// Name: ratified 2026-11-03 (UTC), #3147. Ratified as it stood. Refused `TX_COMMAND` (tx
/// abbreviates transmit).
pub const TRANSMIT_COMMAND: u8 = 0x01 | 0x02 | 0x08;

// Both words OR single distinct bits, so | and ^ compose them identically; the compiler checks
// that here, and .cargo/mutants.toml excludes the constants' OR-to-XOR swaps on this assertion.
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () = assert!(
    RECEIVE_FRAME_ERROR_MASK == 0x01 ^ 0x02 ^ 0x04 ^ 0x10 ^ 0x80
        && TRANSMIT_COMMAND == 0x01 ^ 0x02 ^ 0x08
);

/// Transmit status: descriptor done. OpenBSD's `E1000_TXD_STAT_DD` (`sys/dev/pci/if_em_hw.h`),
/// which FreeBSD and Linux spell the same.
///
/// Name: ratified 2026-11-03 (UTC), #3147. Renamed from `TRANSMIT_STATUS_DESCRIPTOR_DONE`: the abbreviation is
/// spelled out, and the source spelling is the alias. Refused `TX_STATUS_DD` (tx abbreviates
/// transmit).
#[doc(alias = "E1000_TXD_STAT_DD")]
pub const TRANSMIT_STATUS_DESCRIPTOR_DONE: u8 = 0x01;

/// What a receive descriptor's second word says, judged.
///
/// Name: ratified 2026-10-26 (#2761). Ruled in the tree-wide ruling that spells out rx and tx,
/// which names it. Refused `RxVerdict` (rx abbreviates receive).
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
/// use e1000e::{ReceiveVerdict, receive_verdict, RECEIVE_STATUS_DESCRIPTOR_DONE, RECEIVE_STATUS_END_OF_PACKET};
///
/// let word = |len: u64, status: u8| len | (status as u64) << 32;
/// let done = RECEIVE_STATUS_DESCRIPTOR_DONE | RECEIVE_STATUS_END_OF_PACKET;
/// assert_eq!(receive_verdict(word(60, done), 1514), ReceiveVerdict::Frame(60));
/// assert_eq!(receive_verdict(word(60, 0), 1514), ReceiveVerdict::NotYet);
/// assert_eq!(receive_verdict(word(0xffff, done), 1514), ReceiveVerdict::Drop);
/// ```
///
/// Name: ratified 2026-11-03 (UTC), #3147. Ratified as it stood. Refused `rx_verdict` (rx
/// abbreviates receive).
pub fn receive_verdict(word: u64, room: usize) -> ReceiveVerdict {
    let status = (word >> 32) as u8;
    if status & RECEIVE_STATUS_DESCRIPTOR_DONE == 0 {
        return ReceiveVerdict::NotYet;
    }
    let errors = (word >> 40) as u8;
    let len = (word & 0xffff) as usize;
    let fits = (14..=MAXIMUM_FRAME).contains(&len) && len <= room && len as u64 <= BUFFER_SIZE;
    if status & RECEIVE_STATUS_END_OF_PACKET == 0 || errors & RECEIVE_FRAME_ERROR_MASK != 0 || !fits
    {
        return ReceiveVerdict::Drop;
    }
    ReceiveVerdict::Frame(len)
}

// The u16 length and the command byte at bit 24 share no bit, so | and ^ compose them
// identically; the compiler checks that here, and .cargo/mutants.toml excludes the word's
// OR-to-XOR swap on this assertion.
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () = assert!((u16::MAX as u64) & ((TRANSMIT_COMMAND as u64) << 24) == 0);

/// A transmit descriptor's second word, which carries the length, the command and the status:
/// `length` bytes, [`TRANSMIT_COMMAND`], status cleared.
///
/// Name: ratified 2026-11-03 (UTC), #3147. Renamed from `transmit_word`: the second word carries
/// the length, the command and the status, and the name now says which of the words it builds.
/// Refused `tx_word` (tx abbreviates transmit).
pub const fn transmit_descriptor_command_word(length: u16) -> u64 {
    length as u64 | (TRANSMIT_COMMAND as u64) << 24
}
