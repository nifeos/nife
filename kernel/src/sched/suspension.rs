//! `ThreadControlBlock::SUSPEND` and `RESUME` (§300 (the paused state, a TCB verb, silent)):
//! stopping a started thread so that it runs on later from where it stood.
//!
//! A suspended thread is `Blocked` on [`Wait::Resume`], on no queue, with its token at home, so
//! every teardown that finishes a blocked thread finishes a suspended one. `SUSPEND` records
//! [`suspend_requested`](crate::thread::Thread::suspend_requested), and the thread parks where it
//! next meets `schedule`: [`park_suspended_current`] leaving a core, [`park_popped_and_pop_again`]
//! about to be switched onto one. See `notes/suspend-and-resume.md`.
//!
//! Name: provisional (lane/48-tcb-suspend-resume, 2026-10-18 UTC). The module and its four
//! functions are provisional names.

use super::*;

/// **`ThreadControlBlock::SUSPEND`'s body.** Returns once `tid` is off every core, or has been
/// resumed or has ended while this waited. See the ABI constant for the contract.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/48-tcb-suspend-resume.
pub fn suspend_thread_control_block(tid: ThreadId) -> Result<(), abi::Error> {
    let me = current_thread_id();
    let poke = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().ok_or(abi::Error::NoSuchSlot)?;
        let t = sched.threads.get_mut(tid).ok_or(abi::Error::NoSuchSlot)?;
        refuse_unstarted_or_ended(t)?;
        if is_suspended(t) {
            return Ok(());
        }
        t.suspend_requested = true;
        // Running on another core: that core must reschedule to let go of it. Found by its per-CPU
        // `current`, which `schedule` writes under the lock held here, so the answer is exact.
        if tid != me && t.handshake.state == State::Running {
            crate::smp::online_cpus().find(|&c| cpu::of(c).current.load(Ordering::Relaxed) == tid)
        } else {
            None
        }
    };
    if tid == me {
        // Self-suspension: this call parks here and returns 0 when somebody resumes it.
        schedule();
        return Ok(());
    }
    if let Some(core) = poke {
        // The flag before the interrupt, so the handler's `preempt_if_needed` sees it. A lost race
        // costs one tick at most: that core's timer sets the same flag.
        cpu::of(core).need_resched.store(true, Ordering::SeqCst);
        crate::arch::irq::send_reschedule(core);
        wait_until_off_core(tid);
    }
    Ok(())
}

/// **`ThreadControlBlock::RESUME`'s body**: withdraw the order, and if the thread has already
/// parked on it, wake it onto a core.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/48-tcb-suspend-resume.
pub fn resume_thread_control_block(tid: ThreadId) -> Result<(), abi::Error> {
    let remote = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().ok_or(abi::Error::NoSuchSlot)?;
        let t = sched.threads.get_mut(tid).ok_or(abi::Error::NoSuchSlot)?;
        refuse_unstarted_or_ended(t)?;
        // Withdrawn before anything else: a thread that has not parked yet (on a run queue, inside
        // an IPC, or still on its core) now never will.
        t.suspend_requested = false;
        if t.handshake.wait_on != Some(Wait::Resume) {
            return Ok(());
        }
        // RESUME is the delivery this wait was parked for, so the gate passes it. A thread still
        // switching off its core is deferred, and that core's `finish_switch` queues it.
        t.handshake.serve();
        match t.handshake.try_wake() {
            WakeVerdict::Queue => {
                let target = pick_spawn_target();
                t.placement = target as u8;
                place_on(target, take_token(sched, tid))
            }
            WakeVerdict::Deferred | WakeVerdict::NotBlocked | WakeVerdict::Refused => None,
        }
    };
    if let Some(target) = remote {
        crate::arch::irq::send_reschedule(target);
    }
    Ok(())
}

/// **Park the running thread on its suspension order.** Called by `schedule` under `IPC_TABLES`
/// for a `current` that is `Running` with the order set, before the pick, so the pick finds it not
/// runnable and does not requeue it. It stays on its core until the successor's `finish_switch`,
/// as any thread that blocks does.
#[cold]
#[inline(never)]
pub(super) fn park_suspended_current(sched: &mut IpcTables, current: ThreadId) {
    if let Some(t) = sched.threads.get_mut(current) {
        t.handshake.park(Wait::Resume);
    }
}

/// **Park a thread the pick just popped instead of running it, and pop again** until the queue
/// gives a thread without the order or runs dry. Called by `choose_next`, whose answer this is.
/// Each popped token is already home (`hold_token`), so a parked thread is on no queue, and it is
/// `Ready` and off every core. `park` is for a thread parking itself, so the state is written here
/// directly, the same out-of-protocol write `START` makes. Out of line so the pick pays one branch.
#[cold]
#[inline(never)]
pub(super) fn park_popped_and_pop_again(
    sched: &mut IpcTables,
    mut tid: ThreadId,
) -> Option<ThreadId> {
    loop {
        if let Some(t) = sched.threads.get_mut(tid) {
            debug_assert!(t.handshake.state == State::Ready && !t.handshake.on_cpu);
            t.handshake.state = State::Blocked;
            t.handshake.wait_on = Some(Wait::Resume);
            t.handshake.ipc_served = false;
        }
        let token = cpu::current().with_runq(|q| q.pop_front())?;
        match hold_token_reading_order(token) {
            (next, false) => return Some(next),
            (next, true) => tid = next,
        }
    }
}

/// Parked on its suspension order.
fn is_suspended(t: &Thread) -> bool {
    t.handshake.state == State::Blocked && t.handshake.wait_on == Some(Wait::Resume)
}

/// The refusals both verbs share: an embryo has nothing to stop (`START` it), and a thread that has
/// exited, died, or been doomed by a `DESTROY` is beyond either verb.
fn refuse_unstarted_or_ended(t: &Thread) -> Result<(), abi::Error> {
    match t.handshake.state {
        State::Embryo => Err(abi::Error::WrongObject),
        State::Finished | State::Dead => Err(abi::Error::Gone),
        _ if t.killed => Err(abi::Error::Gone),
        State::Ready | State::Running | State::Blocked => Ok(()),
    }
}

/// **Wait for `tid` to stop running**, with `IPC_TABLES` released between looks. Its core has been
/// asked to reschedule, and `schedule` parks it before it can be switched in again, so the wait is
/// one interrupt delivery long. It also ends when the order is withdrawn (a `RESUME` from someone
/// else) or the thread ends, since there is then nothing left to wait for. It yields between looks,
/// so the core it waits on is free for anything else that is ready.
fn wait_until_off_core(tid: ThreadId) {
    loop {
        {
            let guard = IPC_TABLES.lock();
            let still = guard
                .as_ref()
                .and_then(|s| s.threads.get(tid))
                .is_some_and(|t| t.suspend_requested && t.handshake.state == State::Running);
            if !still {
                return;
            }
        }
        yield_now();
    }
}

#[cfg(test)]
mod tests {
    //! §300's two promises, on the paths a real caller takes: a suspended thread runs no further,
    //! and a resumed one runs on from where it stopped rather than from its entry. One test per
    //! hard case in `notes/suspend-and-resume.md`: running on another core, blocked in IPC,
    //! suspending itself, and a TCB with nothing to stop.

    use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    use super::super::tests::wait_for;
    use super::*;

    /// Parked on its suspension order, under the lock.
    fn parked(tid: ThreadId) -> bool {
        IPC_TABLES
            .lock()
            .as_ref()
            .and_then(|s| s.threads.get(tid))
            .is_some_and(is_suspended)
    }

    /// `Running`, on any core.
    fn running(tid: ThreadId) -> bool {
        IPC_TABLES
            .lock()
            .as_ref()
            .and_then(|s| s.threads.get(tid))
            .is_some_and(|t| t.handshake.state == State::Running)
    }

    /// `Running` on a core other than the caller's, which is the case that needs the interrupt.
    fn running_elsewhere(tid: ThreadId) -> bool {
        // One hold for both reads, so the state and the core it names are of one instant.
        let guard = IPC_TABLES.lock();
        guard
            .as_ref()
            .and_then(|s| s.threads.get(tid))
            .is_some_and(|t| t.handshake.state == State::Running)
            && crate::smp::online_cpus()
                .any(|c| c != cpu::id() && cpu::of(c).current.load(Ordering::Relaxed) == tid)
    }

    /// Let about fifty milliseconds of clock pass, yielding, so a thread that could run would.
    fn let_time_pass() {
        let until = crate::arch::timer::now() + crate::arch::timer::frequency() / 20;
        while crate::arch::timer::now() < until {
            yield_now();
        }
    }

    /// **A thread spinning on another core stops when `SUSPEND` returns and continues when
    /// `RESUME` does.** The spinner never yields or traps, so only the reschedule interrupt can take
    /// it off its core. Its counter freezing proves it stopped; the counter going on from the frozen
    /// value with its entry run once proves `RESUME` continued it rather than starting it again.
    #[test_case]
    fn a_suspended_thread_stops_and_a_resumed_one_continues_where_it_stopped() {
        static COUNT: AtomicU64 = AtomicU64::new(0);
        static ENTRIES: AtomicU64 = AtomicU64::new(0);
        static STOP: AtomicBool = AtomicBool::new(false);
        COUNT.store(0, Ordering::SeqCst);
        ENTRIES.store(0, Ordering::SeqCst);
        STOP.store(false, Ordering::SeqCst);

        let elsewhere = crate::smp::online_cpus().find(|&c| c != cpu::id());
        let spin = || {
            ENTRIES.fetch_add(1, Ordering::SeqCst);
            let mut n = 0u64;
            while !STOP.load(Ordering::Relaxed) {
                n += 1;
                COUNT.store(n, Ordering::Relaxed);
            }
        };
        let tid = match elsewhere {
            Some(core) => spawn_on(core, spin),
            None => spawn(spin),
        }
        .expect("spawn failed");
        assert!(
            wait_for(|| COUNT.load(Ordering::Relaxed) > 1000),
            "the spinner never ran"
        );
        if elsewhere.is_some() {
            assert!(
                wait_for(|| running_elsewhere(tid)),
                "the spinner was never seen running on another core, so the case is untested",
            );
        }

        assert_eq!(suspend_thread_control_block(tid), Ok(()));
        let frozen = COUNT.load(Ordering::SeqCst);
        // Off every core at once. Parked may come a pick later: a spinner its tick preempted just
        // before the order is `Ready` on a queue, and parks when that queue next hands it out.
        assert!(
            !running(tid),
            "SUSPEND returned with its target still running"
        );
        assert!(
            wait_for(|| parked(tid)),
            "a suspended thread never parked on its order"
        );
        let_time_pass();
        assert_eq!(
            COUNT.load(Ordering::SeqCst),
            frozen,
            "a suspended thread ran"
        );
        assert_eq!(
            suspend_thread_control_block(tid),
            Ok(()),
            "a second SUSPEND refused"
        );

        assert_eq!(resume_thread_control_block(tid), Ok(()));
        assert!(
            wait_for(|| COUNT.load(Ordering::Relaxed) > frozen + 1000),
            "a resumed thread did not run on",
        );
        assert_eq!(
            ENTRIES.load(Ordering::SeqCst),
            1,
            "RESUME re-ran the thread from its entry"
        );
        assert_eq!(
            resume_thread_control_block(tid),
            Ok(()),
            "RESUME of a running thread refused"
        );

        STOP.store(true, Ordering::SeqCst);
        assert!(
            wait_for(|| !is_thread_present(tid)),
            "the spinner did not finish"
        );
    }

    /// **A thread suspended while blocked in IPC finishes that IPC and stops before it returns.**
    /// The message lands while it is suspended; nothing it does after `RECEIVE` happens until
    /// `RESUME`, and then it returns exactly what was sent.
    #[test_case]
    fn a_thread_suspended_in_receive_takes_its_message_only_when_resumed() {
        static GOT: AtomicU64 = AtomicU64::new(0);
        GOT.store(0, Ordering::SeqCst);
        let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
        let ep = create_rendezvous_from(region).expect("no rendezvous from region");

        let tid = spawn(move || {
            let m = ipc_receive(ep);
            GOT.store(m[0] + 1, Ordering::SeqCst);
        })
        .expect("spawn failed");
        let receiving = |tid| {
            IPC_TABLES
                .lock()
                .as_ref()
                .and_then(|s| s.threads.get(tid))
                .is_some_and(|t| {
                    matches!(
                        t.handshake.wait_on,
                        Some(Wait::Rendezvous(_, WaitRole::Receiver))
                    )
                })
        };
        assert!(
            wait_for(|| receiving(tid)),
            "the receiver never parked in RECEIVE"
        );

        assert_eq!(suspend_thread_control_block(tid), Ok(()));
        assert!(receiving(tid), "SUSPEND took a thread out of its IPC");
        ipc_send(ep, [41, 0, 0]);
        assert!(
            wait_for(|| parked(tid)),
            "the delivered receiver never parked on its order"
        );
        let_time_pass();
        assert_eq!(
            GOT.load(Ordering::SeqCst),
            0,
            "a suspended receiver returned from RECEIVE"
        );

        assert_eq!(resume_thread_control_block(tid), Ok(()));
        assert!(
            wait_for(|| GOT.load(Ordering::SeqCst) != 0),
            "the resumed receiver never ran"
        );
        assert_eq!(
            GOT.load(Ordering::SeqCst),
            42,
            "the resumed receiver lost its message"
        );
        assert!(
            wait_for(|| !is_thread_present(tid)),
            "the receiver did not finish"
        );
        crate::sched::reclaim_region(region).expect("test region would not reclaim");
    }

    /// **A thread may suspend itself, and its `SUSPEND` returns only after a `RESUME`.**
    #[test_case]
    fn a_self_suspended_thread_returns_from_suspend_when_resumed() {
        static AFTER: AtomicBool = AtomicBool::new(false);
        AFTER.store(false, Ordering::SeqCst);
        let tid = spawn(|| {
            assert_eq!(suspend_thread_control_block(current_thread_id()), Ok(()));
            AFTER.store(true, Ordering::SeqCst);
        })
        .expect("spawn failed");
        assert!(
            wait_for(|| parked(tid)),
            "the thread never suspended itself"
        );
        let_time_pass();
        assert!(
            !AFTER.load(Ordering::SeqCst),
            "SUSPEND returned with nobody resuming"
        );
        assert_eq!(resume_thread_control_block(tid), Ok(()));
        assert!(
            wait_for(|| AFTER.load(Ordering::SeqCst)),
            "the resumed thread never returned"
        );
        assert!(
            wait_for(|| !is_thread_present(tid)),
            "the thread did not finish"
        );
    }

    /// **Nothing to stop is refused, and refused without disturbing anything**: an embryo is
    /// `WrongObject` (it has never run, and `START` is its verb), and a thread that has exited and
    /// been reaped is `NoSuchSlot`, its generational name never resolving to a newer thread.
    #[test_case]
    fn an_embryo_or_a_reaped_thread_is_refused() {
        let region = crate::memory_region::create(2).expect("a fresh 2-page region");
        let embryo = create_thread_control_block(region).expect("no TCB from the region");
        assert_eq!(
            suspend_thread_control_block(embryo),
            Err(abi::Error::WrongObject)
        );
        assert_eq!(
            resume_thread_control_block(embryo),
            Err(abi::Error::WrongObject)
        );
        crate::sched::reclaim_region(region).expect("test region would not reclaim");

        let gone = spawn(|| {}).expect("spawn failed");
        assert!(
            wait_for(|| !is_thread_present(gone)),
            "the thread did not finish"
        );
        assert_eq!(
            suspend_thread_control_block(gone),
            Err(abi::Error::NoSuchSlot)
        );
        assert_eq!(
            resume_thread_control_block(gone),
            Err(abi::Error::NoSuchSlot)
        );
    }
}
