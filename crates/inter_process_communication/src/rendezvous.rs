//! **One IPC rendezvous**: its one wait queue, tagged with who waits on it, its pending-signal
//! count, and what a send or a receive on it decided.
//!
//! Name: ratified 2026-11-21 (UTC).

use super::*;

/// One IPC rendezvous: one intrusive wait queue tagged with whether senders or receivers wait on
/// it, and the pending-signal count. Senders and receivers never wait at once, because the tag
/// has room for one queue. See `notes/inter-process-communication.md`.
///
/// # Examples
///
/// The three decisions, and the invariant across them. `T` stands in for the kernel's TCB, with
/// the same one link.
///
/// ```
/// # use core::ptr::NonNull;
/// # use intrusive_queue::{Node, Unqueued};
/// # use inter_process_communication::{Rendezvous, ReceiveVerdict, SendVerdict};
/// # struct ThreadControlBlock { next: Option<NonNull<ThreadControlBlock>> }
/// # // SAFETY: plain field storage, which is the whole of the `Node` contract.
/// # unsafe impl Node for ThreadControlBlock {
/// #     fn next(&self) -> Option<NonNull<Self>> { self.next }
/// #     fn set_next(&mut self, next: Option<NonNull<Self>>) { self.next = next; }
/// # }
/// let mut server = ThreadControlBlock { next: None }; // declared first, so they outlive `ep`
/// let mut client = ThreadControlBlock { next: None };
/// let mut ep: Rendezvous<ThreadControlBlock> = Rendezvous::new();
/// // SAFETY: both are live locals declared before `ep`, on no queue, each minted once.
/// let (server_token, client_token) = unsafe {
///     (Unqueued::new(NonNull::from(&mut server)), Unqueued::new(NonNull::from(&mut client)))
/// };
/// // A receive with nobody sending queues, giving up its token; the caller blocks it.
/// assert_eq!(ep.receive(server_token), ReceiveVerdict::Blocked);
/// assert!(!ep.is_idle() && ep.one_queue_invariant_holds());
///
/// // A send that finds a receiver rendezvouses instead of queuing, and both tokens come back.
/// match ep.send(client_token) {
///     SendVerdict::Rendezvous(receiver, sender) => {
///         assert!(receiver == NonNull::from(&mut server));
///         assert!(sender == NonNull::from(&mut client));
///     }
///     other => panic!("{other:?}"),
/// }
/// assert!(ep.is_idle() && ep.one_queue_invariant_holds());
/// ```
///
/// Name: ratified 2026-11-21 (UTC).
pub struct Rendezvous<T: Node> {
    /// The threads blocked here, and which side of a rendezvous they wait for.
    pub(crate) waiting: Waiting<T>,
    /// Async signals that arrived with nobody waiting. Drained by the next receive, never lost.
    pub(crate) pending: u32,
    /// This rendezvous carries a hardware interrupt, so it takes no message. Set once by
    /// [`bind_to_interrupt`](Self::bind_to_interrupt) and never cleared; while it is set,
    /// [`send`](Self::send) answers [`SendVerdict::Refused`] (§101 (notification objects), as amended)
    /// and the sender queue is empty.
    ///
    /// Name: ratified 2026-11-21 (UTC). Private, read through [`is_bound_to_interrupt`](Self::is_bound_to_interrupt).
    pub(crate) bound_to_interrupt: bool,
}

/// Who is blocked on a [`Rendezvous`]. A tag that names a queue names one with a thread on it:
/// the last pop from a queue returns the tag to [`Nobody`](Self::Nobody), which is what
/// [`one_queue_invariant_holds`](Rendezvous::one_queue_invariant_holds) checks.
///
/// Name: provisional, minted 2026-11-22 (UTC) by 1095-finish-review, after seL4's endpoint state.
/// Considered: `State`, which says nothing about threads.
pub(crate) enum Waiting<T: Node> {
    /// No thread is blocked here.
    Nobody,
    /// Senders blocked here, waiting for a receiver.
    Senders(Queue<T>),
    /// Receivers blocked here, waiting for a sender.
    Receivers(Queue<T>),
}

/// Take `victim` out of `queue` by pointer, returning its token if it was there. The survivors
/// keep their FIFO order, and the cost is the queue's length.
fn take_out<T: Node>(queue: &mut Queue<T>, victim: NonNull<T>) -> Option<Unqueued<T>> {
    let mut kept: Queue<T> = Queue::new();
    let mut found = None;
    while let Some(node) = queue.pop_front() {
        if node == victim {
            found = Some(node);
        } else {
            kept.push_back(node);
        }
    }
    *queue = kept;
    found
}

/// What a hang dump reads from a [`Rendezvous`]: [`debug_counts`](Rendezvous::debug_counts).
///
/// Name: ratified 2026-11-21 (UTC), with its three fields. Named fields in place of a tuple whose positions a
/// reader had to remember. Refused: keeping the tuple.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendezvousCounts {
    /// Senders parked on the rendezvous.
    pub queued_senders: usize,
    /// Receivers parked on the rendezvous.
    pub queued_receivers: usize,
    /// Signals counted and not yet received.
    pub pending_signals: u32,
}

/// What a [`send`](Rendezvous::send) decided. Every verdict that leaves the sender off a queue
/// hands its token back, and the one that queues it does not.
///
/// Name: ratified 2026-11-21 (UTC), with its variants. The `Verdict` suffix keeps the type from sharing a name
/// with `core::marker::Send` in the same namespace, and is the word this crate's prose already uses
/// for what an operation decided. Refused: `Sent` and `Received`, `SendOutcome`, and keeping the
/// bare name.
pub enum SendVerdict<T> {
    /// A receiver was waiting: rendezvous with this one (its token, since it has left the receiver
    /// queue), and the sender does not join a queue (its token, second).
    Rendezvous(Unqueued<T>, Unqueued<T>),
    /// Nobody was waiting: the sender is now queued on this rendezvous, holding its token.
    Blocked,
    /// The rendezvous carries an interrupt and takes no message (see
    /// [`bind_to_interrupt`](Rendezvous::bind_to_interrupt)). Nothing was queued and no receiver
    /// was taken, and the sender's token comes back. The caller turns this into an error and must
    /// not block.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    Refused(Unqueued<T>),
}

/// What a [`receive`](Rendezvous::receive) decided. As for [`SendVerdict`], a verdict that leaves the
/// receiver off a queue hands its token back.
///
/// Name: ratified 2026-11-21 (UTC), with its variants. The `Verdict` suffix matches `SendVerdict`, which needs
/// it so the type does not share a name with `core::marker::Send`, and is the word this crate's
/// prose already uses for what an operation decided. Refused: `Received`, `ReceiveOutcome`, and
/// keeping the bare name.
pub enum ReceiveVerdict<T> {
    /// A pending async signal was drained; the receiver does not block (its token).
    Signal(Unqueued<T>),
    /// This queued sender was collected (its token, since it has left the sender queue); the caller
    /// decides whether to wake it. The receiver's own token is second.
    FromSender(Unqueued<T>, Unqueued<T>),
    /// Nobody was waiting: the receiver is now queued on this rendezvous, holding its token.
    Blocked,
}

// Manual impls rather than derives: a derive would demand `T: PartialEq`/`T: Debug` even though
// only the *pointer* is stored and compared, and the kernel's `T` (a TCB) is neither. Two verdicts
// are equal when they decided the same thing about the same nodes.
impl<T> PartialEq for SendVerdict<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (SendVerdict::Rendezvous(a, s), SendVerdict::Rendezvous(b, t)) => {
                a.as_non_null() == b.as_non_null() && s.as_non_null() == t.as_non_null()
            }
            (SendVerdict::Blocked, SendVerdict::Blocked) => true,
            (SendVerdict::Refused(a), SendVerdict::Refused(b)) => {
                a.as_non_null() == b.as_non_null()
            }
            _ => false,
        }
    }
}

impl<T> Eq for SendVerdict<T> {}

impl<T> core::fmt::Debug for SendVerdict<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SendVerdict::Rendezvous(r, s) => f.debug_tuple("Rendezvous").field(r).field(s).finish(),
            SendVerdict::Blocked => f.write_str("Blocked"),
            SendVerdict::Refused(s) => f.debug_tuple("Refused").field(s).finish(),
        }
    }
}

impl<T> PartialEq for ReceiveVerdict<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ReceiveVerdict::Signal(a), ReceiveVerdict::Signal(b)) => {
                a.as_non_null() == b.as_non_null()
            }
            (ReceiveVerdict::FromSender(a, r), ReceiveVerdict::FromSender(b, q)) => {
                a.as_non_null() == b.as_non_null() && r.as_non_null() == q.as_non_null()
            }
            (ReceiveVerdict::Blocked, ReceiveVerdict::Blocked) => true,
            _ => false,
        }
    }
}

impl<T> Eq for ReceiveVerdict<T> {}

impl<T> core::fmt::Debug for ReceiveVerdict<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ReceiveVerdict::Signal(r) => f.debug_tuple("Signal").field(r).finish(),
            ReceiveVerdict::FromSender(s, r) => {
                f.debug_tuple("FromSender").field(s).field(r).finish()
            }
            ReceiveVerdict::Blocked => f.write_str("Blocked"),
        }
    }
}

impl<T: Node> Rendezvous<T> {
    /// An idle rendezvous: nobody waiting, no pending signal. `const` so the kernel can
    /// build the rendezvous table at compile time rather than at boot.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub const fn new() -> Self {
        Self {
            waiting: Waiting::Nobody,
            pending: 0,
            bound_to_interrupt: false,
        }
    }

    /// Make this the rendezvous a hardware interrupt is delivered to, for good, refusing the
    /// senders already parked here. Every later [`send`](Self::send) is [`SendVerdict::Refused`];
    /// [`signal`](Self::signal) and [`receive`](Self::receive) are unchanged. There is no unbind,
    /// because the kernel has none.
    ///
    /// `refuse` is handed each sender queued before the bind, in FIFO order, with its token; the
    /// caller wakes each one with the error a refused send answers. Afterwards the sender queue is
    /// empty, and since `send` refuses from now on it stays empty, so no [`receive`](Self::receive)
    /// here can return [`ReceiveVerdict::FromSender`]. Queued receivers stay: they wait for the interrupt.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub fn bind_to_interrupt(&mut self, mut refuse: impl FnMut(Unqueued<T>)) {
        self.bound_to_interrupt = true;
        while let Some(sender) = self.take_sender() {
            refuse(sender);
        }
    }

    /// Whether [`bind_to_interrupt`](Self::bind_to_interrupt) has been called on this rendezvous.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub fn is_bound_to_interrupt(&self) -> bool {
        self.bound_to_interrupt
    }

    /// **At most one wait queue is ever non-empty, and the tag names a queue only while a thread
    /// is on it.** The layout holds the first half, since [`Waiting`] has room for one queue.
    /// This checks the second: the last pop from a queue returns the tag to `Nobody`, so
    /// [`is_idle`](Self::is_idle), which reads the tag, never calls an empty rendezvous busy.
    ///
    /// Name: ratified 2026-11-21 (UTC). The verb form the Rust predicate rule asks for, as its notification twin
    /// `invariant_holds` has. Refused: `is_one_queue`, which reads as a question about the queues
    /// and not about the invariant.
    pub fn one_queue_invariant_holds(&self) -> bool {
        match &self.waiting {
            Waiting::Nobody => true,
            Waiting::Senders(queue) | Waiting::Receivers(queue) => !queue.is_empty(),
        }
    }

    /// **No thread is blocked on this rendezvous** (the tag says nobody waits). The pending signal count
    /// does not count: a signal holds no thread.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub fn is_idle(&self) -> bool {
        matches!(self.waiting, Waiting::Nobody)
    }

    /// Diagnostic: how many senders and receivers are queued and how many signals are pending. For
    /// a hang dump, a nonzero sender count with a zero receiver count on a request rendezvous is a
    /// stalled server.
    ///
    /// Name: ratified 2026-11-21 (UTC), with [`RendezvousCounts`] and its three fields, which replace a tuple
    /// whose positions a reader had to remember.
    pub fn debug_counts(&self) -> RendezvousCounts {
        let (queued_senders, queued_receivers) = match &self.waiting {
            Waiting::Nobody => (0, 0),
            Waiting::Senders(senders) => (senders.len(), 0),
            Waiting::Receivers(receivers) => (0, receivers.len()),
        };
        RendezvousCounts {
            queued_senders,
            queued_receivers,
            pending_signals: self.pending,
        }
    }

    /// Empty the wait queue, handing every blocked thread to `f` with its token, for object
    /// revocation. The caller wakes each one with an error. Afterwards [`is_idle`](Self::is_idle)
    /// holds.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub fn drain_waiters(&mut self, mut f: impl FnMut(Unqueued<T>)) {
        match core::mem::replace(&mut self.waiting, Waiting::Nobody) {
            Waiting::Nobody => {}
            Waiting::Senders(mut queue) | Waiting::Receivers(mut queue) => {
                while let Some(w) = queue.pop_front() {
                    f(w);
                }
            }
        }
    }

    /// Take one specific sender back off the queue, returning its token if it was there.
    ///
    /// For reaping a corpse parked as a sender, which must be unlinked before its TCB is freed.
    /// `victim` is compared by pointer and never dereferenced. The survivors keep their FIFO order,
    /// and the cost is the queue's length. See `notes/inter-process-communication.md` ("Taking a
    /// thread back off a queue").
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub fn remove_sender(&mut self, victim: NonNull<T>) -> Option<Unqueued<T>> {
        let Waiting::Senders(senders) = &mut self.waiting else {
            return None;
        };
        let found = take_out(senders, victim);
        if senders.is_empty() {
            self.waiting = Waiting::Nobody;
        }
        found
    }

    /// Take one specific receiver back off the queue, returning its token if it was there. The
    /// twin of [`remove_sender`](Self::remove_sender), for a region teardown that ends a thread
    /// blocked in `RECEIVE` here. The kernel asks both queues, because a thread's recorded wait
    /// role does not say which queue holds it.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub fn remove_receiver(&mut self, victim: NonNull<T>) -> Option<Unqueued<T>> {
        let Waiting::Receivers(receivers) = &mut self.waiting else {
            return None;
        };
        let found = take_out(receivers, victim);
        if receivers.is_empty() {
            self.waiting = Waiting::Nobody;
        }
        found
    }

    /// Pop the first receiver, if receivers wait here, and return the tag to `Nobody` after the
    /// last.
    fn take_receiver(&mut self) -> Option<Unqueued<T>> {
        let Waiting::Receivers(receivers) = &mut self.waiting else {
            return None;
        };
        let receiver = receivers.pop_front();
        if receivers.is_empty() {
            self.waiting = Waiting::Nobody;
        }
        receiver
    }

    /// Pop the first sender, if senders wait here, and return the tag to `Nobody` after the last.
    fn take_sender(&mut self) -> Option<Unqueued<T>> {
        let Waiting::Senders(senders) = &mut self.waiting else {
            return None;
        };
        let sender = senders.pop_front();
        if senders.is_empty() {
            self.waiting = Waiting::Nobody;
        }
        sender
    }

    /// Queue `me` as a sender. Called only after [`take_receiver`](Self::take_receiver) found
    /// nobody, so the tag is `Senders` or `Nobody`.
    fn queue_sender(&mut self, me: Unqueued<T>) {
        match &mut self.waiting {
            Waiting::Senders(senders) => senders.push_back(me),
            Waiting::Nobody | Waiting::Receivers(_) => {
                debug_assert!(self.is_idle(), "a sender queued past a waiting receiver");
                let mut senders = Queue::new();
                senders.push_back(me);
                self.waiting = Waiting::Senders(senders);
            }
        }
    }

    /// Queue `me` as a receiver. Called only after [`take_sender`](Self::take_sender) found
    /// nobody, so the tag is `Receivers` or `Nobody`.
    fn queue_receiver(&mut self, me: Unqueued<T>) {
        match &mut self.waiting {
            Waiting::Receivers(receivers) => receivers.push_back(me),
            Waiting::Nobody | Waiting::Senders(_) => {
                debug_assert!(self.is_idle(), "a receiver queued past a waiting sender");
                let mut receivers = Queue::new();
                receivers.push_back(me);
                self.waiting = Waiting::Receivers(receivers);
            }
        }
    }

    /// A sender `me` arrives. Rendezvous with a waiting receiver if there is one, otherwise `me`
    /// joins the sender queue and the caller blocks it. On a rendezvous that carries an interrupt,
    /// [`SendVerdict::Refused`] with nothing touched.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub fn send(&mut self, me: Unqueued<T>) -> SendVerdict<T> {
        // Before a receiver is popped: the receiver here is the driver, and handing it a sender's
        // words is the forgery the refusal exists to stop.
        if self.bound_to_interrupt {
            SendVerdict::Refused(me)
        } else if let Some(receiver) = self.take_receiver() {
            SendVerdict::Rendezvous(receiver, me)
        } else {
            self.queue_sender(me);
            SendVerdict::Blocked
        }
    }

    /// A receiver `me` arrives. Drain a pending signal first (never lose one behind a later
    /// sender), then collect a queued sender, otherwise `me` joins the receiver queue (and the
    /// caller should block it).
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub fn receive(&mut self, me: Unqueued<T>) -> ReceiveVerdict<T> {
        if self.pending > 0 {
            self.pending -= 1;
            ReceiveVerdict::Signal(me)
        } else if let Some(sender) = self.take_sender() {
            ReceiveVerdict::FromSender(sender, me)
        } else {
            self.queue_receiver(me);
            ReceiveVerdict::Blocked
        }
    }

    /// An async signal arrives. Wake a waiting receiver (returned, already dequeued), or count it
    /// for the next receive. **Not a rendezvous:** it never joins the sender queue and is never
    /// lost.
    ///
    /// # Examples
    ///
    /// Two interrupts with nobody receiving are counted, not dropped, and drained without blocking:
    ///
    /// ```
    /// # use core::ptr::NonNull;
    /// # use intrusive_queue::{Node, Unqueued};
    /// # use inter_process_communication::{Rendezvous, ReceiveVerdict};
    /// # struct ThreadControlBlock { next: Option<NonNull<ThreadControlBlock>> }
    /// # unsafe impl Node for ThreadControlBlock {
    /// #     fn next(&self) -> Option<NonNull<Self>> { self.next }
    /// #     fn set_next(&mut self, next: Option<NonNull<Self>>) { self.next = next; }
    /// # }
    /// let mut driver = ThreadControlBlock { next: None };
    /// let mut ep: Rendezvous<ThreadControlBlock> = Rendezvous::new();
    ///
    /// // Two interrupts arrive with nobody in receive. Neither is dropped; both are counted.
    /// assert!(ep.signal().is_none());
    /// assert!(ep.signal().is_none());
    ///
    /// // SAFETY: `driver` is a live local declared before `ep`, on no queue, minted once.
    /// let token = unsafe { Unqueued::new(NonNull::from(&mut driver)) };
    ///
    /// // The driver's next two receives drain them, and it never blocks: each hands its token back.
    /// let ReceiveVerdict::Signal(token) = ep.receive(token) else { panic!("not drained") };
    /// let ReceiveVerdict::Signal(token) = ep.receive(token) else { panic!("not drained") };
    /// // The third finds nothing left and queues.
    /// assert_eq!(ep.receive(token), ReceiveVerdict::Blocked);
    ///
    /// // And a signal arriving now wakes it, already dequeued, with its token.
    /// assert!(ep.signal().is_some_and(|t| t == NonNull::from(&mut driver)));
    /// assert!(ep.is_idle());
    /// ```
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub fn signal(&mut self) -> Option<Unqueued<T>> {
        if let Some(receiver) = self.take_receiver() {
            Some(receiver)
        } else {
            self.pending = self.pending.saturating_add(1);
            None
        }
    }
}

impl<T: Node> Default for Rendezvous<T> {
    fn default() -> Self {
        Self::new()
    }
}
