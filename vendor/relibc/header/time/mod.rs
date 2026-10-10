// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
//! `time.h` implementation.
//!
//! See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/time.h.html>.

use crate::{
    error::{Errno, ResultExt},
    header::errno::{ENOMEM, EOVERFLOW, ETIMEDOUT},
    out::Out,
    platform::{
        self, Pal, Sys,
        types::{
            c_char, c_double, c_int, c_long, clock_t, clockid_t, pid_t, size_t, time_t,
        },
    },
    raw_cell::RawCell,
};
use core::{convert::TryFrom, mem, ptr};

pub use crate::header::bits_timespec::timespec;

pub use self::constants::*;

pub mod constants;

mod strftime;
mod strptime;
pub use strptime::strptime;

/// cbindgen:ignore
const YEARS_PER_ERA: time_t = 400;
/// cbindgen:ignore
const DAYS_PER_ERA: time_t = 146097;
/// cbindgen:ignore
const SECS_PER_DAY: time_t = 24 * 60 * 60;
/// cbindgen:ignore
pub(crate) const NANOSECONDS: c_long = 1_000_000_000;
/// cbindgen:ignore
const UTC_STR: &core::ffi::CStr = c"UTC";

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/time.h.html>.
#[allow(non_camel_case_types)]
#[repr(C)]
pub struct tm {
    pub tm_sec: c_int,          // 0 - 60
    pub tm_min: c_int,          // 0 - 59
    pub tm_hour: c_int,         // 0 - 23
    pub tm_mday: c_int,         // 1 - 31
    pub tm_mon: c_int,          // 0 - 11
    pub tm_year: c_int,         // years since 1900
    pub tm_wday: c_int,         // 0 - 6 (Sunday - Saturday)
    pub tm_yday: c_int,         // 0 - 365
    pub tm_isdst: c_int,        // >0 if DST, 0 if not, <0 if unknown
    pub tm_gmtoff: c_long,      // offset from UTC in seconds
    pub tm_zone: *const c_char, // timezone abbreviation
}

unsafe impl Sync for tm {}

/// cbindgen:ignore
// The C Standard says that localtime and gmtime return the same pointer.
static GMTIME_LOCALTIME_RETURN_TM: RawCell<tm> = RawCell::new(blank_tm());

/// cbindgen:ignore
// The C Standard says that ctime and asctime return the same pointer.
static mut ASCTIME: [c_char; 26] = [0; 26];

#[repr(transparent)]
pub struct TzName([*mut c_char; 2]);

unsafe impl Sync for TzName {}

// Should only be accessed by relibc when `TIMEZONE_LOCK` is held
/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/tzset.html>.
#[unsafe(no_mangle)]
pub static mut daylight: c_int = 0;

// Should only be accessed by relibc when `TIMEZONE_LOCK` is held
/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/tzset.html>.
#[unsafe(no_mangle)]
pub static mut timezone: c_long = 0;

// Should only be accessed by relibc when `TIMEZONE_LOCK` is held
/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/tzset.html>.
#[unsafe(no_mangle)]
pub static mut tzname: TzName = TzName([ptr::null_mut(); 2]);

#[unsafe(no_mangle)]
pub static mut getdate_err: c_int = 0;

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/time.h.html>.
#[repr(C)]
#[derive(Clone, Default)]
pub struct itimerspec {
    pub it_interval: timespec,
    pub it_value: timespec,
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/asctime.html>.
///
/// # Deprecation
/// The `asctime()` function was marked obsolescent in the Open Group Base
/// Specifications Issue 7.
#[deprecated]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn asctime(timeptr: *const tm) -> *mut c_char {
    unsafe {
        #[expect(deprecated)]
        asctime_r(timeptr, (&raw mut ASCTIME).cast())
    }
}

/// See <https://pubs.opengroup.org/onlinepubs/9699919799/functions/asctime.html>.
///
/// # Deprecation
/// The `asctime_r()` was marked obsolescent in the Open Group Base
/// Specifications Issue 7, and removed in Issue 8.
#[deprecated]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn asctime_r(tm: *const tm, buf: *mut c_char) -> *mut c_char {
    let tm_sec = unsafe { (*tm).tm_sec };
    let tm_min = unsafe { (*tm).tm_min };
    let tm_hour = unsafe { (*tm).tm_hour };
    let tm_mday = unsafe { (*tm).tm_mday };
    let tm_mon = unsafe { (*tm).tm_mon };
    let tm_year = unsafe { (*tm).tm_year };
    let tm_wday = unsafe { (*tm).tm_wday };

    /* Panic when we run into undefined behavior.
     *
     * POSIX says (since issue 7) that asctime()/asctime_r() cause UB
     * when the tm member values would cause out-of-bounds array access
     * or overflow the output buffer. This contrasts with ISO C11+,
     * which specifies UB for any tm members being outside their normal
     * ranges. While POSIX explicitly defers to the C standard in case
     * of contradictions, the assertions below follow the interpretation
     * that POSIX simply defines some of C's undefined behavior, rather
     * than conflict with the ISO standard.
     *
     * Note that C's "%.2d" formatting, unlike Rust's "{:02}"
     * formatting, does not count a minus sign against the two digits to
     * print, meaning that we must reject all negative values for
     * seconds, minutes and hours. However, C's "%3d" (for day-of-month)
     * is similar to Rust's "{:3}".
     *
     * To avoid year overflow problems (in Rust, where numeric overflow
     * is considered an error), we subtract 1900 from the endpoints,
     * rather than adding to the tm_year value. POSIX' requirement that
     * tm_year be at most {INT_MAX}-1990 is satisfied for all legal
     * values of {INT_MAX} through the max-4-digit requirement on the
     * year.
     *
     * The tm_mon and tm_wday fields are used for array access and thus
     * will already cause a panic in Rust code when out of range.
     * However, using the assertions below allows a consistent error
     * message for all fields. */
    const OUT_OF_RANGE_MESSAGE: &str = "tm member out of range";

    assert!((0..=99).contains(&tm_sec), "{OUT_OF_RANGE_MESSAGE}");
    assert!((0..=99).contains(&tm_min), "{OUT_OF_RANGE_MESSAGE}");
    assert!((0..=99).contains(&tm_hour), "{OUT_OF_RANGE_MESSAGE}");
    assert!((-99..=999).contains(&tm_mday), "{OUT_OF_RANGE_MESSAGE}");
    assert!((0..=11).contains(&tm_mon), "{OUT_OF_RANGE_MESSAGE}");
    assert!(
        (-999 - 1900..=9999 - 1900).contains(&tm_year),
        "{OUT_OF_RANGE_MESSAGE}"
    );
    assert!((0..=6).contains(&tm_wday), "{OUT_OF_RANGE_MESSAGE}");

    // At this point, we can safely use the values as given.
    let write_result = core::fmt::write(
        // buf may be either `*mut u8` or `*mut i8`
        &mut platform::UnsafeStringWriter(buf.cast()),
        format_args!(
            "{:.3} {:.3}{:3} {:02}:{:02}:{:02} {}\n",
            DAY_NAMES[usize::try_from(tm_wday).unwrap()],
            MON_NAMES[usize::try_from(tm_mon).unwrap()],
            tm_mday,
            tm_hour,
            tm_min,
            tm_sec,
            1900 + tm_year
        ),
    );
    match write_result {
        Ok(()) => buf,
        Err(_) => {
            /* asctime()/asctime_r() or the equivalent sprintf() call
             * have no defined errno setting */
            ptr::null_mut()
        }
    }
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock.html>.
#[unsafe(no_mangle)]
pub extern "C" fn clock() -> clock_t {
    let mut ts = mem::MaybeUninit::<timespec>::uninit();

    if unsafe { clock_gettime(CLOCK_PROCESS_CPUTIME_ID, ts.as_mut_ptr()) } != 0 {
        return -1;
    }
    let ts = unsafe { ts.assume_init() };

    #[cfg(target_arch = "x86")]
    let clocks = ts.tv_sec * i64::from(CLOCKS_PER_SEC)
        + i64::from(ts.tv_nsec / (1_000_000_000 / CLOCKS_PER_SEC));
    #[cfg(not(target_arch = "x86"))]
    let clocks = ts.tv_sec * CLOCKS_PER_SEC + (ts.tv_nsec / (1_000_000_000 / CLOCKS_PER_SEC));
    clock_t::try_from(clocks).unwrap_or(-1)
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock_getcpuclockid.html>.
// #[unsafe(no_mangle)]
#[expect(unused_variables, reason = "function not yet implemented")]
pub extern "C" fn clock_getcpuclockid(pid: pid_t, clock_id: *mut clockid_t) -> c_int {
    unimplemented!();
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock_getres.html>.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn clock_getres(clock_id: clockid_t, res: *mut timespec) -> c_int {
    Sys::clock_getres(clock_id, unsafe { Out::nullable(res) })
        .map(|()| 0)
        .or_minus_one_errno()
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock_getres.html>.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn clock_gettime(clock_id: clockid_t, tp: *mut timespec) -> c_int {
    Sys::clock_gettime(clock_id, unsafe { Out::nonnull(tp) })
        .map(|()| 0)
        .or_minus_one_errno()
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock_nanosleep.html>.
// #[unsafe(no_mangle)]
#[expect(unused_variables, reason = "function not yet implemented")]
pub extern "C" fn clock_nanosleep(
    clock_id: clockid_t,
    flags: c_int,
    rqtp: *const timespec,
    rmtp: *mut timespec,
) -> c_int {
    unimplemented!();
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock_getres.html>.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn clock_settime(clock_id: clockid_t, tp: *const timespec) -> c_int {
    unsafe { Sys::clock_settime(clock_id, tp) }
        .map(|()| 0)
        .or_minus_one_errno()
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/ctime.html>.
///
/// # Deprecation
/// The `ctime()` function was marked obsolescent in the Open Group Base
/// Specifications Issue 7.
#[deprecated]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ctime(clock: *const time_t) -> *mut c_char {
    unsafe {
        #[expect(deprecated)]
        asctime(localtime(clock))
    }
}

/// See <https://pubs.opengroup.org/onlinepubs/9699919799/functions/ctime.html>.
///
/// # Deprecation
/// The `ctime_r()` function was marked obsolescent in the Open Group Base
/// Specifications Issue 7, and removed in Issue 8.
#[deprecated]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ctime_r(clock: *const time_t, buf: *mut c_char) -> *mut c_char {
    // Using MaybeUninit<tm> seems to cause a panic during the build process
    let mut tm1 = blank_tm();
    unsafe { localtime_r(clock, &raw mut tm1) };
    unsafe {
        #[expect(deprecated)]
        asctime_r(&raw const tm1, buf)
    }
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/difftime.html>.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn difftime(time1: time_t, time0: time_t) -> c_double {
    (time1 - time0) as _
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/getdate.html>.
// #[unsafe(no_mangle)]
#[expect(unused_variables, reason = "function not yet implemented")]
pub unsafe extern "C" fn getdate(string: *const c_char) -> *const tm {
    unimplemented!();
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/gmtime.html>.
///
/// # Safety
/// The caller is required to ensure that:
/// * `timer` is a valid pointer
/// * the function has exclusive access to the static `tm` structure it
///   returns. This includes avoiding simultaneous calls to this function as
///   well as to [`localtime()`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gmtime(timer: *const time_t) -> *mut tm {
    // SAFETY: the caller is required to uphold the safety requirements for
    // `gmtime_r()` in addition to exclusive access to
    // `GMTIME_LOCALTIME_RETURN_TM`.
    unsafe { gmtime_r(timer, GMTIME_LOCALTIME_RETURN_TM.as_mut_ptr()) }
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/gmtime.html>.
///
/// # Safety
/// The caller is required to ensure that:
/// * `timer` is a valid pointer
/// * `result` is convertible to an [`Out<tm>`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gmtime_r(timer: *const time_t, result: *mut tm) -> *mut tm {
    // SAFETY: the caller is required to ensure that `timer` is a valid pointer.
    let timer_val = unsafe { *timer };
    let Some(t) = utc_tm(timer_val) else {
        platform::ERRNO.set(EOVERFLOW);
        return ptr::null_mut();
    };
    // SAFETY: the caller is required to ensure that `result` is convertible to an `Out<tm>`.
    unsafe { Out::nonnull(result) }.write(t);
    result
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/localtime.html>.
///
/// # Safety
/// The caller is required to ensure that:
/// * `timer` is a valid pointer
/// * the function has exclusive access to the static `tm` structure it
///   returns. This implies avoiding simultaneous calls to this function as
///   well as to [`gmtime()`]
/// * the variables [`daylight`], [`timezone`] and [`tzname`] are not accessed
///   by user code for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn localtime(timer: *const time_t) -> *mut tm {
    // SAFETY: the caller is required to uphold the safety requirements for
    // `localtime_r()` in addition to exclusive access to
    // `GMTIME_LOCALTIME_RETURN_TM`.
    unsafe { localtime_r(timer, GMTIME_LOCALTIME_RETURN_TM.as_mut_ptr()) }
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/localtime.html>.
///
/// # Safety
/// The caller is required to ensure that:
/// * `timer` is a valid pointer
/// * `result` is convertible to an [`Out<tm>`]
/// * the variables [`daylight`], [`timezone`] and [`tzname`] are not accessed
///   by user code for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn localtime_r(timer: *const time_t, result: *mut tm) -> *mut tm {
    // nife: local time is UTC (see `utc_tm`).
    unsafe { tzset() };
    unsafe { gmtime_r(timer, result) }
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/mktime.html>.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mktime(timeptr: *mut tm) -> time_t {
    // nife: local time is UTC (see `utc_tm`).
    unsafe { tzset() };
    unsafe { timegm(timeptr) }
}

// FIXME seems redox-rt sys posix_nanosleep calls wrapper which disables signals
/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/nanosleep.html>.
///
/// Causes the current thread to be suspended from execution until either the
/// time interval specified by `rqtp` has elapsed or a signal is delivered to
/// the calling thread, and its action is to invoke a signal-catching function
/// or to terminate the process.
///
/// Has no effect on the action or blockage of any signal.
///
/// Upon success, returns `0`. Upon failure, returns `-1` and sets errno to
/// indicate the error. If `rmtp` is non-NULL and `nanosleep()` is interrupted
/// by a signal, returns `-1` and updates `rmtp` to contain the requested time
/// minus the actually elapsed time. If `rmtp` is NULL the remaining time is
/// not returned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nanosleep(rqtp: *const timespec, rmtp: *mut timespec) -> c_int {
    unsafe { Sys::nanosleep(rqtp, rmtp) }
        .map(|()| 0)
        .or_minus_one_errno()
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strftime.html>.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strftime(
    s: *mut c_char,
    maxsize: size_t,
    format: *const c_char,
    timeptr: *const tm,
) -> size_t {
    let mut w = platform::StringWriter(s, maxsize);
    let ret = unsafe { strftime::strftime(&mut w, format, timeptr) };
    if ret < maxsize { ret } else { 0 }
}

// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strftime.html>.
// TODO: needs locale_t
// #[unsafe(no_mangle)]
/*pub extern "C" fn strftime_l(s: *mut char, maxsize: size_t, format: *const c_char, timeptr: *const tm, locale: locale_t) -> size_t {
    unimplemented!();
}*/

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/time.html>.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn time(tloc: *mut time_t) -> time_t {
    let mut ts = timespec::default();
    if Sys::clock_gettime(CLOCK_REALTIME, Out::from_mut(&mut ts)).is_ok() {}; // TODO what to do if Err?
    if !tloc.is_null() {
        unsafe { *tloc = ts.tv_sec }
    };
    ts.tv_sec
}

/// Non-POSIX, see <https://www.man7.org/linux/man-pages/man3/timegm.3.html>.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn timegm(tm: *mut tm) -> time_t {
    // SAFETY: the caller passes a valid `tm`.
    let tm_val = unsafe { &mut *tm };
    let Some(secs) = utc_from_tm(tm_val) else {
        platform::ERRNO.set(EOVERFLOW);
        return -1;
    };
    // `mktime` normalizes the fields it was given (a 32nd of January becomes the 1st of
    // February) and fills in the weekday and the day of the year.
    match utc_tm(secs) {
        Some(t) => *tm_val = t,
        None => {
            platform::ERRNO.set(EOVERFLOW);
            return -1;
        }
    }
    secs
}

/// Non-POSIX, see <https://www.man7.org/linux/man-pages/man3/timegm.3.html>.
#[deprecated]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn timelocal(tm: *mut tm) -> time_t {
    unsafe { timegm(tm) }
}

/// ISO C equivalent to [`Sys::clock_gettime`].
///
/// The main differences are that this function:
/// * returns `0` on error and `base` on success
/// * only mandates TIME_UTC as a base
///
/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/timespec_get.html>.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn timespec_get(tp: *mut timespec, base: c_int) -> c_int {
    let tp = unsafe { Out::nonnull(tp) };
    Sys::clock_gettime(base - 1, tp).map(|()| base).unwrap_or(0)
}

/// ISO C equivalent to [`Sys::clock_getres`].
///
/// The main differences are that this function:
/// * returns `0` on error and `base` on success
/// * only mandates TIME_UTC as a base
#[unsafe(no_mangle)]
pub unsafe extern "C" fn timespec_getres(res: *mut timespec, base: c_int) -> c_int {
    let res = unsafe { Out::nullable(res) };
    Sys::clock_getres(base - 1, res).map(|()| base).unwrap_or(0)
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/tzset.html>.
///
/// # Safety
/// The caller must ensure that [`daylight`], [`timezone`] and [`tzname`] are
/// not accessed by user code for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tzset() {
    // nife: always UTC, whatever `TZ` says (see `utc_tm`).
    unsafe {
        tzname.0[0] = UTC_STR.as_ptr().cast_mut().cast();
        tzname.0[1] = UTC_STR.as_ptr().cast_mut().cast();
        timezone = 0;
        daylight = 0;
    }
}

/// nife: a `tm` in UTC, from the in-tree `calendar` crate, in place of relibc's `chrono` and
/// `chrono-tz`. nife has no time-zone database (`/usr/share/zoneinfo`), so local time is UTC and
/// `TZ` is not read (c_library/README.md, BUGS). `calendar` covers years 0 to 9999, and a time
/// outside them is `EOVERFLOW`.
fn utc_tm(timer: time_t) -> Option<tm> {
    let c = calendar::Civil::from_unix(timer).ok()?;
    let mut t = blank_tm();
    t.tm_sec = c_int::from(c.second());
    t.tm_min = c_int::from(c.minute());
    t.tm_hour = c_int::from(c.hour());
    t.tm_mday = c_int::from(c.day());
    t.tm_mon = c_int::from(c.month()) - 1;
    t.tm_year = c.year() - 1900;
    t.tm_wday = c_int::from(c.weekday().iso_number() % 7);
    t.tm_yday = c_int::from(c.day_of_year()) - 1;
    t.tm_isdst = 0;
    t.tm_gmtoff = 0;
    t.tm_zone = UTC_STR.as_ptr().cast();
    Some(t)
}

/// The UTC time a `tm` names, with every field allowed out of range the way `mktime` allows it.
fn utc_from_tm(t: &tm) -> Option<time_t> {
    let months = i64::from(t.tm_year) * 12 + i64::from(t.tm_mon);
    let year = i32::try_from(1900 + months.div_euclid(12)).ok()?;
    let month = u8::try_from(months.rem_euclid(12) + 1).ok()?;
    let first = calendar::Civil::new(year, month, 1, 0, 0, 0).ok()?;
    let days = first.days_since_epoch() + i64::from(t.tm_mday) - 1;
    days.checked_mul(SECS_PER_DAY)?
        .checked_add(i64::from(t.tm_hour) * 3600)?
        .checked_add(i64::from(t.tm_min) * 60)?
        .checked_add(i64::from(t.tm_sec))
}

const fn blank_tm() -> tm {
    tm {
        tm_year: 0,
        tm_mon: 0,
        tm_mday: 0,
        tm_hour: 0,
        tm_min: 0,
        tm_sec: 0,
        tm_wday: 0,
        tm_yday: 0,
        tm_isdst: -1,
        tm_gmtoff: 0,
        tm_zone: ptr::null_mut(),
    }
}

pub(crate) fn timespec_realtime_to_monotonic(abstime: &timespec) -> Result<timespec, Errno> {
    let mut realtime = timespec::default();
    unsafe { clock_gettime(CLOCK_REALTIME, &raw mut realtime) };
    let mut monotonic = timespec::default();
    unsafe { clock_gettime(CLOCK_MONOTONIC, &raw mut monotonic) };
    let Some(delta) = timespec::subtract(abstime, &realtime) else {
        return Err(Errno(ETIMEDOUT));
    };
    let Some(relative) = timespec::add(&monotonic, &delta) else {
        return Err(Errno(ENOMEM));
    };
    Ok(relative)
}
