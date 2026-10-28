//! **One Transfer Request Block (TRB)** and the builders for the TRBs the driver writes.
//!
//! Name: ratified 2026-10-25 (#2720). Refused `trb` (spelled out, as the acronym test asks).

use super::*;

/// **One TRB**: four little-endian dwords, the cycle bit in bit 0 of the last.
///
/// Name: ratified 2026-10-25 (#2720). Refused `Trb` (spelled out, as the acronym test asks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransferRequestBlock(pub [u32; 4]);

/// Interrupt On Completion: ask for an event when this TRB completes.
pub(crate) const IOC: u32 = 1 << 5;

/// Interrupt on Short Packet.
pub(crate) const ISP: u32 = 1 << 2;

/// Immediate Data: the TRB's first two dwords are the data itself (the setup packet).
pub(crate) const IDT: u32 = 1 << 6;

/// The transfer control word's fields share no bit: the type at 10:15 against the toggle, short
/// packet, completion and immediate-data bits at 1, 2, 5 and 6, all of them against dword 1's
/// fields at 16 and above, and the endpoint at 16:23 against the slot at 24:31. The compiler
/// checks it here; the mutation exclusions for this file's TRB builders name this constant. The
/// constant-assertion lint is off on purpose: the assertion is constant by construction, and the
/// compiler checking it is its job.
///
/// Name: provisional, minted 2026-10-28 (UTC) by lane/954-remaining-crates.
#[allow(clippy::assertions_on_constants, clippy::eq_op)]
// This module is private, so nothing outside can reach the constants; the mutation exclusions in
// .cargo/mutants.toml are their readers, and helpers/mutation_exclusion_check.py fails if they
// are gone.
#[allow(dead_code)]
pub const THE_TRANSFER_CONTROL_FIELDS_SHARE_NO_BIT: () = assert!(
    ((0x3fu32 << 10) & (1 << 1 | 1 << 2 | 1 << 5 | 1 << 6)) == 0
        && ((0x3fu32 << 10 | 1 << 1 | 1 << 2 | 1 << 5 | 1 << 6) & (u32::MAX << 16)) == 0
        && ((0xffu32 << 16) & (0xff << 24)) == 0
);

/// The stamped cycle bit shares no bit with the word the [`TransferRequestBlock::with_cycle`]
/// masks first: the mask clears bit 0 and the stamp is a `bool`, so the compiler's own range
/// check closes the argument. The mutation exclusion for `with_cycle` names this constant. The
/// constant-assertion lint is off on purpose: the assertion is constant by construction, and the
/// compiler checking it is its job.
///
/// Name: provisional, minted 2026-10-28 (UTC) by lane/954-remaining-crates.
#[allow(clippy::assertions_on_constants, clippy::eq_op)]
// This module is private, so nothing outside can reach the constant; the mutation exclusions in
// .cargo/mutants.toml are its readers, and helpers/mutation_exclusion_check.py fails if it is
// gone.
#[allow(dead_code)]
pub const THE_CYCLE_BIT_AND_CLEARED_WORD_SHARE_NO_BIT: () = assert!(((!1u32) & 1) == 0);

pub(crate) const fn control(kind: u8) -> u32 {
    (kind as u32) << 10
}

impl TransferRequestBlock {
    /// The TRB's type code.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn kind(&self) -> u8 {
        ((self.0[3] >> 10) & 0x3f) as u8
    }

    /// The cycle bit.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn cycle(&self) -> bool {
        self.0[3] & 1 != 0
    }

    /// The same TRB stamped with `cycle`.
    pub(crate) const fn with_cycle(self, cycle: bool) -> TransferRequestBlock {
        let mut d = self.0;
        d[3] = (d[3] & !1) | cycle as u32;
        TransferRequestBlock(d)
    }

    const fn pointer(address: u64, rest: [u32; 2]) -> TransferRequestBlock {
        TransferRequestBlock([address as u32, (address >> 32) as u32, rest[0], rest[1]])
    }

    /// A link TRB back to `target` with Toggle Cycle set, which is what flips the consumer's
    /// cycle as it follows the link.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn link(target: u64) -> TransferRequestBlock {
        TransferRequestBlock::pointer(
            target,
            [0, control(transfer_request_block_type::LINK) | 1 << 1],
        )
    }

    /// Enable Slot: ask the controller for a device slot.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn enable_slot() -> TransferRequestBlock {
        TransferRequestBlock([0, 0, 0, control(transfer_request_block_type::ENABLE_SLOT)])
    }

    /// Disable Slot `slot`: give it back.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn disable_slot(slot: u8) -> TransferRequestBlock {
        TransferRequestBlock([
            0,
            0,
            0,
            control(transfer_request_block_type::DISABLE_SLOT) | (slot as u32) << 24,
        ])
    }

    /// Address Device: take the input context at `input` (physical) for `slot`, and send the
    /// device its address.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn address_device(input: u64, slot: u8) -> TransferRequestBlock {
        TransferRequestBlock::pointer(
            input,
            [
                0,
                control(transfer_request_block_type::ADDRESS_DEVICE) | (slot as u32) << 24,
            ],
        )
    }

    /// Configure Endpoint: add the endpoints the input context's add flags name.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn configure_endpoint(input: u64, slot: u8) -> TransferRequestBlock {
        TransferRequestBlock::pointer(
            input,
            [
                0,
                control(transfer_request_block_type::CONFIGURE_ENDPOINT) | (slot as u32) << 24,
            ],
        )
    }

    /// Evaluate Context: update the fields the input context names (here, endpoint 0's packet
    /// size once the device has said what it is).
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn evaluate_context(input: u64, slot: u8) -> TransferRequestBlock {
        TransferRequestBlock::pointer(
            input,
            [
                0,
                control(transfer_request_block_type::EVALUATE_CONTEXT) | (slot as u32) << 24,
            ],
        )
    }

    /// Reset Endpoint `dci` of `slot`, after a stall halted it.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn reset_endpoint(slot: u8, dci: u8) -> TransferRequestBlock {
        TransferRequestBlock([
            0,
            0,
            0,
            control(transfer_request_block_type::RESET_ENDPOINT)
                | (dci as u32) << 16
                | (slot as u32) << 24,
        ])
    }

    /// Set TR Dequeue Pointer of endpoint `dci` of `slot` to `pointer` (the ring position with the
    /// consumer cycle in bit 0, [`ProducerRing::dequeue_pointer`]), skipping what a stall left on
    /// the ring.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn set_dequeue(pointer: u64, slot: u8, dci: u8) -> TransferRequestBlock {
        TransferRequestBlock::pointer(
            pointer,
            [
                0,
                control(transfer_request_block_type::SET_TR_DEQUEUE)
                    | (dci as u32) << 16
                    | (slot as u32) << 24,
            ],
        )
    }

    /// A control transfer's Setup Stage, the eight-byte packet carried immediately. `data_in` is
    /// `Some(true)` for a data stage device to host, `Some(false)` host to device, `None` for none
    /// (the Transfer Type field, xHCI 1.2 section 6.4.1.2.1).
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn setup(packet: u64, data_in: Option<bool>) -> TransferRequestBlock {
        let trt = match data_in {
            None => 0,
            Some(false) => 2,
            Some(true) => 3,
        };
        TransferRequestBlock([
            packet as u32,
            (packet >> 32) as u32,
            8,
            control(transfer_request_block_type::SETUP) | IDT | trt << 16,
        ])
    }

    /// A control transfer's Data Stage: `len` bytes at `buffer` (physical).
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn data(buffer: u64, len: u16, inbound: bool) -> TransferRequestBlock {
        TransferRequestBlock::pointer(
            buffer,
            [
                len as u32,
                control(transfer_request_block_type::DATA) | (inbound as u32) << 16 | ISP,
            ],
        )
    }

    /// A control transfer's Status Stage, which flows opposite to the data stage (device to host
    /// when there was none, or when it flowed host to device). Asks for the completion event.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn status(data_was_inbound: bool) -> TransferRequestBlock {
        TransferRequestBlock([
            0,
            0,
            0,
            control(transfer_request_block_type::STATUS) | (!data_was_inbound as u32) << 16 | IOC,
        ])
    }

    /// A Normal TRB: `len` bytes at `buffer` (physical), with an event on completion or a short
    /// packet. What an interrupt IN endpoint's ring holds.
    ///
    /// Name: ratified 2026-10-25 (#2720).
    pub const fn normal(buffer: u64, len: u16) -> TransferRequestBlock {
        TransferRequestBlock::pointer(
            buffer,
            [
                len as u32,
                control(transfer_request_block_type::NORMAL) | IOC | ISP,
            ],
        )
    }
}
