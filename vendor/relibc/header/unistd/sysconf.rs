// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
//! `sysconf.h` implementation.
//!
//! See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/sysconf.html>.

#[cfg(target_os = "redox")]
#[path = "sysconf/redox.rs"]
mod sys;

#[cfg(any(target_os = "linux", target_os = "nife"))]
#[path = "sysconf/linux.rs"]
mod sys;

pub mod constants;

pub use constants::*;
pub use sys::*;

use core::ffi::{c_int, c_long};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sysconf(name: c_int) -> c_long {
    sysconf_impl(name)
}
