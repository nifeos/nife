//! **`memory_region_heap`: the untyped-backed heap, `alloc` for userspace, paid from the
//! process's own budget** (milestone 27 (Rust `std` on the native ABI)). Its own crate, so a program
//! with no heap links no allocator.
//!
//! A [`GlobalAlloc`] that grows by mapping pages out of the untyped capability the process was
//! granted (`memory_region::MAP`, one zeroed page per invoke). The algorithm is `talc`'s core,
//! [`talc::base::Talc`] (milestone 784 (reuse what the tree wrote), item d), with the `Manual`
//! source: growing is a syscall that can fail on a budget, so the retry lives here, by the cap.
//! This crate is the policy: a spinlock, a committed-range cursor and grow-on-demand.
//!
//! A program declares it in its own source, because the heap's inputs are part of its capability
//! contract (notes/abi.md, the out-of-band slot convention):
//!
//! ```ignore
//! #[global_allocator]
//! static HEAP: memory_region_heap::MemoryRegionHeap = memory_region_heap::MemoryRegionHeap::new();
//! // first thing in _start:
//! HEAP.init(MEMORY_REGION_SLOT, memory_region_heap::DEFAULT_BASE, 4 * 1024 * 1024);
//! ```
//!
//! - **Which untyped slot** pays is the program's convention with its parent. No untyped, no heap.
//! - **Where the heap lives**: [`DEFAULT_BASE`] (1 GiB) starts `address_space_map::HEAP`, the band
//!   the address-space map keeps for a heap and nothing else.
//! - **`max_bytes`** caps growth, so a leak exhausts the heap visibly before it eats the budget the
//!   program builds children from. `MAP`'s `OutOfMemory` is the hard ceiling either way, reported
//!   as an allocation failure. Allocating before `init` fails the same way.
//!
//! A spinlock is enough: the holder is always scheduled again to release it under this kernel's
//! preemptive scheduler. It is not efficient under real contention (notes/heap.md).
//!
//! **A crate of its own** because `talc` brings `alloc` into a program's crate graph (through
//! `allocator-api2`), and a graph with `alloc` in it must name a global allocator. Only a program
//! that names this crate links it, so a program with no heap needs none.
//!
//! Name: provisional (lane/784-talc, 2026-10-21 UTC), for the type it holds; calef has not ruled
//! on it.

#![no_std]

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, Ordering};

use talc::base::Talc;
use talc::base::binning::DefaultBinning;
use talc::source::Manual;

/// The suggested heap base: 1 GiB, the start of the address-space map's heap band (milestone 206 (a program image has under 896 KiB)).
/// A convention, not a law; a program with its own layout passes its own base to [`init`], and one
/// that does should still keep it inside `address_space_map::HEAP`.
///
/// [`init`]: MemoryRegionHeap::init
pub const DEFAULT_BASE: u64 = address_space_map::HEAP.start;

const PAGE: u64 = 4096;

/// The smallest growth step, in pages. Growing one page per allocation would make every early
/// `Vec` push a syscall; 8 pages (32 KiB) amortizes that without hoarding a small budget.
const MIN_GROW_PAGES: u64 = 8;

struct Inner {
    heap: Talc<Manual, DefaultBinning>,
    /// The end `talc` reports for the one heap it holds, which the next grow extends; `None`
    /// until the first grow claims the first run.
    heap_end: Option<NonNull<u8>>,
    /// Which capability table slot holds the untyped that pays for pages. `u64::MAX` until `init`.
    memory_region_slot: u64,
    /// The heap's virtual range: `[base, base + max)`, of which `[base, base + committed)` is
    /// mapped and donated to the allocator so far.
    base: u64,
    committed: u64,
    max: u64,
}

/// The untyped-backed global allocator. One static per program, named in its
/// `#[global_allocator]` attribute; see the module docs for the contract.
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
    pub const fn new() -> Self {
        MemoryRegionHeap {
            locked: AtomicBool::new(false),
            inner: core::cell::UnsafeCell::new(Inner {
                heap: Talc::new(Manual),
                heap_end: None,
                memory_region_slot: u64::MAX,
                base: 0,
                committed: 0,
                max: 0,
            }),
        }
    }

    /// Wire the heap to its budget: pages come from the untyped in `memory_region_slot`, mapped at
    /// `[base, base + max_bytes)`. Call once, before the first allocation, first thing in
    /// `_start`. `base` must be page-aligned and inside the user half.
    pub fn init(&self, memory_region_slot: u64, base: u64, max_bytes: u64) {
        let mut g = self.lock();
        let inner = g.inner();
        inner.memory_region_slot = memory_region_slot;
        inner.base = base;
        inner.committed = 0;
        inner.max = max_bytes & !(PAGE - 1);
    }

    /// **Map the whole capped range now**, rather than on the first allocations that need it.
    /// `false` when the budget could not cover it, in which case whatever was mapped stays usable.
    ///
    /// For a program that carves its budget last-in first-out (the shell's `--mem` grants, pipes and
    /// staging regions come back only while they are the budget's newest carve): committing at
    /// `_start` puts every heap page and its page tables under all of those, so a later grow can
    /// never land above one and strand it. The alternative, a region split off for the heap, is
    /// worse: a region that has been split cannot be destroyed until its child is, so the whole
    /// budget would stay held after the program exits. Milestone 47 (navigation and naming) met
    /// that first: its shell's kernel tests kept a budget's worth of frames each.
    pub fn commit_all(&self) -> bool {
        let mut g = self.lock();
        let inner = g.inner();
        let rest = inner.max.saturating_sub(inner.committed);
        rest == 0 || Self::grow(inner, rest)
    }

    /// Bytes of untyped currently mapped for the heap. The demo workload reports this so the
    /// test can assert growth actually happened (and stayed bounded).
    pub fn committed_bytes(&self) -> u64 {
        let mut g = self.lock();
        g.inner().committed
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
            return false; // init() has not run; there is nothing to pay with.
        }
        let need_pages = need.div_ceil(PAGE);
        // Grow geometrically (double the committed range) but never less than the request and
        // never past the cap, so a burst of small allocations costs O(log n) syscall batches.
        let want_pages = need_pages
            .max(inner.committed / PAGE) // doubling: as many again as we have
            .max(MIN_GROW_PAGES)
            .min((inner.max - inner.committed) / PAGE);
        if want_pages < need_pages {
            return false; // the cap is closer than the request; growing cannot help.
        }

        let start = inner.base + inner.committed;
        let mut got: u64 = 0;
        while got < want_pages {
            let va = start + got * PAGE;
            // SAFETY: plain syscall; the kernel validates the slot, the va, and the budget.
            if unsafe {
                user_mode_runtime::invoke(
                    inner.memory_region_slot,
                    abi::memory_region::MAP,
                    va,
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
            // fits: a grow is at least `MIN_GROW_PAGES`, far above talc's first-heap size.
            inner.heap_end = unsafe {
                match inner.heap_end {
                    Some(end) => Some(inner.heap.extend(end, run.wrapping_add(bytes))),
                    None => inner.heap.claim(run, bytes),
                }
            };
        }
        // Only a fresh contiguous run at least as big as the request guarantees the retry
        // succeeds; anything less and the honest answer is allocation failure.
        got >= need_pages
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
