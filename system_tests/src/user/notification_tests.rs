//! The tests for milestone 151 (notification objects), which builds DECISIONS §101 (notification objects), on
//! every architecture.
//!
//! Each proves something the crate's Kani harnesses cannot, because it is about the kernel half:
//! the registry, the wake, the TCB binding reaching into another object's wait queue, the syscall
//! layer's rights and registers, and teardown. The state machine itself (no bit lost, a waiter
//! before the bound receiver, the invariant) is proved in
//! `crates/inter_process_communication/src/notification.rs` and not re-tested here.

use core::sync::atomic::{AtomicU64, Ordering};

use abi::Error;

use super::{Spawn, program, run, wait_for};
use crate::arch::exceptions::TrapFrame;
use crate::cap::{Object, Rights};
use crate::sched;
use crate::thread::{Wait, WaitRole};

/// A region big enough for a couple of notification pages and a rendezvous.
fn region() -> u64 {
    crate::memory_region::create(4).expect("no region for a notification")
}

/// Give a test's region back once nothing in it is still live, so the suite's frame budget is what
/// it was before the test ran. It matters: the suite runs close to its frame ceiling, and the
/// first version of these tests, which kept their regions, ran a later test out of page frames on
/// riscv64.
fn reclaim(r: u64) {
    assert!(
        wait_for(|| sched::reclaim_region(r).is_ok()),
        "a test region could not be reclaimed: something in it is still live"
    );
}

/// Invoke through the real syscall layer, from this kernel thread's own capability table.
fn invoke(slot: u64, method: u64, a0: u64) -> (Result<i64, Error>, TrapFrame) {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    let r = crate::syscall::invoke(&mut frame, slot, method, a0, 0, 0);
    (r, frame)
}

/// Is `tid` parked the way `want` says? Read under the scheduler's lock, from the test accessor.
fn parked(tid: crate::thread::ThreadId, want: impl Fn(Option<Wait>) -> bool) -> bool {
    sched::thread_death_disposition(tid)
        .is_some_and(|d| d.state == crate::thread::State::Blocked && want(d.wait_on))
}

/// **A bound receiver is woken out of `RECEIVE` by a signal, can tell, and the endpoint still works.**
///
/// The binding's whole mechanism in one test. The receiver blocks in an ordinary `ipc_receive`; a
/// signal on its bound notification must unlink it from the rendezvous's receiver queue, deliver
/// `(BOUND, word, 0, 0, BOUND)`, and wake it. Then a real message on the same rendezvous must still
/// reach it: a bound delivery that left a stale link in the queue (the classic intrusive-queue
/// failure) would lose or misdeliver that second message, or crash on it.
///
/// And the forgery §101's encoding permitted: the second message is `SEND(BOUND, ...)`, which puts
/// `BOUND` in `w0`, and the receiver must see `w4 == 0`, a message.
#[test_case]
fn a_bound_receiver_is_woken_out_of_receive_and_the_endpoint_still_works() {
    static FIRST: [AtomicU64; 5] = [const { AtomicU64::new(u64::MAX) }; 5];
    static SECOND: [AtomicU64; 5] = [const { AtomicU64::new(u64::MAX) }; 5];
    // Each DONE flag is published with Release after all five stores of its message. Waiting on
    // slot 0 alone raced: the main thread read the later slots before the receiver wrote them
    // (SECOND on rva23s64, #1635; FIRST on rv64, read as [2, MAX, MAX, MAX, MAX] in #1801).
    static FIRST_DONE: AtomicU64 = AtomicU64::new(0);
    static SECOND_DONE: AtomicU64 = AtomicU64::new(0);

    let r = region();
    let n = sched::create_notification_from(r).expect("notification");
    let ep = sched::create_rendezvous_from(r).expect("rendezvous");

    let receiver = sched::spawn(move || {
        let m = sched::ipc_receive(ep);
        for (slot, w) in FIRST.iter().zip(m) {
            slot.store(w, Ordering::Relaxed);
        }
        FIRST_DONE.store(1, Ordering::Release);
        let m = sched::ipc_receive(ep);
        for (slot, w) in SECOND.iter().zip(m) {
            slot.store(w, Ordering::Relaxed);
        }
        SECOND_DONE.store(1, Ordering::Release);
    })
    .expect("spawn the receiver");

    assert!(
        wait_for(|| parked(receiver, |w| matches!(
            w,
            Some(Wait::Rendezvous(e, WaitRole::Receiver)) if e == ep
        ))),
        "the receiver never parked in RECEIVE"
    );
    // Bound while already receiving, with nothing pending: nothing happens yet.
    assert_eq!(sched::notification_bind(n, receiver), Ok(()));
    assert_eq!(sched::rendezvous_waiting_receivers(ep), 1);

    assert_eq!(sched::notification_signal(n, 0b1010), Ok(()));
    assert!(
        wait_for(|| FIRST_DONE.load(Ordering::Acquire) == 1),
        "a signal on the bound notification did not end the RECEIVE"
    );
    let first: [u64; 5] = core::array::from_fn(|i| FIRST[i].load(Ordering::Relaxed));
    assert_eq!(
        first,
        [
            abi::notification::BOUND,
            0b1010,
            0,
            0,
            abi::notification::BOUND
        ],
        "a bound delivery must carry the tag in w0 and w4 and the word in w1"
    );

    // Back in RECEIVE on the same rendezvous: the queue must hold exactly this thread again.
    assert!(
        wait_for(|| sched::rendezvous_waiting_receivers(ep) == 1),
        "the receiver did not re-park, or the bound delivery left the queue corrupt"
    );
    sched::ipc_send(ep, [abi::notification::BOUND, 0xdead, 7]);
    assert!(
        wait_for(|| SECOND_DONE.load(Ordering::Acquire) == 1),
        "a message after a bound delivery never arrived"
    );
    let second: [u64; 5] = core::array::from_fn(|i| SECOND[i].load(Ordering::Relaxed));
    assert_eq!(
        second,
        [abi::notification::BOUND, 0xdead, 7, 0, 0],
        "a sender that forged BOUND in w0 must still arrive with w4 == 0"
    );
    reclaim(r);
}

/// **A signal counted while the bound thread was elsewhere ends its next receive at once**, on
/// both receive shapes (`RECEIVE` and `RECEIVE_CAP`). §101's rule 3: "counted ... and delivered when the
/// TCB next enters RECEIVE on any endpoint". Without the receive-side check a server that took a
/// signal while busy would sleep on it until an unrelated message arrived.
#[test_case]
fn a_signal_counted_while_elsewhere_ends_the_next_receive_at_once() {
    static GO: AtomicU64 = AtomicU64::new(0);
    static PLAIN: [AtomicU64; 5] = [const { AtomicU64::new(u64::MAX) }; 5];
    static WITH_CAP: [AtomicU64; 5] = [const { AtomicU64::new(u64::MAX) }; 5];
    // Release after all five stores of each message, for the reason FIRST_DONE gives above.
    static PLAIN_DONE: AtomicU64 = AtomicU64::new(0);
    static WITH_CAPABILITY_DONE: AtomicU64 = AtomicU64::new(0);

    let r = region();
    let n = sched::create_notification_from(r).expect("notification");
    let ep = sched::create_rendezvous_from(r).expect("rendezvous");

    let t = sched::spawn(move || {
        // Busy, not receiving, until told.
        while GO.load(Ordering::SeqCst) == 0 {
            sched::yield_now();
        }
        let m = sched::ipc_receive(ep);
        for (slot, w) in PLAIN.iter().zip(m) {
            slot.store(w, Ordering::Relaxed);
        }
        PLAIN_DONE.store(1, Ordering::Release);
        while GO.load(Ordering::SeqCst) == 1 {
            sched::yield_now();
        }
        let m = sched::ipc_receive_cap(ep);
        for (slot, w) in WITH_CAP.iter().zip(m) {
            slot.store(w, Ordering::Relaxed);
        }
        WITH_CAPABILITY_DONE.store(1, Ordering::Release);
    })
    .expect("spawn");

    assert_eq!(sched::notification_bind(n, t), Ok(()));
    assert_eq!(sched::notification_signal(n, 0b1), Ok(()));
    assert_eq!(sched::notification_signal(n, 0b100), Ok(()));
    GO.store(1, Ordering::SeqCst);
    assert!(wait_for(|| PLAIN_DONE.load(Ordering::Acquire) == 1));
    let plain: [u64; 5] = core::array::from_fn(|i| PLAIN[i].load(Ordering::Relaxed));
    assert_eq!(
        plain,
        [
            abi::notification::BOUND,
            0b101,
            0,
            0,
            abi::notification::BOUND
        ],
        "RECEIVE must take the accumulated word on entry"
    );
    assert_eq!(
        sched::rendezvous_waiting_receivers(ep),
        0,
        "it must not have queued"
    );

    assert_eq!(sched::notification_signal(n, 0b1000), Ok(()));
    GO.store(2, Ordering::SeqCst);
    assert!(wait_for(
        || WITH_CAPABILITY_DONE.load(Ordering::Acquire) == 1
    ));
    let with_cap: [u64; 5] = core::array::from_fn(|i| WITH_CAP[i].load(Ordering::Relaxed));
    assert_eq!(
        with_cap,
        [
            abi::notification::BOUND,
            0b1000,
            0,
            0,
            abi::notification::BOUND
        ],
        "RECEIVE_CAP must take it too"
    );
    reclaim(r);
}

/// **`WAIT` blocks, a signal wakes it with exactly its bits, and a notification destroyed under a
/// waiter answers `Gone`** rather than leaving it asleep for the life of the machine: the region
/// sweep aborts a notification's waiters as it aborts a rendezvous's.
#[test_case]
fn wait_blocks_until_signalled_and_a_destroyed_notification_answers_gone() {
    static WOKE: AtomicU64 = AtomicU64::new(0);
    static ORPHANED: AtomicU64 = AtomicU64::new(0);

    let r = region();
    let n = sched::create_notification_from(r).expect("notification");
    let waiter = sched::spawn(move || {
        WOKE.store(
            sched::notification_wait(n).unwrap_or(u64::MAX),
            Ordering::SeqCst,
        );
    })
    .expect("spawn");
    assert!(
        wait_for(|| parked(waiter, |w| w == Some(Wait::Notification(n)))),
        "the waiter never blocked in WAIT"
    );
    assert_eq!(sched::notification_signal(n, 0x40), Ok(()));
    assert!(wait_for(|| WOKE.load(Ordering::SeqCst) != 0));
    assert_eq!(WOKE.load(Ordering::SeqCst), 0x40);
    assert_eq!(
        sched::notification_poll(n),
        Ok(0),
        "the woken waiter took the bits"
    );

    let doomed_region = region();
    let doomed = sched::create_notification_from(doomed_region).expect("notification");
    let orphan = sched::spawn(move || {
        let answer = match sched::notification_wait(doomed) {
            Err(Error::Gone) => 1,
            _ => 2,
        };
        ORPHANED.store(answer, Ordering::SeqCst);
    })
    .expect("spawn");
    assert!(wait_for(
        || parked(orphan, |w| w == Some(Wait::Notification(doomed)))
    ));
    assert_eq!(sched::reclaim_region(doomed_region), Ok(()));
    assert!(
        wait_for(|| ORPHANED.load(Ordering::SeqCst) != 0),
        "a waiter on a destroyed notification was never woken"
    );
    assert_eq!(ORPHANED.load(Ordering::SeqCst), 1, "it must answer Gone");
    assert_eq!(sched::notification_signal(doomed, 1), Err(Error::Gone));
    assert_eq!(sched::notification_poll(doomed), Err(Error::Gone));
    reclaim(r);
}

/// **Bind is once per side, and a dead binding does not count.** A notification binds one thread
/// and a thread binds one notification (§101); a thread whose notification has been destroyed may
/// be bound again, because the stale name resolves to nothing.
#[test_case]
fn bind_is_once_per_side_and_a_destroyed_binding_frees_the_thread() {
    static STOP: AtomicU64 = AtomicU64::new(0);
    let first_region = region();
    let a = sched::create_notification_from(first_region).expect("a");
    let second_region = region();
    let b = sched::create_notification_from(second_region).expect("b");
    let t = sched::spawn(|| {
        while STOP.load(Ordering::SeqCst) == 0 {
            sched::yield_now();
        }
    })
    .expect("spawn");
    let u = sched::spawn(|| {
        while STOP.load(Ordering::SeqCst) == 0 {
            sched::yield_now();
        }
    })
    .expect("spawn");

    assert_eq!(sched::notification_bind(a, t), Ok(()));
    assert_eq!(
        sched::notification_bind(a, u),
        Err(Error::NotPermitted),
        "notification twice"
    );
    assert_eq!(
        sched::notification_bind(b, t),
        Err(Error::NotPermitted),
        "thread twice"
    );

    assert_eq!(sched::reclaim_region(first_region), Ok(()));
    assert_eq!(
        sched::notification_bind(b, t),
        Ok(()),
        "a thread whose notification was destroyed must be bindable again"
    );
    STOP.store(1, Ordering::SeqCst);
    reclaim(second_region);
}

/// **The rights are §101's, enforced at the syscall layer**: `WRITE` signals and binds, `READ`
/// waits and polls, and neither does the other's job. Driven through `syscall::invoke` so the
/// boundary is what is proved, not the helper behind it. `RETYPE_OBJ` mints the object, and the
/// creator's capability carries every right its budget holds (milestone 824 (a retype mints no
/// right its budget lacks)); the budget here is the root's, `Rights::ALL`.
#[test_case]
fn the_rights_are_101s_and_retype_obj_mints_a_full_rights_notification() {
    let r = region();
    let ut = sched::grant(crate::cap::memory_region_root_cap(r)).expect("grant the region");
    let (minted, _) = invoke(
        ut,
        abi::memory_region::RETYPE_OBJ,
        abi::objtype::NOTIFICATION,
    );
    let full = minted.expect("RETYPE_OBJ(NOTIFICATION)") as u64;
    let cap = sched::current_cap(full).expect("the minted slot");
    let Object::Notification(id) = cap.object else {
        panic!("RETYPE_OBJ(NOTIFICATION) minted {:?}", cap.object);
    };
    assert_eq!(cap.rights, Rights::ALL);

    let read = sched::grant(crate::cap::notification_cap(id, Rights::READ)).expect("read view");
    let write = sched::grant(crate::cap::notification_cap(id, Rights::WRITE)).expect("write view");

    assert_eq!(
        invoke(read, abi::notification::SIGNAL, 1).0,
        Err(Error::NotPermitted)
    );
    assert_eq!(
        invoke(read, abi::notification::BIND, write).0,
        Err(Error::NotPermitted)
    );
    assert_eq!(
        invoke(write, abi::notification::POLL, 0).0,
        Err(Error::NotPermitted)
    );
    assert_eq!(
        invoke(write, abi::notification::WAIT, 0).0,
        Err(Error::NotPermitted)
    );
    assert_eq!(invoke(write, 99, 0).0, Err(Error::BadMethod));

    assert_eq!(invoke(write, abi::notification::SIGNAL, 0b11).0, Ok(0));
    assert_eq!(invoke(read, abi::notification::POLL, 0).0, Ok(0b11));
    assert_eq!(invoke(write, abi::notification::SIGNAL, 0b100).0, Ok(0));
    assert_eq!(invoke(read, abi::notification::WAIT, 0).0, Ok(0b100));
    // BIND's thread slot must hold a thread.
    assert_eq!(
        invoke(write, abi::notification::BIND, read).0,
        Err(Error::WrongObject)
    );

    for slot in [write, read, full, ut] {
        let _ = sched::delete_current_cap(slot);
    }
    reclaim(r);
}

/// **A kernel-originated signal wakes a driver blocked in `Interrupt::WAIT`, through the syscall
/// layer's registers.** This is the case of milestone 106 (a wait that ends on either the interrupt or the deadline), per §147 (a timer a userspace service cannot hold): `net_stack` blocks on its interrupt,
/// and a timer expiry must be able to end that wait. The signal comes through
/// [`sched::signal_notification_from_interrupt`], the entry 106's tick will call; it is invoked from
/// a kernel thread here, not from a real interrupt, so what this proves is the load-aware wake and
/// the `Interrupt::WAIT` registers, not the interrupt-stack context (which `interrupt_notify`
/// already shares).
///
/// The interrupt number is one no device on any of the three machines raises, chosen because a
/// route is global and this test must not steal a real device's.
///
/// Falsification: unfalsified. Renamed by the `irq` spelling sweep (it was
/// `a_kernel_signal_ends_an_irq_wait_with_the_word_in_x1_and_the_tag_in_x4`), which ran it green
/// and changed nothing it asserts; no defect has been injected to turn it red.
#[test_case]
fn a_kernel_signal_ends_an_interrupt_wait_with_the_word_in_x1_and_the_tag_in_x4() {
    const QUIET_INTID: u32 = 251;
    static RESULT: [AtomicU64; 3] = [const { AtomicU64::new(u64::MAX) }; 3];

    let r = region();
    let n = sched::create_notification_from(r).expect("notification");
    let ep = match sched::interrupt_route(QUIET_INTID) {
        Some(ep) => ep,
        None => {
            let ep = sched::create_rendezvous_from(r).expect("rendezvous");
            sched::bind_interrupt(QUIET_INTID, ep);
            ep
        }
    };

    let driver = sched::spawn(move || {
        let slot = sched::grant(crate::cap::interrupt_cap_rights(QUIET_INTID, Rights::READ))
            .expect("grant the interrupt");
        let (answer, frame) = invoke(slot, abi::interrupt::WAIT, 0);
        RESULT[0].store(answer.map_or(u64::MAX - 1, |v| v as u64), Ordering::SeqCst);
        RESULT[1].store(frame.arg(1), Ordering::SeqCst);
        RESULT[2].store(frame.arg(4), Ordering::SeqCst);
        let _ = sched::delete_current_cap(slot);
    })
    .expect("spawn the driver");

    assert_eq!(sched::notification_bind(n, driver), Ok(()));
    assert!(
        wait_for(|| parked(driver, |w| matches!(
            w,
            Some(Wait::Rendezvous(e, WaitRole::Receiver)) if e == ep
        ))),
        "the driver never blocked in Interrupt::WAIT"
    );
    sched::signal_notification_from_interrupt(n, 0x8000);
    assert!(wait_for(|| RESULT[2].load(Ordering::SeqCst) != u64::MAX));
    assert_eq!(RESULT[0].load(Ordering::SeqCst), abi::notification::BOUND);
    assert_eq!(RESULT[1].load(Ordering::SeqCst), 0x8000);
    assert_eq!(RESULT[2].load(Ordering::SeqCst), abi::notification::BOUND);
    reclaim(r);
}

/// **A program binds a notification to itself and is woken out of `RECEIVE` by it**, through the
/// real syscall boundary and the real user-mode wrapper, on every architecture. The fixture
/// (`fixtures/src/notification_binder.rs`) checks from inside what it can: accumulation, poll,
/// wait-without-blocking, a zero signal, bind-once, `WrongObject`, and a counted signal ending its
/// next receive. It then hands this test a signal-only view of its notification and blocks, and
/// this test proves the two things only the other side can see: a signal from another thread
/// wakes it with the tag in `x4`, and a sender forging `BOUND` in `w0` arrives as a message.
#[test_case]
fn a_program_binds_a_notification_and_receive_tells_a_signal_from_a_forgery() {
    let Some(image) = program("notification_binder") else {
        crate::testing::skip!("no notification_binder program in this archive");
    };
    let budget = region();
    // From the test's own region rather than the kernel's endpoint chunks: a chunk is 32 pages
    // carved on demand and never returned, and measured here the kernel-chunk version cost the
    // suite 33 frames for good.
    let ep = sched::create_rendezvous_from(budget).expect("rendezvous");
    let report = sched::create_rendezvous_from(budget).expect("rendezvous");

    sched::spawn(move || {
        let me = sched::current();
        run(
            image,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    crate::cap::memory_region_lending_capability(budget),
                    crate::cap::thread_control_block_cap(me, Rights::WRITE),
                    crate::cap::rendezvous_cap(ep, Rights::READ),
                    crate::cap::rendezvous_cap(report, Rights::WRITE),
                ],
                maps: &[],
            },
        )
    })
    .expect("spawn the binder");

    // The delegation: a WRITE-only notification capability, landed in this thread's table.
    let [_, slot, ..] = sched::ipc_receive_cap(report);
    assert_ne!(
        slot,
        abi::rendezvous::NO_CAP,
        "the binder's checks failed before delegating"
    );
    let cap = sched::current_cap(slot).expect("delegated slot");
    let Object::Notification(n) = cap.object else {
        panic!("the binder delegated {:?}", cap.object);
    };
    assert_eq!(cap.rights, Rights::WRITE);

    assert!(
        wait_for(|| sched::rendezvous_waiting_receivers(ep) == 1),
        "the binder never blocked in RECEIVE"
    );
    let (signalled, _) = invoke(slot, abi::notification::SIGNAL, 0x1_0000);
    assert_eq!(signalled, Ok(0));
    let m = sched::ipc_receive(report);
    assert_eq!(
        [m[0], m[1], m[2]],
        [abi::notification::BOUND, 0x1_0000, 1],
        "the program's RECEIVE was not ended by the signal, or could not tell"
    );

    assert!(wait_for(|| sched::rendezvous_waiting_receivers(ep) == 1));
    sched::ipc_send(ep, [abi::notification::BOUND, 0xbad, 0]);
    let m = sched::ipc_receive(report);
    assert_eq!(
        [m[0], m[1], m[2]],
        [abi::notification::BOUND, 0xbad, 0],
        "a sender that put BOUND in w0 was taken for a notification"
    );
    let _ = sched::delete_current_cap(slot);
    let _ = n;
    reclaim(budget);
}
