//! The test kernel fails one thread's Nth retype, answering exactly as a region with nothing left
//! would: the spending function returns `None` with the region untouched, and the syscall returns
//! `OutOfMemory` (milestone 757 (a test kernel fails a process on its Nth retype)). Sweeping N from
//! 1 fails a program at every allocation it makes, in order. Names provisional: the module,
//! [`arm`], [`disarm`], [`Tally`] and [`fails_now`].
//!
//! One retype is one call into `memory_region::split`, `retype_run` or `retype_object_page`, so a
//! `MAP` counts once per page table it builds. The target is a thread, read from the core's
//! current-thread word. Only the test kernel has it (`cfg(feature = "system_tests")`). See
//! notes/test-kernel-seams.md, which has the reasoning.

//! # BUGS
//!
//! - One thread, not a process: a second thread of the same process that spends is not counted.
//! - Only the Nth retype fails, not every one after it, so cleanup that allocates succeeds.
//! - One target at a time: arming again replaces the earlier target.

use core::sync::atomic::{AtomicU64, Ordering};

use crate::thread::ThreadId;

/// No thread is armed. `cpu::NO_TID`, which no live thread ever carries.
const UNARMED: ThreadId = crate::cpu::NO_TID;

static TARGET: AtomicU64 = AtomicU64::new(UNARMED);
static FAIL_AT: AtomicU64 = AtomicU64::new(0);
static SEEN: AtomicU64 = AtomicU64::new(0);
static REFUSED: AtomicU64 = AtomicU64::new(0);

/// What one armed run saw, from [`disarm`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tally {
    /// How many times the target spent, or tried to spend, a page of one of its regions.
    pub seen: u64,
    /// Whether the Nth arrived and was refused. `false` means the run made fewer than N retypes,
    /// which is how a sweep knows it has passed the last one.
    pub refused: bool,
}

/// **Fail `target`'s `n`th retype from now on** (`n` counts from 1). `u64::MAX` counts without ever
/// failing, which is how a test learns how many retypes an exchange makes.
pub fn arm(target: ThreadId, n: u64) {
    assert!(n >= 1, "retype_fault::arm counts from 1");
    assert_ne!(target, UNARMED, "retype_fault::arm needs a real thread");
    // Disarm first, so a spender racing this on another core sees either the old target with its
    // old count or nothing, never the new target with a stale count.
    TARGET.store(UNARMED, Ordering::SeqCst);
    SEEN.store(0, Ordering::SeqCst);
    REFUSED.store(0, Ordering::SeqCst);
    FAIL_AT.store(n, Ordering::SeqCst);
    TARGET.store(target, Ordering::SeqCst);
}

/// Stop counting, and say what the run saw.
pub fn disarm() -> Tally {
    TARGET.store(UNARMED, Ordering::SeqCst);
    Tally {
        seen: SEEN.load(Ordering::SeqCst),
        refused: REFUSED.load(Ordering::SeqCst) != 0,
    }
}

/// **Whether this retype is the one to fail.** Called at the top of each spending function in
/// `memory_region`, before the region lock is taken, so a refusal leaves the region exactly as an
/// exhausted one would be left: untouched.
#[inline]
pub(crate) fn fails_now() -> bool {
    let target = TARGET.load(Ordering::Acquire);
    if target == UNARMED || crate::sched::current() != target {
        return false;
    }
    let n = SEEN.fetch_add(1, Ordering::AcqRel) + 1;
    if n == FAIL_AT.load(Ordering::Acquire) {
        REFUSED.store(1, Ordering::Release);
        return true;
    }
    false
}
