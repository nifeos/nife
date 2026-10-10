// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
//! `sys/uio.h` implementation.
//!
//! See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/sys_uio.h.html>.

use crate::{
    error::ResultExt,
    header::{errno, limits::IOV_MAX},
    platform::{
        self, Pal, Sys,
        types::{c_int, ssize_t},
    },
};

pub use crate::header::bits_iovec::{gather, iovec, scatter};

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/readv.html>.
///
/// Equivalent to `read()` but places the input data into the `iovcnt` buffers
/// specified by the members of the `iov` array.
///
/// When successful, returns a non-negative number indicating the number of
/// bytes actually read. Upon failure, returns `-1`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn readv(fd: c_int, iov: *const iovec, iovcnt: c_int) -> ssize_t {
    if !(0..=IOV_MAX).contains(&iovcnt) {
        platform::ERRNO.set(errno::EINVAL);
        return -1;
    }

    unsafe { Sys::readv(fd, iov, iovcnt) }
        .map(|n| n as ssize_t)
        .or_minus_one_errno()
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/writev.html>.
///
/// Equivalent to `write()` but shall gather output data from the `iovcnt`
/// buffers specified by the members of the `iov` array.
///
/// When successful, returns a non-negative number indicating the number of
/// bytes actually written to the file associated with `fildes`. Upon failure,
/// returns `-1`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn writev(fd: c_int, iov: *const iovec, iovcnt: c_int) -> ssize_t {
    if !(0..=IOV_MAX).contains(&iovcnt) {
        platform::ERRNO.set(errno::EINVAL);
        return -1;
    }

    unsafe { Sys::writev(fd, iov, iovcnt) }
        .map(|n| n as ssize_t)
        .or_minus_one_errno()
}
