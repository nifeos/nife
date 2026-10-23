//! The test kernel holds one thread at the start of a delegation or a mapping, so a test can run a
//! revocation sweep while it waits. Names provisional: the module, [`disarm`], [`arm`],
//! [`is_parked`], [`release`] and [`here`].
//!
//! [`here`] sits in `SEND_CAP`, `ThreadControlBlock::CAP_INSERT`, `PageFrame::SLICE`,
//! `PageFrame::MAP`, `AddressSpace::MAP_INTO` and `MemoryRegion::MAP`, just before the critical
//! section that reads the source capability and files the copy. Nothing is held at the seam. Only
//! the test kernel has it: the module and every call are under `cfg(feature = "system_tests")`. See
//! notes/test-kernel-seams.md, which has the protocol and the `BUGS`.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::thread::ThreadId;

const UNARMED: ThreadId = crate::cpu::NO_TID;

static TARGET: AtomicU64 = AtomicU64::new(UNARMED);
static PARKED: AtomicBool = AtomicBool::new(false);
static RELEASED: AtomicBool = AtomicBool::new(false);

/// Forget any earlier pause: no target, nothing parked, nothing released. A test calls this before
/// it starts the thread that will [`arm`], so a [`is_parked`] left `true` by an earlier test is not
/// read as this one's thread arriving.
pub fn disarm() {
    TARGET.store(UNARMED, Ordering::SeqCst);
    PARKED.store(false, Ordering::SeqCst);
    RELEASED.store(false, Ordering::SeqCst);
}

/// **Hold `target` at its next delegation's seam** until [`release`]. Called by the thread itself,
/// after the test has called [`disarm`].
pub fn arm(target: ThreadId) {
    assert_ne!(target, UNARMED, "delegation_pause::arm needs a real thread");
    TARGET.store(target, Ordering::SeqCst);
}

/// Whether the armed thread has reached its seam and is waiting there.
///
/// Name: provisional, minted 2026-10-18 (UTC) by maintainer/names-comply-question-shapes. Refused
/// `parked` (a participle alone reads as a getter).
pub fn is_parked() -> bool {
    PARKED.load(Ordering::SeqCst)
}

/// Let the held delegation continue.
pub fn release() {
    RELEASED.store(true, Ordering::SeqCst);
}

/// **The seam.** A relaxed load and a compare on every delegation of the test kernel; nothing in any
/// other build. Must be called with no lock held, which each call site states.
#[inline]
pub(crate) fn here() {
    let target = TARGET.load(Ordering::Acquire);
    if target == UNARMED || crate::sched::current() != target {
        return;
    }
    TARGET.store(UNARMED, Ordering::SeqCst);
    PARKED.store(true, Ordering::SeqCst);
    while !RELEASED.load(Ordering::SeqCst) {
        crate::sched::yield_now();
    }
}
