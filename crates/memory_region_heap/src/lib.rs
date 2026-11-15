//! The memory-region-backed heap, a [`GlobalAlloc`] paid from the process's own memory region
//! (milestone 27 (Rust `std` on the native ABI)). A memory region is the budget of pages a
//! process was handed; the kernel's own word for its capability is "untyped". Pages are mapped out
//! of it one at a time and given to the allocator, so the region is what pays for the heap, the
//! heap lives at one virtual range the program picks, and nothing else supplies memory.
//!
//! A program declares it (see notes/heap.md):
//!
//! ```ignore
//! #[global_allocator]
//! static HEAP: memory_region_heap::MemoryRegionHeap = memory_region_heap::MemoryRegionHeap::new();
//! // first thing in _start:
//! HEAP.initialize(MEMORY_REGION_SLOT, memory_region_heap::DEFAULT_BASE_VIRTUAL_ADDRESS, 4 * 1024 * 1024)
//!     .expect("a page-aligned base and a cap of at least one page");
//! ```
//!
//! **How it grows.** The heap starts empty and maps nothing until the first allocation. A grow maps
//! as many pages again as the heap already holds, never fewer than 8 pages (32 KiB) and never past
//! the cap, so a burst of small allocations costs a logarithmic number of syscall batches.
//! [`MemoryRegionHeap::commit_all`] maps the whole cap at once instead.
//!
//! **When it refuses.** An allocation returns null when the cap is reached, when the region has no
//! page left to pay with ([`abi::Error::OutOfMemory`]), or after the region refused for any other
//! reason. The first two leave the heap able to serve what it already holds. The third stops growth
//! for good, because a retry would meet the same refusal and might spend a page each time.
//! [`MemoryRegionHeap::last_refusal`] says what the region last answered.
//!
//! **The smallest useful cap is one page** (4,096 bytes), which [`MemoryRegionHeap::initialize`]
//! enforces. `talc`'s first heap needs 1,575 bytes of bookkeeping on a 64-bit target, so a one-page
//! heap serves about 2,500 bytes of allocations; a test pins the figure.
//!
//! **The heap never shrinks.** `talc`'s `truncate` is never called and the memory-region interface
//! has no call that returns one page, so a heap that peaked holds its peak until the region's owner
//! destroys the region.
//!
//! **Which `talc`.** The manifest asks crates.io for `talc` 5.1.1. Inside nife's workspace the
//! root `[patch.crates-io]` builds it from `vendor/talc`, which carries upstream pull request 57
//! (two fit checks that compared addresses where they meant sizes); outside nife the package is
//! unpatched and lacks that fix. The wrapped-address case it repairs needs a free gap within one
//! chunk of the top of the address space, which a 64-bit user process never maps.
//!
//! The page source is a seam: [`MemoryRegionHeap`] takes any [`PageSource`], with the syscall
//! ([`MemoryRegionPages`]) as the default, so the growth policy runs and is tested on a host.
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
/// and one that does should still keep it inside `address_space_map::HEAP`.
///
/// [`initialize`]: MemoryRegionHeap::initialize
///
/// Name: ratified 2026-11-14 (UTC) (block 1100 decision A). An address is `_VIRTUAL_ADDRESS` in
/// every location.
pub const DEFAULT_BASE_VIRTUAL_ADDRESS: u64 = address_space_map::HEAP.start;

/// The size of the pages the heap maps, in bytes.
const PAGE_BYTES: u64 = 4096;

/// The smallest growth step, in pages. Growing one page per allocation would make every early
/// `Vec` push a syscall; 8 pages (32 KiB) amortizes that without hoarding a small budget.
const MINIMUM_GROW_PAGES: u64 = 8;

/// Why a page source did not map a page: the raw nonzero answer of the call that maps one. For
/// [`MemoryRegionPages`] it is the kernel's return, one of [`abi::Error`]'s codes.
///
/// Name: provisional (block 1100 item 7, 2026-11-15 UTC).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// The nonzero value the page source answered with.
    pub code: i64,
}

impl Refusal {
    /// Whether the region has no page left to pay with, the one refusal that only means the budget
    /// is spent. Any other refusal stops the heap from growing again.
    ///
    /// Name: provisional (block 1100 item 7, 2026-11-15 UTC).
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
/// Name: provisional (block 1100 item 8, 2026-11-15 UTC), after `dlmalloc`'s `Allocator` and
/// `rlsf`'s `FlexSource`.
pub trait PageSource {
    /// Map the page at `virtual_address`, or say why not.
    ///
    /// Name: provisional (block 1100 item 8, 2026-11-15 UTC).
    fn map_page(&mut self, virtual_address: u64) -> Result<(), Refusal>;
}

/// The default [`PageSource`]: `MAP` on the memory region in one capability table slot.
///
/// Name: provisional (block 1100 item 8, 2026-11-15 UTC).
pub struct MemoryRegionPages {
    /// Which capability table slot holds the memory region that pays for pages.
    slot: u64,
}

impl MemoryRegionPages {
    const fn unwired() -> Self {
        MemoryRegionPages { slot: u64::MAX }
    }
}

impl PageSource for MemoryRegionPages {
    fn map_page(&mut self, virtual_address: u64) -> Result<(), Refusal> {
        // SAFETY: plain syscall; the kernel validates the slot, the virtual address, and the budget.
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
/// Name: provisional (block 1100 decision A, 2026-11-14 UTC).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Short {
    /// Bytes of the cap, a whole number of pages, that the heap could not commit.
    pub uncommitted_bytes: u64,
}

/// Why [`MemoryRegionHeap::initialize`] left the heap as it was.
///
/// Name: provisional (block 1100 items 5 and 7, 2026-11-15 UTC).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WiringRefusal {
    /// An earlier call already wired the heap; this one changed nothing.
    AlreadyWired,
    /// The base is not a multiple of the page size, so the kernel would refuse every page.
    MisalignedBase,
    /// The cap, rounded down to a page, is zero: the heap could never map a page.
    CapUnderOnePage,
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
/// provisional (block 1100 item 8, 2026-11-15 UTC).
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
        Self::with_source(MemoryRegionPages::unwired())
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
    /// provisional (block 1100 items 5 and 7, 2026-11-15 UTC).
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
    /// Name: provisional (block 1100 item 8, 2026-11-15 UTC).
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
    /// Name: provisional (block 1100 item 8, 2026-11-15 UTC).
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
            return Err(WiringRefusal::MisalignedBase);
        }
        let maximum = maximum_bytes & !(PAGE_BYTES - 1);
        if maximum == 0 {
            return Err(WiringRefusal::CapUnderOnePage);
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
    /// Name: provisional, minted 2026-11-14 (UTC) by maintainer/review-hostile-input-fixes.
    pub fn is_initialized(&self) -> bool {
        let mut g = self.lock();
        g.inner().wired
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
    /// Name: provisional (block 1100 item 13, decision A, 2026-11-14 UTC).
    pub fn maximum_bytes(&self) -> u64 {
        let mut g = self.lock();
        g.inner().maximum
    }

    /// The virtual address where the heap starts: the `base` given to
    /// [`initialize`](MemoryRegionHeap::initialize). Zero until the heap is wired. A later
    /// `initialize` does not change it.
    ///
    /// Name: provisional (block 1100 item 13, decision A, 2026-11-14 UTC).
    pub fn base_virtual_address(&self) -> u64 {
        let mut g = self.lock();
        g.inner().base
    }

    /// What the page source last refused with, or `None` if it never refused. A failed first boot
    /// reads it: `OutOfMemory` means the budget is smaller than the heap's cap, and anything else
    /// (a misaligned or already-mapped address, a slot without the right to write) means the heap
    /// stopped growing the first time it happened.
    ///
    /// Name: provisional (block 1100 item 7, 2026-11-15 UTC).
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

/// The pages a grow of `need` bytes plans over a heap of `committed` bytes under a cap of
/// `maximum` bytes (both whole pages, `committed <= maximum`): `(need_pages, want_pages)`, or
/// `None` when the cap is closer than the request.
///
/// Growth is geometric (double the committed range) but never less than the request, never less
/// than [`MINIMUM_GROW_PAGES`], and never past the cap, so a burst of small allocations costs
/// O(log n) syscall batches.
fn growth_pages(committed: u64, maximum: u64, need: u64) -> Option<(u64, u64)> {
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
fn run_succeeded(got_pages: u64, need_pages: u64) -> bool {
    got_pages > 0 && got_pages >= need_pages
}

/// The bytes a fresh run at the top must add for `layout` to fit: its chunk (the size and talc's
/// one-word tag, on talc's 32-byte grid), any alignment past that grid, and talc's bins on the
/// first run (`first_run_bytes`, zero after it), less what is already free at the top
/// (`free_top_bytes`), which the run merges with. Exact, not generous, because a cap set at a power
/// of two must still hold an allocation of that size.
/// The chunk arithmetic restates talc's `required_chunk_size`; were talc to round otherwise, a
/// short grow is followed by another, which doubles, until the cap refuses.
fn required_bytes(layout: Layout, first_run_bytes: usize, free_top_bytes: usize) -> u64 {
    let unit = talc::base::CHUNK_UNIT;
    let chunk = (layout.size() + core::mem::size_of::<usize>()).next_multiple_of(unit);
    let over_aligned = if layout.align() > unit {
        layout.align()
    } else {
        0
    };
    (chunk + over_aligned + first_run_bytes).saturating_sub(free_top_bytes) as u64
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
    required_bytes(layout, first, free_top)
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
            let new_ptr = self.alloc(new_layout);
            if !new_ptr.is_null() {
                core::ptr::copy_nonoverlapping(ptr, new_ptr, layout.size().min(new_size));
                self.dealloc(ptr, layout);
            }
            new_ptr
        }
    }
}

/// Proofs of the arithmetic `talc` cannot be part of: `talc` is raw pointers over mapped memory,
/// which the prover does not model, so the policy is split into the pure functions below and
/// proved on those. Each harness calls the function the heap calls.
#[cfg(kani)]
mod verification {
    use super::*;

    /// **A grow never commits past the cap, and keeps the range whole pages.** For any cap, any
    /// committed range inside it, any request, and any number of pages the source maps up to the
    /// plan (a refused page ends the run early), the committed range after the run is inside the
    /// cap and a whole number of pages, and a run that counts as a success covers the request.
    ///
    /// The two `assume`s about `committed` and `maximum` are the type's invariant, held by
    /// `wire` and by `grow`'s own addition; the host test
    /// `committed_stays_inside_the_cap_whatever_the_source_refuses` drives sequences of grows
    /// against refusing sources and asserts them after each step. The bound on `got` is the loop
    /// in `grow`, which maps at most `want_pages`; `a_grow_maps_no_more_pages_than_it_planned`
    /// counts the calls.
    /// Falsification: replayable `crates/memory_region_heap/falsifications/verification.a_grow_never_commits_past_the_cap.patch`
    #[kani::proof]
    fn a_grow_never_commits_past_the_cap() {
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
    /// Falsification: replayable `crates/memory_region_heap/falsifications/verification.a_grow_at_the_cap_is_a_refusal.patch`
    #[kani::proof]
    fn a_grow_at_the_cap_is_a_refusal() {
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

        let asked = required_bytes(layout, first_run_bytes, free_top_bytes);
        assert!(
            asked as u128 + free_top_bytes as u128
                >= layout.size() as u128 + core::mem::size_of::<usize>() as u128,
            "the run plus the free top must hold the size and its tag"
        );
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::alloc::{alloc_zeroed, dealloc};
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::vec::Vec;

    use super::*;

    /// `abi::Error::OutOfMemory` and `BadPointer` as the kernel returns them.
    const OUT_OF_MEMORY: i64 = abi::Error::OutOfMemory as i64;
    const BAD_POINTER: i64 = abi::Error::BadPointer as i64;

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

    /// A heap over `arena`, capped at `cap_pages`, whose region pays for `budget_pages` pages and
    /// then refuses with `refusal_code`.
    fn heap_over(
        arena: &Arena,
        cap_pages: u64,
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
        heap.initialize_source(arena.base_virtual_address(), cap_pages * PAGE_BYTES)
            .unwrap();
        (heap, region)
    }

    /// A heap with a region that never runs out inside the cap.
    fn heap_with_plenty(arena: &Arena, cap_pages: u64) -> MemoryRegionHeap<FakePages> {
        heap_over(arena, cap_pages, u64::MAX, OUT_OF_MEMORY).0
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
    fn a_refused_initialize_leaves_the_heap_unwired() {
        let heap = MemoryRegionHeap::new();
        let refused = |base, bytes| heap.initialize(3, base, bytes);
        assert_eq!(
            refused(DEFAULT_BASE_VIRTUAL_ADDRESS + 8, 16 * PAGE_BYTES),
            Err(WiringRefusal::MisalignedBase)
        );
        assert_eq!(
            refused(DEFAULT_BASE_VIRTUAL_ADDRESS, PAGE_BYTES - 1),
            Err(WiringRefusal::CapUnderOnePage),
            "a cap of 4,095 bytes rounds to zero"
        );
        assert_eq!(
            refused(DEFAULT_BASE_VIRTUAL_ADDRESS, 0),
            Err(WiringRefusal::CapUnderOnePage)
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
    fn an_allocation_past_the_cap_is_null_not_a_hang() {
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
        assert!(
            !unsafe { heap.alloc(layout(3000, 8)) }.is_null(),
            "freed space is reusable"
        );
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
    fn a_power_of_two_cap_holds_the_largest_allocation_that_fits() {
        let cap = 16 * PAGE_BYTES as usize;
        let first = talc::min_first_heap_size::<DefaultBinning>();
        let unit = talc::base::CHUNK_UNIT;
        let largest = (cap - first) / unit * unit - core::mem::size_of::<usize>();

        let arena = Arena::new(16);
        let heap = heap_with_plenty(&arena, 16);
        // SAFETY: a nonzero size.
        let p = unsafe { heap.alloc(layout(largest, 8)) };
        assert!(!p.is_null(), "{largest} bytes fit in {cap}");
        assert_eq!(heap.committed_bytes(), cap as u64);

        let arena = Arena::new(16);
        let heap = heap_with_plenty(&arena, 16);
        // SAFETY: a nonzero size.
        assert!(unsafe { heap.alloc(layout(largest + unit, 8)) }.is_null());
    }

    /// `realloc` in place keeps the pointer; by copy it moves the bytes and frees the old block.
    #[test]
    fn realloc_in_place_and_by_copy() {
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
    fn a_refused_address_stops_growth_for_good() {
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
    fn commit_all_maps_the_cap_or_reports_the_shortfall() {
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
    /// `commit_all` makes no syscall and succeeds; an unwired default heap refuses allocations
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
        // SAFETY: a nonzero size. The unwired default source would trap on a syscall, so reaching
        // the null return shows none was made.
        assert!(unsafe { heap.alloc(layout(8, 8)) }.is_null());
    }

    /// The accessors report what `initialize` was given, the cap rounded down to a page.
    #[test]
    fn the_accessors_report_the_maximum_and_the_base() {
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
    /// it planned. These are the facts `a_grow_never_commits_past_the_cap` takes as given.
    #[test]
    fn committed_stays_inside_the_cap_whatever_the_source_refuses() {
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for round in 0..200 {
            let cap_pages = 1 + next() % 40;
            let budget = next() % 60;
            let code = if round % 2 == 0 {
                OUT_OF_MEMORY
            } else {
                BAD_POINTER
            };
            let arena = Arena::new(cap_pages);
            let (heap, region) = heap_over(&arena, cap_pages, budget, code);
            for _ in 0..12 {
                let need = next() % (cap_pages * PAGE_BYTES + 3 * PAGE_BYTES);
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
        assert_eq!(required_bytes(layout(1, 1), 0, 0), unit as u64);
        // A request the free top already covers asks for nothing.
        assert_eq!(required_bytes(layout(1, 1), 0, 4 * unit), 0);
        // The first run adds talc's bookkeeping, and alignment past the grid adds its slack.
        assert_eq!(required_bytes(layout(1, 1), 1575, 0), unit as u64 + 1575);
        assert_eq!(required_bytes(layout(1, 256), 0, 0), unit as u64 + 256);
    }
}
