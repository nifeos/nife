//! **The `PageFrame` methods**: map, slice and revoke. See notes/syscall-boundary.md.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::address_space::{run_end_va, unmap_run_prefix};
use super::*;
use crate::arch::mmu;

/// `PageFrame::MAP`: map the whole run the frame in `slot` names at consecutive pages from `va` in
/// the caller's own space, read/write if `writable` is nonzero, with page tables retyped from the
/// region in `ut_slot`. `page_frame_revoke` is the other half. Maps the run or none of it.
///
/// It takes the slot, not the capability `invoke` read, and reads it again under the
/// [`MappingHold`](crate::revoke::MappingHold), which has the argument. A slot that no longer holds
/// a `PageFrame` answers `NoSuchSlot`, what a `MAP` made at that instant would have found.
///
/// Context: takes the mapping hold (`MAPPINGS`) for the frame read and the whole run.
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
    // Both ends, before anything is mapped, as `MAP_INTO` checks them.
    let Some(last_va) = run_end_va(va, count) else {
        return Err(Error::BadPointer);
    };
    if !paging::is_user_page_va::<crate::arch::mmu::Format>(va)
        || !paging::is_user_page_va::<crate::arch::mmu::Format>(last_va)
    {
        return Err(Error::BadPointer);
    }
    // Where a narrowed frame is confined: READ alone maps it to look, never to change it.
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
    // Under the hold only for the refusal order: a region destroyed after this read refuses to
    // retype, because it is generational (§16 (object revocation)).
    let ut = sched::current_cap(ut_slot).map_err(|_| Error::NoSuchSlot)?;
    let Object::MemoryRegion(region) = ut.object else {
        return Err(Error::WrongObject);
    };
    if !ut.rights.allows(Rights::WRITE) {
        return Err(Error::NotPermitted);
    }
    let root = mmu::current_user_root();
    // One invalidate for the whole run, when this drops.
    let mut batch = mmu::UserMapBatch::new();
    for k in 0..count {
        let (page_phys, page_va) = (phys + k * paging::PAGE_SIZE, va + k * paging::PAGE_SIZE);
        // Each table is recorded as it is retyped: its region need not be this space's, and its
        // `DESTROY` must find the table to cut it out of this walk (`revoke::revoke_region`).
        match batch.map(page_va, page_phys, flags, || {
            hold.retype_table(region, root, page_va)
        }) {
            Ok(()) => {
                // Unrecordable means unmappable (§13 (capability revocation and untyped
                // reclamation)). The source is `phys`, the run's base, which a `REVOKE` scopes by
                // (DECISIONS §132 (what `PageFrame::REVOKE` owes an overlapping run)).
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
            // All or nothing, as `MAP_INTO`.
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

/// `PageFrame::SLICE`: a new capability, with the same rights, naming the `len` pages `first`
/// pages into the run in `slot`. Returns its slot. Needs `GRANT`; `BadPointer` for an empty or
/// out-of-range window. `abi::page_frame::SLICE` has the rules.
#[inline(never)]
pub(super) fn page_frame_slice(slot: u64, first: u64, len: u64) -> Result<i64, Error> {
    // The source is read under the table lock the slice is filed under, so no reclamation sweep
    // falls between the read and the filing (`sched::Delegation`).
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

/// `PageFrame::REVOKE`: un-share the run of `count` frames at `phys` from every holder and delete
/// every capability naming it, the caller's own included. The pages are not reclaimed; that is
/// `MemoryRegion::DESTROY` (§13 (capability revocation and untyped reclamation)).
#[inline(never)]
pub(super) fn page_frame_revoke(phys: u64, count: u64) -> Result<i64, Error> {
    crate::revoke::revoke_page_frame_run(phys, count);
    Ok(0)
}
