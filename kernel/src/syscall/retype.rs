//! **`MemoryRegion::RETYPE_OBJ`**: a kernel object made in a region's memory, and the capability to
//! it filed in the caller's table.
//!
//! The third word is a floor: the capability lands in the first free slot at or above it, and `0`
//! is the first free slot. A `std` program's low slots are fixed by number and an empty one means
//! "not granted" (`std_runtime_contract`), so what it makes for itself must land above them.
//!
//! Name: provisional (milestone 812's lane, 2026-10-10 UTC).

use abi::Error;

use crate::cap::{Capability, Object, Rights};
use crate::sched;

/// `MemoryRegion::RETYPE_OBJ`: retype a page of `region` into a kernel object of `kind` and file a
/// capability to it at or above `floor`; returns the slot. The object lives in the carved page, and
/// the region is pinned: a live endpoint's page must never be freed under a blocked thread.
/// `space_slot` names the address space a `PROCESS` holds. `BadMethod` for an unknown kind.
///
/// Every kind carries the budget's rights and no others, through `budget.mint_child` (milestone
/// 824 (a retype mints no right its budget lacks)). The root budget is `Rights::ALL`, so the
/// progenitor still gets every right on what it makes.
#[inline(never)]
pub(super) fn memory_region_retype_obj(
    budget: Capability,
    region: u64,
    kind: u64,
    space_slot: u64,
    floor: u64,
) -> Result<i64, Error> {
    match kind {
        abi::objtype::RENDEZVOUS => {
            let ep = sched::create_rendezvous_from(region).ok_or(Error::OutOfMemory)?;
            // The budget's rights, never a hand-written list, which would stop being full the
            // day a right is added.
            let slot = sched::grant_from(floor, budget.mint_child(Object::Rendezvous(ep, 0)))
                .map_err(|_| Error::OutOfMemory)?;
            Ok(slot as i64)
        }
        // The page becomes the root table, and the region backs the space's tables and records
        // (design/init-and-granular-spawn.md).
        abi::objtype::ADDRESS_SPACE => {
            let name = crate::user::user_address_space_create(region).ok_or(Error::OutOfMemory)?;
            let slot = sched::grant_from(floor, budget.mint_child(Object::AddressSpace(name)))
                .map_err(|_| Error::OutOfMemory)?;
            Ok(slot as i64)
        }
        // An embryo, in no queue and not runnable until `CONFIGURE` and `START`.
        abi::objtype::THREAD_CONTROL_BLOCK => {
            let tid = sched::create_thread_control_block(region).ok_or(Error::OutOfMemory)?;
            let slot = sched::grant_from(floor, budget.mint_child(Object::ThreadControlBlock(tid)))
                .map_err(|_| Error::OutOfMemory)?;
            Ok(slot as i64)
        }
        abi::objtype::NOTIFICATION => {
            let id = sched::create_notification_from(region).ok_or(Error::OutOfMemory)?;
            let slot = sched::grant_from(floor, budget.mint_child(Object::Notification(id)))
                .map_err(|_| Error::OutOfMemory)?;
            Ok(slot as i64)
        }
        abi::objtype::TIMER => {
            let id = sched::create_timer_from(region).ok_or(Error::OutOfMemory)?;
            let slot = sched::grant_from(floor, budget.mint_child(Object::Timer(id)))
                .map_err(|_| Error::OutOfMemory)?;
            Ok(slot as i64)
        }
        // `space_slot` needs `WRITE` on the space and is consumed, as `CONFIGURE` consumes one.
        abi::objtype::PROCESS => {
            let space = sched::current_cap(space_slot).map_err(|_| Error::NoSuchSlot)?;
            let crate::cap::Object::AddressSpace(name) = space.object else {
                return Err(Error::WrongObject);
            };
            if !space.rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            let pid = sched::create_process_from(region, name)?;
            let _ = sched::delete_current_cap(space_slot);
            let slot = sched::grant_from(floor, budget.mint_child(Object::Process(pid)))
                .map_err(|_| Error::OutOfMemory)?;
            Ok(slot as i64)
        }
        _ => Err(Error::BadMethod), // no such object type
    }
}
