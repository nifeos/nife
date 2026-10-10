// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
//! `sys/time.h` implementation.
//!
//! See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/sys_time.h.html>.
//!
//! Note that since the Open Group Base Specifications Issue 8, the
//! [`timeval`] struct has been moved to
//! the [`sys/select.h`](crate::header::sys_select) header.

use crate::{
    c_str::CStr,
    error::ResultExt,
    header::{sys_select::timeval, time::timespec},
    out::Out,
    platform::{
        Pal, Sys,
        types::{c_char, c_int, c_long},
    },
};

/// See <https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/sys_time.h.html>.
///
/// # Deprecation
/// The `ITIMER_REAL` symbolic constant was marked obsolescent in the Open
/// Group Base Specifications Issue 7, and removed in Issue 8.
#[deprecated]
pub const ITIMER_REAL: c_int = 0;

/// See <https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/sys_time.h.html>.
///
/// # Deprecation
/// The `ITIMER_VIRTUAL` symbolic constant was marked obsolescent in the Open
/// Group Base Specifications Issue 7, and removed in Issue 8.
#[deprecated]
pub const ITIMER_VIRTUAL: c_int = 1;

/// See <https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/sys_time.h.html>.
///
/// # Deprecation
/// The `ITIMER_PROF` symbolic constant was marked obsolescent in the Open
/// Group Base Specifications Issue 7, and removed in Issue 8.
#[deprecated]
pub const ITIMER_PROF: c_int = 2;

/// See <https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/sys_time.h.html>.
///
/// # Deprecation
/// The `itimerval` struct was marked obsolescent in the Open Group Base
/// Specifications Issue 7, and removed in Issue 8.
#[deprecated]
#[repr(C)]
#[derive(Default)]
pub struct itimerval {
    pub it_interval: timeval,
    pub it_value: timeval,
}

/// Non-POSIX, see <https://www.man7.org/linux/man-pages/man2/gettimeofday.2.html>.
#[repr(C)]
#[derive(Default)]
pub struct timezone {
    pub tz_minuteswest: c_int,
    pub tz_dsttime: c_int,
}

/// See <https://pubs.opengroup.org/onlinepubs/9699919799/functions/gettimeofday.html>.
///
/// See also <https://www.man7.org/linux/man-pages/man2/gettimeofday.2.html>
/// for further details on the `tzp` argument.
///
/// # Deprecation
/// The `gettimeofday()` function was marked obsolescent in the Open Group Base
/// Specifications Issue 7, and removed in Issue 8.
#[deprecated]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gettimeofday(tp: *mut timeval, tzp: *mut timezone) -> c_int {
    Sys::gettimeofday(unsafe { Out::nonnull(tp) }, unsafe { Out::nullable(tzp) })
        .map(|()| 0)
        .or_minus_one_errno()
}

// `select()` declared in `sys/select.h`, as specified in modern POSIX

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/utimes.html>.
///
/// # Deprecation
/// The `utimes()` function was marked legacy in the Open Group Base
/// Specifications Issue 6, and then unmarked in Issue 7.
#[deprecated]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn utimes(path: *const c_char, times: *const timeval) -> c_int {
    let path = unsafe { CStr::from_ptr(path) };
    let times_spec = if times.is_null() {
        None
    } else {
        Some([
            timespec {
                tv_sec: unsafe { (*times).tv_sec },
                tv_nsec: c_long::from(unsafe { (*times).tv_usec }) * 1000,
            },
            timespec {
                tv_sec: unsafe { (*times.add(1)).tv_sec },
                tv_nsec: c_long::from(unsafe { (*times.add(1)).tv_usec }) * 1000,
            },
        ])
    };
    let times_ptr = match &times_spec {
        Some(times_spec) => times_spec.as_ptr(),
        // Nullptr is valid here, it means "use current time"
        None => core::ptr::null(),
    };
    unsafe { Sys::utimens(path, times_ptr) }
        .map(|()| 0)
        .or_minus_one_errno()
}
