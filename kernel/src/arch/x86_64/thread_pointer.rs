//! **A thread's own thread pointer, the `FS` base, kept per thread by the kernel** (milestone 812
//! (`std::thread::spawn` runs real threads in one address space), §269 (how threads share a
//! process) fork 4).
//!
//! The ABI lives in `sched` (`Thread::thread_pointer`, set from `CONFIGURE` and
//! `ThreadControlBlock::SET_THREAD_POINTER`); this file is its `x86_64` register half. Ring 3
//! cannot change the `FS` base, by ruling: `CR4.FSGSBASE` stays off (§269 fork 4), so the kernel's
//! copy is always the register's value. The write is lazy: the register always holds the running
//! thread's value, so a switch writes `IA32_FS_BASE` only when the incoming value differs. See
//! notes/x86-port/cpu-state.md.

use super::exceptions::TrapFrame;

/// `IA32_FS_BASE`, the architectural MSR behind `fs:`-relative addressing in 64-bit mode.
const IA32_FS_BASE: u32 = 0xC000_0100;

/// Write the register.
///
/// `#[inline(always)]` because `schedule` reaches it on every switch whose two threads differ, and
/// `script/fastpath-footprint` requires everything on that path to sit in the pinned hot section.
#[inline(always)]
fn write(value: u64) {
    // SAFETY: `IA32_FS_BASE` exists on every 64-bit core. The value is canonical because every
    // writer of `Thread::thread_pointer` refuses one that is not a user address (`sched`), and a
    // non-canonical write is the one way this `wrmsr` could fault. It changes what `fs:`
    // addresses mean, which the kernel never uses.
    unsafe { super::write_msr(IA32_FS_BASE, value) };
}

/// **Install the incoming thread's `FS` base**, when it differs from the outgoing thread's, which
/// is what the register holds. Called by `sched::schedule` with `IPC_TABLES` held and interrupts
/// masked. Nothing is saved, because ring 3 cannot have changed it.
#[inline(always)]
pub(crate) fn hand_over(outgoing: &mut u64, incoming: u64) {
    if *outgoing != incoming {
        write(incoming);
    }
}

/// **Change the calling thread's `FS` base now**, for `SET_THREAD_POINTER` aimed at oneself. The
/// caller is the thread this core runs, so the register becomes its new value, as the hand-over's
/// invariant needs.
pub(crate) fn set_live(_frame: &mut TrapFrame, value: u64) {
    write(value);
}
