//! **How far the heap grows**: the pure arithmetic of the growth policy, with no allocator, no lock
//! and no page source in it, so the proofs and host tests reach it directly and a second heap built
//! on the same policy can take this file whole rather than restate it.
//!
//! Every function here depends on `core` alone. The allocator's chunk grid is an argument, not a
//! dependency, so a copy compiles wherever `talc` sits in its crate.
//!
//! Name: provisional, minted 2026-11-20 (UTC) by lane/1100-memory-region-heap-outstanding, for the
//! module and the file.

use core::alloc::Layout;

/// The size of the pages the heap maps, in bytes.
pub(crate) const PAGE_BYTES: u64 = 4096;

/// The smallest growth step, in pages. Growing one page per allocation would make every early
/// `Vec` push a system call; 8 pages (32 KiB) amortizes that without hoarding a small budget.
pub(crate) const MINIMUM_GROW_PAGES: u64 = 8;

/// The pages a grow of `need` bytes plans over a heap of `committed` bytes under a cap of
/// `maximum` bytes (both whole pages, `committed <= maximum`): `(need_pages, want_pages)`, or
/// `None` when the cap is closer than the request.
///
/// Growth is geometric (double the committed range) but never less than the request, never less
/// than [`MINIMUM_GROW_PAGES`], and never past the cap, so a burst of small allocations costs
/// O(log n) system call batches.
pub(crate) fn growth_pages(committed: u64, maximum: u64, need: u64) -> Option<(u64, u64)> {
    let need_pages = need.div_ceil(PAGE_BYTES);
    let want_pages = need_pages
        .max(committed / PAGE_BYTES) // doubling: as many again as we have
        .max(MINIMUM_GROW_PAGES)
        .min(maximum.saturating_sub(committed) / PAGE_BYTES);
    if want_pages < need_pages {
        return None;
    }
    Some((need_pages, want_pages))
}

/// Whether a grow that mapped `got_pages` met a request of `need_pages`. Only a fresh contiguous
/// run at least as big as the request guarantees the retry succeeds; anything less and the honest
/// answer is allocation failure. A run of no pages is never a success, even for a request that
/// needed none: the caller retries after a `true`, and a retry after nothing changed would ask the
/// same question forever.
pub(crate) fn run_succeeded(got_pages: u64, need_pages: u64) -> bool {
    got_pages > 0 && got_pages >= need_pages
}

/// The bytes a fresh run at the top must add for `layout` to fit: its chunk (the size and the
/// allocator's one-word tag, on its `chunk_unit` grid, which is `talc::base::CHUNK_UNIT`), any
/// alignment past that grid, and the allocator's bins on the first run (`first_run_bytes`, zero
/// after it), less what is already free at the top (`free_top_bytes`), which the run merges with.
/// Exact, not generous, because a cap set at a power of two must still hold an allocation of that
/// size.
///
/// The chunk arithmetic restates talc's `required_chunk_size`; were talc to round otherwise, a
/// short grow is followed by another, which doubles, until the cap refuses.
pub(crate) fn required_bytes(
    layout: Layout,
    chunk_unit: usize,
    first_run_bytes: usize,
    free_top_bytes: usize,
) -> u64 {
    let chunk = (layout.size() + core::mem::size_of::<usize>()).next_multiple_of(chunk_unit);
    let over_aligned = if layout.align() > chunk_unit {
        layout.align()
    } else {
        0
    };
    (chunk + over_aligned + first_run_bytes).saturating_sub(free_top_bytes) as u64
}
