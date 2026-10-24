//! The riscv64 register half of each thread's own thread pointer, `tp` (§269 (how threads share a
//! process) fork 4).
//!
//! `tp` is a general register, so it travels in the trap frame and the context switch moves
//! nothing. The kernel's copy matters only at the first entry to U-mode and when a thread sets its
//! own. U-mode may also write `tp` itself. See notes/thread-pointer.md.

use super::exceptions::TrapFrame;

/// Nothing to switch: the outgoing thread's `tp` is in the frame it trapped through, and the
/// incoming thread's is in its own.
#[inline(always)]
pub(crate) fn hand_over(_outgoing: &mut u64, _incoming: u64) {}

/// **Change the calling thread's `tp` now**, for `SET_THREAD_POINTER` aimed at oneself: the
/// syscall returns through `frame`, and `trap_return` restores `x4` from it for a U-mode return.
pub(crate) fn set_live(frame: &mut TrapFrame, value: u64) {
    frame.set_thread_pointer(value);
}
