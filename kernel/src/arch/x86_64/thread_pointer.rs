//! **A thread's own thread pointer, the `FS` base, kept per thread by the kernel** (milestone 812
//! (`std::thread::spawn` runs real threads in one address space), §269 (how threads share a
//! process) fork 4).
//!
//! The ABI is the same on all three architectures and lives in `sched`: the kernel holds each
//! thread's value in `Thread::thread_pointer`, sets it from `CONFIGURE` and
//! `ThreadControlBlock::SET_THREAD_POINTER`, and installs it whenever the thread is switched in.
//! This file is the `x86_64` register half of that.
//!
//! **Ring 3 cannot change the `FS` base here, and that is a ruling, not an accident.**
//! `CR4.FSGSBASE` stays off (§269 fork 4), so `wrfsbase` faults, and there is no `arch_prctl`. The
//! kernel's copy is therefore always the register's value, and the hand-over only installs. The
//! reason it stays off is the `GS` half of the same bit: with it on, a user could choose a `GS`
//! base the kernel's entry path then trusts across the NMI exit window in `trap.s`, which has no
//! paranoid path yet.
//!
//! **The write is lazy, and needs no record of its own.** `wrmsr` to `IA32_FS_BASE` is serializing
//! and far dearer than a compare, and most switches are between threads that both hold zero (every
//! program today) or between a thread and a kernel thread. The register on a core always holds the
//! value of the thread that core is running: every switch in installs it, and so does a thread
//! setting its own, and nothing else writes it. So the outgoing thread's value *is* what the
//! register holds, and comparing it with the incoming one decides the write.

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
pub fn hand_over(outgoing: &mut u64, incoming: u64) {
    if *outgoing != incoming {
        write(incoming);
    }
}

/// **Change the calling thread's `FS` base now**, for `SET_THREAD_POINTER` aimed at oneself. The
/// caller is the thread this core runs, so the register becomes its new value, as the hand-over's
/// invariant needs.
pub fn set_live(_frame: &mut TrapFrame, value: u64) {
    write(value);
}
