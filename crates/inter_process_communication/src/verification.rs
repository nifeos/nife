use super::*;

/// A minimal node: a link and nothing else, the way the proofs like it.
struct N {
    next: Option<NonNull<N>>,
}

impl N {
    fn new() -> Self {
        N { next: None }
    }
}

// SAFETY: `next` and `set_next` read and write the same `next` field and nothing else, which is
// the whole of the `Node` contract.
unsafe impl Node for N {
    fn next(&self) -> Option<NonNull<Self>> {
        self.next
    }
    fn set_next(&mut self, next: Option<NonNull<Self>>) {
        self.next = next;
    }
}

/// Mint `n`'s token, from `&raw mut` on the harness local rather than from a reference
/// (`Unqueued::new`'s provenance clause).
///
/// # Safety
/// `n` is `&raw mut` on a harness local declared before the rendezvous, on no queue, minted only
/// once, and not named again.
unsafe fn mint(n: *mut N) -> Unqueued<N> {
    // SAFETY: this function's own contract is `Unqueued::new`'s, and `&raw mut` on a local is
    // never null.
    unsafe { Unqueued::new(NonNull::new_unchecked(n)) }
}

/// Put `e` into an arbitrary valid state: at most one queue non-empty (modeled as one waiter), a
/// symbolic pending count and a symbolic interrupt binding. A token that is not queued is dropped,
/// which strands its node and nothing else. A binding paired with a queued sender cannot arise,
/// because `bind_to_interrupt` empties the sender queue; seeding it anyway only widens what each
/// harness covers.
fn seed(e: &mut Rendezvous<N>, sender: Unqueued<N>, receiver: Unqueued<N>) {
    e.pending = kani::any();
    e.bound_to_interrupt = kani::any();
    match kani::any::<u8>() {
        0 => e.senders.push_back(sender),
        1 => e.receivers.push_back(receiver),
        _ => {} // both empty
    }
}

/// Falsification: replayable `crates/inter_process_communication/falsifications/verification.send_preserves_the_invariant.patch`
#[kani::proof]
fn send_preserves_the_invariant() {
    let (mut s, mut r, mut me) = (N::new(), N::new(), N::new());
    let mut e: Rendezvous<N> = Rendezvous::new();
    // SAFETY: three distinct fresh locals declared before `e`, each minted once (module note).
    let (s, r, me) = unsafe { (mint(&raw mut s), mint(&raw mut r), mint(&raw mut me)) };
    seed(&mut e, s, r);
    let _ = e.send(me);
    assert!(e.one_queue_invariant());
}

/// Falsification: replayable `crates/inter_process_communication/falsifications/verification.receive_preserves_the_invariant.patch`
#[kani::proof]
fn receive_preserves_the_invariant() {
    let (mut s, mut r, mut me) = (N::new(), N::new(), N::new());
    let mut e: Rendezvous<N> = Rendezvous::new();
    // SAFETY: as in `send_preserves_the_invariant` above.
    let (s, r, me) = unsafe { (mint(&raw mut s), mint(&raw mut r), mint(&raw mut me)) };
    seed(&mut e, s, r);
    let _ = e.receive(me);
    assert!(e.one_queue_invariant());
}

/// **A signal preserves the one-queue invariant**, and it does not panic on any count. `signal`
/// takes no node, so no slip in it can enqueue anything; what it can get wrong is the count. With
/// `seed`'s symbolic `pending`, a plain `pending + 1` overflows at `u32::MAX` and Kani's overflow
/// check turns this harness red, which is why the count saturates.
/// Falsification: replayable `crates/inter_process_communication/falsifications/verification.signal_preserves_the_invariant.patch`
#[kani::proof]
fn signal_preserves_the_invariant() {
    let (mut s, mut r) = (N::new(), N::new());
    let mut e: Rendezvous<N> = Rendezvous::new();
    // SAFETY: two distinct fresh locals declared before `e`, each minted once.
    let (s, r) = unsafe { (mint(&raw mut s), mint(&raw mut r)) };
    seed(&mut e, s, r);
    let _ = e.signal();
    assert!(e.one_queue_invariant());
}

/// **A rendezvous bound to an interrupt never hands a receiver a sender's words.** From every
/// seeded state, binding refuses the parked sender (exactly that one, if `seed` queued it), and
/// after any one further send, signal or receive, a receive is never [`Receive::FromSender`].
/// `unwind(3)` is sound for `removing_a_waiter_preserves_the_invariant`'s reason: `seed` queues at
/// most one waiter, and Kani checks the unwinding assertion.
/// Falsification: replayable `crates/inter_process_communication/falsifications/verification.a_bound_rendezvous_never_yields_a_sender.patch`
#[kani::proof]
#[kani::unwind(3)]
fn a_bound_rendezvous_never_yields_a_sender() {
    let (mut s, mut r, mut other, mut me) = (N::new(), N::new(), N::new(), N::new());
    let sender_pointer = NonNull::from(&mut s);
    let mut e: Rendezvous<N> = Rendezvous::new();
    // SAFETY: four distinct fresh locals declared before `e`, each minted once. `sender_pointer` is
    // only ever compared.
    let (s, r, other, me) =
        unsafe { (mint(&mut s), mint(&mut r), mint(&mut other), mint(&mut me)) };
    seed(&mut e, s, r);

    let sender_was_queued = !e.senders.is_empty();
    let mut refused = 0usize;
    e.bind_to_interrupt(|t| {
        assert!(
            t == sender_pointer,
            "refused a thread nobody queued as a sender"
        );
        refused += 1;
    });
    assert_eq!(refused, usize::from(sender_was_queued));
    assert!(e.one_queue_invariant());

    // One symbolic step between the bind and the receive: a send (refused), a signal, a receive.
    match kani::any::<u8>() {
        0 => assert!(matches!(e.send(other), Send::Refused(_))),
        1 => {
            let _ = e.signal();
        }
        2 => {
            let _ = e.receive(other);
        }
        _ => {}
    }
    assert!(!matches!(e.receive(me), Receive::FromSender(..)));
}

/// **A send rendezvouses exactly when a receiver was waiting**, and with exactly *that*
/// receiver, else blocks. On a rendezvous bound to an interrupt (§101 (notification objects)), the
/// send is refused and changes nothing; removing the refusal turns both `!bound` assertions red.
/// Falsification: replayable `crates/inter_process_communication/falsifications/verification.send_rendezvous_iff_a_receiver_waited.patch`
#[kani::proof]
fn send_rendezvous_iff_a_receiver_waited() {
    let (mut s, mut r, mut me) = (N::new(), N::new(), N::new());
    let receiver_ptr = NonNull::from(&mut r);
    let mut e: Rendezvous<N> = Rendezvous::new();
    // SAFETY: three distinct fresh locals declared before `e`, each minted once. `receiver_ptr`
    // is only ever compared, never dereferenced.
    let (s, r, me) = unsafe { (mint(&raw mut s), mint(&raw mut r), mint(&raw mut me)) };
    seed(&mut e, s, r);

    let had_receiver = !e.receivers.is_empty();
    let bound = e.bound_to_interrupt;
    let before = (e.senders.is_empty(), e.receivers.is_empty(), e.pending);
    match e.send(me) {
        Send::Rendezvous(got, _) => {
            assert!(had_receiver && !bound);
            assert!(
                got == receiver_ptr,
                "rendezvoused with a thread nobody queued"
            );
        }
        Send::Blocked => assert!(!had_receiver && !bound),
        // Refused, and nothing touched: not the waiting driver popped, not the sender parked
        // where the driver's next receive would find it, not the pending interrupt count.
        Send::Refused(_) => {
            assert!(bound);
            assert_eq!(
                (e.senders.is_empty(), e.receivers.is_empty(), e.pending),
                before
            );
        }
    }
}

/// **A caller's token comes back exactly when its node did not join a queue, and it is the
/// caller's own.** For both operations over every seeded state, the verdict that holds no token
/// for the caller is the one that grew the caller's queue, and every other verdict hands back the
/// caller's token. The kernel files that token on the running thread's TCB (`sched::hold_token`).
/// Falsification: replayable `crates/inter_process_communication/falsifications/verification.a_token_comes_back_exactly_when_its_node_did_not_queue.patch`
#[kani::proof]
fn a_token_comes_back_exactly_when_its_node_did_not_queue() {
    let (mut s, mut r, mut me) = (N::new(), N::new(), N::new());
    let me_ptr = NonNull::from(&mut me);
    let mut e: Rendezvous<N> = Rendezvous::new();
    // SAFETY: three distinct fresh locals declared before `e`, each minted once. `me_ptr` is
    // only ever compared.
    let (s, r, me) = unsafe { (mint(&raw mut s), mint(&raw mut r), mint(&raw mut me)) };
    seed(&mut e, s, r);
    let (senders, receivers) = (e.senders.len(), e.receivers.len());

    if kani::any() {
        match e.send(me) {
            Send::Blocked => assert_eq!(e.senders.len(), senders + 1),
            Send::Rendezvous(_, back) | Send::Refused(back) => {
                assert!(back == me_ptr, "handed back somebody else's token");
                assert_eq!(e.senders.len(), senders);
            }
        }
    } else {
        match e.receive(me) {
            Receive::Blocked => assert_eq!(e.receivers.len(), receivers + 1),
            Receive::Signal(back) | Receive::FromSender(_, back) => {
                assert!(back == me_ptr, "handed back somebody else's token");
                assert_eq!(e.receivers.len(), receivers);
            }
        }
    }
}

/// **Taking a waiter back off a queue preserves the one-queue invariant**, for either queue and
/// whether or not the victim was there, and a hit returns the victim's token.
///
/// The miss matters most: the kernel asks both removes on every victim, and a drain over a queue
/// the victim is not on must rebuild it exactly. `unwind(3)` is sound because `seed` queues at
/// most one waiter and Kani checks the unwinding assertion. See
/// `notes/inter-process-communication.md` ("The rendezvous harnesses").
/// Falsification: replayable `crates/inter_process_communication/falsifications/verification.removing_a_waiter_preserves_the_invariant.patch`
#[kani::proof]
#[kani::unwind(3)]
fn removing_a_waiter_preserves_the_invariant() {
    let (mut s, mut r, mut stranger) = (N::new(), N::new(), N::new());
    let (sender_ptr, receiver_ptr) = (NonNull::from(&mut s), NonNull::from(&mut r));
    let stranger_ptr = NonNull::from(&mut stranger);
    let mut e: Rendezvous<N> = Rendezvous::new();
    // SAFETY: two distinct fresh locals declared before `e`, each minted once. The three
    // pointers above are only ever compared.
    let (s, r) = unsafe { (mint(&raw mut s), mint(&raw mut r)) };
    seed(&mut e, s, r);

    // A symbolic victim: one of the two nodes `seed` may have queued, or a third that is on no
    // queue at all, which is the miss the kernel takes on every call.
    let victim = match kani::any::<u8>() {
        0 => sender_ptr,
        1 => receiver_ptr,
        _ => stranger_ptr,
    };
    let found = if kani::any() {
        e.remove_sender(victim)
    } else {
        e.remove_receiver(victim)
    };
    assert!(e.one_queue_invariant());
    // A remove that reports a hit really emptied the queue it was given: `seed` queues at most
    // one waiter, so a hit leaves both queues empty and the rendezvous idle.
    assert!(found.is_none() || e.is_idle());
    assert!(found.is_none_or(|t| t == victim));
}

/// **A pending signal is taken before a queued sender.** A receive drains a counted signal
/// first, so an async signal delivered with nobody waiting is never lost behind a later
/// synchronous sender.
/// Falsification: replayable `crates/inter_process_communication/falsifications/verification.receive_drains_a_pending_signal_first.patch`
#[kani::proof]
fn receive_drains_a_pending_signal_first() {
    let (mut s, mut r, mut me) = (N::new(), N::new(), N::new());
    let mut e: Rendezvous<N> = Rendezvous::new();
    // SAFETY: three distinct fresh locals declared before `e`, each minted once.
    let (s, r, me) = unsafe { (mint(&raw mut s), mint(&raw mut r), mint(&raw mut me)) };
    seed(&mut e, s, r);
    if e.pending > 0 {
        let outcome = e.receive(me);
        assert!(matches!(outcome, Receive::Signal(_)));
    }
}

/// **A collected sender is forgotten by the rendezvous**: the rendezvous half of the one-shot
/// Reply of §12 (Call/Reply IPC). After a receive collects a sender, neither queue names it and no
/// later receive can produce it again, so the Reply capability is the only name for the caller.
/// One waiter covers the general case, because the decision depends only on whether a queue is
/// empty. Separately, a blocked thread cannot run, so it cannot queue itself a second time.
/// Falsification: replayable `crates/inter_process_communication/falsifications/verification.a_collected_sender_is_forgotten.patch`
#[kani::proof]
fn a_collected_sender_is_forgotten() {
    let (mut s, mut r, mut me, mut me2) = (N::new(), N::new(), N::new(), N::new());
    let mut e: Rendezvous<N> = Rendezvous::new();
    // SAFETY: four distinct fresh locals declared before `e`, each minted once. `me2` exists so
    // the second receive does not need `me`'s token back from the first.
    let (s, r, me, me2) = unsafe {
        (
            mint(&raw mut s),
            mint(&raw mut r),
            mint(&raw mut me),
            mint(&raw mut me2),
        )
    };
    seed(&mut e, s, r);
    if matches!(e.receive(me), Receive::FromSender(..)) {
        assert!(e.senders.is_empty() && e.receivers.is_empty());
        assert!(!matches!(e.receive(me2), Receive::FromSender(..)));
    }
}
