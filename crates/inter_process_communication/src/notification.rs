//! The notification state machine of milestone 151 (notification objects), to the rules of §101
//! (notification objects): a data word and a wait queue, and what a signal, a wait and a try-wait do
//! with them.
//!
//! A signaler ORs bits into the word and never blocks; a waiter takes whatever has accumulated. A
//! non-zero word and a queued waiter never coexist. Whether the bound thread is parked in a
//! receive is the kernel's to decide, and [`Notification::signal`] takes it as a boolean. See
//! `notes/inter-process-communication.md` ("The notification").
//!
//! Name: ratified 2026-11-21 (UTC). `Notification` is §101's own word for the object, and seL4's; the module,
//! the type and the two verdict enums are ratified.

use core::ptr::NonNull;

use intrusive_queue::{Node, Queue, Unqueued};

/// One notification object: the accumulated word and the threads waiting for it to become
/// non-zero. See `notes/inter-process-communication.md`.
///
/// # Examples
///
/// ```
/// use core::ptr::NonNull;
/// use intrusive_queue::{Node, Unqueued};
/// use inter_process_communication::notification::{Notification, SignalVerdict, WaitVerdict};
/// # struct ThreadControlBlock {
/// #     next: Option<NonNull<ThreadControlBlock>>,
/// # }
/// # // SAFETY: plain field storage, which is the whole of the `Node` contract.
/// # unsafe impl Node for ThreadControlBlock {
/// #     fn next(&self) -> Option<NonNull<Self>> { self.next }
/// #     fn set_next(&mut self, next: Option<NonNull<Self>>) { self.next = next; }
/// # }
/// let mut waiter = ThreadControlBlock { next: None };
/// let waiter = NonNull::new(&raw mut waiter).unwrap();
/// let mut n: Notification<ThreadControlBlock> = Notification::new();
/// // Two signals with nobody waiting accumulate into one word. Neither is lost.
/// assert_eq!(n.signal(0b01, false), SignalVerdict::Counted);
/// assert_eq!(n.signal(0b10, false), SignalVerdict::Counted);
///
/// // SAFETY: from a live local's place, declared before `n`, not named again, on no queue.
/// let token = unsafe { Unqueued::new(waiter) };
///
/// // A wait takes the whole word and clears it, without blocking, and hands the token back.
/// let WaitVerdict::Word(word, token) = n.wait(token) else { panic!("blocked on a non-zero word") };
/// assert_eq!(word, 0b11);
///
/// // The next wait finds nothing and queues; a signal now wakes it with its bits, already dequeued.
/// assert_eq!(n.wait(token), WaitVerdict::Blocked);
/// match n.signal(0b100, false) {
///     SignalVerdict::Woke(woken, bits) => assert!(woken == waiter && bits == 0b100),
///     other => panic!("{other:?}"),
/// }
/// assert!(n.is_idle());
/// ```
///
/// Name: ratified 2026-11-21 (UTC).
pub struct Notification<T: Node> {
    /// Bits signaled since the last wait, try-wait or bound delivery took them. A bitfield of binary
    /// semaphores, as seL4's is: a signal ORs in, a take clears all of it.
    word: u64,
    /// Threads blocked in `WAIT`, oldest first.
    waiters: Queue<T>,
}

/// What a hang dump reads from a [`Notification`]:
/// [`debug_counts`](Notification::debug_counts).
///
/// Name: ratified 2026-11-21 (UTC), with its two fields. Named fields in place of a tuple whose positions a
/// reader had to remember. Refused: keeping the tuple.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationCounts {
    /// Threads blocked waiting.
    pub waiters: usize,
    /// The accumulated word.
    pub word: u64,
}

/// What a [`signal`](Notification::signal) decided.
///
/// Name: ratified 2026-11-21 (UTC), with its variants. The `Verdict` suffix gives all five operation enums one
/// shape, and is the word this crate's prose already uses for what an operation decided. Refused:
/// `SignalOutcome` and keeping the bare name.
pub enum SignalVerdict<T> {
    /// A thread was waiting on the notification itself: deliver this word to it and wake it. It is
    /// already off the queue, and this is its token.
    Woke(Unqueued<T>, u64),
    /// Nobody waited here, and the bound thread is parked in a receive on an endpoint: deliver this
    /// word to it there (the kernel unlinks it from that endpoint's queue). The word is cleared.
    ToBound(u64),
    /// Nobody could take it now; the bits are in the word for the next wait, try-wait or receive.
    Counted,
    /// The signal carried no bits, so there was nothing to deliver and nobody was woken.
    Empty,
}

/// What a [`wait`](Notification::wait) decided. As for the rendezvous's verdicts, the one that
/// leaves the caller off the queue hands its token back.
///
/// Name: ratified 2026-11-21 (UTC), with its variants. The `Verdict` suffix gives all five operation enums one
/// shape, and is the word this crate's prose already uses for what an operation decided. Refused:
/// `WaitOutcome` and keeping the bare name.
pub enum WaitVerdict<T> {
    /// The word was non-zero: here it is, and it is now clear. The caller does not block, and keeps
    /// its token.
    Word(u64, Unqueued<T>),
    /// The word was zero: the caller is now queued, holding its token, and should block.
    Blocked,
}

impl<T> PartialEq for WaitVerdict<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (WaitVerdict::Word(x, a), WaitVerdict::Word(y, b)) => {
                x == y && a.as_non_null() == b.as_non_null()
            }
            (WaitVerdict::Blocked, WaitVerdict::Blocked) => true,
            _ => false,
        }
    }
}
impl<T> Eq for WaitVerdict<T> {}
impl<T> core::fmt::Debug for WaitVerdict<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            WaitVerdict::Word(w, t) => f.debug_tuple("Word").field(w).field(t).finish(),
            WaitVerdict::Blocked => f.write_str("Blocked"),
        }
    }
}

// Manual impls, for the reason the rendezvous's `SendVerdict` gives: only the pointer is compared, and the
// kernel's `T` (a TCB) is neither `PartialEq` nor `Debug`.
impl<T> PartialEq for SignalVerdict<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (SignalVerdict::Woke(a, x), SignalVerdict::Woke(b, y)) => {
                a.as_non_null() == b.as_non_null() && x == y
            }
            (SignalVerdict::ToBound(x), SignalVerdict::ToBound(y)) => x == y,
            (SignalVerdict::Counted, SignalVerdict::Counted)
            | (SignalVerdict::Empty, SignalVerdict::Empty) => true,
            _ => false,
        }
    }
}
impl<T> Eq for SignalVerdict<T> {}
impl<T> core::fmt::Debug for SignalVerdict<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SignalVerdict::Woke(p, w) => f.debug_tuple("Woke").field(p).field(w).finish(),
            SignalVerdict::ToBound(w) => f.debug_tuple("ToBound").field(w).finish(),
            SignalVerdict::Counted => f.write_str("Counted"),
            SignalVerdict::Empty => f.write_str("Empty"),
        }
    }
}

impl<T: Node> Notification<T> {
    /// An idle notification: word zero, nobody waiting.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub const fn new() -> Self {
        Self {
            word: 0,
            waiters: Queue::new(),
        }
    }

    /// **A non-zero word and a queued waiter never coexist.** The load-bearing invariant.
    ///
    /// Name: ratified 2026-11-21 (UTC). The verb form the Rust predicate rule asks for, as its rendezvous twin
    /// (`one_queue_invariant_holds`) has.
    pub fn invariant_holds(&self) -> bool {
        self.word == 0 || self.waiters.is_empty()
    }

    /// No thread is waiting here. The word does not count: a word holds no thread.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub fn is_idle(&self) -> bool {
        self.waiters.is_empty()
    }

    /// The accumulated word, without taking it. Diagnostic (a hang dump), and what the kernel's
    /// bind reads to decide whether a newly bound receiver has something waiting for it.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub fn word(&self) -> u64 {
        self.word
    }

    /// Diagnostic: how many threads are waiting and the accumulated word, for a hang dump.
    ///
    /// Name: ratified 2026-11-21 (UTC), with [`NotificationCounts`] and its two fields, which replace a tuple
    /// whose positions a reader had to remember.
    pub fn debug_counts(&self) -> NotificationCounts {
        NotificationCounts {
            waiters: self.waiters.len(),
            word: self.word,
        }
    }

    /// A signal arrives carrying `bits`. Never blocks, never queues the signaler, never loses a
    /// bit. In §101's order: a thread waiting here takes the bits ([`SignalVerdict::Woke`]); otherwise, if
    /// `bound_receiving` says the bound thread is parked in a receive, it takes the whole word
    /// ([`SignalVerdict::ToBound`]); otherwise the bits are merged into the word by OR
    /// ([`SignalVerdict::Counted`]).
    ///
    /// Zero bits is [`SignalVerdict::Empty`] and changes nothing, for the reason
    /// `partsfoundry/record:notes/notification-objects.md` gives.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub fn signal(&mut self, bits: u64, bound_receiving: bool) -> SignalVerdict<T> {
        if bits == 0 {
            return SignalVerdict::Empty;
        }
        if let Some(waiter) = self.waiters.pop_front() {
            // The invariant says the word is zero while anyone waits, so these bits are the whole
            // of what is pending. `| self.word` would be a no-op, and writing it would hide that.
            SignalVerdict::Woke(waiter, bits)
        } else if bound_receiving {
            let word = self.word | bits;
            self.word = 0;
            SignalVerdict::ToBound(word)
        } else {
            self.word |= bits;
            SignalVerdict::Counted
        }
    }

    /// A thread `me` waits. Take the word if it is non-zero, handing `me` back; otherwise queue
    /// `me`, and the caller blocks it.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub fn wait(&mut self, me: Unqueued<T>) -> WaitVerdict<T> {
        if self.word != 0 {
            WaitVerdict::Word(core::mem::take(&mut self.word), me)
        } else {
            self.waiters.push_back(me);
            WaitVerdict::Blocked
        }
    }

    /// **Take the word without blocking**: the non-blocking `WAIT`, and also what a bound thread's
    /// receive does on entry to collect a signal that arrived while it was elsewhere. Zero means
    /// nothing was pending.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub fn try_wait(&mut self) -> u64 {
        core::mem::take(&mut self.word)
    }

    /// Take one specific waiter back off the queue, returning its token if it was there. For a
    /// teardown that ends a thread blocked in `WAIT`, which must unlink it before its page is
    /// freed. The same unlink as
    /// [`Rendezvous::remove_receiver`](crate::Rendezvous::remove_receiver): `victim` is only
    /// compared unless the queue holds it.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub fn remove_waiter(&mut self, victim: NonNull<T>) -> Option<Unqueued<T>> {
        self.waiters.remove(victim)
    }

    /// **Empty the wait queue, handing every waiter to `f`** (the notification is being destroyed).
    /// Each is popped before `f` sees it, token and all, so `f` may queue it elsewhere.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub fn drain_waiters(&mut self, mut f: impl FnMut(Unqueued<T>)) {
        while let Some(w) = self.waiters.pop_front() {
            f(w);
        }
    }
}

impl<T: Node> Default for Notification<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Machine-checked proofs of the notification state machine, in the rendezvous module's shape:
/// inductive steps from an arbitrary valid state, with a non-empty queue modeled as one waiter,
/// because every decision here depends on whether the queue is *empty* and never on its length.
///
/// The only `unsafe` left is minting tokens, and its two obligations are stated once in
/// `test_node`: every node is declared before the notification (so it outlives it), and no node is
/// minted twice.
#[cfg(kani)]
mod verification {
    use super::*;
    use crate::test_node::{N, mint};

    /// An arbitrary valid state: either a symbolic non-zero word and nobody waiting, or a zero word
    /// and at most one waiter. Safe: the waiter arrives as a token.
    fn seed(n: &mut Notification<N>, waiter: Unqueued<N>) {
        if kani::any() {
            n.word = kani::any();
        } else if kani::any() {
            n.waiters.push_back(waiter);
        }
    }

    /// **Every operation preserves the invariant.** One symbolic operation from an arbitrary valid
    /// state: signal (with an arbitrary word and an arbitrary binding verdict), wait, try-wait, or
    /// remove a waiter.
    /// Falsification: replayable `crates/inter_process_communication/falsifications/notification.verification.every_operation_preserves_the_invariant.patch`
    #[kani::proof]
    #[kani::unwind(3)]
    fn every_operation_preserves_the_invariant() {
        let (mut w, mut me) = (N::new(), N::new());
        let waiter = NonNull::from(&mut w);
        let mut n: Notification<N> = Notification::new();
        // SAFETY: two distinct fresh nodes declared before `n`, each minted once.
        let (w, me) = unsafe { (mint(&raw mut w), mint(&raw mut me)) };
        seed(&mut n, w);
        match kani::any::<u8>() {
            0 => {
                let _ = n.signal(kani::any(), kani::any());
            }
            1 => {
                let _ = n.wait(me);
            }
            2 => {
                n.try_wait();
            }
            _ => {
                let _ = n.remove_waiter(waiter);
            }
        }
        assert!(n.invariant_holds());
    }

    /// **A signal loses no bit.** Whatever was pending plus what was signaled ends up either in
    /// the word or in the delivery, exactly: `delivered | word_after == word_before | bits`.
    /// Falsification: replayable `crates/inter_process_communication/falsifications/notification.verification.a_signal_loses_no_bit.patch`
    #[kani::proof]
    fn a_signal_loses_no_bit() {
        let mut w = N::new();
        let mut n: Notification<N> = Notification::new();
        // SAFETY: `w` is a fresh node declared before `n`, minted once.
        seed(&mut n, unsafe { mint(&raw mut w) });
        let before = n.word;
        let bits: u64 = kani::any();
        let delivered = match n.signal(bits, kani::any()) {
            SignalVerdict::Woke(_, word) | SignalVerdict::ToBound(word) => word,
            SignalVerdict::Counted | SignalVerdict::Empty => 0,
        };
        assert_eq!(delivered | n.word, before | bits);
    }

    /// **A signal wakes the waiter when there is one, and only then**, and with exactly that
    /// waiter; the bound receiver is reached only when nobody waits on the notification itself.
    /// §101's order, rule 1 before rule 2.
    /// Falsification: replayable `crates/inter_process_communication/falsifications/notification.verification.a_waiter_is_woken_before_the_bound_receiver.patch`
    #[kani::proof]
    fn a_waiter_is_woken_before_the_bound_receiver() {
        let mut w = N::new();
        let waiter = NonNull::from(&mut w);
        let mut n: Notification<N> = Notification::new();
        // SAFETY: `w` is a fresh node declared before `n`, minted once; `waiter` is only compared.
        seed(&mut n, unsafe { mint(&raw mut w) });
        let had_waiter = !n.waiters.is_empty();
        let bits: u64 = kani::any();
        kani::assume(bits != 0);
        let bound: bool = kani::any();
        match n.signal(bits, bound) {
            SignalVerdict::Woke(got, _) => {
                assert!(had_waiter);
                assert!(got == waiter);
            }
            SignalVerdict::ToBound(_) => {
                assert!(!had_waiter && bound);
            }
            SignalVerdict::Counted => {
                assert!(!had_waiter && !bound);
            }
            SignalVerdict::Empty => panic!("a non-zero signal reported nothing to deliver"),
        }
    }

    /// **A wait blocks exactly when the word is zero, and a word it returns is non-zero and
    /// cleared.** So `WAIT` never returns an empty word, which is what lets zero mean "nothing"
    /// from `POLL`.
    /// Falsification: replayable `crates/inter_process_communication/falsifications/notification.verification.a_wait_returns_a_nonzero_word_or_blocks.patch`
    #[kani::proof]
    fn a_wait_returns_a_nonzero_word_or_blocks() {
        let (mut w, mut me) = (N::new(), N::new());
        let mut n: Notification<N> = Notification::new();
        // SAFETY: two distinct fresh nodes declared before `n`, each minted once.
        let (w, me) = unsafe { (mint(&raw mut w), mint(&raw mut me)) };
        seed(&mut n, w);
        let before = n.word;
        match n.wait(me) {
            WaitVerdict::Word(word, _) => {
                assert!(word != 0);
                assert_eq!(word, before);
                assert_eq!(n.word, 0);
            }
            WaitVerdict::Blocked => assert_eq!(before, 0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_node::{N, node, token};

    /// **A wait's verdict is equal only to itself**: the same word for another node, another word,
    /// or a block are all different verdicts. Each node has one token, so only a verdict compared
    /// with itself shares one.
    #[test]
    fn a_wait_verdict_equals_itself_and_no_other() {
        let (mut a, mut b, mut c) = (node(), node(), node());
        let waits = [
            WaitVerdict::Word(1, token(&mut a)),
            WaitVerdict::Word(1, token(&mut b)),
            WaitVerdict::Word(2, token(&mut c)),
            WaitVerdict::Blocked,
        ];
        for (i, x) in waits.iter().enumerate() {
            for (j, y) in waits.iter().enumerate() {
                assert_eq!(x == y, i == j, "waits {i} and {j}");
            }
        }
    }

    /// The bound path takes the whole accumulated word, not just the newest bits: a signal that
    /// was counted while the bound thread was elsewhere rides along with the one that finds it
    /// receiving.
    #[test]
    fn the_bound_receiver_takes_what_accumulated() {
        let mut n: Notification<N> = Notification::new();
        assert_eq!(n.signal(0b001, false), SignalVerdict::Counted);
        assert_eq!(n.signal(0b100, true), SignalVerdict::ToBound(0b101));
        assert_eq!(n.word(), 0);
    }

    /// The diagnostics a hang dump reads agree with the state, and a drain hands back every waiter
    /// and leaves the object idle: the teardown path a destroyed notification takes.
    #[test]
    fn diagnostics_and_drain_agree_with_the_state() {
        let (mut a, mut b) = (node(), node());
        let (pointer_a, pointer_b) = (NonNull::from(&mut *a), NonNull::from(&mut *b));
        let mut n: Notification<N> = Notification::default();
        assert!(n.is_idle() && n.invariant_holds());
        assert_eq!(
            n.debug_counts(),
            NotificationCounts {
                waiters: 0,
                word: 0
            }
        );
        assert_eq!(n.wait(token(&mut a)), WaitVerdict::Blocked);
        assert_eq!(n.wait(token(&mut b)), WaitVerdict::Blocked);
        assert!(!n.is_idle());
        assert_eq!(
            n.debug_counts(),
            NotificationCounts {
                waiters: 2,
                word: 0
            }
        );
        let mut drained = Vec::new();
        n.drain_waiters(|w| drained.push(w.as_non_null()));
        assert_eq!(drained, vec![pointer_a, pointer_b]);
        assert!(n.is_idle());
        assert_eq!(
            n.signal(0, true),
            SignalVerdict::Empty,
            "no bits is nothing to deliver"
        );
        assert_eq!(n.signal(0b10, false), SignalVerdict::Counted);
        assert_eq!(
            n.debug_counts(),
            NotificationCounts {
                waiters: 0,
                word: 0b10
            }
        );
    }

    /// The outcome enums compare and print by variant and payload, which is what every assertion in
    /// the kernel's own tests leans on.
    #[test]
    fn outcomes_compare_and_print_by_variant() {
        let (mut a, mut b, mut c) = (node(), node(), node());
        let all: [SignalVerdict<N>; 4] = [
            SignalVerdict::Woke(token(&mut a), 1),
            SignalVerdict::ToBound(2),
            SignalVerdict::Counted,
            SignalVerdict::Empty,
        ];
        for (i, x) in all.iter().enumerate() {
            for (j, y) in all.iter().enumerate() {
                assert_eq!(x == y, i == j);
            }
        }
        assert_ne!(SignalVerdict::<N>::ToBound(1), SignalVerdict::ToBound(2));
        assert_ne!(
            SignalVerdict::Woke(token(&mut b), 1),
            SignalVerdict::Woke(token(&mut c), 1)
        );
        let printed: Vec<String> = all.iter().map(|s| format!("{s:?}")).collect();
        assert!(printed[0].starts_with("Woke("));
        assert_eq!(printed[1], "ToBound(2)");
        assert_eq!(printed[2], "Counted");
        assert_eq!(printed[3], "Empty");
        assert_eq!(format!("{:?}", WaitVerdict::<N>::Blocked), "Blocked");
    }

    /// Waiters leave in arrival order, and removing one from the middle keeps the others.
    #[test]
    fn remove_waiter_keeps_the_rest_in_order() {
        let (mut a, mut b, mut c) = (node(), node(), node());
        let (pointer_a, pointer_b, pointer_c) = (
            NonNull::from(&mut *a),
            NonNull::from(&mut *b),
            NonNull::from(&mut *c),
        );
        let mut n: Notification<N> = Notification::new();
        assert_eq!(n.wait(token(&mut a)), WaitVerdict::Blocked);
        assert_eq!(n.wait(token(&mut b)), WaitVerdict::Blocked);
        assert_eq!(n.wait(token(&mut c)), WaitVerdict::Blocked);
        assert!(n.remove_waiter(pointer_b).is_some_and(|t| t == pointer_b));
        assert!(n.remove_waiter(pointer_b).is_none());
        assert!(matches!(n.signal(1, false), SignalVerdict::Woke(t, 1) if t == pointer_a));
        assert!(matches!(n.signal(2, false), SignalVerdict::Woke(t, 2) if t == pointer_c));
        assert_eq!(n.signal(4, false), SignalVerdict::Counted);
        assert_eq!(n.try_wait(), 4);
        assert_eq!(n.try_wait(), 0);
    }

    /// The invariant is "no word while anyone waits", and it is the pair that matters: a word with
    /// nobody waiting is fine, waiters with no word is fine, both is the broken state. The API
    /// cannot reach the broken one, so this writes the word directly.
    #[test]
    fn the_invariant_fails_only_for_a_word_with_a_waiter() {
        let mut a = node();
        let mut n: Notification<N> = Notification::new();
        assert!(n.invariant_holds(), "empty");
        n.word = 0b1;
        assert!(n.invariant_holds(), "a word, nobody waiting");
        n.word = 0;
        assert_eq!(n.wait(token(&mut a)), WaitVerdict::Blocked);
        assert!(n.invariant_holds(), "a waiter, no word");
        n.word = 0b1;
        assert!(!n.invariant_holds(), "a word and a waiter");
        n.word = 0;
        n.drain_waiters(|_| {});
    }

    /// Counted bits are readable without being taken, and bits that overlap what is already there
    /// merge into it; the bound path hands over the union, not the bits that differ.
    #[test]
    fn counted_bits_accumulate_as_a_union_and_are_readable() {
        let mut n: Notification<N> = Notification::new();
        assert_eq!(n.signal(0b0101, false), SignalVerdict::Counted);
        assert_eq!(n.word(), 0b0101);
        assert_eq!(n.signal(0b0100, false), SignalVerdict::Counted);
        assert_eq!(n.word(), 0b0101, "an overlapping bit is not a toggle");
        assert_eq!(n.signal(0b0110, true), SignalVerdict::ToBound(0b0111));
        assert_eq!(n.word(), 0);
    }
}
