// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
//! `size_t` from `stddef.h` implementation.
//!
//! See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/stddef.h.html>.

use crate::platform::types::c_ulong;

/// Unsigned integer type of the result of the sizeof operator.
#[allow(non_camel_case_types)]
pub type size_t = c_ulong;
