//! Thread control blocks: creating one from a region, inserting capabilities before it starts,
//! starting it, and the binders that name what a thread is bound to. Configuring an embryo is
//! [`configure`]. See notes/thread-control-block.md.
//! Cut out of `sched.rs` unchanged by milestone 840 (the scheduler file is split along its seams).
//!
//! Name: provisional (milestone 840's lane, 2026-10-11 UTC; §271 (the scheduler file splits into
//! submodules of `sched`)).

use super::*;

/// Retype a TCB out of `region`: an embryo thread, page-resident in a page of the creator's own
/// untyped, in the thread table but on no queue and not runnable. Returns its `ThreadId` (what an
/// `Object::ThreadControlBlock` capability carries), or `None` if the region is out of budget or
/// the table is full.
pub fn create_thread_control_block(region: u64) -> Option<ThreadId> {
    let page =
        crate::memory_region::retype_object_page(region, crate::memory_region::ObjectKind::Thread)?;
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut()?;
    let name = sched.threads.insert_from_page(page, |tid| {
        let mut t = Thread::embryo();
        t.id = tid;
        // The region that paid, which a rendezvous reap reclaims (§32); only here is it known.
        t.thread_control_block_region = Some(region);
        t
    });
    // On a full table the page stays the region's (spend-only); nothing to recycle. The region
    // is already pinned by retype_object_page, so its destroy is refused regardless.
    name
}

/// Install a kernel-minted `capability` into the embryo `tid`'s table: its initial authority, one
/// slot at a time before it runs. Returns the slot it landed in; refuses a non-embryo.
///
/// `target` is `None` for the first free slot, or `Some(slot)` for a specific free one (the
/// reserved fault slot); an occupied or out-of-range target is `OutOfMemory`, so a reservation is
/// never overwritten. A copy of a capability the spawner holds goes through
/// [`thread_control_block_delegate_cap`] instead ([`Delegation`] has why).
pub fn thread_control_block_insert_cap(
    tid: ThreadId,
    capability: crate::cap::Cap,
    target: Option<u64>,
) -> Result<u64, abi::Error> {
    thread_control_block_insert_from(tid, target, |_, _| Ok(capability))
}

/// `ThreadControlBlock::CAP_INSERT`'s body: endow an embryo with a narrowed copy of a capability
/// the running thread holds, the source read under the `IPC_TABLES` hold that files the copy
/// ([`Delegation`]). Name provisional.
pub(crate) fn thread_control_block_delegate_cap(
    tid: ThreadId,
    delegation: Delegation,
    target: Option<u64>,
) -> Result<u64, abi::Error> {
    thread_control_block_insert_from(tid, target, |sched, current| {
        let caps = sched
            .threads
            .capabilities(current)
            .ok_or(abi::Error::NoSuchSlot)?;
        delegation.derive(&caps.lock())
    })
}

/// The body both share. `source` runs first, so its answer comes before the embryo's, the order the
/// syscall layer answered in when it read the source itself. Its table lock is released before the
/// embryo's is taken: two tables are never held at once (`sync::rank::CAPABILITY_TABLE`).
pub(super) fn thread_control_block_insert_from(
    tid: ThreadId,
    target: Option<u64>,
    source: impl FnOnce(&IpcTables, ThreadId) -> Result<crate::cap::Cap, abi::Error>,
) -> Result<u64, abi::Error> {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().ok_or(abi::Error::NoSuchSlot)?;
    let capability = source(sched, current_thread_id())?;
    let (t, caps) = sched
        .threads
        .get_mut_with_capabilities(tid)
        .ok_or(abi::Error::NoSuchSlot)?;
    if t.handshake.state != State::Embryo {
        return Err(abi::Error::WrongObject);
    }
    let landed = match target {
        None => caps.lock().insert(capability),
        Some(slot) => caps.lock().insert_at(slot, capability),
    }
    .map_err(|_| abi::Error::OutOfMemory)?;
    // The one place a port capability enters a thread, so the context switch reads one field, not
    // the table. Only the last of several ranges is kept (no consumer holds two), and one
    // delegated later by `SEND_CAP` is not cached. A `PortRange` without `WRITE` grants nothing:
    // the TSS bitmap cannot permit `in` without `out` (§121); the capability still lands.
    #[cfg(target_arch = "x86_64")]
    if let crate::cap::Object::PortRange(base, count) = capability.object
        && capability.rights.allows(crate::cap::Rights::WRITE)
    {
        t.port_range_grant = Some((base, count));
    }
    Ok(landed)
}

/// Start the embryo `tid` with `args` in its first registers: refuses with `WrongObject` a TCB that
/// is not an embryo, and with `NotPermitted` one with no bound address space or no entry, since a
/// half-built thread must never run; `Gone` for a member of an ended process. On success the
/// thread gets its kernel stack and entry context and is placed by the power of two choices.
pub fn start_thread_control_block(tid: ThreadId, args: [u64; 3]) -> Result<(), abi::Error> {
    // Built before `IPC_TABLES` is taken: it maps pages and allocates frames, which other cores'
    // IPC would wait behind (notes/job-mix/null-syscall-under-load.md). Declared before the guard,
    // so a refusal releases the lock first; a missing stack is still reported after the checks.
    let stack = crate::thread::KernelStack::new();
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().ok_or(abi::Error::NoSuchSlot)?;
    // A member of a process that has ended may not start: `Gone`.
    if sched
        .threads
        .get(tid)
        .is_some_and(|t| process::refuses_start(sched, t))
    {
        return Err(abi::Error::Gone);
    }
    let (t, capabilities) = sched
        .threads
        .get_mut_with_capabilities(tid)
        .ok_or(abi::Error::NoSuchSlot)?;

    if t.handshake.state != State::Embryo {
        return Err(abi::Error::WrongObject); // already started (or not a TCB)
    }
    // Whole, or refuse: without a space or an entry it would drop to user mode with no code.
    if t.space.is_none() || t.entry.0 == 0 {
        return Err(abi::Error::NotPermitted); // configure it first
    }

    // The spawn-slot convention (§26): a `Rendezvous` capability in the reserved fault slot makes
    // this thread supervised, fixed here for life. The slot is consumed, so the child cannot forge
    // fault messages, and its badge, the child's label (§148), reaches only the supervisor.
    let mut table = capabilities.lock();
    if let Ok(fault_cap) = table.get(abi::fault::FAULT_EP_SLOT)
        && let crate::cap::Object::Rendezvous(ep, label) = fault_cap.object
    {
        t.fault_ep = Some(ep);
        t.fault_label = label;
        let _ = table.delete(abi::fault::FAULT_EP_SLOT);
    }
    drop(table);

    t.start_args = args; // the child's x0, x1, x2 (19d/19e)
    let Some(stack) = stack else {
        return Err(abi::Error::OutOfMemory); // no kernel stack to be had
    };
    t.arm_for_start(stack);
    t.handshake.state = State::Ready;
    // Placement, decided once under the lock (§28): asking `target != cpu::id()` again after the
    // guard drops can disagree once this thread is stolen (see `place_on`).
    let target = pick_spawn_target();
    // Recorded for `abi::survey::record::PLACEMENT`, before `take_token` re-borrows `sched`.
    t.placement = target as u8;
    let member_of = t.process;
    let remote = place_on(target, take_token(sched, tid));
    if let Some(pid) = member_of {
        process::count_start(sched, pid);
    }
    drop(guard);
    if let Some(target) = remote {
        crate::arch::irq::send_reschedule(target);
    }
    Ok(())
}

/// Hand the current thread the address space `space`, and install it.
///
/// The space goes into the registry bound to this thread (§249), and the thread keeps the copy the
/// context switch reads; the reaper drops it when the thread dies. Returns the space's name, which
/// an address-space capability carries (`user::run_with_own_space`).
pub fn adopt_address_space(space: crate::user::AddressSpace) -> u64 {
    let current = current_thread_id();
    // Before `IPC_TABLES`: the registry's lock ranks above it. The thread is live and running, so
    // the region sweep cannot take the entry in the moment before the thread holds its copy.
    let bound = crate::user::register_bound_address_space(space, current);

    // Published here as well as at switch-in: a thread adopting a space while already running was
    // switched in before it had one, and `current_cpu` would read `None` until its next preemption.
    bound.publish_current_cpu(cpu::id() as u64);

    {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");
        sched
            .threads
            .get_mut(current)
            .expect("no current thread")
            .space = Some(bound);
    }

    // SAFETY: `bound` is the copy of a space the registry now owns, bound to the *current* thread.
    // The current thread is the one executing this line, so it is on a CPU and cannot be reaped,
    // and the space is dropped only once its thread can never be switched in again.
    unsafe { crate::arch::mmu::switch_user_root(bound.ttbr0()) };
    bound.name()
}

/// Whether the thread a bound address space names can still run (§249; name provisional): what the
/// region sweep asks before it takes a bound space (`user::reap_address_spaces_in_region`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Binder {
    /// The name no longer resolves: the thread was reaped. Generational, so it is never a newer
    /// thread in the same slot.
    Gone,
    /// `Dead` or `Finished`, and no core is standing on its stack: it will never be switched in
    /// again, so nothing will install its root.
    Corpse,
    /// Anything else, an embryo included (a `START` would run it), and a corpse still on a core.
    CanRun,
}

/// Answer [`Binder`] questions under one hold of `IPC_TABLES`: `f` may ask as often as it likes,
/// since the sweep asks about every bound space on every scan. A scheduler not yet built answers
/// `CanRun`, so nothing is freed on a guess.
///
/// Context: called under the address-space registry's lock (61 above 60); takes `IPC_TABLES`.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the region sweep uses `with_holders`
pub fn with_binders<R>(f: impl FnOnce(&dyn Fn(ThreadId) -> Binder) -> R) -> R {
    let guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_ref() else {
        return f(&|_| Binder::CanRun);
    };
    f(&|tid| binder_of(sched, tid))
}

/// [`with_binders`]'s answer for one thread, for `process::with_holders` too.
pub(super) fn binder_of(sched: &IpcTables, tid: ThreadId) -> Binder {
    match sched.threads.get(tid) {
        None => Binder::Gone,
        Some(t)
            if matches!(t.handshake.state, State::Dead | State::Finished)
                && !t.handshake.on_cpu =>
        {
            Binder::Corpse
        }
        Some(_) => Binder::CanRun,
    }
}
