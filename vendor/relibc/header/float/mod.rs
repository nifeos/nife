// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
//! `float.h` implementation.
//!
//! See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/float.h.html>.

use crate::platform::types::c_int;

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/float.h.html>.
pub const FLT_RADIX: c_int = 2;

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/float.h.html>.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn flt_rounds() -> c_int {
    // nife: no `fenv.h` in stage 1. The userspace targets are soft-float (milestone 534), and
    // compiler-builtins' soft-float rounds to nearest, always.
    1
}
