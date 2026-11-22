//! The crate's host tests.
//!
//! Every node in these tests is minted once, through [`token`], on an [`Owned`] node declared
//! before its `Rendezvous`, which meets the two obligations [`test_node`] states.

use super::*;

/// The counts a rendezvous reports, written out in the order the old tuple used.
fn counts(
    queued_senders: usize,
    queued_receivers: usize,
    pending_signals: u32,
) -> RendezvousCounts {
    RendezvousCounts {
        queued_senders,
        queued_receivers,
        pending_signals,
    }
}

use test_node::{N, Owned, node, token};

/// **Two verdicts are equal when they decided the same thing about the same nodes**, and a
/// verdict about other nodes, or of another kind, is not. Every node here has one token, so a
/// verdict can share its nodes only with itself.
#[test]
fn a_verdict_equals_itself_and_no_other() {
    {
        let mut n: Vec<Owned> = (0..6).map(|_| node()).collect();
        let mut t = n.iter_mut().map(token);
        let mut next = || t.next().unwrap();
        let sends = [
            SendVerdict::Rendezvous(next(), next()),
            SendVerdict::Rendezvous(next(), next()),
            SendVerdict::Refused(next()),
            SendVerdict::Refused(next()),
            SendVerdict::Blocked,
        ];
        for (i, x) in sends.iter().enumerate() {
            for (j, y) in sends.iter().enumerate() {
                assert_eq!(x == y, i == j, "sends {i} and {j}");
            }
        }
    }
    let mut n: Vec<Owned> = (0..6).map(|_| node()).collect();
    let mut t = n.iter_mut().map(token);
    let mut next = || t.next().unwrap();
    let receives = [
        ReceiveVerdict::Signal(next()),
        ReceiveVerdict::Signal(next()),
        ReceiveVerdict::FromSender(next(), next()),
        ReceiveVerdict::FromSender(next(), next()),
        ReceiveVerdict::Blocked,
    ];
    for (i, x) in receives.iter().enumerate() {
        for (j, y) in receives.iter().enumerate() {
            assert_eq!(x == y, i == j, "receives {i} and {j}");
        }
    }
}

/// The rendezvous, both orderings: whoever arrives first waits, the second completes the pair
/// and gets the first: the very node, by identity, not a name to look up.
#[test]
fn sender_first_then_receiver_rendezvous() {
    let (mut s, mut r) = (node(), node());
    let (sp, rp) = (NonNull::from(&mut *s), NonNull::from(&mut *r));
    let (st, rt) = (token(&mut s), token(&mut r));
    let mut e: Rendezvous<N> = Rendezvous::new();

    assert_eq!(e.send(st), SendVerdict::Blocked); // nobody waiting: park the sender
    match e.receive(rt) {
        // receiver collects it, and keeps its own token
        ReceiveVerdict::FromSender(got, back) => assert!(got == sp && back == rp),
        other => panic!("{other:?}"),
    }
    assert!(e.one_queue_invariant_holds());
}

/// **A driver waiting on its interrupt is handed the interrupt and nothing else** (DECISIONS
/// §101, ruling B). The driver parks first, which is the case that matters: without the refusal
/// a sender would rendezvous with it and it would wake holding the sender's words.
#[test]
fn a_driver_waiting_on_its_interrupt_is_not_handed_a_send() {
    let (mut driver, mut forger) = (node(), node());
    let (dp, fp) = (NonNull::from(&mut *driver), NonNull::from(&mut *forger));
    let (dt, ft) = (token(&mut driver), token(&mut forger));
    let mut e: Rendezvous<N> = Rendezvous::new();
    e.bind_to_interrupt(|_| panic!("nobody was queued"));

    assert_eq!(e.receive(dt), ReceiveVerdict::Blocked);
    match e.send(ft) {
        SendVerdict::Refused(back) => assert!(back == fp, "the forger keeps its own token"),
        other => panic!("{other:?}"),
    }
    // Still parked, still the only waiter, and the interrupt still reaches it.
    assert_eq!(e.debug_counts(), counts(0, 1, 0));
    assert!(e.signal().is_some_and(|t| t == dp));
    assert!(e.is_idle());
}

/// **Binding refuses the senders already parked, so a receive there never collects one.** Two
/// senders queue before the bind; the bind hands both back in order, and the driver's receive
/// that follows blocks for the interrupt instead of waking with a sender's words.
#[test]
fn binding_refuses_the_senders_already_parked() {
    let (mut first, mut second, mut driver) = (node(), node(), node());
    let first_at = NonNull::from(&mut *first);
    let second_at = NonNull::from(&mut *second);
    let driver_at = NonNull::from(&mut *driver);
    let (first, second, driver) = (token(&mut first), token(&mut second), token(&mut driver));
    let mut e: Rendezvous<N> = Rendezvous::new();
    assert_eq!(e.send(first), SendVerdict::Blocked);
    assert_eq!(e.send(second), SendVerdict::Blocked);

    let mut refused = Vec::new();
    e.bind_to_interrupt(|t| refused.push(t.as_non_null()));
    assert_eq!(
        refused,
        [first_at, second_at],
        "every parked sender, oldest first"
    );
    assert_eq!(e.debug_counts(), counts(0, 0, 0));

    assert_eq!(e.receive(driver), ReceiveVerdict::Blocked);
    assert!(e.signal().is_some_and(|t| t == driver_at));
    assert!(e.is_idle());
}

#[test]
fn receiver_first_then_sender_rendezvous() {
    let (mut s, mut r) = (node(), node());
    let (sp, rp) = (NonNull::from(&mut *s), NonNull::from(&mut *r));
    let (st, rt) = (token(&mut s), token(&mut r));
    let mut e: Rendezvous<N> = Rendezvous::new();

    assert_eq!(e.receive(rt), ReceiveVerdict::Blocked);
    match e.send(st) {
        // sender meets the waiter
        SendVerdict::Rendezvous(got, back) => assert!(got == rp && back == sp),
        other => panic!("{other:?}"),
    }
}

/// Two senders queue in FIFO order; two receivers drain them in the same order.
#[test]
fn senders_queue_in_arrival_order() {
    let (mut a, mut b, mut r) = (node(), node(), node());
    let (ap, bp) = (NonNull::from(&mut *a), NonNull::from(&mut *b));
    let (at, bt, rt) = (token(&mut a), token(&mut b), token(&mut r));
    let mut e: Rendezvous<N> = Rendezvous::new();

    assert_eq!(e.send(at), SendVerdict::Blocked);
    assert_eq!(e.send(bt), SendVerdict::Blocked);
    // The receiver's token comes back from each collect, and is what the next receive takes:
    // that it was never queued is now a fact the second call cannot be written without.
    let ReceiveVerdict::FromSender(first, rt) = e.receive(rt) else {
        panic!("no sender collected")
    };
    assert!(first == ap);
    let ReceiveVerdict::FromSender(second, _) = e.receive(rt) else {
        panic!("no sender collected")
    };
    assert!(second == bp);
}

/// A signal with nobody waiting is counted; the next receives drain it, then block.
#[test]
fn a_signal_to_an_empty_rendezvous_is_counted_then_drained() {
    let mut r = node();
    let rt = token(&mut r);
    let mut e: Rendezvous<N> = Rendezvous::new();

    assert!(e.signal().is_none()); // counted
    assert!(e.signal().is_none());
    let ReceiveVerdict::Signal(rt) = e.receive(rt) else {
        panic!("the first counted signal was not drained")
    };
    let ReceiveVerdict::Signal(rt) = e.receive(rt) else {
        panic!("the second counted signal was not drained")
    };
    assert_eq!(e.receive(rt), ReceiveVerdict::Blocked);
}

/// The rendezvous-destroy contract (object revocation): `drain_waiters` hands back every parked
/// thread exactly once, in queue order, and leaves the rendezvous idle. The kernel's `revoke`
/// wakes each one with an error; if a waiter were skipped it would sleep forever on a dead
/// rendezvous, and if one were handed back twice it would be double-queued on a run queue.
#[test]
fn drain_hands_back_every_waiter_and_leaves_the_rendezvous_idle() {
    let (mut a, mut b, mut r) = (node(), node(), node());
    let (ap, bp, rp) = (
        NonNull::from(&mut *a),
        NonNull::from(&mut *b),
        NonNull::from(&mut *r),
    );
    let (at, bt, rt) = (token(&mut a), token(&mut b), token(&mut r));
    // Via `default()`: the kernel retypes rendezvous pages through it, not through `new()`.
    let mut e: Rendezvous<N> = Rendezvous::default();

    assert_eq!(e.send(at), SendVerdict::Blocked);
    assert_eq!(e.send(bt), SendVerdict::Blocked);
    assert!(!e.is_idle(), "parked senders hold the rendezvous live");

    let mut drained = Vec::new();
    e.drain_waiters(|w| drained.push(w.as_non_null()));
    assert_eq!(drained, [ap, bp]);
    assert!(e.is_idle());

    // The other queue drains through the same path: a receiver can be parked too.
    assert_eq!(e.receive(rt), ReceiveVerdict::Blocked);
    drained.clear();
    e.drain_waiters(|w| drained.push(w.as_non_null()));
    assert_eq!(drained, [rp]);
    assert!(e.is_idle());
}

/// Pending signals do not hold an rendezvous live: `is_idle` counts blocked threads, not
/// counters. An rendezvous whose only state is undelivered signals is safe to destroy (a
/// signal holds no thread, so nobody is left sleeping), and revocation relies on that.
#[test]
fn pending_signals_do_not_make_an_rendezvous_busy() {
    let mut e: Rendezvous<N> = Rendezvous::new();
    assert!(e.signal().is_none());
    assert!(e.signal().is_none());
    assert!(e.is_idle());
}

/// **The pending count saturates and never wraps**: a signal arriving at the largest count leaves
/// it there (wrapping to zero would lose every counted signal), and a receive still drains one.
#[test]
fn the_pending_count_saturates_and_a_receive_still_drains_one() {
    let mut e: Rendezvous<N> = Rendezvous::new();
    e.pending = u32::MAX;
    assert!(e.signal().is_none());
    assert_eq!(e.debug_counts(), counts(0, 0, u32::MAX));
    let mut driver = node();
    let ReceiveVerdict::Signal(back) = e.receive(token(&mut driver)) else {
        panic!("a pending signal was not drained");
    };
    assert_eq!(e.debug_counts(), counts(0, 0, u32::MAX - 1));
    drop(back);
}

/// **A queued sender can be taken back out of the middle**, which is what reaping a corpse
/// needs (DECISIONS §32, and §16's `DESTROY` before it): a supervised thread that died with
/// nobody receiving is parked here with its death message, and freeing it while it is still
/// linked would leave the next `receive` following a dangling pointer. The survivors keep FIFO
/// order, the length drops by exactly one, the removal hands back the victim's token, and
/// removing something that is not queued reports `None` and changes nothing.
#[test]
fn a_queued_sender_can_be_removed_from_the_middle() {
    let (mut a, mut b, mut c, mut r) = (node(), node(), node(), node());
    let (ap, bp, cp) = (
        NonNull::from(&mut *a),
        NonNull::from(&mut *b),
        NonNull::from(&mut *c),
    );
    let rt = token(&mut r);
    let mut e: Rendezvous<N> = Rendezvous::new();

    for t in [token(&mut a), token(&mut b), token(&mut c)] {
        assert_eq!(e.send(t), SendVerdict::Blocked);
    }
    assert!(e.remove_sender(bp).is_some_and(|t| t == bp), "b was queued");
    assert_eq!(
        e.debug_counts().queued_senders,
        2,
        "exactly one sender left the queue"
    );
    let ReceiveVerdict::FromSender(first, rt) = e.receive(rt) else {
        panic!("a was not collected")
    };
    assert!(first == ap);
    let ReceiveVerdict::FromSender(second, rt) = e.receive(rt) else {
        panic!("c was not collected")
    };
    assert!(second == cp);
    assert_eq!(e.receive(rt), ReceiveVerdict::Blocked);

    // Not queued (already collected, the ordinary case): a no-op that says so.
    let mut e2: Rendezvous<N> = Rendezvous::new();
    assert!(e2.remove_sender(ap).is_none());
    assert!(e2.is_idle());
}

/// Removing the *only* queued sender leaves the rendezvous idle rather than a queue with a stale
/// tail: the classic drained-to-empty bug, which matters here because the single-corpse case is
/// the common one.
#[test]
fn removing_the_only_sender_leaves_the_rendezvous_idle() {
    let (mut a, mut r) = (node(), node());
    let ap = NonNull::from(&mut *a);
    let (at, rt) = (token(&mut a), token(&mut r));
    let mut e: Rendezvous<N> = Rendezvous::new();

    assert_eq!(e.send(at), SendVerdict::Blocked);
    assert!(e.remove_sender(ap).is_some());
    assert!(e.is_idle(), "the rendezvous still holds a sender");
    assert_eq!(e.receive(rt), ReceiveVerdict::Blocked);
    // And it can be used again afterwards: push, pop, no ghost.
    assert!(e.one_queue_invariant_holds());
}

/// **A queued receiver can be taken back out of the middle**, which is what milestone 133's
/// completed reclaim needs: a server parked in `RECEIVE` on somebody else's rendezvous is linked
/// here, and `MemoryRegion::DESTROY` on the region holding its TCB frees the page that link
/// points into. The survivors keep FIFO order, the count drops by exactly one, and removing
/// something that is not queued reports `None` and changes nothing.
#[test]
fn a_queued_receiver_can_be_removed_from_the_middle() {
    let (mut a, mut b, mut c, mut sdr) = (node(), node(), node(), node());
    let (ap, bp, cp) = (
        NonNull::from(&mut *a),
        NonNull::from(&mut *b),
        NonNull::from(&mut *c),
    );
    let st = token(&mut sdr);
    let mut e: Rendezvous<N> = Rendezvous::new();

    let mut a_again = None;
    for t in [token(&mut a), token(&mut b), token(&mut c)] {
        assert_eq!(e.receive(t), ReceiveVerdict::Blocked);
    }
    assert!(
        e.remove_receiver(bp).is_some_and(|t| t == bp),
        "b was queued"
    );
    assert_eq!(
        e.debug_counts().queued_receivers,
        2,
        "exactly one receiver left the queue"
    );
    let SendVerdict::Rendezvous(first, st) = e.send(st) else {
        panic!("a was not met")
    };
    assert!(first == ap);
    a_again.replace(first);
    let SendVerdict::Rendezvous(second, _) = e.send(st) else {
        panic!("c was not met")
    };
    assert!(second == cp);

    // Not queued: a no-op that says so, and it does not reach into the *sender* queue either.
    let mut e2: Rendezvous<N> = Rendezvous::new();
    assert_eq!(e2.send(a_again.take().unwrap()), SendVerdict::Blocked);
    assert!(
        e2.remove_receiver(ap).is_none(),
        "a queued sender is not a receiver"
    );
    assert_eq!(
        e2.debug_counts().queued_senders,
        1,
        "the sender queue was left alone"
    );
}

/// Removing the *only* queued receiver leaves the rendezvous idle rather than a queue with a
/// stale tail, the same drained-to-empty check `remove_sender` carries. A lone blocked server
/// is the common shape of the thread milestone 133 ends.
#[test]
fn removing_the_only_receiver_leaves_the_rendezvous_idle() {
    let (mut a, mut sdr) = (node(), node());
    let ap = NonNull::from(&mut *a);
    let (at, st) = (token(&mut a), token(&mut sdr));
    let mut e: Rendezvous<N> = Rendezvous::new();

    assert_eq!(e.receive(at), ReceiveVerdict::Blocked);
    assert!(e.remove_receiver(ap).is_some());
    assert!(e.is_idle(), "the rendezvous still holds a receiver");
    assert_eq!(e.send(st), SendVerdict::Blocked);
    assert!(e.one_queue_invariant_holds());
}

/// A signal with a receiver waiting hands it back directly and counts nothing.
#[test]
fn a_signal_wakes_a_waiting_receiver() {
    let mut r = node();
    let rp = NonNull::from(&mut *r);
    let rt = token(&mut r);
    let mut e: Rendezvous<N> = Rendezvous::new();

    assert_eq!(e.receive(rt), ReceiveVerdict::Blocked);
    assert!(e.signal().is_some_and(|t| t == rp)); // the waiter, dequeued
    assert!(e.one_queue_invariant_holds());
}

/// The manual `PartialEq` and `Debug` impls answer for themselves. Every use above compares
/// equal values, so an eq stuck at `true` passed (milestone 85); different variants must
/// disagree, and the rendering is the string a hang dump prints.
#[test]
fn verdict_variants_are_distinct_and_print_their_names() {
    let mut n: Vec<Owned> = (0..6).map(|_| node()).collect();
    let [s1, s2, s3, s4, s5, t1] = [0, 1, 2, 3, 4, 5].map(|i| token(&mut n[i]));
    assert_ne!(SendVerdict::<N>::Blocked, SendVerdict::Rendezvous(s1, s2));
    assert_ne!(ReceiveVerdict::Signal(s3), ReceiveVerdict::Blocked);
    let from = ReceiveVerdict::FromSender(s4, t1);
    assert!(format!("{from:?}").starts_with("FromSender"));
    assert_ne!(from, ReceiveVerdict::Signal(s5));
    assert_eq!(format!("{:?}", SendVerdict::<N>::Blocked), "Blocked");
    assert_eq!(format!("{:?}", ReceiveVerdict::<N>::Blocked), "Blocked");
}

/// A rendezvous starts as an ordinary one, and binding it to an interrupt is visible and stays.
#[test]
fn binding_to_an_interrupt_is_visible_and_one_way() {
    let mut e: Rendezvous<N> = Rendezvous::new();
    assert!(!e.is_bound_to_interrupt());
    e.bind_to_interrupt(|_| panic!("nobody was queued"));
    assert!(e.is_bound_to_interrupt());
    e.bind_to_interrupt(|_| panic!("nobody was queued"));
    assert!(e.is_bound_to_interrupt(), "binding twice does not undo it");
}

/// One tagged queue in place of two: a queue's three words, the tag, and the count with the flag,
/// five words on a 64-bit target where two queues took seven.
#[test]
#[cfg(target_pointer_width = "64")]
fn a_rendezvous_is_five_words() {
    assert_eq!(core::mem::size_of::<Rendezvous<N>>(), 5 * 8);
}
