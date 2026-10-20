//! **The `PageFrame` methods**: map, slice and revoke.
//!
//! Cut out of `syscall.rs` unchanged on 2026-10-12 (UTC) by lane maintainer/near-ceiling-splits,
//! so the parent stays under §266 (a Rust source file stays under 2,000 lines) and moves toward
//! §282 (when a file is split, the parent becomes an index). Each method keeps its
//! `#[inline(never)]`, so [`invoke`](super::invoke) reaches it through one call, as before, and
//! `script/fastpath-footprint` measures the same code under a longer symbol name. `pub(super)`
//! is the visibility a private item of `syscall` had: `syscall` and its children.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::address_space::{run_end_va, unmap_run_prefix};
use super::*;
use crate::arch::mmu;

/// `PageFrame::MAP`: map the run of frames the capability in `slot` names at consecutive pages
/// starting at `va` in the caller's own address space (`a1` writable 0/1, `a2` an untyped slot the
/// page tables come from). Un-share is `page_frame_revoke`; this is the other half.
/// `#[inline(never)]` for the reason `memory_region_map` gives.
///
/// §102: `count` is fixed on the capability, not passed here, so this is one `MAP` call regardless
/// of the run's length; a single-page frame (`count: 1`) runs the loop below once, exactly the
/// pre-§102 behavior.
///
/// **It takes the slot, not the capability `invoke` dispatched on** (the map-revocation-window
/// lane, 2026-10-04 UTC). That read was a critical section of its own, and a sweep could run
/// wholly between it and the record: the capability was deleted, the unmap pass scanned a log the
/// mapping was not yet in, and the mapping survived `PageFrame::REVOKE` and `MemoryRegion::DESTROY`
/// alike. `system_tests::user::map_revocation_window_tests` drove it. The frame is now read again
/// under the [`MappingHold`](crate::revoke::MappingHold) that maps and records it, and that hold
/// has the argument. A slot that holds something other than a `PageFrame` by then answers
/// `NoSuchSlot`: it was emptied in between, and an empty slot is what a `MAP` made at that instant
/// would have found.
#[inline(never)]
pub(super) fn page_frame_map(
    slot: u64,
    va: u64,
    writable: u64,
    ut_slot: u64,
) -> Result<i64, Error> {
    #[cfg(feature = "system_tests")]
    crate::delegation_pause::here(); // no lock held
    let mut hold = crate::revoke::hold();
    let frame = hold.current_cap(slot).map_err(|_| Error::NoSuchSlot)?;
    let Object::PageFrame(phys, count) = frame.object else {
        return Err(Error::NoSuchSlot);
    };
    let count = count.get();
    // Checked against the run's last page, not just its first: a `va` that only overflows partway
    // through the run must be refused before anything is mapped, the same "reject the cheap
    // failures before spending a page" discipline `memory_region_map` documents.
    let Some(last_va) = run_end_va(va, count) else {
        return Err(Error::BadPointer);
    };
    if !paging::is_user_page_va::<crate::arch::mmu::Format>(va)
        || !paging::is_user_page_va::<crate::arch::mmu::Format>(last_va)
    {
        return Err(Error::BadPointer);
    }
    // A read/write mapping needs WRITE on the frame; a read-only one needs READ. This is where a
    // delegated, narrowed frame is confined: a peer handed READ alone can map it to look, never
    // to change it. One check for the whole run: rights live on the capability, not per page.
    let flags = if writable != 0 {
        if !frame.rights.allows(Rights::WRITE) {
            return Err(Error::NotPermitted);
        }
        paging::Flags::user_data()
    } else {
        if !frame.rights.allows(Rights::READ) {
            return Err(Error::NotPermitted);
        }
        paging::Flags::user_rodata()
    };
    // Page tables come from an untyped the caller holds, so mapping a frame, like everything a
    // process spends, comes out of its own budget and not the kernel's. Read under the hold only
    // because it sits here in the refusal order: a region is generational (§16 (object revocation)), so one destroyed
    // after this read refuses to retype, and the hold is not what protects it.
    let ut = sched::current_cap(ut_slot).map_err(|_| Error::NoSuchSlot)?;
    let Object::MemoryRegion(region) = ut.object else {
        return Err(Error::WrongObject);
    };
    if !ut.rights.allows(Rights::WRITE) {
        return Err(Error::NotPermitted);
    }
    let root = mmu::current_user_root();
    // One invalidate for the whole run, when this drops, rather than one per page.
    let mut batch = mmu::UserMapBatch::new();
    for k in 0..count {
        let (page_phys, page_va) = (phys + k * paging::PAGE_SIZE, va + k * paging::PAGE_SIZE);
        // Each table is recorded against this space as it is retyped, because the region it comes
        // from need not be the space's own, and its `DESTROY` has to find the table to cut it out
        // of this walk before the page goes back (`revoke::revoke_region`).
        match batch.map(page_va, page_phys, flags, || {
            hold.retype_table(region, root, page_va)
        }) {
            Ok(()) => {
                // Record the mapping so a later REVOKE (or memory_region::destroy) can pull this
                // page out of every holder before it is reused (§13). Unrecordable means
                // unmappable, at the mapper's own expense (phase C): see MemoryRegion::MAP.
                // `phys` (the run's base, not `page_phys`) is the object: it names the capability
                // this mapping was made under, derivatives included, which is what lets a later
                // `REVOKE` take back this authority without touching an overlapping holder's
                // (DECISIONS §132).
                if !hold.record_mapping(
                    page_phys,
                    root,
                    page_va,
                    crate::revoke::PageMapSource::Capability(phys),
                ) {
                    mmu::unmap_user_at(root, page_va);
                    unmap_run_prefix(&mut hold, root, phys, va, k);
                    return Err(Error::OutOfMemory);
                }
            }
            // **All or nothing across the run** (milestone 142's review, MAJOR 2): see the same
            // rollback at `MAP_INTO`, which fails the same way for the same reasons.
            Err(e) => {
                unmap_run_prefix(&mut hold, root, phys, va, k);
                return Err(match e {
                    paging::MapError::OutOfPageFrames => Error::OutOfMemory,
                    // misaligned, already mapped, or wrong half
                    _ => Error::BadPointer,
                });
            }
        }
    }
    Ok(0)
}

/// `PageFrame::SLICE` (milestone 599, calef's option-4 ruling of 2026-09-27): a capability naming
/// the `len` pages `first` pages into this run, with the same rights. `abi::page_frame::SLICE` has
/// the rules; `#[inline(never)]` for the reason `memory_region_map` gives, since slicing is
/// spawn-time wiring and never a step of the IPC round trip.
#[inline(never)]
pub(super) fn page_frame_slice(slot: u64, first: u64, len: u64) -> Result<i64, Error> {
    // The source is re-read under the table lock the slice is filed under (`sched::grant_derived`),
    // not taken from the capability `invoke` dispatched on: that read was a critical section of its
    // own, and a reclamation sweep could delete the run between it and the filing, leaving a slice
    // of pages the allocator was about to reuse (`sched::Delegation`). The source that matters is the
    // one standing when the slice is filed.
    #[cfg(feature = "system_tests")]
    crate::delegation_pause::here(); // no lock held
    let slot = sched::grant_derived(slot, |src| {
        let Object::PageFrame(phys, count) = src.object else {
            return Err(Error::WrongObject);
        };
        if !src.rights.allows(Rights::GRANT) {
            return Err(Error::NotPermitted);
        }
        let (Some(end), Some(len)) = (first.checked_add(len), core::num::NonZeroU64::new(len))
        else {
            return Err(Error::BadPointer);
        };
        if end > count.get() {
            return Err(Error::BadPointer);
        }
        let base = phys + first * page_frames::FRAME_SIZE;
        Ok(crate::cap::page_frame_run_cap(base, len, src.rights))
    })?;
    Ok(slot as i64)
}

/// `PageFrame::REVOKE`: un-share the run of `count` frames starting at `phys` from every holder and
/// delete every capability naming the run, including the caller's own. Does not reclaim the pages
/// (untyped is spend-only); that is `MemoryRegion::DESTROY`. §13, §102.
/// `#[inline(never)]` for the reason `memory_region_map` gives.
#[inline(never)]
pub(super) fn page_frame_revoke(phys: u64, count: u64) -> Result<i64, Error> {
    crate::revoke::revoke_page_frame_run(phys, count);
    Ok(0)
}
