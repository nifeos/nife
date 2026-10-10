//! The one trait relibc took from the `plain` crate: "a type any bit pattern is a valid value of".
//!
//! nife: written beside the seed rather than taken as a dependency (§46 (thin primitives or whole subsystems;
//! we write everything in between)), because all the seed uses of the crate is this marker and its
//! impls for the primitive integers.

/// # Safety
/// Implement only for types with no padding and no invalid bit patterns.
pub unsafe trait Plain {}

macro_rules! plain_impl {
    ($($t:ty),*) => { $(unsafe impl Plain for $t {})* };
}
plain_impl!(u8, i8, u16, i16, u32, i32, u64, i64, usize, isize);
