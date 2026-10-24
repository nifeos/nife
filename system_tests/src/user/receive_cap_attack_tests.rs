//! **A plain `SEND` received by `RECEIVE_CAP` hands the receiver `NO_CAP` in `x1`, never a
//! sender-chosen word** (milestone 634 (a plain SEND received by RECEIVE_CAP never hands the receiver
//! a sender-chosen slot), fatal risk 7's confinement claim).
//!
//! `RECEIVE_CAP` returns in `x1` the slot a delivered capability landed in, or `NO_CAP`, and a
//! `CALL` server reads it as its `Reply`'s slot. The receive side writes `NO_CAP` unless a
//! capability was installed for this delivery (`Thread::cap_delivered`, `kernel/src/sched.rs`), so
//! the receiver-first order, where `ipc_send` drops its words into the parked receiver's mailbox,
//! matches the sender-first one. A chosen word would aim a server's `cap_delete` at its own
//! capabilities. See notes/confinement-claims.md, "What `RECEIVE_CAP` hands back".

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::cap::{Rights, rendezvous_cap};
use crate::sched;

fn wait_for(mut cond: impl FnMut() -> bool) -> bool {
    let deadline = crate::arch::timer::now() + 2 * crate::arch::timer::frequency();
    while crate::arch::timer::now() < deadline {
        if cond() {
            return true;
        }
        sched::yield_now();
    }
    cond()
}

/// A sentinel the attacker's second word carries. It is not `NO_CAP`, so a receiver that returned
/// it in `x1` would be returning a sender-chosen number.
const CHOSEN: u64 = 7;

/// **Both arrival orders deliver `NO_CAP` in `x1` for a plain `SEND`.** Receiver-first was the
/// exploitable order on `main`; sender-first always returned `NO_CAP`. Asserting both is what makes
/// the fix's symmetry the property rather than one patched branch.
///
/// Falsification: replayable `system_tests/falsifications/user.receive_cap_attack_tests.a_plain_send_to_receive_cap_delivers_no_cap_whichever_side_parks_first.patch`
#[test_case]
fn a_plain_send_to_receive_cap_delivers_no_cap_whichever_side_parks_first() {
    static RECEIVER_FIRST_X1: AtomicU64 = AtomicU64::new(0);
    static SERVER_PARKED: AtomicBool = AtomicBool::new(false);
    static RECEIVER_FIRST_DONE: AtomicBool = AtomicBool::new(false);

    let region = crate::memory_region::create(4).expect("no region");
    let ep = sched::create_rendezvous_from(region).expect("no rendezvous");

    // Receiver-first: the server parks in RECEIVE_CAP, then the attacker sends a chosen word.
    sched::spawn(move || {
        SERVER_PARKED.store(true, Ordering::SeqCst);
        let [_w0, x1, ..] = sched::ipc_receive_cap(ep);
        RECEIVER_FIRST_X1.store(x1, Ordering::SeqCst);
        RECEIVER_FIRST_DONE.store(true, Ordering::SeqCst);
    })
    .expect("no server thread");
    assert!(
        wait_for(
            || SERVER_PARKED.load(Ordering::SeqCst) && sched::rendezvous_waiting_receivers(ep) == 1
        ),
        "the server never parked in RECEIVE_CAP, so nothing proved the receiver-first order",
    );
    sched::ipc_send(ep, [1, CHOSEN, 0]);
    assert!(
        wait_for(|| RECEIVER_FIRST_DONE.load(Ordering::SeqCst)),
        "the receiver-first server never returned from RECEIVE_CAP",
    );
    assert_eq!(
        RECEIVER_FIRST_X1.load(Ordering::SeqCst),
        abi::rendezvous::NO_CAP,
        "a plain SEND handed the receiver x1 = {} on the receiver-first order, a sender-chosen \
         word where a CALL server reads a reply slot",
        RECEIVER_FIRST_X1.load(Ordering::SeqCst),
    );

    // Sender-first: the attacker parks as a sender, then this thread receives.
    static SENT: AtomicBool = AtomicBool::new(false);
    sched::spawn(move || {
        SENT.store(true, Ordering::SeqCst);
        sched::ipc_send(ep, [1, CHOSEN, 0]);
    })
    .expect("no attacker thread");
    assert!(
        wait_for(|| SENT.load(Ordering::SeqCst) && sched::rendezvous_waiting_senders(ep) == 1),
        "the attacker never parked as a sender, so nothing proved the sender-first order",
    );
    let [_w0, x1, ..] = sched::ipc_receive_cap(ep);
    assert_eq!(
        x1,
        abi::rendezvous::NO_CAP,
        "a plain SEND handed the receiver x1 = {x1} on the sender-first order",
    );

    crate::memory_region::destroy(region);
}

const VICTIM_SLOT: u64 = 7;

/// **A server that deletes the received slot deletes nothing of its own** (the `net_stack` shape).
/// The server grants itself a capability at a fixed slot, receives a plain `SEND`, and
/// unconditionally deletes `x1`. With `x1 == NO_CAP` the delete finds nothing, so the server's own
/// capability survives on both orders. On `main` the receiver-first order deleted it.
#[test_case]
fn a_server_that_deletes_the_received_slot_deletes_nothing_of_its_own() {
    for receiver_first in [true, false] {
        static SURVIVED: AtomicBool = AtomicBool::new(false);
        static DONE: AtomicBool = AtomicBool::new(false);
        static READY: AtomicBool = AtomicBool::new(false);
        SURVIVED.store(false, Ordering::SeqCst);
        DONE.store(false, Ordering::SeqCst);
        READY.store(false, Ordering::SeqCst);

        let region = crate::memory_region::create(4).expect("no region");
        let ep = sched::create_rendezvous_from(region).expect("no rendezvous");
        let victim_ep = sched::create_rendezvous_from(region).expect("no victim rendezvous");

        let server = move || {
            sched::grant_at(VICTIM_SLOT, rendezvous_cap(victim_ep, Rights::WRITE))
                .expect("the server could not hold its victim capability");
            READY.store(true, Ordering::SeqCst);
            let [_w0, x1, ..] = sched::ipc_receive_cap(ep);
            let _ = sched::delete_current_cap(x1); // net_stack's unconditional cap_delete(x1)
            SURVIVED.store(sched::current_cap(VICTIM_SLOT).is_ok(), Ordering::SeqCst);
            let _ = sched::delete_current_cap(VICTIM_SLOT); // leave this thread's table clean
            DONE.store(true, Ordering::SeqCst);
        };

        if receiver_first {
            sched::spawn(server).expect("no server thread");
            assert!(
                wait_for(
                    || READY.load(Ordering::SeqCst) && sched::rendezvous_waiting_receivers(ep) == 1
                ),
                "the server never parked (receiver-first)",
            );
            sched::ipc_send(ep, [1, VICTIM_SLOT, 0]);
        } else {
            static SENT: AtomicBool = AtomicBool::new(false);
            SENT.store(false, Ordering::SeqCst);
            sched::spawn(move || {
                SENT.store(true, Ordering::SeqCst);
                sched::ipc_send(ep, [1, VICTIM_SLOT, 0]);
            })
            .expect("no attacker thread");
            assert!(
                wait_for(
                    || SENT.load(Ordering::SeqCst) && sched::rendezvous_waiting_senders(ep) == 1
                ),
                "the attacker never parked (sender-first)",
            );
            sched::spawn(server).expect("no server thread");
        }

        assert!(
            wait_for(|| DONE.load(Ordering::SeqCst)),
            "the server never finished"
        );
        assert!(
            SURVIVED.load(Ordering::SeqCst),
            "the server deleted its own capability at the attacker-chosen slot (receiver_first = {receiver_first})",
        );
        crate::memory_region::destroy(region);
    }
}

/// **A `SEND_CAP` aborted by its rendezvous's teardown stages nothing for a later plain `SEND`**
/// (the 2026-10-03 security audit's follow-up, item (c)). A sender parked in `SEND_CAP` keeps its
/// delegation in `Thread::outgoing_cap` until a receiver takes it. `reclaim_region` drains the
/// rendezvous and wakes the sender aborted; before the fix in `set_ipc_aborted` the capability
/// stayed staged, and the sender's next plain `SEND`, parked on an unrelated rendezvous, handed it
/// to whoever `RECEIVE_CAP`ed there: a delegation made to one endpoint, delivered to another.
#[test_case]
fn a_send_cap_aborted_by_teardown_stages_nothing_for_a_later_plain_send() {
    static PARKED_ON_DOOMED: AtomicBool = AtomicBool::new(false);
    static ABORTED: AtomicBool = AtomicBool::new(false);
    static DONE: AtomicBool = AtomicBool::new(false);

    let doomed = crate::memory_region::create(4).expect("no doomed region");
    let doomed_ep = sched::create_rendezvous_from(doomed).expect("no doomed rendezvous");
    let region = crate::memory_region::create(4).expect("no region");
    let other_ep = sched::create_rendezvous_from(region).expect("no other rendezvous");
    // Any capability will do as the delegation; what matters is that it must not arrive at
    // `other_ep`.
    let delegated = rendezvous_cap(other_ep, Rights::WRITE);

    sched::spawn(move || {
        PARKED_ON_DOOMED.store(true, Ordering::SeqCst);
        sched::ipc_send_cap(doomed_ep, 1, delegated, 0);
        // The teardown below wakes us aborted; a syscall would read and clear the flag here.
        let _ = sched::take_ipc_aborted();
        ABORTED.store(true, Ordering::SeqCst);
        sched::ipc_send(other_ep, [2, CHOSEN, 0]);
        DONE.store(true, Ordering::SeqCst);
    })
    .expect("no sender thread");
    assert!(
        wait_for(|| PARKED_ON_DOOMED.load(Ordering::SeqCst)
            && sched::rendezvous_waiting_senders(doomed_ep) == 1),
        "the sender never parked in SEND_CAP on the doomed rendezvous",
    );
    sched::reclaim_region(doomed).expect("the doomed region did not reclaim");
    assert!(
        wait_for(
            || ABORTED.load(Ordering::SeqCst) && sched::rendezvous_waiting_senders(other_ep) == 1
        ),
        "the sender never came back aborted and parked its plain SEND on the other rendezvous",
    );
    let [w0, x1, ..] = sched::ipc_receive_cap(other_ep);
    assert_eq!(w0, 2, "the plain SEND's first word did not arrive");
    assert_eq!(
        x1,
        abi::rendezvous::NO_CAP,
        "a plain SEND after an aborted SEND_CAP delivered the stale delegation at slot {x1}",
    );
    assert!(
        wait_for(|| DONE.load(Ordering::SeqCst)),
        "the sender never finished"
    );
    crate::memory_region::destroy(region);
}

/// **A `SEND_CAP` collected by a plain `RECEIVE` stages nothing for a later plain `SEND`** (milestone
/// 633 (an outside agent attacks the confinement claim), fatal risk 7's first outsider pass,
/// 2026-10-03 UTC). The non-abort sibling of
/// `a_send_cap_aborted_by_teardown_stages_nothing_for_a_later_plain_send`.
///
/// Woken by a plain `RECEIVE` that collects it (`ipc_receive`'s `!leave_blocked` branch skips
/// `set_ipc_aborted`), the sender must not keep its delegation in `outgoing_cap` for its next plain
/// `SEND`. Milestone 634's `cap_delivered` covers only receiver-first, not this `FromSender` path.
///
/// Falsification: replayable `system_tests/falsifications/user.receive_cap_attack_tests.a_send_cap_collected_by_a_plain_receive_stages_nothing_for_a_later_plain_send.patch`
#[test_case]
fn a_send_cap_collected_by_a_plain_receive_stages_nothing_for_a_later_plain_send() {
    static PARKED_ON_FIRST: AtomicBool = AtomicBool::new(false);
    static DONE: AtomicBool = AtomicBool::new(false);

    let first = crate::memory_region::create(4).expect("no first region");
    let first_ep = sched::create_rendezvous_from(first).expect("no first rendezvous");
    let region = crate::memory_region::create(4).expect("no region");
    let other_ep = sched::create_rendezvous_from(region).expect("no other rendezvous");
    // Any capability will do as the delegation; what matters is that it must not arrive at
    // `other_ep`, where the sender only does a plain `SEND`.
    let delegated = rendezvous_cap(other_ep, Rights::WRITE);

    sched::spawn(move || {
        PARKED_ON_FIRST.store(true, Ordering::SeqCst);
        // Parks as a SEND_CAP sender on first_ep; the plain RECEIVE below collects it and delivers no
        // capability, completing the rendezvous without an abort.
        sched::ipc_send_cap(first_ep, 1, delegated, 0);
        // Then a plain SEND on an unrelated rendezvous. If the delegation stayed staged, this hands
        // it to the RECEIVE_CAP below.
        sched::ipc_send(other_ep, [2, CHOSEN, 0]);
        DONE.store(true, Ordering::SeqCst);
    })
    .expect("no sender thread");
    assert!(
        wait_for(|| PARKED_ON_FIRST.load(Ordering::SeqCst)
            && sched::rendezvous_waiting_senders(first_ep) == 1),
        "the sender never parked in SEND_CAP on the first rendezvous",
    );
    // Plain RECEIVE: takes the data word, no capability, and wakes the sender through the successful
    // path (not set_ipc_aborted). This is the vacuity guard's premise: a SEND_CAP really was in
    // flight and was collected by a receiver that cannot hold a capability.
    let [w0, ..] = sched::ipc_receive(first_ep);
    assert_eq!(
        w0, 1,
        "the SEND_CAP's data word did not arrive at the plain RECEIVE, so nothing was collected",
    );
    assert!(
        wait_for(|| sched::rendezvous_waiting_senders(other_ep) == 1),
        "the sender never parked its plain SEND on the other rendezvous",
    );
    let [w0, x1, ..] = sched::ipc_receive_cap(other_ep);
    assert_eq!(w0, 2, "the plain SEND's first word did not arrive");
    assert_eq!(
        x1,
        abi::rendezvous::NO_CAP,
        "a plain SEND after a plain-RECEIVE-collected SEND_CAP delivered the stale delegation at slot \
         {x1}, a capability granted to one endpoint reaching a receiver on another",
    );
    assert!(
        wait_for(|| DONE.load(Ordering::SeqCst)),
        "the sender never finished"
    );
    crate::memory_region::destroy(region);
}

/// How many slots of the running thread's capability table hold something.
fn held() -> usize {
    (0..crate::cap::CAPABILITY_TABLE_SLOTS as u64)
        .filter(|&slot| sched::current_cap(slot).is_ok())
        .count()
}

/// What one round of [`a_call_server_tells_its_reply_from_a_delegation_on_both_arrival_orders`]
/// saw, written by its server, caller and attacker threads and read by the test once they are done.
struct CallServerRound {
    received: AtomicU64,
    server_done: AtomicBool,
    held_before: AtomicU64,
    held_after: AtomicU64,
    delegation_x4: AtomicU64,
    call_x4: AtomicU64,
    call_answer: AtomicU64,
    call_done: AtomicBool,
    delegation_sent: AtomicBool,
}

static CALL_SERVER_ROUND: CallServerRound = CallServerRound {
    received: AtomicU64::new(0),
    server_done: AtomicBool::new(false),
    held_before: AtomicU64::new(0),
    held_after: AtomicU64::new(0),
    delegation_x4: AtomicU64::new(u64::MAX),
    call_x4: AtomicU64::new(u64::MAX),
    call_answer: AtomicU64::new(0),
    call_done: AtomicBool::new(false),
    delegation_sent: AtomicBool::new(false),
};

impl CallServerRound {
    /// The attacker's first word.
    const DELEGATION: u64 = 1;
    /// The caller's first word.
    const REQUEST: u64 = 2;
    /// What the server answers the caller.
    const ANSWER: u64 = 0x706;

    /// Back to the start of a round. The two `held_*` counts are not reset: the server writes both
    /// before anything reads them.
    fn reset(&self) {
        self.received.store(0, Ordering::SeqCst);
        self.server_done.store(false, Ordering::SeqCst);
        self.delegation_x4.store(u64::MAX, Ordering::SeqCst);
        self.call_x4.store(u64::MAX, Ordering::SeqCst);
        self.call_answer.store(0, Ordering::SeqCst);
        self.call_done.store(false, Ordering::SeqCst);
        self.delegation_sent.store(false, Ordering::SeqCst);
    }
}

/// **A `CALL` server told by `x4` answers its caller and keeps nothing a delegation left behind**
/// (milestone 706 (a `CALL` server can tell a Reply from a delegation), DECISIONS §245 (a `CALL`
/// server tells a Reply from a delegation), fatal risk 7's confinement claim).
///
/// A client `SEND_CAP`s an unreceived rendezvous to a `CALL` server that reads a receive as
/// `user_mode_runtime::receive_request` does (`x4 == REPLY_DELIVERED` is a Reply, any other slot a
/// delegation to delete); on both orders it must answer its caller and keep nothing. The tag is
/// asserted, not the hang, which would need milestone 417's timed receive to end.
///
/// Falsification: replayable `system_tests/falsifications/user.receive_cap_attack_tests.a_call_server_tells_its_reply_from_a_delegation_on_both_arrival_orders.patch`
#[test_case]
fn a_call_server_tells_its_reply_from_a_delegation_on_both_arrival_orders() {
    for server_first in [true, false] {
        let round = &CALL_SERVER_ROUND;
        round.reset();

        let region = crate::memory_region::create(4).expect("no region");
        let ep = sched::create_rendezvous_from(region).expect("no rendezvous");
        // The attacker's delegation: a rendezvous nobody ever receives on, so a `REPLY` (method 0,
        // `SEND` on a rendezvous) through it would never return.
        let dead_end = sched::create_rendezvous_from(region).expect("no dead-end rendezvous");
        let delegated = rendezvous_cap(dead_end, Rights::WRITE);

        let server = move || {
            round.held_before.store(held() as u64, Ordering::SeqCst);
            for _ in 0..2 {
                let [w0, x1, _w1, _badge, x4] = sched::ipc_receive_cap(ep);
                if w0 == CallServerRound::DELEGATION {
                    round.delegation_x4.store(x4, Ordering::SeqCst);
                } else {
                    round.call_x4.store(x4, Ordering::SeqCst);
                }
                round.received.fetch_add(1, Ordering::SeqCst);
                if x4 == abi::rendezvous::REPLY_DELIVERED {
                    // The tag's claim, checked against the table: the slot is a Reply.
                    if let Ok(crate::cap::Cap {
                        object: crate::cap::Object::Reply(caller),
                        ..
                    }) = sched::current_cap(x1)
                    {
                        sched::ipc_reply(caller, [CallServerRound::ANSWER, 0]);
                    }
                    let _ = sched::delete_current_cap(x1);
                } else if x1 != abi::rendezvous::NO_CAP {
                    let _ = sched::delete_current_cap(x1); // `Delivered::into_reply`'s delete
                }
            }
            round.held_after.store(held() as u64, Ordering::SeqCst);
            round.server_done.store(true, Ordering::SeqCst);
        };
        let caller = move || {
            let [r0, ..] = sched::ipc_call(ep, [CallServerRound::REQUEST, 0]);
            round.call_answer.store(r0, Ordering::SeqCst);
            round.call_done.store(true, Ordering::SeqCst);
        };
        let attacker = move || {
            round.delegation_sent.store(true, Ordering::SeqCst);
            sched::ipc_send_cap(ep, CallServerRound::DELEGATION, delegated, 0);
        };

        spawn_in_arrival_order(round, server_first, ep, server, attacker, caller);

        the_server_answered_and_kept_nothing(round, server_first);
        crate::memory_region::destroy(region);
    }
}

/// Start the server, the attacker and the caller in the order `server_first` names, waiting at
/// each step until the one before has parked where that order needs it.
fn spawn_in_arrival_order(
    round: &CallServerRound,
    server_first: bool,
    ep: sched::RendezvousId,
    server: impl FnOnce() + Send + 'static,
    attacker: impl FnOnce() + Send + 'static,
    caller: impl FnOnce() + Send + 'static,
) {
    if server_first {
        // The server parks, and each message reaches it through the sender's own delivery
        // (`ipc_send_cap`'s and `ipc_call_badged`'s rendezvous arms).
        sched::spawn(server).expect("no server thread");
        assert!(
            wait_for(|| sched::rendezvous_waiting_receivers(ep) == 1),
            "the server never parked in RECEIVE_CAP (server first)",
        );
        sched::spawn(attacker).expect("no attacker thread");
        assert!(
            wait_for(|| round.received.load(Ordering::SeqCst) == 1
                && sched::rendezvous_waiting_receivers(ep) == 1),
            "the server never took the delegation and parked again (server first)",
        );
        sched::spawn(caller).expect("no caller thread");
    } else {
        // Both clients park first, and the server collects them (`ipc_receive_cap`'s
        // `FromSender` arm), the delegation ahead of the call.
        sched::spawn(attacker).expect("no attacker thread");
        assert!(
            wait_for(|| round.delegation_sent.load(Ordering::SeqCst)
                && sched::rendezvous_waiting_senders(ep) == 1),
            "the attacker never parked in SEND_CAP (clients first)",
        );
        sched::spawn(caller).expect("no caller thread");
        assert!(
            wait_for(|| sched::rendezvous_waiting_senders(ep) == 2),
            "the caller never parked behind the attacker (clients first)",
        );
        sched::spawn(server).expect("no server thread");
    }
}

/// What the round must end with: both messages taken, the caller answered through its Reply,
/// each delivery tagged for what it was, and the server holding what it held before.
fn the_server_answered_and_kept_nothing(round: &CallServerRound, server_first: bool) {
    assert!(
        wait_for(|| round.server_done.load(Ordering::SeqCst)),
        "the server never took both messages (server_first = {server_first}); one of them \
         parked it, which is the hang 706 closes",
    );
    assert!(
        wait_for(|| round.call_done.load(Ordering::SeqCst)),
        "the caller was never answered (server_first = {server_first}): the server could not \
         tell its Reply from the delegation (x4 = {:#x} on the CALL), so it deleted the Reply",
        round.call_x4.load(Ordering::SeqCst),
    );
    assert_eq!(
        round.call_answer.load(Ordering::SeqCst),
        CallServerRound::ANSWER,
        "the caller woke with something other than the server's answer",
    );
    assert_eq!(
        round.call_x4.load(Ordering::SeqCst),
        abi::rendezvous::REPLY_DELIVERED,
        "a CALL's delivery did not carry REPLY_DELIVERED in x4 (server_first = {server_first})",
    );
    assert_eq!(
        round.delegation_x4.load(Ordering::SeqCst),
        0,
        "a SEND_CAP delegation arrived tagged as a Reply (server_first = {server_first})",
    );
    assert_eq!(
        round.held_after.load(Ordering::SeqCst),
        round.held_before.load(Ordering::SeqCst),
        "the server ended holding a slot it did not hold before (server_first = \
         {server_first}): a delivery was kept rather than answered or deleted",
    );
}

/// An interrupt number no device on any of the three machines raises, so binding it steals no real
/// route (the reasoning, and the value, are `irq_send_refusal_tests`'). `bind_irq` has no unbind,
/// so a neighbour that rebinds it simply replaces this test's route.
const QUIET_INTID: u32 = 250;

/// **An interrupt signal received by `RECEIVE_CAP` delivers `NO_CAP` in `x1` whichever side arrives
/// first** (milestone 714 (the sibling `RECEIVE_CAP` paths get a receiver-first test), fatal risk 7's
/// confinement claim; the sibling of milestone 634's plain-`SEND` test above).
///
/// `irq_notify` drops `[1, 0, 0, 0, 0]` into a parked receiver's mailbox, so without 634's
/// `cap_delivered` default a receiver-first `RECEIVE_CAP` sees `x1 = 0`: a kernel word, not a
/// chosen one, but a driver deleting `x1` as a delegation would lose its slot 0. The only test
/// driving an interrupt through that order; the signal-pending order is the control.
///
/// Falsification: replayable `system_tests/falsifications/user.receive_cap_attack_tests.an_interrupt_signal_received_by_receive_cap_delivers_no_cap_whichever_side_parks_first.patch`
#[test_case]
fn an_interrupt_signal_received_by_receive_cap_delivers_no_cap_whichever_side_parks_first() {
    static RECEIVER_X0: AtomicU64 = AtomicU64::new(u64::MAX - 1);
    static RECEIVER_X1: AtomicU64 = AtomicU64::new(u64::MAX - 1);
    static PARKING: AtomicBool = AtomicBool::new(false);
    static DONE: AtomicBool = AtomicBool::new(false);

    let region = crate::memory_region::create(4).expect("no region");
    let ep = sched::create_rendezvous_from(region).expect("no rendezvous");
    sched::bind_irq(QUIET_INTID, ep);

    // Receiver-first: the driver parks in RECEIVE_CAP, then the interrupt arrives.
    sched::spawn(move || {
        PARKING.store(true, Ordering::SeqCst);
        let [x0, x1, ..] = sched::ipc_receive_cap(ep);
        RECEIVER_X0.store(x0, Ordering::SeqCst);
        RECEIVER_X1.store(x1, Ordering::SeqCst);
        DONE.store(true, Ordering::SeqCst);
    })
    .expect("no driver thread");
    assert!(
        wait_for(|| PARKING.load(Ordering::SeqCst) && sched::rendezvous_waiting_receivers(ep) == 1),
        "the driver never parked in RECEIVE_CAP, so nothing proved the receiver-first order",
    );
    sched::irq_notify(ep);
    assert!(
        wait_for(|| DONE.load(Ordering::SeqCst)),
        "the interrupt never reached the parked RECEIVE_CAP receiver",
    );
    assert_eq!(
        RECEIVER_X0.load(Ordering::SeqCst),
        1,
        "the interrupt's w0 did not arrive"
    );
    assert_eq!(
        RECEIVER_X1.load(Ordering::SeqCst),
        abi::rendezvous::NO_CAP,
        "an interrupt signal handed the receiver x1 = {} on the receiver-first order, a slot number \
         where the message carried no capability",
        RECEIVER_X1.load(Ordering::SeqCst),
    );

    // Signal-first: the interrupt is counted, then this thread receives.
    sched::irq_notify(ep);
    let [x0, x1, ..] = sched::ipc_receive_cap(ep);
    assert_eq!(x0, 1, "the pending interrupt's w0 did not arrive");
    assert_eq!(
        x1,
        abi::rendezvous::NO_CAP,
        "an interrupt signal handed the receiver x1 = {x1} on the signal-first order",
    );

    sched::reclaim_region(region).expect("the endpoint's region did not come back");
}

/// **A death message received by `RECEIVE_CAP` delivers `NO_CAP` in `x1` whichever side arrives
/// first** (milestone 714, fatal risk 7's confinement claim).
///
/// The kernel's death message is `[event, tid, pc, addr, 0]` (`sched::depart`); a supervisor parked
/// in `RECEIVE_CAP` must not read the tid in `x1` as a slot. The word is the kernel's, so this is
/// order-dependence, not a chosen word. `x2` carries the tid on both orders (checked after `x1`, so
/// a revert is red at `x1`), so a fix that blanked the whole message would fail.
///
/// Falsification: replayable `system_tests/falsifications/user.receive_cap_attack_tests.a_death_message_received_by_receive_cap_delivers_no_cap_whichever_side_parks_first.patch`
#[test_case]
fn a_death_message_received_by_receive_cap_delivers_no_cap_whichever_side_parks_first() {
    use super::supervision_tests::{FAULT_STUB, build_child_in};

    static SUPERVISOR_PARKING: AtomicBool = AtomicBool::new(false);
    static SUPERVISOR_DONE: AtomicBool = AtomicBool::new(false);
    static SUPERVISOR_MSG: [AtomicU64; 3] = [const { AtomicU64::new(u64::MAX - 1) }; 3];

    // Receiver-first: the supervisor parks in RECEIVE_CAP, then the child dies.
    let endpoints = crate::memory_region::create(2).expect("no endpoint region");
    let fault_ep = sched::create_rendezvous_from(endpoints).expect("no fault rendezvous");
    let region = crate::memory_region::create(16).expect("no region for the child");
    sched::spawn(move || {
        SUPERVISOR_PARKING.store(true, Ordering::SeqCst);
        let [event, x1, x2, ..] = sched::ipc_receive_cap(fault_ep);
        SUPERVISOR_MSG[0].store(event, Ordering::SeqCst);
        SUPERVISOR_MSG[1].store(x1, Ordering::SeqCst);
        SUPERVISOR_MSG[2].store(x2, Ordering::SeqCst);
        SUPERVISOR_DONE.store(true, Ordering::SeqCst);
    })
    .expect("no supervisor thread");
    assert!(
        wait_for(|| SUPERVISOR_PARKING.load(Ordering::SeqCst)
            && sched::rendezvous_waiting_receivers(fault_ep) == 1),
        "the supervisor never parked in RECEIVE_CAP, so nothing proved the receiver-first order",
    );
    let child = build_child_in(region, FAULT_STUB, None, Some(fault_ep));
    assert!(
        wait_for(|| SUPERVISOR_DONE.load(Ordering::SeqCst)),
        "the death message never reached the parked RECEIVE_CAP supervisor",
    );
    assert_eq!(
        SUPERVISOR_MSG[0].load(Ordering::SeqCst),
        abi::fault::EVENT_FAULT,
        "the message that arrived was not the child's fault"
    );
    assert_eq!(
        SUPERVISOR_MSG[1].load(Ordering::SeqCst),
        abi::rendezvous::NO_CAP,
        "a death message handed the supervisor x1 = {} on the receiver-first order, the dead \
         thread's id where the message carried no capability",
        SUPERVISOR_MSG[1].load(Ordering::SeqCst),
    );
    assert_eq!(
        SUPERVISOR_MSG[2].load(Ordering::SeqCst),
        child,
        "the death message's thread id (RECEIVE_CAP's x2) did not arrive on the receiver-first order",
    );
    assert!(
        wait_for(|| sched::reclaim_region(region).is_ok()),
        "reaping the first corpse's region failed",
    );

    // Corpse-first: the child dies and parks on the supervision rendezvous, then this thread receives.
    let fault_ep = sched::create_rendezvous_from(endpoints).expect("no fault rendezvous");
    let region = crate::memory_region::create(16).expect("no region for the second child");
    let child = build_child_in(region, FAULT_STUB, None, Some(fault_ep));
    assert!(
        wait_for(|| sched::rendezvous_waiting_senders(fault_ep) == 1),
        "the second child never died and parked its message, so nothing proved the corpse-first order",
    );
    let [event, x1, x2, ..] = sched::ipc_receive_cap(fault_ep);
    assert_eq!(event, abi::fault::EVENT_FAULT, "not the child's fault");
    assert_eq!(
        x1,
        abi::rendezvous::NO_CAP,
        "a death message handed the supervisor x1 = {x1} on the corpse-first order",
    );
    assert_eq!(x2, child, "the death message's thread id did not arrive");
    assert!(
        wait_for(|| sched::reclaim_region(region).is_ok()),
        "reaping the second corpse's region failed",
    );
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// The five words a plain `RECEIVE` returned, and how many slots its thread held before and after.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct PlainReceipt {
    words: [u64; 5],
    held_before: usize,
    held_after: usize,
}

/// **Run one plain `RECEIVE` against one sender, in the order asked for**, and report what the
/// receiver saw. `send` is the sender's whole body; it runs on a thread of its own either way, so a
/// sender the kernel never releases is a timeout here rather than a hung test thread.
///
/// Receiver-first: a spawned receiver parks, then the sender runs and meets it (the sender's own
/// rendezvous arm, `ipc_send_cap`'s or `ipc_call_badged`'s). Sender-first: the sender parks, then
/// this thread receives and collects it (`ipc_receive`'s `FromSender` arm).
fn plain_receive_against(
    ep: sched::RendezvousId,
    receiver_first: bool,
    send: fn(sched::RendezvousId),
) -> PlainReceipt {
    static WORDS: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];
    static HELD_BEFORE: AtomicU64 = AtomicU64::new(0);
    static HELD_AFTER: AtomicU64 = AtomicU64::new(0);
    static PARKING: AtomicBool = AtomicBool::new(false);
    static RECEIVED: AtomicBool = AtomicBool::new(false);
    static SENDING: AtomicBool = AtomicBool::new(false);
    PARKING.store(false, Ordering::SeqCst);
    RECEIVED.store(false, Ordering::SeqCst);
    SENDING.store(false, Ordering::SeqCst);

    fn receive(ep: sched::RendezvousId) {
        HELD_BEFORE.store(held() as u64, Ordering::SeqCst);
        PARKING.store(true, Ordering::SeqCst);
        let words = sched::ipc_receive(ep);
        for (w, v) in WORDS.iter().zip(words) {
            w.store(v, Ordering::SeqCst);
        }
        HELD_AFTER.store(held() as u64, Ordering::SeqCst);
        RECEIVED.store(true, Ordering::SeqCst);
    }
    let sender = move || {
        SENDING.store(true, Ordering::SeqCst);
        send(ep);
    };

    if receiver_first {
        sched::spawn(move || receive(ep)).expect("no receiver thread");
        assert!(
            wait_for(
                || PARKING.load(Ordering::SeqCst) && sched::rendezvous_waiting_receivers(ep) == 1
            ),
            "the receiver never parked in plain RECEIVE, so nothing proved the receiver-first order",
        );
        sched::spawn(sender).expect("no sender thread");
    } else {
        sched::spawn(sender).expect("no sender thread");
        assert!(
            wait_for(
                || SENDING.load(Ordering::SeqCst) && sched::rendezvous_waiting_senders(ep) == 1
            ),
            "the sender never parked, so nothing proved the sender-first order",
        );
        receive(ep);
    }
    assert!(
        wait_for(|| RECEIVED.load(Ordering::SeqCst)),
        "the plain RECEIVE never returned (receiver_first = {receiver_first})",
    );
    PlainReceipt {
        words: core::array::from_fn(|i| WORDS[i].load(Ordering::SeqCst)),
        held_before: HELD_BEFORE.load(Ordering::SeqCst) as usize,
        held_after: HELD_AFTER.load(Ordering::SeqCst) as usize,
    }
}

/// **A `SEND_CAP` reaching a plain `RECEIVE` delivers its data word and no capability, whichever
/// side parks first** (§246 (a plain `RECEIVE` never takes a capability), PROVISIONAL number;
/// calef's ruling A of 2026-10-04 UTC, fatal risk 7).
///
/// Both orders must return the same five words, and the receiver must end holding exactly what it
/// held: a server that drains output with plain `RECEIVE` and never reads `x1` (swish, `wc`, `mdr`,
/// the terminal sink) would otherwise lose one of its 32 slots per delegation, for good.
///
/// Falsification: replayable `system_tests/falsifications/user.receive_cap_attack_tests.a_send_cap_to_a_plain_receive_installs_nothing_whichever_side_parks_first.patch`
#[test_case]
fn a_send_cap_to_a_plain_receive_installs_nothing_whichever_side_parks_first() {
    const DATA: u64 = 0x246;
    let mut seen = [None; 2];
    for (i, receiver_first) in [true, false].into_iter().enumerate() {
        let region = crate::memory_region::create(4).expect("no region");
        let ep = sched::create_rendezvous_from(region).expect("no rendezvous");
        static TARGET: AtomicU64 = AtomicU64::new(0);
        let target = sched::create_rendezvous_from(region).expect("no delegated rendezvous");
        TARGET.store(target, Ordering::SeqCst);
        let got = plain_receive_against(ep, receiver_first, |ep| {
            let target = TARGET.load(Ordering::SeqCst);
            sched::ipc_send_cap(ep, DATA, rendezvous_cap(target, Rights::WRITE), 0);
        });
        assert_eq!(
            got.words[0], DATA,
            "the SEND_CAP's data word did not arrive (receiver_first = {receiver_first})",
        );
        assert_eq!(
            got.held_after, got.held_before,
            "a plain RECEIVE ended holding {} slots, having held {} (receiver_first = \
             {receiver_first}): the delegation was installed in a table that never asked for it",
            got.held_after, got.held_before,
        );
        seen[i] = Some(got.words);
        crate::memory_region::destroy(region);
    }
    assert_eq!(
        seen[0], seen[1],
        "the two arrival orders returned different words to a plain RECEIVE (receiver first, then \
         sender first): x1 is a slot on one order and not the other",
    );
}

/// **A `CALL` reaching a plain `RECEIVE` is answered `Gone`, whichever side parks first** (§246,
/// PROVISIONAL number; calef's ruling A of 2026-10-04 UTC).
///
/// A plain `RECEIVE` has nowhere to put the one-shot Reply, so it cannot answer. Before the ruling,
/// receiver-first installed the Reply in the receiver's table anyway (a slot no plain receiver
/// reads), and sender-first delivered the words and left the caller parked on a Reply that existed
/// nowhere, for the life of the rendezvous. Both orders now deliver the request's words, keep the
/// receiver's table as it was, and release the caller with `Gone`.
///
/// Falsification: replayable `system_tests/falsifications/user.receive_cap_attack_tests.a_call_to_a_plain_receive_is_answered_gone_whichever_side_parks_first.patch`
#[test_case]
fn a_call_to_a_plain_receive_is_answered_gone_whichever_side_parks_first() {
    const REQUEST: u64 = 0x2460;
    const SECOND: u64 = 0x2461;
    static CALLER_DONE: AtomicBool = AtomicBool::new(false);
    static CALLER_GONE: AtomicBool = AtomicBool::new(false);
    let mut seen = [None; 2];
    for (i, receiver_first) in [true, false].into_iter().enumerate() {
        CALLER_DONE.store(false, Ordering::SeqCst);
        CALLER_GONE.store(false, Ordering::SeqCst);
        let region = crate::memory_region::create(4).expect("no region");
        let ep = sched::create_rendezvous_from(region).expect("no rendezvous");
        let got = plain_receive_against(ep, receiver_first, |ep| {
            let _ = sched::ipc_call(ep, [REQUEST, SECOND]);
            let aborted = sched::take_ipc_aborted();
            let refused = aborted && sched::take_ipc_refused();
            CALLER_GONE.store(aborted && !refused, Ordering::SeqCst);
            CALLER_DONE.store(true, Ordering::SeqCst);
        });
        assert_eq!(
            got.words[0], REQUEST,
            "the CALL's request did not arrive (receiver_first = {receiver_first})",
        );
        assert!(
            wait_for(|| CALLER_DONE.load(Ordering::SeqCst)),
            "the caller was never released (receiver_first = {receiver_first}): it waits on a \
             Reply no plain RECEIVE can send",
        );
        assert!(
            CALLER_GONE.load(Ordering::SeqCst),
            "the caller returned without Gone (receiver_first = {receiver_first})",
        );
        assert_eq!(
            got.held_after, got.held_before,
            "a plain RECEIVE ended holding {} slots, having held {} (receiver_first = \
             {receiver_first}): the caller's Reply was installed in a table that never asked for it",
            got.held_after, got.held_before,
        );
        seen[i] = Some(got.words);
        crate::memory_region::destroy(region);
    }
    assert_eq!(
        seen[0], seen[1],
        "the two arrival orders returned different words to a plain RECEIVE for a CALL",
    );
}
