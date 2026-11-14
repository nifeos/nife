//! **Machine-checked proofs (Kani) of the size-class arithmetic**: every request the cache serves
//! lands in a class whose blocks are big enough and under one grid step too big, and the layout a
//! heap sees is never empty, never smaller and never less aligned than the request. Behind
//! `#[cfg(kani)]`, so only `cargo kani` (script/verify) compiles them. The list walk of `put` and
//! `take` is tested against a real heap instead; this module proves the pure functions only.
//!
//! Name: provisional, minted 2026-11-14 (UTC) by lane maintainer/outside-review-fixes-1075-1088,
//! after the module every other crate's harnesses live in.

use super::*;

/// **A cacheable request lands in a class that holds it**: the class is a valid index, its blocks
/// are at least `size` bytes and under one grid step more, and a request over [`LARGEST`] or over
/// [`GRID`]-aligned has no class.
/// Falsification: replayable `crates/allocation_cache/falsifications/verification.a_request_lands_in_a_class_that_holds_it.patch`
#[kani::proof]
fn a_request_lands_in_a_class_that_holds_it() {
    let size: usize = kani::any();
    let align: usize = kani::any();
    match class(size, align) {
        Some(c) => {
            assert!(c < CLASSES);
            assert!(align <= GRID && size <= LARGEST);
            let bytes = class_bytes(c);
            assert!(bytes >= size);
            assert!(bytes > size.saturating_sub(1) && bytes < size.max(1) + GRID);
            assert!(bytes.is_multiple_of(GRID));
        }
        None => assert!(align > GRID || size > LARGEST),
    }
}

/// **[`heap_layout`] never shrinks, empties or unaligns a request**: its alignment is the
/// request's, its size is at least one byte and at least the request's, and a request with a class
/// gets exactly the class's layout.
/// Falsification: replayable `crates/allocation_cache/falsifications/verification.the_heap_layout_is_never_empty_or_smaller.patch`
#[kani::proof]
fn the_heap_layout_is_never_empty_or_smaller() {
    let size: usize = kani::any();
    let shift: u32 = kani::any();
    kani::assume(shift < 16);
    let align = 1usize << shift;
    kani::assume(size <= 1 << 20);
    let layout = Layout::from_size_align(size, align).unwrap();
    let heap = heap_layout(layout);
    assert!(heap.size() >= 1);
    assert!(heap.size() >= layout.size());
    assert!(heap.align() >= layout.align());
    if let Some(c) = class(layout.size(), layout.align()) {
        assert!(heap == class_layout(c));
    }
}
