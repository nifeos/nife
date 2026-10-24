/// `invoke(cap, SEND, w0, w1, w2)` -> 0. **Blocks until a receiver takes the message.**
///
/// The three words travel in registers and never touch memory. That is the whole of the
/// fastpath, and it is DECISIONS §10's rule made real: *IPC carries control.* Bulk data will
/// move later by handing over a frame capability, not by copying bytes into a message.
///
/// Two refusals a sender must tell apart, and the ABI does (milestone 50 (pipes and redirection)):
/// [`crate::Error::NoSuchSlot`] means the slot is empty, and [`crate::Error::Gone`] means the
/// endpoint this capability names has been destroyed, including while this thread was blocked
/// inside the send. See `crates/byte_sink_contract`.
pub const SEND: u64 = 0;

/// `invoke(cap, RECEIVE, _, _, _)` -> w0, with w1 in x1 and w2 in x2. **Blocks until a message
/// arrives.** Only the kernel writes `x3` and `x4`: `x3` is the [`BADGE`] on the sender's capability
/// (`0` unbadged), or a death's fault address under §26 (the fault endpoint), and `x4` is `0` or
/// [`notification::BOUND`](crate::notification::BOUND). **It never takes a capability**, §246 (a
/// plain `RECEIVE` never takes a capability): a [`SEND_CAP`]'s is dropped and a [`CALL`] gets
/// [`Gone`](crate::Error::Gone); use [`RECEIVE_CAP`] for both.
///
/// # BUGS
/// - **`x0` is the sender's own `w0`**, so a negative `w0` equal to an [`Error`](crate::Error) code
///   reads as that error to a wrapper that checks the sign (notes/abi/method-contracts.md).
pub const RECEIVE: u64 = 1;

/// `invoke(cap, SEND_CAP, cap_slot, rights, w0)` -> 0. **Delegate a capability.** Passes the
/// capability in the sender's `cap_slot`, narrowed to `rights` (see [`crate::rights`]), plus one
/// data word, over this endpoint; blocks until a receiver takes it. The endpoint capability needs
/// `WRITE` (you may send here), and the *delegated* capability needs `GRANT` (you were trusted to
/// pass it on). `rights` may only narrow what the sender holds, never widen it. This is the
/// operation that makes nife a capability system a process can actually compose in: authority moves
/// between processes at runtime instead of being wired by the kernel at spawn. Only a
/// [`RECEIVE_CAP`] takes the capability: a plain [`RECEIVE`] gets the data word and the copy is
/// dropped, on either arrival order (§246, PROVISIONAL number).
pub const SEND_CAP: u64 = 2;

/// `invoke(cap, RECEIVE_CAP, _, _, _)` -> w0, with the received capability's slot in x1 (or
/// [`NO_CAP`]), a second data word in x2 and the sender's [`BADGE`] in x3 (0 unbadged). **Blocks
/// until a message arrives.** The kernel chooses a free slot; for a [`CALL`] it holds a one-shot
/// [`crate::reply`] naming the caller. Only the kernel writes `x4`: [`REPLY_DELIVERED`] for a
/// `CALL`'s Reply, `notification::BOUND` when a bound notification ended the receive, else `0`.
/// `x1` is `NO_CAP` unless a capability was installed for this delivery. Needs `READ`.
///
/// # BUGS
/// - **A §26 death read here loses the faulting `pc`**: supervisors receive deaths with `RECEIVE`.
/// - A delegation passes the `NO_CAP` guard; see notes/abi/method-contracts.md.
pub const RECEIVE_CAP: u64 = 3;

/// `invoke(cap, CALL, w0, w1, _)` -> r0, with r1 in x1 and in x2 the slot of a capability the
/// server's [`crate::reply::REPLY_CAPABILITY`] carried, or [`NO_CAP`]. **Send two words and block
/// until replied.** The atomic send-and-wait a server can answer safely: at the rendezvous the
/// kernel mints a one-shot [`crate::reply`] capability naming *this* caller and hands it to the
/// server (through [`RECEIVE_CAP`]), so the server can answer a caller it was never wired to,
/// exactly once, and only that caller. Needs `WRITE`. Milestone 12 (call/reply IPC); see
/// notes/ipc-naming.md. A `CALL` that reaches a plain [`RECEIVE`] returns
/// [`Gone`](crate::Error::Gone): the receiver gets the two words, and there is nowhere to put the
/// Reply (§246, PROVISIONAL number).
pub const CALL: u64 = 4;

/// `invoke(cap, REAP, tid, _, _)` -> 0. **Collect a corpse this endpoint supervises**, §32 (a
/// supervisor may collect a corpse): reclaim the TCB, space and region of the thread `tid` names.
/// Needs `READ`, and the thread's recorded fault endpoint must *be* this endpoint.
///
/// - [`crate::Error::StillAlive`]: not dead yet. Killing stays with `MemoryRegion::DESTROY`.
/// - [`crate::Error::NotSupervised`]: one error for not yours, already collected, or a stale tid.
/// - [`crate::Error::NotPermitted`]: the corpse's region cannot be reclaimed yet (live children).
///
/// # BUGS
/// - `NotPermitted` has a second, unmeasured cause; see notes/abi/method-contracts.md.
pub const REAP: u64 = 5;

/// `invoke(cap, SURVEY, cursor, record, 0)` -> `(next_cursor, tid, word)`: **read one entry of the
/// domain this endpoint supervises** (milestone 126 (the `procps` package)), the threads whose fault
/// endpoint is this one. `record` picks the word ([`survey::record`](super::survey::record)); an
/// unknown one is [`crate::Error::BadMethod`], refused before the walk.
///
/// Needs [`rights::ENUMERATE`](super::rights::ENUMERATE), not `READ`: a domain names its members and
/// does not act on them (calef, 2026-08-17). Without it: `NotPermitted`, never an empty domain.
/// Start at cursor 0 and feed each `next_cursor` back until [`survey::DONE`](super::survey::DONE).
/// Each call is its own snapshot, never a lock held across the walk; see notes/abi/method-contracts.md.
pub const SURVEY: u64 = 6;

/// `invoke(cap, BADGE, badge, _, _)` -> the slot of a badged copy of this endpoint capability, in
/// x0 as `memory_region::RETYPE_OBJ` answers, or a negative [`crate::Error`] (milestone 599 (a
/// frame per filesystem client channel); provisional name and number). The copy names the same
/// endpoint with the same rights plus `badge`, which the kernel delivers to the server when its
/// holder sends here, so a server serving many clients on one endpoint tells them apart.
///
/// Needs `GRANT`, a delegation-class power. Refuses a `badge` of 0 (the unbadged value) and a
/// source already badged: a badge is set once (seL4's rule), so a holder can trust the one it was
/// given. It rides through a later [`SEND_CAP`], because that copies the object.
pub const BADGE: u64 = 7;

/// The x1 value from [`RECEIVE_CAP`] when the message carried no capability.
pub const NO_CAP: u64 = u64::MAX;

/// **The tag on a receive that delivered a `CALL`'s Reply** (milestone 706 (a `CALL` server can
/// tell a Reply from a delegation), DECISIONS §245 (a `CALL` server tells a Reply from a
/// delegation)). [`RECEIVE_CAP`] returns it in `x4` exactly when the kernel installed the one-shot
/// [`crate::reply`] it minted for a [`CALL`], with `x1` its slot. Only the kernel writes `x4`: a
/// delegation, a plain [`SEND`], an interrupt signal and a §26 death leave it `0`. A `CALL` whose
/// Reply did not fit the table arrives with `x1 ==` [`NO_CAP`] and `x4 == 0`. **Test this, never
/// `x1 != NO_CAP`**: `user_mode_runtime::receive_request` does, and returns the typed Reply
/// `user_mode_runtime::reply` accepts. Ruled by calef on 2026-10-03 (UTC): "Option 1 with the typed
/// runtime helper."
///
/// Name: ratified 2026-10-03 (calef, reviewing milestone 706 (a `CALL` server can tell a Reply from
/// a delegation): "Ratify `REPLY_DELIVERED`.").
pub const REPLY_DELIVERED: u64 = 1;

// `x4` carries both tags, so they must differ from each other and from the untagged `0`.
const _: () = assert!(
    REPLY_DELIVERED != 0
        && REPLY_DELIVERED != super::notification::BOUND
        && super::notification::BOUND != 0
);
