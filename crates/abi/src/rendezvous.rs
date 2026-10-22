/// `invoke(cap, SEND, w0, w1, w2)` -> 0. **Blocks until a receiver takes the message.**
///
/// The three words travel in registers and never touch memory. That is the whole of the
/// fastpath, and it is DECISIONS §10's rule made real: *IPC carries control.* Bulk data will
/// move later by handing over a frame capability, not by copying bytes into a message.
///
/// Two refusals a sender must tell apart, and the ABI does (milestone 50): [`crate::Error::NoSuchSlot`]
/// means the slot is empty, and [`crate::Error::Gone`] means the endpoint this capability names has
/// been destroyed, including while this thread was blocked inside the send. See
/// `crates/byte_sink_contract`.
pub const SEND: u64 = 0;

/// `invoke(cap, RECEIVE, _, _, _)` -> w0, with w1 in x1 and w2 in x2. **Blocks until a message
/// arrives.** `x3` and `x4` are written only by the kernel: for an ordinary message `x3` is the
/// badge on the endpoint capability the sender invoked ([`BADGE`], `0` when unbadged) and `x4`
/// is `0`; the fault address and a reserved `0` for a §26 death message; and
/// [`notification::BOUND`](crate::notification::BOUND) in `x4` when a bound notification ended
/// the receive (milestone 151 (notification objects)).
///
/// The badge in `x3` is milestone 613 (a system log service: the in-memory half)'s amendment
/// to §230 (badged endpoint capabilities), which delivered it on [`RECEIVE_CAP`] only: the log
/// stamps a byte-sink writer from its badge, and a byte-sink writer `SEND`s. Before it, `x3` was
/// always `0` for an ordinary message. A death message is told apart by its first word, as
/// before; nothing reads `x3` before checking that.
///
/// **A plain `RECEIVE` never takes a capability**, whichever side reached the rendezvous first
/// (§246 (a plain `RECEIVE` never takes a capability), PROVISIONAL number; calef's ruling A,
/// 2026-10-04 UTC). A [`SEND_CAP`] delivers its data word in `x0`, `0` in `x1` and `x2`, and the
/// capability is dropped (the sender keeps its own copy). A [`CALL`] delivers its two words in
/// `x0` and `x1`, and the caller is answered [`Gone`](crate::Error::Gone), because the one-shot
/// Reply has nowhere to go. Take capabilities with [`RECEIVE_CAP`], and serve a `CALL` with it.
/// Until the ruling a receiver parked first had the capability installed in its table and its
/// slot in `x1`, so any holder of `WRITE` here and `GRANT` on anything could fill the table of a
/// receiver that never asked (found by milestone 752 (a seeded syscall driver with a shadow
/// model)).
///
/// # BUGS
///
/// **`x0` is the sender's own `w0`, so a sender can make a receiver read an error.** A negative
/// `w0` equal to an [`Error`](crate::Error) code is decoded as that error by any wrapper that
/// checks the sign, and the receiver cannot tell it from a real refusal. Found by milestone 151
/// while checking the premise of §101 (notification objects) that `w0` could carry a tag; the same fact is why the
/// notification tag lives in `x4`. Not fixed: the fix is a register convention for `RECEIVE`'s
/// status, which every receiver in the tree is written against.
pub const RECEIVE: u64 = 1;

/// `invoke(cap, SEND_CAP, cap_slot, rights, w0)` -> 0. **Delegate a capability.** Passes the
/// capability in the sender's `cap_slot`, narrowed to `rights` (see [`crate::rights`]), plus one data
/// word, over this endpoint; blocks until a receiver takes it. The endpoint capability needs
/// `WRITE` (you may send here), and the *delegated* capability needs `GRANT` (you were trusted
/// to pass it on). `rights` may only narrow what the sender holds, never widen it. This is the
/// operation that makes nife a capability system a process can actually compose in:
/// authority moves between processes at runtime instead of being wired by the kernel at spawn.
/// Only a [`RECEIVE_CAP`] takes the capability: a plain [`RECEIVE`] gets the data word and the
/// copy is dropped, on either arrival order (§246, PROVISIONAL number).
pub const SEND_CAP: u64 = 2;

/// `invoke(cap, RECEIVE_CAP, _, _, _)` -> w0, with the received capability's new slot in x1, a
/// second data word in x2, and the **sender's badge in x3** (milestone 599, provisional), or
/// [`NO_CAP`] in x1 if the message carried no capability. **Blocks until a message arrives.**
/// The received capability lands in a free slot of the receiver's own capability table, chosen
/// by the kernel; x1 is where. This is also how a server receives a [`CALL`]: the slot in x1
/// holds a one-shot [`crate::reply`] capability naming the caller. **x3 is the badge on the
/// endpoint capability the sender invoked** ([`BADGE`]), or 0 when it was unbadged, which is how
/// a server serving many clients on one endpoint tells them apart. **x4 is
/// [`REPLY_DELIVERED`] when x1 holds a `CALL`'s Reply** (milestone 706), `notification::BOUND`
/// when a bound notification ended the receive, and `0` otherwise; x4 is written only by the
/// kernel. Needs `READ`.
///
/// `x1` is [`NO_CAP`] unless a capability was installed for this delivery, on both arrival
/// orders, so a plain `SEND` never hands the receiver a sender-chosen slot.
///
/// # BUGS
///
/// - **A §26 (the fault endpoint) death message read here loses the faulting `pc`.** The kernel's
///   five words are `(event, tid, pc, addr, 0)`, and this method returns
///   `(event, NO_CAP, tid, addr, 0)`. Every supervisor in the tree receives deaths with `RECEIVE`
///   through `user_mode_runtime::receive_fault` instead.
/// - **A delegated capability passes the `NO_CAP` guard.** Any client with `WRITE` on the endpoint
///   and `GRANT` on a capability can [`SEND_CAP`] it here, and a `CALL` server that invoked
///   [`crate::reply::REPLY`] on it would run method `0` of whatever object it is. The kernel writes
///   [`REPLY_DELIVERED`] in `x4` only for a `CALL`'s Reply (DECISIONS §245 (a `CALL` server tells a
///   Reply from a delegation)), and `user_mode_runtime::receive_request` checks it and deletes an
///   unasked delegation; a server that reads `x1` raw is as exposed as before. An endpoint that
///   refuses `SEND_CAP` outright is milestone 706's recorded follow-on.
pub const RECEIVE_CAP: u64 = 3;

/// `invoke(cap, CALL, w0, w1, _)` -> r0, with r1 in x1 and in x2 the slot of a capability the
/// server's [`crate::reply::REPLY_CAPABILITY`] carried, or [`NO_CAP`]. **Send two words and block until
/// replied.** The atomic send-and-wait a server can answer safely: at the rendezvous the kernel
/// mints a one-shot [`crate::reply`] capability naming *this* caller and hands it to the server (through
/// [`RECEIVE_CAP`]), so the server can answer a caller it was never wired to, exactly once, and only
/// that caller. Needs `WRITE`. Milestone 12 (call/reply IPC); see notes/ipc-naming.md. A `CALL`
/// that reaches a plain [`RECEIVE`] returns [`Gone`](crate::Error::Gone): the receiver gets the
/// two words, and there is nowhere to put the Reply (§246, PROVISIONAL number).
pub const CALL: u64 = 4;

/// `invoke(cap, REAP, tid, _, _)` -> 0. **Collect a corpse this endpoint supervises**
/// (DECISIONS §32). `tid` is the thread id the kernel stamped on the death message
/// ([`fault`](super::fault)); the kernel reclaims that thread's TCB, its address space, and the
/// region behind them, exactly what [`memory_region::DESTROY`](super::memory_region::DESTROY) would have
/// reclaimed. Needs `READ` on this endpoint: the authority to collect is the authority to
/// *receive* deaths here, which is what a supervisor holds.
///
/// **Authorization is the supervision relationship, not a rights bit and not a registry.** The
/// kernel checks that the named thread's recorded fault endpoint *is this endpoint*, so a tid
/// means something only relative to the capability it arrived through. A supervisor therefore
/// needs no capability to the child's memory, and gains none: the reclaimed region returns to
/// its owner under §13 region ownership, which is the **builder**. A supervisor can free a
/// child's memory; it cannot spend it.
///
/// Three distinct refusals, because a restart policy wants to tell them apart:
///
/// - [`crate::Error::StillAlive`] the thread is not dead. Collecting a corpse is not killing; killing a
///   live child is the stronger act and stays with `MemoryRegion::DESTROY` (§24's forcible `^C`).
/// - [`crate::Error::NotSupervised`] no thread by that tid supervised by *this* endpoint: another
///   supervisor's child, an already-collected one, or a stale tid whose generational name no
///   longer resolves. The two are deliberately one error, so a supervisor cannot probe the tid
///   space of children it does not supervise.
/// - [`crate::Error::NotPermitted`] the corpse's region cannot be reclaimed yet (the child `SPLIT` its
///   own budget and those children are still live), the same refusal `DESTROY` gives.
///
/// # BUGS
///
/// - **`NotPermitted` has a second cause this list does not name.** The reclaim behind `REAP` also
///   refuses when another thread whose page lies in the corpse's region can still run, and that
///   refusal arms the kill of §16 (object revocation) on it, exactly as a refused `DESTROY` does.
///   A supervisor that treats the first `NotPermitted` as fatal, as `terminal_supervisor` and
///   `sub_server_supervisor` do, cannot tell the two apart. No test builds a corpse that shares its
///   region with a live thread, so whether a supervisor can meet this is unmeasured. (The corpse
///   itself still leaving its core is waited out by the kernel and never reaches here.)
pub const REAP: u64 = 5;

/// `invoke(cap, SURVEY, cursor, record, 0)` -> `(next_cursor, tid, word)`. **Read one entry of
/// the domain this endpoint supervises** (milestone 126 (who else is running, and who is allowed to
/// ask)). `next_cursor` returns in x0, `tid` in x1, and the selected record's own word in x2.
///
/// **`record` is a selector** (calef, 2026-09-21): a new per-thread fact becomes a new
/// [`survey::record`](super::survey::record) value, never a new return register, as Zircon's
/// `zx_object_get_info` is a selector over topics. x0 and x1 are the frame and only x2 belongs to
/// the record, so a caller wanting two facts walks twice and joins on the tid.
/// [`survey::record::STATE`](super::survey::record::STATE) is 0 so callers that passed an unused 0
/// keep reading what they read. An unknown record is [`crate::Error::BadMethod`], refused before
/// the walk, so a bad selector against an empty domain is not a
/// [`survey::DONE`](super::survey::DONE) that reads as "nothing here".
///
/// **The scope is the supervision domain**: a thread is in the survey exactly when its recorded
/// fault endpoint *is* this endpoint, the relationship [`REAP`] is authorized by. A `ps` launched
/// from a shell sees the shell's children; an operator's `ps` sees the machine only because
/// somebody handed it the endpoint that supervises the machine.
///
/// **Needs [`rights::ENUMERATE`](super::rights::ENUMERATE), and pointedly not `READ`**, which
/// [`RECEIVE`] and [`REAP`] take: **a domain names its members, it does not act on them** (calef,
/// 2026-08-17). A holder without it is refused with [`crate::Error::NotPermitted`], loudly, never
/// shown an empty domain.
///
/// **The cursor is a resume point.** Start at 0; each entry returns the `next_cursor` for the one
/// after it, and [`survey::DONE`](super::survey::DONE) means finished (`tid` and the word are then
/// 0). An empty domain finishes on the first call. **It is a snapshot per call, not per survey**:
/// `readdir`'s bargain, because holding `IPC_TABLES` across a survey would put a userspace program
/// in charge of how long the scheduler is locked.
pub const SURVEY: u64 = 6;

/// `invoke(cap, BADGE, badge, _, _)` -> the slot the badged copy landed in, or a negative
/// [`crate::Error`]. The slot comes back in x0, the way `memory_region::RETYPE_OBJ` answers.
///
/// **Mint a badged copy of this endpoint capability** (milestone 599 (a frame per filesystem
/// client channel), provisional name and number, kept free by DECISIONS §148 (a supervisor restarts by asking)). The new
/// capability names the same endpoint with the same rights, plus `badge` stamped on it; the
/// kernel delivers that `badge` to a server's [`RECEIVE_CAP`] whenever this copy's holder `CALL`s
/// or `SEND_CAP`s here, so a server serving many clients on one endpoint can tell them apart.
///
/// Needs `GRANT` (minting a delegatable view is a delegation-class power) and refuses a `badge`
/// of 0 (that is the unbadged value) or a source that is already badged (seL4's rule: a badge is
/// set once and never changed, which is what lets a holder trust the badge it was given). The
/// caller then hands the badged copy to a client with `SEND_CAP`, narrowing rights as usual; the
/// badge rides through that delegation because `SEND_CAP` copies the object.
pub const BADGE: u64 = 7;

/// The x1 value from [`RECEIVE_CAP`] when the message carried no capability.
pub const NO_CAP: u64 = u64::MAX;

/// **The tag on a receive that delivered a `CALL`'s Reply** (milestone 706 (a `CALL` server
/// can tell a Reply from a delegation), DECISIONS §245; name provisional).
///
/// [`RECEIVE_CAP`] returns this in `x4` exactly when the kernel installed the one-shot
/// [`crate::reply`] capability it minted for a [`CALL`], and `x1` is the slot it landed in.
/// `x4` is written only by the kernel, so no sender can forge it: a [`SEND_CAP`] delegation, a
/// plain [`SEND`], an interrupt signal and a §26 death message all leave it `0`, and a bound
/// notification leaves [`notification::BOUND`](crate::notification::BOUND). A `CALL` whose
/// Reply could not be installed (the receiver's table was full) arrives with `x1 ==`
/// [`NO_CAP`] and `x4 == 0`, because there is no Reply to name.
///
/// **Test this, never `x1 != NO_CAP`, before answering.** A delegation passes the `NO_CAP`
/// guard; only this says the slot holds a Reply. `user_mode_runtime::receive_request` reads it and
/// returns a typed Reply, which is the only thing `user_mode_runtime::reply` accepts.
///
/// Ruled by calef on 2026-10-03 (UTC): "Option 1 with the typed runtime helper."
///
/// Name: ratified 2026-10-03 (calef, reviewing milestone 706 (a `CALL` server can tell a Reply from a
/// delegation): "Ratify `REPLY_DELIVERED`.").
pub const REPLY_DELIVERED: u64 = 1;

// `x4` carries both tags, so they must differ from each other and from the untagged `0`.
const _: () = assert!(
    REPLY_DELIVERED != 0
        && REPLY_DELIVERED != super::notification::BOUND
        && super::notification::BOUND != 0
);
