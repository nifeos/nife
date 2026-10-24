//! The current thread's capabilities, and deleting capabilities across every table: lookup,
//! `grant` and delegation, and the page-frame, device-frame and port-range deleters, which walk
//! every thread's table under `IPC_TABLES` and so stay in `sched` (§271 part 2). See
//! notes/capability-lifecycle.md ("The sweeps that delete capabilities from every table").
//!
//! Name: provisional (milestone 840's lane, 2026-10-11 UTC; §271 (the scheduler file splits into
//! submodules of `sched`)).

use super::*;

/// Delete every `PageFrame` capability naming exactly the run `(phys, count)` from every thread's
/// table and hand-off slot, the caller's included: `PageFrame::REVOKE`'s sweep (§13 (capability
/// revocation and untyped reclamation), §102 (a frame names a run of pages)).
///
/// Matches by object equality: a narrowed copy matches, a different sub-range of the same memory
/// does not. A `count` of 0 deletes nothing. Reclamation wants
/// [`delete_page_frame_caps_overlapping`] instead. See notes/capability-lifecycle.md.
///
/// Context: takes `IPC_TABLES`, then each table's lock.
pub(crate) fn delete_page_frame_caps(phys: u64, count: u64) {
    let Some(count) = core::num::NonZeroU64::new(count) else {
        return;
    };
    let target = crate::cap::Object::PageFrame(phys, count);
    delete_page_frame_caps_where(|object| *object == target);
}

/// Delete every `PageFrame` capability whose run overlaps `[base, base + size)` from every thread's
/// table and hand-off slot: the reclamation sweep, run before `memory_region::destroy` returns the
/// range to the allocator.
///
/// Overlap, because equality misses a run capability that covers a page of the range. The end
/// saturates, so a run near `u64::MAX` overlaps everything rather than wrapping out. See
/// notes/capability-lifecycle.md.
///
/// Context: takes `IPC_TABLES`, then each table's lock.
pub(crate) fn delete_page_frame_caps_overlapping(base: u64, size: u64) {
    let end = base.saturating_add(size);
    delete_page_frame_caps_where(|object| match object {
        crate::cap::Object::PageFrame(phys, count) => {
            let run_end = phys.saturating_add(count.get().saturating_mul(page_frames::FRAME_SIZE));
            *phys < end && base < run_end
        }
        _ => false,
    });
}

/// The body both `PageFrame` sweeps share: delete every slot whose object satisfies `matches`, in
/// every thread's table, and the [`Thread::outgoing_cap`] parked in its hand-off slot.
///
/// A capability in flight to a receiver is in no table, so a sweep of tables alone would leave it
/// alive. The send still delivers its data word and the receiver sees `NO_CAP`. See
/// notes/capability-lifecycle.md.
///
/// Context: takes `IPC_TABLES`, then each table's lock.
pub(super) fn delete_page_frame_caps_where(matches: impl Fn(&crate::cap::Object) -> bool) {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return;
    };
    for (t, caps) in sched.threads.iter_mut_with_capabilities() {
        caps.lock().delete_matching(&matches);
        if matches!(t.outgoing_cap, Some(c) if matches(&c.object)) {
            t.outgoing_cap = None;
        }
    }
}

/// Delete every `DeviceFrame` capability naming `phys` from every table and hand-off slot except
/// the calling thread's (§41 (the endpoint is the broker)). The caller keeps its own: this takes a
/// device back to hand on. [`crate::revoke::revoke_device_from_others`] has the reasoning.
///
/// Context: takes `IPC_TABLES`, then each table's lock.
pub(crate) fn delete_device_frame_caps_from_others(phys: u64) {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return;
    };
    let keeper = current_thread_id();
    let target = crate::cap::Object::DeviceFrame(phys);
    for (t, owned) in sched.threads.iter_mut_with_capabilities() {
        if t.id == keeper {
            continue;
        }
        let mut table = owned.lock();
        for slot in 0..table.len() as u64 {
            if table.get(slot).is_ok_and(|c| c.object == target) {
                let _ = table.delete(slot);
            }
        }
        drop(table);
        if matches!(t.outgoing_cap, Some(c) if c.object == target) {
            t.outgoing_cap = None;
        }
    }
}

/// Take a port range back from every other thread: `PortRange::REVOKE`'s body. `x86_64` only.
///
/// Deletes every `PortRange(base, count)` capability but the caller's, clears each affected
/// thread's cached grant, and resets every other core's TSS bitmap, so the holder faults on its
/// next `in`/`out`. See notes/capability-lifecycle.md.
///
/// # BUGS
///
/// Matches by object equality, so a capability over a wider or narrower range of the same ports is
/// left alone. No syscall can name a sub-range today (notes/capability-lifecycle.md, "BUGS").
#[cfg(target_arch = "x86_64")]
pub fn delete_port_range_caps_from_others(base: u16, count: u16) {
    delete_port_range_caps_impl(base, count, Some(current_thread_id()));
}

/// Take a port range back from everyone, the invoker included: the whole-machine revoke the
/// kernel's tests use. No syscall reaches it. `x86_64` only.
#[cfg(target_arch = "x86_64")]
#[cfg_attr(
    not(any(test, feature = "system_tests", initial_ramdisk)),
    allow(dead_code)
)]
pub fn delete_port_range_caps(base: u16, count: u16) {
    delete_port_range_caps_impl(base, count, None);
}

/// The body of both port-range sweeps. `keeper` is the thread spared (the take-back's invoker), or
/// `None` for the whole-machine sweep. `x86_64` only.
///
/// Context: takes `IPC_TABLES`, then each table's lock, and sends the TSS broadcast under it.
#[cfg(target_arch = "x86_64")]
pub(super) fn delete_port_range_caps_impl(base: u16, count: u16, keeper: Option<ThreadId>) {
    {
        let mut guard = IPC_TABLES.lock();
        let Some(sched) = guard.as_mut() else {
            return;
        };
        let target = crate::cap::Object::PortRange(base, count);
        for (t, caps) in sched.threads.iter_mut_with_capabilities() {
            if Some(t.id) == keeper {
                continue;
            }
            let mut table = caps.lock();
            for slot in 0..table.len() as u64 {
                if table.get(slot).is_ok_and(|c| c.object == target) {
                    let _ = table.delete(slot);
                }
            }
            drop(table);
            // A `PortRange` in flight is in no table; see `delete_page_frame_caps_where`.
            if matches!(t.outgoing_cap, Some(c) if c.object == target) {
                t.outgoing_cap = None;
            }
            // Forget the cached grant if it named the revoked range, so switching to this thread
            // installs nothing. x86 only; the field exists nowhere else.
            #[cfg(target_arch = "x86_64")]
            if t.port_range_grant == Some((base, count)) {
                t.port_range_grant = None;
            }
        }
        // Inside the lock: every TSS bitmap writer holds `IPC_TABLES`, so the revocation NMI
        // cannot land inside `install_port_grant` and no core can install a grant this sweep
        // cleared. The invoker's own core keeps its bitmap, which is the invoker's own grant.
        #[cfg(target_arch = "x86_64")]
        crate::arch::segments::revoke_port_grant_everywhere(base, count, keeper.is_some());
    }
}

/// Remove the capability in `slot` from the current thread's table. This is how a one-shot `Reply`
/// is consumed the instant it is invoked (§12 (call/reply IPC)). On `x86_64`, deleting the
/// `PortRange` behind the thread's cached grant also drops the grant and resets this core's TSS
/// bitmap. Returns the table's error for an empty slot. See notes/capability-lifecycle.md.
///
/// Context: takes the thread's own table lock; on `x86_64` a deleted `PortRange` then takes
/// `IPC_TABLES`.
///
/// # BUGS
/// Deleting one of two copies of one `PortRange` drops the grant both name (fails safe).
pub fn delete_current_cap(slot: u64) -> Result<(), crate::cap::Error> {
    // Read before the delete, under the same hold: a deleted slot names nothing.
    let deleted = {
        let mut t = current_capabilities().ok_or(crate::cap::Error::NoSuchSlot)?;
        let capability = t.get(slot)?;
        t.delete(slot)?;
        capability
    };
    #[cfg(target_arch = "x86_64")]
    if let crate::cap::Object::PortRange(base, count) = deleted.object {
        let mut guard = IPC_TABLES.lock();
        let current = current_thread_id();
        let held = guard
            .as_mut()
            .and_then(|sched| sched.threads.get_mut(current))
            .filter(|t| t.port_range_grant == Some((base, count)));
        if let Some(t) = held {
            t.port_range_grant = None;
            // Under the lock, as every TSS bitmap writer is. This core is the only one to tell:
            // a core's bitmap permits a range only for the thread running on it, the caller.
            crate::arch::segments::revoke_installed_port_grant(base, count);
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = deleted; // no capability is enforced outside the table on this architecture
    Ok(())
}

/// Look up a capability in the **current thread's** table.
///
/// `slot` comes from userspace and indexes a table in kernel memory that userspace has never seen.
/// An empty slot is `NoSuchSlot`: there is nothing there, not "permission denied". Takes the
/// running thread's own table lock, not `IPC_TABLES`.
///
/// `#[inline(never)]` because `script/fastpath-footprint` counts it once as a root and
/// `syscall::dispatch` flat (milestone 758 (the IPC fast paths shrink back inside their band));
/// inlined, it lands on aarch64's `syscall_entry`.
#[inline(never)]
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(
    target_os = "none",
    unsafe(link_section = ".text.hot.sched.current_cap")
)]
pub fn current_cap(slot: u64) -> Result<crate::cap::Cap, crate::cap::Error> {
    // The lock-wait instrument marks this call as `site=current_cap`. The closure takes no other
    // lock, so that marks exactly the table lock's acquisition (`crate::lock_wait`).
    #[cfg(feature = "lock_wait")]
    crate::lock_wait::enter_current_cap();
    let table = current_capabilities();
    #[cfg(feature = "lock_wait")]
    crate::lock_wait::leave_current_cap();
    match table {
        Some(table) => table.get(slot),
        None => Err(crate::cap::Error::NoSuchSlot),
    }
}

/// Hand the current thread a capability. **The only way authority ever enters a process.**
///
/// The running thread's own table, under its own lock and not `IPC_TABLES`, as [`current_cap`].
pub fn grant(capability: crate::cap::Cap) -> Result<u64, crate::cap::Error> {
    current_capabilities().map_or(Err(crate::cap::Error::NoFreeSlot), |mut t| {
        t.insert(capability)
    })
}

/// `(occupied, capacity)` of the running thread's capability table, for the test runner's repeat
/// mode (`testing::runner`), which refuses before a slot leaked per run fills the table.
///
/// Name: provisional, minted 2026-10-22 (UTC) by lane/321-repeat-count.
#[cfg(any(test, feature = "system_tests"))]
pub fn current_capability_slots() -> (usize, usize) {
    current_capabilities().map_or((0, 0), |t| (t.used(), t.len()))
}

/// Hand the current thread a capability at an explicit `slot`, leaving lower slots empty.
///
/// [`grant`] fills the first free slot. Some conventions (notes/abi.md, section 4) name a fixed
/// slot whose lower neighbors' emptiness is meaningful: a std program holding slot 4 with 2 and 3
/// empty has no network. The kernel's counterpart of `ThreadControlBlock::CAP_INSERT`.
pub fn grant_at(slot: u64, capability: crate::cap::Cap) -> Result<u64, crate::cap::Error> {
    current_capabilities().map_or(Err(crate::cap::Error::NoFreeSlot), |mut t| {
        t.insert_at(slot, capability)
    })
}

/// How many pages every `PageFrame` run the current thread holds names, summed
/// ([`crate::cap::Object::mappable_pages`]). Name provisional (the surface map budget lane,
/// 2026-10-21 UTC).
///
/// `user::entry`'s spawn sizes the new space's mapping log from it. It counts the table itself, so
/// a run placed with [`grant_at`] and absent from `Spawn::grants` is still counted.
pub(crate) fn current_mappable_pages() -> u64 {
    current_capabilities().map_or(0, |t| {
        (0..t.len() as u64)
            .filter_map(|slot| t.get(slot).ok())
            .map(|c| c.object.mappable_pages())
            .sum()
    })
}

/// [`grant`], into the first free slot at or above `floor` (milestone 812): `RETYPE_OBJ`'s third
/// word, so a `std` program can make objects without filling a fixed slot it was not given.
pub(crate) fn grant_from(
    floor: u64,
    capability: crate::cap::Cap,
) -> Result<u64, crate::cap::Error> {
    current_capabilities().map_or(Err(crate::cap::Error::NoFreeSlot), |mut t| {
        t.insert_at_or_above(floor, capability)
    })
}

/// A copy of a capability the running thread holds, narrowed, about to be filed somewhere: the
/// source `slot` and the `rights` the copy keeps. [`Delegation::derive`] is the rule. Name
/// provisional.
///
/// The source is a slot, not a capability, so it is read in the same critical section that files
/// the copy, which excludes every revocation sweep. See notes/capability-lifecycle.md
/// ("Delegation reads its source in the critical section that files the copy").
#[derive(Clone, Copy)]
pub(crate) struct Delegation {
    /// The source's slot in the running thread's table.
    pub slot: u64,
    /// The rights the copy keeps: a subset of the source's.
    pub rights: crate::cap::Rights,
}

impl Delegation {
    /// **The delegation rule**: the source exists, its holder was trusted to pass it on (`GRANT`),
    /// and the copy only narrows. Applied to a table the caller holds under a lock every sweep
    /// takes.
    pub(super) fn derive(
        self,
        table: &crate::cap::CapabilityTable,
    ) -> Result<crate::cap::Cap, abi::Error> {
        let src = table.get(self.slot).map_err(|_| abi::Error::NoSuchSlot)?;
        if !src.rights.allows(crate::cap::Rights::GRANT) {
            return Err(abi::Error::NotPermitted); // holder may not pass this on
        }
        if !self.rights.is_subset_of(src.rights) {
            return Err(abi::Error::NotPermitted); // delegation may only narrow, never widen
        }
        Ok(crate::cap::Cap {
            object: src.object,
            rights: self.rights,
        })
    }
}

/// File, in the running thread's own table, a capability derived from the one in `slot`, reading
/// the source and filing the copy under one hold of that table's lock ([`Delegation`] has why).
/// `Err` is `derive`'s, `NoSuchSlot` for an empty slot, or `OutOfMemory` for a full table. Name
/// provisional.
pub(crate) fn grant_derived(
    slot: u64,
    derive: impl FnOnce(crate::cap::Cap) -> Result<crate::cap::Cap, abi::Error>,
) -> Result<u64, abi::Error> {
    let mut table = current_capabilities().ok_or(abi::Error::NoSuchSlot)?;
    let src = table.get(slot).map_err(|_| abi::Error::NoSuchSlot)?;
    let copy = derive(src)?;
    table.insert(copy).map_err(|_| abi::Error::OutOfMemory)
}
