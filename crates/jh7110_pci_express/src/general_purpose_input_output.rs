//! The system (SYS) general-purpose input and output (GPIO) controller's output words, four pins to a word, one byte each.
//!
//! Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.

/// The first output-enable word.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "JH7110_SYS_DOEN")]
pub const OUTPUT_ENABLE: u32 = 0x000;
/// The output-enable field's width in a pin's byte.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const OUTPUT_ENABLE_MASK: u32 = 0x3f;
/// The output-enable value that drives the pin (`GPOEN_ENABLE`).
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "GPOEN_ENABLE")]
pub const DRIVE: u32 = 0;
/// The first output word.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "JH7110_SYS_DOUT")]
pub const OUTPUT: u32 = 0x040;
/// The output field's width in a pin's byte.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const OUTPUT_MASK: u32 = 0x7f;
/// The output value for a low pin (`GPOUT_LOW`).
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "GPOUT_LOW")]
pub const LOW: u32 = 0;
/// The output value for a high pin (`GPOUT_HIGH`).
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "GPOUT_HIGH")]
pub const HIGH: u32 = 1;

/// The word offset from a first word and the bit shift of `pin`'s byte.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[must_use]
pub const fn word_and_shift(pin: u32) -> (u32, u32) {
    (4 * (pin / 4), 8 * (pin % 4))
}
