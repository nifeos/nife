//! **The `AddressSpace` methods**: map into, unmap, the futex pair and the listing, with the run
//! arithmetic `PageFrame::MAP` shares. See notes/syscall-boundary.md.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::*;
use crate::arch::mmu;

/// Decode an `AddressSpace` method on the space `name` and run it, out of line like every method
/// off the IPC round trip (notes/syscall-boundary.md).
#[inline(never)]
pub(super) fn address_space_invoke(
    frame: &mut TrapFrame,
    cap: crate::cap::Cap,
    name: u64,
    method: u64,
    [a0, a1, a2]: [u64; 3],
) -> Result<i64, Error> {
    match method {
        abi::address_space::MAP_INTO => address_space_map_into(cap, name, a0, a1, a2),
        // `ENUMERATE`, not `WRITE`: seeing a space is not shaping it (DECISIONS §114 (`pmap`
        // gets its listing: `ENUMERATE` extends to the address-space object)).
        abi::address_space::LIST => {
            if !cap.rights.allows(Rights::ENUMERATE) {
                return Err(Error::NotPermitted);
            }
            address_space_list(frame, name, a0)
        }
        // `WRITE`, the right `MAP_INTO` takes: shaping a space is one right in both directions.
        abi::address_space::UNMAP => {
            if !cap.rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            address_space_unmap(name, a0)
        }
        // §269 (how threads share a process) fork 2.
        abi::address_space::WAIT | abi::address_space::WAKE => {
            address_space_futex(cap.rights, name, method, a0, a1, a2)
        }
        _ => Err(Error::BadMethod),
    }
}

/// `AddressSpace::MAP_INTO`: map the frame in slot `a1` at `a0` in the space `name`, shaped by
/// `a2` for a `PageFrame` (`MAP_RW`, `MAP_CODE`, else read-only), device read/write for a
/// `DeviceFrame`. The other half of [`page_frame_map`](super::page_frame::page_frame_map).
///
/// Needs `WRITE` on the space and the shape's right on the frame. Maps the whole run or none of
/// it: `BadPointer` for a run outside the user half or a space that does not resolve,
/// `OutOfMemory` when page tables run out. See notes/syscall-boundary.md.
///
/// Context: takes `USER_SPACES` (rank `ADDRESS_SPACES`), then the mapping hold (`MAPPINGS`), and
/// keeps both from the frame read to the end of the run.
#[inline(never)]
fn address_space_map_into(
    cap: crate::cap::Cap,
    name: u64,
    a0: u64,
    a1: u64,
    a2: u64,
) -> Result<i64, Error> {
    if !cap.rights.allows(Rights::WRITE) {
        return Err(Error::NotPermitted);
    }
    let va = a0;
    // The frame is read under the hold that maps and records it, so no sweep falls between the
    // read and the record (`revoke::MappingHold`). The space registry ranks above that hold.
    #[cfg(feature = "system_tests")]
    crate::delegation_pause::here(); // no lock held
    crate::user::with_user_address_space(name, |space| {
        let mut hold = crate::revoke::hold();
        let frame = hold.current_cap(a1).map_err(|_| Error::NoSuchSlot)?;
        let (phys, count, flags) = match frame.object {
            Object::DeviceFrame(phys) => {
                if !frame.rights.allows(Rights::WRITE) {
                    return Err(Error::NotPermitted);
                }
                (phys, 1u64, paging::Flags::user_device())
            }
            Object::PageFrame(phys, count) => {
                // Code is W^X (`user_code` is never writable), so it needs only READ.
                let flags = match a2 {
                    abi::address_space::MAP_RW => {
                        if !frame.rights.allows(Rights::WRITE) {
                            return Err(Error::NotPermitted);
                        }
                        paging::Flags::user_data()
                    }
                    abi::address_space::MAP_CODE => {
                        if !frame.rights.allows(Rights::READ) {
                            return Err(Error::NotPermitted);
                        }
                        paging::Flags::user_code()
                    }
                    _ => {
                        if !frame.rights.allows(Rights::READ) {
                            return Err(Error::NotPermitted);
                        }
                        paging::Flags::user_rodata()
                    }
                };
                (phys, count.get(), flags)
            }
            _ => return Err(Error::WrongObject),
        };
        // Both ends of the run, before any page is mapped; `syscall::proofs` shows the pages
        // between need no check of their own.
        let Some(last_va) = run_end_va(va, count) else {
            return Err(Error::BadPointer);
        };
        if !paging::is_user_page_va::<crate::arch::mmu::Format>(va)
            || !paging::is_user_page_va::<crate::arch::mmu::Format>(last_va)
        {
            return Err(Error::BadPointer);
        }
        // An absent space fails here, before the loop spends a page table on it.
        let Some(space) = space else {
            return Err(Error::BadPointer);
        };
        let root = space.root();
        for k in 0..count {
            let (page_phys, page_va) = (phys + k * paging::PAGE_SIZE, va + k * paging::PAGE_SIZE);
            // `phys` is the object the capability names in both arms (§132 (what
            // `PageFrame::REVOKE` owes an overlapping run)).
            match space.map_physical_held(
                &mut hold,
                page_va,
                page_phys,
                flags,
                crate::revoke::PageMapSource::Capability(phys),
            ) {
                Ok(()) => {
                    // The instruction fetcher must see what the writer stored, or the child
                    // takes an illegal-instruction fault at its entry; `user/loader.rs` does the
                    // same for the kernel's own loads.
                    if flags.is_user_executable() {
                        crate::arch::sync_icache(
                            crate::arch::mmu::phys_to_virt(page_phys),
                            paging::PAGE_SIZE as usize,
                        );
                    }
                }
                // All or nothing: undo what this run mapped before it failed.
                Err(e) => {
                    unmap_run_prefix(&mut hold, root, phys, va, k);
                    return Err(match e {
                        paging::MapError::OutOfPageFrames => Error::OutOfMemory,
                        // misaligned, already mapped, unknown space
                        _ => Error::BadPointer,
                    });
                }
            }
        }
        Ok(0)
    })
}

/// `AddressSpace::UNMAP`: take the page at `va` out of the space `name`, out of every core's TLB
/// and out of the space's mapping record (DECISIONS §162 (whether a holder can give up a mapping),
/// option A). Returns 0, or `BadPointer` when nothing is mapped there; no capability is read,
/// consumed or changed (both as §162's 2026-10-05 amendment rules). The TLB flush is
/// `mmu::unmap_user_at`'s, the function every revoke unmaps with. See notes/unmap.md.
///
/// Context: takes `USER_SPACES` (rank `ADDRESS_SPACES`), then the mapping hold (`MAPPINGS`), the
/// order `MAP_INTO` takes them, so the two are ordered whole on one `va`.
#[inline(never)]
fn address_space_unmap(name: u64, va: u64) -> Result<i64, Error> {
    if !paging::is_user_page_va::<crate::arch::mmu::Format>(va) {
        return Err(Error::BadPointer);
    }
    crate::user::with_user_address_space(name, |space| {
        // A space that no longer resolves has nothing this capability can reach.
        let Some(space) = space else {
            return Err(Error::BadPointer);
        };
        let root = space.root();
        let mut hold = crate::revoke::hold();
        // Both halves, neither short-circuiting the other: a page with no record is still a window,
        // and a record with no page is still one a later revoke would act on.
        let unmapped = mmu::unmap_user_at(root, va);
        let forgotten = hold.forget_mapping_at(root, va);
        if unmapped.is_none() && forgotten.is_none() {
            return Err(Error::BadPointer);
        }
        Ok(0)
    })
}

/// The last virtual address of a `count`-page run starting at `va`, or `None` if the run does not
/// fit in a `u64`, or `count` is zero. The multiply is checked as well as the add. Both mapping
/// paths guard on it, and `syscall::proofs` proves it exact.
pub(super) fn run_end_va(va: u64, count: u64) -> Option<u64> {
    va.checked_add(count.checked_sub(1)?.checked_mul(paging::PAGE_SIZE)?)
}

/// Undo the first `mapped` pages of a failed run-map in the space rooted at `root`: unmap each
/// page and forget its mapping record, leaving the space as the call found it.
///
/// # BUGS
///
/// The page tables the prefix retyped are not returned: a region is spend-only, so a failed
/// multi-page map still costs the caller those tables. Recorded in notes/frames.md.
pub(super) fn unmap_run_prefix(
    hold: &mut crate::revoke::MappingHold,
    root: u64,
    phys: u64,
    va: u64,
    mapped: u64,
) {
    for k in 0..mapped {
        let (page_phys, page_va) = (phys + k * paging::PAGE_SIZE, va + k * paging::PAGE_SIZE);
        mmu::unmap_user_at(root, page_va);
        hold.forget_mapping(page_phys, root, page_va);
    }
}

/// `AddressSpace::WAIT` and `WAKE`: the checks every futex method shares, then the scheduler.
/// The order of the refusals is the contract `abi::address_space::WAIT` states: the flags first
/// (`BadMethod`, a form this kernel does not answer), then the right (`NotPermitted`), the address
/// (`BadPointer`), and last whether the space is the caller's own (`WrongObject`).
#[inline(never)]
fn address_space_futex(
    rights: Rights,
    space: u64,
    method: u64,
    va: u64,
    flags: u64,
    word: u64,
) -> Result<i64, Error> {
    if !abi::futex::admitted(flags) {
        return Err(Error::BadMethod);
    }
    if !rights.allows(Rights::READ) {
        return Err(Error::NotPermitted);
    }
    if !va.is_multiple_of(4)
        || !<crate::arch::mmu::Format as paging::PageFormat>::is_in_half(paging::Half::Low, va)
    {
        return Err(Error::BadPointer);
    }
    if !sched::is_current_space(space) {
        return Err(Error::WrongObject);
    }
    if method == abi::address_space::WAIT {
        // The expected value is the low 32 bits; the size admitted is 32 (§269).
        sched::futex_wait(space, va, word as u32).map(|r| r as i64)
    } else {
        Ok(sched::futex_wake(space, va, word) as i64)
    }
}

/// `AddressSpace::LIST`: one entry of what the space `name` has mapped, from `cursor` (0 to
/// start). Writes the entry's `va` and kind to argument registers 1 and 2 and returns the next
/// cursor, or `survey::DONE`; a foreign cursor is `BadPointer` (DECISIONS §114).
#[inline(never)]
fn address_space_list(frame: &mut TrapFrame, name: u64, cursor: u64) -> Result<i64, Error> {
    // A dead space lists as empty, not refused: the capability is real and never widened.
    let Some(root) = crate::user::user_address_space_root(name) else {
        frame.set_arg(1, 0);
        frame.set_arg(2, 0);
        return Ok(abi::survey::DONE as i64);
    };
    // `list_mapping` checks the caller's cursor under the hold that walks the chain. An entry
    // whose `va` no longer translates (a revoke raced it) is skipped; the log is finite.
    let mut cursor = cursor;
    loop {
        match crate::revoke::list_mapping(root, cursor) {
            crate::revoke::Listing::Done => {
                frame.set_arg(1, 0);
                frame.set_arg(2, 0);
                return Ok(abi::survey::DONE as i64);
            }
            // Only the caller's own word can be foreign; §114's 2026-10-06 addendum answers it
            // `BadPointer`.
            crate::revoke::Listing::ForeignCursor => return Err(Error::BadPointer),
            crate::revoke::Listing::Entry(next, va) => {
                if let Some((_, flags)) = crate::arch::mmu::translate_at(root, va) {
                    let kind = if flags.is_user_executable() {
                        abi::address_space::MAP_CODE
                    } else if flags.is_writable() {
                        abi::address_space::MAP_RW
                    } else {
                        abi::address_space::MAP_RO
                    };
                    frame.set_arg(1, va);
                    frame.set_arg(2, kind);
                    return Ok(next as i64);
                }
                cursor = next;
            }
        }
    }
}
