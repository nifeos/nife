//! **The heap itself**: the page source it is paid from, the lock around `talc`, and the
//! [`GlobalAlloc`] that grows it by the policy in `growth`.
//!
//! Name: provisional, minted 2026-11-20 (UTC) by lane/1100-memory-region-heap-outstanding, for the
//! module and the file.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, Ordering};

use talc::base::Talc;
use talc::base::binning::DefaultBinning;
use talc::source::Manual;

use crate::growth::{PAGE_BYTES, growth_pages, required_bytes, run_succeeded};

/// Why a page source did not map a page: the raw nonzero answer of the call that maps one. For
/// [`MemoryRegionPages`] it is the kernel's return, one of [`abi::Error`]'s codes.
///
/// Name: ratified 2026-11-19 (UTC) (block 1100 names ruling).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// The nonzero value the page source answered with.
    pub code: i64,
}

impl Refusal {
    /// Whether the region has no page left to pay with, the one refusal that only means the budget
    /// is spent. Any other refusal stops the heap from growing again.
    ///
    /// Name: ratified 2026-11-19 (UTC) (block 1100 names ruling).
    pub fn is_budget_spent(self) -> bool {
        abi::Error::from_ret(self.code) == Some(abi::Error::OutOfMemory)
    }
}

/// Where the heap's pages come from: one call that maps one writable page at a virtual address.
/// The default, [`MemoryRegionPages`], is the kernel's `MAP`; a host test supplies its own.
///
/// An implementation maps exactly the page at `virtual_address` and nothing else, and answers
/// `Ok(())` only when that page is mapped and writable for the caller.
///
/// Name: ratified 2026-11-19 (UTC) (block 1100 names ruling), after `dlmalloc`'s `Allocator` and
/// `rlsf`'s `FlexSource`.
pub trait PageSource {
    /// Map the page at `virtual_address`, or say why not.
    ///
    /// Name: ratified 2026-11-19 (UTC) (block 1100 names ruling).
    fn map_page(&mut self, virtual_address: u64) -> Result<(), Refusal>;
}

/// The default [`PageSource`]: `MAP` on the memory region in one capability table slot.
///
/// Name: ratified 2026-11-19 (UTC) (block 1100 names ruling).
pub struct MemoryRegionPages {
    /// Which capability table slot holds the memory region that pays for pages.
    slot: u64,
}

impl MemoryRegionPages {
    const fn not_yet_wired() -> Self {
        MemoryRegionPages { slot: u64::MAX }
    }
}

impl PageSource for MemoryRegionPages {
    fn map_page(&mut self, virtual_address: u64) -> Result<(), Refusal> {
        // SAFETY: plain system call; the kernel validates the slot, the virtual address, and the budget.
        let answer = unsafe {
            user_mode_runtime::invoke(self.slot, abi::memory_region::MAP, virtual_address, 0, 0)
        };
        if answer == 0 {
            Ok(())
        } else {
            Err(Refusal { code: answer })
        }
    }
}

struct Inner<S> {
    heap: Talc<Manual, DefaultBinning>,
    /// The end `talc` reports for the one heap it holds, which the next grow extends; `None`
    /// until the first grow claims the first run.
    heap_end: Option<NonNull<u8>>,
    /// Where pages come from.
    source: S,
    /// Whether `initialize` has run. Only the first call wires the heap.
    wired: bool,
    /// Set when the source refused for a reason other than a spent budget: growth stops for good.
    stopped: bool,
    /// What the source last refused with, if it ever did.
    last_refusal: Option<Refusal>,
    /// The heap's virtual range: `[base, base + max)`, of which `[base, base + committed)` is
    /// mapped and donated to the allocator so far. `committed` never exceeds `maximum`, and both
    /// are whole pages.
    base: u64,
    committed: u64,
    maximum: u64,
}

/// What [`MemoryRegionHeap::commit_all`] could not do: the bytes of the capped range that stayed
/// unmapped because the budget ran out.
///
/// Name: ratified 2026-11-19 (UTC) (block 1100 names ruling).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Short {
    /// Bytes of the cap, a whole number of pages, that the heap could not commit.
    pub uncommitted_bytes: u64,
}

/// Why [`MemoryRegionHeap::initialize`] left the heap as it was.
///
/// Name: ratified 2026-11-19 (UTC) (block 1100 names ruling).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WiringRefusal {
    /// An earlier call already wired the heap; this one changed nothing.
    AlreadyWired,
    /// The base is not a multiple of the page size, so the kernel would refuse every page.
    BaseNotPageAligned,
    /// The cap, rounded down to a page, is zero: the heap could never map a page.
    MaximumUnderOnePage,
    /// The base plus the cap runs past the end of the address space.
    RangeOverflows,
}

/// The memory-region-backed global allocator. One static per program, named in its
/// `#[global_allocator]` attribute; see the module docs for the contract.
///
/// **Concurrency.** One spinlock (a compare-exchange loop on an `AtomicBool`, with no queue and no
/// yielding) guards every operation, so a thread that is preempted while it holds the lock keeps
/// every other allocating thread spinning until it runs again. The lock protects the one `Inner`
/// that holds `talc`'s raw heap pointers, which make it `!Sync` by itself; only a holder of the
/// lock reaches it. Two rules keep that sound. The allocator calls nothing that allocates while it
/// holds the lock (a [`PageSource`] must not allocate from this heap), and no signal-like upcall
/// may allocate from this heap, because it would wait on a lock its own thread holds. The default
/// panic handler (`user_mode_runtime::panic_handler!`) obeys both: it ignores the panic message
/// and traps, so a debug assertion inside `talc` ends the process rather than hanging it. A
/// program that installs a handler that allocates should not do so while the heap can be locked.
///
/// Name: ratified 2026-11-14 (UTC) (block 1100 decision A). The page-source parameter is
/// ratified 2026-11-19 (UTC) (block 1100 names ruling).
pub struct MemoryRegionHeap<S: PageSource = MemoryRegionPages> {
    locked: AtomicBool,
    inner: core::cell::UnsafeCell<Inner<S>>,
}

// SAFETY: all access to `inner` goes through the `locked` spinlock (see `lock`), so a `&` to the
// heap lets one thread at a time reach `Inner`; the page source moves to whichever thread holds
// the lock, which is why it must be `Send`.
unsafe impl<S: PageSource + Send> Sync for MemoryRegionHeap<S> {}

/// A held lock on the heap; releases on drop. Internal only.
struct Guard<'a, S: PageSource>(&'a MemoryRegionHeap<S>);

impl<S: PageSource> Drop for Guard<'_, S> {
    fn drop(&mut self) {
        self.0.locked.store(false, Ordering::Release);
    }
}

impl<S: PageSource> Guard<'_, S> {
    fn inner(&mut self) -> &mut Inner<S> {
        // SAFETY: holding the guard means holding the spinlock; access is exclusive, and the
        // `&mut self` receiver stops one guard from handing out two live borrows.
        unsafe { &mut *self.0.inner.get() }
    }
}

impl MemoryRegionHeap<MemoryRegionPages> {
    /// An empty, un-wired heap, `const` so it can be the `#[global_allocator]` static.
    ///
    /// Name: ratified 2026-11-14 (UTC) (block 1100 decision A).
    pub const fn new() -> Self {
        Self::with_source(MemoryRegionPages::not_yet_wired())
    }

    /// Wire the heap to its budget: pages come from the memory region in `memory_region_slot`,
    /// mapped at `[base, base + maximum_bytes)`. Call once, before the first allocation, first
    /// thing in `_start`.
    ///
    /// **Only the first call wires the heap.** A later call changes nothing and answers
    /// [`WiringRefusal::AlreadyWired`], because rewiring a heap that may already hold allocations
    /// would leave its allocator owning pages the new base never mapped, and a later allocation
    /// could be handed memory in that hole. A caller that only wants to know asks
    /// [`is_initialized`](Self::is_initialized).
    ///
    /// Refused, with the heap left unwired: a `base` that is not a multiple of the 4,096-byte
    /// page; a cap under one page after rounding down (so 4,095 bytes is refused, not silently a
    /// cap of zero); a range that runs past the end of the address space.
    ///
    /// Name: ratified 2026-11-14 (UTC) (block 1100 decision A). Spelled out under the `init` ruling
    /// of 2026-10-26 (UTC), in design/naming/acronyms-and-abbreviations.md. The `Result` return is
    /// ratified 2026-11-19 (UTC) (block 1100 names ruling).
    #[must_use = "a refused wiring leaves the heap unable to allocate"]
    pub fn initialize(
        &self,
        memory_region_slot: u64,
        base: u64,
        maximum_bytes: u64,
    ) -> Result<(), WiringRefusal> {
        let mut g = self.lock();
        let inner = g.inner();
        Self::check_wiring(inner, base, maximum_bytes)?;
        inner.source.slot = memory_region_slot;
        Self::wire(inner, base, maximum_bytes);
        Ok(())
    }
}

impl<S: PageSource> MemoryRegionHeap<S> {
    /// An empty, un-wired heap over `source`, `const` so it can be a static. Wire it with
    /// [`initialize_source`](Self::initialize_source).
    ///
    /// Name: ratified 2026-11-19 (UTC) (block 1100 names ruling).
    pub const fn with_source(source: S) -> Self {
        MemoryRegionHeap {
            locked: AtomicBool::new(false),
            inner: core::cell::UnsafeCell::new(Inner {
                heap: Talc::new(Manual),
                heap_end: None,
                source,
                wired: false,
                stopped: false,
                last_refusal: None,
                base: 0,
                committed: 0,
                maximum: 0,
            }),
        }
    }

    /// [`initialize`](MemoryRegionHeap::initialize) for a heap over a custom [`PageSource`]: the
    /// source already knows what pays, so only the range is given. The same refusals apply.
    ///
    /// Name: ratified 2026-11-19 (UTC) (block 1100 names ruling).
    #[must_use = "a refused wiring leaves the heap unable to allocate"]
    pub fn initialize_source(&self, base: u64, maximum_bytes: u64) -> Result<(), WiringRefusal> {
        let mut g = self.lock();
        let inner = g.inner();
        Self::check_wiring(inner, base, maximum_bytes)?;
        Self::wire(inner, base, maximum_bytes);
        Ok(())
    }

    fn check_wiring(inner: &Inner<S>, base: u64, maximum_bytes: u64) -> Result<(), WiringRefusal> {
        if inner.wired {
            return Err(WiringRefusal::AlreadyWired);
        }
        if !base.is_multiple_of(PAGE_BYTES) {
            return Err(WiringRefusal::BaseNotPageAligned);
        }
        let maximum = maximum_bytes & !(PAGE_BYTES - 1);
        if maximum == 0 {
            return Err(WiringRefusal::MaximumUnderOnePage);
        }
        if base.checked_add(maximum).is_none() {
            return Err(WiringRefusal::RangeOverflows);
        }
        Ok(())
    }

    /// Record a wiring that `check_wiring` accepted.
    fn wire(inner: &mut Inner<S>, base: u64, maximum_bytes: u64) {
        inner.wired = true;
        inner.base = base;
        inner.committed = 0;
        inner.maximum = maximum_bytes & !(PAGE_BYTES - 1);
    }

    /// Whether [`initialize`](MemoryRegionHeap::initialize) has succeeded, so that a further call
    /// would change nothing.
    ///
    /// Name: ratified 2026-11-19 (UTC) (block 1100 names ruling).
    pub fn is_initialized(&self) -> bool {
        let mut g = self.lock();
        g.inner().wired
    }

    /// **Map the whole capped range now**, rather than on the first allocations that need it.
    /// `Err(`[`Short`]`)` when the budget could not cover it, naming the bytes left unmapped; whatever
    /// was mapped stays usable.
    ///
    /// For a program that carves its budget last-in first-out (grants, pipes and staging regions
    /// come back only while they are the budget's newest carve): committing at `_start` puts every
    /// heap page and its page tables under all of those, so a later grow can never land above one
    /// and strand it. The alternative, a region split off for the heap, is worse: a region that has
    /// been split cannot be destroyed until its child is, so the whole budget would stay held after
    /// the program exits.
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

    /// Bytes of memory region currently mapped for the heap. A program or a test reads it to see
    /// that growth happened and stayed under [`maximum_bytes`](Self::maximum_bytes).
    ///
    /// Name: ratified 2026-11-14 (UTC) (block 1100 decision A).
    pub fn committed_bytes(&self) -> u64 {
        let mut g = self.lock();
        g.inner().committed
    }

    /// The cap in force, in bytes: the `maximum_bytes` given to
    /// [`initialize`](MemoryRegionHeap::initialize), rounded down to a whole page. Zero until the
    /// heap is wired. A later `initialize` does not change it.
    ///
    /// Name: ratified 2026-11-19 (UTC) (block 1100 names ruling).
    pub fn maximum_bytes(&self) -> u64 {
        let mut g = self.lock();
        g.inner().maximum
    }

    /// The virtual address where the heap starts: the `base` given to
    /// [`initialize`](MemoryRegionHeap::initialize). Zero until the heap is wired. A later
    /// `initialize` does not change it.
    ///
    /// Name: ratified 2026-11-19 (UTC) (block 1100 names ruling).
    pub fn base_virtual_address(&self) -> u64 {
        let mut g = self.lock();
        g.inner().base
    }

    /// What the page source last refused with, or `None` if it never refused. A failed first boot
    /// reads it: `OutOfMemory` means the budget is smaller than the heap's cap, and anything else
    /// (a misaligned or already-mapped address, a slot without the right to write) means the heap
    /// stopped growing the first time it happened.
    ///
    /// Name: ratified 2026-11-19 (UTC) (block 1100 names ruling).
    pub fn last_refusal(&self) -> Option<Refusal> {
        let mut g = self.lock();
        g.inner().last_refusal
    }

    fn lock(&self) -> Guard<'_, S> {
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
    fn grow(inner: &mut Inner<S>, need: u64) -> bool {
        if !inner.wired || inner.stopped {
            return false; // nothing to pay with, or the source refused for a reason a retry repeats.
        }
        let Some((need_pages, want_pages)) = growth_pages(inner.committed, inner.maximum, need)
        else {
            return false; // the cap is closer than the request; growing cannot help.
        };

        let start = inner.base + inner.committed;
        let mut got: u64 = 0;
        while got < want_pages {
            match inner.source.map_page(start + got * PAGE_BYTES) {
                Ok(()) => got += 1,
                Err(refusal) => {
                    inner.last_refusal = Some(refusal);
                    // Anything but a spent budget would meet the same refusal on every retry, and
                    // an already-mapped address costs the region a page each time.
                    inner.stopped = !refusal.is_budget_spent();
                    break; // keep what we got.
                }
            }
        }
        if got > 0 {
            inner.committed += got * PAGE_BYTES;
            let run = core::ptr::with_exposed_provenance_mut::<u8>(start as usize);
            let bytes = (got * PAGE_BYTES) as usize;
            // SAFETY: `[start, start + bytes)` was just mapped writable for us alone, and is
            // adjacent to the heap `heap_end` ends, or the first run of all. The first claim
            // fits: a cap is at least one page, and one page is above talc's first-heap size
            // (`the_first_heap_fits_in_one_page`).
            inner.heap_end = unsafe {
                match inner.heap_end {
                    Some(end) => Some(inner.heap.extend(end, run.wrapping_add(bytes))),
                    None => inner.heap.claim(run, bytes),
                }
            };
        }
        run_succeeded(got, need_pages)
    }
}

impl Default for MemoryRegionHeap<MemoryRegionPages> {
    fn default() -> Self {
        Self::new()
    }
}

fn shortfall<S>(inner: &Inner<S>, layout: Layout) -> u64 {
    let (first, free_top) = match inner.heap_end {
        // SAFETY: `end` is the heap end talc last returned, and the heap has not changed since.
        Some(end) => (
            0,
            end.as_ptr() as usize - unsafe { inner.heap.reserved(end) }.up_to.as_ptr() as usize,
        ),
        None => (talc::min_first_heap_size::<DefaultBinning>(), 0),
    };
    required_bytes(layout, talc::base::CHUNK_UNIT, first, free_top)
}

/// `layout` as `talc` is given it: at least one byte, because `talc` takes no empty layout and
/// `alloc` and `dealloc` must agree on the size.
fn nonempty(layout: Layout) -> Layout {
    // SAFETY: the alignment is `layout`'s own, and one byte cannot overflow it.
    unsafe { Layout::from_size_align_unchecked(layout.size().max(1), layout.align()) }
}

// SAFETY: alloc/dealloc uphold the GlobalAlloc contract: unique live pointers, the same layout to
// `talc` at both ends (`nonempty`), and no unwinding (there is none on this target).
unsafe impl<S: PageSource> GlobalAlloc for MemoryRegionHeap<S> {
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
            let new_pointer = self.alloc(new_layout);
            if !new_pointer.is_null() {
                core::ptr::copy_nonoverlapping(ptr, new_pointer, layout.size().min(new_size));
                self.dealloc(ptr, layout);
            }
            new_pointer
        }
    }
}

#[cfg(test)]
mod tests;
