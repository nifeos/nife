//! Name: provisional.

/// The word is 8 bits. Reserved; refused.
///
/// Name: ratified 2026-11-22 (UTC), with the other three widths: a width carries its unit, in
/// bits. Refused `SIZE_U8` (a size with no unit).
pub const WIDTH_8_BITS: u64 = 0;
/// The word is 16 bits. Reserved; refused.
///
/// Name: ratified 2026-11-22 (UTC), with [`WIDTH_8_BITS`]. Refused `SIZE_U16`.
pub const WIDTH_16_BITS: u64 = 1;
/// The word is 32 bits: the one size admitted, and the one `std`'s address wait locks use.
///
/// Name: ratified 2026-11-22 (UTC), with [`WIDTH_8_BITS`]. Refused `SIZE_U32`.
pub const WIDTH_32_BITS: u64 = 2;
/// The word is 64 bits. Reserved; refused.
///
/// Name: ratified 2026-11-22 (UTC), with [`WIDTH_8_BITS`]. Refused `SIZE_U64`.
pub const WIDTH_64_BITS: u64 = 3;
/// Where the size lives.
///
/// Name: ratified 2026-11-22 (UTC), with [`WIDTH_8_BITS`]. Refused `SIZE_MASK`.
pub const WIDTH_MASK: u64 = 3;
/// Keyed by (address space, address): only threads of one space meet. Without it the address wait is
/// shared, keyed by (memory object, offset), which is reserved and which this kernel refuses.
///
/// Name: provisional.
pub const PRIVATE: u64 = 1 << 7;

/// **Is this a `flags` word the kernel answers?** Exactly one today.
///
/// Name: provisional. A function that answers yes or no is named like a question.
pub const fn is_admitted(flags: u64) -> bool {
    flags == PRIVATE | WIDTH_32_BITS
}
