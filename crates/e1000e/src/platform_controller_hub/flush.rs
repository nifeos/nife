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
/// Future Extended NVM 11 (`FEXTNVM11`), the "disable MULR fix" bit, set before the flush. `MULR` is
/// the sources' own term and is left unexpanded: no source expands it for this bit. FreeBSD's
/// `e1000_ich8lan.h` defines `E1000_FEXTNVM11_DISABLE_MULR_FIX` with no comment, and the
/// similar-looking `E1000_TCTL_MULR` ("Multiple request support") is a different bit in the
/// transmit control register; nothing connects them, and the architect chose not to guess. "Future
/// Extended NVM 11" is the confirmed expansion of the register (FreeBSD's `e1000_regs.h` comment on
/// `E1000_FEXTNVM11`, offset 0x5BBC). Sources: OpenBSD's `sys/dev/pci/if_em_hw.h`, then FreeBSD's
/// `sys/dev/e1000/e1000_ich8lan.h`.
///
/// Name: ratified 2026-11-03 (UTC), #3150. Renamed from `FEXTNVM11_DISABLE_MULR_FIX`: the register
/// name is spelled out and `MULR` stays as the sources have it.
#[doc(alias = "FEXTNVM11_DISABLE_MULR_FIX")]
#[doc(alias = "E1000_FEXTNVM11_DISABLE_MULR_FIX")]
pub const FUTURE_EXTENDED_NON_VOLATILE_MEMORY_11_DISABLE_MULR_FIX: u32 = 0x0000_2000;
/// The dummy transmit's length: FreeBSD sends 512 bytes of the ring itself.
pub(crate) const TRANSMIT_DUMMY_LENGTH: u16 = 512;

/// Is a flush required, given the status word and `TDLEN`? FreeBSD flushes nothing when the
/// transmit ring has no length.
///
/// Name: ratified 2026-11-03 (UTC), #3151. Renamed from `required`, a bare adjective; the name now
/// has the yes-or-no shape of a question.
pub const fn is_flush_required(status: u16, transmit_descriptor_length: u32) -> bool {
    status & FLUSH_DESC_REQUIRED != 0 && transmit_descriptor_length != 0
}
// The 512-byte length and the flag byte at bit 24 share no bit, so | and ^ compose them
// identically; the compiler checks that here, so the word's
// OR-to-XOR swap is an equivalent mutant.
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () = assert!((TRANSMIT_DUMMY_LENGTH as u64) & (0x02u64 << 24) == 0);

/// The dummy transmit descriptor's second word: insert FCS, 512 bytes, no end of packet and no
/// status report, exactly FreeBSD's `txd_lower`.
///
/// Name: ratified 2026-11-03 (UTC), #3151. Renamed from `transmit_word`, which this function
/// shared with the real descriptor's function; the word is the command word of the dummy
/// descriptor, so the name says both.
pub const fn dummy_transmit_descriptor_command_word() -> u64 {
    TRANSMIT_DUMMY_LENGTH as u64 | (0x02u64 << 24)
}
/// **The one divergence.** FreeBSD writes the dummy at its own completion cursor and then
/// writes that same index to `TDT`. This driver has no completion cursor at reset time, so it
/// writes the dummy at the device's current tail and advances the tail by one, which hands the
/// device exactly that descriptor: the effect the FreeBSD comment describes ("assign the ring
/// itself as the data of the next descriptor").
///
/// Name: ratified 2026-11-03 (UTC), #3151. Renamed from `tail_after`, which did not say after what;
/// it is the tail after the dummy transmit.
pub const fn tail_after_dummy_transmit(transmit_descriptor_tail: u32, entries: u16) -> u32 {
    (transmit_descriptor_tail + 1) % entries as u32
}
// The mask clears the register's low half, 0x1f and 1 << 8 land in that cleared half, and the two
// small constants and 0x0100_0000 share no bits with each other, so in the OR chain every swapped
// XOR meets two operands that share no bit. The compiler checks that here, so
// the word's OR-to-XOR swaps are equivalent mutants.
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
/// Name: ratified 2026-11-03 (UTC), #3150. Renamed from `rxdctl`: the register mnemonic is spelled
/// out and stays as an alias.
#[doc(alias = "receive descriptor control")]
#[doc(alias = "rxdctl")]
#[doc(alias = "E1000_RXDCTL")]
pub const fn receive_descriptor_control_register_value_for_flush(current: u32) -> u32 {
    (current & 0xffff_c000) | 0x1f | 1 << 8 | 0x0100_0000
}

/// **What the flush needs beyond BAR0**: the configuration-space status word and the transmit
/// ring's memory, which [`Hardware`] does not reach.
///
/// Name: ratified 2026-11-05 (UTC). Renamed from `RingAccess`: the trait is what the flush needs
/// from its caller, so the name says flush support.
pub trait FlushSupport {
    /// Read the 16-bit descriptor ring status word at [`PCICFG_DESC_RING_STATUS`] in the
    /// function's configuration space.
    fn descriptor_ring_status(&mut self) -> u16;
    /// Store `words` as transmit descriptor `index` of the ring the device reads, and order the
    /// stores before any later register write, since the next write hands the descriptor to the
    /// device.
    fn write_transmit_descriptor(&mut self, index: u16, words: [u64; 2]);
}

/// What [`empty_descriptor_rings`] did.
///
/// Name: ratified 2026-11-05 (UTC). Renamed from `Outcome`: the name says what the function
/// reports, which rings it emptied. The variants keep their names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EmptiedRings {
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
/// descriptor's buffer. The one divergence is said at [`tail_after_dummy_transmit`], and one check is added: the
/// flush runs only when the device's transmit ring is `transmit_ring`.
///
/// Name: ratified 2026-11-05 (UTC). Renamed from `descriptor_rings`, a noun phrase that did not say
/// what the function does; the name is now a verb phrase after FreeBSD's `em_flush_desc_rings`.
#[doc(alias = "em_flush_desc_rings")]
#[doc(alias = "em_flush_tx_ring")]
#[doc(alias = "em_flush_rx_ring")]
pub fn empty_descriptor_rings(
    hardware: &mut impl Hardware,
    rings: &mut impl FlushSupport,
    transmit_ring: u64,
) -> EmptiedRings {
    use crate::control_registers::{RCTL_EN, TCTL_EN};
    use crate::registers::{
        RECEIVE_CONTROL_REGISTER, TRANSMIT_CONTROL_REGISTER,
        TRANSMIT_DESCRIPTOR_BASE_ADDRESS_HIGH_REGISTER,
        TRANSMIT_DESCRIPTOR_BASE_ADDRESS_LOW_REGISTER, TRANSMIT_DESCRIPTOR_LENGTH_REGISTER,
        TRANSMIT_DESCRIPTOR_TAIL_REGISTER,
    };
    let extended = hardware.read(super::registers::FUTURE_EXTENDED_NON_VOLATILE_MEMORY_11_REGISTER);
    hardware.write(
        super::registers::FUTURE_EXTENDED_NON_VOLATILE_MEMORY_11_REGISTER,
        extended | FUTURE_EXTENDED_NON_VOLATILE_MEMORY_11_DISABLE_MULR_FIX,
    );
    let status = rings.descriptor_ring_status();
    if !is_flush_required(status, hardware.read(TRANSMIT_DESCRIPTOR_LENGTH_REGISTER)) {
        return EmptiedRings::NotRequired;
    }
    // The halves share no bit, so their sum is the joined address, and unlike an OR no mutation of
    // the sum survives a test that gives both halves a value.
    let base = u64::from(hardware.read(TRANSMIT_DESCRIPTOR_BASE_ADDRESS_LOW_REGISTER))
        + (u64::from(hardware.read(TRANSMIT_DESCRIPTOR_BASE_ADDRESS_HIGH_REGISTER)) << 32);
    if base != transmit_ring {
        return EmptiedRings::ForeignTransmitRing { base };
    }
    let transmit_control = hardware.read(TRANSMIT_CONTROL_REGISTER);
    hardware.write(TRANSMIT_CONTROL_REGISTER, transmit_control | TCTL_EN);
    let tail =
        hardware.read(TRANSMIT_DESCRIPTOR_TAIL_REGISTER) % u32::from(crate::TRANSMIT_ENTRIES);
    rings.write_transmit_descriptor(
        tail as u16,
        [transmit_ring, dummy_transmit_descriptor_command_word()],
    );
    hardware.write(
        TRANSMIT_DESCRIPTOR_TAIL_REGISTER,
        tail_after_dummy_transmit(tail, crate::TRANSMIT_ENTRIES),
    );
    hardware.delay_microseconds(250);
    if rings.descriptor_ring_status() & FLUSH_DESC_REQUIRED == 0 {
        return EmptiedRings::TransmitFlushed;
    }
    let receive_control = hardware.read(RECEIVE_CONTROL_REGISTER);
    hardware.write(RECEIVE_CONTROL_REGISTER, receive_control & !RCTL_EN);
    super::flush_writes(hardware);
    hardware.delay_microseconds(150);
    let current = hardware.read(super::registers::RECEIVE_DESCRIPTOR_CONTROL_REGISTER);
    hardware.write(
        super::registers::RECEIVE_DESCRIPTOR_CONTROL_REGISTER,
        receive_descriptor_control_register_value_for_flush(current),
    );
    hardware.write(RECEIVE_CONTROL_REGISTER, receive_control | RCTL_EN);
    super::flush_writes(hardware);
    hardware.delay_microseconds(150);
    hardware.write(RECEIVE_CONTROL_REGISTER, receive_control & !RCTL_EN);
    EmptiedRings::BothFlushed
}
