//! Waking a blocked thread. See notes/scheduler.md ("Wakes") and notes/interleaving.md.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// Move the blocked thread `tid` back to a ready queue on this core.
///
/// The decision is `thread_wake_handshake::Handshake::try_wake`, which loom searches on the host
/// (notes/interleaving.md); this is the queue push and the trace ring. A wake whose critical
/// section delivered nothing is refused (the undelivered-wake gate, recorded as `refuse:tid`), and
/// a thread still on its CPU has its wake parked for its own core's `finish_switch` to complete
/// once its context is saved (notes/intrusive-queues.md).
///
/// Context: caller holds `IPC_TABLES`. In the pinned hot section (milestone 796 (pin the hot trap
/// path's placement)).
#[cfg_attr(target_os = "none", unsafe(link_section = ".text.hot.sched.wake"))]
pub(super) fn wake(sched: &mut IpcTables, tid: ThreadId) {
    if let Some(t) = sched.threads.get_mut(tid) {
        match t.handshake.try_wake() {
            WakeVerdict::NotBlocked => {}
            WakeVerdict::Refused => {
                trace::record(trace::Event::WakeRefused, tid, 0);
            }
            WakeVerdict::Deferred => {
                // Forward progress for the hang watchdog even when queueing is deferred, so a slow
                // but live pipeline is not read as a deadlock.
                #[cfg(any(test, feature = "system_tests"))]
                crate::testing::note_progress();
                trace::record(trace::Event::WakeDeferred, tid, 0);
            }
            WakeVerdict::Queue => {
                #[cfg(any(test, feature = "system_tests"))]
                crate::testing::note_progress();
                // Blocked -> Ready: whatever unlinked it handed its token back (`hold_token`), so
                // it is on the thread; a `None` is a thread still linked into a wait queue.
                let Some(token) = t.own_token.take() else {
                    missing_token()
                };
                trace::record(trace::Event::Wake, tid, 0);
                // Onto this core's queue: a rendezvous wake stays local on purpose (§28.2), the
                // message is in registers and the cache is warm. Every caller (ipc_*, irq_notify)
                // holds IPC_TABLES, so interrupts are masked.
                cpu::current().with_runq(|q| q.push_back(token));
            }
        }
    }
}
