//! **The heap underneath every allocation**: one spin-locked region, grown a page run at a time,
//! and the allocation from it that the thread caches refill from.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

pub(super) const PAGE: u64 = 4096;

pub(super) const MIN_GROW_PAGES: u64 = 8;

pub(super) type Talc = talc::base::Talc<talc::source::Manual, talc::base::binning::DefaultBinning>;

pub(super) struct Inner {
    pub(super) heap: Talc,
    /// Where talc's one heap ends, which the next grow extends; `None` before the first claim.
    heap_end: Option<NonNull<u8>>,
    committed: u64,
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
/// untyped in slot 0. Same growth policy as `memory_region_heap`: geometric, floor of 8 pages,
/// capped by `rt::HEAP_MAX`; only a fresh run big enough for the request counts as success.
pub(super) fn grow(inner: &mut Inner, need: u64) -> bool {
    let need_pages = need.div_ceil(PAGE);
    let want_pages = need_pages
        .max(inner.committed / PAGE)
        .max(MIN_GROW_PAGES)
        .min((rt::HEAP_MAX - inner.committed) / PAGE);
    if want_pages < need_pages {
        return false;
    }
    let start = rt::HEAP_BASE + inner.committed;
    let mut got: u64 = 0;
    while got < want_pages {
        let va = start + got * PAGE;
        // SAFETY: plain syscall; the kernel validates the slot, the va, and the budget.
        if unsafe { rt::invoke(rt::MEMORY_REGION_SLOT, abi::memory_region::MAP, va, 0, 0) } != 0 {
            break;
        }
        got += 1;
    }
    if got > 0 {
        inner.committed += got * PAGE;
        let run = crate::ptr::with_exposed_provenance_mut::<u8>(start as usize);
        let bytes = (got * PAGE) as usize;
        // SAFETY: the range was just mapped writable for this process alone, adjacent to the heap
        // `heap_end` ends, or the first run of all, which `heap_alloc`'s `need` sizes for talc's bins.
        inner.heap_end = unsafe {
            match inner.heap_end {
                Some(end) => Some(inner.heap.extend(end, run.wrapping_add(bytes))),
                None => inner.heap.claim(run, bytes),
            }
        };
    }
    got >= need_pages
}

/// A block from the heap itself, growing it as needed; `None` when the region is spent. `layout` is
/// what [`heap_layout`] made of the request, so it is never empty.
pub(super) fn heap_alloc(inner: &mut Inner, layout: Layout) -> Option<NonNull<u8>> {
    loop {
        // SAFETY: `layout` is nonempty (`heap_layout`).
        if let Some(p) = unsafe { inner.heap.try_allocate(layout) } {
            return Some(p);
        }
        // Generous, as `memory_region_heap` is: the request, its alignment, a chunk's
        // rounding and tag, and talc's bins on the first run. A short grow is followed by another.
        let mut need = (layout.size() + layout.align() + 2 * talc::base::CHUNK_UNIT) as u64;
        if inner.heap_end.is_none() {
            need += talc::min_first_heap_size::<talc::base::binning::DefaultBinning>() as u64;
        }
        if !grow(inner, need) {
            return None;
        }
    }
}
