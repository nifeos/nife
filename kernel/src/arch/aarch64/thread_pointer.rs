//! **A thread's own thread pointer, `TPIDR_EL0`, kept per thread by the kernel** (milestone 812
//! (`std::thread::spawn` runs real threads in one address space), §269 (how threads share a
//! process) fork 4).
//!
//! The ABI lives in `sched` (`Thread::thread_pointer`, set by `CONFIGURE` and
//! `ThreadControlBlock::SET_THREAD_POINTER`); this file is the aarch64 register half.
//!
//! EL0 can write `TPIDR_EL0` itself, so the hand-over saves the outgoing value rather than
//! assuming the kernel's copy is current. See notes/thread-pointer.md.

use super::exceptions::TrapFrame;
use super::instructions;

/// **Switch the register from the outgoing thread to the incoming one.** Called by
/// `sched::schedule` on the core about to run `incoming`, with `IPC_TABLES` held and interrupts
/// masked, so nothing can return to EL0 between the save and the install. Two instructions and a
/// store. No barrier: the only way back to EL0 is an `eret`, which is context-synchronizing.
#[inline(always)]
pub(crate) fn hand_over(outgoing: &mut u64, incoming: u64) {
    *outgoing = instructions::read_tpidr_el0();
    instructions::write_tpidr_el0(incoming);
}

/// **Change the calling thread's register now**, for `SET_THREAD_POINTER` aimed at oneself. The
/// field was already written under the lock; this makes the register agree before the `eret`.
pub(crate) fn set_live(_frame: &mut TrapFrame, value: u64) {
    instructions::write_tpidr_el0(value);
}
