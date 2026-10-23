//! How a thread leaves the CPU: yielding, exiting, faulting, and the death message a supervisor
//! receives (§26 (the fault endpoint)).
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// Give up the CPU voluntarily.
pub fn yield_now() {
    schedule();
}

/// The current thread exited cleanly (`SYS_EXIT`). Never returns.
pub fn exit() -> ! {
    depart(abi::fault::EVENT_EXIT, 0, 0, true)
}

/// `SYS_EXIT_THREAD`: end the calling thread, and its process only if no other
/// member runs. `word`, if not zero, is cleared and its futex waiters woken first: see
/// `futex::clear_and_wake`. Never returns.
pub fn exit_thread(word: u64) -> ! {
    futex::clear_and_wake(word);
    depart(abi::fault::EVENT_EXIT, 0, 0, false)
}

/// The current thread faulted (a bad access, an illegal instruction) and is being killed. Never
/// returns. `pc` is the faulting instruction and `addr` the faulting address (0 if the fault class
/// carries none). Called on the faulting thread's own kernel stack, as `exit` is.
pub fn fault(pc: u64, addr: u64) -> ! {
    depart(abi::fault::EVENT_FAULT, pc, addr, true)
}

/// A thread's last act: report its death, then leave the CPU forever (§26).
///
/// `event`, `pc` and `addr` are the death message's words; `whole` ends the thread's process too
/// (`exit` and a fault) rather than only the thread (`exit_thread`). Unsupervised, the thread is
/// marked `Finished` and the next thread's `finish_switch` reaps it. Supervised (`fault_ep` set at
/// `START`), the five-word message is kept on the corpse, delivered to the supervision rendezvous
/// by [`deliver_death`], and the thread is marked `Dead`, kept intact until its supervisor reaps
/// it. Either way it still runs on its own kernel stack, so it only marks state and switches away.
///
/// Context: takes `IPC_TABLES`, and releases it before the final `schedule()`.
pub(super) fn depart(event: u64, pc: u64, addr: u64, whole: bool) -> ! {
    {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("depart before sched::init");
        let current = current_thread_id();

        // A member of a process ends it on `exit` or a fault, and on `exit_thread` when it was
        // the last to run; its own supervision is what it reports.
        let (fault_ep, label) = process::depart(sched, current, whole, [event, pc, addr])
            .map_or((None, 0), |(ep, label)| (Some(ep), label));

        // Free every caller whose `CALL` this thread collected and never answered, so it returns
        // `Error::Gone` instead of blocking forever (notes/blocked-thread-teardown.md).
        strand_callers_of(sched, current);

        match fault_ep {
            None => {
                if let Some(t) = sched.threads.get_mut(current) {
                    t.handshake.state = State::Finished;
                }
            }
            Some(ep) => {
                let msg = [event, current, pc, addr, 0];
                if let Some(t) = sched.threads.get_mut(current) {
                    // Kept for postmortem, and staged as the mailbox a parked delivery hands over.
                    t.fault_msg = Some(msg);
                    t.mailbox = msg;
                    t.handshake.state = State::Dead;
                }
                deliver_death(sched, current, ep, msg, label);
            }
        }
        // Not requeued and not removed: the switch below leaves this stack, and only then may the
        // next thread reap a `Finished` one.
    }

    schedule();
    unreachable!("a departed thread was scheduled again");
}

/// Deliver a corpse's five-word death message `msg` to its supervision rendezvous `ep`.
///
/// The ordinary synchronous send: a supervisor blocked in `RECEIVE` is handed the message and
/// woken; otherwise the corpse joins the sender queue with the message in its mailbox, so it waits
/// there, and `ipc_receive` leaves a `Dead` sender dead after taking it. A gone or interrupt-bound
/// rendezvous drops the message. `label` is the badge on the child's supervision capability (§148
/// (a supervisor restarts by asking)), handed over by [`hand_over_label`] or collected later in
/// [`collected_without_serving`].
///
/// Context: caller holds `IPC_TABLES`, with the corpse already `Dead` and `msg` in its mailbox.
pub(super) fn deliver_death(
    sched: &mut IpcTables,
    corpse: ThreadId,
    ep: RendezvousId,
    msg: [u64; 5],
    label: u64,
) {
    let Some(rendezvous) = rendezvous_of(sched, ep) else {
        return;
    };
    // The corpse's token: if it joins the sender queue it stays there until the supervisor
    // drains it; otherwise it comes home to the TCB, where the reap drops it.
    match rendezvous.send(take_token(sched, corpse)) {
        inter_process_communication::Send::Rendezvous(receiver, me) => {
            hold_token(me);
            let receiver = hold_token(receiver);
            let r = sched.threads.get_mut(receiver).unwrap();
            r.mailbox = msg;
            hand_over_label(r, label);
            r.handshake.serve(); // delivered: this wake passes the boot-8 gate
            trace::record(trace::Event::Served, receiver, 8);
            wake(sched, receiver);
        }
        inter_process_communication::Send::Blocked => {
            // Recorded so a dump shows where the death message waits.
            if let Some(t) = sched.threads.get_mut(corpse) {
                t.handshake.wait_on = Some(Wait::Rendezvous(ep, WaitRole::Sender));
            }
        }
        // Interrupt-bound (§101 (notification objects) ruling B): dropped, because `EVENT_FAULT` is
        // 1 and a driver would read it as its interrupt. Needs a `Rendezvous` capability no program
        // is granted.
        inter_process_communication::Send::Refused(me) => {
            hold_token(me);
        }
    }
}
