//! **Each thread's cache of small blocks**: which requests may use it, refilling it on a miss, and
//! giving it back when the thread ends.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// The calling thread's cache, when a request of this shape may use it.
#[inline]
pub(super) fn cache_for(layout: Layout) -> Option<(&'static mut Cache, usize)> {
    let c = class(layout.size(), layout.align())?;
    let cache = tls::allocation_cache();
    // SAFETY: the calling thread's own cache, which nothing else touches (`tls::allocation_cache`),
    // and nothing here allocates while this borrow is live.
    (!cache.is_null()).then(|| (unsafe { &mut *cache }, c))
}

/// A miss: up to eight blocks of the class from the heap under one acquisition, then one of them.
#[cold]
pub(super) fn refill_and_take(cache: &mut Cache, c: usize) -> *mut u8 {
    {
        let mut g = lock();
        let inner = g.inner();
        // SAFETY: each block is the heap's, `class_bytes(c)` long, on the grid, and handed out
        // to nobody else.
        unsafe { cache.refill(c, || heap_alloc(inner, class_layout(c))) };
    }
    cache.take(c).map_or(crate::ptr::null_mut(), NonNull::as_ptr)
}

/// **A thread's end**: give every block its cache holds back to the heap under one acquisition.
/// The caller has already closed the cache, so nothing refills it after this.
pub(crate) fn spill_thread_cache(cache: &mut Cache) {
    let mut g = lock();
    let inner = g.inner();
    // SAFETY: every cached block of class `c` is the heap's and was cached under `class_layout(c)`.
    cache.drain(|c, b| unsafe { inner.heap.deallocate(b.as_ptr(), class_layout(c)) });
}
