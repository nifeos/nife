//! **The allocator's four entry points**, as `std`'s `System` calls them.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

#[inline]
pub unsafe fn alloc(layout: Layout) -> *mut u8 {
    if let Some((cache, c)) = cache_for(layout) {
        if let Some(p) = cache.take(c) {
            return p.as_ptr();
        }
        return refill_and_take(cache, c);
    }
    let mut g = lock();
    heap_alloc(g.inner(), heap_layout(layout)).map_or(crate::ptr::null_mut(), NonNull::as_ptr)
}

#[inline]
pub unsafe fn alloc_zeroed(layout: Layout) -> *mut u8 {
    // Fresh pages from memory_region::MAP arrive zeroed, but recycled free-list memory does not, so
    // zero unconditionally rather than track provenance.
    let p = unsafe { alloc(layout) };
    if !p.is_null() {
        unsafe { crate::ptr::write_bytes(p, 0, layout.size()) };
    }
    p
}

#[inline]
pub unsafe fn dealloc(ptr: *mut u8, layout: Layout) {
    let Some(p) = NonNull::new(ptr) else {
        return;
    };
    if let Some((cache, c)) = cache_for(layout) {
        // SAFETY: `p` came from `alloc(layout)`, so it is `class_bytes(c)` grid-aligned bytes
        // of the heap's (a cached block is fungible within its class), and it is ours now.
        if let Err(p) = unsafe { cache.put(c, p) } {
            {
                let mut g = lock();
                let inner = g.inner();
                // SAFETY: as `spill_thread_cache`.
                cache.spill(c, |b| unsafe { inner.heap.deallocate(b.as_ptr(), class_layout(c)) });
            }
            // SAFETY: as the first `put`; the spill made room.
            let kept = unsafe { cache.put(c, p) };
            debug_assert!(kept.is_ok());
        }
        return;
    }
    let mut g = lock();
    // SAFETY: forwarded GlobalAlloc contract: `ptr` came from `alloc(layout)` on this heap, which
    // gave the heap `heap_layout(layout)` whether or not a cache served it.
    unsafe { g.inner().heap.deallocate(p.as_ptr(), heap_layout(layout)) };
}

#[inline]
pub unsafe fn realloc(ptr: *mut u8, old_layout: Layout, new_size: usize) -> *mut u8 {
    // In place when neither size is a cached class: a cached block must stay its class's size.
    // This is where talc's grow-in-place pays, on a `Vec` that doubles past the cache's sizes.
    if class(old_layout.size(), old_layout.align()).is_none()
        && class(new_size, old_layout.align()).is_none()
    {
        let mut g = lock();
        // SAFETY: forwarded GlobalAlloc contract: `ptr` is live under `heap_layout(old_layout)`,
        // which is `old_layout` made nonempty, and `new_size`, uncached, is nonzero.
        if unsafe { g.inner().heap.try_realloc_in_place(ptr, heap_layout(old_layout), new_size) } {
            return ptr;
        }
    }
    // SAFETY: the GlobalAlloc realloc contract guarantees `new_size` is valid for the align.
    unsafe {
        let new_layout = Layout::from_size_align_unchecked(new_size, old_layout.align());
        let new_ptr = alloc(new_layout);
        if !new_ptr.is_null() {
            crate::ptr::copy_nonoverlapping(ptr, new_ptr, usize::min(old_layout.size(), new_size));
            dealloc(ptr, old_layout);
        }
        new_ptr
    }
}
