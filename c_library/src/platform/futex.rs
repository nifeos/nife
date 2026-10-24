//! The wait-on-address pair relibc's locks are built on (`Pal::futex_wait` and `futex_wake`), on
//! `std`'s thread parking (milestone 836 (a C library, stage 2: threads)).
//!
//! `std` does not export the kernel futex its own locks use (`notes/futex.md`), so this builds the
//! pair from `thread::park` and `Thread::unpark` as a parking lot: a fixed table of buckets keyed by
//! address, each a short `std::sync::Mutex` over a queue of parked threads. A wait checks the word
//! and queues itself under the bucket's lock, and a wake dequeues under the same lock, so a wake
//! never falls between a waiter's check and its sleep. The word is read `Relaxed` because the bucket
//! lock's release and acquire order the waker's writes before it. notes/c-library.md ("The platform
//! layer's own constraints") has the rest.

//! # BUGS
//!
//! - **A wake costs a second crossing.** The kernel already keys its waiters by address; this
//!   table does it again in userspace, and a woken waiter is woken through its own parking word.
//!   A `std` that exported its futex would remove the table and the extra wake.
//! - **A timed wait burns a core**, as every timed wait in nife's `std` does until the kernel's
//!   `WAIT` takes a deadline (`notes/futex.md`): `park_timeout` yields until its deadline.
//!
//! Name: provisional, lane milestone/836-a-c-library-stage-two-threads (#2009), 2026-10-12 (UTC),
//! for the module and the file. The pull request records no other name considered.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use core::time::Duration;
use std::sync::Mutex;
use std::thread::Thread;

use crate::error::{Errno, Result};
use crate::header::errno::{EAGAIN, EINVAL, ETIMEDOUT};
use crate::header::time::timespec;

/// One parked thread: the word it waits on and how to wake it. A thread waits on one word at a
/// time, so each thread has exactly one of these, made on its first wait and kept.
struct Waiter {
    /// The waited-on address. Written by its own thread only while it is not queued, and read by
    /// a waker only under the bucket lock the thread queued itself with, so `Relaxed` suffices.
    addr: AtomicUsize,
    woken: AtomicBool,
    thread: Thread,
}

/// How many buckets. A power of two, and more than nife's 16 threads a process (`std`'s
/// `THREAD_PAGE_SLOTS`), so threads waiting on different words rarely share a lock.
const BUCKETS: usize = 64;

static TABLE: [Mutex<Vec<Arc<Waiter>>>; BUCKETS] = [const { Mutex::new(Vec::new()) }; BUCKETS];

fn bucket(addr: usize) -> &'static Mutex<Vec<Arc<Waiter>>> {
    // Words are 4-aligned, so the low two bits carry nothing; mix the rest (Fibonacci hashing).
    let h = (addr >> 2).wrapping_mul(0x9e37_79b9_7f4a_7c15_u64 as usize);
    &TABLE[h >> (usize::BITS - BUCKETS.trailing_zeros())]
}

std::thread_local! {
    static ME: Arc<Waiter> = Arc::new(Waiter {
        addr: AtomicUsize::new(0),
        woken: AtomicBool::new(false),
        thread: std::thread::current(),
    });
}

/// `deadline`, an absolute `CLOCK_MONOTONIC` time as relibc passes it, as a [`Duration`] on the
/// same clock as `super::sys::monotonic`.
fn deadline_of(deadline: &timespec) -> Result<Duration> {
    let nanos = u32::try_from(deadline.tv_nsec)
        .ok()
        .filter(|&n| n < 1_000_000_000)
        .ok_or(Errno(EINVAL))?;
    // A deadline before the clock's zero has passed already.
    let secs = u64::try_from(deadline.tv_sec).unwrap_or(0);
    Ok(Duration::new(secs, nanos))
}

/// **Sleep while `*addr == val`**, as [`wait_on`] does, for relibc's locks, which hand over a
/// raw word.
///
/// # Safety
/// `addr` is a live, 4-aligned word that other threads change only atomically.
pub unsafe fn wait(addr: *mut u32, val: u32, deadline: Option<&timespec>) -> Result<()> {
    if addr.is_null() || !addr.is_aligned() {
        return Err(Errno(EINVAL));
    }
    // SAFETY: the caller's contract: a live, aligned word changed only atomically.
    wait_on(unsafe { AtomicU32::from_ptr(addr) }, val, deadline)
}

/// **Sleep while `word` holds `val`**, until a wake of its address or `deadline` (absolute, on
/// `CLOCK_MONOTONIC`). `EAGAIN` if the word already differs, `ETIMEDOUT` if the deadline came
/// first. A return of `Ok` may be spurious, which every caller in relibc allows for.
///
/// Name: provisional, lane milestone/836-a-c-library-stage-two-threads (#2009), 2026-10-12 (UTC).
/// The pull request records no other name considered.
pub fn wait_on(word: &AtomicU32, val: u32, deadline: Option<&timespec>) -> Result<()> {
    let key = word.as_ptr() as usize;
    let deadline = deadline.map(deadline_of).transpose()?;
    let me = ME.with(Arc::clone);
    {
        let mut queue = bucket(key).lock().unwrap_or_else(|e| e.into_inner());
        if word.load(Ordering::Relaxed) != val {
            return Err(Errno(EAGAIN));
        }
        me.addr.store(key, Ordering::Relaxed);
        me.woken.store(false, Ordering::Relaxed);
        queue.push(Arc::clone(&me));
    }
    loop {
        // Acquire: pairs with the waker's release, so what it wrote before waking is seen.
        if me.woken.load(Ordering::Acquire) {
            return Ok(());
        }
        match deadline {
            None => std::thread::park(),
            Some(at) => match at
                .checked_sub(super::sys::monotonic())
                .filter(|d| !d.is_zero())
            {
                Some(left) => std::thread::park_timeout(left),
                None => {
                    // Out of time. Leave the queue, unless a waker has already taken us out of it,
                    // in which case that wake is ours and the wait succeeded.
                    let mut queue = bucket(key).lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(i) = queue.iter().position(|w| Arc::ptr_eq(w, &me)) {
                        queue.swap_remove(i);
                        return Err(Errno(ETIMEDOUT));
                    }
                    return Ok(());
                }
            },
        }
    }
}

/// **Wake up to `n` threads sleeping on `addr`**, oldest first, and answer how many were woken.
/// `addr` is only compared, never read, so any address is safe to pass.
pub fn wake(addr: *mut u32, n: u32) -> u32 {
    let key = addr as usize;
    let mut woken = Vec::new();
    {
        let mut queue = bucket(key).lock().unwrap_or_else(|e| e.into_inner());
        let mut i = 0;
        while i < queue.len() && woken.len() < n as usize {
            if queue[i].addr.load(Ordering::Relaxed) == key {
                // `remove`, not `swap_remove`: the rest keep their order, so waiters are woken
                // oldest first.
                woken.push(queue.remove(i));
            } else {
                i += 1;
            }
        }
    }
    for w in &woken {
        // Release: pairs with the waiter's acquire load of `woken`.
        w.woken.store(true, Ordering::Release);
        w.thread.unpark();
    }
    woken.len() as u32
}
