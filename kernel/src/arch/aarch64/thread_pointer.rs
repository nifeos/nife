//! **A thread's own thread pointer, `TPIDR_EL0`, kept per thread by the kernel** (milestone 812
//! (`std::thread::spawn` runs real threads in one address space), §269 (how threads share a
//! process) fork 4).
//!
//! The ABI is the same on all three architectures and lives in `sched`: the kernel holds each
//! thread's value in `Thread::thread_pointer`, sets it from `CONFIGURE` and
//! `ThreadControlBlock::SET_THREAD_POINTER`, and installs it whenever the thread is switched in.
//! This file is the aarch64 register half of that.
//!
//! **EL0 can write `TPIDR_EL0` itself, and that is the asymmetry §269 records below the ABI.** A
//! user write is not part of the contract, but the hardware allows it, so the hand-over saves the
//! outgoing value rather than assuming the kernel's copy is current: a program that writes its own
//! register keeps what it wrote for the rest of its life, and never sees a sibling's. `x86_64`
//! cannot do this at all (`CR4.FSGSBASE` stays off), so there the hand-over only installs. The
//! scope note is `notes/thread-pointer.md`.
//!
//! Before this file the kernel never touched `TPIDR_EL0`, so whatever one program wrote there was
//! read by the next program to run on the same core. Nothing used the register, which is the only
//! reason that was not a leak anybody noticed.

use aarch64_cpu::registers::TPIDR_EL0;
use tock_registers::interfaces::{Readable, Writeable};

use super::exceptions::TrapFrame;

/// **Switch the register from the outgoing thread to the incoming one.** Called by
/// `sched::schedule` on the core about to run `incoming`, with `IPC_TABLES` held and interrupts
/// masked, so nothing can return to EL0 between the save and the install. Two instructions and a
/// store. No barrier: the only way back to EL0 is an `eret`, which is context-synchronizing.
#[inline(always)]
pub fn hand_over(outgoing: &mut u64, incoming: u64) {
    *outgoing = TPIDR_EL0.get();
    TPIDR_EL0.set(incoming);
}

/// **Change the calling thread's register now**, for `SET_THREAD_POINTER` aimed at oneself. The
/// field was already written under the lock; this makes the register agree before the `eret`.
pub fn set_live(_frame: &mut TrapFrame, value: u64) {
    TPIDR_EL0.set(value);
}
