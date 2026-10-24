/// `invoke(cap, SIGNAL, bits, _, _)` -> 0. OR `bits` into the word, waking a waiter or the
/// bound receiver if there is one. **Never blocks and never loses a bit.** A signal of zero
/// bits does nothing and wakes nobody, because it has nothing to deliver. `Gone` if the
/// notification has been destroyed. Needs `WRITE`.
pub const SIGNAL: u64 = 0;

/// `invoke(cap, WAIT, _, _, _)` -> word. If the word is non-zero, return it and clear it;
/// otherwise block until a signal arrives. **Never returns zero.** `Gone` if the notification
/// is destroyed, including while this thread waits on it. Needs `READ`.
///
/// The word travels in `x0`, where a negative value is an error, so a word whose top bit is set
/// and whose value lands on an error code reads as that error. See the BUGS section of
/// `design/roadmap/0151-notification-objects.md`.
pub const WAIT: u64 = 1;

/// `invoke(cap, POLL, _, _, _)` -> word. `WAIT` without blocking: the word, cleared, and `0` if
/// nothing was pending. Needs `READ`.
pub const POLL: u64 = 2;

/// `invoke(cap, BIND, tcb_slot, _, _)` -> 0. Bind this notification to the thread named by the
/// `ThreadControlBlock` capability in `tcb_slot`, embryo or running. Needs `WRITE` on both.
/// `NotPermitted` if either is already bound; `WrongObject` if the slot is not a thread;
/// `Gone` if either object has been destroyed. If the thread is already blocked receiving and
/// the word is non-zero, the bind delivers it at once rather than leaving it for later.
pub const BIND: u64 = 3;

/// **The tag on a receive that the bound notification ended** rather than a message. A receive
/// (`RECEIVE`, `RECEIVE_CAP`, `Irq::WAIT`) woken by the bound notification returns `(BOUND, word,
/// 0, 0, BOUND)`: this value in `x0` as §101 (notification objects) specified, the word in `x1`,
/// **and this value again in `x4`, which is the one to test.** Any sender can put `BOUND` in `x0`;
/// `x4` is written only by the kernel, and is `0` on every other receive. Ruled by calef on
/// 2026-09-26 (`notes/notification-objects.md`, option B). The fault-reply protocol
/// that §26 (the fault endpoint) reserves `w4` for must not put this value there.
pub const BOUND: u64 = 2;
