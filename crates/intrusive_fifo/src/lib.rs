//! An intrusive FIFO queue: the link lives inside the node, so push and pop cannot allocate or
//! fail, and a pop hands back the node itself. The kernel's run queues and migration inboxes use
//! it. Nothing here checks rule 2 below. See notes/intrusive-queues.md.
//!
//! # The caller's contract
//!
//! 1. A node is on at most one queue until popped; a push consumes its [`Unqueued`] token.
//! 2. A node outlives its time on the queue; stated once, at [`Unqueued::new`].
//! 3. The caller serializes all access to a queue and its links (`&mut self`, for the queue).
//!
//! Name: ratified 2026-08-23 (calef, a kernel-dependency crate naming review). Renamed from
//! `intrusive`, an adjective without its noun; the primary type is `Fifo<T: Node>`.

#![cfg_attr(not(test), no_std)]

use core::ptr::NonNull;

/// A type that carries its own queue link.
///
/// # Safety
///
/// `next`/`set_next` must be plain storage: reading back exactly what was stored, touching
/// nothing else. The queue threads its structure through these two methods, so a clever
/// implementation is a corrupted queue.
///
/// # Examples
///
/// The implementation every node writes, and the only correct one:
///
/// ```
/// use core::ptr::NonNull;
/// use intrusive_fifo::Node;
///
/// struct Thread {
///     tid: u32,
///     next: Option<NonNull<Thread>>,
/// }
///
/// // SAFETY: `next` and `set_next` are plain field storage and touch nothing else, which is the
/// // whole of the `Node` contract. A clever implementation here is a corrupted queue.
/// unsafe impl Node for Thread {
///     fn next(&self) -> Option<NonNull<Self>> {
///         self.next
///     }
///     fn set_next(&mut self, next: Option<NonNull<Self>>) {
///         self.next = next;
///     }
/// }
/// ```
pub unsafe trait Node: Sized {
    /// Read back the link stored by the most recent [`set_next`](Self::set_next).
    fn next(&self) -> Option<NonNull<Self>>;
    /// Store the link. Plain storage only; see the trait's safety section.
    fn set_next(&mut self, next: Option<NonNull<Self>>);
}

/// Proof that one node is on no queue, and the only thing a queue will take.
///
/// Neither `Clone` nor `Copy`: [`Fifo::push_back`] consumes it and [`Fifo::pop_front`] hands it
/// back, so a node cannot be on two queues without `unsafe`. `#[repr(transparent)]` over its
/// [`NonNull`], so `Option<Unqueued<T>>` is one pointer wide. It cannot carry lifetime (rule 2),
/// which [`Unqueued::new`] discharges once. A dropped token strands its node. See
/// notes/intrusive-queues.md.
///
/// Name: provisional (milestone 139, round 10, 2026-10-08 UTC). `Unqueued` is the name option C on
/// #1842 carried when calef chose that option; whether choosing it ratified the name is his to say.
/// `new`, `as_ptr` and `as_non_null` are this lane's: calef names public items.
#[repr(transparent)]
#[must_use = "a dropped token strands its node: nothing can queue it again without minting another"]
pub struct Unqueued<T>(NonNull<T>);

impl<T> Unqueued<T> {
    /// Mint the token for `node`. The one `unsafe` step in queueing.
    ///
    /// # Safety
    ///
    /// - `node` is valid to read and write, and stays valid for as long as this token, or any
    ///   queue it is pushed onto, can still reach it (rule 2).
    /// - `node` is on no queue, and no other `Unqueued` for it exists or will be minted while this
    ///   one, or the queue that consumes it, is alive (rule 1: this is what makes the token unique).
    /// - While a queue holds `node`, nothing but the queue writes its link, and no reference to
    ///   `node` is live across a queue operation (rule 3).
    #[inline(always)]
    pub const unsafe fn new(node: NonNull<T>) -> Self {
        Self(node)
    }

    /// The node, as a pointer. Safe to take: holding a pointer grants nothing, and dereferencing it
    /// is still the caller's `unsafe`.
    #[inline(always)]
    pub const fn as_non_null(&self) -> NonNull<T> {
        self.0
    }

    /// The node, as a raw pointer. See [`as_non_null`](Self::as_non_null).
    #[inline(always)]
    pub const fn as_ptr(&self) -> *mut T {
        self.0.as_ptr()
    }
}

impl<T> PartialEq<NonNull<T>> for Unqueued<T> {
    fn eq(&self, other: &NonNull<T>) -> bool {
        self.0 == *other
    }
}

impl<T> core::fmt::Debug for Unqueued<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Unqueued").field(&self.0).finish()
    }
}

/// The queue: two pointers into nodes it does not own, plus a count.
///
/// FIFO because the scheduler's queues are round-robin: threads leave in the order they arrived.
///
/// # Examples
///
/// A run queue: threads leave in the order they arrived.
///
/// ```
/// # use core::ptr::NonNull;
/// # use intrusive_fifo::{Fifo, Node, Unqueued};
/// # struct Thread { tid: u32, next: Option<NonNull<Thread>> }
/// # // SAFETY: plain field storage, which is the whole of the `Node` contract.
/// # unsafe impl Node for Thread {
/// #     fn next(&self) -> Option<NonNull<Self>> { self.next }
/// #     fn set_next(&mut self, next: Option<NonNull<Self>>) { self.next = next; }
/// # }
/// // Rule 2, as drop order: the nodes are declared before the queue, so they outlive it.
/// let mut a = Thread { tid: 1, next: None };
/// let mut b = Thread { tid: 2, next: None };
/// let mut q: Fifo<Thread> = Fifo::new();
///
/// // SAFETY: both are live locals declared before `q`, on no queue and with no token yet.
/// let (ta, tb) = unsafe { (Unqueued::new(NonNull::from(&mut a)), Unqueued::new(NonNull::from(&mut b))) };
/// q.push_back(ta);
/// q.push_back(tb);
/// assert_eq!(q.len(), 2);
///
/// let first = q.pop_front().unwrap();
/// // SAFETY: the token's node is one of the live locals above.
/// assert_eq!(unsafe { (*first.as_ptr()).tid }, 1);
///
/// // A drained-then-refilled queue is where the classic tail-pointer bug lives.
/// q.push_back(first);
/// let order: Vec<u32> = core::iter::from_fn(|| q.pop_front())
///     // SAFETY: each token's node is one of the live locals above.
///     .map(|t| unsafe { (*t.as_ptr()).tid })
///     .collect();
/// assert_eq!(order, vec![2, 1]);
/// ```
pub struct Fifo<T: Node> {
    head: Option<NonNull<T>>,
    tail: Option<NonNull<T>>,
    len: usize,
}

// SAFETY: the queue owns no data; it holds nodes only under the caller's contract (rule 3: all
// access serialized by the caller). Same justification as the slab allocator's Send: sharing is
// the caller's problem, solved with a lock or a single-owner rule.
unsafe impl<T: Node> Send for Fifo<T> {}

impl<T: Node> Fifo<T> {
    /// An empty queue.
    pub const fn new() -> Self {
        Self {
            head: None,
            tail: None,
            len: 0,
        }
    }

    /// Whether the queue currently holds no nodes.
    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    /// The number of nodes currently queued. O(1): maintained on every push and pop.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Append a node, consuming its token. Safe: the token is the proof that the node is valid
    /// and on no queue, and taking it by value is what stops the same node being pushed twice.
    ///
    /// # Examples
    ///
    /// A token is not `Clone`, so pushing one node onto two queues does not compile:
    ///
    /// ```compile_fail
    /// # use core::ptr::NonNull;
    /// # use intrusive_fifo::{Fifo, Node, Unqueued};
    /// # struct T { next: Option<NonNull<T>> }
    /// # unsafe impl Node for T {
    /// #     fn next(&self) -> Option<NonNull<Self>> { self.next }
    /// #     fn set_next(&mut self, next: Option<NonNull<Self>>) { self.next = next; }
    /// # }
    /// # let mut b = T { next: None };
    /// # let (mut ready, mut inbox) = (Fifo::<T>::new(), Fifo::<T>::new());
    /// // SAFETY: `b` is live, on no queue, and has no token yet.
    /// let token = unsafe { Unqueued::new(NonNull::from(&mut b)) };
    /// ready.push_back(token);
    /// inbox.push_back(token); // error[E0382]: use of moved value: `token`
    /// ```
    pub fn push_back(&mut self, node: Unqueued<T>) {
        let node = node.0;
        // SAFETY: valid and on no queue, which is what `Unqueued::new`'s caller promised for this
        // token; and the token is consumed, so no other push can link this node until a pop.
        unsafe { (*node.as_ptr()).set_next(None) };

        match self.tail {
            // SAFETY: `tail` was pushed earlier with its token and not yet popped, so it is valid.
            Some(tail) => unsafe { (*tail.as_ptr()).set_next(Some(node)) },
            None => self.head = Some(node),
        }
        self.tail = Some(node);
        // Wrapping, on every IPC (release builds check overflow, notes/overflow-checks.md): `len`
        // counts distinct live linked nodes of nonzero size, so it never reaches `usize::MAX`.
        self.len = self.len.wrapping_add(1);
    }

    /// Detach and return the oldest node, with its token. The returned node's link is cleared: it
    /// leaves the queue carrying no dangling reference into it, and the token says it may be
    /// queued again.
    pub fn pop_front(&mut self) -> Option<Unqueued<T>> {
        let node = self.head?;
        // SAFETY: every node between head and tail was pushed with a token and is still valid
        // (rule 2, the token's minting contract); we are the only accessor (rule 3).
        unsafe {
            self.head = (*node.as_ptr()).next();
            if self.head.is_none() {
                self.tail = None;
            }
            (*node.as_ptr()).set_next(None);
        }
        // Wrapping: `head` was `Some`, so `len >= 1`. The Kani proof below holds `len` to the
        // model's count after every step, so a wrapped decrement would fail it.
        self.len = self.len.wrapping_sub(1);
        Some(Unqueued(node))
    }
}

impl<T: Node> Default for Fifo<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Machine-checked proofs (DECISIONS §14 (the project's direction)). The harness drives the real
/// `Fifo` with a symbolic operation sequence against a trivially-correct model (an array and two
/// counters), covering every push/pop pattern up to the bound, the regrowth-from-empty transitions
/// included.
#[cfg(kani)]
mod verification {
    use super::*;

    struct N {
        next: Option<NonNull<N>>,
        tag: usize,
    }

    // SAFETY: `next` and `set_next` are plain field storage and touch nothing else, which is the
    // whole of the `Node` contract.
    unsafe impl Node for N {
        fn next(&self) -> Option<NonNull<Self>> {
            self.next
        }
        fn set_next(&mut self, next: Option<NonNull<Self>>) {
            self.next = next;
        }
    }

    /// FIFO order, no loss, no invention, across every operation sequence. Six symbolic
    /// steps over three nodes: each step pushes some not-currently-queued node or pops. The
    /// model records the order tags went in; the queue must hand them back in exactly that
    /// order, agree about emptiness and length at every step, and never dereference a stale
    /// link (which Kani would report as an invalid pointer access).
    /// Falsification: replayable `crates/intrusive_fifo/falsifications/verification.any_push_pop_interleaving_is_fifo_and_lossless.patch`
    #[kani::proof]
    fn any_push_pop_interleaving_is_fifo_and_lossless() {
        let mut nodes = [
            N { next: None, tag: 0 },
            N { next: None, tag: 1 },
            N { next: None, tag: 2 },
        ];
        // One token per node, minted from references: the same API the kernel calls. A token is
        // in `held` while its node is off the queue.
        //
        // SAFETY: three distinct live locals declared before `q`, each on no queue, each minted
        // once, and the harness is the only accessor.
        let mut held: [Option<Unqueued<N>>; 3] = unsafe {
            [
                Some(Unqueued::new(NonNull::from(&mut nodes[0]))),
                Some(Unqueued::new(NonNull::from(&mut nodes[1]))),
                Some(Unqueued::new(NonNull::from(&mut nodes[2]))),
            ]
        };

        let mut q: Fifo<N> = Fifo::new();

        // The model: a ring of tags in push order, and which nodes are currently queued.
        let mut model = [usize::MAX; 6];
        let (mut m_head, mut m_tail) = (0usize, 0usize);
        let mut queued = [false; 3];

        for _ in 0..6 {
            let choice: usize = kani::any();
            kani::assume(choice <= 3);
            if choice < 3 {
                // The token is the check now: a node is pushed exactly when its token is held.
                if let Some(token) = held[choice].take() {
                    assert!(!queued[choice], "a token was held for a queued node");
                    queued[choice] = true;
                    model[m_tail] = choice;
                    m_tail += 1;
                    q.push_back(token);
                }
            } else {
                let popped = q.pop_front();
                if m_head == m_tail {
                    assert!(popped.is_none(), "popped from an empty queue");
                } else {
                    let expect = model[m_head];
                    m_head += 1;
                    queued[expect] = false;
                    let token = popped.expect("lost a node");
                    // SAFETY: pop returns only nodes we pushed, all still valid.
                    let got = unsafe { (*token.as_ptr()).tag };
                    assert_eq!(got, expect, "not FIFO");
                    held[expect] = Some(token);
                }
            }
            assert_eq!(q.len(), m_tail - m_head);
            assert_eq!(q.is_empty(), m_head == m_tail);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct N {
        next: Option<NonNull<N>>,
        tag: u32,
    }

    // SAFETY: `next` and `set_next` are plain field storage and touch nothing else, which is the whole of the `Node` contract.
    unsafe impl Node for N {
        fn next(&self) -> Option<NonNull<Self>> {
            self.next
        }
        fn set_next(&mut self, next: Option<NonNull<Self>>) {
            self.next = next;
        }
    }

    fn node(tag: u32) -> Box<N> {
        Box::new(N { next: None, tag })
    }

    /// Mint `n`'s token. Every test calls this once per node, on a `Box` declared before its
    /// queue, so the node outlives the queue (locals drop in reverse) and is on no queue yet.
    fn token(n: &mut Box<N>) -> Unqueued<N> {
        // SAFETY: a live boxed node on no queue, minted once per test (see above); the test is the
        // only accessor.
        unsafe { Unqueued::new(NonNull::from(&mut **n)) }
    }

    fn tag(t: &Unqueued<N>) -> u32 {
        // SAFETY: every token in these tests names one of the test's live boxed nodes.
        unsafe { (*t.as_ptr()).tag }
    }

    #[test]
    fn fifo_order() {
        let (mut a, mut b, mut c) = (node(1), node(2), node(3));
        let mut q: Fifo<N> = Fifo::new();
        assert!(q.is_empty());

        q.push_back(token(&mut a));
        q.push_back(token(&mut b));
        q.push_back(token(&mut c));
        assert_eq!(q.len(), 3);

        // A non-empty queue says so; a body stuck at `true` passes every empty-state check.
        assert!(!q.is_empty());

        // len falls by exactly one per pop; the scheduler's run-queue accounting trusts it.
        let first = q.pop_front().map(|t| tag(&t));
        assert_eq!(first, Some(1));
        assert_eq!(q.len(), 2);

        let tags: Vec<u32> = core::iter::from_fn(|| q.pop_front())
            .map(|t| tag(&t))
            .collect();
        assert_eq!(tags, vec![2, 3]);
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
    }

    /// The empty-again transitions: a queue drained to empty accepts new nodes correctly (the
    /// classic tail-pointer bug), and a popped node can be pushed again with the token its pop
    /// handed back.
    #[test]
    fn drain_then_reuse() {
        let (mut a, mut b) = (node(1), node(2));
        let mut q: Fifo<N> = Fifo::new();

        q.push_back(token(&mut a));
        let ta = q.pop_front().unwrap();
        assert_eq!(tag(&ta), 1);
        assert!(q.pop_front().is_none());

        q.push_back(token(&mut b));
        q.push_back(ta); // the token the pop handed back is what makes queueing `a` again legal
        assert_eq!(q.pop_front().map(|t| tag(&t)), Some(2));
        assert_eq!(q.pop_front().map(|t| tag(&t)), Some(1));
        assert!(q.is_empty());
    }

    /// The token is the pointer and nothing more: `#[repr(transparent)]` is the zero-cost claim the
    /// kernel's footprint gate rests on, and `Option` of one is still one word.
    #[test]
    fn a_token_is_one_pointer_wide() {
        use core::mem::size_of;
        assert_eq!(size_of::<Unqueued<N>>(), size_of::<NonNull<N>>());
        assert_eq!(size_of::<Option<Unqueued<N>>>(), size_of::<NonNull<N>>());
    }

    /// A token compares equal to its node's pointer and to no other, and prints as that pointer,
    /// which is what lets a caller assert which node a pop handed back.
    #[test]
    fn a_token_is_its_nodes_pointer() {
        let (mut a, mut b) = (node(1), node(2));
        let pb = NonNull::from(&mut *b);
        let ta = token(&mut a);
        assert!(ta == NonNull::from(&mut *a));
        assert!(!(ta == pb));
        assert_eq!(
            format!("{ta:?}"),
            format!("Unqueued({:?})", ta.as_non_null())
        );
    }

    /// A popped node leaves with a clean link: it does not secretly point back into the queue.
    #[test]
    fn a_popped_node_carries_no_link() {
        let (mut a, mut b) = (node(1), node(2));
        let mut q: Fifo<N> = Fifo::new();
        q.push_back(token(&mut a));
        q.push_back(token(&mut b));
        let p = q.pop_front().unwrap();
        // SAFETY: `p` was just popped, so it is live and unlinked; that it is unlinked is what this test asserts.
        assert!(unsafe { (*p.as_ptr()).next() }.is_none());
    }
}
