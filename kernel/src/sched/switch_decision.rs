//! **The two steps of `schedule`'s locked decision that are not the switch itself**: choosing the
//! thread to run next, and reading the switch's operands off both threads' TCB pages. Both run
//! under `IPC_TABLES` with interrupts masked, once per switch, and both are `#[inline(always)]`, so
//! their bytes are `schedule`'s own and stay in its pinned hot section. They live in a child of
//! `sched` because `sched.rs` is a split parent, which only shrinks (§282 (when a file is split,
//! the parent becomes an index)). See notes/scheduler/switch.md.
//!
//! Name: provisional (maintainer/schedule-split, 2026-10-12 UTC). The module and both functions,
//! `choose_next` and `switch_operands`, are provisional names.

use super::*;

/// **Choose the thread `schedule` runs next, and put the outgoing one back on the queue.**
///
/// Returns `None` to keep running `current`: the queue was empty and it can still run, or the pop
/// handed back `current` itself, which `heal_self_pop` repairs. Panics when nothing can run and
/// there is no idle thread. `current`'s state is read here rather than passed in, and the function
/// is `#[inline(always)]`, both for code size; see notes/scheduler/switch.md.
///
/// Context: caller holds `IPC_TABLES` with interrupts masked, and has already carried out any kill
/// or suspension order on `current`.
#[inline(always)]
pub(super) fn choose_next(sched: &mut IpcTables, current: ThreadId) -> Option<ThreadId> {
    let state = sched.threads.get(current).map(|t| t.handshake.state);

    // Only a `Running` thread is requeued: the timer can run `schedule` while a thread is
    // blocking itself.
    let runnable = state == Some(State::Running);

    let idle_tid = cpu::current().idle.load(Ordering::Relaxed);

    // A popped thread carrying a suspension order (DECISIONS §300 (the paused state, a TCB verb,
    // silent)) is parked and the pick goes on: the pop is the one place a queued thread reaches
    // a CPU.
    let popped = match cpu::current().with_runq(|q| q.pop_front()) {
        None => None,
        Some(token) => match hold_token_reading_order(token) {
            (id, false) => Some(id),
            (id, true) => suspension::park_popped_and_pop_again(sched, id),
        },
    };
    let next = match popped {
        Some(id) => id,
        None => {
            if runnable {
                // A thread yielding into an empty queue, the idle thread included, carries on.
                return None;
            }
            // Nothing to run and current cannot: the idle thread waits for the interrupt.
            if idle_tid == u64::MAX || current == idle_tid {
                // No idle thread yet, or the idle thread itself cannot run.
                match state {
                    Some(State::Finished) => {
                        panic!("the last thread exited; nothing left to run")
                    }
                    _ => panic!("nothing runnable and no idle thread"),
                }
            }
            idle_tid
        }
    };

    // The pop precedes the requeue, so `current` coming off its own queue means something queued
    // a running thread; see `heal_self_pop`.
    if next == current {
        heal_self_pop(sched, current);
        return None;
    }

    // Never requeue the idle thread, which lives outside the ready queue. A killed thread is
    // already `Finished`, so `finish_switch` reaps it.
    if runnable && current != idle_tid {
        // `preempt` leaves `on_cpu` set until `finish_switch`, so only this core may pop its own
        // queue (`crates/thread_wake_handshake`'s steal-versus-switch-out rule).
        let t = sched.threads.get_mut(current).unwrap();
        t.handshake.preempt();
        // The token a running thread holds on its own TCB. Round robin: the back.
        let Some(token) = t.own_token.take() else {
            missing_token()
        };
        cpu::current().with_runq(|q| q.push_back(token));
    }
    Some(next)
}

/// **Read the switch's operands off both threads' TCB pages while `IPC_TABLES` is held**: the
/// outgoing context slot and register file, and the incoming context, root and register file. Also
/// publishes the incoming current-CPU page, hands the thread pointer over and, on x86_64, installs
/// the port grant, each of which must happen under the lock. Inlined as [`choose_next`] is.
///
/// # Safety
///
/// The caller holds `IPC_TABLES` with interrupts masked, `current` and `next` are live, `next` has
/// been switched in and made current, and `next_tcb` is `sched.threads.pointer(next)`.
#[inline(always)]
pub(super) unsafe fn switch_operands(
    sched: &mut IpcTables,
    current: ThreadId,
    next: ThreadId,
    next_tcb: *mut Thread,
) -> (
    *mut *mut Context,
    *mut Context,
    u64,
    *mut crate::arch::fp::FpState,
    *const crate::arch::fp::FpState,
) {
    // The incoming low half; a kernel thread gets the empty reserved table, so every low address
    // faults. The current-CPU page is published here, on the core that will run the thread, so
    // writer and reader are one hardware thread (`crates/current_cpu_contract`).
    let next_root = match sched.threads.get(next).unwrap().space.as_ref() {
        Some(space) => {
            space.publish_current_cpu(cpu::id() as u64);
            space.ttbr0()
        }
        None => crate::arch::mmu::reserved_root(),
    };

    // Raw pointers, copied out before the lock drops; the TCB pages pin their contents. The page
    // pointer, not `get_mut`: the FP register file lives past the struct in the same page, and a
    // `&mut Thread`'s provenance stops at the struct.
    let prev_ptr = sched.threads.pointer(current).unwrap();

    // The thread pointer, from the outgoing thread to the incoming one (milestone 812), here
    // because nothing returns to user mode before the switch; `arch::thread_pointer` has each
    // architecture's half.
    //
    // SAFETY: live threads' TCB pages from the table, under `IPC_TABLES`; one field each.
    unsafe {
        crate::arch::thread_pointer::hand_over(
            &mut (*prev_ptr).thread_pointer,
            (*next_tcb).thread_pointer,
        );
    }

    // SAFETY: a live thread's TCB page, held under IPC_TABLES. Field projection through a raw
    // pointer, and `fp_state_of`'s contract is exactly what `pointer` returns.
    let (prev_slot, prev_fp): (*mut *mut Context, *mut crate::arch::fp::FpState) = unsafe {
        (
            &raw mut (*prev_ptr).context,
            crate::thread::fp_state_of(prev_ptr),
        )
    };

    // `next_ctx`, and on x86 the port grant beside it, out of one lookup. The grant goes in an
    // `x86_64`-only variable rather than the switch tuple, so the other ISAs pay nothing.
    #[cfg(not(target_arch = "x86_64"))]
    // SAFETY: as `prev_ptr`; `next` was popped off this core's run queue and marked `Running`
    // inside the caller's locked block, which still holds the lock.
    let (next_ctx, next_fp): (*mut Context, *const crate::arch::fp::FpState) = unsafe {
        let next_ptr = sched.threads.pointer(next).unwrap();
        (
            (*next_ptr).context,
            crate::thread::fp_state_of(next_ptr).cast_const(),
        )
    };
    #[cfg(target_arch = "x86_64")]
    // SAFETY: as the arm above, plus the port grant, which is a plain field read.
    let (next_ctx, next_fp, next_port_grant): (
        *mut Context,
        *const crate::arch::fp::FpState,
        Option<(u16, u16)>,
    ) = unsafe {
        let next_ptr = sched.threads.pointer(next).unwrap();
        (
            (*next_ptr).context,
            crate::thread::fp_state_of(next_ptr).cast_const(),
            (*next_ptr).port_range_grant,
        )
    };

    // Under the lock, not after it drops: a revocation clears grants on other cores by NMI while
    // holding `IPC_TABLES`, which closes every window only if every bitmap writer holds it too.
    // No ring-3 `in`/`out` runs before the switch: interrupts are masked throughout.
    #[cfg(target_arch = "x86_64")]
    install_port_grant(next_port_grant);

    (prev_slot, next_ctx, next_root, prev_fp, next_fp)
}
