//! **The `ThreadControlBlock` methods**: `CONFIGURE`, `CAP_INSERT`, `START` and
//! `SET_THREAD_POINTER`, the spawn machinery a loader runs once per child, and §300 (the paused
//! state, a TCB verb, silent)'s `SUSPEND` and `RESUME`. See notes/syscall-boundary.md.
//!
//! Name: provisional (milestone 812's lane, 2026-10-10 UTC).

use abi::Error;

use crate::arch::exceptions::TrapFrame;
use crate::cap::{Object, Rights};
use crate::sched;

/// Decode a `ThreadControlBlock` method on the thread `tid` and run it, out of line like every
/// method off the IPC round trip. `WRITE` on the capability is the authority for every method.
/// The spawn methods, but a self-aimed `SET_THREAD_POINTER`, refuse a thread that is not an
/// embryo; `SUSPEND` and `RESUME` refuse one that is.
#[inline(never)]
pub(super) fn thread_control_block_invoke(
    frame: &mut TrapFrame,
    rights: Rights,
    tid: crate::thread::ThreadId,
    method: u64,
    [a0, a1, a2]: [u64; 3],
) -> Result<i64, Error> {
    match method {
        abi::thread_control_block::CONFIGURE => {
            if !rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            thread_control_block_configure(tid, a0, a1, a2, frame.arg(5))
        }
        abi::thread_control_block::SET_THREAD_POINTER => {
            if !rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            thread_control_block_set_thread_pointer(tid, a0, frame)
        }
        abi::thread_control_block::CAP_INSERT => {
            if !rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            thread_control_block_cap_insert(tid, a0, a1, a2)
        }
        abi::thread_control_block::START => {
            if !rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            sched::start_thread_control_block(tid, [a0, a1, a2])?; // the child's x0, x1, x2 (19d/19e)
            Ok(0)
        }
        abi::thread_control_block::SUSPEND => {
            if !rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            sched::suspend_thread_control_block(tid)?;
            Ok(0)
        }
        abi::thread_control_block::RESUME => {
            if !rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            sched::resume_thread_control_block(tid)?;
            Ok(0)
        }
        _ => Err(Error::BadMethod),
    }
}

/// `ThreadControlBlock::CONFIGURE`: give the embryo `tid` its `entry`, `stack`, thread pointer and
/// address space. `aspace_slot` names an `AddressSpace` with `WRITE`, which is consumed (the space
/// keeps its name), or a `Process` with `BIND`, which the thread joins and which stays. Returns
/// the thread's current-CPU page.
#[inline(never)]
fn thread_control_block_configure(
    tid: crate::thread::ThreadId,
    entry: u64,
    stack: u64,
    aspace_slot: u64,
    thread_pointer: u64,
) -> Result<i64, Error> {
    let aspace = sched::current_cap(aspace_slot).map_err(|_| Error::NoSuchSlot)?;
    let aspace_name = match aspace.object {
        Object::AddressSpace(name) => name,
        Object::Process(pid) => {
            if !aspace.rights.allows(Rights::BIND) {
                return Err(Error::NotPermitted);
            }
            let page = sched::configure_thread_control_block_joining(
                tid,
                entry,
                stack,
                pid,
                thread_pointer,
            )?;
            return Ok(page as i64);
        }
        _ => return Err(Error::WrongObject),
    };
    if !aspace.rights.allows(Rights::WRITE) {
        return Err(Error::NotPermitted);
    }
    let page = sched::configure_thread_control_block_with_thread_pointer(
        tid,
        entry,
        stack,
        aspace_name,
        thread_pointer,
    )?;
    // Consumed, not retired: a copy made first keeps naming the space (§249 (a running address
    // space stays nameable)), and keeping one past `START` is retaining authority over the child
    // (§142 (what a spawner retains over a child after `START`)). A second bind through another
    // copy is refused by the registry.
    let _ = sched::delete_current_cap(aspace_slot);
    Ok(page as i64)
}

/// `ThreadControlBlock::SET_THREAD_POINTER`: set the thread pointer of the embryo `tid`, or of
/// the caller itself, to `value`.
#[inline(never)]
fn thread_control_block_set_thread_pointer(
    tid: crate::thread::ThreadId,
    value: u64,
    frame: &mut TrapFrame,
) -> Result<i64, Error> {
    sched::set_thread_pointer(tid, value, frame)?;
    Ok(0)
}

/// `ThreadControlBlock::CAP_INSERT`: endow the embryo with the capability in `src_slot`, narrowed
/// to `rights`, at `target` (0 the first free slot, n slot n - 1, so a supervisor can place a fault
/// endpoint in the reserved slot). Needs `GRANT` and only narrows, as `SEND_CAP`. Returns the slot.
#[inline(never)]
fn thread_control_block_cap_insert(
    tid: crate::thread::ThreadId,
    src_slot: u64,
    rights: u64,
    target: u64,
) -> Result<i64, Error> {
    let target = (target != 0).then(|| target - 1);
    // Read and checked inside `sched`, under the hold that files the copy (`sched::Delegation`).
    #[cfg(feature = "system_tests")]
    crate::delegation_pause::here(); // no lock held
    let child_slot = sched::thread_control_block_delegate_cap(
        tid,
        sched::Delegation {
            slot: src_slot,
            rights: Rights::from_bits(rights as u32),
        },
        target,
    )?;
    Ok(child_slot as i64)
}
