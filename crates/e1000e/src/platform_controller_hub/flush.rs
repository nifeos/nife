//! Name: ratified 2026-11-03 (UTC), #3145. Ruled in the group E1 ruling on e1000e's module names;
//! kept as it stands.

use super::Hardware;

/// PCI configuration offset of the descriptor ring status word.
///
/// Name: ratified 2026-10-26 (#2761). Ruled in the ruling that keeps a register or bit name copied
/// from a cited source, strictly: FreeBSD's `PCICFG_DESC_RING_STATUS` (`if_em.h`). Refused
/// `DESC_RING_STATUS` (a shortened copy).
pub const PCICFG_DESC_RING_STATUS: u64 = 0xe4;
/// The bit in it that says a flush is required.
///
/// Name: ratified 2026-10-26 (#2761). Ruled in the ruling that keeps a register or bit name copied
/// from a cited source, strictly: FreeBSD's `FLUSH_DESC_REQUIRED` (`if_em.h`). Refused `REQUIRED`
/// (a shortened copy).
pub const FLUSH_DESC_REQUIRED: u16 = 0x100;
/// `FEXTNVM11.DISABLE_MULR_FIX`, set before the flush.
pub const FEXTNVM11_DISABLE_MULR_FIX: u32 = 0x0000_2000;
/// The dummy transmit's length: FreeBSD sends 512 bytes of the ring itself.
pub(crate) const TRANSMIT_DUMMY_LENGTH: u16 = 512;

/// Is a flush required, given the status word and `TDLEN`? FreeBSD flushes nothing when the
/// transmit ring has no length.
pub const fn required(status: u16, tdlen: u32) -> bool {
    status & FLUSH_DESC_REQUIRED != 0 && tdlen != 0
}
// The 512-byte length and the flag byte at bit 24 share no bit, so | and ^ compose them
// identically; the compiler checks that here, and .cargo/mutants.toml excludes the word's
// OR-to-XOR swap on this assertion.
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () = assert!((TRANSMIT_DUMMY_LENGTH as u64) & (0x02u64 << 24) == 0);

/// The dummy transmit descriptor's second word: insert FCS, 512 bytes, no end of packet and no
/// status report, exactly FreeBSD's `txd_lower`.
///
/// Name: provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling
/// covers the rest of the name. Refused `tx_word` (tx abbreviates transmit).
pub const fn transmit_word() -> u64 {
    TRANSMIT_DUMMY_LENGTH as u64 | (0x02u64 << 24)
}
/// **The one divergence.** FreeBSD writes the dummy at its own completion cursor and then
/// writes that same index to `TDT`. This driver has no completion cursor at reset time, so it
/// writes the dummy at the device's current tail and advances the tail by one, which hands the
/// device exactly that descriptor: the effect the FreeBSD comment describes ("assign the ring
/// itself as the data of the next descriptor").
pub const fn tail_after(tdt: u32, entries: u16) -> u32 {
    (tdt + 1) % entries as u32
}
// The mask clears the register's low half, 0x1f and 1 << 8 land in that cleared half, and the two
// small constants and 0x0100_0000 share no bits with each other, so in the OR chain every swapped
// XOR meets two operands that share no bit. The compiler checks that here, and
// .cargo/mutants.toml excludes the word's OR-to-XOR swaps on this assertion.
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () = assert!(
    0x1f_u32 & 0xffff_c000 == 0
        && (1u32 << 8) & 0xffff_c000 == 0
        && 0x1f & (1u32 << 8) == 0
        && (1u32 << 8) & 0x0100_0000 == 0
);

/// **Receive Descriptor Control** (`E1000_RXDCTL`, FreeBSD's `e1000_regs.h`) for the receive flush:
/// prefetch threshold 31, host threshold 1, granularity in descriptors, the rest of the register
/// kept.
///
#[doc(alias = "receive descriptor control")]
pub const fn rxdctl(rxdctl: u32) -> u32 {
    (rxdctl & 0xffff_c000) | 0x1f | 1 << 8 | 0x0100_0000
}

/// **What the flush needs beyond BAR0**: the configuration-space status word and the transmit
/// ring's memory, which [`Hardware`] does not reach.
///
/// Name: provisional. No ruling covers it.
pub trait RingAccess {
    /// Read the 16-bit descriptor ring status word at [`PCICFG_DESC_RING_STATUS`] in the
    /// function's configuration space.
    fn descriptor_ring_status(&mut self) -> u16;
    /// Store `words` as transmit descriptor `index` of the ring the device reads, and order the
    /// stores before any later register write, since the next write hands the descriptor to the
    /// device.
    fn write_transmit_descriptor(&mut self, index: u16, words: [u64; 2]);
}

/// What [`descriptor_rings`] did.
///
/// Name: provisional. No ruling covers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The device did not ask for a flush, or the transmit ring has no length.
    NotRequired,
    /// The device asked, and its transmit ring is at `base`, not where the caller's ring is. Nothing
    /// was sent, since the dummy descriptor must go where the device reads and the caller owns,
    /// so the reset that follows may hang the device.
    ForeignTransmitRing {
        /// The transmit ring's base as `TDBAL` and `TDBAH` hold it.
        base: u64,
    },
    /// The transmit ring was flushed, and the device then stopped asking.
    TransmitFlushed,
    /// The transmit ring was flushed, the device still asked, and the receive ring was flushed.
    BothFlushed,
}

/// **Empty the descriptor rings before the reset, if the device says it must**: FreeBSD's
/// `em_flush_desc_rings`, `em_flush_tx_ring` and `em_flush_rx_ring` (`if_em.c`), in their order.
/// `transmit_ring` is the bus address of the caller's transmit ring, which is also the dummy
/// descriptor's buffer. The one divergence is said at [`tail_after`], and one check is added: the
/// flush runs only when the device's transmit ring is `transmit_ring`.
///
/// Name: provisional. No ruling covers it.
pub fn descriptor_rings(
    hardware: &mut impl Hardware,
    rings: &mut impl RingAccess,
    transmit_ring: u64,
) -> Outcome {
    use crate::control_registers::{RCTL_EN, TCTL_EN};
    use crate::registers::{RCTL, TCTL, TDBAH, TDBAL, TDLEN, TDT};
    let extended = hardware.read(super::registers::FEXTNVM11);
    hardware.write(
        super::registers::FEXTNVM11,
        extended | FEXTNVM11_DISABLE_MULR_FIX,
    );
    let status = rings.descriptor_ring_status();
    if !required(status, hardware.read(TDLEN)) {
        return Outcome::NotRequired;
    }
    let base = u64::from(hardware.read(TDBAL)) | u64::from(hardware.read(TDBAH)) << 32;
    if base != transmit_ring {
        return Outcome::ForeignTransmitRing { base };
    }
    let transmit_control = hardware.read(TCTL);
    hardware.write(TCTL, transmit_control | TCTL_EN);
    let tail = hardware.read(TDT) % u32::from(crate::TRANSMIT_ENTRIES);
    rings.write_transmit_descriptor(tail as u16, [transmit_ring, transmit_word()]);
    hardware.write(TDT, tail_after(tail, crate::TRANSMIT_ENTRIES));
    hardware.delay_microseconds(250);
    if rings.descriptor_ring_status() & FLUSH_DESC_REQUIRED == 0 {
        return Outcome::TransmitFlushed;
    }
    let receive_control = hardware.read(RCTL);
    hardware.write(RCTL, receive_control & !RCTL_EN);
    super::flush_writes(hardware);
    hardware.delay_microseconds(150);
    let current = hardware.read(super::registers::RXDCTL);
    hardware.write(super::registers::RXDCTL, rxdctl(current));
    hardware.write(RCTL, receive_control | RCTL_EN);
    super::flush_writes(hardware);
    hardware.delay_microseconds(150);
    hardware.write(RCTL, receive_control & !RCTL_EN);
    Outcome::BothFlushed
}
