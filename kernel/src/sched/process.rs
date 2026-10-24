//! The process object (§269 (how threads share a process) forks 1, 3 and 5): a page retyped from
//! its creator's region, holding what its threads share, the capability table, the address space,
//! the count of its members, and how it ended. `abi::process` is the contract; `notes/processes.md`
//! has the argument and the limits.
//!
//! Membership is counted, and the count keeps every pointer here sound: the page is reclaimed only
//! when no thread is a member ([`reap_region`]), and the space released only once the process has
//! ended with no member left ([`leave`], [`destroy`]). A process ends when its last member leaves,
//! so no table the revocation sweeps cannot reach through a thread is ever joined again.
//!
//! Name: provisional (milestone 812's lane, 2026-10-10 UTC).

use super::{
    Binder, IPC_TABLES, IpcTables, RendezvousId, State, Thread, ThreadId, deliver_death,
    finish_blocked_resident,
};
use crate::thread::CapabilityTableLock;
use crate::user::{BoundSpace, Holder};

/// A process's name: generational over the scheduler's process registry, what an `Object::Process`
/// capability carries. *(Provisional, as the object is.)*
pub type ProcessId = u64;

/// The most processes that can exist at once, whole machine: one per thread at most, so the thread
/// ceiling.
pub(super) const MAX_PROCESSES: usize = super::MAX_THREADS;

/// **What lives at the start of a process's page.** The capability table follows it, at
/// [`TABLE_OFFSET`], outside the struct for the reason `thread::capability_table_of` gives: a
/// member reads the table under its own lock while another core may hold `IPC_TABLES` and a `&mut`
/// to this.
pub(super) struct ProcessPage {
    /// The copy each member takes as its own `Thread::space`; `None` once released.
    space: Option<BoundSpace>,
    /// The registry's name for the space, by which it is released.
    space_name: u64,
    /// Threads whose `process` is this one and that are still in the thread table.
    members: u32,
    /// Members that have ever been started, which only grows: how a finished program proves, from
    /// the kernel's side, how many threads it ran.
    started: u32,
    /// No thread may join or start here any more, and every member has been ended.
    ended: bool,
    /// How it ended, `[event, pc, addr]` in `abi::fault`'s format: what a supervised member reports
    /// when it is ended by another member's `exit` or fault.
    ended_by: [u64; 3],
}

/// Where the shared table lives in the page.
const TABLE_OFFSET: usize =
    size_of::<ProcessPage>().next_multiple_of(align_of::<CapabilityTableLock>());
const _: () = assert!(
    TABLE_OFFSET + size_of::<CapabilityTableLock>() <= paging::PAGE_SIZE as usize,
    "a process and its capability table no longer fit in one page"
);

fn page_at(phys: u64) -> *mut ProcessPage {
    crate::arch::mmu::phys_to_virt(phys) as *mut ProcessPage
}

fn table_at(phys: u64) -> *const CapabilityTableLock {
    (crate::arch::mmu::phys_to_virt(phys) as usize + TABLE_OFFSET) as *const CapabilityTableLock
}

fn process_of(sched: &IpcTables, pid: ProcessId) -> Option<&'static mut ProcessPage> {
    let phys = *sched.processes.get(pid)?;
    // SAFETY: retyped exclusively for this process, its region pinned while the name resolves,
    // direct-mapped, and serialized by IPC_TABLES, which every caller holds.
    Some(unsafe { &mut *page_at(phys) })
}

/// **`RETYPE_OBJ` of `objtype::PROCESS`**: a process in `region`'s memory, holding the space
/// `space` names. `WrongObject` if the space already has a holder, `NoSuchSlot` if it does not
/// resolve, `OutOfMemory` if the region or the registry is full.
pub fn create_process_from(region: u64, space: u64) -> Result<ProcessId, abi::Error> {
    let mut created = None;
    crate::user::bind_user_address_space(space, |bound| {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().ok_or(abi::Error::NoSuchSlot)?;
        if sched.processes.len() >= MAX_PROCESSES {
            return Err(abi::Error::OutOfMemory);
        }
        let phys = crate::memory_region::retype_object_page(
            region,
            crate::memory_region::ObjectKind::Process,
        )
        .ok_or(abi::Error::OutOfMemory)?;
        // SAFETY: a fresh page retyped for this process alone, direct-mapped; both writes are
        // inside it by the assertion on `TABLE_OFFSET`.
        unsafe {
            page_at(phys).write(ProcessPage {
                space: Some(bound),
                space_name: space,
                members: 0,
                started: 0,
                ended: false,
                ended_by: [0; 3],
            });
            crate::thread::write_empty_capability_table(table_at(phys).cast_mut());
        }
        // Cannot fail: capacity was checked above under this same hold.
        let pid = sched
            .processes
            .insert_with(|_| phys)
            .ok_or(abi::Error::OutOfMemory)?;
        created = Some(pid);
        Ok(Holder::Process(pid))
    })?;
    created.ok_or(abi::Error::OutOfMemory)
}

/// **Join the embryo `tid` to `pid`**: the `CONFIGURE` that is given a process capability with
/// `BIND`. The thread takes the process's space and table and becomes a member, and whatever its
/// own table holds moves into the process's at the same slots, which is what lets a loader endow a
/// thread before it joins exactly as it always has. Caller holds `IPC_TABLES` and has checked that
/// `tid` is an embryo; the move is under that hold, so no revocation sweep sees it half done.
///
/// - `Gone` if the process has ended or no longer resolves: nothing joins a process that is over.
/// - `WrongObject` if a slot the embryo holds is already taken in the process's table. Nothing
///   moves on a refusal.
pub(super) fn join(
    sched: &mut IpcTables,
    tid: ThreadId,
    pid: ProcessId,
) -> Result<BoundSpace, abi::Error> {
    let phys = *sched.processes.get(pid).ok_or(abi::Error::Gone)?;
    let process = process_of(sched, pid).ok_or(abi::Error::Gone)?;
    let (false, Some(space)) = (process.ended, process.space) else {
        return Err(abi::Error::Gone);
    };
    let ptr = sched.threads.pointer(tid).ok_or(abi::Error::NoSuchSlot)?;
    // SAFETY: a live embryo's page pointer and this process's page, under IPC_TABLES. The two
    // tables are locked one at a time, never nested, since they share a rank; and slot by slot,
    // not copied whole, because a table is two kilobytes and this runs on a kernel stack.
    unsafe {
        let own = &*crate::thread::own_capability_table_of(ptr);
        let shared = &*table_at(phys);
        let slots = 0..abi::CAPABILITY_TABLE_SLOTS;
        let held = |table: &CapabilityTableLock, slot| {
            let guard = table.lock();
            guard.get(slot).is_ok()
        };
        if slots
            .clone()
            .any(|slot| held(own, slot) && held(shared, slot))
        {
            return Err(abi::Error::WrongObject);
        }
        for slot in slots {
            let carried = {
                let guard = own.lock();
                guard.get(slot).ok()
            };
            if let Some(capability) = carried {
                {
                    let mut guard = shared.lock();
                    let _ = guard.insert_at(slot, capability);
                }
                let mut guard = own.lock();
                let _ = guard.delete(slot);
            }
        }
        (*ptr).process = Some(pid);
        (*ptr).capability_table = table_at(phys);
    }
    process.members += 1;
    Ok(space)
}

/// The registry's name for `pid`'s space, while the process may still be joined: what a joining
/// thread takes its current-CPU page slot in before it takes `IPC_TABLES` to join.
pub(super) fn space_name_of(pid: ProcessId) -> Option<u64> {
    let guard = IPC_TABLES.lock();
    let p = process_of(guard.as_ref()?, pid)?;
    (!p.ended && p.space.is_some()).then_some(p.space_name)
}

/// **May `tid` start?** Not if it is a member of a process that has ended (`Gone`).
pub(super) fn refuses_start(sched: &IpcTables, t: &Thread) -> bool {
    t.process
        .is_some_and(|pid| process_of(sched, pid).is_none_or(|p| p.ended))
}

/// **A member of `pid` has just been started**: count it in [`ProcessPage::started`]. Caller holds
/// `IPC_TABLES`, and the start has passed [`refuses_start`].
pub(super) fn count_start(sched: &IpcTables, pid: ProcessId) {
    if let Some(process) = process_of(sched, pid) {
        process.started = process.started.saturating_add(1);
    }
}

/// Is `t` a thread that runs, or may yet run (`Ready`, `Running`, `Blocked`)?
fn is_running(t: &Thread) -> bool {
    matches!(
        t.handshake.state,
        State::Ready | State::Running | State::Blocked
    )
}

/// **End `pid`, and every member but `except`.** Records how it ended, so a supervised member ended
/// here reports that. A blocked member is finished in place; a ready or running one is marked
/// killed and dies at its next preemption, the §16 (object revocation) amendment's mechanism. A
/// member that is itself the caller (`except`) departs on its own. Caller holds `IPC_TABLES`.
pub(super) fn end(sched: &mut IpcTables, pid: ProcessId, except: ThreadId, how: [u64; 3]) {
    let Some(process) = process_of(sched, pid) else {
        return;
    };
    if process.ended {
        return;
    }
    process.ended = true;
    process.ended_by = how;
    // Blocked members first, one at a time: finishing one needs the whole of `sched`. A finished
    // member is no longer `Blocked`, so the search moves on.
    loop {
        let blocked = sched
            .threads
            .iter_mut()
            .find(|t| {
                t.process == Some(pid)
                    && t.id != except
                    && t.handshake.state == State::Blocked
                    && !t.handshake.on_cpu
                    && !t.being_reaped
            })
            .map(|t| t.id);
        let Some(tid) = blocked else { break };
        finish_blocked_resident(sched, tid);
        report_if_supervised(sched, tid);
    }
    for t in sched.threads.iter_mut() {
        if t.process == Some(pid) && t.id != except && is_running(t) {
            t.killed = true;
        }
    }
}

/// **A supervised member that its process's end has just turned into a corpse** reports that end to
/// its supervisor, as it would have reported its own: `[event, tid, pc, addr, 0]`, from the corpse.
/// A member nobody supervises is simply finished. Caller holds `IPC_TABLES`, and `tid` is off every
/// queue with its token at home.
pub(super) fn report_if_supervised(sched: &mut IpcTables, tid: ThreadId) {
    let Some(t) = sched.threads.get(tid) else {
        return;
    };
    let (Some(ep), Some(pid)) = (t.fault_ep, t.process) else {
        return;
    };
    let label = t.fault_label;
    let Some(how) = process_of(sched, pid).map(|p| p.ended_by) else {
        return;
    };
    let msg = [how[0], tid, how[1], how[2], 0];
    if let Some(t) = sched.threads.get_mut(tid) {
        t.fault_msg = Some(msg);
        t.mailbox = msg;
        t.handshake.state = State::Dead;
    }
    deliver_death(sched, tid, ep, msg, label);
}

/// **The killed-thread conversion's half**: a killed member that has just become `Finished` at its
/// own preemption reports its process's end if it is supervised. Caller holds `IPC_TABLES`; `tid`
/// is the current thread, so its token is the one the switch is holding.
pub(super) fn on_killed(sched: &mut IpcTables, tid: ThreadId) {
    if sched
        .threads
        .get(tid)
        .is_some_and(|t| t.process.is_some() && t.fault_ep.is_some())
    {
        report_if_supervised(sched, tid);
    }
}

/// **The calling member departs**, by `exit` (`whole`), a fault (`whole`), or `exit_thread`.
/// Decides whether the process ends with it: always for `exit` and a fault, and for `exit_thread`
/// only when no other member runs. Ending it ends every other member. Returns the supervision this
/// departure reports to: the thread's own, which a process gives its first member. Caller holds
/// `IPC_TABLES`.
pub(super) fn depart(
    sched: &mut IpcTables,
    current: ThreadId,
    whole: bool,
    how: [u64; 3],
) -> Option<(RendezvousId, u64)> {
    let t = sched.threads.get(current)?;
    let own = t.fault_ep.map(|ep| (ep, t.fault_label));
    let Some(pid) = t.process else {
        return own;
    };
    let others_run = sched
        .threads
        .iter_mut()
        .any(|o| o.process == Some(pid) && o.id != current && is_running(o));
    if whole || !others_run {
        end(sched, pid, current, how);
    }
    own
}

/// **`tid` leaves its process**, because it is being removed from the thread table (reaped, or
/// swept with its region). The last member out ends the process; the returned name is its space,
/// for the caller to take out of the registry once `IPC_TABLES` is released, when the process has
/// ended with nobody left. Idempotent: a thread leaves once.
pub(super) fn leave(sched: &mut IpcTables, tid: ThreadId) -> Option<u64> {
    let pid = sched.threads.get_mut(tid)?.process.take()?;
    let process = process_of(sched, pid)?;
    process.members = process.members.saturating_sub(1);
    if process.members == 0 {
        process.ended = true;
    }
    release(process)
}

/// The space to release, once: when the process has ended and no member is left.
fn release(process: &mut ProcessPage) -> Option<u64> {
    if process.ended && process.members == 0 {
        process.space.take().map(|_| process.space_name)
    } else {
        None
    }
}

/// Free a finished thread that no core stands on: its stack, its thread page, and its space or its
/// share of its process's. Takes the held `guard` and releases it before any `Drop`.
pub(super) fn reap_corpse(
    mut guard: crate::sync::IrqSafeGuard<'_, Option<IpcTables>>,
    prev: ThreadId,
) {
    let Some(sched) = guard.as_mut() else {
        return;
    };
    let (space, stack, page) = match sched.threads.get_mut(prev) {
        Some(t) => {
            t.being_reaped = true;
            (t.space.take(), t.stack.take(), t.thread_page.take())
        }
        None => return,
    };
    // A member of a process does not hold its space; the process gives it up with its last member.
    let space = if sched.threads.get(prev).is_some_and(|t| t.process.is_some()) {
        leave(sched, prev)
    } else {
        space.map(|bound| bound.name())
    };
    drop(guard);

    #[cfg(feature = "lock_wait")]
    let t0 = crate::arch::timer::now();
    drop(stack);
    #[cfg(feature = "lock_wait")]
    crate::lock_wait::stack_freed(crate::arch::timer::now() - t0);
    // The thread kept a copy; the registry owns the space (§249 (a running address space stays
    // nameable)). `None` means the region sweep took it first. Taken and dropped as two statements
    // so the registry's lock is released before the `Drop`, which takes the revocation, region and
    // ASID locks.
    if let Some((name, slot)) = page {
        crate::user::give_back_thread_page(name, slot); // milestone 812: the slot is free again
    }
    if let Some(name) = space {
        let space = crate::user::take_user_address_space(name);
        drop(space);
    }

    let mut guard = IPC_TABLES.lock();
    if let Some(sched) = guard.as_mut() {
        #[cfg(feature = "lock_wait")]
        let t0 = crate::arch::timer::now();
        sched.threads.remove(prev);
        #[cfg(feature = "lock_wait")]
        crate::lock_wait::reaped(crate::arch::timer::now() - t0);
    }
}

/// **Reap the members `end` finished where they were blocked** (milestone 812). A member that ends
/// itself is reaped at its own switch-out; one finished in place is never switched again, so this
/// frees it instead: it runs after the departing member's reap and after `DESTROY`. Without it a
/// `std` program whose idle workers sleep on a futex at `exit` keeps its space until its region is
/// destroyed. A supervised member is `Dead`, not `Finished`, and waits for its supervisor's `REAP`.
pub(super) fn reap_corpses(pid: ProcessId) {
    // A killed member that was mid-way into blocking when the end came is still on another core for
    // a moment, finishing its switch-out. It is waited for, a bounded spin with the lock released
    // (this runs inside a reaper, where yielding would re-enter the scheduler), and then finished
    // where it stopped.
    let mut grace = 10_000u32;
    loop {
        let mut guard = IPC_TABLES.lock();
        let Some(sched) = guard.as_mut() else {
            return;
        };
        let member = |t: &Thread| t.process == Some(pid) && !t.being_reaped;
        let stranded = |t: &Thread| member(t) && t.killed && t.handshake.state == State::Blocked;
        let corpse = sched
            .threads
            .iter_mut()
            .find(|t| member(t) && t.handshake.state == State::Finished && !t.handshake.on_cpu)
            .map(|t| t.id);
        if let Some(tid) = corpse {
            reap_corpse(guard, tid);
            continue;
        }
        let parked = sched
            .threads
            .iter_mut()
            .find(|t| stranded(t) && !t.handshake.on_cpu && !t.handshake.wake_pending)
            .map(|t| t.id);
        if let Some(tid) = parked {
            finish_blocked_resident(sched, tid);
            report_if_supervised(sched, tid);
            continue;
        }
        let switching = sched
            .threads
            .iter_mut()
            .any(|t| stranded(t) && t.handshake.on_cpu);
        drop(guard);
        if !switching || grace == 0 {
            return;
        }
        grace -= 1;
        for _ in 0..64 {
            core::hint::spin_loop();
        }
    }
}

/// **`Process::DESTROY`**: the supervisor's kill (§269 fork 5). Ends the process and every member,
/// and releases the space at once if no member remains. A caller that is itself a member ends with
/// it, by `exit`. `Gone` if the name no longer resolves.
pub fn destroy(pid: ProcessId) -> Result<(), abi::Error> {
    let current = super::current_thread_id();
    let member = {
        let guard = IPC_TABLES.lock();
        let sched = guard.as_ref().ok_or(abi::Error::Gone)?;
        process_of(sched, pid).ok_or(abi::Error::Gone)?;
        sched
            .threads
            .get(current)
            .is_some_and(|t| t.process == Some(pid))
    };
    if member {
        super::exit();
    }
    {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().ok_or(abi::Error::Gone)?;
        end(
            sched,
            pid,
            crate::cpu::NO_TID,
            [abi::fault::EVENT_EXIT, 0, 0],
        );
    }
    reap_corpses(pid);
    let released = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().ok_or(abi::Error::Gone)?;
        process_of(sched, pid).and_then(release)
    };
    if let Some(name) = released {
        drop(crate::user::take_user_address_space(name));
    }
    Ok(())
}

/// **Tear down every process whose page lies in `[base, end)`**, a region being destroyed. Each is
/// ended; an embryo member is detached (its own table and no space again, so it can neither start
/// into a page that is gone nor reach it); a running member refuses this pass, ended and killed,
/// and the owner's retry finds it gone. A process with no member left loses its name, and its
/// space falls to the registry sweep that follows ([`forget_empty`] does the forgetting, once this
/// region's own threads are gone). Caller holds `IPC_TABLES`.
pub(super) fn reap_region(sched: &mut IpcTables, base: u64, end_: u64) {
    let doomed = |phys: u64| base <= phys && phys < end_;
    // End each one still live. `end` marks it ended, so each is found once.
    loop {
        let live = sched.processes.iter().find_map(|(pid, &phys)| {
            (doomed(phys) && process_of(sched, pid).is_some_and(|p| !p.ended)).then_some(pid)
        });
        let Some(pid) = live else { break };
        end(
            sched,
            pid,
            crate::cpu::NO_TID,
            [abi::fault::EVENT_EXIT, 0, 0],
        );
    }
    // Detach each embryo member of a doomed process. `leave` clears its `process`, so each is
    // found once.
    loop {
        let processes = &sched.processes;
        let embryo = sched
            .threads
            .iter_mut()
            .find(|t| {
                t.handshake.state == State::Embryo
                    && t.process
                        .and_then(|pid| processes.get(pid))
                        .is_some_and(|&phys| doomed(phys))
            })
            .map(|t| t.id);
        let Some(tid) = embryo else { break };
        let Some(ptr) = sched.threads.pointer(tid) else {
            break;
        };
        // SAFETY: a live embryo's page pointer under IPC_TABLES; field writes only.
        unsafe {
            (*ptr).capability_table = crate::thread::own_capability_table_of(ptr);
            (*ptr).space = None;
        }
        let _ = leave(sched, tid);
    }
}

/// **Forget every process in `[base, end)` that nobody is a member of any more**: the last step of
/// a region's teardown, after its own threads have been removed (each leaving its process). One
/// whose member still runs elsewhere refuses this pass; [`reap_region`] killed that member, and the
/// owner's retry finds it gone. Caller holds `IPC_TABLES`.
pub(super) fn forget_empty(sched: &mut IpcTables, base: u64, end_: u64) -> Result<(), ()> {
    let doomed = |phys: u64| base <= phys && phys < end_;
    loop {
        let empty = sched.processes.iter().find_map(|(pid, &phys)| {
            (doomed(phys) && process_of(sched, pid).is_some_and(|p| p.members == 0)).then_some(pid)
        });
        let Some(pid) = empty else { break };
        sched.processes.remove(pid);
    }
    if sched.processes.iter().any(|(_, &phys)| doomed(phys)) {
        Err(())
    } else {
        Ok(())
    }
}

/// **Can the holder of a space still run in it?** The region sweep's question, for either kind of
/// holder: a thread as `with_binders` answers it, and a process that is `Gone` once its name is
/// dead or it has ended with no member, and `CanRun` otherwise. One hold of `IPC_TABLES`, under the
/// registry's lock (61 above 60).
pub fn with_holders<R>(f: impl FnOnce(&dyn Fn(Holder) -> Binder) -> R) -> R {
    let guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_ref() else {
        return f(&|_| Binder::CanRun);
    };
    f(&|holder| match holder {
        Holder::Thread(tid) => super::binder_of(sched, tid),
        Holder::Process(pid) => match process_of(sched, pid) {
            None => Binder::Gone,
            Some(p) if p.ended && p.members == 0 => Binder::Gone,
            Some(_) => Binder::CanRun,
        },
    })
}

/// Take `pid`'s token-free bookkeeping for a test: `(members, ended)`.
#[cfg(feature = "system_tests")]
pub fn process_state(pid: ProcessId) -> Option<(u32, bool)> {
    let guard = IPC_TABLES.lock();
    let sched = guard.as_ref()?;
    process_of(sched, pid).map(|p| (p.members, p.ended))
}

/// **How many threads `pid` has ever started**, for a test: [`ProcessPage::started`]. Readable
/// after the process has ended and every member has left, until its region is reclaimed.
///
/// Name: provisional, lane milestone/121-rg-threads (#2016), 2026-10-12 (UTC).
/// The pull request records no other name considered.
#[cfg(feature = "system_tests")]
pub fn process_threads_started(pid: ProcessId) -> Option<u32> {
    let guard = IPC_TABLES.lock();
    let sched = guard.as_ref()?;
    process_of(sched, pid).map(|p| p.started)
}

/// **Strand a member for a test**: `pid` ended and `tid` killed while `tid` stays parked, which is
/// what a member mid-way into blocking when its process ended looks like once its switch-out is
/// done. The race cannot be staged on demand; this stages its outcome for [`reap_corpses`].
#[cfg(feature = "system_tests")]
pub fn strand_member(pid: ProcessId, tid: ThreadId) {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return;
    };
    if let Some(process) = process_of(sched, pid) {
        process.ended = true;
    }
    if let Some(t) = sched.threads.get_mut(tid) {
        t.killed = true;
    }
}

/// [`reap_corpses`], for a test.
#[cfg(feature = "system_tests")]
pub fn reap_corpses_now(pid: ProcessId) {
    reap_corpses(pid);
}
