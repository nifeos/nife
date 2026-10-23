//! **The `MemoryRegion` methods**: map, retype, split, destroy and the usage records, and the arm
//! of [`invoke`](super::invoke) that decodes them. See notes/syscall-boundary.md.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::*;
use crate::arch::mmu;

/// **The `MemoryRegion` arm of [`invoke`](super::invoke)**: the WRITE check each spending method
/// asks for, then the method's out-of-line body.
///
/// `#[inline(always)]`, unlike the bodies, because this is decoder and `script/fastpath-footprint`
/// counts it inside `syscall_entry` (notes/syscall-boundary.md).
///
/// Name: provisional (lane maintainer/long-function-paydown-invoke, 2026-10-12 UTC).
#[inline(always)]
pub(super) fn memory_region_invoke(
    capability: crate::cap::Cap,
    region: u64,
    method: u64,
    [a0, a1, a2]: [u64; 3],
) -> Result<i64, Error> {
    match method {
        abi::memory_region::MAP => {
            if !capability.rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            memory_region_map(region, a0)
        }
        abi::memory_region::RETYPE_OBJ => {
            if !capability.rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            super::retype::memory_region_retype_obj(capability, region, a0, a1, a2)
        }
        abi::memory_region::RETYPE => {
            if !capability.rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            memory_region_retype(capability, region, a0)
        }
        abi::memory_region::SPLIT => {
            if !capability.rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            memory_region_split(capability, region, a0)
        }
        abi::memory_region::DESTROY => {
            if !capability.rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            memory_region_destroy(region)
        }
        // `ENUMERATE` alone, checked inside: learning what a budget went to is not the authority
        // to spend it (DECISIONS §225 (`free` sees the machine and your share)).
        abi::memory_region::USAGE => memory_region_usage(capability, region, a0),
        _ => Err(Error::BadMethod),
    }
}

/// `MemoryRegion::MAP`: retype a page out of the region and map it, writable, at `va` in the
/// caller's own address space. The page and any page tables come from the region, so the kernel
/// allocates nothing. Returns 0; `BadPointer` for a `va` that cannot be mapped, `OutOfMemory` when
/// the region or the mapping record cannot be paid for.
///
/// Context: takes the mapping hold ([`MappingHold`](crate::revoke::MappingHold), `MAPPINGS`) across
/// the retype, the map and the record, so a `DESTROY` either refuses the retype or finds the
/// record; beneath it, `MEMORY_REGION` for each page retyped.
#[inline(never)]
pub(super) fn memory_region_map(region: u64, va: u64) -> Result<i64, Error> {
    // Before the retype, or each refused address spends a page of the caller's region. An
    // already-mapped `va` still costs one page.
    if !paging::is_user_page_va::<crate::arch::mmu::Format>(va) {
        return Err(Error::BadPointer);
    }
    #[cfg(feature = "system_tests")]
    crate::delegation_pause::here(); // no lock held
    let mut hold = crate::revoke::hold();
    let root = mmu::current_user_root();
    // The leaf, then the tables, each table recorded as it is retyped: `DESTROY` must find the
    // tables to cut them out of this walk before the pages go back (`revoke::revoke_region`).
    let Some(phys) = crate::memory_region::retype_page(region) else {
        return Err(Error::OutOfMemory);
    };
    match mmu::map_current_user_page_frame(va, phys, paging::Flags::user_data(), || {
        hold.retype_table(region, root, va)
    }) {
        Ok(()) => {
            // No record, no mapping: an unrecorded mapping is invisible to revocation (§13
            // (capability revocation and untyped reclamation)). No capability names the page.
            if !hold.record_mapping(phys, root, va, crate::revoke::PageMapSource::NoCapability) {
                mmu::unmap_user_at(root, va);
                return Err(Error::OutOfMemory);
            }
            Ok(0)
        }
        Err(paging::MapError::OutOfPageFrames) => Err(Error::OutOfMemory),
        Err(_) => Err(Error::BadPointer), // misaligned, already mapped, or wrong half
    }
}

/// `MemoryRegion::RETYPE`: retype `requested` pages (`0` meaning one) into one `PageFrame`
/// capability over the run (§102 (a Frame names a run of pages)), mapped nowhere yet. Returns the
/// slot; `OutOfMemory` when the region or the table is full. The frame carries the budget's
/// rights and no others (milestone 824 (a retype mints no right its budget lacks)), so `GRANT` is
/// delegated down the budget tree and never minted at a leaf.
///
/// # BUGS
///
/// - A full capability table leaves the run retyped and unnamed until the region is destroyed.
/// - `RETYPE_OBJ`'s arms have the same shape: an object nobody can name.
#[inline(never)]
pub(super) fn memory_region_retype(
    capability: Capability,
    region: u64,
    requested: u64,
) -> Result<i64, Error> {
    let (phys, count) =
        crate::memory_region::retype_run(region, requested).ok_or(Error::OutOfMemory)?;
    // The run is non-empty by construction (`retype_pages` never returns zero).
    let count = core::num::NonZeroU64::new(count).ok_or(Error::OutOfMemory)?;
    // `mint_child`, the inheriting mint `split_never_widens_rights` proves.
    let slot = sched::grant(capability.mint_child(Object::PageFrame(phys, count)))
        .map_err(|_| Error::OutOfMemory)?;
    Ok(slot as i64)
}

/// `MemoryRegion::SPLIT`: carve a child region of `count` pages off this one, so a spawner can
/// give each child its own reclaimable region. Returns the child's slot; `OutOfMemory` when the
/// region or the caller's table is full, with the parent left as it was.
#[inline(never)]
pub(super) fn memory_region_split(
    cap: crate::cap::Cap,
    region: u64,
    count: u64,
) -> Result<i64, Error> {
    let child = crate::memory_region::split(region, count).ok_or(Error::OutOfMemory)?;
    // A fresh mint, so it must not widen by hand: the child gets this capability's rights,
    // never more (`split_never_widens_rights` proves `mint_child`). A full table gives the child
    // back; it is childless and at the top of the parent's watermark, so `destroy` undoes it.
    match sched::grant(cap.mint_child(crate::cap::Object::MemoryRegion(child))) {
        Ok(slot) => Ok(slot as i64),
        Err(_) => {
            crate::memory_region::destroy(child);
            Err(Error::OutOfMemory) // capability table full
        }
    }
}

/// `MemoryRegion::DESTROY`: reclaim this region and every object retyped from it, and return the
/// memory (§16 (object revocation)). `NotPermitted` while a live thread occupies it or while it
/// has children. Every capability to a reclaimed object is stale on its next use.
#[inline(never)]
pub(super) fn memory_region_destroy(region: u64) -> Result<i64, Error> {
    sched::reclaim_region(region).map_err(|_| Error::NotPermitted)?;
    Ok(0)
}

/// `MemoryRegion::USAGE`: how many pages of the region went to `record`. Needs `ENUMERATE`;
/// `BadMethod` for an unknown record, checked before the region is looked up, and `Gone` for a
/// region that no longer resolves.
#[inline(never)]
pub(super) fn memory_region_usage(
    cap: crate::cap::Cap,
    region: u64,
    record: u64,
) -> Result<i64, Error> {
    if !cap.rights.allows(Rights::ENUMERATE) {
        return Err(Error::NotPermitted);
    }
    if !abi::usage::is_known(record) {
        return Err(Error::BadMethod);
    }
    crate::memory_region::usage_record(region, record)
        .map(|pages| pages as i64)
        .ok_or(Error::Gone)
}
