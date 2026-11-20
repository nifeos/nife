//! **Messages between processes**: send, receive and call on endpoints, the reply and request
//! shapes, notifications, timers and sleeping, supervision's reap, survey and list, and handing a
//! capability over.
//!
//! The crate root re-exports every public item, so each keeps its crate-root path.
//!
//! Name: ratified 2026-11-20 (UTC); first minted, minted 2026-10-18 (UTC) by maintainer/names-comply-free-crates, for the
//! module and the file. Named for the calls it holds, one module per seam of the old crate root.
//! Refused `ipc` (§154 (the acronym test is whether the phrase is spoken) spells it out).

use crate::objects::{self, retype_into_kernel_object};
use crate::system_call::{invoke, invoke5, invoke6};

/// `SEND` three words on the endpoint capability in `slot`. Blocks until a receiver takes them.
pub fn send(slot: u64, word_zero: u64, word_one: u64, word_two: u64) -> i64 {
    // SAFETY: `svc` traps to EL1, which validates the capability named by `slot`.
    unsafe { invoke(slot, abi::rendezvous::SEND, word_zero, word_one, word_two) }
}

/// **Collect the corpse of a child this supervision endpoint supervises** (DECISIONS §32).
/// `thread_id` is the thread id the kernel stamped on the death message [`receive_message`] returned. `0` on
/// success; a negative [`abi::Error`] otherwise, and the three that matter are worth telling apart:
/// `StillAlive` (not dead yet, so wait or escalate to the owner's `MemoryRegion::DESTROY`),
/// `NotSupervised` (not a child of this endpoint, or already collected), and `NotPermitted` (the
/// corpse's region is not reclaimable yet).
///
/// The point of the method: this needs no capability to the child's memory, so a supervisor can be
/// a process that cannot build one. The reclaimed pages go back to the builder's budget.
///
/// Name: ratified 2026-11-20 (UTC), replacing `reap`, which stays as a `doc(alias)`: the word
/// alone does not say what is collected, so the thread is named.
#[doc(alias = "reap")]
pub fn reap_thread(slot: u64, thread_id: u64) -> i64 {
    // SAFETY: `svc`/`ecall`; the kernel validates the capability and the supervision relationship.
    unsafe { invoke(slot, abi::rendezvous::REAP, thread_id, 0, 0) }
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
/// Name: ratified 2026-11-20 (UTC).
pub fn survey_record(slot: u64, cursor: u64, record: u64) -> (i64, u64, u64) {
    // SAFETY: forwarded from `invoke5`'s contract; SURVEY reads no more than the three words used.
    let (r0, w1, w2, ..) = unsafe { invoke5(slot, abi::rendezvous::SURVEY, cursor, record, 0) };
    (r0 as i64, w1, w2)
}

/// **Read one entry of what this address space has mapped** (milestone 126, `pmap`,
/// `address_space::LIST`, DECISIONS §114 (`pmap` gets its listing)). Returns `(next_cursor, virtual_address, kind)`: start at `cursor = 0`, feed
/// each `next_cursor` back, and stop when [`abi::survey::DONE`] comes back. [`survey`]'s twin,
/// same three-word-out-of-one-`invoke` shape, one object type over.
///
/// A negative first word is an [`abi::Error`], and the one that matters is `NotPermitted`: this
/// capability does not carry `ENUMERATE`, so the holder may map into the space but not list it.
/// **That is a refusal and not an empty listing**, and a caller must print it as one.
///
/// Name: ratified 2026-11-20 (UTC); first minted, minted 2026-11-15 (UTC) by lane/1112-outside-review-fixes.
pub fn virtual_address_space_list(slot: u64, cursor: u64) -> (i64, u64, u64) {
    // SAFETY: forwarded from `invoke5`'s contract; LIST reads no more than the three words used.
    let (r0, w1, w2, ..) = unsafe { invoke5(slot, abi::address_space::LIST, cursor, 0, 0) };
    (r0 as i64, w1, w2)
}

/// `RECEIVE` three words on the endpoint capability in `slot`. Blocks until a sender arrives; returns
/// the three words the sender passed in `x0`, `x1`, `x2`.
///
/// # Examples
/// A pipeline stage. There is no `open` and no path: nothing that was not handed over can be named.
/// The rendezvous is the flow control, and `Gone` from `send` is this system's `SIGPIPE`.
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

/// `RECEIVE_CAPABILITY` on the endpoint capability in `slot`: receive a message that may carry a
/// capability. Blocks until one arrives; returns `(w0, capability_slot, w1)`, where `capability_slot` is where
/// the incoming capability landed in this thread's capability table, or [`abi::rendezvous::NO_CAPABILITY`] if the
/// message carried none. **For a program receiving a delegation**, whose `capability_slot` it then uses
/// as the capability it expects. A server that answers a [`call`] receives with [`receive_request`]
/// instead: `capability_slot` here is whatever the sender chose, a Reply only if the sender `CALL`ed, and
/// [`reply`] takes the typed [`Reply`] only `receive_request` returns (milestone 706).
///
/// Name: ratified 2026-11-20 (UTC); first minted, minted 2026-10-27 (UTC) by lane capability-sweep-6. Spelled out from
/// `receive_cap` under the capability ruling of 2026-10-06 (UTC).
pub fn receive_capability(slot: u64) -> (u64, u64, u64) {
    // SAFETY: forwarded from `invoke5`'s contract; RECEIVE_CAPABILITY reads no more than the three words used.
    let (w0, w1, w2, ..) = unsafe { invoke5(slot, abi::rendezvous::RECEIVE_CAPABILITY, 0, 0, 0) };
    (w0, w1, w2)
}

/// **Mint a badged copy of the endpoint capability in `slot`** (milestone 599 (a frame per filesystem client channel)):
/// `abi::rendezvous::BADGE`. Returns the slot the badged copy landed in, or a negative error
/// (`NotPermitted` without `GRANT`, or for a zero badge or an already-badged source). The new slot
/// holds a copy of the endpoint with `badge` stamped on it. The kernel delivers
/// that badge to a server's [`receive_capability_message`] whenever this copy is used to `CALL` or `SEND_CAPABILITY`,
/// which is how a client the progenitor built is told apart from its siblings on one endpoint.
pub fn badge(slot: u64, badge: u64) -> i64 {
    // SAFETY: `svc`/`ecall`; the kernel validates the endpoint capability and mints into a free slot.
    unsafe { invoke(slot, abi::rendezvous::BADGE, badge, 0, 0) }
}

/// **One message a receive returned**: the three words the sender passed, the badge on the endpoint
/// capability it invoked, the label the kernel stamped, and what came with the first word, typed.
/// Milestone 1112 (`user_mode_runtime`: what an outside maintainer would fix before it leaves),
/// Decision 3, made it one type.
///
/// The fields are the registers the kernel wrote, so each receive is one projection of the same
/// words rather than its own return shape:
///
/// | field | register | meaning |
/// |---|---|---|
/// | `w0` | `x0` | the sender's first word |
/// | `w1` | `x1` | the sender's second word, or the capability slot of a [`receive_capability_message`] |
/// | `w2` | `x2` | the sender's third word |
/// | `badge` | `x3` | the sender's badge, `0` when unbadged |
/// | `label` | `x5` | a death message's label (`abi::fault`), `0` otherwise |
///
/// A death message from a supervision endpoint (`abi::fault`) is the same words read another way:
/// `w0` is the event, `w1` the thread id, `w2` the program counter and `badge` the faulting address.
///
/// Name: ratified 2026-11-20 (UTC); first minted, minted 2026-11-15 (UTC) by lane/1112-outside-review-fixes.
#[derive(Debug, PartialEq, Eq)]
pub struct Message {
    /// `x0`: the sender's first word, or a death message's event.
    pub w0: u64,
    /// `x1`: the sender's second word, or a capability message's slot, or a death message's thread id.
    pub w1: u64,
    /// `x2`: the sender's third word, or a death message's program counter.
    pub w2: u64,
    /// `x3`: the badge on the capability the sender invoked, `0` when unbadged, or a death
    /// message's faulting address.
    pub badge: u64,
    /// `x5`: the dead child's label (§148 (resolves by asking the kernel)), `0` when unbadged or
    /// unstamped. `invoke6` zeroes the register on the way in, so any other receive reads `0`.
    pub label: u64,
    /// What `w1` is, told by the kernel-written `x4`: a [`Reply`], a delegated capability or
    /// nothing. Always [`Delivered::Nothing`] after a plain [`receive_message`].
    pub delivered: Delivered,
}

/// **What a receive on a thread with a bound notification returned** (milestone 151 (notification
/// objects)): an ordinary message, or the bound notification's word.
#[derive(Debug, PartialEq, Eq)]
pub enum Received {
    /// A sender's message.
    Message(Message),
    /// The bound notification ended the receive; this is its word.
    Notification(u64),
}

impl Received {
    /// The message, for a thread that has no notification bound, where the kernel never writes
    /// [`abi::notification::BOUND`] and so the other arm cannot happen.
    ///
    /// # Panics
    /// If the receive was ended by a notification: the caller bound one and should match instead.
    ///
    /// Name: ratified 2026-11-20 (UTC); first minted, minted 2026-11-15 (UTC) by lane/1112-outside-review-fixes.
    pub fn into_message(self) -> Message {
        match self {
            Received::Message(message) => message,
            Received::Notification(word) => {
                panic!("a notification ended a receive on a thread that bound none: word {word:#x}")
            }
        }
    }
}

/// **`RECEIVE` on the endpoint capability in `slot`, every word the kernel returns**: blocks until a
/// sender arrives, or, for a thread with a notification bound to it ([`notification_bind`]), until
/// the notification is signalled, and says which. One call for the receives that used to differ
/// only in the words they kept: the badge, the six words of a death message and the bound
/// notification.
///
/// **It tests `x4`, not `x0`**: `x0` is the sender's own first word, so a sender can put
/// [`abi::notification::BOUND`] there, while `x4` is written only by the kernel. See
/// `abi::notification::BOUND` and notes/notification-objects.md. A caller with no bound notification
/// takes the message with [`Received::into_message`].
///
/// Name: ratified 2026-11-20 (UTC); first minted, minted 2026-11-15 (UTC) by lane/1112-outside-review-fixes.
pub fn receive_message(slot: u64) -> Received {
    // SAFETY: forwarded from `invoke6`'s contract; RECEIVE returns up to six words.
    let (w0, w1, w2, badge, w4, label) =
        unsafe { invoke6(slot, abi::rendezvous::RECEIVE, 0, 0, 0) };
    if w4 == abi::notification::BOUND {
        Received::Notification(w1)
    } else {
        Received::Message(Message {
            w0,
            w1,
            w2,
            badge,
            label,
            delivered: Delivered::Nothing,
        })
    }
}

/// **`RECEIVE_CAPABILITY` on the endpoint capability in `slot`, every word the kernel returns**:
/// [`receive_message`] for a message that may carry a capability. `w1` is where the capability
/// landed (or [`abi::rendezvous::NO_CAPABILITY`]), and `delivered` types it by the kernel's `x4`,
/// as [`receive_request`] does for a `CALL` server.
///
/// Name: ratified 2026-11-20 (UTC); first minted, minted 2026-11-15 (UTC) by lane/1112-outside-review-fixes.
pub fn receive_capability_message(slot: u64) -> Received {
    // SAFETY: forwarded from `invoke6`'s contract; RECEIVE_CAPABILITY returns up to six words.
    let (w0, w1, w2, badge, w4, label) =
        unsafe { invoke6(slot, abi::rendezvous::RECEIVE_CAPABILITY, 0, 0, 0) };
    if w4 == abi::notification::BOUND {
        Received::Notification(w1)
    } else {
        Received::Message(Message {
            w0,
            w1,
            w2,
            badge,
            label,
            delivered: Delivered::decode(w1, w4),
        })
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
/// ([`counter_ticks`](crate::counter_ticks)) reaches `deadline`, OR `bits` into the notification in
/// `notification_slot`. Replaces any pending deadline; a deadline already reached signals at once.
/// `0`, or a negative [`abi::Error`].
///
/// A timer does not block. Wait on the notification ([`notification_wait`]), or receive with it
/// bound ([`receive_message`]), and the wait ends on the deadline or on anything else that signals it.
pub fn timer_arm(timer_slot: u64, deadline: u64, notification_slot: u64, bits: u64) -> i64 {
    // SAFETY: `svc`/`ecall`/`system_call`; the kernel validates both capabilities.
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
    // SAFETY: `svc`/`ecall`/`system_call`; the kernel validates the capability.
    unsafe { invoke(timer_slot, abi::timer::CANCEL, 0, 0, 0) }
}

/// **A timer and a notification to sleep on, retyped from the untyped in `memory_region_slot`**:
/// `Some((timer_slot, notification_slot))`, or `None` if they cannot be made safely. For
/// [`sleep_until`]. A caller keeps the pair for its life rather than making one per sleep, because
/// each is a page it cannot give back short of destroying the region.
///
/// **`reserved` is how many low slots mean something by being empty**, and the pair is refused
/// while any of them is. `RETYPE_OBJECT` puts a new capability in the first free slot, and several
/// wirings say "you have no directory" or "no entropy" by leaving a fixed slot empty. A sleeper that
/// landed there would read as that service. So this probes slots `0..reserved` first, and a
/// caller with an empty one keeps whatever it did before (its `BUGS` says which).
pub fn retype_sleeper(memory_region_slot: u64, reserved: u64) -> Option<(u64, u64)> {
    for slot in 0..reserved {
        if !objects::is_granted(slot) {
            return None;
        }
    }
    let n = retype_into_kernel_object(memory_region_slot, abi::object_type::NOTIFICATION);
    if n < 0 {
        return None;
    }
    let t = retype_into_kernel_object(memory_region_slot, abi::object_type::TIMER);
    if t < 0 {
        objects::capability_delete(n as u64);
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
/// // from an answer without a probe request. See `entropy_contract::delivered_bytes`.
/// assert!((r0 as i64) >= 0 || abi::Error::from_ret(r0 as i64).is_some());
/// # let _ = r1;
/// # }
/// ```
pub fn call(slot: u64, word_zero: u64, word_one: u64) -> (u64, u64) {
    // SAFETY: forwarded from `invoke5`'s contract; CALL reads no more than the two words used.
    let (r0, r1, ..) = unsafe { invoke5(slot, abi::rendezvous::CALL, word_zero, word_one, 0) };
    (r0, r1)
}

/// [`call`], also returning the capability the server's reply carried, if it carried one: the slot
/// a [`reply_capability`] filed in this process's table, or `None` after a plain [`reply`] or when the
/// table was full (§255 (each socket is its own capability)).
pub fn call_receiving(slot: u64, word_zero: u64, word_one: u64) -> (u64, u64, Option<u64>) {
    // `NO_CAPABILITY` goes in as the unused third argument: a refused CALL returns without writing x2,
    // so x2 is then this value rather than a 0 that would read as slot 0.
    let none = abi::rendezvous::NO_CAPABILITY;
    // SAFETY: forwarded from `invoke5`'s contract; CALL returns the delivered slot in x2.
    let (r0, r1, x2, ..) =
        unsafe { invoke5(slot, abi::rendezvous::CALL, word_zero, word_one, none) };
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
    result_word_zero: u64,
    result_word_one: u64,
    capability: u64,
) -> Result<bool, (Reply, i64)> {
    // SAFETY: `svc`/`ecall`; the kernel validates the Reply and the carried capability.
    let r = unsafe {
        invoke(
            to.0,
            abi::reply::REPLY_CAPABILITY,
            result_word_zero,
            result_word_one,
            capability,
        )
    };
    if r < 0 { Err((to, r)) } else { Ok(r == 0) }
}

/// `REPLY` through a `CALL`'s one-shot Reply capability: deliver two words to the blocked caller
/// and wake it. The capability is consumed by the kernel on use (that is what makes it one-shot),
/// so the slot is free again when this returns, and this takes the [`Reply`] by value to say so.
///
/// **It takes a [`Reply`], not a slot** (milestone 706 (a `CALL` server can tell a Reply from a
/// delegation), DECISIONS §245 (a `CALL` server tells a Reply from a delegation)). A slot from
/// `RECEIVE_CAPABILITY` may hold anything a client chose to `SEND_CAPABILITY`, and `REPLY` is method `0`, which on a
/// rendezvous is `SEND`: a server that answered a delegated rendezvous parked itself in that `SEND`
/// for the life of the machine. A [`Reply`] comes only from [`receive_request`], which built it from
/// the kernel's own tag, so the type is the check and no server has to remember it.
pub fn reply(to: Reply, result_word_zero: u64, result_word_one: u64) -> i64 {
    // SAFETY: `svc`/`ecall`; the kernel validates the Reply capability and consumes it.
    unsafe {
        invoke(
            to.0,
            abi::reply::REPLY,
            result_word_zero,
            result_word_one,
            0,
        )
    }
}

/// **A `CALL`'s one-shot Reply capability, known to be one** (milestone 706, DECISIONS §245). The only constructor is [`receive_request`]'s reading of the kernel-written
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

/// **What a `RECEIVE_CAPABILITY` put in `x1`, told apart by the kernel-written `x4`** (milestone 706,
/// DECISIONS §245).
#[derive(Debug, PartialEq, Eq)]
pub enum Delivered {
    /// A `CALL`: `x1` is the Reply the kernel minted for this caller, and `x4` said so.
    Reply(Reply),
    /// A `SEND_CAPABILITY`: `x1` is a capability the sender chose, of any type. Not a Reply, whatever the
    /// sender meant by it.
    Delegation(u64),
    /// No capability: a plain `SEND`, an interrupt signal, a death message, or a `CALL` whose Reply
    /// did not fit in a full table (there is nobody to answer).
    Nothing,
}

impl Delivered {
    /// Read `capability_word` (the kernel's `x1`) with `tag_word` (`x4`) beside it. Only `0` in `x4` makes a real slot a delegation; a bound
    /// notification's `tag_word` means `capability_word` is its word, not a slot, so it reads as
    /// [`Delivered::Nothing`] rather than as a capability to delete.
    pub fn decode(capability_word: u64, tag_word: u64) -> Self {
        if capability_word == abi::rendezvous::NO_CAPABILITY {
            Delivered::Nothing
        } else if tag_word == abi::rendezvous::REPLY_DELIVERED {
            Delivered::Reply(Reply(capability_word))
        } else if tag_word == 0 {
            Delivered::Delegation(capability_word)
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
                objects::capability_delete(slot);
                None
            }
            Delivered::Nothing => None,
        }
    }
}

/// **One message a `CALL` server received** (milestone 706, DECISIONS §245): the
/// two data words, the sender's badge, and what came with it, typed.
#[derive(Debug, PartialEq, Eq)]
pub struct Request {
    /// The first data word (`x0`).
    pub w0: u64,
    /// The second data word (`x2`): a `CALL`'s second word, `0` for a `SEND_CAPABILITY`.
    pub w1: u64,
    /// The badge on the endpoint capability the sender invoked (`x3`), `0` when unbadged.
    pub badge: u64,
    /// What arrived in `x1`.
    pub delivered: Delivered,
}

/// **`RECEIVE_CAPABILITY` for a `CALL` server** (milestone 706, DECISIONS §245): blocks
/// until a message arrives and returns it with `x1` typed by the kernel's `x4`. The receive every
/// server that answers with [`reply`] uses, because it is the only source of a [`Reply`].
pub fn receive_request(slot: u64) -> Request {
    // SAFETY: forwarded from `invoke5`'s contract; RECEIVE_CAPABILITY returns five words.
    let (w0, x1, w1, badge, x4) =
        unsafe { invoke5(slot, abi::rendezvous::RECEIVE_CAPABILITY, 0, 0, 0) };
    Request {
        w0,
        w1,
        badge,
        delivered: Delivered::decode(x1, x4),
    }
}

/// `SEND_CAPABILITY` on the endpoint capability in `slot`: delegate a (possibly narrowed) copy of the
/// capability in `capability_slot`, narrowed to `rights`, alongside the data word `word_one`, and block until a
/// receiver takes them. `0` or a positive ack on success; a negative `abi::Error`.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2). His words: "Ratify ... `send_capability`, ..." Refused `send_capability` (round 7's
/// provisional name; capability is spelled out, calef, 2026-10-06).
pub fn send_capability(slot: u64, capability_slot: u64, rights: u64, word_one: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates the endpoint, `GRANT` on the delegated capability,
    // and narrows it to `rights` before it delegates anything.
    unsafe {
        invoke(
            slot,
            abi::rendezvous::SEND_CAPABILITY,
            capability_slot,
            rights,
            word_one,
        )
    }
}
