//! **One IPC rendezvous**: its two wait queues and pending-signal count, and what a send or a
//! receive on it decided.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// One IPC rendezvous: two intrusive wait queues and the pending-signal count. At most one queue
/// is non-empty at any time. See `notes/inter-process-communication.md`.
///
/// # Examples
///
/// The three decisions, and the invariant across them. `T` stands in for the kernel's TCB, with
/// the same one link.
///
/// ```
/// # use core::ptr::NonNull;
/// # use intrusive_fifo::{Node, Unqueued};
/// # use inter_process_communication::{Rendezvous, Receive, Send};
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
///
/// // A receive with nobody sending queues, giving up its token; the caller blocks it.
/// assert_eq!(ep.receive(server_token), Receive::Blocked);
/// assert!(!ep.is_idle() && ep.one_queue_invariant());
///
/// // A send that finds a receiver rendezvouses instead of queueing, and both tokens come back.
/// match ep.send(client_token) {
///     Send::Rendezvous(receiver, sender) => {
///         assert!(receiver == NonNull::from(&mut server));
///         assert!(sender == NonNull::from(&mut client));
///     }
///     other => panic!("{other:?}"),
/// }
/// assert!(ep.is_idle() && ep.one_queue_invariant());
/// ```
pub struct Rendezvous<T: Node> {
    /// Senders blocked here, waiting for a receiver.
    pub(crate) senders: Fifo<T>,
    /// Receivers blocked here, waiting for a sender.
    pub(crate) receivers: Fifo<T>,
    /// Async signals that arrived with nobody waiting. Drained by the next receive, never lost.
    pub(crate) pending: u32,
    /// This rendezvous carries a hardware interrupt, so it takes no message. Set once by
    /// [`bind_to_interrupt`](Self::bind_to_interrupt) and never cleared; while it is set,
    /// [`send`](Self::send) answers [`Send::Refused`] (§101 (notification objects), as amended).
    ///
    /// Name: provisional (milestone 603 (an interrupt's endpoint refuses every send)): calef names
    /// public items, and this is read through the public methods.
    pub(crate) bound_to_interrupt: bool,
}

/// What a [`send`](Rendezvous::send) decided. Every verdict that leaves the sender off a queue
/// hands its token back, and the one that queues it does not.
pub enum Send<T> {
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
    /// Name: provisional (milestone 603): calef names public items.
    Refused(Unqueued<T>),
}

/// What a [`receive`](Rendezvous::receive) decided. As for [`Send`], a verdict that leaves the
/// receiver off a queue hands its token back.
pub enum Receive<T> {
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
impl<T> PartialEq for Send<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Send::Rendezvous(a, s), Send::Rendezvous(b, t)) => {
                a.as_non_null() == b.as_non_null() && s.as_non_null() == t.as_non_null()
            }
            (Send::Blocked, Send::Blocked) => true,
            (Send::Refused(a), Send::Refused(b)) => a.as_non_null() == b.as_non_null(),
            _ => false,
        }
    }
}

impl<T> Eq for Send<T> {}

impl<T> core::fmt::Debug for Send<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Send::Rendezvous(r, s) => f.debug_tuple("Rendezvous").field(r).field(s).finish(),
            Send::Blocked => f.write_str("Blocked"),
            Send::Refused(s) => f.debug_tuple("Refused").field(s).finish(),
        }
    }
}

impl<T> PartialEq for Receive<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Receive::Signal(a), Receive::Signal(b)) => a.as_non_null() == b.as_non_null(),
            (Receive::FromSender(a, r), Receive::FromSender(b, q)) => {
                a.as_non_null() == b.as_non_null() && r.as_non_null() == q.as_non_null()
            }
            (Receive::Blocked, Receive::Blocked) => true,
            _ => false,
        }
    }
}

impl<T> Eq for Receive<T> {}

impl<T> core::fmt::Debug for Receive<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Receive::Signal(r) => f.debug_tuple("Signal").field(r).finish(),
            Receive::FromSender(s, r) => f.debug_tuple("FromSender").field(s).field(r).finish(),
            Receive::Blocked => f.write_str("Blocked"),
        }
    }
}

impl<T: Node> Rendezvous<T> {
    /// An idle rendezvous: both wait queues empty, no pending signal. `const` so the kernel can
    /// build the rendezvous table at compile time rather than at boot.
    pub const fn new() -> Self {
        Self {
            senders: Fifo::new(),
            receivers: Fifo::new(),
            pending: 0,
            bound_to_interrupt: false,
        }
    }

    /// Make this the rendezvous a hardware interrupt is delivered to, for good. Every later
    /// [`send`](Self::send) is [`Send::Refused`]; [`signal`](Self::signal) and
    /// [`receive`](Self::receive) are unchanged. There is no unbind, because the kernel has none.
    ///
    /// Name: provisional (milestone 603): calef names public items.
    pub fn bind_to_interrupt(&mut self) {
        self.bound_to_interrupt = true;
    }

    /// Whether [`bind_to_interrupt`](Self::bind_to_interrupt) has been called on this rendezvous.
    ///
    /// Name: provisional (milestone 603): calef names public items.
    pub fn is_bound_to_interrupt(&self) -> bool {
        self.bound_to_interrupt
    }

    /// **At most one wait queue is ever non-empty.** The load-bearing invariant.
    ///
    /// Name: provisional, flagged 2026-09-24 by the boolean-predicate pass
    /// (design/naming/functions-that-answer-yes-or-no-worklist.md). It does not yet follow the Rust predicate
    /// rule calef ratified 2026-09-24; recommended `one_queue_invariant_holds`, because a noun
    /// phrase; the verb form keeps the invariant's name visible.
    pub fn one_queue_invariant(&self) -> bool {
        self.senders.is_empty() || self.receivers.is_empty()
    }

    /// **No thread is blocked on this rendezvous** (both wait queues empty). The pending signal count
    /// does not count: a signal holds no thread.
    pub fn is_idle(&self) -> bool {
        self.senders.is_empty() && self.receivers.is_empty()
    }

    /// Diagnostic: `(queued senders, queued receivers, pending signals)`. For a hang dump, a
    /// nonzero sender count with a zero receiver count on a request rendezvous is a stalled server.
    pub fn debug_counts(&self) -> (usize, usize, u32) {
        (self.senders.len(), self.receivers.len(), self.pending)
    }

    /// Empty both wait queues, handing every blocked thread to `f` with its token, for object
    /// revocation. The caller wakes each one with an error. Afterwards [`is_idle`](Self::is_idle)
    /// holds.
    pub fn drain_waiters(&mut self, mut f: impl FnMut(Unqueued<T>)) {
        while let Some(w) = self.senders.pop_front() {
            f(w);
        }
        while let Some(w) = self.receivers.pop_front() {
            f(w);
        }
    }

    /// Take one specific sender back off the queue, returning its token if it was there.
    ///
    /// For reaping a corpse parked as a sender, which must be unlinked before its TCB is freed.
    /// `victim` is compared by pointer and never dereferenced. The survivors keep their FIFO order,
    /// and the cost is the queue's length. See `notes/inter-process-communication.md` ("Taking a
    /// thread back off a queue").
    pub fn remove_sender(&mut self, victim: NonNull<T>) -> Option<Unqueued<T>> {
        let mut kept: Fifo<T> = Fifo::new();
        let mut found = None;
        while let Some(node) = self.senders.pop_front() {
            if node == victim {
                found = Some(node);
            } else {
                kept.push_back(node);
            }
        }
        self.senders = kept;
        found
    }

    /// Take one specific receiver back off the queue, returning its token if it was there. The
    /// twin of [`remove_sender`](Self::remove_sender), for a region teardown that ends a thread
    /// blocked in `RECEIVE` here. The kernel asks both queues, because a thread's recorded wait
    /// role does not say which queue holds it.
    pub fn remove_receiver(&mut self, victim: NonNull<T>) -> Option<Unqueued<T>> {
        let mut kept: Fifo<T> = Fifo::new();
        let mut found = None;
        while let Some(node) = self.receivers.pop_front() {
            if node == victim {
                found = Some(node);
            } else {
                kept.push_back(node);
            }
        }
        self.receivers = kept;
        found
    }

    /// A sender `me` arrives. Rendezvous with a waiting receiver if there is one, otherwise `me`
    /// joins the sender queue and the caller blocks it. On a rendezvous that carries an interrupt,
    /// [`Send::Refused`] with nothing touched.
    pub fn send(&mut self, me: Unqueued<T>) -> Send<T> {
        // Before a receiver is popped: the receiver here is the driver, and handing it a sender's
        // words is the forgery the refusal exists to stop.
        if self.bound_to_interrupt {
            Send::Refused(me)
        } else if let Some(receiver) = self.receivers.pop_front() {
            Send::Rendezvous(receiver, me)
        } else {
            self.senders.push_back(me);
            Send::Blocked
        }
    }

    /// A receiver `me` arrives. Drain a pending signal first (never lose one behind a later
    /// sender), then collect a queued sender, otherwise `me` joins the receiver queue (and the
    /// caller should block it).
    pub fn receive(&mut self, me: Unqueued<T>) -> Receive<T> {
        if self.pending > 0 {
            self.pending -= 1;
            Receive::Signal(me)
        } else if let Some(sender) = self.senders.pop_front() {
            Receive::FromSender(sender, me)
        } else {
            self.receivers.push_back(me);
            Receive::Blocked
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
    /// # use intrusive_fifo::{Node, Unqueued};
    /// # use inter_process_communication::{Rendezvous, Receive};
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
    /// let Receive::Signal(token) = ep.receive(token) else { panic!("not drained") };
    /// let Receive::Signal(token) = ep.receive(token) else { panic!("not drained") };
    /// // The third finds nothing left and queues.
    /// assert_eq!(ep.receive(token), Receive::Blocked);
    ///
    /// // And a signal arriving now wakes it, already dequeued, with its token.
    /// assert!(ep.signal().is_some_and(|t| t == NonNull::from(&mut driver)));
    /// assert!(ep.is_idle());
    /// ```
    pub fn signal(&mut self) -> Option<Unqueued<T>> {
        if let Some(receiver) = self.receivers.pop_front() {
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
