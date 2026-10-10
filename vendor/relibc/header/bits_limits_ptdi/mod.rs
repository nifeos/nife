// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
//! `PTHREAD_DESTRUCTOR_ITERATIONS` for `limits.h` implementation.
//!
//! See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/limits.h.html>.

use crate::platform::types::c_long;

/// Minimum required value for `PTHREAD_DESTRUCTOR_ITERATIONS`.
pub const _POSIX_THREAD_DESTRUCTOR_ITERATIONS: c_long = 4;

/// Maximum number of attempts made to destroy a thread's thread-specific data
/// values on thread exit.
pub const PTHREAD_DESTRUCTOR_ITERATIONS: c_long = _POSIX_THREAD_DESTRUCTOR_ITERATIONS;
