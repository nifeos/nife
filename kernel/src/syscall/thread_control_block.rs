//! **The `ThreadControlBlock` methods**: `CONFIGURE`, `CAP_INSERT`, `START` and milestone 812
//! (`std::thread::spawn` runs real threads in one address space)'s `SET_THREAD_POINTER`, the
//! process-spawn machinery a loader runs once per child. Cut out of `syscall.rs` along this seam by
//! §266 (a Rust source file stays under 2,000 lines); what moved is unchanged.
//!
//! Name: provisional (milestone 812's lane, 2026-10-10 UTC).

use abi::Error;

use crate::arch::exceptions::TrapFrame;
use crate::cap::{Object, Rights};
use crate::sched;

/// **Every `ThreadControlBlock` method, out of line** (milestone 812 (`std::thread::spawn` runs
/// real threads in one address space)). Each is process-spawn machinery a loader runs once per
/// child, never a step of the IPC round trip, and milestone 812's `SET_THREAD_POINTER` arm put
/// aarch64's flat `syscall_entry` 6% over `script/fastpath-footprint`'s 5% bound while it was
/// folded into [`super::invoke`]. One call here instead costs the entry one branch. WRITE on the TCB
/// capability is the authority to shape and start it; every method but a self-aimed
/// `SET_THREAD_POINTER` refuses a thread that is not an embryo, in the scheduler.
#[inline(never)]
pub(super) fn thread_control_block_invoke(
    frame: &mut TrapFrame,
    rights: Rights,
    tid: crate::thread::ThreadId,
    method: u64,
    [a0, a1, a2]: [u64; 3],
) -> Result<i64, Error> {
    match method {
        // Body extracted (milestone 156): every `ThreadControlBlock` method is process-spawn machinery a
        // loader runs once per child, never a step of the IPC round trip, so each moves out
        // of `invoke`'s own bytes. See `memory_region_map`'s doc comment for the full reasoning.
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
        _ => Err(Error::BadMethod),
    }
}

/// `ThreadControlBlock::CONFIGURE`: bind an address space to an embryo thread and set its entry point and stack
/// (`a0` entry, `a1` stack, `a2` the address space cap slot, consumed; the space keeps its name). `#[inline(never)]` for the
/// reason `memory_region_map` gives.
#[inline(never)]
fn thread_control_block_configure(
    tid: crate::thread::ThreadId,
    entry: u64,
    stack: u64,
    aspace_slot: u64,
    thread_pointer: u64,
) -> Result<i64, Error> {
    // aspace_slot must name a WRITE AddressSpace cap, and it is consumed.
    let aspace = sched::current_cap(aspace_slot).map_err(|_| Error::NoSuchSlot)?;
    let Object::AddressSpace(aspace_name) = aspace.object else {
        return Err(Error::WrongObject);
    };
    if !aspace.rights.allows(Rights::WRITE) {
        return Err(Error::NotPermitted);
    }
    sched::configure_thread_control_block_with_thread_pointer(
        tid,
        entry,
        stack,
        aspace_name,
        thread_pointer,
    )?;
    // Consume the capability passed, which §249 (a running address space stays nameable) keeps: a
    // builder that wants to go on naming the child's space makes a copy first and says so, as §142
    // (what a spawner retains over a child after `START`) asks. The name itself is not retired, and
    // a second bind through some other copy is refused by the registry's bound mark, not by this
    // delete.
    let _ = sched::delete_current_cap(aspace_slot);
    Ok(0)
}

/// `ThreadControlBlock::SET_THREAD_POINTER` (milestone 812): an embryo's or the caller's own
/// thread pointer. Out of line for `memory_region_map`'s reason: never a step of the IPC round trip.
#[inline(never)]
fn thread_control_block_set_thread_pointer(
    tid: crate::thread::ThreadId,
    value: u64,
    frame: &mut TrapFrame,
) -> Result<i64, Error> {
    sched::set_thread_pointer(tid, value, frame)?;
    Ok(0)
}

/// `ThreadControlBlock::CAP_INSERT`: endow the embryo with a capability (`a0` the cap to give, `a1` the rights
/// to narrow it to, `a2` the target slot: 0 is first-free, n is slot n - 1, a supervisor placing
/// a fault endpoint in the reserved slot, milestone 22). GRANT-gated and narrowing-only, exactly
/// as `SEND_CAP`: you may endow a child only with authority you were trusted to pass on, and only
/// narrowed. `#[inline(never)]` for the reason `memory_region_map` gives.
#[inline(never)]
fn thread_control_block_cap_insert(
    tid: crate::thread::ThreadId,
    src_slot: u64,
    rights: u64,
    target: u64,
) -> Result<i64, Error> {
    let target = (target != 0).then(|| target - 1);
    // The source is read and checked inside `sched`, under the hold that files the copy
    // (`sched::Delegation`): reading it here first left a revocation sweep room to fall between.
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
