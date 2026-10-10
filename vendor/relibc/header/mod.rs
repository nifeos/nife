// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
//! POSIX header implementations.

pub mod assert;
pub mod bits_arpainet;
#[path = "bits_clock-t/mod.rs"]
pub mod bits_clock_t;
#[path = "bits_clockid-t/mod.rs"]
pub mod bits_clockid_t;
#[path = "bits_dev-t/mod.rs"]
pub mod bits_dev_t;
pub mod bits_fcntl;
#[path = "bits_gid-t/mod.rs"]
pub mod bits_gid_t;
#[path = "bits_id-t/mod.rs"]
pub mod bits_id_t;
#[path = "bits_ino-t/mod.rs"]
pub mod bits_ino_t;
#[path = "bits_intptr-t/mod.rs"]
pub mod bits_intptr_t;
pub mod bits_iovec;
#[path = "bits_key-t/mod.rs"]
pub mod bits_key_t;
pub mod bits_limits_ptdi;
#[path = "bits_locale-t/mod.rs"]
pub mod bits_locale_t;
#[path = "bits_mode-t/mod.rs"]
pub mod bits_mode_t;
#[path = "bits_nlink-t/mod.rs"]
pub mod bits_nlink_t;
pub mod bits_null;
#[path = "bits_off-t/mod.rs"]
pub mod bits_off_t;
#[path = "bits_open-flags/mod.rs"]
pub mod bits_open_flags;
#[path = "bits_pid-t/mod.rs"]
pub mod bits_pid_t;
pub mod bits_pthread;
#[path = "bits_pthread-t/mod.rs"]
pub mod bits_pthread_t;
#[path = "bits_pthreadattr-t/mod.rs"]
pub mod bits_pthreadattr_t;
pub mod bits_pthreadoi;
#[path = "bits_pthreadonce-t/mod.rs"]
pub mod bits_pthreadonce_t;
#[path = "bits_reclen-t/mod.rs"]
pub mod bits_reclen_t;
#[path = "bits_safamily-t/mod.rs"]
pub mod bits_safamily_t;
#[path = "bits_sigset-t/mod.rs"]
pub mod bits_sigset_t;
#[path = "bits_size-t/mod.rs"]
pub mod bits_size_t;
#[path = "bits_socklen-t/mod.rs"]
pub mod bits_socklen_t;
#[path = "bits_ssize-t/mod.rs"]
pub mod bits_ssize_t;
#[path = "bits_suseconds-t/mod.rs"]
pub mod bits_suseconds_t;
pub mod bits_sys_stat;
pub mod bits_sys_statvfs;
pub mod bits_threads;
#[path = "bits_time-t/mod.rs"]
pub mod bits_time_t;
#[path = "bits_timer-t/mod.rs"]
pub mod bits_timer_t;
pub mod bits_timespec;
pub mod bits_timeval;
pub mod bits_ucred;
#[path = "bits_uid-t/mod.rs"]
pub mod bits_uid_t;
#[path = "bits_uint32-t/mod.rs"]
pub mod bits_uint32_t;
pub mod bits_uio;
#[path = "bits_useconds-t/mod.rs"]
pub mod bits_useconds_t;
pub mod bits_valist;
#[path = "bits_wchar-t/mod.rs"]
pub mod bits_wchar_t;
pub mod bits_winsize;
pub mod ctype;
// TODO: curses.h (deprecated)
// TODO: devctl.h
pub mod dirent;
pub mod errno;
pub mod fcntl;
pub mod float;
pub mod inttypes;
// iso646.h implemented in C
pub mod langinfo;
pub mod getopt;
pub mod limits;
pub mod locale;
pub mod malloc;
// nife: built unconditionally, where relibc gates it behind its opt-in `math_libm` feature and
// otherwise uses openlibm (vendor/README.md).
pub mod math;
pub mod pthread;
// TODO: stdalign.h (likely C implementation)
pub mod signal;
pub mod stdarg;
// stdatomic.h implemented in C
// stdbool.h implemented in C
pub mod stddef;
// stdint.h implemented in C
pub mod stdio;
pub mod stdlib;
// TODO: stdnoreturn.h (likely C implementation)
pub mod string;
pub mod strings;
pub mod sys_ioctl;
pub mod sys_mman;
pub mod sys_select;
pub mod sys_stat;
pub mod sys_time;
pub mod sys_types;
#[allow(non_camel_case_types)]
pub mod sys_types_extra;
pub mod sys_uio;
pub mod sys_utsname;
pub mod sys_wait;
pub mod time;
// TODO: uchar.h
// TODO: ucontext.h (deprecated)
// TODO: ulimit.h (deprecated)
// TODO: unctrl.h (deprecated)
pub mod unistd;
#[deprecated]
pub mod utime;
// TODO: utmpx.h
// TODO: varargs.h (deprecated)
pub mod wchar;
pub mod wctype;
