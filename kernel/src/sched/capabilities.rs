//! **The current thread's capabilities, and deleting capabilities across every table**: lookup,
//! `grant` and delegation, and the page-frame, device-frame and port-range deleters, which walk
//! every thread's table under `IPC_TABLES` and so stay in `sched` (§271 part 2).
//!
//! Cut out of `sched.rs` unchanged by milestone 840 (the scheduler file is split along its seams).
//!
//! Name: provisional (milestone 840's lane, 2026-10-11 UTC; §271 (the scheduler file splits into
//! submodules of `sched`)).

use super::*;

/// Delete every `PageFrame` capability naming the run `(phys, count)` from every thread's capability
/// table (§13, widened by §102). Part of revocation: once a frame (or a run of them) is being
/// revoked, no holder may keep a capability that could re-map it. The caller's own cap is deleted
/// too, which is intended: a revoke destroys all access to the page(s).
///
/// **Object equality, not overlap.** A capability matches only if it names exactly this `(phys,
/// count)` run: a narrowed derivative (rights alone differ) still matches, because `derive` never
/// changes the object, but a capability naming a different sub-range of the same physical memory
/// (see DECISIONS §102, "What this does NOT decide") does not, and is left alone. `count: 1` is the
/// pre-§102 single-page case, so every existing caller is unaffected.
///
/// **This is the right sweep for `PageFrame::REVOKE` and the wrong one for reclamation**, and the
/// difference is which question is being asked. `REVOKE` names one capability's run and takes that
/// authority back; whether it should also take an *overlapping* holder's separate capability is an
/// open question (design/decisions/0132-*.md). Reclamation asks the stronger question, "may any
/// capability still name a page this allocator is about to hand out", and only
/// [`delete_page_frame_caps_overlapping`] answers it.
pub(crate) fn delete_page_frame_caps(phys: u64, count: u64) {
    let Some(count) = core::num::NonZeroU64::new(count) else {
        return;
    };
    let target = crate::cap::Object::PageFrame(phys, count);
    delete_page_frame_caps_where(|object| *object == target);
}

/// Delete every `PageFrame` capability whose run **overlaps** `[base, base + size)`, from every
/// thread's capability table. The reclamation sweep: `memory_region::destroy` is about to return
/// these pages to an allocator that will hand them out again, so the question is not "who holds
/// exactly this object" but "may anyone still name any page of this range".
///
/// **Overlap rather than equality, because equality was a use-after-free** (found by milestone
/// 142's review). `PageFrame(base, 311)` is not equal to `PageFrame(base, 1)`, nor to any
/// `PageFrame(p, 1)` for a page inside its run, so the pre-existing per-page reclamation sweep
/// walked straight past every run capability §102 made possible: the pages came back to the
/// allocator while a holder still held a capability naming them, and one `PageFrame::MAP` later
/// re-mapped a page table or another process's stack read/write. That is exactly the hole
/// `MemoryRegion::DESTROY` exists to close, reopened by the widening.
///
/// **It also closes an older one, at no extra cost.** [`crate::revoke::revoke_region`] used to
/// delete capabilities one mapped page at a time, driven by the mapping log, so a capability whose
/// page was *never mapped* was never even looked at: nothing recorded it, so nothing found it. A
/// range sweep does not consult the log at all, so a retyped-but-unmapped frame in a destroyed
/// region loses its capability like every other.
///
/// Saturating arithmetic on the run's end: a `(phys, count)` near `u64::MAX` cannot wrap its way
/// out of the comparison, and saturating to `u64::MAX` overlaps everything, which is the safe
/// direction for a sweep whose failure mode is missing a holder.
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

/// The body both `PageFrame` sweeps share: walk every thread's table and delete every slot whose
/// object satisfies `matches`. The caller's own capability goes too, which is intended in both
/// cases: a revoke destroys all access to the page(s), including the revoker's.
///
/// **[`Thread::outgoing_cap`] goes too**, and until 2026-09-21 it did not. A capability handed to a
/// rendezvous nobody is receiving on yet is in no capability table at all: `ipc_send_cap` parks it
/// in the hand-off slot and blocks the sender, and the next `RECEIVE_CAP` files it in the receiver's
/// own table. So a sweep that reads tables alone left a live capability naming a revoked run in the
/// one place it could not see, and `MemoryRegion::DESTROY` then returned those pages to an allocator
/// while that capability was still on its way to somebody. That is the use-after-free DECISIONS
/// §13 (capability revocation and untyped reclamation) exists to prevent
/// through the slot [`delete_reply_caps_naming`] already sweeps for `Reply`, whose doc comment states
/// the rule this had not been applied to: a live capability in a hand-off slot is the same forgery
/// one step earlier. Found by risk 7's adversarial pass;
/// `kernel::user::revocation_in_flight_tests` is the falsification.
///
/// Dropping the parked capability rather than failing the send is the behaviour `ipc_send_cap`
/// already documents for the other way a hand-off can come up empty (a receiver whose table is
/// full): the data word still arrives and the receiver sees `NO_CAP`. No new error reaches
/// userspace, so this is a fix inside the established model rather than a syscall-surface change.
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

/// Delete every `DeviceFrame` capability naming `phys` from every capability table **except the calling
/// thread's** (milestone 23, DECISIONS §41). The caller keeps its own, which is the difference
/// between reclaiming a page and taking a device back to hand on; [`crate::revoke::
/// revoke_device_from_others`] has the reasoning.
///
/// **A `DeviceFrame` parked in a hand-off slot goes too** (2026-09-21), for
/// [`delete_page_frame_caps_where`]'s reason, which carries the whole finding: a capability in
/// flight to a receiver is in no capability table, so a sweep that reads tables alone leaves one
/// alive. Spared on the keeper, exactly as its table is.
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

/// **Take a port range back from every other thread** (milestone 299): delete every
/// `PortRange(base, count)` capability from every table but the caller's, and forget the cached
/// grant on each affected thread so the next context switch to it installs no bitmap and it faults
/// on its next `in`/`out`. The invoker keeps its own, the same take-back asymmetry
/// [`delete_device_frame_caps_from_others`] has and for the same reason (the kernel mints a port
/// capability once, at boot). This is `PortRange::REVOKE`'s body. `x86_64` only, like the
/// `PortRange` object it deletes.
///
/// # BUGS
///
/// **Object equality, not overlap**, as [`delete_page_frame_caps`] does. A capability naming a wider
/// or narrower range over the same ports is left alone, and so is its cached grant. No syscall can
/// reach that: `PortRange::REVOKE` takes `(base, count)` from the invoker's own capability. A
/// change that let a caller name a sub-range would reopen it. Found by the eighth outsider pass
/// (`notes/confinement-outsider-pass-8.md`, near miss 1).
#[cfg(target_arch = "x86_64")]
pub fn delete_port_range_caps_from_others(base: u16, count: u16) {
    delete_port_range_caps_impl(base, count, Some(current_thread_id()));
}

/// **Take a port range back from everyone**, the invoker included. The whole-machine revoke the
/// kernel's own tests use to prove a holder faults after its capability is gone (mirroring
/// [`crate::revoke::revoke_page_frame`]'s test-only whole-machine sweep); no syscall reaches it,
/// because a live driver replacement wants the sparing variant above. `x86_64` only.
#[cfg(target_arch = "x86_64")]
#[cfg_attr(
    not(any(test, feature = "system_tests", initial_ramdisk)),
    allow(dead_code)
)]
pub fn delete_port_range_caps(base: u16, count: u16) {
    delete_port_range_caps_impl(base, count, None);
}

/// The body of both: walk every thread, delete the matching capability, and clear the cached grant.
/// `keeper` is spared (the take-back's invoker) or `None` (the whole-machine sweep). `x86_64` only.
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
            // A `PortRange` parked in a hand-off slot goes too (2026-09-21), for
            // `delete_page_frame_caps_where`'s reason: a capability in flight to a receiver is in
            // no capability table, so a sweep that reads tables alone leaves one alive.
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
        // Reach the TSS of **every** core that might already hold the revoked bitmap, before
        // releasing the lock (milestone 315). Clearing the cached grant above is not enough on a
        // multi-core machine: a holder running on another core keeps that core's bitmap until its
        // next context switch, so its `in`/`out` kept succeeding for up to a tick after this
        // returned. Milestone 313's audit accepted that window on the reasoning that it could not
        // reopen; at two cores it was red in 7 of 12 runs of the test written to see it.
        //
        // **Inside the lock, and that is the whole of why the broadcast is safe.** The far end
        // writes a core's TSS from an NMI handler, which lands at an arbitrary instruction
        // boundary; holding `IPC_TABLES` across the send is what guarantees no other core is
        // inside `install_port_grant` at that instant, and equally that none can read a grant this
        // sweep has already cleared and install it behind the broadcast's back. `schedule`'s
        // install was moved under the lock in the same change, and `segments::set_port_range_grant_on`
        // states the resulting rule at the writer.
        //
        // The NMI is forced rather than chosen: it is the only message an x86 core takes while it
        // spins for this very lock with interrupts masked (notes/x86-tlb-shootdown.md). A no-op on
        // every architecture with no TSS.
        //
        // The invoker's own core is spared when there is an invoker (`keeper.is_some()`): its
        // installed bitmap is the invoker's own grant, which this sweep deliberately left in the
        // invoker's table (2026-09-24 security audit; `segments::revoke_port_grant_everywhere`).
        #[cfg(target_arch = "x86_64")]
        crate::arch::segments::revoke_port_grant_everywhere(base, count, keeper.is_some());
    }
}

/// Remove a capability from the **current thread's** table. Used to consume a one-shot Reply
/// capability the instant it is invoked (§12), which is what makes a second reply impossible.
///
/// **On `x86_64`, deleting the `PortRange` capability behind the thread's cached port grant also
/// drops the grant** (the audit in milestone 313 (the security audit that was due since August),
/// 2026-09-17). A port range is enforced by
/// `Thread::port_range_grant` and the TSS bitmap the context switch installs from it, not by the
/// capability table, so until this was added a thread that `SYS_CAP_DELETE`d its own port capability
/// kept `in`/`out` access to those ports for the rest of its life: the table said the authority was
/// gone and the switch kept installing it. That is §12's "a consumed capability cannot be used again"
/// failing for the one object whose enforcement lives outside the table, and it was live rather than
/// theoretical: `system_initializer` deletes its console device capability (`cap_delete(g.uart_dev)`)
/// on every boot, which on x86 is exactly this object. The cache is cleared here and this core's TSS
/// is reset at once, so the ports fault on the very next access rather than after a switch.
///
/// **The delete itself takes only the thread's own table lock** (2026-10-04 UTC), as
/// [`current_cap`] does, so the one-shot Reply that every `REPLY` consumes no longer costs a second
/// acquisition of `IPC_TABLES`. Only a deleted `PortRange` then takes `IPC_TABLES`, to clear the
/// cached grant and this core's bitmap under it (milestone 315's reason, below). Between the two
/// steps no user code runs: the thread whose grant it is, is the one executing this syscall, and a
/// core's bitmap only ever permits a range for the thread running on it. A concurrent
/// `PortRange::REVOKE` that clears the grant first leaves this step nothing to do.
///
/// # BUGS
///
/// **Deleting one of two copies of the same `PortRange` disturbs the other.** The grant is one
/// `Option` on the thread rather than a record per capability, so a thread holding the range in two
/// slots loses `in`/`out` the moment it deletes either, while the surviving slot still names the
/// range. §12's "dropping one capability does not disturb the others" (`notes/confinement-claims.md`
/// row 5) is therefore false for this one object on this one architecture. It fails safe (authority
/// is removed, never granted) and no real consumer holds a range twice, so it is recorded rather
/// than fixed; a count or a table scan on delete would close it. Found by milestone 633 (an outside
/// agent attacks the confinement claim)'s second pass.
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
            // Under the lock, with `delete_port_range_caps_impl`'s broadcast and for its reason
            // (milestone 315): every writer of a TSS port bitmap holds `IPC_TABLES`, so a
            // revocation NMI cannot land inside one. No broadcast is owed here. The caller is the
            // thread whose grant is installed, a core's bitmap only ever permits a range for the
            // thread currently running on it, and that thread is running here, so this core is
            // the only core to tell.
            crate::arch::segments::revoke_installed_port_grant(base, count);
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = deleted; // no capability is enforced outside the table on this architecture
    Ok(())
}

/// Look up a capability in the **current thread's** table.
///
/// The lookup that is the security mechanism. `slot` came from userspace, in a register, and it
/// indexes an array that lives in kernel memory and that userspace has never seen. An empty slot
/// is `NoSuchSlot`, which is not "permission denied": **there is nothing there.**
///
/// **`#[inline(never)]`, because `script/fastpath-footprint` counts it once as a root and
/// `syscall::dispatch` flat** (milestone 758 (the IPC fast paths shrink back inside their band),
/// provisional). Left to LLVM, outlining the lock-order panic made this body small enough to inline
/// into one of `invoke`'s call sites, which put 804 bytes on aarch64's `syscall_entry` for a lookup
/// the closure already counts. Every capability syscall calls it, so one copy is also the cheaper
/// copy to keep warm. Milestone 368 (`script/fastpath-footprint`'s entry set is flat) holds the
/// other instances of this shape.
#[inline(never)]
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(
    target_os = "none",
    unsafe(link_section = ".text.hot.sched.current_cap")
)]
pub fn current_cap(slot: u64) -> Result<crate::cap::Cap, crate::cap::Error> {
    // **No `IPC_TABLES` here** (2026-10-04 UTC): the running thread's own table, under its own lock.
    // This was the global lock on every capability syscall, and on radon at four busy cores 41% of
    // these lookups found it held (notes/job-mix/null-syscall-under-load.md).
    //
    // The lock-wait instrument marks this call and its one acquisition, as it did when the lock was
    // `IPC_TABLES`, so `site=current_cap` means the same thing before and after. The closure takes
    // no other lock, so marking the whole call marks exactly that acquisition. See
    // `crate::lock_wait`; absent from every build without `lock_wait`.
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

/// **`(occupied, capacity)` of the running thread's capability table**, read by the test runner's
/// repeat mode (`testing::runner`). A test that leaves one slot filled per run is invisible once,
/// and at a few dozen repeats [`grant`] refuses with `NoFreeSlot` in an iteration that did nothing
/// wrong, so the runner prints this every iteration and refuses before the table is full.
///
/// Name: provisional, minted 2026-10-22 (UTC) by lane/321-repeat-count.
#[cfg(any(test, feature = "system_tests"))]
pub fn current_capability_slots() -> (usize, usize) {
    current_capabilities().map_or((0, 0), |t| (t.used(), t.len()))
}

/// Hand the current thread a capability **at an explicit slot**, leaving lower slots empty.
///
/// [`grant`] fills the first free slot, which is what an ordinary `Spawn` literal wants: slot 0 is
/// `grants[0]` and reading the literal tells you the whole authority. But some out-of-band
/// conventions (notes/abi.md §4) name a *fixed* slot that a program may hold without holding the
/// ones below it: a std program granted a directory capability but not the network holds slot 4
/// with 2 and 3 empty, and the emptiness is load-bearing (it is how `std::net` knows it has no
/// network). This is the same explicit-target move `ThreadControlBlock::CAP_INSERT` already offers a userspace
/// loader, available to the kernel's own service wiring.
pub fn grant_at(slot: u64, capability: crate::cap::Cap) -> Result<u64, crate::cap::Error> {
    current_capabilities().map_or(Err(crate::cap::Error::NoFreeSlot), |mut t| {
        t.insert_at(slot, capability)
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

/// **A copy of a capability the running thread holds, narrowed, about to be filed somewhere.** The
/// source slot and the rights the copy keeps; [`Delegation::derive`] is the rule. Name provisional.
///
/// # Why the source is a slot and not a capability
///
/// A revocation sweep (`delete_page_frame_caps_where`, `delete_device_frame_caps_from_others`,
/// `x86_64`'s `delete_port_range_caps_impl`) holds `IPC_TABLES` for its whole walk and takes each
/// table's lock beneath it. A delegation used to read its source with [`current_cap`], let go, and
/// file the copy in a second critical section, so a sweep could run entirely between the two: it
/// deleted the source and the copy was filed after it had passed. A `PageFrame` has no generation
/// to make that copy inert, so under `PageFrame::REVOKE` it was authority the revoker had taken
/// back, and under `MemoryRegion::DESTROY` it named pages the allocator was about to reuse (§13's
/// use-after-free). Milestone 761 (capability lookup off the global lock) recorded the gap; `system_tests::user::revocation_window_tests`
/// drove a sweep into it on `SEND_CAP`, `CAP_INSERT` and `SLICE` and all three filed the copy.
///
/// **The invariant now is that a copy is derived from a source read in the same critical section
/// that files the copy, and that critical section excludes every sweep.** A delegation into
/// another thread holds `IPC_TABLES` across both ([`ipc_delegate_cap`],
/// [`thread_control_block_delegate_cap`]), which every sweep holds for its whole walk. A derivation
/// into the running thread's own table ([`grant_derived`]) holds that table's lock across both,
/// which every sweep takes to delete from it. Either way the sweep is wholly before (the source is
/// gone and the delegation answers `NoSuchSlot`, as if it had started after the revoke) or wholly
/// after (the copy is in a table the sweep then walks). Passing the slot rather than a capability
/// is what makes the old shape hard to write again: these functions have no parameter a stale copy
/// could arrive through.
///
/// **Objects with generational names do not need this, and the derivations of them that still
/// read-then-grant are safe for that reason**: `Rendezvous::BADGE` mints a badged copy of an
/// endpoint, and `MemoryRegion::SPLIT` a child of a region, and a copy minted after its object died
/// names a dead generation and fails on use (`crates/slots`, §16 (object revocation)).
#[derive(Clone, Copy)]
pub(crate) struct Delegation {
    /// The source's slot in the running thread's table.
    pub slot: u64,
    /// The rights the copy keeps: a subset of the source's.
    pub rights: crate::cap::Rights,
}

impl Delegation {
    /// **The delegation rule**: the source exists, its holder was trusted to pass it on (`GRANT`),
    /// and the copy only narrows. Applied to a table the caller holds under a lock every sweep takes.
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

/// **File, in the running thread's own table, a capability derived from one it holds**, reading the
/// source and filing the result under one hold of that table's lock ([`Delegation`] has why).
/// `derive` decides what the copy is from the source as it stands at that instant. `Err` is
/// `derive`'s answer, `NoSuchSlot` for an empty slot, or `OutOfMemory` for a full table. Name
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
