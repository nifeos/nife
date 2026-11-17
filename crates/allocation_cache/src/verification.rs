//! **Machine-checked proofs (Kani) of the size-class arithmetic**: every request the cache serves
//! lands in a class whose blocks are big enough and under one grid step too big, and the layout a
//! heap sees is never empty, never smaller and never less aligned than the request. Behind
//! `#[cfg(kani)]`, so only `cargo kani` (script/verify) compiles them. The list walk of `put` and
//! X
//!
//! Name: provisional, after the module every other crate's harnesses live in.

use super::*;

/// **A cacheable request lands in a class that holds it**: the class is a valid index, its blocks
/// are at least `size` bytes and under one grid step more, and a request over [`MAXIMUM_CLASS_BYTES`] or over
/// [`GRID_BYTES`]-aligned has no class.
/// Falsification: replayable `crates/allocation_cache/falsifications/verification.a_request_lands_in_a_class_that_holds_it.patch`
#[kani::proof]
fn a_request_lands_in_a_class_that_holds_it() {
    let size: usize = kani::any();
    let align: usize = kani::any();
    match class(size, align) {
        Some(c) => {
            assert!(c.index() < CLASSES);
            assert!(align <= GRID_BYTES && size <= MAXIMUM_CLASS_BYTES);
            let bytes = class_bytes(c);
            assert!(bytes >= size);
            assert!(bytes > size.saturating_sub(1) && bytes < size.max(1) + GRID_BYTES);
            assert!(bytes.is_multiple_of(GRID_BYTES));
        }
        None => assert!(align > GRID_BYTES || size > MAXIMUM_CLASS_BYTES),
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

/// **A pop returns the block last pushed, and the cached bytes are the sum**: two blocks pushed to
/// one class come back newest first and then `None`, the count follows each step, and the bytes
/// the cache reports are the class's size times the count.
/// Falsification: replayable `crates/allocation_cache/falsifications/verification.a_pop_returns_the_block_last_pushed.patch`
#[kani::proof]
#[kani::unwind(18)]
fn a_pop_returns_the_block_last_pushed() {
    let size: usize = kani::any();
    kani::assume(size <= MAXIMUM_CLASS_BYTES);
    let c = class(size, 1).unwrap();
    // Room for two blocks of the largest class, one after the other.
    let mut arena = [0u128; 2 * CLASSES * 2];
    let base = arena.as_mut_ptr().cast::<u8>();
    let first = NonNull::new(base).unwrap();
    // SAFETY: inside `arena`, one largest class past its start.
    let second = NonNull::new(unsafe { base.add(MAXIMUM_CLASS_BYTES) }).unwrap();
    let mut cache = Cache::new();
    // SAFETY: each block is `class_bytes(c)` writable, grid-aligned bytes of `arena`, disjoint.
    assert!(unsafe { cache.push(c, first) }.is_ok());
    // SAFETY: as above.
    assert!(unsafe { cache.push(c, second) }.is_ok());
    assert!(cache.count(c) == 2);
    assert!(cache.cached_bytes() == 2 * class_bytes(c));
    assert!(cache.pop(c) == Some(second));
    assert!(cache.count(c) == 1);
    assert!(cache.pop(c) == Some(first));
    assert!(cache.pop(c).is_none());
    assert!(cache.cached_bytes() == 0);
}

/// **A class never holds more than [`BLOCKS_PER_CLASS`] blocks**: after that many pushes the next
/// one is refused and handed back unchanged, and the count stays at the bound.
/// Falsification: replayable `crates/allocation_cache/falsifications/verification.a_class_never_holds_more_than_its_bound.patch`
#[kani::proof]
#[kani::unwind(18)]
fn a_class_never_holds_more_than_its_bound() {
    let c = class(1, 1).unwrap();
    let mut arena = [0u128; BLOCKS_PER_CLASS + 1];
    let base = arena.as_mut_ptr().cast::<u8>();
    let mut cache = Cache::new();
    let mut i = 0;
    while i < BLOCKS_PER_CLASS {
        // SAFETY: inside `arena`, one grid step per block.
        let block = NonNull::new(unsafe { base.add(i * GRID_BYTES) }).unwrap();
        // SAFETY: a writable, grid-aligned block of `arena`, disjoint from the others.
        assert!(unsafe { cache.push(c, block) }.is_ok());
        i += 1;
    }
    assert!(cache.count(c) == BLOCKS_PER_CLASS);
    // SAFETY: inside `arena`.
    let extra = NonNull::new(unsafe { base.add(BLOCKS_PER_CLASS * GRID_BYTES) }).unwrap();
    // SAFETY: refused before the block is touched.
    assert!(unsafe { cache.push(c, extra) } == Err(extra));
    assert!(cache.count(c) == BLOCKS_PER_CLASS);
    assert!(cache.cached_bytes() == BLOCKS_PER_CLASS * GRID_BYTES);
    // The arena owns the blocks the cache holds, so the cache is not dropped with them in it.
    core::mem::forget(cache);
}

/// **A push to one class leaves every other class alone**: for any two classes, the class pushed to
/// holds one block, the other holds none unless it is the same class, and the bytes are the one
/// block's size.
/// Falsification: replayable `crates/allocation_cache/falsifications/verification.a_push_to_one_class_leaves_the_others_alone.patch`
#[kani::proof]
#[kani::unwind(18)]
fn a_push_to_one_class_leaves_the_others_alone() {
    let pushed: usize = kani::any();
    let other: usize = kani::any();
    kani::assume(pushed < CLASSES && other < CLASSES);
    let (pushed, other) = (Class::at(pushed), Class::at(other));
    let mut arena = [0u128; CLASSES];
    let block = NonNull::new(arena.as_mut_ptr().cast::<u8>()).unwrap();
    let mut cache = Cache::new();
    // SAFETY: `arena` is 256 writable, grid-aligned bytes, at least `class_bytes(pushed)`.
    assert!(unsafe { cache.push(pushed, block) }.is_ok());
    assert!(cache.count(pushed) == 1);
    assert!(cache.count(other) == usize::from(other == pushed));
    assert!(cache.cached_bytes() == class_bytes(pushed));
    assert!(cache.pop(pushed) == Some(block));
}
