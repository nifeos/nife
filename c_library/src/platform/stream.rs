//! Pipes, `socketpair` and `poll` inside one process (milestone 836 (a C library, stage 2:
//! threads)).
//!
//! A threaded C program talks to itself through descriptors (hackbench's thread mode passes every
//! message over a `pipe` or a `socketpair(AF_UNIX, SOCK_STREAM)` and waits with `poll`). Both ends
//! live in one process, so a pipe is a byte queue in its memory behind a lock and a socket pair is
//! two of them crossed. Nothing here makes a system call; blocking is `std`'s `Condvar`, the kernel
//! futex on nife (`notes/futex.md`). `poll` waits on a process-wide generation counter that every
//! pipe change advances, so one sleeping `poll` wakes for any pipe and rescans.

//! # BUGS
//!
//! - **A pipe cannot leave the process.** No `fork` inherits one and no spawn passes one
//!   (milestone 838 (a C library: `posix_spawn`, and no fork)).
//! - **No `SIGPIPE`.** A write with no reader left answers `EPIPE` and raises nothing.
//! - **`poll` wakes for every pipe's change**, not only the ones it watches: cheap only while
//!   pipes are few.
//! - **`AF_UNIX` stream pairs only.** `SOCK_DGRAM` and `SOCK_SEQPACKET` pairs are refused, and
//!   every other socket call is stage 3's (milestone 837 (a C library, stage 3: sockets)).
//!
//! Name: provisional, lane milestone/836-a-c-library-stage-two-threads (#2009), 2026-10-12 (UTC),
//! for the module and the file. The pull request records no other name considered.

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use core::time::Duration;
use std::sync::{Condvar, Mutex, MutexGuard};

use crate::error::{Errno, Result};
use crate::header::errno::{EAGAIN, EPIPE, ETIMEDOUT};
use crate::header::limits::PIPE_BUF;
use crate::header::poll::{
    POLLERR, POLLHUP, POLLIN, POLLNVAL, POLLOUT, POLLRDNORM, POLLWRNORM, pollfd,
};
use crate::header::time::timespec;
use crate::platform::types::{c_int, c_short, time_t};

/// How many bytes a pipe holds before a writer waits: Linux's default, sixteen pages.
const CAPACITY: usize = 16 * 4096;

/// One direction of a stream: a bounded byte queue and who still holds each side of it.
pub(crate) struct Pipe {
    state: Mutex<State>,
    /// Signaled on every change: bytes in, bytes out, a side closed.
    changed: Condvar,
}

struct State {
    bytes: VecDeque<u8>,
    readers: usize,
    writers: usize,
}

impl Pipe {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State {
                bytes: VecDeque::new(),
                readers: 0,
                writers: 0,
            }),
            changed: Condvar::new(),
        })
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // A panic aborts a nife process, so a poisoned lock is never observed; take it anyway.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Wake whoever waits on this pipe, and any `poll`.
    fn changed(&self) {
        self.changed.notify_all();
        advance();
    }
}

/// What a descriptor from `pipe` or `socketpair` holds: the pipe it reads, the pipe it writes, or
/// (for a socket) both. Dropping it, which `close` does, gives up its sides.
pub(crate) struct End {
    rx: Option<Arc<Pipe>>,
    tx: Option<Arc<Pipe>>,
    nonblock: AtomicBool,
    /// A socket answers `fstat` as one, and a pipe end as a FIFO.
    pub(crate) socket: bool,
}

impl End {
    fn new(rx: Option<Arc<Pipe>>, tx: Option<Arc<Pipe>>, socket: bool, nonblock: bool) -> Self {
        if let Some(p) = &rx {
            p.lock().readers += 1;
        }
        if let Some(p) = &tx {
            p.lock().writers += 1;
        }
        Self {
            rx,
            tx,
            nonblock: AtomicBool::new(nonblock),
            socket,
        }
    }

    pub(crate) fn nonblocking(&self) -> bool {
        self.nonblock.load(Ordering::Relaxed)
    }

    pub(crate) fn set_nonblocking(&self, on: bool) {
        self.nonblock.store(on, Ordering::Relaxed);
    }

    /// Whether this end can be read, written, or both, as `O_RDONLY`, `O_WRONLY` or `O_RDWR`.
    pub(crate) fn access(&self) -> c_int {
        use crate::header::fcntl::{O_RDONLY, O_RDWR, O_WRONLY};
        match (self.rx.is_some(), self.tx.is_some()) {
            (true, true) => O_RDWR,
            (false, true) => O_WRONLY,
            _ => O_RDONLY,
        }
    }

    /// **`read`**: what is queued, up to `buf.len()`; 0 once every writer is gone; otherwise wait
    /// for bytes, or `EAGAIN` if this end does not block.
    pub(crate) fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let Some(pipe) = &self.rx else {
            return Err(Errno(crate::header::errno::EBADF));
        };
        if buf.is_empty() {
            return Ok(0);
        }
        let mut st = pipe.lock();
        loop {
            if !st.bytes.is_empty() {
                let n = buf.len().min(st.bytes.len());
                for (d, s) in buf.iter_mut().zip(st.bytes.drain(..n)) {
                    *d = s;
                }
                drop(st);
                pipe.changed();
                return Ok(n);
            }
            if st.writers == 0 {
                return Ok(0);
            }
            if self.nonblocking() {
                return Err(Errno(EAGAIN));
            }
            st = pipe.changed.wait(st).unwrap_or_else(|e| e.into_inner());
        }
    }

    /// **`write`**: all of `buf`, waiting for room as often as it takes. A write of `PIPE_BUF`
    /// bytes or fewer goes in whole, never interleaved with another writer's, as POSIX requires;
    /// a longer one may be split. `EPIPE` once every reader is gone. A non-blocking end writes what
    /// fits now and answers `EAGAIN` if nothing does.
    pub(crate) fn write(&self, buf: &[u8]) -> Result<usize> {
        let Some(pipe) = &self.tx else {
            return Err(Errno(crate::header::errno::EBADF));
        };
        let atomic = buf.len() <= PIPE_BUF as usize;
        let mut done = 0;
        let mut st = pipe.lock();
        while done < buf.len() {
            if st.readers == 0 {
                return if done > 0 {
                    Ok(done)
                } else {
                    Err(Errno(EPIPE))
                };
            }
            let room = CAPACITY - st.bytes.len();
            let want = buf.len() - done;
            let fits = if atomic { room >= want } else { room > 0 };
            if fits {
                let n = want.min(room);
                st.bytes.extend(&buf[done..done + n]);
                done += n;
                drop(st);
                pipe.changed();
                st = pipe.lock();
                continue;
            }
            if self.nonblocking() {
                return if done > 0 {
                    Ok(done)
                } else {
                    Err(Errno(EAGAIN))
                };
            }
            st = pipe.changed.wait(st).unwrap_or_else(|e| e.into_inner());
        }
        Ok(done)
    }

    /// The `poll` events this end shows now, of those asked for in `events` (and the ones POSIX
    /// reports whether asked or not).
    pub(crate) fn revents(&self, events: c_short) -> c_short {
        let mut r = 0;
        if let Some(p) = &self.rx {
            let st = p.lock();
            if !st.bytes.is_empty() {
                r |= events & (POLLIN | POLLRDNORM);
            }
            if st.writers == 0 {
                r |= POLLHUP;
            }
        }
        if let Some(p) = &self.tx {
            let st = p.lock();
            if st.readers == 0 {
                r |= POLLERR;
            } else if CAPACITY - st.bytes.len() >= PIPE_BUF as usize {
                r |= events & (POLLOUT | POLLWRNORM);
            }
        }
        r
    }
}

impl Drop for End {
    fn drop(&mut self) {
        if let Some(p) = &self.rx {
            p.lock().readers -= 1;
            p.changed();
        }
        if let Some(p) = &self.tx {
            p.lock().writers -= 1;
            p.changed();
        }
    }
}

/// **`pipe`**: a read end and a write end of one new pipe.
pub(crate) fn pipe(nonblock: bool) -> (End, End) {
    let p = Pipe::new();
    (
        End::new(Some(Arc::clone(&p)), None, false, nonblock),
        End::new(None, Some(p), false, nonblock),
    )
}

/// **`socketpair(AF_UNIX, SOCK_STREAM)`**: two ends, each reading what the other writes.
pub(crate) fn socket_pair(nonblock: bool) -> (End, End) {
    let a = Pipe::new();
    let b = Pipe::new();
    (
        End::new(Some(Arc::clone(&a)), Some(Arc::clone(&b)), true, nonblock),
        End::new(Some(b), Some(a), true, nonblock),
    )
}

// --- poll ----------------------------------------------------------------------------------------

/// Advanced by every change to any pipe. A `poll` with nothing ready sleeps on it.
static GENERATION: AtomicU32 = AtomicU32::new(0);
/// How many `poll`s are between reading [`GENERATION`] and leaving, so a change wakes only when
/// one might be asleep.
static POLLERS: AtomicUsize = AtomicUsize::new(0);

/// Note a change. `SeqCst` on both words, against [`poll`]'s: either this sees the poller and
/// wakes it, or the poller's later read of the generation sees this advance and does not sleep.
fn advance() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    if POLLERS.load(Ordering::SeqCst) > 0 {
        super::futex::wake(GENERATION.as_ptr(), u32::MAX);
    }
}

/// **`poll`** over the calling process's descriptors. `revents_of` answers one descriptor's
/// events now (`POLLNVAL` for one that is not open); `timeout` is how long to wait, `None` for no
/// limit. It is a `Duration` rather than `poll`'s milliseconds so that `ppoll` and `select`, which
/// are given nanoseconds and microseconds, do not round a short wait down to none.
pub(crate) fn poll(
    fds: &mut [pollfd],
    timeout: Option<Duration>,
    revents_of: impl Fn(c_int, c_short) -> c_short,
) -> Result<c_int> {
    let deadline = timeout.map(|t| {
        let at = super::sys::monotonic() + t;
        timespec {
            tv_sec: at.as_secs() as time_t,
            tv_nsec: at.subsec_nanos().into(),
        }
    });
    loop {
        POLLERS.fetch_add(1, Ordering::SeqCst);
        let seen = GENERATION.load(Ordering::SeqCst);
        let mut ready = 0;
        for pfd in fds.iter_mut() {
            pfd.revents = if pfd.fd < 0 {
                0
            } else {
                revents_of(pfd.fd, pfd.events)
            };
            if pfd.revents != 0 {
                ready += 1;
            }
        }
        if ready > 0 || timeout == Some(Duration::ZERO) {
            POLLERS.fetch_sub(1, Ordering::SeqCst);
            return Ok(ready);
        }
        let r = super::futex::wait_on(&GENERATION, seen, deadline.as_ref());
        POLLERS.fetch_sub(1, Ordering::SeqCst);
        if let Err(Errno(ETIMEDOUT)) = r {
            return Ok(0);
        }
    }
}

/// The events every descriptor that is not a stream shows: a file, a directory or a standard
/// stream never makes a reader or a writer wait, so it is always ready for what it allows.
pub(crate) fn always_ready(events: c_short, readable: bool, writable: bool) -> c_short {
    let mut r = 0;
    if readable {
        r |= events & (POLLIN | POLLRDNORM);
    }
    if writable {
        r |= events & (POLLOUT | POLLWRNORM);
    }
    r
}

/// What `poll` reports for a descriptor that names nothing.
pub(crate) const NOT_OPEN: c_short = POLLNVAL;
