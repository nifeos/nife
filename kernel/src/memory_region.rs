//! Memory regions: untyped memory a process holds a capability to and retypes (milestone 11).
//!
//! The kernel owns no pool to hand out from. A process holds a `MemoryRegion` capability and
//! retypes part of it into the page or object it wants; the kernel advances a watermark and hands
//! back a physical address, so its free-frame count does not move while a process allocates. The
//! table and its decisions are `crates/memory_regions`; this module is the I/O: the frame
//! allocator, the direct map, the revoke and the lock. See notes/memory-regions.md.
//!
//! # BUGS
//! `Error::OutOfMemory` collapses three causes into one code (DECISIONS §119); see the note.

pub use memory_regions::ObjectKind;
use memory_regions::{PageUse, RegionTable};
use page_frames::{FRAME_SIZE, PageFrame};

use crate::memory;
use crate::sync::{IrqSafeMutex, rank};

/// The most untyped regions that can be live **at once**. Slots are reused (the table is
/// generational), so this bounds concurrent regions, not creations over the kernel's lifetime.
///
/// Read the ledger of what holds regions at the peak, in notes/memory-regions.md, before raising or
/// lowering it. A full table refuses quietly, and there is no gate.
pub(crate) const MAX_REGIONS: usize = 256;

/// **The most regions that were ever live at once on this boot**, the instrument that makes
/// [`MAX_REGIONS`] a measured number: a full table is refused in whichever test asks next, and by
/// then every earlier reading is gone. `sched::PEAK_THREADS`'s twin.
///
/// Updated under the `REGIONS` lock on the two paths that can grow the table, [`create`] and
/// [`split`], so it never races. `fetch_max` rather than a compare-store, for `PEAK_THREADS`'s
/// reason: cheap, and neither path is hot (each already costs at least a page).
static PEAK_REGIONS: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// Record the table's occupancy against [`PEAK_REGIONS`]. Takes the table rather than the lock so
/// the reading is the one the insert just produced, under the same hold.
fn note_peak(table: &RegionTable<MAX_REGIONS>) {
    PEAK_REGIONS.fetch_max(table.len(), core::sync::atomic::Ordering::Relaxed);
}

/// The high-water mark [`PEAK_REGIONS`] holds. Printed by the test suite's closing summary.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // the closing summary is the only reader
pub(crate) fn peak_region_count() -> usize {
    PEAK_REGIONS.load(core::sync::atomic::Ordering::Relaxed)
}

/// The untyped regions (`crates/memory_regions`, notes/generational-names.md). Generational:
/// reclaiming a region bumps its slot's generation, so every `MemoryRegion` capability minted for
/// it stops resolving, and the slot is reused. An `Object::MemoryRegion` capability carries the
/// generational `u64` name.
///
/// **Page units inside, byte addresses outside.** The crate is `FRAME_SIZE`-agnostic on purpose, so
/// every conversion happens here, at the boundary where the constant is already in scope.
static REGIONS: IrqSafeMutex<RegionTable<MAX_REGIONS>> =
    IrqSafeMutex::new(rank::MEMORY_REGION, RegionTable::new());

/// Carve `pages` of physical memory out of the frame allocator, once, and make it an untyped
/// region. **This is the kernel's one allocation for this memory**: the seL4 boundary, where all
/// free RAM becomes untyped handed to the first process. Everything the owner does afterward
/// spends this, not the allocator.
pub fn create(pages: u64) -> Option<u64> {
    let base = memory::alloc_contiguous(pages as usize)?.addr();

    let name = {
        let mut table = REGIONS.lock();
        let name = table.insert_root(base / FRAME_SIZE, pages);
        note_peak(&table);
        name
    };
    if name.is_none() {
        // No free region slot: give the memory back rather than leak it. With reuse this is now a
        // genuine concurrency limit (too many live regions), not a lifetime one.
        for i in 0..pages {
            memory::free(PageFrame::from_addr(base + i * FRAME_SIZE));
        }
    }
    name
}

/// **Carve `pages` off `parent`'s unspent budget into a new child region**, and return its name.
/// The parent's watermark advances and it cannot be destroyed while the child lives. `None` if the
/// parent is unknown or exhausted, `pages` is zero, or the region table is full.
///
/// A child destroyed at the top of the parent's watermark gives its pages back (DECISIONS §16); one
/// freed out of order leaves a hole until no child above it is live.
///
/// **A split refused for a full table still bumps the parent**, and the parent can then never be
/// reclaimed: `RegionTable::split`'s `# BUGS` entry has the consequence and a reproduction, and
/// `notes/region-split-on-a-full-table.md` the proposal.
pub fn split(parent: u64, pages: u64) -> Option<u64> {
    // The test kernel's Nth-retype fault (milestone 757 (a test kernel fails a process on its Nth
    // retype), provisional): refused before the lock, so the region is left exactly as an
    // exhausted one would be. Absent from every other build.
    #[cfg(feature = "system_tests")]
    if crate::retype_fault::fails_now() {
        return None;
    }
    let mut table = REGIONS.lock();
    let child = table.split(parent, pages);
    note_peak(&table);
    child
}

/// Whether this region has live children (was split and they are not all reclaimed), so it cannot be
/// reclaimed (`sched::reclaim_region` refuses it, as [`destroy`] does). `false` for an unknown or
/// stale name. A parent with zero live children is destroyable again, the point of the count over a
/// bool.
pub fn has_children(region: u64) -> bool {
    REGIONS.lock().has_children(region)
}

/// **Retype one page out of the region**, zeroed, returning its physical address. `None` when the
/// region is exhausted: the *process* is out of budget, not the kernel.
///
/// Zeroed because the caller may make this page a page table, where a stale descriptor is a
/// pointer to nowhere followed at speed, and because a process should not see the previous
/// contents of its own untyped.
pub fn retype_page(region: u64) -> Option<u64> {
    retype_run(region, 1).map(|(phys, _)| phys)
}

/// **Retype a run of pages out of the region** (`MemoryRegion::RETYPE`'s count, calef's ruling of
/// 2026-09-26), zeroed, returning the first page's physical address and the run's length. The run
/// is `requested` pages, or one when `requested` is `0` (`memory_regions::retype_pages`). `None`,
/// with the region's watermark unmoved, when it does not fit: the proved arithmetic in
/// `memory_regions::retype_new_watermark` decides, and this only does the I/O.
pub fn retype_run(region: u64, requested: u64) -> Option<(u64, u64)> {
    // The test kernel's Nth-retype fault (milestone 757, provisional): refused before the lock,
    // so the region is left exactly as an exhausted one would be. Absent from every other build.
    #[cfg(feature = "system_tests")]
    if crate::retype_fault::fails_now() {
        return None;
    }
    // Released at the semicolon: zeroing the whole run must not happen under the region lock.
    let page = REGIONS.lock().retype_run(region, requested)?;
    let count = memory_regions::retype_pages(requested);
    let phys = page * FRAME_SIZE;

    // SAFETY: the run is inside a region we carved from the allocator and own exclusively, and it
    // is contiguous because a region's watermark is; the direct map reaches all of it. Zero it
    // before anyone can read a stale descriptor out of it.
    unsafe {
        core::ptr::write_bytes(
            crate::arch::mmu::phys_to_virt(phys) as *mut u8,
            0,
            (count * FRAME_SIZE) as usize,
        );
    }
    Some((phys, count))
}

/// **Retype one page for a kernel object, pinning the region in the same breath** (19a). Pin and
/// carve happen under one hold of the region lock, so no [`destroy`] can slip between them and
/// free a page that is about to hold an endpoint. Zeroed like every retyped page.
pub fn retype_object_page(region: u64, kind: ObjectKind) -> Option<u64> {
    // The test kernel's Nth-retype fault (milestone 757, provisional): refused before the lock,
    // so the region is left exactly as an exhausted one would be. Absent from every other build.
    #[cfg(feature = "system_tests")]
    if crate::retype_fault::fails_now() {
        return None;
    }
    // Released at the semicolon, as in `retype_page`: the pin and the carve are under the lock, the
    // zeroing is not.
    let page = REGIONS.lock().retype_object_page(region, kind)?;
    let phys = page * FRAME_SIZE;

    // SAFETY: as retype_page: exclusively ours, direct-mapped; zero before anyone reads it.
    unsafe {
        core::ptr::write_bytes(
            crate::arch::mmu::phys_to_virt(phys) as *mut u8,
            0,
            FRAME_SIZE as usize,
        );
    }
    Some(phys)
}

/// **One figure of a region's spending, for `MemoryRegion::USAGE`** (milestone 126 (the `procps` package), DECISIONS §225 (`free` sees the machine and your share)
/// part 1). `record` is an `abi::usage` selector the caller has already checked with
/// `abi::usage::is_known`; `None` for a dead region.
pub(crate) fn usage_record(region: u64, record: u64) -> Option<u64> {
    let table = REGIONS.lock();
    let on = match record {
        abi::usage::SIZE => return table.usage(region).map(|(_, pages)| pages),
        abi::usage::COMMITTED => return table.usage(region).map(|(spent, _)| spent),
        abi::usage::FRAMES => PageUse::Frames,
        abi::usage::RENDEZVOUS => PageUse::Object(ObjectKind::Rendezvous),
        abi::usage::ADDRESS_SPACES => PageUse::Object(ObjectKind::AddressSpace),
        abi::usage::THREADS => PageUse::Object(ObjectKind::Thread),
        _ => PageUse::Children,
    };
    table.spent(region, on)
}

/// How many pages the region has retyped, and its size. For the demo and tests.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // the untyped property test is the only caller
pub fn usage(region: u64) -> Option<(u64, u64)> {
    REGIONS.lock().usage(region)
}

/// This region's physical span `(base, size_in_bytes)`, or `None` if the name is stale. Object
/// revocation needs it to find which kernel objects live in the region (`sched::reclaim_region`
/// scans the registries for TCB/endpoint/address space pages that fall inside this span).
pub fn region_bounds(region: u64) -> Option<(u64, u64)> {
    REGIONS
        .lock()
        .bounds(region)
        .map(|(base_page, pages)| (base_page * FRAME_SIZE, pages * FRAME_SIZE))
}

/// Clear a region's object pin, **after** its objects have been torn down. Object revocation
/// (`sched::reclaim_region`): reap the objects with `IPC_TABLES`, unpin, then [`destroy`].
///
/// Separate from [`destroy`] because [`destroy`] must never take `IPC_TABLES`: it is reachable from
/// `AddressSpace::Drop`, which already runs under it. See notes/memory-regions.md.
pub fn unpin(region: u64) {
    REGIONS.lock().unpin(region);
}

/// Return a region's whole backing, **safely**: a root's pages to the frame allocator, a child's to
/// its parent. The name goes with it, and its slot is reused by the next `create` or [`split`].
///
/// Refused, doing nothing, for a pinned region, one with live children, or a name another caller
/// already claimed. Every mapped page is revoked first (§13), so no peer is left mapping a returned
/// page. `RegionTable::claim_for_destroy` decides and removes the name in one borrow, so at most
/// one caller frees a region's pages. Takes `REGIONS`, releasing it before the revoke takes
/// `IPC_TABLES`. See notes/memory-regions.md.
pub fn destroy(region: u64) {
    // The guard is a temporary of this `let`, so `REGIONS` is released at the semicolon, before
    // the revoke: revocation takes `IPC_TABLES`, a higher rank, and holding `REGIONS` across it
    // would invert the order.
    let Some(claim) = REGIONS.lock().claim_for_destroy(region) else {
        return; // refused (pinned, or has live children), or a racing caller already won
    };
    let base = claim.base_page() * FRAME_SIZE;
    let pages = claim.pages();

    // Unmap any page still mapped anywhere before the pages leave this region, whether they go back
    // to the allocator (a root) or back to the parent's budget (a child); a returned page that a
    // peer still maps would be the §13 use-after-free either way.
    crate::revoke::revoke_region(base, pages * FRAME_SIZE);

    if claim.is_root() {
        // A root region: its pages came from the frame allocator, so they go back to it. The proved
        // `destroy_outcome` guarantees only roots reach this path, which is what makes double-free
        // impossible: a page reaches the allocator only through the one root that owns it.
        for i in 0..pages {
            memory::free(PageFrame::from_addr(base + i * FRAME_SIZE));
        }
    } else {
        // A child region: its pages return to the parent, never the allocator. The LIFO test and the
        // un-bump happen under one hold, with the amount the proved `destroy_outcome` dictates.
        REGIONS.lock().return_to_parent(&claim);
    }
}
