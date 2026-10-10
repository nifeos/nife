//! The synchronous-rendezvous state machine (DECISIONS §14, milestone 18; intrusive as of
//! milestone 14 phase A.3).
//!
//! This owns the decision core of `kernel/src/sched.rs`'s IPC: the two wait queues and the
//! pending-signal count, and what a send, a receive, or a signal *does* with them. The kernel
//! wraps it with the bookkeeping the queues cannot express (mailboxes, waking a thread onto a
//! run queue, the one-shot Reply that leaves a caller blocked); the *policy* lives here, proved,
//! and the scheduler calls it rather than hand-rolling the same branch six times.
//!
//! The wait queues are **intrusive** (`crates/intrusive_fifo`): generic over the node type, so in the
//! kernel a queue entry *is* the TCB, threaded through the same link the run queues use. One link
//! means one queue, so "a blocked thread waits on exactly one rendezvous" is a property of there
//! being one field, not a rule anyone keeps. The queues are the kernel's real rendezvous state, not
//! a model kept in sync; what changed at A.3 is only what a queue entry is (a TCB pointer, no
//! longer a `ThreadId` to be looked up) and that queueing can no longer allocate.
//!
//! The load-bearing invariant, unchanged since the original `Rendezvous`: **"at most one wait
//! queue is ever non-empty."** A sender that finds a receiver rendezvouses instead of joining a
//! queue, so a thread only queues when nobody was waiting for it. Every operation is proved to
//! preserve it, now over the real intrusive queues (the `Fifo`'s own FIFO correctness is proved
//! separately, in its crate; here we prove the *decisions* made over it).
//!
//! # Examples
//!
//! The three decisions an rendezvous makes, and the invariant holding across all of them. `T` is the
//! kernel's TCB; here it is a stand-in with the same one link, because one link is the whole reason
//! the invariant is structural rather than remembered.
//!
//! ```
//! use core::ptr::NonNull;
//! use intrusive_fifo::{Node, Unqueued};
//! use inter_process_communication::{Rendezvous, Receive, Send};
//!
//! struct ThreadControlBlock {
//!     next: Option<NonNull<ThreadControlBlock>>,
//! }
//!
//! // SAFETY: plain field storage, which is the whole of the `Node` contract.
//! unsafe impl Node for ThreadControlBlock {
//!     fn next(&self) -> Option<NonNull<Self>> {
//!         self.next
//!     }
//!     fn set_next(&mut self, next: Option<NonNull<Self>>) {
//!         self.next = next;
//!     }
//! }
//!
//! // Declared before the rendezvous, so they outlive it.
//! let mut server = ThreadControlBlock { next: None };
//! let mut client = ThreadControlBlock { next: None };
//! let mut ep: Rendezvous<ThreadControlBlock> = Rendezvous::new();
//! assert!(ep.is_idle());
//!
//! // Each thread's token, minted once. The kernel does this when a thread is created.
//! // SAFETY: both are live locals declared before `ep`, on no queue, each minted once, and this is
//! // the only accessor.
//! let (server_token, client_token) = unsafe {
//!     (Unqueued::new(NonNull::from(&mut server)), Unqueued::new(NonNull::from(&mut client)))
//! };
//!
//! // The server calls receive with nobody sending, so it queues, giving up its token. The caller
//! // blocks it.
//! let waiting = ep.receive(server_token);
//! assert_eq!(waiting, Receive::Blocked);
//! assert!(!ep.is_idle());
//! assert!(ep.one_queue_invariant());
//!
//! // Now a client sends. There is a receiver, so it **rendezvouses instead of queueing**, which is
//! // why at most one of the two queues is ever non-empty. Both tokens come back: the receiver's,
//! // because it left the queue, and the sender's, because it never joined one.
//! match ep.send(client_token) {
//!     Send::Rendezvous(receiver, sender) => {
//!         assert!(receiver == NonNull::from(&mut server));
//!         assert!(sender == NonNull::from(&mut client));
//!     }
//!     other => panic!("{other:?}"),
//! }
//! assert!(ep.is_idle()); // the receiver left the queue and the sender never joined one
//! assert!(ep.one_queue_invariant());
//! ```
//!
//! A signal is the operation that is deliberately **not** a rendezvous: it never queues the signaller
//! and it is never lost, which is what lets an interrupt handler use one.
//!
//! ```
//! # use core::ptr::NonNull;
//! # use intrusive_fifo::{Node, Unqueued};
//! # use inter_process_communication::{Rendezvous, Receive};
//! # struct ThreadControlBlock { next: Option<NonNull<ThreadControlBlock>> }
//! # unsafe impl Node for ThreadControlBlock {
//! #     fn next(&self) -> Option<NonNull<Self>> { self.next }
//! #     fn set_next(&mut self, next: Option<NonNull<Self>>) { self.next = next; }
//! # }
//! let mut driver = ThreadControlBlock { next: None };
//! let mut ep: Rendezvous<ThreadControlBlock> = Rendezvous::new();
//!
//! // Two interrupts arrive with nobody in receive. Neither is dropped; both are counted.
//! assert!(ep.signal().is_none());
//! assert!(ep.signal().is_none());
//!
//! // SAFETY: `driver` is a live local declared before `ep`, on no queue, minted once.
//! let token = unsafe { Unqueued::new(NonNull::from(&mut driver)) };
//!
//! // The driver's next two receives drain them, and it never blocks: each hands its token back.
//! let Receive::Signal(token) = ep.receive(token) else { panic!("not drained") };
//! let Receive::Signal(token) = ep.receive(token) else { panic!("not drained") };
//! // The third finds nothing left and queues.
//! assert_eq!(ep.receive(token), Receive::Blocked);
//!
//! // And a signal arriving now wakes it, already dequeued, with its token.
//! assert!(ep.signal().is_some_and(|t| t == NonNull::from(&mut driver)));
//! assert!(ep.is_idle());
//! ```
//!
//! Name: ratified 2026-09-18 (calef, `design/decisions/` §154), **deratifying the 2026-08-01
//! ratification** to do it, and performed 2026-09-19. Refused `ipc`.
//!
//! §154's test is whether the expansion is a phrase people actually say. "inter-process communication" is,
//! so it goes, where `pci` stays because "peripheral component interconnect" is not.
//!
//! The 2026-08-01 block called this one of the standard terms "already right and must not be
//! touched". That was an exemption rather than a test, and §154 records the three-layer
//! contradiction the exemptions left behind.
//!
//! **Performed 2026-09-19**, the last of §154's renames. No public type or fuzz target carried the
//! acronym, so only the crate moved. **IPC the concept did not move with it**: it keeps its name
//! when this crate is deleted, so the lock §118 named and its type, the syscall-path
//! `ipc_send`/`ipc_receive`/`ipc_call`/`ipc_reply` family, `notes/ipc-naming.md` and the word in prose
//! all stay.
//!
//! Census of lowercase `ipc` as a word, outside this file: 206 before, 138 after. Of the
//! survivors, 61 name the concept's own files (`notes/ipc-naming.md`,
//! `notes/ipc-tables-lock-inventory.md`, roadmap and decision slugs), 35 are in `design/decisions/`
//! or `design/naming.md`, which this rename did not edit, 25 are the old name in an account or a
//! measurement, 9 are the concept in code (`bench`'s "ipc server", `board_console`'s `ipc` field),
//! and 5 are `ipc::Endpoint` pointers that were already stale when `Endpoint` became `Rendezvous`
//! (§113) and were deliberately not repointed. The last three are a quoted command and the clause
//! beside it, and `README.md`'s crate list, whose other entries are stale too.

#![cfg_attr(not(test), no_std)]

use core::ptr::NonNull;

use intrusive_fifo::{Fifo, Node, Unqueued};

pub mod futex;
pub mod notification;
pub mod timer;

/// One IPC rendezvous: two intrusive wait queues and the pending-signal count.
pub struct Rendezvous<T: Node> {
    /// Senders blocked here, waiting for a receiver.
    senders: Fifo<T>,
    /// Receivers blocked here, waiting for a sender.
    receivers: Fifo<T>,
    /// Async signals that arrived with nobody waiting. Drained by the next receive, never lost.
    pending: u32,
    /// **This rendezvous carries a hardware interrupt, so it takes no message** (DECISIONS §101 (notification objects),
    /// amended 2026-09-26: calef's ruling B). Set once by [`bind_to_interrupt`](Self::bind_to_interrupt)
    /// and never cleared. While it is set, [`send`](Self::send) answers [`Send::Refused`] and
    /// touches nothing, so the only thing a receiver here can ever be handed is a
    /// [`signal`](Self::signal). That is what makes an interrupt's `w0 = 1` unforgeable by rule
    /// rather than by nobody having been handed a `WRITE` capability.
    ///
    /// It sits in the padding after `pending`, so the object did not grow.
    ///
    /// Name: provisional (milestone 603 (provisional), an interrupt's rendezvous refuses every
    /// send): calef names public items, and this is read through the public methods.
    bound_to_interrupt: bool,
}

/// What a [`send`](Rendezvous::send) decided.
///
/// **Every verdict that leaves the sender off a queue hands its token back** (milestone 139 (drive
/// the unsafe count down), round 10), and the one that queues it does not. That is the whole of
/// what moved here: the variants and their meanings are the ones they always were.
pub enum Send<T> {
    /// A receiver was waiting: rendezvous with this one (its token, since it has left the receiver
    /// queue), and the sender does not join a queue (its token, second).
    Rendezvous(Unqueued<T>, Unqueued<T>),
    /// Nobody was waiting: the sender is now queued on this rendezvous, holding its token.
    Blocked,
    /// **The rendezvous carries an interrupt and takes no message** (see
    /// [`bind_to_interrupt`](Rendezvous::bind_to_interrupt)). Nothing was queued and no receiver
    /// was taken: the rendezvous is exactly as it was. The caller turns this into an error and must
    /// not block.
    ///
    /// A variant rather than a check at each call site, so that every path that deposits into a
    /// rendezvous (the kernel has four today: `SEND`, `SEND_CAP`, `CALL`, and a §26 (the fault endpoint) death message)
    /// is made to say what it does here by the compiler, and a fifth cannot forget to.
    ///
    /// Name: provisional (milestone 603 (provisional)): calef names public items.
    ///
    /// Carries the sender's token back, since nothing was queued.
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

    /// **Make this the rendezvous a hardware interrupt is delivered to, for good** (DECISIONS §101,
    /// ruling B). From now on every [`send`](Self::send) is [`Send::Refused`]; [`signal`](Self::signal)
    /// and [`receive`](Self::receive) are unchanged. One-way: there is no unbind, because the kernel has
    /// none (`sched::bind_irq` only ever overwrites a route), and a rendezvous that once carried an
    /// interrupt is safer left refusing than reopened to senders a driver does not expect.
    ///
    /// Name: provisional (milestone 603 (provisional)): calef names public items.
    pub fn bind_to_interrupt(&mut self) {
        self.bound_to_interrupt = true;
    }

    /// Whether [`bind_to_interrupt`](Self::bind_to_interrupt) has been called on this rendezvous.
    ///
    /// Name: provisional (milestone 603 (provisional)): calef names public items.
    pub fn is_bound_to_interrupt(&self) -> bool {
        self.bound_to_interrupt
    }

    /// **At most one wait queue is ever non-empty.** The load-bearing invariant.
    ///
    /// Name: provisional, flagged 2026-09-24 by the boolean-predicate pass
    /// (design/naming/boolean-predicates-worklist.md). It does not yet follow the Rust predicate
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

    /// **Empty both wait queues, handing every blocked thread back to `f`** (object revocation): the
    /// rendezvous is about to be destroyed, so each waiter is popped off here, token and all (which
    /// is what lets `f` re-queue it onto a run queue), and the caller wakes it with an error. After
    /// this both queues are empty, so [`is_idle`](Self::is_idle) holds and the one-queue invariant
    /// trivially does.
    pub fn drain_waiters(&mut self, mut f: impl FnMut(Unqueued<T>)) {
        while let Some(w) = self.senders.pop_front() {
            f(w);
        }
        while let Some(w) = self.receivers.pop_front() {
            f(w);
        }
    }

    /// **Take one specific sender back off the queue**, returning its token if it was there.
    ///
    /// The one operation an intrusive `Fifo` deliberately does not offer (arbitrary remove), needed
    /// here for one reason: a **corpse** can be a queued sender. A supervised thread that dies with
    /// nobody in `RECEIVE` parks on its supervision rendezvous's sender queue with the death message in
    /// its mailbox (DECISIONS §26 implementation note 2), and its supervisor may then reap it
    /// (§32's rendezvous reap, or §16's `DESTROY`) *without* having collected the message. Freeing a
    /// TCB that is still linked into a queue leaves a dangling pointer the next `receive` would follow,
    /// so the reap has to unlink it first.
    ///
    /// Expressed as drain-and-repush over `pop_front`/`push_back` rather than as a `Fifo::remove`,
    /// which keeps the "one link, no arbitrary remove" contract intact in the queue itself: the cost
    /// is O(queued senders) on a teardown path, and the queue's own proved invariants are the only
    /// ones in play. FIFO order among the survivors is preserved.
    ///
    /// **Safe since milestone 139's round 10.** `victim` is compared by pointer and never
    /// dereferenced, and every *other* queued sender is re-pushed with the token its own pop just
    /// handed back, so nothing here asks the caller to promise anything. The removal is one of the
    /// three places a token comes from (a pop, a removal, thread creation): the victim's is
    /// returned, which is what lets the caller requeue or free it knowing it is on no queue.
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

    /// **Take one specific receiver back off the queue**, returning its token if it was there. The
    /// twin of [`remove_sender`](Self::remove_sender), and the same drain-and-repush for the same
    /// reason.
    ///
    /// It exists for milestone 133: `MemoryRegion::DESTROY` ends a resident thread that is
    /// permanently `Blocked`, and the commonest such thread is a server parked in `RECEIVE` on a
    /// rendezvous that belongs to somebody else. Its TCB is linked here, so the region's reclaim
    /// has to unlink it before freeing the page the TCB sits on, exactly as a corpse on a
    /// supervision rendezvous's sender queue does.
    ///
    /// **Why both twins rather than one `remove` taking a queue**, which was the tidier shape and
    /// was refused: the caller would then have to name the queue, and naming it means trusting the
    /// victim's recorded [`WaitRole`](crate) to say which one it is. The kernel's `CALL` caller
    /// that met no server is recorded as a `Reply` and *is* on the sender queue, so the role is a
    /// diagnostic rather than the fact. Two pointer-compared removes let the caller ask both
    /// queues and believe neither.
    ///
    /// Safe, for [`remove_sender`](Self::remove_sender)'s reason.
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
    /// joins the sender queue (and the caller should block it). On a rendezvous that carries an
    /// interrupt, neither: [`Send::Refused`], with nothing touched.
    ///
    /// The refusal is tested first, before a receiver is popped, because the receiver on an
    /// interrupt's rendezvous is the driver, and handing it a sender's words is the forgery.
    ///
    /// **Safe since milestone 139's round 10**: `me` is the sender's [`Unqueued`] token, so the
    /// "valid and on no queue" the caller used to promise here is the token's to carry. (The
    /// kernel's discipline behind the token: `me` is the running thread, its token sits on its own
    /// TCB (`thread::Thread::own_token`) until this call takes it, and a thread queued here is
    /// `Blocked`, which the reaper never touches.)
    pub fn send(&mut self, me: Unqueued<T>) -> Send<T> {
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
    /// caller should block it). Safe, for [`send`](Self::send)'s reason.
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
    /// lost. Safe: signalling queues nothing.
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

/// Machine-checked proofs of the rendezvous state machine (DECISIONS §14, milestone 18; restated
/// over the intrusive queues at milestone 14 phase A.3, so the rewire did not demote proved code
/// back to argued code).
///
/// Every operation is proved to preserve the one-queue invariant, and each decision is proved to
/// match the rule. These are inductive-step proofs: assume a valid state, apply one operation,
/// check. A non-empty queue is modeled with a single waiter, because the decision and the
/// invariant depend only on whether a queue is *empty*, never on its length: an operation pops
/// (only shrinking) and pushes only to a queue that was empty, so the emptiness pattern
/// transitions identically for one waiter or many. FIFO order within a queue is the `intrusive_fifo`
/// crate's own proof; these harnesses prove the decisions made over it.
///
/// # The obligations the one `unsafe` call in each harness discharges
///
/// Since milestone 139's round 10 the operations are safe and take [`Unqueued`] tokens, so the only
/// `unsafe` left is minting the tokens, once per harness, through [`mint`]. Its obligations are
/// stated here once rather than at each site.
///
/// **Every node outlives the rendezvous.** Each harness declares its `N`s in one `let` before it
/// declares `e`, and Rust drops locals in reverse declaration order, so the `Rendezvous` is destroyed
/// first. A node still parked on a queue when the harness returns was therefore valid for the whole
/// of its time there, which is rule 2 of the token's contract.
///
/// **Every node is on no queue when it is minted, and minted once.** Each `N::new()` starts with a
/// null link, `e` starts empty, and no harness mints the same node twice. That is why the harnesses
/// carry a separate `me` (and, in one case, a `me2`) rather than reusing `s` or `r`.
///
/// Both are properties the *harness* has, not properties Kani checks. Kani would catch a dangling
/// dereference if one of these were false and a proof reached it, but nothing here proves the
/// contract is kept; that is what the comments are for.
#[cfg(kani)]
mod verification {
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

    /// Mint `n`'s token.
    ///
    /// # Safety
    /// `n` is a harness local declared before the rendezvous, on no queue, and minted only once.
    unsafe fn mint(n: &mut N) -> Unqueued<N> {
        // SAFETY: this function's own contract is `Unqueued::new`'s.
        unsafe { Unqueued::new(NonNull::from(n)) }
    }

    /// Put `e` into an arbitrary valid state: at most one queue non-empty (modeled as one
    /// waiter), and a symbolic pending count. Safe now: the waiters arrive as tokens, so "valid,
    /// distinct and unqueued" is what they are rather than what this function asks for. A token
    /// that is not queued is dropped, which strands its node and nothing else.
    fn seed(e: &mut Rendezvous<N>, sender: Unqueued<N>, receiver: Unqueued<N>) {
        e.pending = kani::any();
        // Symbolic too, so every harness covers a rendezvous that carries an interrupt as well as
        // one that does not, including one bound after a sender had already queued.
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
        let (s, r, me) = unsafe { (mint(&mut s), mint(&mut r), mint(&mut me)) };
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
        let (s, r, me) = unsafe { (mint(&mut s), mint(&mut r), mint(&mut me)) };
        seed(&mut e, s, r);
        let _ = e.receive(me);
        assert!(e.one_queue_invariant());
    }

    /// Falsification: unfalsifiable. No minimal defect in `signal` can turn this harness red, and
    /// that is a fact about the operation rather than a gap in the effort. The invariant is "at
    /// most one queue is non-empty", and `signal` takes no node: its two branches pop a receiver
    /// (which can only empty a queue) and increment a counter. Nothing it can be mistakenly
    /// written to do enqueues anything, so every mutation of it either leaves the invariant true
    /// or is caught by a neighbouring harness instead. The harness earns its place as a guard
    /// against `signal` growing an enqueue path later, not as evidence about the code today.
    /// Recording a patch here would mean patching a sibling operation and filing the red under
    /// this name, which is the manufactured evidence DECISIONS §134 (a harness carries a
    /// machine-replayable falsification record, or it is not evidence) exists to refuse.
    #[kani::proof]
    fn signal_preserves_the_invariant() {
        let (mut s, mut r) = (N::new(), N::new());
        let mut e: Rendezvous<N> = Rendezvous::new();
        // SAFETY: two distinct fresh locals declared before `e`, each minted once.
        let (s, r) = unsafe { (mint(&mut s), mint(&mut r)) };
        seed(&mut e, s, r);
        let _ = e.signal();
        assert!(e.one_queue_invariant());
    }

    /// **A send rendezvouses exactly when a receiver was waiting**, and with exactly *that*
    /// receiver, else blocks. So a message is never dropped, a sender never blocks past a ready
    /// receiver, and the rendezvous partner is the queued thread and no other.
    ///
    /// Unless the rendezvous carries an interrupt (DECISIONS §101 (notification objects), ruling
    /// B, 2026-09-26): then the send is refused and changes nothing. Folded into this harness rather
    /// than given its own, because it is the same decision's other branch. Removing the refusal
    /// from `send` turns this red at both `!bound` assertions, checked 2026-09-26 by
    /// milestone 603 (an interrupt's endpoint refuses every send); the replayable record below is
    /// the older defect, since the convention holds one patch per harness.
    /// Falsification: replayable `crates/inter_process_communication/falsifications/verification.send_rendezvous_iff_a_receiver_waited.patch`
    #[kani::proof]
    fn send_rendezvous_iff_a_receiver_waited() {
        let (mut s, mut r, mut me) = (N::new(), N::new(), N::new());
        let receiver_ptr = NonNull::from(&mut r);
        let mut e: Rendezvous<N> = Rendezvous::new();
        // SAFETY: three distinct fresh locals declared before `e`, each minted once. `receiver_ptr`
        // is only ever compared, never dereferenced.
        let (s, r, me) = unsafe { (mint(&mut s), mint(&mut r), mint(&mut me)) };
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
    /// caller's own.** The obligation milestone 139's round 10 created: the kernel files the token
    /// a `send` or `receive` hands back onto the calling thread's TCB (`sched::hold_token`), so a
    /// verdict that returned the *partner's* token in the caller's place would leave the running
    /// thread holding a token for a thread that is about to be queued by its waker, which is two
    /// tokens for one node and the
    /// double-queue this type exists to make impossible. And a verdict that queued the caller and
    /// also returned its token would be the same defect from the other side.
    ///
    /// So for both operations, over every seeded state: the verdict that holds no token for the
    /// caller is exactly the one that grew the caller's queue, and every other verdict hands back a
    /// token naming the caller and leaves the caller's queue as it was.
    /// Falsification: replayable `crates/inter_process_communication/falsifications/verification.a_token_comes_back_exactly_when_its_node_did_not_queue.patch`
    #[kani::proof]
    fn a_token_comes_back_exactly_when_its_node_did_not_queue() {
        let (mut s, mut r, mut me) = (N::new(), N::new(), N::new());
        let me_ptr = NonNull::from(&mut me);
        let mut e: Rendezvous<N> = Rendezvous::new();
        // SAFETY: three distinct fresh locals declared before `e`, each minted once. `me_ptr` is
        // only ever compared.
        let (s, r, me) = unsafe { (mint(&mut s), mint(&mut r), mint(&mut me)) };
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
    /// whether or not the victim was ever there.
    ///
    /// Milestone 133 needed [`remove_receiver`](Rendezvous::remove_receiver) and asks both removes
    /// on every victim, because the kernel's recorded wait role and the queue that actually holds
    /// the thread genuinely disagree for a `CALL` caller that met no server. So the interesting
    /// case is not the hit, it is the **miss**: a drain-and-repush over a queue the victim is not
    /// on rebuilds that queue from nothing and must put it back exactly as it was. A version that
    /// dropped the survivors, or left both queues non-empty, would break the invariant every other
    /// proof in this module rests on.
    ///
    /// **This one needs a loop bound, and the two facts behind that are worth the paragraph**,
    /// because every other harness in this module is unbounded and finishes in a tenth of a
    /// second. The first shape of it ran both removes twice each and CBMC was still working after
    /// **seventeen minutes**; with `unwind(3)` and a single symbolic remove it takes 0.2 seconds.
    ///
    /// The bound is sound rather than a shortcut: Kani checks the unwinding assertion, so a loop
    /// that needed a fourth iteration would **fail** this proof rather than pass it quietly, and
    /// `seed` queues at most one waiter, so two is the most a drain-and-repush can take. What the
    /// bound costs is exactly what `seed` already costs everywhere in this module: the proof is
    /// over a rendezvous holding at most one waiter, and a queue of many is argued rather than
    /// proved.
    ///
    /// **One remove per run, chosen symbolically, rather than the four the kernel makes**, and
    /// nothing is lost by that: the removes share no state between calls, each reading one queue
    /// and writing it back, so proving one over an arbitrary seeded rendezvous covers a sequence.
    ///
    /// Since milestone 139's round 10 a hit returns the victim's token, so the harness also checks
    /// that a returned token names the victim: the kernel frees or requeues the node it names.
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
        let (s, r) = unsafe { (mint(&mut s), mint(&mut r)) };
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
        let (s, r, me) = unsafe { (mint(&mut s), mint(&mut r), mint(&mut me)) };
        seed(&mut e, s, r);
        if e.pending > 0 {
            let outcome = e.receive(me);
            assert!(matches!(outcome, Receive::Signal(_)));
        }
    }

    /// **A collected sender is forgotten by the rendezvous.** The rendezvous half of the one-shot
    /// Reply guarantee (DECISIONS §12): a `CALL`er queues as a sender and blocks; when a server's
    /// receive collects it, the pop is destructive, so afterwards the rendezvous holds no name for
    /// the caller in either queue and no later receive can produce it again. From that moment the
    /// kernel-minted Reply capability is the *only* name for the blocked caller anywhere, and the
    /// capability side (consume-on-use, proved in `crates/capability`) makes that name single-use.
    ///
    /// One waiter covers the general case here as everywhere in this module, plus one fact the
    /// queue cannot see: a blocked thread cannot run, so it cannot enqueue itself a second time.
    /// Stated through emptiness (the decision core's own vocabulary; a membership scan would hand
    /// the solver an unbounded loop for no added meaning).
    /// Falsification: replayable `crates/inter_process_communication/falsifications/verification.a_collected_sender_is_forgotten.patch`
    #[kani::proof]
    fn a_collected_sender_is_forgotten() {
        let (mut s, mut r, mut me, mut me2) = (N::new(), N::new(), N::new(), N::new());
        let mut e: Rendezvous<N> = Rendezvous::new();
        // SAFETY: four distinct fresh locals declared before `e`, each minted once. `me2` exists so
        // the second receive does not need `me`'s token back from the first.
        let (s, r, me, me2) =
            unsafe { (mint(&mut s), mint(&mut r), mint(&mut me), mint(&mut me2)) };
        seed(&mut e, s, r);
        if matches!(e.receive(me), Receive::FromSender(..)) {
            assert!(e.senders.is_empty() && e.receivers.is_empty());
            assert!(!matches!(e.receive(me2), Receive::FromSender(..)));
        }
    }
}

#[cfg(test)]
mod tests {
    //! Every node in these tests is minted once, through [`token`], on a `Box` declared before its
    //! `Rendezvous`; Rust drops locals in reverse declaration order, so the rendezvous goes first
    //! and a node parked on a queue when a test ends was valid for all of its time there. Since
    //! milestone 139's round 10 that is the only obligation left: the operations are safe, and a
    //! node whose token is queued cannot be passed again, which is what several of these tests used
    //! to argue site by site.

    use super::*;

    struct N {
        next: Option<NonNull<N>>,
    }

    // SAFETY: `next` and `set_next` read and write the same `next` field and nothing else, which is the whole of the `Node` contract.
    unsafe impl Node for N {
        fn next(&self) -> Option<NonNull<Self>> {
            self.next
        }
        fn set_next(&mut self, next: Option<NonNull<Self>>) {
            self.next = next;
        }
    }

    fn node() -> Box<N> {
        Box::new(N { next: None })
    }

    fn token(n: &mut Box<N>) -> Unqueued<N> {
        // SAFETY: a live boxed node declared before its rendezvous, on no queue, minted once per
        // test (see the module note).
        unsafe { Unqueued::new(NonNull::from(&mut **n)) }
    }

    /// The rendezvous, both orderings: whoever arrives first waits, the second completes the pair
    /// and gets the first: the very node, by identity, not a name to look up.
    #[test]
    fn sender_first_then_receiver_rendezvous() {
        let (mut s, mut r) = (node(), node());
        let (sp, rp) = (NonNull::from(&mut *s), NonNull::from(&mut *r));
        let (st, rt) = (token(&mut s), token(&mut r));
        let mut e: Rendezvous<N> = Rendezvous::new();

        assert_eq!(e.send(st), Send::Blocked); // nobody waiting: park the sender
        match e.receive(rt) {
            // receiver collects it, and keeps its own token
            Receive::FromSender(got, back) => assert!(got == sp && back == rp),
            other => panic!("{other:?}"),
        }
        assert!(e.one_queue_invariant());
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
        e.bind_to_interrupt();

        assert_eq!(e.receive(dt), Receive::Blocked);
        match e.send(ft) {
            Send::Refused(back) => assert!(back == fp, "the forger keeps its own token"),
            other => panic!("{other:?}"),
        }
        // Still parked, still the only waiter, and the interrupt still reaches it.
        assert_eq!(e.debug_counts(), (0, 1, 0));
        assert!(e.signal().is_some_and(|t| t == dp));
        assert!(e.is_idle());
    }

    #[test]
    fn receiver_first_then_sender_rendezvous() {
        let (mut s, mut r) = (node(), node());
        let (sp, rp) = (NonNull::from(&mut *s), NonNull::from(&mut *r));
        let (st, rt) = (token(&mut s), token(&mut r));
        let mut e: Rendezvous<N> = Rendezvous::new();

        assert_eq!(e.receive(rt), Receive::Blocked);
        match e.send(st) {
            // sender meets the waiter
            Send::Rendezvous(got, back) => assert!(got == rp && back == sp),
            other => panic!("{other:?}"),
        }
    }

    /// Two senders queue in FIFO order; two receivers drain them in the same order.
    #[test]
    fn senders_queue_fifo() {
        let (mut a, mut b, mut r) = (node(), node(), node());
        let (ap, bp) = (NonNull::from(&mut *a), NonNull::from(&mut *b));
        let (at, bt, rt) = (token(&mut a), token(&mut b), token(&mut r));
        let mut e: Rendezvous<N> = Rendezvous::new();

        assert_eq!(e.send(at), Send::Blocked);
        assert_eq!(e.send(bt), Send::Blocked);
        // The receiver's token comes back from each collect, and is what the next receive takes:
        // that it was never queued is now a fact the second call cannot be written without.
        let Receive::FromSender(first, rt) = e.receive(rt) else {
            panic!("no sender collected")
        };
        assert!(first == ap);
        let Receive::FromSender(second, _) = e.receive(rt) else {
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
        let Receive::Signal(rt) = e.receive(rt) else {
            panic!("the first counted signal was not drained")
        };
        let Receive::Signal(rt) = e.receive(rt) else {
            panic!("the second counted signal was not drained")
        };
        assert_eq!(e.receive(rt), Receive::Blocked);
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

        assert_eq!(e.send(at), Send::Blocked);
        assert_eq!(e.send(bt), Send::Blocked);
        assert!(!e.is_idle(), "parked senders hold the rendezvous live");

        let mut drained = Vec::new();
        e.drain_waiters(|w| drained.push(w.as_non_null()));
        assert_eq!(drained, [ap, bp]);
        assert!(e.is_idle());

        // The other queue drains through the same path: a receiver can be parked too.
        assert_eq!(e.receive(rt), Receive::Blocked);
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
            assert_eq!(e.send(t), Send::Blocked);
        }
        assert!(e.remove_sender(bp).is_some_and(|t| t == bp), "b was queued");
        assert_eq!(e.debug_counts().0, 2, "exactly one sender left the queue");
        let Receive::FromSender(first, rt) = e.receive(rt) else {
            panic!("a was not collected")
        };
        assert!(first == ap);
        let Receive::FromSender(second, rt) = e.receive(rt) else {
            panic!("c was not collected")
        };
        assert!(second == cp);
        assert_eq!(e.receive(rt), Receive::Blocked);

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

        assert_eq!(e.send(at), Send::Blocked);
        assert!(e.remove_sender(ap).is_some());
        assert!(e.is_idle(), "the rendezvous still holds a sender");
        assert_eq!(e.receive(rt), Receive::Blocked);
        // And it can be used again afterwards: push, pop, no ghost.
        assert!(e.one_queue_invariant());
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
            assert_eq!(e.receive(t), Receive::Blocked);
        }
        assert!(
            e.remove_receiver(bp).is_some_and(|t| t == bp),
            "b was queued"
        );
        assert_eq!(e.debug_counts().1, 2, "exactly one receiver left the queue");
        let Send::Rendezvous(first, st) = e.send(st) else {
            panic!("a was not met")
        };
        assert!(first == ap);
        a_again.replace(first);
        let Send::Rendezvous(second, _) = e.send(st) else {
            panic!("c was not met")
        };
        assert!(second == cp);

        // Not queued: a no-op that says so, and it does not reach into the *sender* queue either.
        let mut e2: Rendezvous<N> = Rendezvous::new();
        assert_eq!(e2.send(a_again.take().unwrap()), Send::Blocked);
        assert!(
            e2.remove_receiver(ap).is_none(),
            "a queued sender is not a receiver"
        );
        assert_eq!(e2.debug_counts().0, 1, "the sender queue was left alone");
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

        assert_eq!(e.receive(at), Receive::Blocked);
        assert!(e.remove_receiver(ap).is_some());
        assert!(e.is_idle(), "the rendezvous still holds a receiver");
        assert_eq!(e.send(st), Send::Blocked);
        assert!(e.one_queue_invariant());
    }

    /// A signal with a receiver waiting hands it back directly and counts nothing.
    #[test]
    fn a_signal_wakes_a_waiting_receiver() {
        let mut r = node();
        let rp = NonNull::from(&mut *r);
        let rt = token(&mut r);
        let mut e: Rendezvous<N> = Rendezvous::new();

        assert_eq!(e.receive(rt), Receive::Blocked);
        assert!(e.signal().is_some_and(|t| t == rp)); // the waiter, dequeued
        assert!(e.one_queue_invariant());
    }

    /// The manual `PartialEq` and `Debug` impls answer for themselves. Every use above compares
    /// equal values, so an eq stuck at `true` passed (milestone 85); different variants must
    /// disagree, and the rendering is the string a hang dump prints.
    #[test]
    fn verdict_variants_are_distinct_and_print_their_names() {
        let mut n: Vec<Box<N>> = (0..6).map(|_| node()).collect();
        let [s1, s2, s3, s4, s5, t1] = [0, 1, 2, 3, 4, 5].map(|i| token(&mut n[i]));
        assert_ne!(Send::<N>::Blocked, Send::Rendezvous(s1, s2));
        assert_ne!(Receive::Signal(s3), Receive::Blocked);
        let from = Receive::FromSender(s4, t1);
        assert!(format!("{from:?}").starts_with("FromSender"));
        assert_ne!(from, Receive::Signal(s5));
        assert_eq!(format!("{:?}", Send::<N>::Blocked), "Blocked");
        assert_eq!(format!("{:?}", Receive::<N>::Blocked), "Blocked");
    }

    /// A rendezvous starts as an ordinary one, and binding it to an interrupt is visible and stays.
    #[test]
    fn binding_to_an_interrupt_is_visible_and_one_way() {
        let mut e: Rendezvous<N> = Rendezvous::new();
        assert!(!e.is_bound_to_interrupt());
        e.bind_to_interrupt();
        assert!(e.is_bound_to_interrupt());
        e.bind_to_interrupt();
        assert!(e.is_bound_to_interrupt(), "binding twice does not undo it");
    }
}
