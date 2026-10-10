//! nife's platform layer: relibc's `Pal`, implemented on nife's Rust `std` (milestone 835 (a C
//! library, stage 1: files, clock and memory), §265 (a C library started from relibc, whose Rust
//! platform layer holds the capabilities)).
//!
//! # Why on `std`, and not on the protocol crates directly
//!
//! §265 asks that this layer "speaks the same protocol crates `std` speaks, so the two runtimes
//! cannot disagree about a wire format". Building it on `std` itself is the strongest form of that:
//! there is one client of the file contract (`std`'s `sys/fs/nife.rs`), one of the clock page, one
//! stdout protocol and one heap, and a C program reaches each through it. Nothing here makes a
//! syscall or names a capability slot. The cost is that `std` is linked into every C program,
//! which is a few hundred KiB of the 496 MiB an image may have; the benefit is that a protocol
//! change lands in one place and a C program and a Rust program granted the same directory cannot
//! see different things in it.
//!
//! # File descriptors
//!
//! nife has none in the kernel. This layer keeps a process-local table from `int` to what a
//! descriptor names: one of the three standard streams, a `std::fs::File`, or a directory
//! (by path). 0, 1 and 2 are the standard streams from the start.
//!
//! # Answers that are fixed rather than asked
//!
//! nife issues no process identifier and attaches no user identity to a process (milestone 49
//! (users, login, and attribution: what identity is for once it stops being authority)). So
//! `getpid` is 0, as `std::process::id` is, and the user and group calls answer 65534, the
//! conventional `nobody`, because 0 would claim root and grant nothing. Each is in
//! `c_library/README.md`'s `BUGS`.
//!
//! # Threads
//!
//! A stage-1 program has one thread, so the table lives in a `static` behind a cell with an `unsafe
//! impl Sync` that says so. Stage 2 (milestone 836 (a C library, stage 2: threads)) replaces it
//! with a lock.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::num::NonZeroU64;
use core::ptr;
use std::io::{Read, Seek, SeekFrom, Write};

use super::types::*;
use super::{Pal, PalSignal};
use crate::c_str::CStr;
use crate::error::{Errno, Result};
use crate::header::bits_sigset_t::sigset_t;
use crate::header::errno::*;
use crate::header::fcntl::*;
use crate::header::signal::{sigaction, siginfo_t, sigval, stack_t};
use crate::header::sys_mman::{MAP_ANONYMOUS, MAP_FIXED};
use crate::header::sys_select::timeval;
use crate::header::sys_stat::{S_IFDIR, S_IFREG, stat};
use crate::header::sys_time::timezone;
use crate::header::sys_uio::iovec;
use crate::header::sys_utsname::{UTSLENGTH, utsname};
use crate::header::time::{CLOCK_MONOTONIC, CLOCK_PROCESS_CPUTIME_ID, CLOCK_REALTIME, timespec};
use crate::header::unistd::{SEEK_CUR, SEEK_END, SEEK_SET};
use crate::out::Out;

/// The answer to every user and group question: `nobody`. See the module comment.
const NOBODY: u32 = 65534;

/// nife's page size on all three architectures (notes/abi.md).
const PAGE: usize = 4096;

/// What one descriptor names.
enum Description {
    Stdin,
    Stdout,
    Stderr,
    File {
        file: std::fs::File,
        path: String,
        flags: c_int,
    },
    Directory {
        path: String,
        flags: c_int,
    },
}

/// The process's descriptor table, and the anonymous mappings `munmap` must give back.
struct Table {
    fds: Vec<Option<Description>>,
    maps: BTreeMap<usize, usize>,
    signals: Signals,
}

struct OneThread(RefCell<Table>);
// SAFETY: a stage-1 process has exactly one thread (the userspace targets are `singlethread`), so
// the table is never reached from two threads. Milestone 836 replaces this with a lock.
unsafe impl Sync for OneThread {}

static TABLE: OneThread = OneThread(RefCell::new(Table {
    fds: Vec::new(),
    maps: BTreeMap::new(),
    signals: Signals {
        actions: [const { None }; NSIG],
        mask: 0,
        pending: 0,
    },
}));

fn with_table<R>(f: impl FnOnce(&mut Table) -> R) -> R {
    let mut t = TABLE.0.borrow_mut();
    if t.fds.is_empty() {
        t.fds.push(Some(Description::Stdin));
        t.fds.push(Some(Description::Stdout));
        t.fds.push(Some(Description::Stderr));
    }
    f(&mut t)
}

fn with_fd<R>(fd: c_int, f: impl FnOnce(&mut Description) -> Result<R>) -> Result<R> {
    with_table(|t| {
        let slot = usize::try_from(fd).ok().and_then(|i| t.fds.get_mut(i));
        match slot {
            Some(Some(d)) => f(d),
            _ => Err(Errno(EBADF)),
        }
    })
}

fn install(t: &mut Table, d: Description) -> c_int {
    let i = match t.fds.iter().position(Option::is_none) {
        Some(i) => {
            t.fds[i] = Some(d);
            i
        }
        None => {
            t.fds.push(Some(d));
            t.fds.len() - 1
        }
    };
    i as c_int
}

// The errno numbers `c_library_errno` produces are the ones `errno.h` declares, or a C program
// would compare against the wrong value. Checked here, at compile time, for every one it uses.
const _: () = {
    use c_library_errno::value as v;
    let pairs = [
        (v::EPERM, EPERM),
        (v::ENOENT, ENOENT),
        (v::E2BIG, E2BIG),
        (v::EINTR, EINTR),
        (v::EIO, EIO),
        (v::EAGAIN, EAGAIN),
        (v::ENOMEM, ENOMEM),
        (v::EACCES, EACCES),
        (v::EBUSY, EBUSY),
        (v::EEXIST, EEXIST),
        (v::EXDEV, EXDEV),
        (v::ENOTDIR, ENOTDIR),
        (v::EISDIR, EISDIR),
        (v::EINVAL, EINVAL),
        (v::ETXTBSY, ETXTBSY),
        (v::EFBIG, EFBIG),
        (v::ENOSPC, ENOSPC),
        (v::ESPIPE, ESPIPE),
        (v::EROFS, EROFS),
        (v::EMLINK, EMLINK),
        (v::EPIPE, EPIPE),
        (v::EDEADLK, EDEADLK),
        (v::ENAMETOOLONG, ENAMETOOLONG),
        (v::ENOSYS, ENOSYS),
        (v::ENOTEMPTY, ENOTEMPTY),
        (v::ELOOP, ELOOP),
        (v::EILSEQ, EILSEQ),
        (v::EADDRINUSE, EADDRINUSE),
        (v::EADDRNOTAVAIL, EADDRNOTAVAIL),
        (v::ENETDOWN, ENETDOWN),
        (v::ENETUNREACH, ENETUNREACH),
        (v::ECONNABORTED, ECONNABORTED),
        (v::ECONNRESET, ECONNRESET),
        (v::ENOTCONN, ENOTCONN),
        (v::ETIMEDOUT, ETIMEDOUT),
        (v::ECONNREFUSED, ECONNREFUSED),
        (v::EHOSTUNREACH, EHOSTUNREACH),
        (v::ESTALE, ESTALE),
        (v::EDQUOT, EDQUOT),
    ];
    let mut i = 0;
    while i < pairs.len() {
        assert!(
            pairs[i].0 == pairs[i].1,
            "c_library_errno and errno.h disagree"
        );
        i += 1;
    }
};

/// `std::io::Error` to `errno`, by meaning. The table itself is `c_library_errno` (provisional),
/// a host-tested crate, so the mapping is proven without an emulator.
pub(crate) fn errno_of(e: &std::io::Error) -> Errno {
    Errno(c_library_errno::errno_for(e.kind()))
}

fn io<T>(r: std::io::Result<T>) -> Result<T> {
    r.map_err(|e| errno_of(&e))
}

/// A C path as UTF-8, which is what nife's `std::fs` takes. A path that is not UTF-8 names nothing
/// nife's filesystem can hold.
fn utf8(path: CStr<'_>) -> Result<&str> {
    core::str::from_utf8(path.to_bytes()).map_err(|_| Errno(EILSEQ))
}

/// Resolve `path` against `dirfd` the way the `*at` calls do.
fn resolve(dirfd: c_int, path: CStr) -> Result<String> {
    let p = utf8(path)?;
    if p.starts_with('/') || dirfd == AT_FDCWD {
        return Ok(String::from(p));
    }
    let base = with_fd(dirfd, |d| match d {
        Description::Directory { path, .. } => Ok(path.clone()),
        _ => Err(Errno(ENOTDIR)),
    })?;
    let mut s = base;
    if !s.ends_with('/') {
        s.push('/');
    }
    s.push_str(p);
    Ok(s)
}

/// A file's serial number, which nife's filesystem contract does not carry: a 64-bit FNV-1a hash
/// of its path. Distinct files get distinct numbers, which is what SQLite's unix VFS keys its
/// per-file state on; a renamed file gets a new one, which a real inode would not.
fn ino_of(path: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in path.trim_start_matches('/').bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn fill_stat(mut buf: Out<stat>, md: &std::fs::Metadata, path: &str) {
    let mode = if md.is_dir() {
        S_IFDIR | 0o755
    } else {
        S_IFREG | 0o644
    };
    let mtime = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .unwrap_or_default();
    let ts = timespec {
        tv_sec: mtime.as_secs() as time_t,
        tv_nsec: c_long::from(mtime.subsec_nanos() as i32),
    };
    // SAFETY: `stat` is plain data, and every field is written below or zero.
    let mut st: stat = unsafe { core::mem::zeroed() };
    st.st_dev = 0;
    st.st_ino = ino_of(path) as ino_t;
    st.st_nlink = 1;
    st.st_mode = mode;
    st.st_uid = NOBODY as uid_t;
    st.st_gid = NOBODY as gid_t;
    st.st_size = md.len() as off_t;
    st.st_blksize = PAGE as blksize_t;
    st.st_blocks = md.len().div_ceil(512) as blkcnt_t;
    st.st_atim = timespec { ..ts };
    st.st_mtim = timespec { ..ts };
    st.st_ctim = ts;
    buf.write(st);
}

fn duration_to_timespec(d: core::time::Duration) -> timespec {
    timespec {
        tv_sec: d.as_secs() as time_t,
        tv_nsec: c_long::from(d.subsec_nanos() as i32),
    }
}

/// The monotonic clock's zero: the first time anything asked. `Instant` has no absolute value on
/// any platform, so `CLOCK_MONOTONIC` counts from here.
fn monotonic() -> core::time::Duration {
    static ZERO: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    ZERO.get_or_init(std::time::Instant::now).elapsed()
}

pub struct Sys;

impl Pal for Sys {
    fn access(path: CStr, mode: c_int) -> Result<()> {
        Self::faccessat(AT_FDCWD, path, mode, 0)
    }

    fn faccessat(fd: c_int, path: CStr, _amode: c_int, _flags: c_int) -> Result<()> {
        // nife's filesystem has no permission bits: a file the grant reaches is readable, and
        // writable when the grant is. Existence is the whole answer here, and a write through a
        // read-only grant fails at the write with `EACCES`.
        let p = resolve(fd, path)?;
        io(std::fs::metadata(&p)).map(|_| ())
    }

    unsafe fn brk(_addr: *mut c_void) -> Result<*mut c_void> {
        // Refused by milestone 835: memory comes from `mmap` and `malloc`, both on the heap.
        Err(Errno(ENOMEM))
    }

    fn chdir(_path: CStr) -> Result<()> {
        // `std::env::set_current_dir` refuses on nife (notes/std.md): a process's root is what it
        // was granted, and it has nowhere else to stand.
        Err(Errno(ENOSYS))
    }

    fn clock_getres(clk_id: clockid_t, tp: Option<Out<timespec>>) -> Result<()> {
        match clk_id {
            CLOCK_REALTIME | CLOCK_MONOTONIC | CLOCK_PROCESS_CPUTIME_ID => {
                if let Some(mut tp) = tp {
                    // `std` does not publish the counter's rate; one nanosecond is the
                    // resolution `timespec` can express, and the true one is coarser (BUGS).
                    tp.write(timespec {
                        tv_sec: 0,
                        tv_nsec: 1,
                    });
                }
                Ok(())
            }
            _ => Err(Errno(EINVAL)),
        }
    }

    fn clock_gettime(clk_id: clockid_t, mut tp: Out<timespec>) -> Result<()> {
        let d = match clk_id {
            CLOCK_REALTIME => std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| Errno(EOVERFLOW))?,
            // nife accounts no CPU time to a process, so the process clock is the monotonic one,
            // which for a one-thread program on an idle core is the same thing (BUGS).
            CLOCK_MONOTONIC | CLOCK_PROCESS_CPUTIME_ID => monotonic(),
            _ => return Err(Errno(EINVAL)),
        };
        tp.write(duration_to_timespec(d));
        Ok(())
    }

    unsafe fn clock_settime(_clk_id: clockid_t, _tp: *const timespec) -> Result<()> {
        Err(Errno(EPERM))
    }

    fn close(fildes: c_int) -> Result<()> {
        with_table(|t| {
            let slot = usize::try_from(fildes).ok().and_then(|i| t.fds.get_mut(i));
            match slot {
                Some(s @ Some(_)) => {
                    *s = None;
                    Ok(())
                }
                _ => Err(Errno(EBADF)),
            }
        })
    }

    fn dup2(_fildes: c_int, _fildes2: c_int) -> Result<c_int> {
        // `std::fs::File::try_clone` is unsupported on nife: a file is a server-side handle and
        // the contract has no duplicate.
        Err(Errno(ENOSYS))
    }

    unsafe fn execve(
        _path: CStr,
        _argv: *const *mut c_char,
        _envp: *const *mut c_char,
    ) -> Result<()> {
        Err(Errno(ENOSYS))
    }

    unsafe fn fexecve(
        _fildes: c_int,
        _argv: *const *mut c_char,
        _envp: *const *mut c_char,
    ) -> Result<()> {
        Err(Errno(ENOSYS))
    }

    fn exit(status: c_int) -> ! {
        // `std::process::exit` runs `std`'s own cleanup, which flushes `std`'s stdout and closes
        // the output stream (notes/sink-protocol.md). The C streams were flushed by `exit()`.
        std::process::exit(status)
    }

    fn fchdir(_fildes: c_int) -> Result<()> {
        Err(Errno(ENOSYS))
    }

    fn fchmodat(_dirfd: c_int, _path: Option<CStr>, _mode: mode_t, _flags: c_int) -> Result<()> {
        // No permission bits to change (see `faccessat`).
        Ok(())
    }

    fn fchownat(
        _fildes: c_int,
        _path: CStr,
        _owner: uid_t,
        _group: gid_t,
        _flags: c_int,
    ) -> Result<()> {
        // No owner to change: nife's filesystem records none.
        Err(Errno(EPERM))
    }

    fn fdatasync(fildes: c_int) -> Result<()> {
        Self::fsync(fildes)
    }

    fn flock(_fd: c_int, _operation: c_int) -> Result<()> {
        Err(Errno(ENOSYS))
    }

    fn fstat(fildes: c_int, buf: Out<stat>) -> Result<()> {
        with_fd(fildes, |d| match d {
            Description::File { file, path, .. } => {
                let md = io(file.metadata())?;
                fill_stat(buf, &md, path);
                Ok(())
            }
            Description::Directory { path, .. } => {
                let md = io(std::fs::metadata(&*path))?;
                fill_stat(buf, &md, path);
                Ok(())
            }
            _ => {
                // The standard streams are character devices with no size.
                let mut b = buf;
                // SAFETY: `stat` is plain data.
                let mut st: stat = unsafe { core::mem::zeroed() };
                st.st_mode = crate::header::sys_stat::S_IFCHR | 0o620;
                st.st_nlink = 1;
                st.st_blksize = PAGE as blksize_t;
                b.write(st);
                Ok(())
            }
        })
    }

    fn fstatat(fildes: c_int, path: Option<CStr>, buf: Out<stat>, flags: c_int) -> Result<()> {
        match path {
            None => Self::fstat(fildes, buf),
            Some(p) if p.to_bytes().is_empty() && flags & AT_EMPTY_PATH != 0 => {
                Self::fstat(fildes, buf)
            }
            Some(p) => {
                let full = resolve(fildes, p)?;
                let md = io(std::fs::metadata(&full))?;
                fill_stat(buf, &md, &full);
                Ok(())
            }
        }
    }

    fn fcntl(fildes: c_int, cmd: c_int, _arg: c_ulonglong) -> Result<c_int> {
        with_fd(fildes, |d| match cmd {
            // No `exec`, so close-on-exec is always true and changes nothing.
            F_GETFD => Ok(FD_CLOEXEC),
            F_SETFD => Ok(0),
            F_GETFL => Ok(match d {
                Description::File { flags, .. } | Description::Directory { flags, .. } => *flags,
                Description::Stdin => O_RDONLY,
                Description::Stdout | Description::Stderr => O_WRONLY,
            }),
            // Record locks: nife's file contract has none, so a lock is refused rather than
            // pretended. A C program that needs one learns it here (`ENOLCK`, "no locks
            // available"), and SQLite is run with its `unix-none` VFS, which takes none (BUGS).
            F_GETLK | F_SETLK | F_SETLKW => Err(Errno(ENOLCK)),
            _ => Err(Errno(EINVAL)),
        })
    }

    unsafe fn fork() -> Result<pid_t> {
        // Declined for good (§264 (`fork` is declined for good, and spawn is the supported way to
        // start a program)): spawn is nife's process creation, and `posix_spawn` is milestone 838
        // (a C library: `posix_spawn`, and no fork)'s.
        Err(Errno(ENOSYS))
    }

    fn fpath(fildes: c_int, out: &mut [u8]) -> Result<usize> {
        with_fd(fildes, |d| match d {
            Description::File { path, .. } | Description::Directory { path, .. } => {
                let n = path.len().min(out.len());
                out[..n].copy_from_slice(&path.as_bytes()[..n]);
                Ok(n)
            }
            _ => Err(Errno(EBADF)),
        })
    }

    fn fsync(fildes: c_int) -> Result<()> {
        with_fd(fildes, |d| match d {
            Description::File { file, .. } => io(file.sync_all()),
            // A directory's entries are durable when the server answers the operation that made
            // them (§27 (the filesystem service)), so there is nothing left to sync.
            Description::Directory { .. } => Ok(()),
            Description::Stdout => io(std::io::stdout().flush()),
            Description::Stderr => io(std::io::stderr().flush()),
            Description::Stdin => Err(Errno(EINVAL)),
        })
    }

    fn ftruncate(fildes: c_int, length: off_t) -> Result<()> {
        let len = u64::try_from(length).map_err(|_| Errno(EINVAL))?;
        with_fd(fildes, |d| match d {
            Description::File { file, .. } => io(file.set_len(len)),
            _ => Err(Errno(EINVAL)),
        })
    }

    unsafe fn futex_wait(_addr: *mut u32, _val: u32, _deadline: Option<&timespec>) -> Result<()> {
        // Only a second thread could make a stage-1 lock wait, and there is none (milestone 836).
        Err(Errno(ENOSYS))
    }

    unsafe fn futex_wake(_addr: *mut u32, _num: u32) -> Result<u32> {
        Ok(0)
    }

    unsafe fn utimensat(
        _dirfd: c_int,
        _path: CStr,
        _times: *const timespec,
        _flag: c_int,
    ) -> Result<()> {
        // `std::fs::File::set_times` needs an open file; nife's contract keeps the modification
        // time itself and refuses to have it set (BUGS).
        Err(Errno(ENOSYS))
    }

    fn getcwd(mut buf: Out<[u8]>) -> Result<()> {
        let cwd = io(std::env::current_dir())?;
        let s = cwd.to_str().ok_or(Errno(EILSEQ))?;
        let bytes = s.as_bytes();
        if bytes.len() + 1 > buf.len() {
            return Err(Errno(ERANGE));
        }
        buf.subslice(0, bytes.len()).copy_from_slice(bytes);
        buf.index(bytes.len()).write(0);
        Ok(())
    }

    fn getdents(_fd: c_int, _buf: &mut [u8], _opaque_offset: u64) -> Result<usize> {
        Err(Errno(ENOSYS))
    }

    fn dir_seek(_fd: c_int, _opaque_offset: u64) -> Result<()> {
        Err(Errno(ENOSYS))
    }

    unsafe fn dent_reclen_offset(_this_dent: &[u8], _offset: usize) -> Option<(u16, u64)> {
        None
    }

    fn getegid() -> gid_t {
        NOBODY as gid_t
    }

    fn geteuid() -> uid_t {
        NOBODY as uid_t
    }

    fn getgid() -> gid_t {
        NOBODY as gid_t
    }

    fn getgroups(_list: Out<[gid_t]>) -> Result<c_int> {
        Ok(0)
    }

    fn getpagesize() -> usize {
        PAGE
    }

    fn getpgid(_pid: pid_t) -> Result<pid_t> {
        Ok(0)
    }

    fn getpid() -> pid_t {
        0
    }

    fn getppid() -> pid_t {
        0
    }

    fn getpriority(_which: c_int, _who: id_t) -> Result<c_int> {
        Ok(0)
    }

    fn getrandom(_buf: &mut [u8], _flags: c_uint) -> Result<usize> {
        // `std::random` panics on nife when the entropy service was not granted (notes/std.md),
        // and a C program must get an error instead. Until `std` can say whether slot 6 is held
        // without panicking, the answer is "not here" (BUGS).
        Err(Errno(ENOSYS))
    }

    fn getresgid(
        rgid: Option<Out<gid_t>>,
        egid: Option<Out<gid_t>>,
        sgid: Option<Out<gid_t>>,
    ) -> Result<()> {
        for mut o in [rgid, egid, sgid].into_iter().flatten() {
            o.write(NOBODY as gid_t);
        }
        Ok(())
    }

    fn getresuid(
        ruid: Option<Out<uid_t>>,
        euid: Option<Out<uid_t>>,
        suid: Option<Out<uid_t>>,
    ) -> Result<()> {
        for mut o in [ruid, euid, suid].into_iter().flatten() {
            o.write(NOBODY as uid_t);
        }
        Ok(())
    }

    fn getsid(_pid: pid_t) -> Result<pid_t> {
        Ok(0)
    }

    fn gettid() -> pid_t {
        // Not 0: a mutex's lock word holds its owner's thread ID and reserves 0 for "unlocked"
        // (`sync/pthread_mutex.rs`). One thread, so one constant.
        1
    }

    fn gettimeofday(mut tp: Out<timeval>, tzp: Option<Out<timezone>>) -> Result<()> {
        let d = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Errno(EOVERFLOW))?;
        tp.write(timeval {
            tv_sec: d.as_secs() as time_t,
            tv_usec: d.subsec_micros() as suseconds_t,
        });
        if let Some(mut tz) = tzp {
            tz.write(timezone {
                tz_minuteswest: 0,
                tz_dsttime: 0,
            });
        }
        Ok(())
    }

    fn getuid() -> uid_t {
        NOBODY as uid_t
    }

    fn linkat(_fd1: c_int, _old: CStr, _fd2: c_int, _new: CStr, _flags: c_int) -> Result<()> {
        // RedoxFS has hard links; nife's file contract does not carry them yet.
        Err(Errno(ENOSYS))
    }

    fn lseek(fildes: c_int, offset: off_t, whence: c_int) -> Result<off_t> {
        let pos = match whence {
            SEEK_SET => SeekFrom::Start(u64::try_from(offset).map_err(|_| Errno(EINVAL))?),
            SEEK_CUR => SeekFrom::Current(offset),
            SEEK_END => SeekFrom::End(offset),
            _ => return Err(Errno(EINVAL)),
        };
        with_fd(fildes, |d| match d {
            Description::File { file, .. } => io(file.seek(pos)).map(|p| p as off_t),
            Description::Directory { .. } => Err(Errno(EISDIR)),
            _ => Err(Errno(ESPIPE)),
        })
    }

    fn mkdirat(fildes: c_int, path: CStr, _mode: mode_t) -> Result<()> {
        let p = resolve(fildes, path)?;
        io(std::fs::create_dir(&p))
    }

    fn mkfifoat(_dir_fd: c_int, _path: CStr, _mode: mode_t) -> Result<()> {
        Err(Errno(ENOSYS))
    }

    fn mknodat(_fildes: c_int, _path: CStr, _mode: mode_t, _dev: dev_t) -> Result<()> {
        Err(Errno(ENOSYS))
    }

    unsafe fn mlock(_addr: *const c_void, _len: usize) -> Result<()> {
        // Nothing on nife is paged out, so every page is already locked.
        Ok(())
    }

    unsafe fn mlockall(_flags: c_int) -> Result<()> {
        Ok(())
    }

    unsafe fn mmap(
        addr: *mut c_void,
        len: usize,
        _prot: c_int,
        flags: c_int,
        _fildes: c_int,
        _off: off_t,
    ) -> Result<*mut c_void> {
        // Anonymous memory only, from the same heap `malloc` uses, which is `std`'s allocator and
        // so `crates/user_mode_heap` over this process's untyped budget (§31 (the foreign-language
        // seam) rule 4). A file mapping needs a shared page from the file
        // server, which the contract does not offer; `ENODEV` is POSIX's answer for "this file
        // cannot be mapped" (BUGS).
        if flags & MAP_ANONYMOUS == 0 {
            return Err(Errno(ENODEV));
        }
        if flags & MAP_FIXED != 0 || !addr.is_null() && flags & MAP_FIXED != 0 {
            return Err(Errno(EINVAL));
        }
        if len == 0 {
            return Err(Errno(EINVAL));
        }
        let size = len.checked_next_multiple_of(PAGE).ok_or(Errno(ENOMEM))?;
        let layout = core::alloc::Layout::from_size_align(size, PAGE).map_err(|_| Errno(ENOMEM))?;
        // SAFETY: `layout` has a non-zero size.
        let p = unsafe { std::alloc::alloc_zeroed(layout) };
        if p.is_null() {
            return Err(Errno(ENOMEM));
        }
        with_table(|t| t.maps.insert(p as usize, size));
        Ok(p.cast())
    }

    unsafe fn mremap(
        _addr: *mut c_void,
        _len: usize,
        _new_len: usize,
        _flags: c_int,
        _args: *mut c_void,
    ) -> Result<*mut c_void> {
        Err(Errno(ENOSYS))
    }

    unsafe fn mprotect(_addr: *mut c_void, _len: usize, _prot: c_int) -> Result<()> {
        // A heap page's protection is the heap's, and this layer cannot change it (BUGS).
        Err(Errno(ENOSYS))
    }

    unsafe fn msync(_addr: *mut c_void, _len: usize, _flags: c_int) -> Result<()> {
        Ok(())
    }

    unsafe fn munlock(_addr: *const c_void, _len: usize) -> Result<()> {
        Ok(())
    }

    unsafe fn madvise(_addr: *mut c_void, _len: usize, _flags: c_int) -> Result<()> {
        Ok(())
    }

    unsafe fn munlockall() -> Result<()> {
        Ok(())
    }

    unsafe fn munmap(addr: *mut c_void, len: usize) -> Result<()> {
        // Whole mappings only: the heap cannot give back part of an allocation.
        let size = with_table(|t| t.maps.get(&(addr as usize)).copied()).ok_or(Errno(EINVAL))?;
        if len.checked_next_multiple_of(PAGE) != Some(size) {
            return Err(Errno(EINVAL));
        }
        with_table(|t| t.maps.remove(&(addr as usize)));
        // SAFETY: `addr` came from `mmap` above with exactly this layout, and is unmapped once.
        unsafe {
            std::alloc::dealloc(
                addr.cast(),
                core::alloc::Layout::from_size_align_unchecked(size, PAGE),
            )
        };
        Ok(())
    }

    unsafe fn nanosleep(rqtp: *const timespec, rmtp: *mut timespec) -> Result<()> {
        // SAFETY: the caller passes a valid `timespec`, as POSIX requires.
        let rq = unsafe { &*rqtp };
        if rq.tv_nsec < 0 || rq.tv_nsec >= 1_000_000_000 || rq.tv_sec < 0 {
            return Err(Errno(EINVAL));
        }
        std::thread::sleep(core::time::Duration::new(
            rq.tv_sec as u64,
            rq.tv_nsec as u32,
        ));
        if !rmtp.is_null() {
            // No signals, so a sleep is never interrupted and nothing remains.
            // SAFETY: non-null, and the caller passes a valid `timespec` when it passes one.
            unsafe {
                ptr::write(
                    rmtp,
                    timespec {
                        tv_sec: 0,
                        tv_nsec: 0,
                    },
                )
            };
        }
        Ok(())
    }

    fn openat(dirfd: c_int, path: CStr, oflag: c_int, _mode: mode_t) -> Result<c_int> {
        let p = resolve(dirfd, path)?;
        let acc = oflag & O_ACCMODE;
        let existing = std::fs::metadata(&p);
        if let Ok(md) = &existing
            && md.is_dir()
        {
            if acc != O_RDONLY {
                return Err(Errno(EISDIR));
            }
            return Ok(with_table(|t| {
                install(
                    t,
                    Description::Directory {
                        path: p,
                        flags: oflag,
                    },
                )
            }));
        }
        if oflag & O_DIRECTORY != 0 {
            return Err(Errno(if existing.is_ok() { ENOTDIR } else { ENOENT }));
        }
        let mut opts = std::fs::OpenOptions::new();
        opts.read(acc == O_RDONLY || acc == O_RDWR)
            .write(acc == O_WRONLY || acc == O_RDWR)
            .append(oflag & O_APPEND != 0)
            .truncate(oflag & O_TRUNC != 0 && acc != O_RDONLY);
        if oflag & O_CREAT != 0 {
            if oflag & O_EXCL != 0 {
                opts.create_new(true);
            } else {
                opts.create(true);
            }
        }
        let file = io(opts.open(&p))?;
        Ok(with_table(|t| {
            install(
                t,
                Description::File {
                    file,
                    path: p,
                    flags: oflag,
                },
            )
        }))
    }

    fn pipe2(_fildes: Out<[c_int; 2]>, _flags: c_int) -> Result<()> {
        Err(Errno(ENOSYS))
    }

    fn posix_fallocate(fd: c_int, offset: u64, length: NonZeroU64) -> Result<()> {
        let want = offset.checked_add(length.get()).ok_or(Errno(EFBIG))?;
        with_fd(fd, |d| match d {
            Description::File { file, .. } => {
                let have = io(file.metadata())?.len();
                if want > have {
                    io(file.set_len(want))
                } else {
                    Ok(())
                }
            }
            _ => Err(Errno(EBADF)),
        })
    }

    fn posix_getdents(_fildes: c_int, _buf: &mut [u8]) -> Result<usize> {
        Err(Errno(ENOSYS))
    }

    fn read(fildes: c_int, buf: &mut [u8]) -> Result<usize> {
        with_fd(fildes, |d| match d {
            Description::File { file, .. } => io(file.read(buf)),
            Description::Stdin => io(std::io::stdin().read(buf)),
            Description::Directory { .. } => Err(Errno(EISDIR)),
            _ => Err(Errno(EBADF)),
        })
    }

    fn pread(fildes: c_int, buf: &mut [u8], offset: off_t) -> Result<usize> {
        let at = u64::try_from(offset).map_err(|_| Errno(EINVAL))?;
        with_fd(fildes, |d| match d {
            Description::File { file, .. } => {
                // The contract has no positioned read, so this is seek, read, seek back, which
                // is atomic only because a stage-1 process has one thread.
                let was = io(file.stream_position())?;
                io(file.seek(SeekFrom::Start(at)))?;
                let mut got = 0;
                let r = loop {
                    match file.read(&mut buf[got..]) {
                        Ok(0) => break Ok(got),
                        Ok(n) => {
                            got += n;
                            if got == buf.len() {
                                break Ok(got);
                            }
                        }
                        Err(e) => break Err(errno_of(&e)),
                    }
                };
                io(file.seek(SeekFrom::Start(was)))?;
                r
            }
            Description::Directory { .. } => Err(Errno(EISDIR)),
            _ => Err(Errno(ESPIPE)),
        })
    }

    unsafe fn readv(fildes: c_int, iov: *const iovec, iovcnt: c_int) -> Result<usize> {
        let mut total = 0;
        for i in 0..usize::try_from(iovcnt).map_err(|_| Errno(EINVAL))? {
            // SAFETY: the caller passes `iovcnt` valid `iovec`s, as POSIX requires.
            let v = unsafe { &*iov.add(i) };
            // SAFETY: each `iovec` names a writable buffer of `iov_len` bytes.
            let b = unsafe { core::slice::from_raw_parts_mut(v.iov_base.cast::<u8>(), v.iov_len) };
            let n = Self::read(fildes, b)?;
            total += n;
            if n < b.len() {
                break;
            }
        }
        Ok(total)
    }

    fn readlinkat(_dirfd: c_int, _pathname: CStr, _out: &mut [u8]) -> Result<usize> {
        // nife's file contract has no symbolic links, so nothing is one.
        Err(Errno(EINVAL))
    }

    fn renameat(old_dir: c_int, old_path: CStr, new_dir: c_int, new_path: CStr) -> Result<()> {
        let a = resolve(old_dir, old_path)?;
        let b = resolve(new_dir, new_path)?;
        io(std::fs::rename(&a, &b))
    }

    fn renameat2(
        old_dir: c_int,
        old_path: CStr,
        new_dir: c_int,
        new_path: CStr,
        flags: c_uint,
    ) -> Result<()> {
        if flags != 0 {
            return Err(Errno(EINVAL));
        }
        Self::renameat(old_dir, old_path, new_dir, new_path)
    }

    fn sched_yield() -> Result<()> {
        std::thread::yield_now();
        Ok(())
    }

    unsafe fn setgroups(_size: size_t, _list: *const gid_t) -> Result<()> {
        Err(Errno(EPERM))
    }

    fn setpgid(_pid: pid_t, _pgid: pid_t) -> Result<()> {
        Err(Errno(EPERM))
    }

    fn setpriority(_which: c_int, _who: id_t, _prio: c_int) -> Result<()> {
        Err(Errno(EPERM))
    }

    fn setresgid(_rgid: gid_t, _egid: gid_t, _sgid: gid_t) -> Result<()> {
        Err(Errno(EPERM))
    }

    fn setresuid(_ruid: uid_t, _euid: uid_t, _suid: uid_t) -> Result<()> {
        Err(Errno(EPERM))
    }

    fn setsid() -> Result<c_int> {
        Err(Errno(EPERM))
    }

    fn symlinkat(_path1: CStr, _fd: c_int, _path2: CStr) -> Result<()> {
        Err(Errno(ENOSYS))
    }

    fn sync() -> Result<()> {
        Ok(())
    }

    fn umask(_mask: mode_t) -> mode_t {
        0o022
    }

    fn uname(mut utsname: Out<utsname>) -> Result<()> {
        fn field(s: &str) -> [c_char; UTSLENGTH] {
            let mut f = [0 as c_char; UTSLENGTH];
            for (d, b) in f.iter_mut().zip(s.bytes().take(UTSLENGTH - 1)) {
                *d = b as c_char;
            }
            f
        }
        utsname.write(utsname {
            sysname: field("nife"),
            nodename: field(""),
            release: field(env!("CARGO_PKG_VERSION")),
            version: field("c_library stage 1"),
            machine: field(std::env::consts::ARCH),
            domainname: field(""),
        });
        Ok(())
    }

    fn unlinkat(fd: c_int, path: CStr, flags: c_int) -> Result<()> {
        let p = resolve(fd, path)?;
        if flags & AT_REMOVEDIR != 0 {
            io(std::fs::remove_dir(&p))
        } else {
            io(std::fs::remove_file(&p))
        }
    }

    fn waitpid(_pid: pid_t, _stat_loc: Option<Out<c_int>>, _options: c_int) -> Result<pid_t> {
        // No children: there is no `fork`, and `posix_spawn` is milestone 838's.
        Err(Errno(ECHILD))
    }

    fn write(fildes: c_int, buf: &[u8]) -> Result<usize> {
        with_fd(fildes, |d| match d {
            Description::File { file, .. } => io(file.write(buf)),
            Description::Stdout => io(std::io::stdout().write(buf)),
            Description::Stderr => io(std::io::stderr().write(buf)),
            Description::Directory { .. } => Err(Errno(EISDIR)),
            Description::Stdin => Err(Errno(EBADF)),
        })
    }

    fn pwrite(fildes: c_int, buf: &[u8], offset: off_t) -> Result<usize> {
        let at = u64::try_from(offset).map_err(|_| Errno(EINVAL))?;
        with_fd(fildes, |d| match d {
            Description::File { file, .. } => {
                // As `pread`: seek, write, seek back, atomic because there is one thread.
                let was = io(file.stream_position())?;
                io(file.seek(SeekFrom::Start(at)))?;
                let r = io(file.write_all(buf)).map(|()| buf.len());
                io(file.seek(SeekFrom::Start(was)))?;
                r
            }
            Description::Directory { .. } => Err(Errno(EISDIR)),
            _ => Err(Errno(ESPIPE)),
        })
    }

    unsafe fn writev(fildes: c_int, iov: *const iovec, iovcnt: c_int) -> Result<usize> {
        let mut total = 0;
        for i in 0..usize::try_from(iovcnt).map_err(|_| Errno(EINVAL))? {
            // SAFETY: the caller passes `iovcnt` valid `iovec`s, as POSIX requires.
            let v = unsafe { &*iov.add(i) };
            // SAFETY: each `iovec` names a readable buffer of `iov_len` bytes.
            let b = unsafe { core::slice::from_raw_parts(v.iov_base.cast::<u8>(), v.iov_len) };
            let n = Self::write(fildes, b)?;
            total += n;
            if n < b.len() {
                break;
            }
        }
        Ok(total)
    }

    fn verify() -> bool {
        true
    }
}

// --- Signals -------------------------------------------------------------------------------------
//
// nife has no signals: no kernel object sends one, and a fault is an event to the process's
// supervisor (§26 (the fault endpoint)), not a handler call inside it. What a C program can still
// observe is its own side of the interface, and that is implemented exactly: `sigaction` records a
// disposition and reports the old one, `sigprocmask` keeps a mask, and `raise` (or `kill` of this
// process) delivers synchronously, before returning, as POSIX requires of `raise` in a
// single-threaded process. A handler for anything else never runs, because nothing else ever
// arrives (c_library/README.md, BUGS). The table lives in the descriptor table's cell, for the same
// one-thread reason.

/// `SIG_DFL` and `SIG_IGN`, as `signal.h` defines them: handler values 0 and 1.
const SIG_DFL: usize = 0;
const SIG_IGN: usize = 1;
/// Signal numbers run 1 to `SIGRTMAX` (64) in Linux's numbering, which `signal.h` uses.
const NSIG: usize = 65;

struct Signals {
    /// Each signal's `sigaction`, as the program last set it. `None` is the default disposition.
    actions: [Option<sigaction>; NSIG],
    /// Blocked signals, bit `n - 1` for signal `n`, as `sigset_t` lays them out.
    mask: sigset_t,
    /// Raised while blocked, delivered when unblocked.
    pending: sigset_t,
}

/// The signal table, which lives in the process's one [`Table`]. Borrowed only to read or update
/// it, never while a handler runs, because a handler may itself call `raise` or `open`.
fn signals<R>(f: impl FnOnce(&mut Signals) -> R) -> R {
    with_table(|t| f(&mut t.signals))
}

fn valid(sig: c_int) -> Result<usize> {
    match usize::try_from(sig) {
        Ok(n) if (1..NSIG).contains(&n) => Ok(n),
        _ => Err(Errno(EINVAL)),
    }
}

fn bit(n: usize) -> sigset_t {
    1 << (n - 1)
}

/// What the default disposition does to signal `n` in a process that cannot be stopped or
/// continued: ignore it, or end the process.
fn default_action(n: usize) {
    use crate::header::signal::{
        SIGABRT, SIGBUS, SIGCHLD, SIGCONT, SIGFPE, SIGILL, SIGQUIT, SIGSEGV, SIGSTOP, SIGSYS,
        SIGTRAP, SIGTSTP, SIGTTIN, SIGTTOU, SIGURG, SIGWINCH, SIGXCPU, SIGXFSZ,
    };
    match n {
        // Ignored by default. The stop signals are here too: there is no job control to stop a
        // nife process for, so stopping is the one default action this layer cannot take.
        SIGCHLD | SIGURG | SIGWINCH | SIGCONT | SIGSTOP | SIGTSTP | SIGTTIN | SIGTTOU => {}
        // "Terminate with a core": `std`'s abort, a breakpoint fault the supervisor sees.
        SIGABRT | SIGBUS | SIGFPE | SIGILL | SIGQUIT | SIGSEGV | SIGSYS | SIGTRAP | SIGXCPU
        | SIGXFSZ => std::process::abort(),
        // "Terminate": the status a shell reports for death by signal `n`.
        _ => std::process::exit(128 + n as i32),
    }
}

/// Deliver signal `n` now: run its handler, ignore it, or take the default action.
fn deliver(n: usize) {
    let action = signals(|s| s.actions[n].clone());
    let handler = action
        .as_ref()
        .and_then(|a| a.sa_handler)
        .map_or(SIG_DFL, |h| h as usize);
    match handler {
        SIG_DFL => default_action(n),
        SIG_IGN => {}
        _ => {
            let a = action.expect("a handler that is not SIG_DFL came from a recorded action");
            // The handler runs with its own signal and its `sa_mask` blocked, as POSIX says, so a
            // `raise` inside it is held until it returns.
            let saved = signals(|s| {
                let saved = s.mask;
                s.mask |= a.sa_mask | bit(n);
                saved
            });
            if a.sa_flags & crate::header::signal::SA_SIGINFO as c_int != 0 {
                // SAFETY: with `SA_SIGINFO` the program stored a three-argument `sa_sigaction`
                // in the same field, which is how `signal.h`'s union lays them out.
                let f: extern "C" fn(c_int, *mut siginfo_t, *mut c_void) =
                    unsafe { core::mem::transmute(handler) };
                // SAFETY: `siginfo_t` is plain data; a signal sent by `raise` has every field
                // zero but its number (and `si_code` 0, `SI_USER`).
                let mut info: siginfo_t = unsafe { core::mem::zeroed() };
                info.si_signo = n as c_int;
                f(n as c_int, &mut info, ptr::null_mut());
            } else {
                // SAFETY: a one-argument handler, as `sa_handler` is typed.
                let f: extern "C" fn(c_int) = unsafe { core::mem::transmute(handler) };
                f(n as c_int);
            }
            signals(|s| s.mask = saved);
            deliver_unblocked();
        }
    }
}

/// Deliver every pending signal the mask no longer blocks, lowest number first.
fn deliver_unblocked() {
    loop {
        let ready = signals(|s| s.pending & !s.mask);
        if ready == 0 {
            return;
        }
        let n = ready.trailing_zeros() as usize + 1;
        signals(|s| s.pending &= !bit(n));
        deliver(n);
    }
}

// `itimerval` is obsolescent in POSIX, and so deprecated in relibc; the trait still names it.
#[expect(deprecated)]
use crate::header::sys_time::itimerval;

#[expect(deprecated)]
impl PalSignal for Sys {
    fn getitimer(_which: c_int, _out: &mut itimerval) -> Result<()> {
        // A timer would deliver `SIGALRM`, and nothing here can deliver a signal unasked.
        Err(Errno(ENOSYS))
    }

    fn kill(pid: pid_t, sig: c_int) -> Result<()> {
        // This process is the only one it can name: there are no process identifiers (`getpid`
        // is 0), so 0 and its own 0 mean itself, and every other number names nothing.
        if pid != 0 {
            return Err(Errno(ESRCH));
        }
        if sig == 0 {
            return Ok(());
        }
        Self::raise(sig)
    }

    fn sigqueue(pid: pid_t, sig: c_int, _val: sigval) -> Result<()> {
        Self::kill(pid, sig)
    }

    fn killpg(pgrp: pid_t, sig: c_int) -> Result<()> {
        Self::kill(pgrp, sig)
    }

    fn raise(sig: c_int) -> Result<()> {
        let n = valid(sig)?;
        let blocked = signals(|s| s.mask & bit(n) != 0);
        if blocked {
            signals(|s| s.pending |= bit(n));
        } else {
            deliver(n);
        }
        Ok(())
    }

    fn setitimer(_which: c_int, _new: &itimerval, _old: Option<&mut itimerval>) -> Result<()> {
        Err(Errno(ENOSYS))
    }

    fn sigaction(sig: c_int, act: Option<&sigaction>, oact: Option<&mut sigaction>) -> Result<()> {
        use crate::header::signal::{SIGKILL, SIGSTOP};
        let n = valid(sig)?;
        signals(|s| {
            if let Some(o) = oact {
                // SAFETY: `sigaction` is plain data and a zeroed one is `SIG_DFL` with no flags.
                *o = s.actions[n]
                    .clone()
                    .unwrap_or(unsafe { core::mem::zeroed() });
            }
            if let Some(a) = act {
                if n == SIGKILL || n == SIGSTOP {
                    return Err(Errno(EINVAL));
                }
                s.actions[n] = Some(a.clone());
            }
            Ok(())
        })
    }

    unsafe fn sigaltstack(_ss: Option<&stack_t>, _old_ss: Option<&mut stack_t>) -> Result<()> {
        // An alternate stack is for a handler that runs on a fault, and on nife a fault goes to
        // the supervisor instead.
        Err(Errno(ENOSYS))
    }

    fn sigpending(set: &mut sigset_t) -> Result<()> {
        *set = signals(|s| s.pending);
        Ok(())
    }

    fn sigprocmask(how: c_int, set: Option<&sigset_t>, oset: Option<&mut sigset_t>) -> Result<()> {
        use crate::header::signal::{SIG_BLOCK, SIG_SETMASK, SIG_UNBLOCK, SIGKILL, SIGSTOP};
        signals(|s| {
            if let Some(o) = oset {
                *o = s.mask;
            }
            if let Some(&new) = set {
                s.mask = match how {
                    SIG_BLOCK => s.mask | new,
                    SIG_UNBLOCK => s.mask & !new,
                    SIG_SETMASK => new,
                    _ => return Err(Errno(EINVAL)),
                } & !(bit(SIGKILL) | bit(SIGSTOP));
            }
            Ok(())
        })?;
        deliver_unblocked();
        Ok(())
    }

    fn sigsuspend(_mask: &sigset_t) -> Errno {
        // It waits for a signal, and none will ever come: refusing is better than hanging forever.
        Errno(ENOSYS)
    }

    fn sigtimedwait(
        set: &sigset_t,
        sig: Option<&mut siginfo_t>,
        tp: Option<&timespec>,
    ) -> Result<c_int> {
        let taken = signals(|s| {
            let ready = s.pending & *set;
            (ready != 0).then(|| {
                let n = ready.trailing_zeros() as usize + 1;
                s.pending &= !bit(n);
                n
            })
        });
        match (taken, tp) {
            (Some(n), _) => {
                if let Some(info) = sig {
                    // SAFETY: plain data; see `deliver`.
                    *info = unsafe { core::mem::zeroed() };
                    info.si_signo = n as c_int;
                }
                Ok(n as c_int)
            }
            // Nothing pending can become pending while this thread sleeps, so the wait is the
            // timeout, and then "nothing arrived".
            (None, Some(t)) => {
                std::thread::sleep(core::time::Duration::new(
                    u64::try_from(t.tv_sec).map_err(|_| Errno(EINVAL))?,
                    u32::try_from(t.tv_nsec).map_err(|_| Errno(EINVAL))?,
                ));
                Err(Errno(EAGAIN))
            }
            (None, None) => Err(Errno(ENOSYS)),
        }
    }
}
