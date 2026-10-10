//! `malloc` and its family, on `std`'s global allocator (milestone 835 (a C library, stage 1:
//! files, clock and memory)).
//!
//! relibc's own allocator (dlmalloc over `mmap`) is not seeded. §31 (the foreign-language seam)
//! rule 4 already tied a C program's heap to `crates/user_mode_heap`, and on nife that is
//! what `std`'s `GlobalAlloc` is (notes/std.md), so C and Rust in one process share one heap and
//! one budget.
//!
//! C's `free` is given a pointer and nothing else, and `std::alloc::dealloc` needs the layout back.
//! So every block carries a 16-byte header just below the pointer handed out: the requested size,
//! then the alignment the block was made with. The header is `ALIGN` bytes (at least 16) so the
//! returned pointer keeps the block's alignment.

use core::alloc::Layout;
use core::ptr;

use super::types::c_void;

/// `max_align_t` on all three architectures: what `malloc` promises without being asked.
const MIN_ALIGN: usize = 16;

fn layout_for(size: usize, align: usize) -> Option<Layout> {
    let total = size.checked_add(align)?;
    Layout::from_size_align(total, align).ok()
}

/// Write the header and return the pointer the caller sees.
///
/// # Safety
/// `base` is null or the start of a live block of at least `align + size` bytes, aligned to
/// `align`, with `align >= 16`.
unsafe fn finish(base: *mut u8, size: usize, align: usize) -> *mut c_void {
    if base.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: the block is `align + size` bytes and `align >= 16`, so the header's two words lie
    // inside it, below the returned pointer, at an aligned address.
    unsafe {
        let user = base.add(align);
        user.cast::<usize>().sub(2).write(size);
        user.cast::<usize>().sub(1).write(align);
        user.cast()
    }
}

/// The header of a block `alloc_align` made: (requested size, alignment).
///
/// # Safety
/// `p` is a live, non-null pointer this module returned.
unsafe fn header(p: *mut c_void) -> (usize, usize) {
    // SAFETY: the caller passes a pointer this module returned, so the two words below it are the
    // header `finish` wrote.
    unsafe {
        let words = p.cast::<usize>();
        (words.sub(2).read(), words.sub(1).read())
    }
}

/// `malloc`: `size` bytes at `max_align_t` alignment, or null.
///
/// # Safety
/// None beyond `malloc`'s: the block is freed once, through [`free`] or [`realloc`].
pub unsafe fn alloc(size: usize) -> *mut c_void {
    unsafe { alloc_align(size, MIN_ALIGN) }
}

/// `aligned_alloc` and `memalign`: `size` bytes at `align` (a power of two), or null.
///
/// # Safety
/// As [`alloc`]; `align` is a power of two, which the callers in `stdlib` check.
pub unsafe fn alloc_align(size: usize, align: usize) -> *mut c_void {
    let align = align.max(MIN_ALIGN);
    let Some(layout) = layout_for(size, align) else {
        return ptr::null_mut();
    };
    // SAFETY: `layout` has a non-zero size (it includes the header).
    unsafe { finish(std::alloc::alloc(layout), size, align) }
}

/// `realloc`: the block resized, its alignment kept, or null with the old block intact.
///
/// # Safety
/// `p` is null or a live pointer this module returned, not used again after a non-null return.
pub unsafe fn realloc(p: *mut c_void, size: usize) -> *mut c_void {
    if p.is_null() {
        return unsafe { alloc(size) };
    }
    // SAFETY: `p` is a live block from this module.
    let (old, align) = unsafe { header(p) };
    let (Some(old_layout), Some(_)) = (layout_for(old, align), layout_for(size, align)) else {
        return ptr::null_mut();
    };
    // SAFETY: `base` and `old_layout` are exactly what `alloc_align` allocated with, and the new
    // size (header included) was checked not to overflow a `Layout` above.
    unsafe {
        let base = p.cast::<u8>().sub(align);
        finish(
            std::alloc::realloc(base, old_layout, size + align),
            size,
            align,
        )
    }
}

/// `free`.
///
/// # Safety
/// `p` is null or a live pointer this module returned, freed once.
pub unsafe fn free(p: *mut c_void) {
    if p.is_null() {
        return;
    }
    // SAFETY: `p` is a live block from this module, freed once, so its header is intact and the
    // layout rebuilt from it is the one it was allocated with.
    unsafe {
        let (size, align) = header(p);
        let base = p.cast::<u8>().sub(align);
        std::alloc::dealloc(base, Layout::from_size_align_unchecked(size + align, align));
    }
}

/// `malloc_usable_size`: the size that was asked for, which is what the block may hold.
///
/// # Safety
/// `p` is null or a live pointer this module returned.
pub unsafe fn alloc_usable_size(p: *mut c_void) -> usize {
    if p.is_null() {
        return 0;
    }
    // SAFETY: as `free`.
    unsafe { header(p).0 }
}
