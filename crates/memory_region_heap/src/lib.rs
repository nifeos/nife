//! The untyped-backed heap, a [`GlobalAlloc`] paid from the process's own untyped capability
//! (milestone 27 (Rust `std` on the native ABI)). A program declares it (see notes/heap.md):
//!
//! ```ignore
//! #[global_allocator]
//! static HEAP: memory_region_heap::MemoryRegionHeap = memory_region_heap::MemoryRegionHeap::new();
//! // first thing in _start:
//! HEAP.initialize(MEMORY_REGION_SLOT, memory_region_heap::DEFAULT_BASE_VIRTUAL_ADDRESS, 4 * 1024 * 1024);
//! ```
//!
//! Name: ratified 2026-11-14 (UTC) (block 1100 decision A), for the type it holds.

#![no_std]

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, Ordering};

use talc::base::Talc;
use talc::base::binning::DefaultBinning;
use talc::source::Manual;

/// The suggested heap base: 1 GiB, the start of the address-space map's heap band (milestone 206 (a program image has under 896 KiB)).
/// A convention, not a law; a program with its own layout passes its own base to [`initialize`],
/// and one that does should still keep it inside `address_space_layout::HEAP`.
///
/// [`initialize`]: MemoryRegionHeap::initialize
///
/// Name: ratified 2026-11-14 (UTC) (block 1100 decision A). An address is `_VIRTUAL_ADDRESS` in
/// every location.
pub const DEFAULT_BASE_VIRTUAL_ADDRESS: u64 = address_space_layout::HEAP.start;

const PAGE: u64 = 4096;

/// The smallest growth step, in pages. Growing one page per allocation would make every early
/// `Vec` push a syscall; 8 pages (32 KiB) amortizes that without hoarding a small budget.
const MINIMUM_GROW_PAGES: u64 = 8;

struct Inner {
    heap: Talc<Manual, DefaultBinning>,
    /// The end `talc` reports for the one heap it holds, which the next grow extends; `None`
    /// until the first grow claims the first run.
    heap_end: Option<NonNull<u8>>,
    /// Which capability table slot holds the untyped that pays for pages. `u64::MAX` until
    /// `initialize`.
    memory_region_slot: u64,
    /// The heap's virtual range: `[base, base + max)`, of which `[base, base + committed)` is
    /// mapped and donated to the allocator so far.
    base: u64,
    committed: u64,
    maximum: u64,
}

/// What [`MemoryRegionHeap::commit_all`] could not do: the bytes of the capped range that stayed
/// unmapped because the budget ran out.
///
/// Name: provisional (block 1100 decision A, 2026-11-14 UTC).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Short {
    /// Bytes of the cap, a whole number of pages, that the heap could not commit.
    pub uncommitted_bytes: u64,
}

/// The untyped-backed global allocator. One static per program, named in its
/// `#[global_allocator]` attribute; see the module docs for the contract.
///
/// Name: ratified 2026-11-14 (UTC) (block 1100 decision A).
pub struct MemoryRegionHeap {
    locked: AtomicBool,
    inner: core::cell::UnsafeCell<Inner>,
}

// SAFETY: all access to `inner` goes through the `locked` spinlock (see `lock`).
unsafe impl Sync for MemoryRegionHeap {}

/// A held lock on the heap; releases on drop. Internal only.
struct Guard<'a>(&'a MemoryRegionHeap);

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        self.0.locked.store(false, Ordering::Release);
    }
}

impl Guard<'_> {
    fn inner(&mut self) -> &mut Inner {
        // SAFETY: holding the guard means holding the spinlock; access is exclusive.
        unsafe { &mut *self.0.inner.get() }
    }
}

impl MemoryRegionHeap {
    /// An empty, un-wired heap, `const` so it can be the `#[global_allocator]` static.
    ///
    /// Name: ratified 2026-11-14 (UTC) (block 1100 decision A).
    pub const fn new() -> Self {
        MemoryRegionHeap {
            locked: AtomicBool::new(false),
            inner: core::cell::UnsafeCell::new(Inner {
                heap: Talc::new(Manual),
                heap_end: None,
                memory_region_slot: u64::MAX,
                base: 0,
                committed: 0,
                maximum: 0,
            }),
        }
    }

    /// Wire the heap to its budget: pages come from the untyped in `memory_region_slot`, mapped at
    /// `[base, base + maximum_bytes)`. Call once, before the first allocation, first thing in
    /// `_start`. `base` must be page-aligned and inside the user half.
    ///
    /// **Only the first call wires the heap.** A later call changes nothing, because rewiring a heap
    /// that may already hold allocations would leave its allocator owning pages the new base never
    /// mapped, and a later allocation could be handed memory in that hole. A caller that wants to
    /// know asks [`is_initialized`](Self::is_initialized) first.
    ///
    /// Name: ratified 2026-11-14 (UTC) (block 1100 decision A). Spelled out under the `init` ruling
    /// of 2026-10-26 (UTC), in design/naming/acronyms-and-abbreviations.md.
    pub fn initialize(&self, memory_region_slot: u64, base: u64, maximum_bytes: u64) {
        let mut g = self.lock();
        let inner = g.inner();
        if Self::wired(inner) {
            return;
        }
        inner.memory_region_slot = memory_region_slot;
        inner.base = base;
        inner.committed = 0;
        inner.maximum = maximum_bytes & !(PAGE - 1);
    }

    /// Whether [`initialize`](Self::initialize) has run, so that a further call would change
    /// nothing.
    ///
    /// Name: provisional, minted 2026-11-14 (UTC) by maintainer/review-hostile-input-fixes.
    pub fn is_initialized(&self) -> bool {
        let mut g = self.lock();
        Self::wired(g.inner())
    }

    /// Whether the heap has been wired to a budget, or has already taken pages from one.
    fn wired(inner: &Inner) -> bool {
        inner.memory_region_slot != u64::MAX || inner.heap_end.is_some() || inner.committed > 0
    }

    /// **Map the whole capped range now**, rather than on the first allocations that need it.
    /// `Err(`[`Short`]`)` when the budget could not cover it, naming the bytes left unmapped; whatever
    /// was mapped stays usable.
    ///
    /// For a program that carves its budget last-in first-out (the shell's `--mem` grants, pipes and
    /// staging regions come back only while they are the budget's newest carve): committing at
    /// `_start` puts every heap page and its page tables under all of those, so a later grow can
    /// never land above one and strand it. The alternative, a region split off for the heap, is
    /// worse: a region that has been split cannot be destroyed until its child is, so the whole
    /// budget would stay held after the program exits. Milestone 47 (navigation and naming) met
    /// that first: its shell's kernel tests kept a budget's worth of frames each.
    ///
    /// Name: ratified 2026-11-14 (UTC) (block 1100 decision A). The return changed from `bool` to
    /// `Result<(), Short>` under the same ruling.
    pub fn commit_all(&self) -> Result<(), Short> {
        let mut g = self.lock();
        let inner = g.inner();
        let rest = inner.maximum.saturating_sub(inner.committed);
        if rest == 0 || Self::grow(inner, rest) {
            return Ok(());
        }
        Err(Short {
            uncommitted_bytes: inner.maximum.saturating_sub(inner.committed),
        })
    }

    /// Bytes of untyped currently mapped for the heap. The demo workload reports this so the
    /// test can assert growth actually happened (and stayed bounded).
    ///
    /// Name: ratified 2026-11-14 (UTC) (block 1100 decision A).
    pub fn committed_bytes(&self) -> u64 {
        let mut g = self.lock();
        g.inner().committed
    }

    /// The cap in force, in bytes: the `maximum_bytes` given to [`initialize`](Self::initialize),
    /// rounded down to a whole page. Zero until the heap is wired. A later `initialize` does not
    /// change it.
    ///
    /// Name: provisional (block 1100 item 13, decision A, 2026-11-14 UTC).
    pub fn maximum_bytes(&self) -> u64 {
        let mut g = self.lock();
        g.inner().maximum
    }

    /// The virtual address where the heap starts: the `base` given to
    /// [`initialize`](Self::initialize). Zero until the heap is wired. A later `initialize` does
    /// not change it.
    ///
    /// Name: provisional (block 1100 item 13, decision A, 2026-11-14 UTC).
    pub fn base_virtual_address(&self) -> u64 {
        let mut g = self.lock();
        g.inner().base
    }

    fn lock(&self) -> Guard<'_> {
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Guard(self)
    }

    /// Map at least `need` more contiguous bytes at the top of the committed range and donate
    /// them to the allocator. Returns false when the cap or the budget refuses, in which case a
    /// retry cannot succeed and the allocation must fail.
    fn grow(inner: &mut Inner, need: u64) -> bool {
        if inner.memory_region_slot == u64::MAX {
            return false; // initialize() has not run; there is nothing to pay with.
        }
        let need_pages = need.div_ceil(PAGE);
        // Grow geometrically (double the committed range) but never less than the request and
        // never past the cap, so a burst of small allocations costs O(log n) syscall batches.
        let want_pages = need_pages
            .max(inner.committed / PAGE) // doubling: as many again as we have
            .max(MINIMUM_GROW_PAGES)
            .min((inner.maximum - inner.committed) / PAGE);
        if want_pages < need_pages {
            return false; // the cap is closer than the request; growing cannot help.
        }

        let start = inner.base + inner.committed;
        let mut got: u64 = 0;
        while got < want_pages {
            let virtual_address = start + got * PAGE;
            // SAFETY: plain syscall; the kernel validates the slot, the virtual address, and the budget.
            if unsafe {
                user_mode_runtime::invoke(
                    inner.memory_region_slot,
                    abi::memory_region::MAP,
                    virtual_address,
                    0,
                    0,
                )
            } != 0
            {
                break; // budget spent (OutOfMemory); keep what we got.
            }
            got += 1;
        }
        if got > 0 {
            inner.committed += got * PAGE;
            let run = core::ptr::with_exposed_provenance_mut::<u8>(start as usize);
            let bytes = (got * PAGE) as usize;
            // SAFETY: `[start, start + bytes)` was just mapped writable for us alone, and is
            // adjacent to the heap `heap_end` ends, or the first run of all. The first claim
            // fits: a grow is at least `MINIMUM_GROW_PAGES`, far above talc's first-heap size.
            inner.heap_end = unsafe {
                match inner.heap_end {
                    Some(end) => Some(inner.heap.extend(end, run.wrapping_add(bytes))),
                    None => inner.heap.claim(run, bytes),
                }
            };
        }
        // Only a fresh contiguous run at least as big as the request guarantees the retry
        // succeeds; anything less and the honest answer is allocation failure. A run of no pages
        // is never a success, even for a request that needed none: the caller retries after a
        // `true`, and a retry after nothing changed would ask the same question forever.
        got > 0 && got >= need_pages
    }
}

impl Default for MemoryRegionHeap {
    fn default() -> Self {
        Self::new()
    }
}

/// The bytes a fresh run at the top must add for `layout` to fit: its chunk (the size and talc's
/// one-word tag, on talc's 32-byte grid), any alignment past that grid, and talc's bins on the
/// first run, less what is already free at the top, which the run merges with. Exact, not
/// generous, because a cap set at a power of two must still hold an allocation of that size.
/// The chunk arithmetic restates talc's `required_chunk_size`; were talc to round otherwise, a
/// short grow is followed by another, which doubles, until the cap refuses.
fn shortfall(inner: &Inner, layout: Layout) -> u64 {
    let unit = talc::base::CHUNK_UNIT;
    let chunk = (layout.size() + core::mem::size_of::<usize>()).next_multiple_of(unit);
    let over_aligned = if layout.align() > unit {
        layout.align()
    } else {
        0
    };
    let (first, free_top) = match inner.heap_end {
        // SAFETY: `end` is the heap end talc last returned, and the heap has not changed since.
        Some(end) => (
            0,
            end.as_ptr() as usize - unsafe { inner.heap.reserved(end) }.up_to.as_ptr() as usize,
        ),
        None => (talc::min_first_heap_size::<DefaultBinning>(), 0),
    };
    (chunk + over_aligned + first).saturating_sub(free_top) as u64
}

/// `layout` as `talc` is given it: at least one byte, because `talc` takes no empty layout and
/// `alloc` and `dealloc` must agree on the size.
fn nonempty(layout: Layout) -> Layout {
    // SAFETY: the alignment is `layout`'s own, and one byte cannot overflow it.
    unsafe { Layout::from_size_align_unchecked(layout.size().max(1), layout.align()) }
}

// SAFETY: alloc/dealloc uphold the GlobalAlloc contract: unique live pointers, the same layout to
// `talc` at both ends (`nonempty`), and no unwinding (there is none on this target).
unsafe impl GlobalAlloc for MemoryRegionHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let layout = nonempty(layout);
        let mut g = self.lock();
        let inner = g.inner();
        loop {
            // SAFETY: `layout` is nonempty.
            if let Some(p) = unsafe { inner.heap.try_allocate(layout) } {
                return p.as_ptr();
            }
            if !Self::grow(inner, shortfall(inner, layout)) {
                return core::ptr::null_mut();
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }
        let mut g = self.lock();
        // SAFETY: forwarded GlobalAlloc contract: `ptr` came from `alloc(layout)` on this heap,
        // which gave talc the same `nonempty(layout)`.
        unsafe { g.inner().heap.deallocate(ptr, nonempty(layout)) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let old = nonempty(layout);
        {
            let mut g = self.lock();
            // SAFETY: forwarded GlobalAlloc contract: `ptr` is live under `layout`, and
            // `new_size` is nonzero. In place, the pointer and its tag stay talc's.
            if new_size != 0 && unsafe { g.inner().heap.try_realloc_in_place(ptr, old, new_size) } {
                return ptr;
            }
        }
        // SAFETY: the GlobalAlloc realloc contract makes `new_size` valid for this alignment.
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        // SAFETY: as `GlobalAlloc::realloc`'s default body: allocate, copy, free.
        unsafe {
            let new_ptr = self.alloc(new_layout);
            if !new_ptr.is_null() {
                core::ptr::copy_nonoverlapping(ptr, new_ptr, layout.size().min(new_size));
                self.dealloc(ptr, layout);
            }
            new_ptr
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    /// A heap wired to a slot, a base and a cap, with every page of the cap already committed, so
    /// the next `grow` is at the cap and issues no syscall.
    fn heap_at_its_maximum() -> MemoryRegionHeap {
        let heap = MemoryRegionHeap::new();
        heap.initialize(3, DEFAULT_BASE_VIRTUAL_ADDRESS, 16 * PAGE);
        let mut g = heap.lock();
        g.inner().committed = 16 * PAGE;
        drop(g);
        heap
    }

    /// Only the first `initialize` wires the heap. A second call, before or after allocations,
    /// changes neither the base nor the cap nor what is committed.
    #[test]
    fn a_second_initialize_changes_nothing() {
        let heap = MemoryRegionHeap::new();
        assert!(!heap.is_initialized());
        heap.initialize(3, DEFAULT_BASE_VIRTUAL_ADDRESS, 16 * PAGE);
        assert!(heap.is_initialized());
        heap.initialize(4, DEFAULT_BASE_VIRTUAL_ADDRESS + 64 * PAGE, 8 * PAGE);

        let mut g = heap.lock();
        let inner = g.inner();
        assert_eq!(
            (inner.memory_region_slot, inner.base, inner.maximum),
            (3, DEFAULT_BASE_VIRTUAL_ADDRESS, 16 * PAGE)
        );
        inner.committed = 4 * PAGE; // as after a grow
        drop(g);

        heap.initialize(3, DEFAULT_BASE_VIRTUAL_ADDRESS + 64 * PAGE, 16 * PAGE);
        assert_eq!(
            heap.committed_bytes(),
            4 * PAGE,
            "a repeated call does not reset it"
        );
    }

    /// A request for zero bytes at the cap maps nothing, so it cannot have grown the heap, and
    /// `grow` must not claim it did: `alloc` retries after a `true`, and a retry that changes
    /// nothing would loop forever.
    #[test]
    fn a_grow_of_nothing_at_the_maximum_is_a_refusal() {
        let heap = heap_at_its_maximum();
        let mut g = heap.lock();
        let inner = g.inner();
        assert!(!MemoryRegionHeap::grow(inner, 0));
        // The control: a real request past the cap is refused too, as before.
        assert!(!MemoryRegionHeap::grow(inner, PAGE));
    }

    /// The accessors report what `initialize` was given, the cap rounded down to a page.
    #[test]
    fn the_accessors_report_the_maximum_and_the_base() {
        let heap = MemoryRegionHeap::new();
        assert_eq!((heap.maximum_bytes(), heap.base_virtual_address()), (0, 0));
        heap.initialize(3, DEFAULT_BASE_VIRTUAL_ADDRESS, 16 * PAGE + 7);
        assert_eq!(heap.maximum_bytes(), 16 * PAGE);
        assert_eq!(heap.base_virtual_address(), DEFAULT_BASE_VIRTUAL_ADDRESS);
        heap.initialize(4, DEFAULT_BASE_VIRTUAL_ADDRESS + PAGE, 8 * PAGE);
        assert_eq!(heap.maximum_bytes(), 16 * PAGE);
    }

    /// A heap already at its cap, or with a cap of zero, has nothing left to commit, so
    /// `commit_all` makes no syscall and succeeds.
    #[test]
    fn commit_all_with_nothing_left_succeeds() {
        assert_eq!(heap_at_its_maximum().commit_all(), Ok(()));
        let heap = MemoryRegionHeap::new();
        assert_eq!(
            heap.commit_all(),
            Ok(()),
            "an unwired heap has a cap of zero"
        );
    }
}
