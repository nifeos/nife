// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
//! `sys/ioctl.h`: `ioctl`, which on nife controls nothing.
//!
//! nife: relibc's module is Redox's and Linux's terminal and device controls, none of which nife
//! has. A device on nife is a capability whose protocol says what it does, so there is no
//! untyped side channel to it. SQLite's unix VFS includes this header unconditionally and calls
//! `ioctl` only on Linux (`F2FS_IOC_*`), so the function exists and answers `ENOTTY`, POSIX's
//! "this descriptor does not accept that request".

use crate::{
    header::errno::ENOTTY,
    platform::{
        self,
        types::{c_int, c_ulong},
    },
};

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/ioctl.html>.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ioctl(_fd: c_int, _request: c_ulong, _: ...) -> c_int {
    platform::ERRNO.set(ENOTTY);
    -1
}
