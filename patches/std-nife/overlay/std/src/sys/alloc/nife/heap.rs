//! **The heap underneath every allocation**: one spin-locked region, grown a page run at a time,
//! and the allocation from it that the thread caches refill from.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;
use super::heap_growth::{PAGE_BYTES, growth_pages, required_bytes, run_succeeded};

pub(super) type Talc = talc::base::Talc<talc::source::Manual, talc::base::binning::DefaultBinning>;

pub(super) struct Inner {
    pub(super) heap: Talc,
    /// Where talc's one heap ends, which the next grow extends; `None` before the first claim.
    heap_end: Option<NonNull<u8>>,
    committed: u64,
    /// Set when `MAP` refuses for any reason but a spent budget, which every retry would meet
    /// again (an already-mapped address costs the region a page each time).
    stopped: bool,
}

pub(super) struct SpinHeap {
    locked: crate::sync::atomic::AtomicBool,
    inner: crate::cell::UnsafeCell<Inner>,
}

// SAFETY: all access to `inner` goes through the `locked` spinlock.
unsafe impl Sync for SpinHeap {}

pub(super) static HEAP: SpinHeap = SpinHeap {
    locked: crate::sync::atomic::AtomicBool::new(false),
    inner: crate::cell::UnsafeCell::new(Inner {
        heap: Talc::new(talc::source::Manual),
        heap_end: None,
        committed: 0,
        stopped: false,
    }),
};

pub(super) struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        HEAP.locked.store(false, crate::sync::atomic::Ordering::Release);
    }
}

pub(super) fn lock() -> Guard {
    use crate::sync::atomic::Ordering;
    while HEAP
        .locked
        .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        crate::hint::spin_loop();
    }
    Guard
}

impl Guard {
    #[allow(clippy::mut_from_ref)]
    pub(super) fn inner(&mut self) -> &mut Inner {
        // SAFETY: holding the guard means holding the spinlock; access is exclusive.
        unsafe { &mut *HEAP.inner.get() }
    }
}

/// Map at least `need` more contiguous bytes at the top of the committed range, from the
/// untyped in slot 0, by `memory_region_heap`'s growth policy (`heap_growth`): geometric, floor of
/// 8 pages, capped by `rt::HEAP_MAXIMUM`, and only a fresh run big enough for the request counts as
/// success. A refusal other than a spent budget stops growth for good, as it does there.
pub(super) fn grow(inner: &mut Inner, need: u64) -> bool {
    if inner.stopped {
        return false;
    }
    let Some((need_pages, want_pages)) = growth_pages(inner.committed, rt::HEAP_MAXIMUM, need)
    else {
        return false;
    };
    let start = rt::HEAP_BASE + inner.committed;
    let mut got: u64 = 0;
    while got < want_pages {
        let virtual_address = start + got * PAGE_BYTES;
        // SAFETY: plain system call; the kernel validates the slot, the address, and the budget.
        let code = unsafe {
            rt::trap_call(moho_abi::system_call::memory_region::map(rt::MEMORY_REGION_SLOT, virtual_address))[0]
                as i64
        };
        if code != 0 {
            inner.stopped = moho_abi::Error::try_from(code) != Ok(moho_abi::Error::OutOfMemory);
            break;
        }
        got += 1;
    }
    if got > 0 {
        inner.committed += got * PAGE_BYTES;
        let run = crate::ptr::with_exposed_provenance_mut::<u8>(start as usize);
        let bytes = (got * PAGE_BYTES) as usize;
        // SAFETY: the range was just mapped writable for this process alone, adjacent to the heap
        // `heap_end` ends, or the first run of all, which `shortfall` sizes for talc's bins.
        inner.heap_end = unsafe {
            match inner.heap_end {
                Some(end) => Some(inner.heap.extend(end, run.wrapping_add(bytes))),
                None => inner.heap.claim(run, bytes),
            }
        };
    }
    run_succeeded(got, need_pages)
}

/// What a fresh run must add for `layout` to fit, as `memory_region_heap`'s `shortfall` reckons it:
/// talc's bins on the first run, and what is already free at the top, given to `required_bytes`.
fn shortfall(inner: &Inner, layout: Layout) -> u64 {
    let (first, free_top) = match inner.heap_end {
        // SAFETY: `end` is the heap end talc last returned, and the heap has not changed since.
        Some(end) => (0, end.addr().get() - unsafe { inner.heap.reserved(end) }.up_to.addr().get()),
        None => (talc::min_first_heap_size::<talc::base::binning::DefaultBinning>(), 0),
    };
    required_bytes(layout, talc::base::CHUNK_UNIT, first, free_top)
}

/// A block from the heap itself, growing it as needed; `None` when the region is spent. `layout` is
/// what [`heap_layout`] made of the request, so it is never empty.
pub(super) fn heap_alloc(inner: &mut Inner, layout: Layout) -> Option<NonNull<u8>> {
    loop {
        // SAFETY: `layout` is nonempty (`heap_layout`).
        if let Some(p) = unsafe { inner.heap.try_allocate(layout) } {
            return Some(p);
        }
        if !grow(inner, shortfall(inner, layout)) {
            return None;
        }
    }
}
