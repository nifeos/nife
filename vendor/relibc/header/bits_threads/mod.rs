// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
//! pthread types shared between `threads.h` and `pthread.h`.
//!
//! See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/sys_types.h.html>.

use crate::{
    platform::types::{c_int, c_uchar, c_ulong},
    pthread_assert_equal_size,
};

/// Used for thread-specific data keys.
pub type pthread_key_t = c_ulong;

/// Used for mutexes.
#[repr(C)]
pub union pthread_mutex_t {
    __relibc_internal_size: [c_uchar; 12],
    __relibc_internal_align: c_int,
}
/// Used for condition variables.
#[repr(C)]
pub union pthread_cond_t {
    __relibc_internal_size: [c_uchar; 8],
    __relibc_internal_align: c_int,
}

pthread_assert_equal_size!(pthread_mutex_t, RlctMutex);
// nife: stage 1 seeds no `RlctCond`; this size check returns with stage 2 (milestone 836).
