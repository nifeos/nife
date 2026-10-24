//! Tests for rendezvous: blocking send and receive, call and reply, the reply capability, and what
//! a revoked or reclaimed rendezvous does to the threads parked on it. See
//! notes/scheduler/rendezvous.md.
//!
//! Name: provisional (milestone 840's lane, 2026-10-11 UTC).

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::sched::tests::{spin_until, wait_for};

/// **A thread blocked on a rendezvous wakes with an error when the rendezvous is revoked.**
/// Revocation drains the wait queue, marks each waiter aborted and wakes it: the reclaim succeeds,
/// and the woken thread's blocking IPC reports the rendezvous gone (`take_ipc_aborted`) instead of
/// returning a message it never received.
#[test_case]
fn a_blocked_waiter_wakes_with_an_error_when_its_rendezvous_is_revoked() {
    static ABORTED: AtomicBool = AtomicBool::new(false);
    static WOKE: AtomicBool = AtomicBool::new(false);
    ABORTED.store(false, Ordering::SeqCst);
    WOKE.store(false, Ordering::SeqCst);

    let region = crate::memory_region::create(2).expect("region");
    let ep = crate::sched::create_rendezvous_from(region).expect("rendezvous from region");

    // A thread that blocks receiving on the rendezvous, then records whether it was aborted.
    crate::sched::spawn(move || {
        let _ = crate::sched::ipc_receive(ep);
        ABORTED.store(crate::sched::take_ipc_aborted(), Ordering::SeqCst);
        WOKE.store(true, Ordering::SeqCst);
    })
    .expect("spawn a waiter");

    // The waiter must be queued on the rendezvous before the reclaim, or there is nothing to
    // wake. Waited on, not yielded for: the waiter is on another core.
    assert!(
        wait_for(|| crate::sched::rendezvous_waiting_receivers(ep) == 1),
        "the waiter never blocked on the rendezvous, so the reclaim had nothing to wake",
    );

    // Reclaiming the rendezvous's region now succeeds: the waiter is woken with an error, not left
    // to strand the reclaim.
    crate::sched::reclaim_region(region)
        .expect("reclaim wakes the blocked waiter rather than refusing");

    // Clock-bounded, not yield-bounded: see `wait_for`. Since §28 the waiter is on another
    // core, so this core's fifty yields can elapse before it has been scheduled at all.
    assert!(
        wait_for(|| WOKE.load(Ordering::SeqCst)),
        "the revoked waiter never woke"
    );
    assert!(
        ABORTED.load(Ordering::SeqCst),
        "the woken waiter did not see its IPC aborted",
    );
}

/// How many threads are parked awaiting a reply through `ep`. The rendezvous itself cannot say: a
/// `CALL` caller whose request was taken is linked on no queue, so only `wait_on` records it.
fn reply_parked_callers(ep: super::RendezvousId) -> usize {
    let guard = super::IPC_TABLES.lock();
    let Some(sched) = guard.as_ref() else {
        return 0;
    };
    sched
        .threads
        .iter_from(0)
        .filter(|(_, t)| {
            matches!(t.handshake.wait_on, Some(super::Wait::Rendezvous(on, super::WaitRole::Reply)) if on == ep)
        })
        .count()
}

/// How many live `Reply` capabilities anywhere in the machine still name `caller`: the sweep's
/// own assertion. `outgoing_cap` counts too, because a caller that met no
/// server rides its own reply capability there awaiting a `RECEIVE_CAP` hand-off, and a live one
/// left in that slot is the same forgery one step earlier.
fn outstanding_reply_capabilities(caller: super::ThreadId) -> usize {
    let guard = super::IPC_TABLES.lock();
    let Some(sched) = guard.as_ref() else {
        return 0;
    };
    let target = crate::cap::Object::Reply(caller);
    let mut found = 0;
    for (_, t) in sched.threads.iter_from(0) {
        let table = sched.threads.capabilities(t.id).unwrap().lock();
        for slot in 0..table.len() as u64 {
            if table.get(slot).is_ok_and(|c| c.object == target) {
                found += 1;
            }
        }
        drop(table);
        if matches!(t.outgoing_cap, Some(c) if c.object == target) {
            found += 1;
        }
    }
    found
}

/// **A caller whose rendezvous is torn down mid-`CALL` returns `Gone` rather than blocking for
/// the life of the machine**, and the stale reply capability the server held is gone before the
/// caller can run again.
///
/// The server has collected the request, so the caller is on no wait queue; that one collected
/// message is the difference from the revoked-waiter test. The second assertion matters most: a
/// woken caller with a live Reply still naming it could be answered in its next conversation.
/// `current_cap` is the lookup `abi::reply::REPLY` makes, so a capability gone here cannot reply.
#[test_case]
fn a_reply_parked_caller_wakes_with_an_error_when_its_rendezvous_is_reclaimed() {
    static COLLECTED: AtomicBool = AtomicBool::new(false);
    static RELEASE: AtomicBool = AtomicBool::new(false);
    static HELD_BEFORE: AtomicBool = AtomicBool::new(false);
    static HELD_AFTER: AtomicBool = AtomicBool::new(true);
    static CHECKED: AtomicBool = AtomicBool::new(false);
    static WOKE: AtomicBool = AtomicBool::new(false);
    static ABORTED: AtomicBool = AtomicBool::new(false);
    static CALLER: AtomicU64 = AtomicU64::new(0);

    let region = crate::memory_region::create(2).expect("region");
    let ep = crate::sched::create_rendezvous_from(region).expect("rendezvous from region");

    // The server: collect the request, keep the one-shot Reply, and never answer. It stays
    // alive on purpose, so the only thing that can free the caller is the rendezvous going
    // away; a server that returned here would depart, which is the *other* trigger and the
    // next test's subject.
    crate::sched::spawn(move || {
        let m = crate::sched::ipc_receive_cap(ep);
        let slot = m[1];
        HELD_BEFORE.store(crate::sched::current_cap(slot).is_ok(), Ordering::SeqCst);
        COLLECTED.store(true, Ordering::SeqCst);
        while !RELEASE.load(Ordering::SeqCst) {
            crate::sched::yield_now();
        }
        HELD_AFTER.store(crate::sched::current_cap(slot).is_ok(), Ordering::SeqCst);
        CHECKED.store(true, Ordering::SeqCst);
    })
    .expect("spawn a server");

    crate::sched::spawn(move || {
        CALLER.store(super::current_thread_id(), Ordering::SeqCst);
        let _ = crate::sched::ipc_call(ep, [1, 2]);
        ABORTED.store(crate::sched::take_ipc_aborted(), Ordering::SeqCst);
        WOKE.store(true, Ordering::SeqCst);
    })
    .expect("spawn a caller");

    // Clock-bounded, never yield-counted: since DECISIONS §28 both threads are on other cores.
    // Both conditions matter. `COLLECTED` says the request was *taken*, which is what puts the
    // caller off every wait queue and is the whole premise; `reply_parked_callers` says it is
    // parked awaiting the reply that will never come.
    assert!(
        wait_for(|| COLLECTED.load(Ordering::SeqCst) && reply_parked_callers(ep) == 1),
        "the server never collected a request from a reply-parked caller",
    );
    assert!(
        HELD_BEFORE.load(Ordering::SeqCst),
        "the server was never handed a reply capability, so this test proves nothing",
    );

    crate::sched::reclaim_region(region)
        .expect("reclaim frees the reply-parked caller rather than refusing");

    assert!(
        wait_for(|| WOKE.load(Ordering::SeqCst)),
        "the stranded caller never woke: this is the defect milestone 254 fixes",
    );
    assert!(
        ABORTED.load(Ordering::SeqCst),
        "the freed caller did not see its CALL aborted, so it returns a reply it never got",
    );
    assert_eq!(
        outstanding_reply_capabilities(CALLER.load(Ordering::SeqCst)),
        0,
        "a Reply capability still names the freed caller: the next CALL can be forged",
    );

    RELEASE.store(true, Ordering::SeqCst);
    assert!(
        wait_for(|| CHECKED.load(Ordering::SeqCst)),
        "the server never re-checked its reply capability",
    );
    assert!(
        !HELD_AFTER.load(Ordering::SeqCst),
        "the server kept a live reply capability naming a caller that has moved on",
    );
}

/// **A server that dies frees the callers it will never answer**, which is
/// QNX Neutrino's headline behaviour: *"if the server thread fails, exits, or disappears, the
/// client thread becomes READY, with `MsgSend()` indicating an error"*. The trigger here is
/// `depart` rather than the rendezvous teardown above, and the rendezvous outlives the server,
/// so nothing else in the kernel is even looking at the caller.
///
/// The reply capability dies with the table it lived in, so the sweep has nothing left to
/// delete by the time the thread is gone; `outstanding_reply_capabilities` asserts that
/// directly rather than assuming it, because the ordering (sweep, then wake) is what makes the
/// claim true and an ordering is exactly the sort of thing a refactor loses.
#[test_case]
fn a_server_that_exits_frees_the_caller_it_never_answered() {
    static COLLECTED: AtomicBool = AtomicBool::new(false);
    static HELD: AtomicBool = AtomicBool::new(false);
    static WOKE: AtomicBool = AtomicBool::new(false);
    static ABORTED: AtomicBool = AtomicBool::new(false);
    static CALLER: AtomicU64 = AtomicU64::new(0);

    let region = crate::memory_region::create(2).expect("region");
    let ep = crate::sched::create_rendezvous_from(region).expect("rendezvous from region");

    // Collect the request, confirm the reply capability is live, and then simply return: the
    // server exits holding it, which is the case the survey found this kernel alone in
    // stranding.
    crate::sched::spawn(move || {
        let m = crate::sched::ipc_receive_cap(ep);
        HELD.store(crate::sched::current_cap(m[1]).is_ok(), Ordering::SeqCst);
        COLLECTED.store(true, Ordering::SeqCst);
    })
    .expect("spawn a server");

    crate::sched::spawn(move || {
        CALLER.store(super::current_thread_id(), Ordering::SeqCst);
        let _ = crate::sched::ipc_call(ep, [3, 4]);
        ABORTED.store(crate::sched::take_ipc_aborted(), Ordering::SeqCst);
        WOKE.store(true, Ordering::SeqCst);
    })
    .expect("spawn a caller");

    assert!(
        wait_for(|| COLLECTED.load(Ordering::SeqCst)),
        "the server never collected the request",
    );
    assert!(
        HELD.load(Ordering::SeqCst),
        "the server was never handed a reply capability, so this test proves nothing",
    );
    assert!(
        wait_for(|| WOKE.load(Ordering::SeqCst)),
        "the caller was still parked after its server exited",
    );
    assert!(
        ABORTED.load(Ordering::SeqCst),
        "the freed caller did not see its CALL aborted",
    );
    assert_eq!(
        outstanding_reply_capabilities(CALLER.load(Ordering::SeqCst)),
        0,
        "a Reply capability outlived the caller's CALL",
    );

    // The rendezvous outlived its server, which is the point of this case; put its region back.
    assert!(
        wait_for(|| crate::sched::reclaim_region(region).is_ok()),
        "the rendezvous's region never came back",
    );
}

/// **A wake with nothing delivered must not complete a parked receiver's `RECEIVE`.** A `Blocked`
/// IPC thread becomes `Ready` only by the hand that completed its rendezvous (message staged,
/// signal counted, or abort flagged). An undelivered wake is refused, the receiver stays parked
/// and linked on the wait queue, and a real sender still reaches it afterwards. See
/// notes/scheduler/undelivered-wake-gate.md.
#[test_case]
fn a_wake_without_delivery_cannot_complete_a_parked_receive() {
    static GOT: AtomicU64 = AtomicU64::new(u64::MAX);
    static DONE: AtomicBool = AtomicBool::new(false);
    GOT.store(u64::MAX, Ordering::SeqCst);
    DONE.store(false, Ordering::SeqCst);

    let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
    let ep = crate::sched::create_rendezvous_from(region).expect("no rendezvous from region");
    let tid = crate::sched::spawn(move || {
        let m = crate::sched::ipc_receive(ep);
        GOT.store(m[0], Ordering::SeqCst);
        DONE.store(true, Ordering::SeqCst);
    })
    .expect("spawn receiver");

    // Queued on the rendezvous, not "probably scheduled by now".
    assert!(
        wait_for(|| crate::sched::rendezvous_waiting_receivers(ep) == 1),
        "the receiver never parked on the rendezvous"
    );

    // The injection: the real wake path, nothing delivered.
    crate::sched::wake_without_delivery(tid);

    // The receiver must stay parked. Held for half a second of yields rather than one look,
    // because the spurious completion needs the receiver to be scheduled first.
    let deadline = crate::arch::timer::now() + crate::arch::timer::frequency() / 2;
    while crate::arch::timer::now() < deadline {
        assert!(
            !DONE.load(Ordering::SeqCst),
            "a wake with nothing delivered completed the receive (it returned {:#x})",
            GOT.load(Ordering::SeqCst),
        );
        crate::sched::yield_now();
    }
    assert_eq!(
        crate::sched::rendezvous_waiting_receivers(ep),
        1,
        "the undelivered wake took the receiver off the rendezvous"
    );

    // And the rendezvous still works: a real sender completes the same receive with its message.
    crate::sched::ipc_send(ep, [81, 0, 0]);
    assert!(
        wait_for(|| DONE.load(Ordering::SeqCst)),
        "the real message never arrived after the refused wake"
    );
    assert_eq!(
        GOT.load(Ordering::SeqCst),
        81,
        "the receive completed with something other than the real message"
    );
    assert!(
        wait_for(|| !crate::sched::is_thread_present(tid)),
        "the receiver never exited",
    );
    crate::sched::reclaim_region(region).expect("test region would not reclaim");
}

/// **A reply only wakes a caller that awaits one.** A `Reply` capability names a tid, not a wait
/// state; delivered to a thread parked as an ordinary receiver, it would clobber the mailbox and
/// wake the thread messageless while its TCB is still linked on the wait queue. The test above's
/// strand, through the one wake site addressed by tid.
#[test_case]
fn a_reply_to_a_thread_parked_as_a_receiver_is_dropped() {
    static GOT: AtomicU64 = AtomicU64::new(u64::MAX);
    static DONE: AtomicBool = AtomicBool::new(false);
    GOT.store(u64::MAX, Ordering::SeqCst);
    DONE.store(false, Ordering::SeqCst);

    let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
    let ep = crate::sched::create_rendezvous_from(region).expect("no rendezvous from region");
    let tid = crate::sched::spawn(move || {
        let m = crate::sched::ipc_receive(ep);
        GOT.store(m[0], Ordering::SeqCst);
        DONE.store(true, Ordering::SeqCst);
    })
    .expect("spawn receiver");

    assert!(
        wait_for(|| crate::sched::rendezvous_waiting_receivers(ep) == 1),
        "the receiver never parked on the rendezvous"
    );

    // A reply aimed at a thread that is not awaiting a reply: dropped, like a reply to a
    // dead caller.
    crate::sched::ipc_reply(tid, [0xDEAD, 0]);

    let deadline = crate::arch::timer::now() + crate::arch::timer::frequency() / 2;
    while crate::arch::timer::now() < deadline {
        assert!(
            !DONE.load(Ordering::SeqCst),
            "a stray reply completed a receiver's receive (it returned {:#x})",
            GOT.load(Ordering::SeqCst),
        );
        crate::sched::yield_now();
    }

    // The mailbox was not clobbered and the rendezvous still works.
    crate::sched::ipc_send(ep, [81, 0, 0]);
    assert!(
        wait_for(|| DONE.load(Ordering::SeqCst)),
        "the real message never arrived after the dropped reply"
    );
    assert_eq!(
        GOT.load(Ordering::SeqCst),
        81,
        "the receive completed with the stray reply's words, not the real message"
    );
    assert!(
        wait_for(|| !crate::sched::is_thread_present(tid)),
        "the receiver never exited",
    );
    crate::sched::reclaim_region(region).expect("test region would not reclaim");
}

/// **The rendezvous, receiver-first.** A thread blocks on an empty rendezvous, and stays
/// blocked, and a *later* sender is what frees it: carrying the message.
#[test_case]
fn a_receiver_blocks_until_a_sender_arrives() {
    static GOT: AtomicU64 = AtomicU64::new(0);
    static RECEIVED: AtomicBool = AtomicBool::new(false);

    let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
    let ep = super::create_rendezvous_from(region).expect("no rendezvous from region");

    let tid = super::spawn(move || {
        let msg = super::ipc_receive(ep); // nobody is sending yet: this BLOCKS
        GOT.store(msg[0], Ordering::SeqCst);
        RECEIVED.store(true, Ordering::SeqCst);
    })
    .expect("spawn failed");

    // Let the receiver run and block. It must NOT have received anything: there is no sender.
    for _ in 0..50 {
        super::yield_now();
    }
    assert!(
        !RECEIVED.load(Ordering::SeqCst),
        "a receiver returned from an rendezvous nobody had sent to",
    );

    // Now send. This should hand the receiver its message and wake it.
    super::ipc_send(ep, [0xABCD, 0, 0]);

    // Clock-bounded, not yield-bounded: see `wait_for`.
    assert!(
        wait_for(|| RECEIVED.load(Ordering::SeqCst)),
        "the receiver never woke"
    );
    assert_eq!(
        GOT.load(Ordering::SeqCst),
        0xABCD,
        "wrong message delivered"
    );
    assert!(
        wait_for(|| !crate::sched::is_thread_present(tid)),
        "the receiver never exited",
    );
    crate::sched::reclaim_region(region).expect("test region would not reclaim");
}

/// **The rendezvous, sender-first.** The other order: a sender blocks on an rendezvous with no
/// receiver, and a later receiver collects the parked message and wakes it.
#[test_case]
fn a_sender_blocks_until_a_receiver_arrives() {
    static SENT_RETURNED: AtomicBool = AtomicBool::new(false);

    let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
    let ep = super::create_rendezvous_from(region).expect("no rendezvous from region");

    let tid = super::spawn(move || {
        super::ipc_send(ep, [0x1234, 0x5678, 0x9abc]); // nobody receiving yet: BLOCKS
        SENT_RETURNED.store(true, Ordering::SeqCst);
    })
    .expect("spawn failed");

    for _ in 0..50 {
        super::yield_now();
    }
    assert!(
        !SENT_RETURNED.load(Ordering::SeqCst),
        "a send returned before anyone received it",
    );

    let msg = super::ipc_receive(ep); // collects the parked message, wakes the sender
    // Five words now (the top two are the fault path's, DECISIONS §26); an ordinary send fills
    // the first three and leaves the rest zero.
    assert_eq!(
        msg,
        [0x1234, 0x5678, 0x9abc, 0, 0],
        "wrong message received"
    );

    // Clock-bounded, not yield-bounded: see `wait_for`.
    assert!(
        wait_for(|| SENT_RETURNED.load(Ordering::SeqCst)),
        "the sender never woke after its message was taken",
    );
    assert!(
        wait_for(|| !crate::sched::is_thread_present(tid)),
        "the sender never exited",
    );
    crate::sched::reclaim_region(region).expect("test region would not reclaim");
}

/// **A request and a reply, over two endpoints.** The shape milestone 8's console server
/// will have: a client sends a request and blocks for the answer; a server loops on the
/// request rendezvous, does the work, and replies on the reply rendezvous.
///
/// All three message words survive the round trip, which is what proves the receiver's
/// `x1`/`x2` handling and the mailbox are correct end to end.
#[test_case]
fn a_request_gets_a_reply() {
    static ANSWER: AtomicU64 = AtomicU64::new(0);
    static DONE: AtomicBool = AtomicBool::new(false);

    let region = crate::memory_region::create(2).expect("no region for two test rendezvous");
    let req = super::create_rendezvous_from(region).expect("no rendezvous from region");
    let rep = super::create_rendezvous_from(region).expect("no rendezvous from region");

    // The server: receive n on `req`, send n + 1 back on `rep`.
    let server = super::spawn(move || {
        let m = super::ipc_receive(req);
        super::ipc_send(rep, [m[0] + 1, m[1], m[2]]);
    })
    .expect("spawn failed");

    // The client.
    let client = super::spawn(move || {
        super::ipc_send(req, [41, 0, 0]);
        let answer = super::ipc_receive(rep);
        ANSWER.store(answer[0], Ordering::SeqCst);
        DONE.store(true, Ordering::SeqCst);
    })
    .expect("spawn failed");

    assert!(
        spin_until(|| DONE.load(Ordering::SeqCst)),
        "the request/reply never completed"
    );
    assert_eq!(
        ANSWER.load(Ordering::SeqCst),
        42,
        "the server computed the wrong answer"
    );
    assert!(
        wait_for(
            || !crate::sched::is_thread_present(server) && !crate::sched::is_thread_present(client)
        ),
        "the request/reply test's own threads had not finished when it returned",
    );
    crate::sched::reclaim_region(region).expect("test region would not reclaim");
}

/// **Milestone 19a: an rendezvous retyped from a region carries IPC, and pins its region.**
/// The kernel-level half of the granular-construction story: `create_rendezvous_from` carves a
/// page, the rendezvous lives in it, rendezvous works over it exactly as over a kernel-wired
/// rendezvous, and `memory_region::destroy` refuses the now-pinned region, because freeing the page
/// under a live rendezvous would dangle every queued thread. The refusal is measured, not
/// assumed: the allocator's free count must not move.
#[test_case]
fn a_retyped_rendezvous_carries_ipc_and_pins_its_region() {
    use core::sync::atomic::{AtomicU64, Ordering};
    static GOT: AtomicU64 = AtomicU64::new(0);

    let region = crate::memory_region::create(2).expect("no region");
    let ep = super::create_rendezvous_from(region).expect("no rendezvous from region");
    let kernel_ep = super::create_rendezvous();
    assert_ne!(ep, kernel_ep, "registry names collide");

    super::spawn(move || {
        GOT.store(super::ipc_receive(ep)[0], Ordering::SeqCst);
    })
    .expect("spawn failed");
    super::ipc_send(ep, [0x2A, 0, 0]);
    spin_until(|| GOT.load(Ordering::SeqCst) != 0);
    assert_eq!(
        GOT.load(Ordering::SeqCst),
        0x2A,
        "no rendezvous over the retyped rendezvous"
    );

    let free_before = crate::memory::stats().unwrap().free();
    crate::memory_region::destroy(region);
    assert_eq!(
        crate::memory::stats().unwrap().free(),
        free_before,
        "destroy reclaimed a pinned region hosting a live rendezvous",
    );
}

/// **Milestone 12: a call gets a reply, over one rendezvous, via a one-shot Reply cap.**
///
/// The client `CALL`s and blocks; the server `RECEIVE_CAP`s (receiving the request word plus a
/// kernel-minted `Reply` cap naming the caller), answers through that cap, and consumes it. One
/// rendezvous, not the two the pre-`Call` pattern needs, and the server was never wired to this
/// client.
#[test_case]
fn a_call_gets_a_reply() {
    static ANSWER: AtomicU64 = AtomicU64::new(0);
    static DONE: AtomicBool = AtomicBool::new(false);

    let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
    let ep = super::create_rendezvous_from(region).expect("no rendezvous from region");

    let server = super::spawn(move || {
        let m = super::ipc_receive_cap(ep); // [n, reply_slot, second_word]
        let slot = m[1];
        let crate::cap::Object::Reply(caller) = super::current_cap(slot).unwrap().object else {
            panic!("RECEIVE_CAP of a CALL did not deliver a Reply capability");
        };
        super::ipc_reply(caller, [m[0] + 1, 0]);
        super::delete_current_cap(slot).expect("consume the one-shot reply");
    })
    .expect("spawn failed");

    let client = super::spawn(move || {
        let r = super::ipc_call(ep, [41, 0]);
        ANSWER.store(r[0], Ordering::SeqCst);
        DONE.store(true, Ordering::SeqCst);
    })
    .expect("spawn failed");

    assert!(
        spin_until(|| DONE.load(Ordering::SeqCst)),
        "the call never returned"
    );
    assert_eq!(ANSWER.load(Ordering::SeqCst), 42, "wrong reply");
    assert!(
        wait_for(
            || !crate::sched::is_thread_present(server) && !crate::sched::is_thread_present(client)
        ),
        "the call/reply test's own threads had not finished when it returned",
    );
    crate::sched::reclaim_region(region).expect("test region would not reclaim");
}

/// **Milestone 12: a reply reaches the caller that called, not another.**
///
/// Two clients call and block at once; the server answers each through *its* Reply cap. Client A
/// (sent 100) must get 111 and client B (sent 200) must get 211. A shared reply rendezvous cannot
/// guarantee this: whichever client's `RECEIVE` runs grabs the reply. The Reply cap, naming the
/// specific blocked caller, makes misrouting unrepresentable.
#[test_case]
fn a_reply_reaches_the_caller_that_called() {
    static GOT_A: AtomicU64 = AtomicU64::new(0);
    static GOT_B: AtomicU64 = AtomicU64::new(0);

    let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
    let ep = super::create_rendezvous_from(region).expect("no rendezvous from region");

    // The server: field two calls, reply each caller its own word + 11, via its own cap.
    let server = super::spawn(move || {
        for _ in 0..2 {
            let m = super::ipc_receive_cap(ep);
            let (word, slot) = (m[0], m[1]);
            let crate::cap::Object::Reply(caller) = super::current_cap(slot).unwrap().object else {
                panic!("not a reply cap");
            };
            super::ipc_reply(caller, [word + 11, 0]);
            super::delete_current_cap(slot).unwrap();
        }
    })
    .expect("spawn failed");

    let client_a = super::spawn(move || {
        let r = super::ipc_call(ep, [100, 0]);
        GOT_A.store(r[0], Ordering::SeqCst);
    })
    .expect("spawn failed");
    let client_b = super::spawn(move || {
        let r = super::ipc_call(ep, [200, 0]);
        GOT_B.store(r[0], Ordering::SeqCst);
    })
    .expect("spawn failed");

    spin_until(|| GOT_A.load(Ordering::SeqCst) != 0 && GOT_B.load(Ordering::SeqCst) != 0);
    assert_eq!(
        GOT_A.load(Ordering::SeqCst),
        111,
        "client A got the wrong caller's reply"
    );
    assert_eq!(
        GOT_B.load(Ordering::SeqCst),
        211,
        "client B got the wrong caller's reply"
    );
    assert!(
        wait_for(|| !crate::sched::is_thread_present(server)
            && !crate::sched::is_thread_present(client_a)
            && !crate::sched::is_thread_present(client_b)),
        "this test's own threads had not finished when it returned",
    );
    crate::sched::reclaim_region(region).expect("test region would not reclaim");
}

/// The kernel's rendezvous supply grows past one chunk, and a retired chunk's endpoints keep working.
///
/// `KERNEL_EP_CHUNK_PAGES + 1` endpoints cross a chunk boundary wherever the current chunk starts,
/// so the carve path is exercised. The second assertion matters more: a rendezvous minted before
/// the transition still resolves, so forgetting a filled chunk's handle orphans nothing.
#[test_case]
fn the_kernels_rendezvous_supply_grows_past_one_chunk() {
    let mut names = [0u64; super::KERNEL_EP_CHUNK_PAGES as usize + 1];
    for slot in names.iter_mut() {
        *slot = super::create_rendezvous();
    }

    // Distinct names: a chunk transition that handed back the same page twice would show here.
    for (i, &a) in names.iter().enumerate() {
        for &b in &names[i + 1..] {
            assert_ne!(a, b, "two endpoints share a name across a chunk transition");
        }
    }

    // Every one still resolves, including the earliest, which is in a chunk we have since retired.
    let mut guard = super::IPC_TABLES.lock();
    let sched = guard.as_mut().expect("no scheduler");
    for (i, &ep) in names.iter().enumerate() {
        assert!(
            super::rendezvous_of(sched, ep).is_some(),
            "rendezvous {i} stopped resolving after the supply grew",
        );
    }
}
