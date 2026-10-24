//! The tests for milestone 106 (a wait that ends on either the interrupt or the deadline), which
//! builds DECISIONS §147 (a timer a userspace service cannot hold) option 1, on every architecture.
//!
//! Each proves the kernel half the crate's Kani harnesses cannot: that the real tick reaches the
//! walk, that the walk's signal wakes a real waiter (and a bound receiver parked in `RECEIVE`)
//! through §101 (notification objects)'s path, and that the syscall layer checks the right rights
//! on the right objects. The arithmetic (the cache never passes an armed deadline, a walk fires
//! exactly the due timers, a replaced or cancelled deadline never fires) is proved in
//! `crates/inter_process_communication/src/timer.rs`. See notes/timer.md.

//! # Timing
//!
//! Timing assertions are one-sided where they can be. "Not before the deadline" is exact, because
//! the walk compares the same counter the test reads. "Soon after" is bounded loosely (a second),
//! because a loaded CI runner under TCG can lose ticks, and a test that flaked on host load would
//! prove nothing about the kernel.

use core::sync::atomic::{AtomicU64, Ordering};

use abi::Error;

use super::wait_for;
use crate::arch::exceptions::TrapFrame;
use crate::arch::timer::{frequency, now};
use crate::cap::{Object, Rights};
use crate::sched;
use crate::thread::{Wait, WaitRole};

/// A region for a notification, two timers and a rendezvous.
fn region() -> u64 {
    crate::memory_region::create(6).expect("no region for a timer")
}

/// Give a test's region back, for `notification_tests::reclaim`'s reason: the suite runs close to
/// its frame ceiling.
fn reclaim(r: u64) {
    assert!(
        wait_for(|| sched::reclaim_region(r).is_ok()),
        "a test region could not be reclaimed: something in it is still live"
    );
}

/// `ms` milliseconds of counter ticks.
fn ms(n: u64) -> u64 {
    frequency() * n / 1000
}

/// Invoke through the real syscall layer, from this kernel thread's own capability table.
fn invoke(slot: u64, method: u64, a0: u64, a1: u64, a2: u64) -> Result<i64, Error> {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    crate::syscall::invoke(&mut frame, slot, method, a0, a1, a2)
}

/// Is `tid` blocked the way `want` says?
fn parked(tid: crate::thread::ThreadId, want: impl Fn(Option<Wait>) -> bool) -> bool {
    sched::thread_death_disposition(tid)
        .is_some_and(|d| d.state == crate::thread::State::Blocked && want(d.wait_on))
}

/// **A wait ends on a signal before the deadline, and the timer is still armed afterwards.**
/// A waiter blocks on a notification a timer will signal in two seconds; another thread signals it
/// first. The waiter must wake with the other signal's bits and not the timer's, well before the
/// deadline, and `CANCEL` must then find the deadline still pending, which is the proof the timer
/// had not fired and been missed.
#[test_case]
fn a_wait_ends_on_a_signal_before_the_deadline() {
    static WORD: AtomicU64 = AtomicU64::new(0);
    static WOKE_AT: AtomicU64 = AtomicU64::new(0);

    let r = region();
    let n = sched::create_notification_from(r).expect("notification");
    let t = sched::create_timer_from(r).expect("timer");
    let deadline = now() + ms(2000);
    assert_eq!(sched::timer_arm(t, deadline, n, 0b10), Ok(()));

    let waiter = sched::spawn(move || {
        let w = sched::notification_wait(n).expect("wait");
        WOKE_AT.store(now(), Ordering::SeqCst);
        WORD.store(w, Ordering::SeqCst);
    })
    .expect("spawn the waiter");
    assert!(
        wait_for(|| parked(
            waiter,
            |w| matches!(w, Some(Wait::Notification(x)) if x == n)
        )),
        "the waiter never parked in WAIT"
    );
    assert_eq!(sched::notification_signal(n, 0b01), Ok(()));
    assert!(
        wait_for(|| WORD.load(Ordering::SeqCst) != 0),
        "a signal did not end the wait"
    );
    assert_eq!(
        WORD.load(Ordering::SeqCst),
        0b01,
        "the wait ended with the timer's bits: it fired early, or both arrived"
    );
    assert!(WOKE_AT.load(Ordering::SeqCst) < deadline);
    assert_eq!(
        sched::timer_cancel(t),
        Ok(true),
        "the deadline should still have been pending"
    );
    reclaim(r);
}

/// **A wait ends at the deadline with no signal, never before it**, both ways a thread can wait:
/// in `WAIT` on the notification, and blocked in `RECEIVE` on an endpoint with the notification bound
/// to it (§147's "a thread blocks in RECEIVE with the notification bound to its TCB, and wakes on
/// either"). Each wakes with the timer's bits, at or after the deadline by the same counter, and
/// the second one's registers say `BOUND`, not a message.
#[test_case]
fn a_wait_ends_at_the_deadline_with_no_signal() {
    static WORD: AtomicU64 = AtomicU64::new(0);
    static WOKE_AT: AtomicU64 = AtomicU64::new(0);
    static RECEIVED: [AtomicU64; 5] = [const { AtomicU64::new(u64::MAX) }; 5];
    static RECEIVED_AT: AtomicU64 = AtomicU64::new(0);
    // Published with Release after all five RECEIVED stores. Waiting on RECEIVED[0] alone could read
    // the later slots before the receiver wrote them, the race notification_tests hit twice.
    static RECEIVED_DONE: AtomicU64 = AtomicU64::new(0);

    let r = region();
    let n = sched::create_notification_from(r).expect("notification");
    let t = sched::create_timer_from(r).expect("timer");

    // In WAIT.
    let deadline = now() + ms(50);
    assert_eq!(sched::timer_arm(t, deadline, n, 0b100), Ok(()));
    sched::spawn(move || {
        let w = sched::notification_wait(n).expect("wait");
        WOKE_AT.store(now(), Ordering::SeqCst);
        WORD.store(w, Ordering::SeqCst);
    })
    .expect("spawn the waiter");
    assert!(
        wait_for(|| WORD.load(Ordering::SeqCst) != 0),
        "the deadline passed and nothing woke the waiter: the tick never reached the walk"
    );
    assert_eq!(WORD.load(Ordering::SeqCst), 0b100);
    let woke = WOKE_AT.load(Ordering::SeqCst);
    assert!(woke >= deadline, "woke before the deadline");
    assert!(
        woke < deadline + ms(1000),
        "woke a second or more late: {} ms",
        (woke - deadline) * 1000 / frequency()
    );
    assert_eq!(
        sched::timer_cancel(t),
        Ok(false),
        "it fired, so nothing was pending"
    );

    // Blocked in RECEIVE, bound: the net_stack and liveness-watch shape.
    let ep = sched::create_rendezvous_from(r).expect("rendezvous");
    let receiver = sched::spawn(move || {
        let m = sched::ipc_receive(ep);
        RECEIVED_AT.store(now(), Ordering::SeqCst);
        for (slot, w) in RECEIVED.iter().zip(m) {
            slot.store(w, Ordering::Relaxed);
        }
        RECEIVED_DONE.store(1, Ordering::Release);
    })
    .expect("spawn the receiver");
    assert!(
        wait_for(|| parked(receiver, |w| matches!(
            w,
            Some(Wait::Rendezvous(e, WaitRole::Receiver)) if e == ep
        ))),
        "the receiver never parked in RECEIVE"
    );
    assert_eq!(sched::notification_bind(n, receiver), Ok(()));
    let deadline = now() + ms(50);
    assert_eq!(sched::timer_arm(t, deadline, n, 0b1000), Ok(()));
    assert!(
        wait_for(|| RECEIVED_DONE.load(Ordering::Acquire) == 1),
        "the deadline did not end a bound RECEIVE"
    );
    let got: [u64; 5] = core::array::from_fn(|i| RECEIVED[i].load(Ordering::Relaxed));
    assert_eq!(
        got,
        [
            abi::notification::BOUND,
            0b1000,
            0,
            0,
            abi::notification::BOUND
        ]
    );
    assert!(RECEIVED_AT.load(Ordering::SeqCst) >= deadline);
    reclaim(r);
}

/// **A cancelled or re-armed timer never fires its old deadline.** Timer A is armed for 30 ms and
/// cancelled, then armed for 30 ms again and re-armed for 400 ms before it could fire. Timer B,
/// armed for 150 ms on the same notification, is the witness: by the time B fires, both of A's
/// abandoned 30 ms deadlines are long past, so the first wake must carry B's bit alone. A's
/// re-armed deadline then fires on its own, once, with its new bits.
#[test_case]
fn a_cancelled_or_rearmed_timer_never_fires_stale() {
    let r = region();
    let n = sched::create_notification_from(r).expect("notification");
    let a = sched::create_timer_from(r).expect("timer a");
    let b = sched::create_timer_from(r).expect("timer b");

    let start = now();
    assert_eq!(sched::timer_arm(a, start + ms(30), n, 0b0001), Ok(()));
    assert_eq!(sched::timer_cancel(a), Ok(true));
    assert_eq!(sched::timer_arm(a, start + ms(30), n, 0b0010), Ok(()));
    assert_eq!(sched::timer_arm(a, start + ms(400), n, 0b0100), Ok(()));
    assert_eq!(sched::timer_arm(b, start + ms(150), n, 0b1000), Ok(()));

    assert_eq!(
        sched::notification_wait(n),
        Ok(0b1000),
        "a cancelled or replaced deadline fired"
    );
    assert!(now() >= start + ms(150));
    assert_eq!(sched::notification_wait(n), Ok(0b0100));
    assert!(now() >= start + ms(400));
    assert_eq!(sched::timer_cancel(a), Ok(false));
    assert_eq!(sched::timer_cancel(b), Ok(false));
    assert_eq!(sched::notification_poll(n), Ok(0), "something fired twice");
    reclaim(r);
}

/// **The syscall layer**: `RETYPE_OBJ` mints a full-rights timer from a full-rights budget (since
/// milestone 824 (a retype mints no right its budget lacks) it carries the budget's rights); `ARM` needs `WRITE` on the timer
/// and on the notification and refuses a slot that is not a notification; a deadline already
/// passed signals before `ARM` returns; and a notification destroyed under an armed timer turns the
/// expiry into a no-op rather than a fault in interrupt context.
#[test_case]
fn arm_checks_both_capabilities_and_a_past_deadline_signals_at_once() {
    let r = region();
    let ut = sched::grant(crate::cap::memory_region_root_cap(r)).expect("grant the region");
    let full = invoke(
        ut,
        abi::memory_region::RETYPE_OBJ,
        abi::objtype::TIMER,
        0,
        0,
    )
    .expect("RETYPE_OBJ(TIMER)") as u64;
    let cap = sched::current_cap(full).expect("the minted slot");
    let Object::Timer(t) = cap.object else {
        panic!("RETYPE_OBJ(TIMER) minted {:?}", cap.object);
    };
    assert_eq!(cap.rights, Rights::ALL);
    let n_slot = invoke(
        ut,
        abi::memory_region::RETYPE_OBJ,
        abi::objtype::NOTIFICATION,
        0,
        0,
    )
    .expect("RETYPE_OBJ(NOTIFICATION)") as u64;
    let Object::Notification(n) = sched::current_cap(n_slot).expect("slot").object else {
        panic!("not a notification");
    };
    let n_read = sched::grant(crate::cap::notification_cap(n, Rights::READ)).expect("read view");
    let t_read = sched::grant(crate::cap::timer_cap(t, Rights::READ)).expect("read view");

    let later = now() + ms(10_000);
    assert_eq!(
        invoke(t_read, abi::timer::ARM, later, n_slot, 1),
        Err(Error::NotPermitted)
    );
    assert_eq!(
        invoke(t_read, abi::timer::CANCEL, 0, 0, 0),
        Err(Error::NotPermitted)
    );
    assert_eq!(
        invoke(full, abi::timer::ARM, later, n_read, 1),
        Err(Error::NotPermitted),
        "arming signals later, so it needs the notification's WRITE"
    );
    assert_eq!(
        invoke(full, abi::timer::ARM, later, ut, 1),
        Err(Error::WrongObject)
    );
    assert_eq!(invoke(full, 99, 0, 0, 0), Err(Error::BadMethod));

    // A deadline already reached: signalled before ARM returns, nothing left armed.
    assert_eq!(
        invoke(full, abi::timer::ARM, now() - 1, n_slot, 0b11),
        Ok(0)
    );
    assert_eq!(sched::notification_poll(n), Ok(0b11));
    assert_eq!(invoke(full, abi::timer::CANCEL, 0, 0, 0), Ok(0));

    // Armed, then its notification destroyed: the expiry must find nobody and do nothing. The
    // notification lives in a second region so it can go while the timer stays.
    let r2 = region();
    let n2 = sched::create_notification_from(r2).expect("second notification");
    let deadline = now() + ms(30);
    assert_eq!(sched::timer_arm(t, deadline, n2, 1), Ok(()));
    reclaim(r2);
    assert!(
        wait_for(|| now() > deadline + ms(50)),
        "the counter stopped"
    );
    assert_eq!(
        sched::timer_cancel(t),
        Ok(false),
        "the expiry should have fired (into nothing) and disarmed"
    );
    assert_eq!(
        sched::timer_arm(t, now() + ms(10), n2, 1),
        Err(Error::Gone),
        "arming at a destroyed notification is refused up front"
    );

    for slot in [t_read, n_read, n_slot, full, ut] {
        let _ = sched::delete_current_cap(slot);
    }
    reclaim(r);
}

/// **The before and after, measured**: 200 ms asleep by a yield loop (read the counter, `yield`,
/// repeat) against arm and `WAIT`. Two kernel threads run the PAL's two loops with the syscall
/// boundary taken out; what the boundary adds per iteration is `null_syscall`'s cost, which
/// `bench/` records.
///
/// The printed line is the measurement, read once per ISA in CI's log. The test asserts only the
/// shape: the timer sleeper is woken once and charged at most a tick or two, where the spinner is
/// charged the ticks that landed while it held the core and loops thousands of times. The exact
/// figures depend on what else the suite has runnable, which is why they are printed.
#[test_case]
fn a_timer_sleep_costs_a_wake_where_a_yield_loop_costs_a_cpu() {
    static SPINS: AtomicU64 = AtomicU64::new(0);
    static SPIN_DONE: AtomicU64 = AtomicU64::new(0);
    static SLEPT: AtomicU64 = AtomicU64::new(0);
    static SPIN_TICKS: AtomicU64 = AtomicU64::new(0);
    static SLEEP_TICKS: AtomicU64 = AtomicU64::new(0);

    let r = region();
    let n = sched::create_notification_from(r).expect("notification");
    let t = sched::create_timer_from(r).expect("timer");
    let period = ms(200);

    sched::spawn(move || {
        let deadline = now() + period;
        let before = sched::current_cpu_ticks();
        let mut spins = 0;
        while now() < deadline {
            sched::yield_now();
            spins += 1;
        }
        SPIN_TICKS.store(sched::current_cpu_ticks() - before, Ordering::SeqCst);
        SPINS.store(spins, Ordering::SeqCst);
        SPIN_DONE.store(1, Ordering::SeqCst);
    })
    .expect("spawn the spinner");
    sched::spawn(move || {
        let deadline = now() + period;
        let before = sched::current_cpu_ticks();
        sched::timer_arm(t, deadline, n, 1).expect("arm");
        let w = sched::notification_wait(n).expect("wait");
        assert!(now() >= deadline);
        SLEEP_TICKS.store(sched::current_cpu_ticks() - before, Ordering::SeqCst);
        SLEPT.store(w, Ordering::SeqCst);
    })
    .expect("spawn the sleeper");

    assert!(
        wait_for(|| SPIN_DONE.load(Ordering::SeqCst) != 0 && SLEPT.load(Ordering::SeqCst) != 0),
        "a 200 ms sleep did not end within two seconds"
    );
    let spin_ticks = SPIN_TICKS.load(Ordering::SeqCst);
    let sleep_ticks = SLEEP_TICKS.load(Ordering::SeqCst);
    let spins = SPINS.load(Ordering::SeqCst);
    crate::println!(
        "timed-wait: 200 ms asleep: yield loop {spins} yields, {spin_ticks} ticks charged; \
         timer 1 wake, {sleep_ticks} ticks charged"
    );
    assert!(
        sleep_ticks <= 2,
        "a thread asleep on a timer was charged {sleep_ticks} ticks"
    );
    assert!(
        spins > 100,
        "the spinner yielded only {spins} times in 200 ms"
    );
    reclaim(r);
}
