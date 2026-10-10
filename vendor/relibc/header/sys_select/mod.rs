// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
//! `sys/select.h`: the types only.
//!
//! nife: relibc implements `select` and `pselect` on epoll, which stage 1 does not seed. `select`
//! over files and sockets is stage 3 (milestone 837), so here are only `timeval`, which other
//! headers take from this module, and `fd_set`, sized as relibc sizes it.

use crate::platform::types::c_ulong;

pub use crate::header::bits_timeval::timeval;

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/sys_select.h.html>.
/// cbindgen:ignore
pub const FD_SETSIZE: usize = 1024;

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/sys_select.h.html>.
/// cbindgen:ignore
#[allow(non_camel_case_types)]
#[repr(C)]
pub struct fd_set {
    pub fds_bits: [c_ulong; FD_SETSIZE / (8 * core::mem::size_of::<c_ulong>())],
}
