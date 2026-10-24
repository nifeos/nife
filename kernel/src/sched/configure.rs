//! Configuring an embryo: binding its address space and entry, its thread pointer, and its
//! cycle-counter grant, everything fixed on a thread before `START` runs it, plus a thread changing
//! its own thread pointer later. The register halves are `arch::thread_pointer` and the scope note
//! is `notes/thread-pointer.md`.
//!
//! Name: provisional (milestone 812's lane, 2026-10-10 UTC).

use super::{IPC_TABLES, State, ThreadId, current_thread_id};

/// Configure the embryo `tid` (§249 (a running address space stays nameable)): bind the address
/// space named by `aspace_name` and set the user `entry` and stack pointer `user_sp`.
///
/// The space stays in the registry and keeps its name; the thread keeps a copy of what the context
/// switch reads, and the registry records the holder, which refuses a second one (`WrongObject`).
/// Threads share a space by joining its process instead. Refuses anything but an `Embryo` with
/// `WrongObject`, so a running thread is never reconfigured under itself; checked before a stale
/// space name answers `NoSuchSlot`, and again under both locks.
pub fn configure_thread_control_block(
    tid: ThreadId,
    entry: u64,
    user_sp: u64,
    aspace_name: u64,
) -> Result<(), abi::Error> {
    configure_thread_control_block_with_thread_pointer(tid, entry, user_sp, aspace_name, 0)
        .map(|_| ())
}

/// Grant the embryo `tid` the cycle counter (§139 (who may read the cycle counter, and by what
/// authority), option 4): once it runs it may read `PMCCNTR_EL0` (aarch64) or the `cycle` CSR
/// (riscv64) from user mode. `x86_64` lets every thread read the TSC, a stated exception to §19.
///
/// Refuses a non-embryo with `WrongObject`, which keeps the grant a field of the spawn manifest
/// rather than something a running program can ask for. One-way. No syscall method reaches it yet
/// (`abi::thread_control_block` says why none is minted). `#[inline(never)]` keeps it out of the
/// bytes `script/fastpath-footprint` bounds.
#[inline(never)]
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
#[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
pub fn grant_cycle_counter(tid: ThreadId) -> Result<(), abi::Error> {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().ok_or(abi::Error::NoSuchSlot)?;
    let t = sched.threads.get_mut(tid).ok_or(abi::Error::NoSuchSlot)?;
    if t.handshake.state != State::Embryo {
        return Err(abi::Error::WrongObject);
    }
    t.cycle_counter_grant = true;
    Ok(())
}

/// Grant the running thread the cycle counter, which breaks the embryo rule [`grant_cycle_counter`]
/// enforces. Two kernel callers, each on a kernel thread before it drops to EL0 through
/// `user::run`: the system test of the EL0 grant, and the bench boot's `ipc_rtt_el0_cycles` probe
/// under `cycle_counter_grant` (milestone 101, the L4 calibration). Neither is a program asking, so the rule's reason
/// (the grant is the spawner's choice, never the grantee's) still holds.
///
/// It writes the register as well as the field, because the caller will not pass through
/// `schedule`'s switch again before it drops to EL0; later switches reapply the field.
#[cfg(any(
    feature = "system_tests",
    all(feature = "bench", feature = "cycle_counter_grant")
))]
pub fn grant_cycle_counter_to_current() {
    {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");
        let current = current_thread_id();
        sched
            .threads
            .get_mut(current)
            .expect("no current thread")
            .cycle_counter_grant = true;
    }
    crate::arch::timer::set_cycle_counter_grant(true);
}

/// [`configure_thread_control_block`], and the thread's first thread pointer with it (§269 (how
/// threads share a process) fork 4; Linux's `CLONE_SETTLS`). Returns the address of the thread's
/// current-CPU page, or zero if the space has none.
///
/// `thread_pointer` comes from `CONFIGURE`'s sixth argument register, zero for none. Refused with
/// `BadPointer` before anything is bound if it is neither zero nor a user address. The space, the
/// entry and the pointer are set in one critical section, so a racing `START` sees all or none.
///
/// Name: provisional (milestone 812 (`std::thread::spawn` runs real threads in one address space)'s
/// lane, 2026-10-10 UTC).
pub fn configure_thread_control_block_with_thread_pointer(
    tid: ThreadId,
    entry: u64,
    user_sp: u64,
    aspace_name: u64,
    thread_pointer: u64,
) -> Result<u64, abi::Error> {
    if !is_thread_pointer(thread_pointer) {
        return Err(abi::Error::BadPointer);
    }
    {
        let guard = IPC_TABLES.lock();
        let sched = guard.as_ref().ok_or(abi::Error::NoSuchSlot)?;
        let t = sched.threads.get(tid).ok_or(abi::Error::NoSuchSlot)?;
        if t.handshake.state != State::Embryo {
            return Err(abi::Error::WrongObject); // only an unstarted TCB may be configured
        }
    }

    // The current-CPU page is attached when a bare space becomes a thread's. The closure runs
    // under the registry's lock (`ADDRESS_SPACES`, 61) and takes `IPC_TABLES` (60) beneath it, so
    // the bound mark and the thread's copy are written in one critical section.
    let mut page = 0;
    crate::user::bind_user_address_space(aspace_name, |bound| {
        if bound.has_current_cpu_page() {
            page = current_cpu_contract::PAGE_VA;
        }
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().ok_or(abi::Error::NoSuchSlot)?;
        let t = sched.threads.get_mut(tid).ok_or(abi::Error::NoSuchSlot)?;
        if t.handshake.state != State::Embryo {
            return Err(abi::Error::WrongObject);
        }
        t.space = Some(bound);
        t.entry = (entry, user_sp);
        t.thread_pointer = thread_pointer;
        Ok(crate::user::Holder::Thread(tid))
    })?;
    Ok(page)
}

/// `CONFIGURE` given a process capability with `BIND` (§269 fork 1): join the embryo `tid` to the
/// process `pid` and set its entry, stack and thread pointer, so it runs in the process's space
/// with the process's capability table. Returns the thread's current-CPU page address.
///
/// `Gone` for a process that has ended; `OutOfMemory` when the process has no thread page left;
/// `WrongObject` for a thread that is not an unconfigured embryo or that carries a slot the
/// process's table already holds (`process::join`). Nothing changes on a refusal.
///
/// Name: provisional (milestone 812's lane, 2026-10-10 UTC).
pub fn configure_thread_control_block_joining(
    tid: ThreadId,
    entry: u64,
    user_sp: u64,
    pid: super::ProcessId,
    thread_pointer: u64,
) -> Result<u64, abi::Error> {
    if !is_thread_pointer(thread_pointer) {
        return Err(abi::Error::BadPointer);
    }
    // The thread page first, because the registry's lock ranks above `IPC_TABLES`.
    let space = super::process::space_name_of(pid).ok_or(abi::Error::Gone)?;
    let (slot, page, kernel_va) =
        crate::user::take_thread_page(space).ok_or(abi::Error::OutOfMemory)?;
    let joined = (|| {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().ok_or(abi::Error::NoSuchSlot)?;
        let t = sched.threads.get(tid).ok_or(abi::Error::NoSuchSlot)?;
        if t.handshake.state != State::Embryo || t.space.is_some() {
            return Err(abi::Error::WrongObject); // only an unconfigured embryo joins
        }
        let bound = super::process::join(sched, tid, pid)?;
        let t = sched.threads.get_mut(tid).ok_or(abi::Error::NoSuchSlot)?;
        t.space = Some(bound.with_current_cpu_page(kernel_va));
        t.thread_page = Some((space, slot));
        t.entry = (entry, user_sp);
        t.thread_pointer = thread_pointer;
        Ok(page)
    })();
    if joined.is_err() {
        crate::user::give_back_thread_page(space, slot);
    }
    joined
}

/// Whether `value` may be a thread pointer: zero (none), or an address in the user half.
///
/// Not alignment or mapping: a bad pointer faults the program that chose it. The half matters
/// because `x86_64`'s `wrmsr` to `IA32_FS_BASE` raises `#GP` in the kernel on a non-canonical
/// value. The same rule on every architecture (§19 (architectural parity is a tenet)).
fn is_thread_pointer(value: u64) -> bool {
    use paging::PageFormat;
    value == 0 || crate::arch::mmu::Format::is_in_half(paging::Half::Low, value)
}

/// **The calling thread's thread pointer**, for the first entry to user mode: riscv64's
/// `TrapFrame::for_user_entry` builds the frame with it, since `tp` is a frame register there. Zero
/// for a thread that has none, and for a kernel thread calling from outside any thread table.
/// riscv64 only: the other two install the register at the switch and never ask.
#[cfg(target_arch = "riscv64")]
pub fn current_thread_pointer() -> u64 {
    let guard = IPC_TABLES.lock();
    guard
        .as_ref()
        .and_then(|sched| sched.threads.get(current_thread_id()))
        .map_or(0, |t| t.thread_pointer)
}

/// `ThreadControlBlock::SET_THREAD_POINTER`: set the thread pointer of the thread `tid` to `value`
/// (§269 fork 4; seL4's `SetTLSBase`).
///
/// An embryo takes it at its first switch in. The caller takes it now, the field and the register
/// (through `frame`, the frame it returns through) under one hold of `IPC_TABLES`, since on aarch64
/// a switch between the two writes would save the old register over the new field. Any other
/// started thread is refused with `WrongObject` (notes/thread-pointer.md).
///
/// `BadPointer` for a value [`is_thread_pointer`] refuses, before anything changes.
///
/// Name: provisional (milestone 812's lane, 2026-10-10 UTC).
pub fn set_thread_pointer(
    tid: ThreadId,
    value: u64,
    frame: &mut crate::arch::exceptions::TrapFrame,
) -> Result<(), abi::Error> {
    if !is_thread_pointer(value) {
        return Err(abi::Error::BadPointer);
    }
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().ok_or(abi::Error::NoSuchSlot)?;
    let caller = current_thread_id();
    let t = sched.threads.get_mut(tid).ok_or(abi::Error::NoSuchSlot)?;
    if t.handshake.state == State::Embryo {
        t.thread_pointer = value;
    } else if tid == caller {
        t.thread_pointer = value;
        crate::arch::thread_pointer::set_live(frame, value);
    } else {
        return Err(abi::Error::WrongObject);
    }
    Ok(())
}
