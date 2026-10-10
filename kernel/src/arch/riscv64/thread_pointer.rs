//! **A thread's own thread pointer, `tp`, kept per thread by the kernel** (milestone 812
//! (`std::thread::spawn` runs real threads in one address space), §269 (how threads share a
//! process) fork 4).
//!
//! The ABI is the same on all three architectures and lives in `sched`: the kernel holds each
//! thread's value in `Thread::thread_pointer`, sets it from `CONFIGURE` and
//! `ThreadControlBlock::SET_THREAD_POINTER`, and makes it the register the thread sees. This file
//! is the riscv64 register half of that, and it is the odd one out.
//!
//! **`tp` is a general register, so it already travels in the trap frame.** `trap.s` saves the
//! user's `tp` on every entry from U-mode, borrows the register for the kernel's per-hart pointer
//! (through `sscratch`), and restores the saved value on the way out. So the context switch has
//! nothing to move, and the kernel's copy matters at two moments only: the first entry to U-mode,
//! whose frame `TrapFrame::for_user_entry` builds with it, and a thread setting its own value,
//! which edits the frame it will return through.
//!
//! **U-mode can write `tp` itself**, which is the asymmetry §269 records below the ABI. A user
//! write is not part of the contract; it persists for that thread, because the frame saves it,
//! and no other thread sees it. `notes/thread-pointer.md` is the scope note.

use super::exceptions::TrapFrame;

/// Nothing to switch: the outgoing thread's `tp` is in the frame it trapped through, and the
/// incoming thread's is in its own.
#[inline(always)]
pub fn hand_over(_outgoing: &mut u64, _incoming: u64) {}

/// **Change the calling thread's `tp` now**, for `SET_THREAD_POINTER` aimed at oneself: the
/// syscall returns through `frame`, and `trap_return` restores `x4` from it for a U-mode return.
pub fn set_live(frame: &mut TrapFrame, value: u64) {
    frame.set_thread_pointer(value);
}
