//! The heap's host tests, over a page source that hands out host memory.
//!
//! Name: provisional, minted 2026-11-20 (UTC) by lane/1100-memory-region-heap-outstanding, after
//! the module every other crate's tests live in.

extern crate std;

use std::alloc::{alloc_zeroed, dealloc};
use std::cell::RefCell;
use std::rc::Rc;
use std::vec::Vec;

use super::*;
use crate::DEFAULT_BASE_VIRTUAL_ADDRESS;

/// `moho_abi::Error::OutOfMemory` and `BadPointer` as the kernel returns them.
const OUT_OF_MEMORY: i64 = moho_abi::Error::OutOfMemory as i64;
const BAD_POINTER: i64 = moho_abi::Error::BadPointer as i64;

/// A page-aligned block of host memory that stands in for the virtual range a heap maps into:
/// its address is the heap's base, so what the heap writes through a "mapped" address lands
/// here. Declared before the heap in each test, so it outlives every use.
struct Arena {
    base: *mut u8,
    pages: u64,
}

impl Arena {
    fn new(pages: u64) -> Self {
        let layout = Layout::from_size_align((pages * PAGE_BYTES) as usize, 4096).unwrap();
        // SAFETY: the layout has a nonzero size.
        let base = unsafe { alloc_zeroed(layout) };
        assert!(!base.is_null());
        Arena { base, pages }
    }

    fn base_virtual_address(&self) -> u64 {
        self.base as u64
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        let layout = Layout::from_size_align((self.pages * PAGE_BYTES) as usize, 4096).unwrap();
        // SAFETY: allocated in `new` with the same layout.
        unsafe { dealloc(self.base, layout) };
    }
}

/// What the fake region saw and what it will answer.
struct Region {
    /// Every address `map_page` was asked for, in order, whether or not it was mapped.
    asked: Vec<u64>,
    /// Pages it will map before it starts refusing.
    pages_before_refusal: u64,
    /// What it answers once it refuses.
    refusal_code: i64,
    mapped: u64,
}

/// A page source over a [`Region`] the test keeps a handle to.
struct FakePages(Rc<RefCell<Region>>);

impl PageSource for FakePages {
    fn map_page(&mut self, virtual_address: u64) -> Result<(), Refusal> {
        let mut region = self.0.borrow_mut();
        region.asked.push(virtual_address);
        if region.mapped >= region.pages_before_refusal {
            return Err(Refusal {
                code: region.refusal_code,
            });
        }
        region.mapped += 1;
        Ok(())
    }
}

/// A heap over `arena`, capped at `maximum_pages`, whose region pays for `budget_pages` pages and
/// then refuses with `refusal_code`.
fn heap_over(
    arena: &Arena,
    maximum_pages: u64,
    budget_pages: u64,
    refusal_code: i64,
) -> (MemoryRegionHeap<FakePages>, Rc<RefCell<Region>>) {
    let region = Rc::new(RefCell::new(Region {
        asked: Vec::new(),
        pages_before_refusal: budget_pages,
        refusal_code,
        mapped: 0,
    }));
    let heap = MemoryRegionHeap::with_source(FakePages(region.clone()));
    heap.initialize_source(arena.base_virtual_address(), maximum_pages * PAGE_BYTES)
        .unwrap();
    (heap, region)
}

/// A heap with a region that never runs out inside the cap.
fn heap_with_plenty(arena: &Arena, maximum_pages: u64) -> MemoryRegionHeap<FakePages> {
    heap_over(arena, maximum_pages, u64::MAX, OUT_OF_MEMORY).0
}

fn layout(size: usize, align: usize) -> Layout {
    Layout::from_size_align(size, align).unwrap()
}

/// `heap`'s next grow is at the cap and asks its source nothing.
fn commit_everything(heap: &MemoryRegionHeap<FakePages>) {
    assert_eq!(heap.commit_all(), Ok(()));
}

/// Only the first `initialize` wires the heap. A second call, before or after allocations,
/// changes neither the base nor the cap nor what is committed, and says so.
#[test]
fn a_second_initialize_changes_nothing() {
    let heap = MemoryRegionHeap::new();
    assert!(!heap.is_initialized());
    assert_eq!(
        heap.initialize(3, DEFAULT_BASE_VIRTUAL_ADDRESS, 16 * PAGE_BYTES),
        Ok(())
    );
    assert!(heap.is_initialized());
    assert_eq!(
        heap.initialize(
            4,
            DEFAULT_BASE_VIRTUAL_ADDRESS + 64 * PAGE_BYTES,
            8 * PAGE_BYTES
        ),
        Err(WiringRefusal::AlreadyWired)
    );

    let mut g = heap.lock();
    let inner = g.inner();
    assert_eq!(
        (inner.source.slot, inner.base, inner.maximum),
        (3, DEFAULT_BASE_VIRTUAL_ADDRESS, 16 * PAGE_BYTES)
    );
    inner.committed = 4 * PAGE_BYTES; // as after a grow
    drop(g);

    assert_eq!(
        heap.initialize(
            3,
            DEFAULT_BASE_VIRTUAL_ADDRESS + 64 * PAGE_BYTES,
            16 * PAGE_BYTES
        ),
        Err(WiringRefusal::AlreadyWired)
    );
    assert_eq!(
        heap.committed_bytes(),
        4 * PAGE_BYTES,
        "a repeated call does not reset it"
    );
}

/// A refused wiring leaves the heap unwired, so a corrected call can still wire it.
#[test]
fn a_refused_initialize_leaves_the_heap_without_a_wiring() {
    let heap = MemoryRegionHeap::new();
    let refused = |base, bytes| heap.initialize(3, base, bytes);
    assert_eq!(
        refused(DEFAULT_BASE_VIRTUAL_ADDRESS + 8, 16 * PAGE_BYTES),
        Err(WiringRefusal::BaseNotPageAligned)
    );
    assert_eq!(
        refused(DEFAULT_BASE_VIRTUAL_ADDRESS, PAGE_BYTES - 1),
        Err(WiringRefusal::MaximumUnderOnePage),
        "a cap of 4,095 bytes rounds to zero"
    );
    assert_eq!(
        refused(DEFAULT_BASE_VIRTUAL_ADDRESS, 0),
        Err(WiringRefusal::MaximumUnderOnePage)
    );
    assert_eq!(
        refused(u64::MAX - (PAGE_BYTES - 1), 2 * PAGE_BYTES),
        Err(WiringRefusal::RangeOverflows)
    );
    assert!(!heap.is_initialized());
    assert_eq!(refused(DEFAULT_BASE_VIRTUAL_ADDRESS, PAGE_BYTES), Ok(()));
    assert!(heap.is_initialized());
}

/// A request for zero bytes at the cap maps nothing, so it cannot have grown the heap, and
/// `grow` must not claim it did: `alloc` retries after a `true`, and a retry that changes
/// nothing would loop forever.
#[test]
fn a_grow_of_nothing_at_the_maximum_is_a_refusal() {
    let arena = Arena::new(16);
    let heap = heap_with_plenty(&arena, 16);
    commit_everything(&heap);
    let mut g = heap.lock();
    let inner = g.inner();
    assert!(!MemoryRegionHeap::grow(inner, 0));
    // The control: a real request past the cap is refused too, as before.
    assert!(!MemoryRegionHeap::grow(inner, PAGE_BYTES));
}

/// An allocation at a reached cap returns null instead of retrying: the loop in `alloc`
/// ends, with the cap and not the source as the reason.
#[test]
fn an_allocation_past_the_maximum_is_null_not_a_hang() {
    let arena = Arena::new(16);
    let (heap, region) = heap_over(&arena, 16, u64::MAX, OUT_OF_MEMORY);
    let mut held = Vec::new();
    loop {
        // SAFETY: a nonzero size.
        let p = unsafe { heap.alloc(layout(3000, 8)) };
        if p.is_null() {
            break;
        }
        held.push(p);
    }
    assert!(!held.is_empty());
    assert_eq!(
        heap.committed_bytes(),
        16 * PAGE_BYTES,
        "it used the whole cap"
    );
    assert_eq!(region.borrow().mapped, 16);
    assert_eq!(heap.last_refusal(), None, "the cap refused, not the source");
    // SAFETY: each pointer came from `alloc` with this layout.
    unsafe { heap.dealloc(held.pop().unwrap(), layout(3000, 8)) };
    // SAFETY: a nonzero size.
    let reused = unsafe { heap.alloc(layout(3000, 8)) };
    assert!(!reused.is_null(), "freed space is reusable");
}

/// Growth doubles from the eight-page floor, and the pages are asked for one at a time, in
/// order, from the base.
#[test]
fn growth_doubles_from_eight_pages() {
    let arena = Arena::new(64);
    let (heap, region) = heap_over(&arena, 64, u64::MAX, OUT_OF_MEMORY);
    let mut sizes: Vec<u64> = Vec::new();
    for _ in 0..200 {
        // SAFETY: a nonzero size.
        let p = unsafe { heap.alloc(layout(3000, 8)) };
        if p.is_null() {
            break;
        }
        let pages = heap.committed_bytes() / PAGE_BYTES;
        if sizes.last() != Some(&pages) {
            sizes.push(pages);
        }
    }
    assert_eq!(sizes, [8, 16, 32, 64]);
    let base = arena.base_virtual_address();
    let expected: Vec<u64> = (0..64).map(|i| base + i * PAGE_BYTES).collect();
    assert_eq!(region.borrow().asked, expected);
}

/// A cap that is a power of two still holds the largest allocation that fits beside talc's
/// bookkeeping, and the next chunk size up is refused: the grow is exact, not generous.
#[test]
fn a_power_of_two_maximum_holds_the_largest_allocation_that_fits() {
    let maximum = 16 * PAGE_BYTES as usize;
    let first = talc::min_first_heap_size::<DefaultBinning>();
    let unit = talc::base::CHUNK_UNIT;
    let largest = (maximum - first) / unit * unit - core::mem::size_of::<usize>();

    let arena = Arena::new(16);
    let heap = heap_with_plenty(&arena, 16);
    // SAFETY: a nonzero size.
    let p = unsafe { heap.alloc(layout(largest, 8)) };
    assert!(!p.is_null(), "{largest} bytes fit in {maximum}");
    assert_eq!(heap.committed_bytes(), maximum as u64);

    let arena = Arena::new(16);
    let heap = heap_with_plenty(&arena, 16);
    // SAFETY: a nonzero size.
    assert!(unsafe { heap.alloc(layout(largest + unit, 8)) }.is_null());
}

/// `realloc` in place keeps the pointer; by copy it moves the bytes and frees the old block.
#[test]
fn resizing_in_place_and_by_copy() {
    let arena = Arena::new(16);
    let heap = heap_with_plenty(&arena, 16);
    let small = layout(64, 8);
    // SAFETY: nonzero sizes throughout, each pointer used under the layout it was made with,
    // and `new_size` nonzero.
    unsafe {
        let a = heap.alloc(small);
        let b = heap.alloc(small); // a neighbor above `a`, so `a` cannot grow in place
        for i in 0..64 {
            a.add(i).write(i as u8);
        }
        let shrunk = heap.realloc(a, small, 32);
        assert_eq!(shrunk, a, "a shrink stays where it is");
        let moved = heap.realloc(shrunk, layout(32, 8), 2000);
        assert_ne!(moved, a, "a block boxed in by its neighbor moves");
        for i in 0..32 {
            assert_eq!(moved.add(i).read(), i as u8, "the bytes came along");
        }
        heap.dealloc(moved, layout(2000, 8));
        heap.dealloc(b, small);
        // A lone block with free space above it grows where it stands.
        let c = heap.alloc(small);
        let grown = heap.realloc(c, small, 128);
        assert_eq!(grown, c);
        heap.dealloc(grown, layout(128, 8));
    }
}

/// An allocation whose alignment exceeds talc's grid is aligned, and a zero-size layout, which
/// `GlobalAlloc` forbids but the heap tolerates, still gets a distinct aligned block.
#[test]
fn zero_size_and_over_aligned_layouts() {
    let arena = Arena::new(16);
    let heap = heap_with_plenty(&arena, 16);
    // SAFETY: the zero size is a contract violation the heap defends against (`nonempty`);
    // every pointer is freed under its own layout.
    unsafe {
        let zero = heap.alloc(layout(0, 8));
        assert!(!zero.is_null());
        let zero_again = heap.alloc(layout(0, 8));
        assert_ne!(zero, zero_again);
        let wide = heap.alloc(layout(10, 256));
        assert!(!wide.is_null());
        assert_eq!(wide as usize % 256, 0);
        heap.dealloc(wide, layout(10, 256));
        heap.dealloc(zero_again, layout(0, 8));
        heap.dealloc(zero, layout(0, 8));
    }
}

/// A refusal other than a spent budget costs one page at most: the heap records the answer and
/// never asks again, so a base that collides with another mapping loses one page, not one per
/// allocation.
#[test]
fn a_refused_page_stops_growth_for_good() {
    let arena = Arena::new(16);
    let (heap, region) = heap_over(&arena, 16, 0, BAD_POINTER);
    for _ in 0..3 {
        // SAFETY: a nonzero size.
        assert!(unsafe { heap.alloc(layout(64, 8)) }.is_null());
    }
    assert_eq!(
        region.borrow().asked.len(),
        1,
        "one page asked, then never again"
    );
    assert_eq!(heap.last_refusal(), Some(Refusal { code: BAD_POINTER }));
    assert!(!heap.last_refusal().unwrap().is_budget_spent());
    assert_eq!(heap.committed_bytes(), 0);
    assert_eq!(
        heap.commit_all(),
        Err(Short {
            uncommitted_bytes: 16 * PAGE_BYTES
        })
    );
}

/// A spent budget keeps what it paid for, and the heap asks again on the next allocation that
/// needs more, because the budget may have been refilled.
#[test]
fn a_spent_budget_keeps_the_pages_it_paid_for() {
    let arena = Arena::new(16);
    let (heap, region) = heap_over(&arena, 16, 3, OUT_OF_MEMORY);
    // SAFETY: nonzero sizes.
    unsafe {
        assert!(!heap.alloc(layout(64, 8)).is_null());
        assert_eq!(heap.committed_bytes(), 3 * PAGE_BYTES);
        assert!(heap.last_refusal().unwrap().is_budget_spent());
        let asked = region.borrow().asked.len();
        assert!(heap.alloc(layout(5 * PAGE_BYTES as usize, 8)).is_null());
        assert!(region.borrow().asked.len() > asked, "it asked again");
    }
}

/// `commit_all` maps the whole cap; with a short budget it reports the bytes left unmapped and
/// leaves what mapped usable.
#[test]
fn commit_all_maps_the_maximum_or_reports_the_shortfall() {
    let arena = Arena::new(16);
    let heap = heap_with_plenty(&arena, 16);
    assert_eq!(heap.commit_all(), Ok(()));
    assert_eq!(heap.committed_bytes(), 16 * PAGE_BYTES);

    let arena = Arena::new(16);
    let (heap, _) = heap_over(&arena, 16, 5, OUT_OF_MEMORY);
    assert_eq!(
        heap.commit_all(),
        Err(Short {
            uncommitted_bytes: 11 * PAGE_BYTES
        })
    );
    // SAFETY: a nonzero size.
    assert!(!unsafe { heap.alloc(layout(1000, 8)) }.is_null());
}

/// A heap already at its cap, with a cap of zero, or never wired has nothing to commit, so
/// `commit_all` makes no system call and succeeds; an unwired default heap refuses allocations
/// without one.
#[test]
fn commit_all_with_nothing_left_succeeds() {
    let arena = Arena::new(16);
    let heap = heap_with_plenty(&arena, 16);
    commit_everything(&heap);
    assert_eq!(heap.commit_all(), Ok(()));
    let heap = MemoryRegionHeap::new();
    assert_eq!(
        heap.commit_all(),
        Ok(()),
        "an unwired heap has a cap of zero"
    );
    // SAFETY: a nonzero size. The unwired default source would trap on a system call, so reaching
    // the null return shows none was made.
    assert!(unsafe { heap.alloc(layout(8, 8)) }.is_null());
}

/// The accessors report what `initialize` was given, the cap rounded down to a page.
#[test]
fn the_readers_report_the_maximum_and_the_base() {
    let heap = MemoryRegionHeap::new();
    assert_eq!((heap.maximum_bytes(), heap.base_virtual_address()), (0, 0));
    assert_eq!(
        heap.initialize(3, DEFAULT_BASE_VIRTUAL_ADDRESS, 16 * PAGE_BYTES + 7),
        Ok(())
    );
    assert_eq!(heap.maximum_bytes(), 16 * PAGE_BYTES);
    assert_eq!(heap.base_virtual_address(), DEFAULT_BASE_VIRTUAL_ADDRESS);
    assert!(
        heap.initialize(4, DEFAULT_BASE_VIRTUAL_ADDRESS + PAGE_BYTES, 8 * PAGE_BYTES)
            .is_err()
    );
    assert_eq!(heap.maximum_bytes(), 16 * PAGE_BYTES);
}

/// The smallest cap, one page, holds talc's first heap and a small allocation; the README's
/// "minimum useful cap" is this figure. On a 64-bit target talc's figure is 1,575 bytes.
#[test]
fn the_first_heap_fits_in_one_page() {
    let first = talc::min_first_heap_size::<DefaultBinning>();
    assert!(first < PAGE_BYTES as usize);
    #[cfg(target_pointer_width = "64")]
    assert_eq!(first, 1575);

    let arena = Arena::new(1);
    let heap = heap_with_plenty(&arena, 1);
    // SAFETY: a nonzero size.
    assert!(!unsafe { heap.alloc(layout(1024, 8)) }.is_null());
    assert_eq!(heap.committed_bytes(), PAGE_BYTES);
}

/// For any sequence of grows against a source that refuses after some page, the committed
/// range stays whole pages inside the cap, and a grow asks its source for no more pages than
/// it planned. These are the facts `a_grow_never_commits_past_the_maximum` takes as given.
#[test]
fn committed_stays_inside_the_maximum_whatever_the_source_refuses() {
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for round in 0..200 {
        let maximum_pages = 1 + next() % 40;
        let budget = next() % 60;
        let code = if round % 2 == 0 {
            OUT_OF_MEMORY
        } else {
            BAD_POINTER
        };
        let arena = Arena::new(maximum_pages);
        let (heap, region) = heap_over(&arena, maximum_pages, budget, code);
        for _ in 0..12 {
            let need = next() % (maximum_pages * PAGE_BYTES + 3 * PAGE_BYTES);
            let mut g = heap.lock();
            let inner = g.inner();
            let planned = growth_pages(inner.committed, inner.maximum, need)
                .map_or(0, |(_, want_pages)| want_pages);
            let asked_before = region.borrow().asked.len() as u64;
            let _ = MemoryRegionHeap::grow(inner, need);
            let asked = region.borrow().asked.len() as u64 - asked_before;
            assert!(asked <= planned, "asked {asked} pages, planned {planned}");
            assert!(inner.committed <= inner.maximum);
            assert_eq!(inner.committed % PAGE_BYTES, 0);
            assert_eq!(inner.maximum % PAGE_BYTES, 0);
        }
    }
}

/// The bytes a grow asks for cover the request's size and tag, on the first run and after.
#[test]
fn the_bytes_a_grow_asks_for_cover_the_chunk() {
    let unit = talc::base::CHUNK_UNIT;
    // 1 byte: chunk 32 (1 + 8 rounded up to the grid), nothing free at the top.
    assert_eq!(required_bytes(layout(1, 1), unit, 0, 0), unit as u64);
    // A request the free top already covers asks for nothing.
    assert_eq!(required_bytes(layout(1, 1), unit, 0, 4 * unit), 0);
    // The first run adds talc's bookkeeping, and alignment past the grid adds its slack.
    assert_eq!(
        required_bytes(layout(1, 1), unit, 1575, 0),
        unit as u64 + 1575
    );
    assert_eq!(
        required_bytes(layout(1, 256), unit, 0, 0),
        unit as u64 + 256
    );
}
