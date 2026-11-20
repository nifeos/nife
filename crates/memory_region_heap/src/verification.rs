//! Proofs of the arithmetic `talc` cannot be part of: `talc` is raw pointers over mapped memory,
//! which the prover does not model, so the policy is split into the pure functions of `growth` and
//! proved on those. Each harness calls the function the heap calls.
//!
//! Name: provisional, minted 2026-11-20 (UTC) by lane/1100-memory-region-heap-outstanding, after
//! the module every other crate's harnesses live in.

use core::alloc::Layout;

use crate::growth::*;

/// **A grow never commits past the cap, and keeps the range whole pages.** For any cap, any
/// committed range inside it, any request, and any number of pages the source maps up to the
/// plan (a refused page ends the run early), the committed range after the run is inside the
/// cap and a whole number of pages, and a run that counts as a success covers the request.
///
/// The two `assume`s about `committed` and `maximum` are the type's invariant, held by
/// `wire` and by `grow`'s own addition; the host test
/// `committed_stays_inside_the_maximum_whatever_the_source_refuses` drives sequences of grows
/// against refusing sources and asserts them after each step. The bound on `got` is the loop
/// in `grow`, which maps at most `want_pages`; `a_grow_maps_no_more_pages_than_it_planned`
/// counts the calls.
/// Falsification: replayable `crates/memory_region_heap/falsifications/verification.a_grow_never_commits_past_the_maximum.patch`
#[kani::proof]
fn a_grow_never_commits_past_the_maximum() {
    let committed: u64 = kani::any();
    let maximum: u64 = kani::any();
    let need: u64 = kani::any();
    let got: u64 = kani::any();
    kani::assume(committed.is_multiple_of(PAGE_BYTES) && maximum.is_multiple_of(PAGE_BYTES));
    kani::assume(committed <= maximum);

    let Some((need_pages, want_pages)) = growth_pages(committed, maximum, need) else {
        return; // refused, and `grow` refuses exactly here
    };
    kani::assume(got <= want_pages);

    let after = committed + got * PAGE_BYTES;
    assert!(after <= maximum, "the committed range left the cap");
    assert!(
        after.is_multiple_of(PAGE_BYTES),
        "the range is not whole pages"
    );
    if run_succeeded(got, need_pages) {
        assert!(got * PAGE_BYTES >= need, "a success must cover the request");
    }
}

/// **A grow at the cap is a refusal, whatever the request.** With the cap reached, the plan
/// either refuses or plans no page, and a run of no page never succeeds, so `alloc` cannot
/// retry forever.
/// Falsification: replayable `crates/memory_region_heap/falsifications/verification.a_grow_at_the_maximum_is_a_refusal.patch`
#[kani::proof]
fn a_grow_at_the_maximum_is_a_refusal() {
    let maximum: u64 = kani::any();
    let need: u64 = kani::any();
    let got: u64 = kani::any();
    kani::assume(maximum.is_multiple_of(PAGE_BYTES));

    let Some((need_pages, want_pages)) = growth_pages(maximum, maximum, need) else {
        return;
    };
    assert_eq!(want_pages, 0, "no page is left to plan");
    kani::assume(got <= want_pages);
    assert!(!run_succeeded(got, need_pages));
}

/// **The bytes a grow asks for never overflow, for any valid layout.** `Layout` bounds the size
/// and alignment so that the size rounded up to the alignment fits in an `isize`; the chunk,
/// the alignment slack and talc's first-heap bookkeeping then add without wrapping, and the
/// result covers the request's size and tag unless the free space at the top already does.
///
/// The `assume` bounds the first run's bookkeeping by one page: `talc`'s figure for the default
/// binning is 1,575 bytes on a 64-bit target, and `the_first_heap_fits_in_one_page` asserts it
/// stays under a page.
/// Falsification: replayable `crates/memory_region_heap/falsifications/verification.the_bytes_a_grow_asks_for_never_overflow.patch`
#[kani::proof]
fn the_bytes_a_grow_asks_for_never_overflow() {
    let size: usize = kani::any();
    let align: usize = kani::any();
    let first_run_bytes: usize = kani::any();
    let free_top_bytes: usize = kani::any();
    kani::assume(first_run_bytes <= PAGE_BYTES as usize);
    let Ok(layout) = Layout::from_size_align(size, align) else {
        return; // not a layout the allocator can be given
    };

    let asked = required_bytes(
        layout,
        talc::base::CHUNK_UNIT,
        first_run_bytes,
        free_top_bytes,
    );
    assert!(
        asked as u128 + free_top_bytes as u128
            >= layout.size() as u128 + core::mem::size_of::<usize>() as u128,
        "the run plus the free top must hold the size and its tag"
    );
}
