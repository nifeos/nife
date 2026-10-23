//! The syscall boundary. **Five calls <!--count:syscalls-->.**
//!
//! `exit`, `exit_thread`, `yield` and `cap_delete` are authority over the caller itself, and
//! `invoke(cap, method, a0, a1, a2)` is everything else, on a capability the caller was handed.
//! Every argument is a scalar in a register, so the kernel follows no user pointer. The ABI is
//! `crates/abi`, which the kernel and every user program share.
//!
//! See notes/syscall-boundary.md, and DECISIONS §10 (process model: capability-based,
//! microkernel) for the choice this surface carries out.

use abi::Error;

use crate::arch::exceptions::TrapFrame;
use crate::cap::{Capability, Object, Rights};
use crate::sched;

// The methods, one child module per object family, for §282 (when a file is split, the parent
// becomes an index).
mod address_space;
mod memory_region;
mod object_methods;
mod page_frame;
mod rendezvous;
mod retype;
mod thread_control_block;
use address_space::address_space_invoke;
use memory_region::memory_region_invoke;
#[cfg(target_arch = "x86_64")]
use object_methods::port_range_invoke;
use object_methods::{
    aborted_send_error, irq_ack, irq_wait, notification_invoke, process_invoke, reboot_invoke,
    rendezvous_badge, rendezvous_reap, reply_other, timer_invoke, virtio_invoke,
};
use page_frame::{page_frame_map, page_frame_revoke, page_frame_slice};
use rendezvous::{
    rendezvous_call, rendezvous_receive, rendezvous_receive_capability, rendezvous_send,
    rendezvous_send_capability, rendezvous_survey,
};
use thread_control_block::thread_control_block_invoke;

// Index exception (2026-10-12): `script/fastpath-footprint` finds the syscall entry by this
// path, `kernel7syscall8dispatch` in the mangled name, and counts it flat as `syscall_entry`.
/// Decode and run one syscall, and write its result to the first argument register.
///
/// Called from the `svc` arm of `exception_body` (`ecall` on riscv64, in `riscv_trap_body`). A
/// syscall stays on the thread's own stack, not the interrupt stack, because it can block.
///
/// `#[inline(never)]` keeps this a symbol `script/fastpath-footprint` can name and sum flat as
/// `syscall_entry`; notes/syscall-boundary.md has what an inlining flip does to that sum.
#[inline(never)]
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(
    target_os = "none",
    unsafe(link_section = ".text.hot.syscall.dispatch")
)]
pub(crate) fn dispatch(frame: &mut TrapFrame) {
    // Through the frame's accessors: each architecture puts the number and arguments in
    // different registers.
    let nr = frame.syscall_nr();
    // Read now: a result may overwrite the method register before the stack-depth sample.
    #[cfg(any(test, feature = "ipc_stack_depth"))]
    let method = frame.arg(1);

    // `exit` never comes back, so it is not part of the result-writing path below.
    if nr == abi::SYS_EXIT {
        sched::exit();
    }
    // Ends this thread only, and like `exit` never comes back.
    if nr == abi::SYS_EXIT_THREAD {
        sched::exit_thread(frame.arg(0));
    }

    let result: Result<i64, Error> = match nr {
        abi::SYS_YIELD => {
            sched::yield_now();
            Ok(0)
        }
        // An empty slot is not an error: a loader recycling slots need not track emptiness.
        abi::SYS_CAP_DELETE => {
            let _ = sched::delete_current_cap(frame.arg(0));
            Ok(0)
        }
        abi::SYS_INVOKE => invoke(
            frame,
            frame.arg(0),
            frame.arg(1),
            frame.arg(2),
            frame.arg(3),
            frame.arg(4),
        ),
        _ => Err(Error::BadSyscall),
    };

    // The trap-restore path pops the frame into the user's registers.
    frame.set_arg(
        0,
        match result {
            Ok(v) => v as u64,
            Err(e) => (e as i64) as u64,
        },
    );

    // Last, so the sample covers the whole syscall; one relaxed load for every thread the
    // instrument is not measuring.
    #[cfg(any(test, feature = "ipc_stack_depth"))]
    crate::ipc_stack_depth::after_syscall(nr, method);
}

// Index exception (2026-10-12): the decoder inlines into `dispatch` above, and moving it to a
// child would let a codegen-unit split change what `syscall_entry` measures.
/// Invoke `method` on the capability in the caller's `slot`, with arguments `a0`, `a1` and `a2`.
///
/// The lookup is the security mechanism: `slot` indexes this thread's own table in kernel memory,
/// and an empty slot is `NoSuchSlot` (there is nothing there), not a refusal. Then the object's
/// rights checks, then the method. Returns the method's result word or its `abi::Error`.
///
/// Public so kernel tests drive this path, not a copy of it. See notes/syscall-boundary.md.
pub fn invoke(
    frame: &mut TrapFrame,
    slot: u64,
    method: u64,
    a0: u64,
    a1: u64,
    a2: u64,
) -> Result<i64, Error> {
    let cap = sched::current_cap(slot).map_err(|_| Error::NoSuchSlot)?;

    match cap.object {
        Object::Rendezvous(ep, badge) => match method {
            // `#[inline(always)]` in `rendezvous`, so the round trip stays inside `invoke`.
            abi::rendezvous::SEND => rendezvous_send(cap.rights, ep, badge, [a0, a1, a2]),
            abi::rendezvous::RECEIVE => rendezvous_receive(frame, cap.rights, ep),
            abi::rendezvous::SEND_CAP => {
                rendezvous_send_capability(cap.rights, ep, badge, [a0, a1, a2])
            }
            abi::rendezvous::RECEIVE_CAP => rendezvous_receive_capability(frame, cap.rights, ep),
            abi::rendezvous::CALL => rendezvous_call(frame, cap.rights, ep, badge, [a0, a1]),

            // READ, the right to receive deaths here, which a send-only peer lacks (DECISIONS §32
            // (a supervisor may collect a corpse without being able to build one)).
            abi::rendezvous::REAP => {
                if !cap.rights.allows(Rights::READ) {
                    return Err(Error::NotPermitted);
                }
                rendezvous_reap(ep, a0)
            }
            abi::rendezvous::SURVEY => rendezvous_survey(frame, cap.rights, ep, a0, a1),
            abi::rendezvous::BADGE => rendezvous_badge(ep, cap.rights, badge, a0),
            _ => Err(Error::BadMethod),
        },

        // §12 (Call/Reply IPC: a one-shot reply capability).
        Object::Reply(tid) => match method {
            abi::reply::REPLY => {
                // Minted WRITE-only and without GRANT, so a narrowed copy cannot answer.
                if !cap.rights.allows(Rights::WRITE) {
                    return Err(Error::NotPermitted);
                }
                sched::ipc_reply(tid, [a0, a1]);
                // Consumed, so the caller cannot be answered twice.
                let _ = sched::delete_current_cap(slot);
                Ok(0)
            }
            // Every other method, `REPLY_CAPABILITY` and `BadMethod` alike, is decided out of line.
            _ => reply_other(slot, method, a0, a1, a2),
        },

        Object::AddressSpace(name) => address_space_invoke(frame, cap, name, method, [a0, a1, a2]),

        Object::ThreadControlBlock(tid) => {
            thread_control_block_invoke(frame, cap.rights, tid, method, [a0, a1, a2])
        }

        Object::Notification(id) => notification_invoke(cap.rights, id, method, a0),

        Object::Timer(id) => timer_invoke(cap.rights, id, method, a0, a1, a2),

        Object::Process(pid) => process_invoke(cap.rights, pid, method),

        // No rights bit: holding the capability is the whole authority (DECISIONS §251 (restarting
        // the machine is a kernel object the progenitor hands out)).
        Object::Reboot => reboot_invoke(method),

        Object::MemoryRegion(region) => memory_region_invoke(cap, region, method, [a0, a1, a2]),

        Object::PageFrame(phys, count) => match method {
            // `MAP` checks its rights inside `page_frame_map`, because they depend on `a1`.
            abi::page_frame::MAP => page_frame_map(slot, a0, a1, a2),
            abi::page_frame::REVOKE => {
                if !cap.rights.allows(Rights::GRANT) {
                    return Err(Error::NotPermitted);
                }
                page_frame_revoke(phys, count.get())
            }
            abi::page_frame::SLICE => page_frame_slice(slot, a0, a1),
            _ => Err(Error::BadMethod),
        },

        Object::DeviceFrame(phys) => match method {
            // Spares the invoker's own capability and mapping, unlike a frame revoke: the kernel
            // mints a `DeviceFrame` once (DECISIONS §41 (the endpoint is the broker, and a device
            // is revoked by taking it back); `revoke::revoke_device_from_others` has why).
            abi::page_frame::REVOKE => {
                if !cap.rights.allows(Rights::GRANT) {
                    return Err(Error::NotPermitted);
                }
                crate::revoke::revoke_device_from_others(phys);
                Ok(0)
            }
            _ => Err(Error::BadMethod),
        },

        Object::Virtio(id) => {
            if !cap.rights.allows(Rights::WRITE) {
                return Err(Error::NotPermitted);
            }
            virtio_invoke(id, method, a0, a1)
        }

        Object::Irq(intid) => match method {
            abi::interrupt::WAIT => {
                if !cap.rights.allows(Rights::READ) {
                    return Err(Error::NotPermitted);
                }
                irq_wait(frame, intid)
            }
            abi::interrupt::ACK => {
                if !cap.rights.allows(Rights::READ) {
                    return Err(Error::NotPermitted);
                }
                irq_ack(intid)
            }
            _ => Err(Error::BadMethod),
        },

        // `x86_64` only; the other two architectures' match is exhaustive without it.
        #[cfg(target_arch = "x86_64")]
        Object::PortRange(base, count) => port_range_invoke(cap.rights, base, count, method),
    }
}

/// Kani proofs over the kernel's own source: the run arithmetic both mapping paths guard with.
///
/// A proof over this crate runs with global assembly skipped, `asm!` unverifiable and no panic
/// handler; notes/kernel-proofs.md lists what that leaves these proofs unable to say.
#[cfg(kani)]
mod proofs;

#[cfg(test)]
mod tests;
