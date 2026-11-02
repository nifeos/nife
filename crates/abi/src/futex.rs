/// The word is 8 bits. Reserved; refused.
pub const SIZE_U8: u64 = 0;
/// The word is 16 bits. Reserved; refused.
pub const SIZE_U16: u64 = 1;
/// The word is 32 bits: the one size admitted, and the one `std`'s futex locks use.
pub const SIZE_U32: u64 = 2;
/// The word is 64 bits. Reserved; refused.
pub const SIZE_U64: u64 = 3;
/// Where the size lives.
pub const SIZE_MASK: u64 = 3;
/// Keyed by (address space, address): only threads of one space meet. Without it the futex is
/// shared, keyed by (memory object, offset), which is reserved and which this kernel refuses.
pub const PRIVATE: u64 = 1 << 7;

/// **Is this a `flags` word the kernel answers?** Exactly one today.
pub const fn admitted(flags: u64) -> bool {
    flags == PRIVATE | SIZE_U32
}
