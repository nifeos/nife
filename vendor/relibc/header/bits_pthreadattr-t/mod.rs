// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
//! `pthread_attr_t` from `sys/types.h` implementation.
//!
//! See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/sys_types.h.html>.

use crate::platform::types::{c_uchar, size_t};

/// Used to identify a thread attribute object.
#[repr(C)]
pub union pthread_attr_t {
    __relibc_internal_size: [c_uchar; 32],
    __relibc_internal_align: size_t,
}

// nife: stage 1 seeds no `RlctAttr`; this size check returns with stage 2 (milestone 836).
