//! **Messages between processes**: send, receive and call on endpoints, the reply and request
//! shapes, notifications, timers and sleeping, supervision's reap, survey and list, and handing a
//! capability over.
//!
//! The crate root re-exports every public item, so each keeps its crate-root path.
//!
//! Name: provisional, minted 2026-10-18 (UTC) by maintainer/names-comply-free-crates, for the
//! module and the file. Named for the calls it holds, one module per seam of the old crate root.
//! Refused `ipc` (§154 (the acronym test is whether the phrase is spoken) spells it out).

use crate::objects::{self, retype_into_kernel_object};
use crate::syscall::{invoke, invoke5, invoke6};

/// `SEND` three words on the endpoint capability in `slot`. Blocks until a receiver takes them.
pub fn send(slot: u64, w0: u64, w1: u64, w2: u64) -> i64 {
    // SAFETY: `svc` traps to EL1, which validates the capability named by `slot`.
    unsafe { invoke(slot, abi::rendezvous::SEND, w0, w1, w2) }
}

/// **Collect the corpse of a child this supervision endpoint supervises** (DECISIONS §32).
/// `tid` is the thread id the kernel stamped on the death message [`receive_fault`] returned. `0` on
/// success; a negative [`abi::Error`] otherwise, and the three that matter are worth telling apart:
/// `StillAlive` (not dead yet, so wait or escalate to the owner's `MemoryRegion::DESTROY`),
/// `NotSupervised` (not a child of this endpoint, or already collected), and `NotPermitted` (the
/// corpse's region is not reclaimable yet).
///
/// The point of the method: this needs no capability to the child's memory, so a supervisor can be
/// a process that cannot build one. The reclaimed pages go back to the builder's budget.
pub fn reap(slot: u64, tid: u64) -> i64 {
    // SAFETY: `svc`/`ecall`; the kernel validates the capability and the supervision relationship.
    unsafe { invoke(slot, abi::rendezvous::REAP, tid, 0, 0) }
}

/// **Read one entry of the domain this supervision endpoint supervises** (milestone 126,
/// `rendezvous::SURVEY`): [`survey_record`] with [`abi::survey::record::STATE`], the run state
/// every caller wants. Returns `(next_cursor, tid, state)`: start at `cursor = 0`, feed each
/// `next_cursor` back, and stop when [`abi::survey::DONE`] comes back.
///
/// A negative first word is an [`abi::Error`]. `NotPermitted` means this endpoint capability does
/// not carry `ENUMERATE`: a refusal, not an empty domain, and a caller must print it as one.
pub fn survey(slot: u64, cursor: u64) -> (i64, u64, u64) {
    survey_record(slot, cursor, abi::survey::record::STATE)
}

/// **Read one entry of a supervised domain, asking for a specific record** (`rendezvous::SURVEY`,
/// §204 (how userspace asks where a thread runs)). Returns `(next_cursor, tid, word)`, walked as
/// [`survey`] is. `record` picks which per-thread fact lands in the third word (see
/// [`abi::survey::record`]); a caller wanting two facts walks twice and joins on the tid.
///
/// A negative first word is an [`abi::Error`], and two are not an empty domain: `NotPermitted`
/// (the capability lacks `ENUMERATE`) and `BadMethod` (a record this kernel does not answer, as a
/// program built against a later kernel gets). A caller must print either as a refusal.
///
/// Name: provisional. calef names public items.
pub fn survey_record(slot: u64, cursor: u64, record: u64) -> (i64, u64, u64) {
    // SAFETY: forwarded from `invoke5`'s contract; SURVEY reads no more than the three words used.
    let (r0, w1, w2, ..) = unsafe { invoke5(slot, abi::rendezvous::SURVEY, cursor, record, 0) };
    (r0 as i64, w1, w2)
}

/// **Read one entry of what this address space has mapped** (milestone 126, `pmap`,
/// `address_space::LIST`, DECISIONS §114). Returns `(next_cursor, va, kind)`: start at `cursor = 0`, feed
/// each `next_cursor` back, and stop when [`abi::survey::DONE`] comes back. [`survey`]'s twin,
/// same three-word-out-of-one-`invoke` shape, one object type over.
///
/// A negative first word is an [`abi::Error`], and the one that matters is `NotPermitted`: this
/// capability does not carry `ENUMERATE`, so the holder may map into the space but not list it.
/// **That is a refusal and not an empty listing**, and a caller must print it as one.
pub fn list(slot: u64, cursor: u64) -> (i64, u64, u64) {
    // SAFETY: forwarded from `invoke5`'s contract; LIST reads no more than the three words used.
    let (r0, w1, w2, ..) = unsafe { invoke5(slot, abi::address_space::LIST, cursor, 0, 0) };
    (r0 as i64, w1, w2)
}

/// `RECEIVE` three words on the endpoint capability in `slot`. Blocks until a sender arrives; returns
/// the three words the sender passed in `x0`, `x1`, `x2`.
///
/// # Examples
/// A pipeline stage. There is no `open` and no path: nothing that was not handed over can be named.
/// The rendezvous is the flow control, and `Gone` from `send` is this system's SIGPIPE.
///
/// ```no_run
/// use user_mode_runtime::{exit, receive, send};
///
/// /// A pipeline stage: read three words off the endpoint in slot 0, pass them to slot 1.
/// fn relay() -> ! {
///     const IN: u64 = 0;
///     const OUT: u64 = 1;
///     loop {
///         // `receive` blocks until a sender rendezvouses. The rendezvous IS the flow control: there
///         // is no buffer to fill and no back-pressure to invent.
///         let (w0, w1, w2) = receive(IN);
///         if send(OUT, w0, w1, w2) < 0 {
///             // A negative return is an `abi::Error`. `Gone` here means the reader exited, which
///             // is this system's SIGPIPE, arriving as a return code rather than as a signal.
///             exit();
///         }
///     }
/// }
/// ```
pub fn receive(slot: u64) -> (u64, u64, u64) {
    // SAFETY: forwarded from `invoke5`'s contract; RECEIVE reads no more than the three words used.
    let (w0, w1, w2, ..) = unsafe { invoke5(slot, abi::rendezvous::RECEIVE, 0, 0, 0) };
    (w0, w1, w2)
}

/// [`receive`], also returning the **sender's badge** in the fourth position: `(w0, w1, w2, badge)`.
/// The badge is the value stamped on the endpoint capability the sender invoked
/// (`abi::rendezvous::BADGE`), or 0 when it was unbadged. The `RECEIVE` twin of [`receive_cap_badged`],
/// for a server whose clients `SEND` rather than `CALL`: the system log stamps a byte-sink writer
/// this way (milestone 613 (a system log service), provisional; the name is provisional too).
pub fn receive_badged(slot: u64) -> (u64, u64, u64, u64) {
    // SAFETY: forwarded from `invoke5`'s contract; RECEIVE writes the badge into the fourth word.
    let (w0, w1, w2, w3, _) = unsafe { invoke5(slot, abi::rendezvous::RECEIVE, 0, 0, 0) };
    (w0, w1, w2, w3)
}

/// [`receive_badged`] for a thread with a bound notification: `Ok((w0, w1, w2, badge))` for a
/// message, `Err(word)` when the bound notification ended the receive, told apart by the
/// kernel-written `x4` exactly as [`receive_bound`] does. The log service's one wait point, which
/// takes writers' lines and the kernel's ring signal alike (milestone 342 (the kernel and the
/// `console` server drive one UART from two address spaces)). Name provisional.
pub fn receive_badged_bound(slot: u64) -> Result<(u64, u64, u64, u64), u64> {
    // SAFETY: forwarded from `invoke5`'s contract; RECEIVE returns five words.
    let (w0, w1, w2, w3, w4) = unsafe { invoke5(slot, abi::rendezvous::RECEIVE, 0, 0, 0) };
    if w4 == abi::notification::BOUND {
        Err(w1)
    } else {
        Ok((w0, w1, w2, w3))
    }
}

/// `RECEIVE` **all six words** on the endpoint capability in `slot`: `(w0, w1, w2, w3, w4, w5)`.
///
/// A fault notification is five words, `(event, tid, pc, addr, reserved)` (§26 (the fault
/// endpoint) implementation note 4); ordinary three-word IPC leaves the top two zero. A checking
/// supervisor reads `w3`, the faulting address. The sixth word is the dead child's label (§148
/// (resolves by asking the kernel), as amended 2026-10-04): the badge its builder put on the
/// supervision capability in `abi::fault::FAULT_EP_SLOT`, or `0` when unbadged or unstamped,
/// because [`invoke6`] zeroes the register on the way in. See `abi::fault`.
pub fn receive_fault(slot: u64) -> (u64, u64, u64, u64, u64, u64) {
    // SAFETY: forwarded from `invoke6`'s contract.
    unsafe { invoke6(slot, abi::rendezvous::RECEIVE, 0, 0, 0) }
}

/// `RECEIVE_CAP` on the endpoint capability in `slot`: receive a message that may carry a
/// capability. Blocks until one arrives; returns `(w0, cap_slot, w1)`, where `cap_slot` is where
/// the incoming capability landed in this thread's capability table, or [`abi::rendezvous::NO_CAP`] if the
/// message carried none. **For a program receiving a delegation**, whose `cap_slot` it then uses
/// as the capability it expects. A server that answers a [`call`] receives with [`receive_request`]
/// instead: `cap_slot` here is whatever the sender chose, a Reply only if the sender `CALL`ed, and
/// [`reply`] takes the typed [`Reply`] only `receive_request` returns (milestone 706).
pub fn receive_cap(slot: u64) -> (u64, u64, u64) {
    // SAFETY: forwarded from `invoke5`'s contract; RECEIVE_CAP reads no more than the three words used.
    let (w0, w1, w2, ..) = unsafe { invoke5(slot, abi::rendezvous::RECEIVE_CAP, 0, 0, 0) };
    (w0, w1, w2)
}

/// [`receive_cap`], also returning the **sender's badge** in the fourth position (milestone 599 (a frame per filesystem client channel),
/// provisional): `(w0, received_slot, w1, badge)`. The badge is the value stamped on the endpoint
/// capability the sender invoked (`abi::rendezvous::BADGE`), or 0 when it was unbadged, so a server
/// serving many clients on one endpoint tells them apart. A server that does not care which client
/// called keeps using [`receive_cap`]; this is for one that maps a per-client resource by badge.
pub fn receive_cap_badged(slot: u64) -> (u64, u64, u64, u64) {
    // SAFETY: forwarded from `invoke5`'s contract; RECEIVE_CAP writes the badge into the fourth word.
    let (w0, w1, w2, w3, ..) = unsafe { invoke5(slot, abi::rendezvous::RECEIVE_CAP, 0, 0, 0) };
    (w0, w1, w2, w3)
}

/// **Mint a badged copy of the endpoint capability in `slot`** (milestone 599, provisional):
/// `abi::rendezvous::BADGE`. Returns the slot the badged copy landed in, or a negative error
/// (`NotPermitted` without `GRANT`, or for a zero badge or an already-badged source). The new slot
/// holds a copy of the endpoint with `badge` stamped on it. The kernel delivers
/// that badge to a server's [`receive_cap_badged`] whenever this copy is used to `CALL` or `SEND_CAP`,
/// which is how a client the progenitor built is told apart from its siblings on one endpoint.
pub fn badge(slot: u64, badge: u64) -> i64 {
    // SAFETY: `svc`/`ecall`; the kernel validates the endpoint capability and mints into a free slot.
    unsafe { invoke(slot, abi::rendezvous::BADGE, badge, 0, 0) }
}

/// **What a bound thread's receive returned** (milestone 151 (notification objects)): an ordinary message, or the bound
/// notification's word. *(Name provisional, milestone 151's lane.)*
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Received {
    /// A sender's three words, exactly what [`receive`] returns.
    Message(u64, u64, u64),
    /// The bound notification ended the receive; this is its word.
    Notification(u64),
}

/// `RECEIVE` on the endpoint capability in `slot`, for a thread with a notification bound to it
/// ([`notification_bind`]): blocks until either a message arrives or the notification is
/// signalled, and says which.
///
/// **It tests `x4`, not `x0`**, and that is the whole reason this wrapper exists rather than
/// callers reading [`receive`]: `x0` is the sender's own first word, so a sender can put
/// [`abi::notification::BOUND`] there, while `x4` is written only by the kernel. See
/// `abi::notification::BOUND` and notes/notification-objects.md. *(Name provisional.)*
pub fn receive_bound(slot: u64) -> Received {
    // SAFETY: forwarded from `invoke5`'s contract; RECEIVE returns five words.
    let (w0, w1, w2, _, w4) = unsafe { invoke5(slot, abi::rendezvous::RECEIVE, 0, 0, 0) };
    if w4 == abi::notification::BOUND {
        Received::Notification(w1)
    } else {
        Received::Message(w0, w1, w2)
    }
}

/// [`receive_cap`], for a thread with a bound notification: a message comes back as
/// `Received::Message(w0, reply_slot, w1)`, and a signal on the bound notification as
/// `Received::Notification(word)`, told apart by the kernel-written `w4` exactly as in
/// [`receive_bound`].
///
/// Name: provisional (the lane for milestone 23 (a capability-routed component OS with live
/// replacement), 2026-09-27), for `broker`'s advisory warning, DECISIONS §231 (a swap's warning to
/// a dependent is advisory, and the supervisor never waits for it).
pub fn receive_cap_bound(slot: u64) -> Received {
    // SAFETY: forwarded from `invoke5`'s contract; RECEIVE_CAP returns five words.
    let (w0, w1, w2, _, w4) = unsafe { invoke5(slot, abi::rendezvous::RECEIVE_CAP, 0, 0, 0) };
    if w4 == abi::notification::BOUND {
        Received::Notification(w1)
    } else {
        Received::Message(w0, w1, w2)
    }
}

/// `Notification::SIGNAL`: OR `bits` into the notification in `slot`. Never blocks. `0`, or a
/// negative [`abi::Error`].
pub fn notification_signal(slot: u64, bits: u64) -> i64 {
    // SAFETY: `svc`/`ecall`; the kernel validates the capability.
    unsafe { invoke(slot, abi::notification::SIGNAL, bits, 0, 0) }
}

/// `Notification::WAIT`: the accumulated word, blocking until it is non-zero. A negative
/// [`abi::Error`] on refusal (see `abi::notification::WAIT` for the top-bit caveat).
pub fn notification_wait(slot: u64) -> i64 {
    // SAFETY: `svc`/`ecall`; the kernel validates the capability.
    unsafe { invoke(slot, abi::notification::WAIT, 0, 0, 0) }
}

/// `Notification::POLL`: the accumulated word without blocking, `0` if nothing was pending.
pub fn notification_poll(slot: u64) -> i64 {
    // SAFETY: `svc`/`ecall`; the kernel validates the capability.
    unsafe { invoke(slot, abi::notification::POLL, 0, 0, 0) }
}

/// `Notification::BIND`: bind the notification in `slot` to the thread whose `ThreadControlBlock`
/// capability is in `thread_slot`. `0`, or a negative [`abi::Error`].
pub fn notification_bind(slot: u64, thread_slot: u64) -> i64 {
    // SAFETY: `svc`/`ecall`; the kernel validates both capabilities.
    unsafe { invoke(slot, abi::notification::BIND, thread_slot, 0, 0) }
}

/// `Timer::ARM` (milestone 106 (a wait that ends on either the interrupt or the deadline),
/// DECISIONS §147 (a timer a userspace service cannot hold)): when the counter
/// ([`now`](crate::now)) reaches `deadline`, OR `bits` into the notification in
/// `notification_slot`. Replaces any pending deadline; a deadline already reached signals at once.
/// `0`, or a negative [`abi::Error`].
///
/// A timer does not block. Wait on the notification ([`notification_wait`]), or receive with it
/// bound ([`receive_bound`]), and the wait ends on the deadline or on anything else that signals it.
pub fn timer_arm(timer_slot: u64, deadline: u64, notification_slot: u64, bits: u64) -> i64 {
    // SAFETY: `svc`/`ecall`/`syscall`; the kernel validates both capabilities.
    unsafe {
        invoke(
            timer_slot,
            abi::timer::ARM,
            deadline,
            notification_slot,
            bits,
        )
    }
}

/// `Timer::CANCEL`: `1` if a pending deadline was disarmed and will never fire, `0` if nothing was
/// pending (so any signal has already been sent), or a negative [`abi::Error`].
pub fn timer_cancel(timer_slot: u64) -> i64 {
    // SAFETY: `svc`/`ecall`/`syscall`; the kernel validates the capability.
    unsafe { invoke(timer_slot, abi::timer::CANCEL, 0, 0, 0) }
}

/// **A timer and a notification to sleep on, retyped from the untyped in `memory_region_slot`**:
/// `Some((timer_slot, notification_slot))`, or `None` if they cannot be made safely. For
/// [`sleep_until`]. A caller keeps the pair for its life rather than making one per sleep, because
/// each is a page it cannot give back short of destroying the region.
///
/// **`reserved` is how many low slots mean something by being empty**, and the pair is refused
/// while any of them is. `RETYPE_OBJ` puts a new capability in the first free slot, and several
/// wirings say "you have no directory" or "no entropy" by leaving a fixed slot empty. A sleeper that
/// landed there would read as that service. So this probes slots `0..reserved` first, and a
/// caller with an empty one keeps whatever it did before (its `BUGS` says which).
pub fn retype_sleeper(memory_region_slot: u64, reserved: u64) -> Option<(u64, u64)> {
    for slot in 0..reserved {
        // SAFETY: `svc`/`ecall`/`syscall` with a method no object has: a held slot answers
        // `BadMethod` and does nothing, and an empty one answers `NoSuchSlot`.
        if unsafe { invoke(slot, u64::MAX, 0, 0, 0) } == abi::Error::NoSuchSlot as i64 {
            return None;
        }
    }
    let n = retype_into_kernel_object(memory_region_slot, abi::objtype::NOTIFICATION);
    if n < 0 {
        return None;
    }
    let t = retype_into_kernel_object(memory_region_slot, abi::objtype::TIMER);
    if t < 0 {
        objects::cap_delete(n as u64);
        return None;
    }
    Some((t as u64, n as u64))
}

/// **Block until `deadline`** (counter ticks), on a timer and a notification this caller holds and
/// uses for nothing else: arm, then wait. Returns the notification's word (the `bits` armed, unless
/// something else also signalled it), or a negative [`abi::Error`]. The one-line sleep every
/// consumer that only wants to sleep would otherwise write; a consumer that wants to wake on
/// something else too arms the timer and waits its own way.
pub fn sleep_until(timer_slot: u64, notification_slot: u64, deadline: u64) -> i64 {
    let armed = timer_arm(timer_slot, deadline, notification_slot, 1);
    if armed < 0 {
        return armed;
    }
    notification_wait(notification_slot)
}

/// `CALL` on the endpoint capability in `slot`: send two words and block until the server
/// replies through the one-shot Reply capability the kernel mints (milestone 12). Returns the
/// two reply words. The atomic send-and-wait that makes a request unmistakably answerable.
///
/// # Examples
/// The client never names the server and the server never names the client:
///
/// ```no_run
/// use user_mode_runtime::call;
///
/// # fn ask() {
/// const SERVICE: u64 = 2;
/// let (r0, r1) = call(SERVICE, 0x0100_0000_0000_0008, 0);
///
/// // Negative-as-u64 is enormous, which is how a wire contract tells "no capability in that slot"
/// // from an answer without a probe request. See `entropy_contract::delivered`.
/// assert!((r0 as i64) >= 0 || abi::Error::from_ret(r0 as i64).is_some());
/// # let _ = r1;
/// # }
/// ```
pub fn call(slot: u64, w0: u64, w1: u64) -> (u64, u64) {
    // SAFETY: forwarded from `invoke5`'s contract; CALL reads no more than the two words used.
    let (r0, r1, ..) = unsafe { invoke5(slot, abi::rendezvous::CALL, w0, w1, 0) };
    (r0, r1)
}

/// [`call`], also returning the capability the server's reply carried, if it carried one: the slot
/// a [`reply_capability`] filed in this process's table, or `None` after a plain [`reply`] or when the
/// table was full (§255 (each socket is its own capability)). Name provisional.
pub fn call_receiving(slot: u64, w0: u64, w1: u64) -> (u64, u64, Option<u64>) {
    // `NO_CAP` goes in as the unused third argument: a refused CALL returns without writing x2,
    // so x2 is then this value rather than a 0 that would read as slot 0.
    let none = abi::rendezvous::NO_CAP;
    // SAFETY: forwarded from `invoke5`'s contract; CALL returns the delivered slot in x2.
    let (r0, r1, x2, ..) = unsafe { invoke5(slot, abi::rendezvous::CALL, w0, w1, none) };
    let delivered = (x2 != none).then_some(x2);
    (r0, r1, delivered)
}

/// **`REPLY_CAPABILITY`: answer a caller with two words and a copy of the capability in slot
/// `capability`** (§255 (each socket is its own capability); name follows the ratified method). The copy keeps the
/// rights it has here and needs `GRANT`. `Ok(true)` means the copy is in the caller's table, and
/// `Ok(false)` that the caller was answered without it (its table was full) or was no longer
/// waiting. On a refusal nothing was delivered and the Reply is still this server's, so it comes
/// back in `Err` to be answered with [`reply`].
pub fn reply_capability(
    to: Reply,
    r0: u64,
    r1: u64,
    capability: u64,
) -> Result<bool, (Reply, i64)> {
    // SAFETY: `svc`/`ecall`; the kernel validates the Reply and the carried capability.
    let r = unsafe { invoke(to.0, abi::reply::REPLY_CAPABILITY, r0, r1, capability) };
    if r < 0 { Err((to, r)) } else { Ok(r == 0) }
}

/// `REPLY` through a `CALL`'s one-shot Reply capability: deliver two words to the blocked caller
/// and wake it. The capability is consumed by the kernel on use (that is what makes it one-shot),
/// so the slot is free again when this returns, and this takes the [`Reply`] by value to say so.
///
/// **It takes a [`Reply`], not a slot** (milestone 706 (a `CALL` server can tell a Reply from a
/// delegation), DECISIONS §245 (a `CALL` server tells a Reply from a delegation)). A slot from
/// `RECEIVE_CAP` may hold anything a client chose to `SEND_CAP`, and `REPLY` is method `0`, which on a
/// rendezvous is `SEND`: a server that answered a delegated rendezvous parked itself in that `SEND`
/// for the life of the machine. A [`Reply`] comes only from [`receive_request`], which built it from
/// the kernel's own tag, so the type is the check and no server has to remember it.
pub fn reply(to: Reply, r0: u64, r1: u64) -> i64 {
    // SAFETY: `svc`/`ecall`; the kernel validates the Reply capability and consumes it.
    unsafe { invoke(to.0, abi::reply::REPLY, r0, r1, 0) }
}

/// **A `CALL`'s one-shot Reply capability, known to be one** (milestone 706, DECISIONS §245; the
/// name is provisional). The only constructor is [`receive_request`]'s reading of the kernel-written
/// `x4` ([`abi::rendezvous::REPLY_DELIVERED`]), and the only consumer is [`reply`], so a server
/// cannot answer through a slot a client delegated. Neither `Copy` nor `Clone`, because a Reply
/// answers once.
#[must_use = "the caller stays blocked until this Reply is answered"]
#[derive(Debug, PartialEq, Eq)]
pub struct Reply(u64);

impl Reply {
    /// The slot this Reply occupies. For naming it to something that is not [`reply`] (a test that
    /// answers twice to prove the second is refused); reading it does not let anyone build one.
    pub fn slot(&self) -> u64 {
        self.0
    }
}

/// **What a `RECEIVE_CAP` put in `x1`, told apart by the kernel-written `x4`** (milestone 706,
/// DECISIONS §245; names provisional).
#[derive(Debug, PartialEq, Eq)]
pub enum Delivered {
    /// A `CALL`: `x1` is the Reply the kernel minted for this caller, and `x4` said so.
    Reply(Reply),
    /// A `SEND_CAP`: `x1` is a capability the sender chose, of any type. Not a Reply, whatever the
    /// sender meant by it.
    Delegation(u64),
    /// No capability: a plain `SEND`, an interrupt signal, a death message, or a `CALL` whose Reply
    /// did not fit in a full table (there is nobody to answer).
    Nothing,
}

impl Delivered {
    /// Read `x1` with the tag beside it. Only `0` in `x4` makes a real slot a delegation; a bound
    /// notification's `x4` means `x1` is its word, not a slot, so it reads as
    /// [`Delivered::Nothing`] rather than as a capability to delete.
    pub fn decode(x1: u64, x4: u64) -> Self {
        if x1 == abi::rendezvous::NO_CAP {
            Delivered::Nothing
        } else if x4 == abi::rendezvous::REPLY_DELIVERED {
            Delivered::Reply(Reply(x1))
        } else if x4 == 0 {
            Delivered::Delegation(x1)
        } else {
            Delivered::Nothing
        }
    }

    /// The Reply, if this was a `CALL`. **A delegation is deleted**, so a server that did not ask
    /// for one does not keep its slot: the table is [`abi::CAPABILITY_TABLE_SLOTS`] slots, and a client that could
    /// fill it could stop the server receiving anything.
    pub fn into_reply(self) -> Option<Reply> {
        match self {
            Delivered::Reply(r) => Some(r),
            Delivered::Delegation(slot) => {
                objects::cap_delete(slot);
                None
            }
            Delivered::Nothing => None,
        }
    }
}

/// **One message a `CALL` server received** (milestone 706, DECISIONS §245; name provisional): the
/// two data words, the sender's badge, and what came with it, typed.
#[derive(Debug, PartialEq, Eq)]
pub struct Request {
    /// The first data word (`x0`).
    pub w0: u64,
    /// The second data word (`x2`): a `CALL`'s second word, `0` for a `SEND_CAP`.
    pub w1: u64,
    /// The badge on the endpoint capability the sender invoked (`x3`), `0` when unbadged.
    pub badge: u64,
    /// What arrived in `x1`.
    pub delivered: Delivered,
}

/// **`RECEIVE_CAP` for a `CALL` server** (milestone 706, DECISIONS §245; name provisional): blocks
/// until a message arrives and returns it with `x1` typed by the kernel's `x4`. The receive every
/// server that answers with [`reply`] uses, because it is the only source of a [`Reply`].
pub fn receive_request(slot: u64) -> Request {
    // SAFETY: forwarded from `invoke5`'s contract; RECEIVE_CAP returns five words.
    let (w0, x1, w1, badge, x4) = unsafe { invoke5(slot, abi::rendezvous::RECEIVE_CAP, 0, 0, 0) };
    Request {
        w0,
        w1,
        badge,
        delivered: Delivered::decode(x1, x4),
    }
}

/// [`receive_request`] for a thread with a bound notification: `Ok` for a message, `Err(word)` when
/// the bound notification ended the receive, told apart by `x4` as [`receive_bound`] does. Name
/// provisional (milestone 706).
pub fn receive_request_bound(slot: u64) -> Result<Request, u64> {
    // SAFETY: forwarded from `invoke5`'s contract; RECEIVE_CAP returns five words.
    let (w0, x1, w1, badge, x4) = unsafe { invoke5(slot, abi::rendezvous::RECEIVE_CAP, 0, 0, 0) };
    if x4 == abi::notification::BOUND {
        return Err(x1);
    }
    Ok(Request {
        w0,
        w1,
        badge,
        delivered: Delivered::decode(x1, x4),
    })
}

/// `SEND_CAP` on the endpoint capability in `slot`: delegate a (possibly narrowed) copy of the
/// capability in `cap_slot`, narrowed to `rights`, alongside the data word `w1`, and block until a
/// receiver takes them. `0` or a positive ack on success; a negative `abi::Error`.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2). His words: "Ratify ... `send_capability`, ..." Refused `send_cap` (round 7's
/// provisional name; capability is spelled out, calef, 2026-10-06).
pub fn send_capability(slot: u64, cap_slot: u64, rights: u64, w1: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates the endpoint, `GRANT` on the delegated capability,
    // and narrows it to `rights` before it delegates anything.
    unsafe { invoke(slot, abi::rendezvous::SEND_CAP, cap_slot, rights, w1) }
}
