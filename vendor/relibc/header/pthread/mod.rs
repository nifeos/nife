// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
//! The part of relibc's `pthread.h` that stage 1 needs internally: the mutex type `stdio` locks
//! each `FILE` with, and the constants that configure it.
//!
//! nife: seeded from relibc's `header/pthread/mod.rs` and `mutex.rs` (constants and
//! `RlctMutexAttr` unchanged). No `pthread_*` function is exported and no `pthread.h` is
//! installed; POSIX threads are stage 2, milestone 836, on milestone 812's threads.

use crate::platform::types::c_int;

pub use crate::sync::pthread_mutex::RlctMutex;

pub const PTHREAD_MUTEX_DEFAULT: c_int = 0;
pub const PTHREAD_MUTEX_ERRORCHECK: c_int = 1;
pub const PTHREAD_MUTEX_NORMAL: c_int = 2;
pub const PTHREAD_MUTEX_RECURSIVE: c_int = 3;

pub const PTHREAD_MUTEX_ROBUST: c_int = 0;
pub const PTHREAD_MUTEX_STALLED: c_int = 1;

pub const PTHREAD_PRIO_NONE: c_int = 0;

pub const PTHREAD_PROCESS_SHARED: c_int = 0;
pub const PTHREAD_PROCESS_PRIVATE: c_int = 1;

#[repr(C)]
#[derive(Clone)]
pub(crate) struct RlctMutexAttr {
    pub prioceiling: c_int,
    pub protocol: c_int,
    pub pshared: c_int,
    pub robust: c_int,
    pub ty: c_int,
}

impl RlctMutexAttr {
    pub const fn default_const() -> Self {
        Self {
            robust: PTHREAD_MUTEX_STALLED,
            pshared: PTHREAD_PROCESS_PRIVATE,
            protocol: PTHREAD_PRIO_NONE,
            prioceiling: 0,
            ty: PTHREAD_MUTEX_DEFAULT,
        }
    }
}

impl Default for RlctMutexAttr {
    fn default() -> Self {
        Self::default_const()
    }
}
