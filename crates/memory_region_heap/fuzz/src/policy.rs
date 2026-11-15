//! A host copy of `memory_region_heap`'s policy around talc, with the page-mapping syscall replaced
//! by a [`PageSource`] over a host arena.
//!
//! The crate itself cannot run on a host yet: `MemoryRegionHeap::grow` traps straight to the kernel
//! through `user_mode_runtime::invoke`, and on an `x86_64` host that instruction is the host's own
//! `syscall` with nife's method number in `rax`. Milestone 1100 (`memory_region_heap`: what an
//! outside maintainer would fix before it leaves) item 8 adds the page-source seam. Until it lands,
//! this module restates `grow`, `shortfall`, `nonempty` and the three `GlobalAlloc` methods line for
//! line, so the fuzz target drives talc the way a nife program does. When the seam lands, the target
//! drives `MemoryRegionHeap` itself and this module goes.
//!
//! Name: provisional, minted 2026-11-14 (UTC) by lane talc-fuzz-and-miri, after the policy it copies.
//!
//! Every difference from `crates/memory_region_heap/src/lib.rs` is listed here, so a reader can check
//! the copy against the original:
//!
//! - No spinlock. The fuzz target is single-threaded and holds `&mut Heap`.
//! - The `MAP` call is [`PageSource::map`], with the same "nonzero is a refusal" reading.
//! - talc is built with its `counters` and `error-scanning-std` features, which add bookkeeping and
//!   a consistency scan before each operation and change no decision it makes.

use core::alloc::Layout;
use core::ptr::NonNull;

use talc::base::Talc;
use talc::base::binning::DefaultBinning;
use talc::source::Manual;

/// The page size `memory_region_heap` maps in.
pub(crate) const PAGE: u64 = 4096;

/// `memory_region_heap`'s smallest growth step, in pages.
const MINIMUM_GROW_PAGES: u64 = 8;

/// The memory region a heap maps its pages from: a page-aligned host arena, with a budget of pages
/// it will still pay for. It stands in for the kernel's `memory_region::MAP` method.
pub(crate) struct PageSource {
    arena: *mut u8,
    arena_bytes: usize,
    /// Pages the region still pays for; `None` means it never refuses for want of budget.
    budget_pages: Option<u64>,
    /// Bytes of the arena mapped so far, from its base. The heap maps only at the top of what it
    /// has, so this is always one contiguous run.
    mapped_bytes: usize,
}

impl PageSource {
    /// A region over `[arena, arena + arena_bytes)`, every page unmapped.
    ///
    /// # Safety
    /// `arena` is page-aligned and valid for reads and writes of `arena_bytes`, owned by the caller
    /// for as long as this source and any heap fed from it live.
    pub(crate) unsafe fn new(
        arena: *mut u8,
        arena_bytes: usize,
        budget_pages: Option<u64>,
    ) -> Self {
        poison(arena, arena_bytes);
        PageSource {
            arena,
            arena_bytes,
            budget_pages,
            mapped_bytes: 0,
        }
    }

    /// The arena's base address, the number a program passes to `initialize`.
    pub(crate) fn base(&self) -> u64 {
        self.arena.expose_provenance() as u64
    }

    /// The arena's size in bytes.
    pub(crate) fn arena_bytes(&self) -> usize {
        self.arena_bytes
    }

    /// Bytes mapped so far.
    pub(crate) fn mapped_bytes(&self) -> usize {
        self.mapped_bytes
    }

    /// The pages the region still pays for; `None` is unlimited.
    pub(crate) fn budget_pages(&self) -> Option<u64> {
        self.budget_pages
    }

    /// Change the pages the region still pays for, as when another consumer of the same budget
    /// spends some.
    pub(crate) fn set_budget(&mut self, budget_pages: Option<u64>) {
        self.budget_pages = budget_pages;
    }

    /// The `memory_region::MAP` method: map the page at `virtual_address`, or refuse with a nonzero
    /// result. A refusal for want of budget is ordinary. The heap mapping a page twice, below the
    /// top of what it holds, or past the arena it was given is a defect in the policy, so those
    /// stop the run.
    pub(crate) fn map(&mut self, _memory_region_slot: u64, virtual_address: u64) -> i64 {
        let expected = self.base() + self.mapped_bytes as u64;
        assert_eq!(
            virtual_address, expected,
            "the heap mapped a page other than the next one at its top"
        );
        assert!(
            self.mapped_bytes + PAGE as usize <= self.arena_bytes,
            "the heap mapped a page past its maximum"
        );
        match self.budget_pages {
            Some(0) => return 1,
            Some(ref mut pages) => *pages -= 1,
            None => {}
        }
        // SAFETY: the page is inside the arena (asserted above), which `new`'s caller owns.
        unpoison(unsafe { self.arena.add(self.mapped_bytes) }, PAGE as usize);
        self.mapped_bytes += PAGE as usize;
        0
    }
}

impl Drop for PageSource {
    fn drop(&mut self) {
        unpoison(self.arena, self.arena_bytes);
    }
}

#[cfg(sanitize = "address")]
unsafe extern "C" {
    fn __asan_poison_memory_region(address: *const u8, size: usize);
    fn __asan_unpoison_memory_region(address: *const u8, size: usize);
}

/// Mark `[address, address + size)` unmapped for AddressSanitizer, so a touch of a page the heap has
/// not mapped is reported where nife would take a fault. A no-op in a build without it.
fn poison(address: *mut u8, size: usize) {
    #[cfg(sanitize = "address")]
    // SAFETY: the range is inside the arena the caller owns; poisoning changes only the shadow.
    unsafe {
        __asan_poison_memory_region(address, size);
    }
    let _ = (address, size);
}

/// Undo [`poison`] for a page the heap has mapped.
fn unpoison(address: *mut u8, size: usize) {
    #[cfg(sanitize = "address")]
    // SAFETY: as `poison`.
    unsafe {
        __asan_unpoison_memory_region(address, size);
    }
    let _ = (address, size);
}

/// `MemoryRegionHeap`'s `Inner`, without the lock.
pub(crate) struct Heap {
    heap: Talc<Manual, DefaultBinning>,
    heap_end: Option<NonNull<u8>>,
    memory_region_slot: u64,
    base: u64,
    committed: u64,
    maximum: u64,
}

impl Heap {
    /// `MemoryRegionHeap::new`.
    pub(crate) const fn new() -> Self {
        Heap {
            heap: Talc::new(Manual),
            heap_end: None,
            memory_region_slot: u64::MAX,
            base: 0,
            committed: 0,
            maximum: 0,
        }
    }

    /// `MemoryRegionHeap::initialize`.
    pub(crate) fn initialize(&mut self, memory_region_slot: u64, base: u64, maximum_bytes: u64) {
        if self.is_initialized() {
            return;
        }
        self.memory_region_slot = memory_region_slot;
        self.base = base;
        self.committed = 0;
        self.maximum = maximum_bytes & !(PAGE - 1);
    }

    /// `MemoryRegionHeap::is_initialized`.
    pub(crate) fn is_initialized(&self) -> bool {
        self.memory_region_slot != u64::MAX || self.heap_end.is_some() || self.committed > 0
    }

    /// `MemoryRegionHeap::commit_all`, reporting only whether every page was mapped.
    pub(crate) fn commit_all(&mut self, source: &mut PageSource) -> bool {
        let rest = self.maximum.saturating_sub(self.committed);
        rest == 0 || self.grow(source, rest)
    }

    /// `MemoryRegionHeap::committed_bytes`.
    pub(crate) fn committed_bytes(&self) -> u64 {
        self.committed
    }

    /// The base and cap the first effective `initialize` set.
    pub(crate) fn base_and_maximum(&self) -> (u64, u64) {
        (self.base, self.maximum)
    }

    /// The end talc reports for the heap, `None` before the first grow claims a run.
    pub(crate) fn heap_end(&self) -> Option<NonNull<u8>> {
        self.heap_end
    }

    /// talc's own accounting.
    pub(crate) fn counters(&self) -> &talc::base::Counters {
        self.heap.counters()
    }

    /// The free bytes at the top of the heap, which a grow merges with: `shortfall`'s `free_top`.
    pub(crate) fn free_top(&self) -> u64 {
        match self.heap_end {
            // SAFETY: `end` is the heap end talc last returned, and the heap has not changed since.
            Some(end) => {
                (end.as_ptr() as usize - unsafe { self.heap.reserved(end) }.up_to.as_ptr() as usize)
                    as u64
            }
            None => 0,
        }
    }

    /// `MemoryRegionHeap::grow`, mapping through `source`.
    fn grow(&mut self, source: &mut PageSource, need: u64) -> bool {
        if self.memory_region_slot == u64::MAX {
            return false;
        }
        let need_pages = need.div_ceil(PAGE);
        let want_pages = need_pages
            .max(self.committed / PAGE)
            .max(MINIMUM_GROW_PAGES)
            .min((self.maximum - self.committed) / PAGE);
        if want_pages < need_pages {
            return false;
        }

        let start = self.base + self.committed;
        let mut got: u64 = 0;
        while got < want_pages {
            let va = start + got * PAGE;
            if source.map(self.memory_region_slot, va) != 0 {
                break;
            }
            got += 1;
        }
        if got > 0 {
            self.committed += got * PAGE;
            let run = core::ptr::with_exposed_provenance_mut::<u8>(start as usize);
            let bytes = (got * PAGE) as usize;
            // SAFETY: `[start, start + bytes)` was just mapped by `source` for this heap alone, and
            // is adjacent to the heap `heap_end` ends, or the first run of all.
            self.heap_end = unsafe {
                match self.heap_end {
                    Some(end) => Some(self.heap.extend(end, run.wrapping_add(bytes))),
                    None => self.heap.claim(run, bytes),
                }
            };
        }
        got > 0 && got >= need_pages
    }

    /// `memory_region_heap`'s `shortfall`.
    pub(crate) fn shortfall(&self, layout: Layout) -> u64 {
        let unit = talc::base::CHUNK_UNIT;
        let chunk = (layout.size() + core::mem::size_of::<usize>()).next_multiple_of(unit);
        let over_aligned = if layout.align() > unit {
            layout.align()
        } else {
            0
        };
        let (first, free_top) = match self.heap_end {
            Some(_) => (0, self.free_top() as usize),
            None => (talc::min_first_heap_size::<DefaultBinning>(), 0),
        };
        (chunk + over_aligned + first).saturating_sub(free_top) as u64
    }

    /// `GlobalAlloc::alloc`.
    ///
    /// # Safety
    /// The `GlobalAlloc` contract: `layout` has a nonzero size or is handled by `nonempty`.
    pub(crate) unsafe fn alloc(&mut self, source: &mut PageSource, layout: Layout) -> *mut u8 {
        let layout = nonempty(layout);
        loop {
            // SAFETY: `layout` is nonempty.
            if let Some(p) = unsafe { self.heap.try_allocate(layout) } {
                return p.as_ptr();
            }
            if !self.grow(source, self.shortfall(layout)) {
                return core::ptr::null_mut();
            }
        }
    }

    /// `GlobalAlloc::dealloc`.
    ///
    /// # Safety
    /// `ptr` came from `alloc(layout)` or `realloc` on this heap and is live.
    pub(crate) unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }
        // SAFETY: forwarded contract; talc was given the same `nonempty(layout)`.
        unsafe { self.heap.deallocate(ptr, nonempty(layout)) };
    }

    /// `GlobalAlloc::realloc`.
    ///
    /// # Safety
    /// `ptr` is live under `layout`, `new_size` is nonzero, and `new_size` rounded up to
    /// `layout.align()` does not overflow `isize`.
    pub(crate) unsafe fn realloc(
        &mut self,
        source: &mut PageSource,
        ptr: *mut u8,
        layout: Layout,
        new_size: usize,
    ) -> *mut u8 {
        let old = nonempty(layout);
        // SAFETY: forwarded contract.
        if new_size != 0 && unsafe { self.heap.try_realloc_in_place(ptr, old, new_size) } {
            return ptr;
        }
        // SAFETY: the contract makes `new_size` valid for this alignment.
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        // SAFETY: allocate, copy, free, as `GlobalAlloc::realloc`'s default body.
        unsafe {
            let new_ptr = self.alloc(source, new_layout);
            if !new_ptr.is_null() {
                core::ptr::copy_nonoverlapping(ptr, new_ptr, layout.size().min(new_size));
                self.dealloc(ptr, layout);
            }
            new_ptr
        }
    }
}

impl Default for Heap {
    fn default() -> Self {
        Self::new()
    }
}

/// `memory_region_heap`'s `nonempty`: talc takes no empty layout.
pub(crate) fn nonempty(layout: Layout) -> Layout {
    // SAFETY: the alignment is `layout`'s own, and one byte cannot overflow it.
    unsafe { Layout::from_size_align_unchecked(layout.size().max(1), layout.align()) }
}
