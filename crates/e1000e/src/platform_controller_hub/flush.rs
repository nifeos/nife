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
