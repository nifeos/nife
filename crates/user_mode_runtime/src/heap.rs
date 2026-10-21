//! **The untyped-backed heap: `alloc` for userspace, paid from the process's own budget**
//! (milestone 27).
//!
//! A [`GlobalAlloc`] that grows by mapping pages out of the untyped capability the process was
//! granted, via `memory_region::MAP` (one page per invoke, zeroed by the kernel, writable). The
//! algorithm is `talc` 5.1.1's core, [`talc::base::Talc`] (milestone 784 (reuse what the tree wrote),
//! item d); this module is the policy and the syscall glue: a spinlock, a committed-range cursor,
//! and grow-on-demand. Its own [`Source`](talc::source::Source) is `Manual`, because growing is a
//! syscall that can fail on a budget, and the retry belongs here, next to the cap.
//!
//! # The contract a program signs
//!
//! Declaring the allocator is two lines and one call, all in the program's own source, because
//! the heap's inputs are part of that program's capability contract (notes/abi.md §4):
//!
//! ```ignore
//! #[global_allocator]
//! static HEAP: user_mode_runtime::heap::MemoryRegionHeap = user_mode_runtime::heap::MemoryRegionHeap::new();
//! // first thing in _start:
//! HEAP.init(MEMORY_REGION_SLOT, user_mode_runtime::heap::DEFAULT_BASE, 4 * 1024 * 1024);
//! ```
//!
//! - **Which untyped slot** feeds the heap is the program's convention with its parent, like
//!   every other slot. There is no ambient memory to fall back on: no untyped granted, no heap.
//! - **Where the heap lives** is a virtual range the program promises not to use for anything
//!   else. [`DEFAULT_BASE`] (1 GiB) is the start of `address_space_map::HEAP`, the band the
//!   address-space map keeps for a heap and nothing else.
//! - **`max_bytes`** caps growth so a leak exhausts the heap, visibly, before it silently eats
//!   the budget the program also builds children from. The untyped itself is the hard ceiling
//!   either way: `MAP` returns `OutOfMemory` when the region is spent, and the allocator reports
//!   that as an allocation failure (which `alloc`'s default OOM handler turns into a panic).
//!
//! Allocating before `init` fails cleanly (null, then the OOM panic): there is nothing to grow
//! from until the program says which capability pays.
//!
//! # Why a spinlock is enough
//!
//! `GlobalAlloc` must be `Sync`. Today a process is one thread (std's `thread::spawn` is
//! `Unsupported` in milestone 27 phase one), so the lock is never contended; when threads within
//! one process arrive, a spinlock stays correct under this kernel's preemptive scheduler because
//! the holder always gets scheduled again to release it. What it is *not* is efficient under real
//! contention; that is a problem worth having, and notes/heap.md carries the caveat.

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
            if unsafe { crate::invoke(inner.memory_region_slot, abi::memory_region::MAP, va, 0, 0) }
                != 0
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
            // Generous on purpose: the request, its alignment, a chunk's rounding and tag, and
            // talc's bins when this is the first run. A grow that still falls short is followed by
            // another, which doubles, until the cap refuses.
            let mut need = (layout.size() + layout.align() + 2 * talc::base::CHUNK_UNIT) as u64;
            if inner.heap_end.is_none() {
                need += talc::min_first_heap_size::<DefaultBinning>() as u64;
            }
            if !Self::grow(inner, need) {
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
