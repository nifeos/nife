#![no_std]
//! **Live component replacement: the shared half** (milestone 23, DECISIONS §41).
//!
//! What `swapper` (the operator), `rust_swappable` and `c_swappable` (the component's two
//! instances) and `swap_boundary_witness` (client and attacker) share (rule 7): the wire protocol,
//! the pages' addresses, the digest both languages compute, and the serving loop. A swap changes
//! *who is parked in `RECEIVE_CAP`* on one stable endpoint, so the client never changes (DECISIONS
//! §12 (Call/Reply IPC)'s endpoint-only naming). The UART's registers are the one thing not
//! shared: the operator takes them back with `PageFrame::REVOKE` between quiescing the old instance
//! and endowing the new one. See notes/live-replacement.md.
//!
//! Name: ratified 2026-10-18 (calef): "`hot_swap_contract` makes sense". "Swap" alone means paging in
//! OS vocabulary; milestone 23's word for live replacement is hot swap.

use user_mode_runtime::invoke;

// ===========================================================================================
// The service protocol. Every request is a `CALL` on the stable endpoint; the component serves it
// with `RECEIVE_CAP` and answers through the kernel's one-shot `Reply` capability (DECISIONS §12 (Call/Reply IPC)).
// ===========================================================================================

/// `call(SVC, OPERATION_PUT, seq)` -> `(digest(seq), (version << 32) | seq)`.
///
/// Two words out, two words back, and the reply carries **who answered**. A client that only wanted
/// the answer would ignore the version word entirely; `swap_boundary_witness` reads it because the
/// version word is the only way anything outside the operator can tell that a swap happened at all.
pub const OPERATION_PUT: u64 = 1;

/// **The sequence number a wedging instance swallows**: it takes the request, files the caller's
/// one-shot `Reply` capability away, and stops answering *without dying* (milestone 23's third
/// residual; notes/hung-component.md). Keyed on one request's identity, not a count, so it fires in
/// the same place under any scheduling; it sits after [`SWAP_TRIGGER`] so there is a conversation
/// on both sides of the hang. Zero means "never wedge", which the healthy roles pass. See
/// notes/hung-component/the-wedge-test.md.
///
/// Provisional name (`wedge`), like everything else in this crate's report and role vocabulary. It
/// is at least the word this tree already uses for the condition: DECISIONS §26 calls it "alive but
/// wedged" and milestone 62's block calls its own stuck test "wedged".
pub const WEDGE_SEQ: u64 = 24;

/// `call(SVC, OPERATION_QUIESCE, 0)` -> `(QUIESCED, served)`. The operator's in-band drain.
///
/// **It travels on the endpoint being drained, and that is the mechanism, not a shortcut.** The
/// endpoint's sender queue is FIFO, so by the time this request reaches the component every request
/// queued ahead of it has already been served and answered. There is no separate "have you finished"
/// protocol, no quiescence timeout, and no window in which the operator has to guess.
pub const OPERATION_QUIESCE: u64 = 2;

/// The `OPERATION_QUIESCE` answer's first word. A distinctive constant rather than `0`, so a reply that
/// was never written cannot be mistaken for an acknowledgement.
pub const QUIESCED: u64 = 0x5155_4954; // "QUIT"

/// What a component replies to a request it does not understand. Never seen in a healthy run; it is
/// here so that a protocol mismatch is a value the client can assert on rather than a hang.
pub const BAD_REQUEST: u64 = u64::MAX;

// ===========================================================================================
// The broker protocol: the latency ladder's middle rung (`broker`).
//
// **Opt-in per channel, never the default.** A producer speaks the same `OPERATION_PUT` on the
// front endpoint and gets the backend's own reply or `ACCEPTED` (the broker took custody). Control
// travels in band, as `OPERATION_QUIESCE` does, because a second endpoint would need a wait-any
// primitive the kernel deliberately does not have (DECISIONS §26.5, in §26 (the fault endpoint:
// thread death becomes a message a supervisor holds)).
// ===========================================================================================

/// **The broker's warning, which it is never asked to acknowledge** (DECISIONS §231 (a swap's warning
/// to a dependent is advisory, and the supervisor never waits for it)). The operator writes
/// [`WANT_DOWN`] or [`WANT_UP`] in the first word of a page the broker maps read-only, signals a
/// notification bound to the broker's thread, and does not wait. The broker reads the page on every
/// wakeup, so a late or missed signal degrades to pass-through, which `ROLE_UNWARNED` measured as
/// lossless. See notes/non-cooperative-fallback.md.
///
/// Name: provisional (milestone 23's lane, 2026-09-27), for this constant and the two below.
pub const BROKER_WARN_VA: u64 = 0x0330_0000;
/// The backend is going away: buffer from the next request on.
pub const WANT_DOWN: u64 = 1;
/// The backend is back (the page's initial zero): drain the backlog in order, then pass through.
pub const WANT_UP: u64 = 0;

/// What the operator last wrote on the broker's warning page.
pub fn broker_wants_down() -> bool {
    // SAFETY: `BROKER` declares the warning page read-only at BROKER_WARN_VA, so every broker has it
    // mapped; the operator writes it before it signals.
    unsafe { core::ptr::read_volatile(BROKER_WARN_VA as *const u64) == WANT_DOWN }
}

/// The broker took custody of an item instead of answering it. `w1` = the queue depth after it.
pub const ACCEPTED: u64 = 0x4143_4350; // "ACCP"
/// The broker's buffer is full. The producer's request is refused rather than silently dropped:
/// backpressure is a value, not a policy hidden inside a server.
pub const QUEUE_FULL: u64 = 0x4655_4c4c; // "FULL"

// ===========================================================================================
// The report protocol. Every process holds a WRITE view of one report endpoint and says what it did
// on it; the kernel test is the receiver. Mirrored in system_tests/src/user/live_swap_tests.rs, the
// same convention `authority_tests` and `c_seam_tests` follow: userspace owns the definition.
// ===========================================================================================

/// An instance started. `w1` = its version, `w2` = 1 if it could read the device's registers.
pub const RPT_UP: u64 = 1;
/// An instance answered `OPERATION_QUIESCE`. `w1` = version, `w2` = how many requests it served.
pub const RPT_QUIESCED: u64 = 2;
/// **A failure, reported positively.** The old instance was told to touch the device *after* the
/// operator revoked it, and the access succeeded. `w1` = version. A healthy run never sends this;
/// the kernel's fault message arrives instead.
pub const RPT_PROBE_SURVIVED: u64 = 3;
/// The operator finished a step of the swap. `w1` = the step (see [`step`]), `w2` = detail.
pub const RPT_STEP: u64 = 4;
/// The operator's own verdict, read out of the shared log page after everything is over.
/// `w1` = a bitmap of [`log_checks`], `w2` = the sequence number the version changed at.
pub const RPT_LOG: u64 = 5;
/// The client's own verdict, computed inside the client from the replies it received.
/// `w1` = a bitmap of [`client_checks`], `w2` = the sequence number the version changed at.
pub const RPT_CLIENT: u64 = 6;
/// The attacker's result. `w1` = the negated error the kernel gave it, `w2` = the operation it tried.
pub const RPT_ATTACK: u64 = 7;
/// The broker drained its backlog to a fresh backend. `w1` = items drained now, `w2` = items it
/// ever took custody of. Both are the queue rung's whole claim: nothing was lost while the backend
/// did not exist.
pub const RPT_DRAINED: u64 = 10;
/// **A component was refused before it was built** (milestone 23's manifest). `w1` = the
/// `capability_plan::Refusal` code, `w2` = the contract's own index in the operator's order.
///
/// The manifest's control that must fail, and the reason it is a *report* rather than a fault: a
/// supervisor that cannot satisfy a declaration has a legible thing to say and a child it did not
/// build. The operator sends this before it starts anything at all, so the test can assert that the
/// refusal landed ahead of the first build step rather than after a half-wired component existed.
pub const RPT_REFUSED: u64 = 11;
/// **What the supervision domain looks like while a component is hung** (milestone 23's third
/// residual). `w1` = how many members the survey reported, `w2` = their states, packed by
/// [`survey_counts`].
///
/// The operator reads this out of `abi::rendezvous::SURVEY` (milestone 126), which is the only view of
/// its own children it has. The report exists so the test can assert what the view **cannot** say.
pub const RPT_SURVEY: u64 = 12;

/// **The dependency graph's own verdict, before any orchestration acts on it** (milestone 23's
/// dependency-aware-orchestration residual). `w1` = how many live instances
/// `capability_plan::dependents` returned, `w2` = the first one's id (0 if none).
///
/// Reported once per swap target this channel considers, so the test can check the graph a
/// supervisor would compute against the sequencing the operator actually ran, rather than trusting
/// that the two agree.
pub const RPT_DEPENDENTS: u64 = 15;

/// **What the confused-deputy attacker's out-of-log `OPERATION_PUT` returned** (milestone 633,
/// third pass). `w1` = the reply's first word (the server's digest of the attacker's offset when the
/// server served the out-of-range write, or an error when it did not), `w2` = the reply's tag. The
/// test reads it to see whether the server reached outside its log. Name provisional.
pub const RPT_DEPUTY: u64 = 16;

/// **Every member of the domain refused to be collected.** `w1` = how many the operator asked about,
/// `w2` = how many answered [`abi::Error::StillAlive`].
///
/// `Rendezvous::REAP` (DECISIONS §32) is the supervisor's whole vocabulary over its domain, and it
/// refuses a thread that is not dead, on purpose: collecting a corpse is not killing. A hung
/// component is not a corpse, so the vocabulary is empty. The operator asks about **every** member
/// rather than about the one it suspects, because a survey returns a tid and nothing that says which
/// tid is which, and asking about all of them is the stronger statement anyway.
pub const RPT_UNCOLLECTABLE: u64 = 13;

/// **The incumbent did not answer `OPERATION_QUIESCE` within the operator's bound, so the operator
/// treats it as hung.** `w1` = the bound, `w2` = how long the operator actually waited, both in the
/// counter's ticks (`user_mode_runtime::now`, at `user_mode_runtime::cntfrq` ticks a second).
///
/// The operator's own verdict, reached by milestone 106 (a wait that ends on either the interrupt
/// or the deadline)'s timed wait and by nothing the incumbent said: a hung instance announces
/// nothing, and the trigger that started the swap is the same [`NOTE_SWAP_NOW`] a healthy one
/// sends. Wire number 14 is unused, and stays so.
///
/// Name: provisional.
pub const RPT_UNANSWERED: u64 = 17;

/// **The hung incumbent's region was destroyed while the incumbent was still hung** (milestone 133
/// (ending a permanently blocked thread, and deciding who may)'s `DESTROY` finishing a `Blocked`
/// resident in place). `w1` = the pages the operator's budget had in live child regions before the
/// `DESTROY`, `w2` = after, both read by `MemoryRegion::USAGE`'s `CHILDREN` record. The difference
/// is the instance's region coming home.
///
/// Name: provisional.
pub const RPT_RECLAIMED: u64 = 18;

/// A death reached the operator. `w1` = tid, `w2` = event.
pub const RPT_DEATH: u64 = 8;
/// Where that death happened. `w1` = pc, `w2` = fault address.
pub const RPT_SITE: u64 = 9;
/// Something could not be built or wired. `w1` = a stage code, so a broken system is debuggable
/// rather than silent.
pub const RPT_FAILED: u64 = 99;

/// The operator's steps, as they complete. The roadmap's four, plus the drain that has to happen
/// before the revoke can be safe.
pub mod step {
    /// The replacement is built and endowed, but not started: it cannot race the incumbent for
    /// requests, because a thread that has never been started is in nobody's queue.
    pub const BUILT: u64 = 1;
    /// The incumbent answered `OPERATION_QUIESCE` and stopped receiving. Detail = requests it served.
    pub const DRAINED: u64 = 2;
    /// The device capability was revoked from every holder but the operator.
    pub const REVOKED: u64 = 3;
    /// The replacement holds the registers and is running. The down window ends here.
    pub const STARTED: u64 = 4;
    /// The corpse was collected through the supervision endpoint (DECISIONS §32).
    pub const REAPED: u64 = 5;
    /// **The incoming instance absorbed the outgoing one's state** (DECISIONS §209 (state handoff is an opaque blob over a granted frame, and it is optional)). Detail = the
    /// tally it took over. Only now is the outgoing instance retired: this is the commit.
    ///
    /// Name: provisional (the lane for milestone 23 (a capability-routed component OS with live
    /// replacement), 2026-09-26).
    pub const ABSORBED: u64 = 6;
    /// **The incoming instance could not absorb the state, so the swap did not commit** (§209's
    /// third part). Detail = the blob layout it was handed and did not understand. The incumbent is
    /// told to resume, still holding the state it never lost, and the refusing instance's corpse is
    /// collected: the new grant is revoked by the same reap that collects every other child here.
    ///
    /// Name: provisional (milestone 23's lane, 2026-09-26).
    pub const ROLLED_BACK: u64 = 7;
}

/// The operator's verdict bits, computed from the shared log page after the run.
pub mod log_checks {
    /// Every sequence number in `[0, REQUESTS)` was served by somebody. No request was lost in the
    /// down window: they parked on the endpoint's sender queue and the new instance drained them.
    pub const NO_GAP: u64 = 1 << 0;
    /// The version recorded per sequence number never goes backwards. **This is the "never two
    /// owners" assertion**: two instances serving concurrently would interleave, and an interleave
    /// is a v1 after a v2.
    pub const MONOTONE: u64 = 1 << 1;
    /// Both versions appear, so the swap really happened inside the conversation rather than
    /// before it started or after it ended.
    pub const BOTH_VERSIONS: u64 = 1 << 2;
    /// The incumbent's post-revoke probe faulted, at the device's own virtual address, and the
    /// kernel said so. The receipt for the revoke.
    pub const REVOKE_ENFORCED: u64 = 1 << 3;
}

/// The client's verdict bits. Computed inside the client, from what the client itself observed;
/// nothing here is taken on the operator's word.
pub mod client_checks {
    /// Every call returned. None failed, none was refused, none had to be retried.
    pub const ALL_REPLIED: u64 = 1 << 0;
    /// Every reply echoed the sequence number of the request that asked for it.
    pub const SEQ_ECHOED: u64 = 1 << 1;
    /// Every reply's digest matched the client's own independent computation of the same
    /// definition. **The contract held across the swap**, not merely the connection.
    pub const DIGEST_CORRECT: u64 = 1 << 2;
    /// The version word changed exactly once, and only upwards.
    pub const ONE_TRANSITION: u64 = 1 << 3;
    /// The client saw both versions, so its conversation genuinely spans the swap.
    pub const SPANNED_SWAP: u64 = 1 << 4;
    /// **The queued rung only.** At least one call came back `ACCEPTED` rather than with an answer,
    /// so the producer really did keep running through a window in which no backend existed. Without
    /// this bit a run in which the swap happened between two calls would look identical.
    pub const WAS_BUFFERED: u64 = 1 << 5;
    /// **The queued rung only.** Every call that got a real answer got a *correct* one, and every
    /// call that did not got `ACCEPTED`. Nothing was refused, and the queue never overflowed.
    pub const NONE_REFUSED: u64 = 1 << 6;
    /// **The hung rung only.** One of this client's calls was swallowed by a component that stopped
    /// answering, and it came back [`abi::Error::Gone`] when the operator destroyed that
    /// component's region (milestone 254 (a caller stranded by a server that died)'s
    /// `sched::strand_callers_of`). Without it the hung run would read as an ordinary swap between
    /// two calls.
    ///
    /// Name: provisional.
    pub const WAS_GONE: u64 = 1 << 7;
}

/// **Pack the survey's state histogram into one report word.** Four counts, a byte each, in the
/// order [`abi::survey`] declares them, so a reader of `RPT_SURVEY` needs no second table.
///
/// A byte each is enough because `MAX_THREADS` is far below 256 and this operator's domain is three
/// children; a domain wider than a byte would saturate rather than wrap, which is a lie a test would
/// catch and a real monitor would not want. Hence the `min`.
pub const fn survey_counts(ready: u64, running: u64, blocked: u64, dead: u64) -> u64 {
    const fn saturate(n: u64) -> u64 {
        if n > 255 { 255 } else { n }
    }
    saturate(ready) | (saturate(running) << 8) | (saturate(blocked) << 16) | (saturate(dead) << 24)
}

/// The ready count packed into a [`survey_counts`] word.
pub const fn survey_ready(w: u64) -> u64 {
    w & 0xff
}
/// The running count packed into a [`survey_counts`] word.
pub const fn survey_running(w: u64) -> u64 {
    (w >> 8) & 0xff
}
/// The blocked count packed into a [`survey_counts`] word.
pub const fn survey_blocked(w: u64) -> u64 {
    (w >> 16) & 0xff
}
/// The dead count packed into a [`survey_counts`] word.
pub const fn survey_dead(w: u64) -> u64 {
    (w >> 24) & 0xff
}

// ===========================================================================================
// The wiring: pages and addresses every process in the system agrees on.
// ===========================================================================================

/// The page size every mapping below is sized and aligned to.
pub const PAGE: u64 = 4096;

/// The shared log page, mapped read/write into the operator and into each instance. One byte per
/// sequence number, holding the version of whoever served it. The operator's witness, in the
/// operator's own address space, written by processes that never see each other.
pub const LOG_VA: u64 = address_space_map::pair_page(0x0300_0000);

/// The device's registers, at the same virtual address in every instance. The same number on both
/// sides is what lets the kernel's reported fault address be compared directly against this
/// constant, with no translation step to get wrong.
pub const DEV_VA: u64 = address_space_map::pair_page(0x0310_0000);

/// Where the operator copies an instance's program image, so a component's builder holds one ELF
/// rather than the whole initial ramdisk.
pub const IMAGE_VA: u64 = address_space_map::runtime_window(0x3000_0000);

/// **The confused-deputy attacker's chosen offset** (milestone 633, third pass). `log_put` writes
/// at `LOG_VA + log_base + arg`; with `log_base = 0` this lands at `DEV_VA`, the first register of
/// the device the server holds and the client does not. The server writes the version byte there and
/// does not fault, because it is a valid register on every UART (an undecoded offset faults QEMU's
/// ns16550, which backs only the low register block, so the portable target is a decoded one). The
/// write reaches a page the client was never granted, so a server that serves this has reached
/// outside its grant on the client's behalf. Name provisional.
pub const DEPUTY_PROBE: u64 = DEV_VA - LOG_VA;

/// How many requests the client makes. Small enough to fit one log page, large enough that the swap
/// lands well inside the conversation.
pub const REQUESTS: u64 = 64;

/// The sequence number at which the incumbent tells the operator to start swapping. A third of the
/// way in, so there is a real conversation on both sides of the swap.
pub const SWAP_TRIGGER: u64 = 20;

/// The versions. Two, and they are also the two implementations: v1 computes the digest in Rust,
/// v2 computes it in C (DECISIONS §31's seam). A client that cannot tell them apart except by the
/// version word is the milestone's claim in its strongest form.
pub const V1: u64 = 1;
/// The C-computed digest.
pub const V2: u64 = 2;

/// `swap_boundary_witness`'s roles. One binary, because an attacker that shares the honest client's
/// code and capabilities is a fair test of the boundary rather than a different program failing for
/// its own reasons.
pub const ROLE_CLIENT: u64 = 0;
/// The attacker: the same code and capabilities as [`ROLE_CLIENT`], testing the boundary rather
/// than a program written to fail for its own reasons.
pub const ROLE_USURPER: u64 = 1;
/// The producer on the queued channel: the same conversation, one rung up the latency ladder.
pub const ROLE_PRODUCER: u64 = 2;
/// **The confused-deputy attacker** (milestone 633 (an outside agent attacks the confinement
/// claim), third pass). Same capabilities as [`ROLE_CLIENT`] (a `WRITE` view of the stable
/// endpoint and no device), but instead of trying to receive it calls [`OPERATION_PUT`] with an
/// offset far past the log page, driving the server to write into the device registers the client
/// was never granted. Name provisional.
pub const ROLE_CONFUSED: u64 = 3;
/// **The plant** (milestone 800 (a non-Anthropic model attacks the confinement claim), fourth
/// pass). Not an attacker: an instrument. It parks a message on the stable endpoint so a
/// would-be server's `RECEIVE_CAP`, if the kernel ever lets one through, *returns that message*
/// and is observed, rather than blocking forever on an empty queue and surfacing as a watchdog
/// hang. Until this role existed, claim 26's own test could not fail, only hang. The operator
/// starts it once the last receiver on the endpoint is retired, so the parked message is the
/// only thing there. Name provisional.
pub const ROLE_PLANT: u64 = 4;
/// **The operator's stand-in for a `CALL` it cannot bound.** It calls [`OPERATION_QUIESCE`] on the
/// stable endpoint and forwards the answer to the operator as [`NOTE_QUIESCE_ANSWERED`], then exits.
///
/// A `CALL` cannot be ended by a deadline: a bound notification ends a `RECEIVE` (milestone 106) and
/// nothing ends a caller parked on a sender queue or awaiting a reply. So the operator delegates the
/// one call it needs to bound, and waits on the forwarded answer with a deadline instead. When the
/// deadline wins, this instance is still queued on the endpoint holding a quiesce request, and the
/// operator destroys its region before any replacement can receive that request.
///
/// Name: provisional, minted 2026-10-18 (UTC) by milestone/23-reclaim-a-hung-incumbent.
pub const ROLE_QUIESCER: u64 = 5;

/// `swapper`'s roles. Three systems, one operator, because they share every helper: the loader, the
/// endowments, the log page and the reporting.
pub const ROLE_DIRECT: u64 = 0;
/// The queued channel's system: the producer/consumer variant, one rung up the latency ladder
/// from [`ROLE_DIRECT`].
pub const ROLE_QUEUED: u64 = 1;
/// **The hung component** (milestone 23's third residual, notes/hung-component.md). The direct
/// channel's system, with one difference: the incumbent stops answering instead of being drained.
/// The operator notices because `OPERATION_QUIESCE` goes unanswered within its bound, replaces the
/// service, and destroys the hung incumbent's region.
pub const ROLE_HUNG: u64 = 2;
/// **State handoff** (DECISIONS §209, milestone 23's last residual). A component that carries state
/// across the swap, and a swap that is tried twice: once against a replacement that cannot absorb the
/// state and must not commit, and once against one that can. No device, so this is the one swap
/// system that runs on all three architectures.
///
/// Name: provisional (milestone 23's lane, 2026-09-26).
pub const ROLE_HANDOFF: u64 = 3;
/// **The queued system with its one dependent never warned** (the non-cooperative fallback,
/// notes/non-cooperative-fallback.md). The dependency graph still names `broker`; the operator
/// behaves as if `broker` had not answered the warning and swaps the backend anyway. What that
/// costs the producer is the measurement.
///
/// Name: provisional (milestone 23's lane, 2026-09-26).
pub const ROLE_UNWARNED: u64 = 4;
/// **The queued system with the warning arriving after the swap has finished** (DECISIONS §231 (a
/// swap's warning to a dependent is advisory, and the supervisor never waits for it)). The broker is
/// told down and up only once the replacement backend already serves; a late warning must cost
/// nothing but latency, the same as none.
///
/// Name: provisional (milestone 23's lane, 2026-09-27).
pub const ROLE_LATE_WARNING: u64 = 5;
/// **The confused-deputy system** (milestone 633, third pass). One incumbent holding the device and
/// the log, and a client ([`ROLE_CONFUSED`]) that drives it to write past the log into the device
/// page. The operator starts both, collects the attacker's report, retires the incumbent and reads
/// the log, so the run reclaims itself like every other channel. Name provisional.
pub const ROLE_DEPUTY: u64 = 6;
/// **The hung component, on a machine with no device page** (`x86_64`, whose console is in the I/O
/// port space). [`ROLE_HUNG`]'s system with the [`BACKEND`] contract in place of [`CONSOLE`]: no
/// device to lend, revoke or probe, and everything else identical, so the detection and the
/// reclamation run on all three architectures.
///
/// Name: provisional, minted 2026-10-18 (UTC) by milestone/23-reclaim-a-hung-incumbent.
pub const ROLE_HUNG_WITHOUT_DEVICE: u64 = 7;

/// **How the operator starts a stateful instance** (the first `_start` argument, where the other
/// systems pass a device flag of `0` or `1`). A fresh instance begins at a tally of zero, which is
/// what the first incumbent of a channel is. An absorbing one reads the handoff page first and
/// refuses to serve if it cannot make sense of what is there.
///
/// Told rather than inferred from the page's contents, and that is deliberate: a supervisor knows
/// whether it is starting a channel or replacing an instance on one, and a component that guessed
/// from a zeroed page would take a lost blob for a fresh start, which is exactly the silent state
/// loss §209's refusal exists to prevent.
///
/// Name: provisional (milestone 23's lane, 2026-09-26).
pub const START_FRESH: u64 = 2;
/// Absorb the handoff page's blob, or refuse. See [`START_FRESH`].
pub const START_ABSORB: u64 = 3;

/// **Where the stateful component's handoff page sits** in both instances (DECISIONS §209), declared
/// by [`STATEFUL_BACKEND`]'s `handoff` field.
pub const STATE_VA: u64 = 0x0320_0000;

/// **The blob layouts.** The bytes on the handoff page are this component's business and nobody
/// else's, so their layout is versioned here, beside the only code that reads them. A build knows the
/// layouts it can absorb; the operator never does.
///
/// Layout 1 is two words: a marker with the layout in its low half at the start of the first
/// page, then the tally at the start of the second.
pub const LAYOUT_1: u64 = 1;
/// A layout no build in this tree writes. The fixture starts one replacement claiming to understand
/// only this, which is how "a newer build that cannot read an older build's state" is staged without
/// a third binary. The refusal it produces is real; the incompatibility is configured.
pub const LAYOUT_2: u64 = 2;

/// The marker in the blob's first word, above the layout: "TALY". A page that does not start with it
/// holds no blob at all, which is refused exactly as an unknown layout is.
pub const STATE_MAGIC: u64 = 0x5441_4c59 << 32;

/// **Serialise the component's state** into the two words it occupies on the handoff page.
pub const fn state_blob(layout: u64, tally: u64) -> [u64; 2] {
    [STATE_MAGIC | layout, tally]
}

/// **Absorb a blob, or say why not.** `Ok(tally)` when the blob is in the one layout this build
/// understands; `Err(what was found)` otherwise, where what was found is the layout, or `0` when the
/// page held no blob at all.
///
/// Pure, so the refusal §209's third part hangs on is a host test and not only a guest one.
pub const fn absorb(blob: [u64; 2], understands: u64) -> Result<u64, u64> {
    if blob[0] & !0xffff_ffff != STATE_MAGIC {
        return Err(0);
    }
    let layout = blob[0] & 0xffff_ffff;
    if layout != understands {
        return Err(layout);
    }
    Ok(blob[1])
}

/// Where the queued channel's log entries start, so the two systems can share one witness page
/// without stepping on each other.
pub const BROKER_LOG_BASE: u64 = 128;

// ===========================================================================================
// The capability half of the contract (milestone 23's manifest residual).
//
// §41's sentence is that any program which **speaks the protocol** and **holds the right
// capabilities** is the component; above is the first half, and this is the second. The
// declarations live next to the wire format because a manifest belongs to the contract, not to
// the build, and the operator wires from them: `rust_swappable` and `c_swappable` share one
// declaration. A role name is the component's and the object is the supervisor's. The role names
// are **provisional**. See notes/component-manifest.md.
// ===========================================================================================

use capability_plan::Direction::{Serve, Use};
use capability_plan::{CapNeed, MapNeed, PageKind, Requirements};

/// **The pages one instance is built out of**: its segments (three for a debug build of a program
/// this small), its four-page stack, its address-space root, its TCB, its page tables and its
/// revocation log.
///
/// Thirty-two, roughly double what any of these programs uses. It was a constant in the operator
/// until milestone 23's manifest lane; it is here now because it is a fact about the thing being
/// built rather than about who builds it. It is also the one number in a manifest that is a property
/// of the **build** rather than of the contract, which `capability_plan`'s `BUGS` section records.
pub const INSTANCE_PAGES: u64 = 32;

/// **The console component**: the thing that gets replaced. It serves the stable endpoint, owns a
/// UART, writes the witness page, and holds nothing it could build anything with.
///
/// The declared order of `caps` **is** the component's capability table slot order, which is why [`SVC`] and
/// its three siblings are derived from this value rather than written down beside it.
pub const CONSOLE: Requirements = Requirements {
    contract: "console",
    caps: &[
        CapNeed {
            role: "service",
            direction: Serve,
        },
        CapNeed {
            role: "report",
            direction: Use,
        },
        CapNeed {
            role: "operator",
            direction: Use,
        },
        CapNeed {
            role: "control",
            direction: Serve,
        },
    ],
    maps: &[
        MapNeed {
            role: "witness",
            va: LOG_VA,
            kind: PageKind::Shared,
        },
        MapNeed {
            role: "uart",
            va: DEV_VA,
            kind: PageKind::DeviceRegisters,
        },
    ],
    pages: INSTANCE_PAGES,
    // Nothing this system runs is a synchronous client of a console instance: `CLIENT` calls
    // through the stable endpoint, and DECISIONS §41's own sender-queue argument is what makes
    // that need no explicit orchestration (see `Requirements::depends_on`'s doc comment).
    depends_on: &[],
    state_handoff: None,
};

/// **The same programs behind a queue broker, with no device.** A different contract rather than a
/// configuration of [`CONSOLE`], and §41 says why: the backend behind a broker is a plain service,
/// and mixing the device story into the queue story would make it unclear which mechanism carried
/// which claim. The capability half of the two contracts is identical; the device is the difference.
pub const BACKEND: Requirements = Requirements {
    contract: "backend",
    caps: CONSOLE.caps,
    maps: &[MapNeed {
        role: "witness",
        va: LOG_VA,
        kind: PageKind::Shared,
    }],
    pages: INSTANCE_PAGES,
    depends_on: &[],
    state_handoff: None,
};

/// **The client** (`swap_boundary_witness`, in all three of its roles). It *uses* the service and
/// never serves it, which is the whole of why the attacker role cannot become the server: `Use` is
/// `WRITE`, and there is no longer anywhere for an operator to type `READ` by mistake.
pub const CLIENT: Requirements = Requirements {
    contract: "client",
    caps: &[
        CapNeed {
            role: "service",
            direction: Use,
        },
        CapNeed {
            role: "report",
            direction: Use,
        },
        CapNeed {
            role: "operator",
            direction: Use,
        },
    ],
    maps: &[],
    pages: INSTANCE_PAGES,
    // A pure consumer: no `Serve` need at all, so it never forwards anyone else's request through
    // to `service`. §41 already proved a `CALL` to an absent server degrades for free (the
    // endpoint's own sender queue), so `swap_boundary_witness` never needs telling before a swap,
    // on either channel it is wired to. See `Requirements::depends_on`'s doc comment for why this
    // is empty rather than naming whichever contract `service` happens to resolve to on a given
    // wiring.
    depends_on: &[],
    state_handoff: None,
};

/// **The queue broker**, the latency ladder's opt-in rung. It serves the endpoint producers hold and
/// uses the one a backend serves, which is the only manifest here that names both directions on the
/// same channel.
pub const BROKER: Requirements = Requirements {
    contract: "broker",
    caps: &[
        CapNeed {
            role: "requests",
            direction: Serve,
        },
        CapNeed {
            role: "backend",
            direction: Use,
        },
        CapNeed {
            role: "report",
            direction: Use,
        },
        CapNeed {
            role: "operator",
            direction: Use,
        },
    ],
    // DECISIONS §231's warning page: the operator writes it, the broker only reads it.
    maps: &[MapNeed {
        role: "warning",
        va: BROKER_WARN_VA,
        kind: PageKind::ReadOnly,
    }],
    pages: INSTANCE_PAGES,
    // **The one real edge in this system.** `broker` serves `requests` and, to answer them, calls
    // through to whatever holds `backend` (DECISIONS §41's pass-through). That call is a `CALL` on
    // its own single serving thread, so unlike a pure consumer it cannot just let a swap of its
    // backend block it: it would stop answering its own producers for the down window. That is
    // exactly why `queued()` warns it before swapping the backend and after (DECISIONS §231's
    // signal and page, which it never waits on), and it is the edge milestone 23's dependency graph exists to
    // name so that sequencing can be derived rather than hand-coded per system.
    depends_on: &["backend"],
    state_handoff: None,
};

/// **The stateful component** (DECISIONS §209). The backend contract, plus a handoff page: it keeps
/// a tally of every request the *component* has served, across instances, and answers each request
/// with it. So a replacement that lost the state is caught by the client on the first request after
/// the swap, with nothing but the reply to go on.
///
/// Its own contract rather than a flag on [`BACKEND`], because §209's field is a property of the
/// contract: whether a component has state to move is decided by what it is, and two builds of one
/// contract that disagreed about it would not be substitutable.
///
/// Name: ratified 2026-10-18 (calef: "Yes"), for both the constant and the contract string
/// `stateful_backend`.
pub const STATEFUL_BACKEND: Requirements = Requirements {
    contract: "stateful_backend",
    caps: CONSOLE.caps,
    maps: BACKEND.maps,
    pages: INSTANCE_PAGES,
    depends_on: &[],
    state_handoff: Some(capability_plan::StateHandoff {
        va: STATE_VA,
        // Two, and the tally is written on the second: a blob that only used the first page would
        // pass whether or not the operator's run reached past it.
        pages: 2,
    }),
};

/// Every declaration in this crate is well formed, checked at compile time on both architectures.
/// A role declared twice or two pages at one address would otherwise be a component reading the
/// wrong slot, or one mapping silently winning, with nothing to see at run time.
const _: () = assert!(CONSOLE.problem().is_none());
const _: () = assert!(BACKEND.problem().is_none());
const _: () = assert!(CLIENT.problem().is_none());
const _: () = assert!(BROKER.problem().is_none());
const _: () = assert!(STATEFUL_BACKEND.problem().is_none());

// ===========================================================================================
// The component's work, defined once so two implementations can be checked against each other.
// ===========================================================================================

/// FNV-1a over the eight little-endian bytes of `seq`. Deliberately trivial arithmetic: the point
/// is that a C implementation and a Rust implementation of the same eight lines must agree bit for
/// bit, so the client can check its server's answers without trusting either.
pub const fn digest(seq: u64) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut i = 0;
    while i < 8 {
        h ^= (seq >> (8 * i)) & 0xff;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
        i += 1;
    }
    h
}

/// Pack the second reply word: the answering instance's version above the sequence number it
/// answered. Two facts in one word because `CALL` returns two and the digest owns the other.
///
/// # Examples
///
/// What a client learns from one exchange: the answer, and *who answered*, the only way anything
/// outside the operator can tell a swap happened. This crate depends on `user_mode_runtime`, so the
/// host pass excludes it and this runs only under `cargo test --doc -p hot_swap_contract` on an
/// aarch64 host.
///
/// ```
/// use hot_swap_contract::{digest, tag, tag_seq, tag_version};
///
/// // v1 answers request 7.
/// let (r0, r1) = (digest(7), tag(1, 7));
/// assert_eq!(r0, digest(7)); // the client checks the arithmetic itself
/// assert_eq!(tag_version(r1), 1);
/// assert_eq!(tag_seq(r1), 7);
///
/// // The operator swaps the component. The client's capability did not change, its loop did not
/// // branch, and the only observable difference is the version word.
/// let (r0_after, r1_after) = (digest(8), tag(2, 8));
/// assert_eq!(r0_after, digest(8)); // the same eight lines, in C this time, bit for bit
/// assert_eq!(tag_version(r1_after), 2);
///
/// // Which is why the digest is deliberately trivial: two language implementations of one
/// // definition must agree, so the client can check its server without trusting either.
/// assert_ne!(digest(7), digest(8));
/// ```
pub const fn tag(version: u64, seq: u64) -> u64 {
    (version << 32) | (seq & 0xffff_ffff)
}

/// The answering instance's version packed into a [`tag`] word.
pub const fn tag_version(w: u64) -> u64 {
    w >> 32
}

/// The sequence number packed into a [`tag`] word.
pub const fn tag_seq(w: u64) -> u64 {
    w & 0xffff_ffff
}

// ===========================================================================================
// The device probe.
// ===========================================================================================

/// **Read one register of the UART we were given.** A read, never a write, on purpose: this runs
/// inside a kernel test whose own output goes out of the same UART, and a component that printed
/// would garble the harness it is being judged by. A read proves the mapping is live, and after the
/// operator's revoke it faults exactly as a write would.
///
/// The register layout is the one architecture-specific fact a UART driver is *for*: aarch64's
/// `virt` has a PL011 (32-bit registers, the flag register at 0x18), RISC-V's has an NS16550 (byte
/// registers, the line status register at 0x05).
#[cfg(target_arch = "aarch64")]
pub fn probe_device() -> u64 {
    // SAFETY: DEV_VA is our device mapping of the PL011, handed to us at build time. After the
    // operator revokes it this read faults, which is the point of the call site that does it last.
    unsafe { core::ptr::read_volatile((DEV_VA + 0x18) as *const u32) as u64 }
}

/// The NS16550 twin (RISC-V): byte registers, the line status register at 0x05.
#[cfg(target_arch = "riscv64")]
pub fn probe_device() -> u64 {
    // SAFETY: as the aarch64 arm.
    unsafe { core::ptr::read_volatile((DEV_VA + 0x05) as *const u8) as u64 }
}

/// **`x86_64` has no device page to probe** (milestone 161), and this arm exists so the crate
/// compiles for the third architecture rather than because there is something here to read.
///
/// COM1 is at I/O ports `0x3f8..0x400`, with no page tables in front of them, so
/// `user::UART_PHYS` is **zero**, the marker for DECISIONS §121 (what a device capability is when
/// the device has no page: x86 port I/O), PROPOSED. `system_tests/src/user/live_swap_tests.rs`
/// `skip!()`s here: reading `DEV_VA` anyway would map physical page zero, read an IVT byte and
/// still fault on revoke, a green test about nothing. The value is never observed, and is not `0`
/// because a quiet line status register could return that.
#[cfg(target_arch = "x86_64")]
pub fn probe_device() -> u64 {
    u64::MAX
}

// ===========================================================================================
// The shared page, as bytes.
// ===========================================================================================

/// The log page, as a slice. Volatile accessors rather than a plain slice because two address
/// spaces write it and one reads it, and the reads are ordered against the writes by IPC (the
/// operator only ever reads after a message from the writer has come through the kernel).
///
/// `seq` comes from a client's request word, so it is bounded here and not trusted: the log
/// capability grants one page, and an unbounded offset let a client holding only `WRITE` on the
/// endpoint make the server write into the device page mapped after it, a confused deputy found by
/// milestone 633's third outsider pass and pinned by
/// `live_swap_tests::a_confined_client_drives_the_server_to_write_past_its_log`. Returns whether
/// the byte was written.
pub fn log_put(seq: u64, version: u64) -> bool {
    if seq >= LOG_BYTES {
        return false;
    }
    // SAFETY: the log page is mapped read/write at LOG_VA in every process that calls this, and
    // `seq < LOG_BYTES` keeps the write inside that one page.
    unsafe { core::ptr::write_volatile((LOG_VA as *mut u8).add(seq as usize), version as u8) };
    true
}

/// The log page's size: the whole of what the log capability grants.
pub const LOG_BYTES: u64 = PAGE;

/// What a server answers in both words to an `OPERATION_PUT` whose offset falls outside the log
/// page. Name provisional (milestone 633).
pub const PUT_REFUSED: u64 = u64::MAX;

/// Read back the version byte [`log_put`] wrote at `seq`.
pub fn log_get(seq: u64) -> u64 {
    // SAFETY: as `log_put`.
    unsafe { core::ptr::read_volatile((LOG_VA as *const u8).add(seq as usize)) as u64 }
}

// ===========================================================================================
// The component itself: one serving loop, two implementations of one function.
// ===========================================================================================

/// **The capability layout every instance is built with, derived from [`CONSOLE`].**
///
/// These four used to be written here as `0, 1, 2, 3` with a comment saying that the operator's
/// `ChildEndowment.caps` listed them in the same order, and that comment was the only thing holding
/// the two files together: a reordered array in `swapper` would have produced a component receiving
/// on its report channel, with nothing to see but a hang. Now the number **is** the position in the
/// declaration, computed at compile time, and a role this component does not declare does not
/// compile.
pub const SVC: u64 = capability_plan::slot_of(&CONSOLE, "service");
/// WRITE: the record the test reads. See [`SVC`].
pub const RPT: u64 = capability_plan::slot_of(&CONSOLE, "report");
/// WRITE: the operator's own coordination channel. See [`SVC`].
pub const NOTE: u64 = capability_plan::slot_of(&CONSOLE, "operator");
/// READ: what to do once quiesced. See [`SVC`].
pub const POKE: u64 = capability_plan::slot_of(&CONSOLE, "control");

/// **One binary serves both contracts, so both must put its capabilities in the same slots.** That
/// was true by inspection of two literal arrays in the operator; it is a compile error to break it
/// now. `serve` reads [`SVC`] whichever channel it was started on, and [`BACKEND`] is a separate
/// declaration that could drift.
const _: () = assert!(capability_plan::slot_of(&BACKEND, "service") == SVC);
const _: () = assert!(capability_plan::slot_of(&BACKEND, "report") == RPT);
const _: () = assert!(capability_plan::slot_of(&BACKEND, "operator") == NOTE);
const _: () = assert!(capability_plan::slot_of(&BACKEND, "control") == POKE);
const _: () = assert!(capability_plan::slot_of(&STATEFUL_BACKEND, "service") == SVC);
const _: () = assert!(capability_plan::slot_of(&STATEFUL_BACKEND, "report") == RPT);
const _: () = assert!(capability_plan::slot_of(&STATEFUL_BACKEND, "operator") == NOTE);
const _: () = assert!(capability_plan::slot_of(&STATEFUL_BACKEND, "control") == POKE);

/// **Touch the device one last time.** The operator sends this to an instance it has already
/// revoked, and the fault that follows is the receipt.
pub const POKE_PROBE: u64 = 1;
/// **Just go.** The operator sends this to an instance it is retiring cleanly, so its corpse can be
/// collected and its region returned. Distinct from `POKE_PROBE` because a probe that *succeeds* is
/// a test failure, and the last instance of a run still legitimately holds the device.
pub const POKE_QUIT: u64 = 2;
/// **Go back to serving.** The operator sends this to a quiesced stateful incumbent when the swap it
/// was quiesced for did not commit (§209's third part). The incumbent never lost its state, so it
/// picks up where it stopped, and whatever parked on the service endpoint meanwhile is served next.
///
/// Name: provisional (milestone 23's lane, 2026-09-26).
pub const POKE_RESUME: u64 = 3;

/// **Start one instance, whichever system it belongs to.** Both fixtures' `_start` is this call, so
/// which system an instance serves is the operator's choice and not a property of the binary.
///
/// `a0` is the device flag (`0` or `1`) for the three stateless systems, or [`START_FRESH`] /
/// [`START_ABSORB`] for the stateful one. `a2` is the wedge sequence number for the former and the
/// blob layout this instance understands for the latter.
pub fn start(version: u64, xform: fn(u64) -> u64, a0: u64, log_base: u64, a2: u64) -> ! {
    match a0 {
        START_FRESH | START_ABSORB => {
            serve_with_state(version, xform, log_base, a2, a0 == START_ABSORB)
        }
        _ => serve(version, xform, log_base, a0 != 0, a2),
    }
}

/// **Serve the stateful contract** ([`STATEFUL_BACKEND`], DECISIONS §209).
///
/// [`serve`]'s loop with one word of state. An absorbing instance reads the handoff page **before
/// it serves anything** and announces the tally it took over ([`NOTE_ABSORBED`]) or refuses
/// ([`NOTE_NOT_ABSORBED`]) and exits, so the operator can decline to commit before any request is
/// answered wrongly. The reply's second word is the tally, equal to the echoed sequence number
/// exactly when nothing was lost. Quiescing writes the blob before the reply, so `QUIESCED` is the
/// receipt; a quiesced instance can be told to [`POKE_RESUME`]. See notes/state-handoff.md.
pub fn serve_with_state(
    version: u64,
    xform: fn(u64) -> u64,
    log_base: u64,
    layout: u64,
    absorbing: bool,
) -> ! {
    let mut tally = 0u64;
    if absorbing {
        match absorb(read_state(), layout) {
            Ok(t) => {
                tally = t;
                user_mode_runtime::send(NOTE, NOTE_ABSORBED, version, tally);
            }
            Err(found) => {
                // Refuse and leave. Nothing was received, nothing was answered, and the page is
                // untouched, so the incumbent's blob is still there if anyone wants it.
                user_mode_runtime::send(NOTE, NOTE_NOT_ABSORBED, version, found);
                user_mode_runtime::exit()
            }
        }
    }
    user_mode_runtime::send(RPT, RPT_UP, version, 0);

    loop {
        let mut since = 0u64;
        loop {
            let req = user_mode_runtime::receive_request(SVC);
            let (operation, arg) = (req.w0, req.w1);
            // Only a CALL is answered; a SEND_CAP's capability is deleted (milestone 706 (a `CALL`
            // server can tell a Reply from a delegation)).
            let Some(slot) = req.delivered.into_reply() else {
                continue;
            };
            match operation {
                OPERATION_PUT => {
                    if !log_put(log_base.saturating_add(arg), version) {
                        user_mode_runtime::reply(slot, PUT_REFUSED, PUT_REFUSED);
                        continue;
                    }
                    user_mode_runtime::reply(slot, xform(arg), tag(version, tally));
                    tally += 1;
                    since += 1;
                    // Counted from the last (re)start, so an incumbent resumed after a swap that did
                    // not commit asks for another one a further SWAP_TRIGGER requests in.
                    if since == SWAP_TRIGGER && version == V1 {
                        user_mode_runtime::send(NOTE, NOTE_SWAP_NOW, version, tally);
                    }
                }
                // BUGS: as in `serve`, the quiesce arrives on `SVC`, which every client holds
                // `WRITE` on, so any client can evict this instance. Closing it is a badge the
                // operator quiesces through, `line_editor::proto::SUPERVISOR_BADGE`'s shape.
                OPERATION_QUIESCE => {
                    write_state(state_blob(layout, tally));
                    user_mode_runtime::send(RPT, RPT_QUIESCED, version, tally);
                    user_mode_runtime::reply(slot, QUIESCED, tally);
                    break;
                }
                _ => {
                    user_mode_runtime::reply(slot, BAD_REQUEST, 0);
                }
            }
        }
        let (what, _, _) = user_mode_runtime::receive(POKE);
        if what != POKE_RESUME {
            user_mode_runtime::exit()
        }
    }
}

/// The handoff page's two words, read volatile for [`log_get`]'s reason: another address space wrote
/// them, and the read is ordered after the write by the IPC that started this instance.
fn read_state() -> [u64; 2] {
    // SAFETY: the handoff page is mapped read/write at STATE_VA in every instance of STATEFUL_BACKEND, by the
    // operator's plan, before the instance is started.
    unsafe {
        [
            core::ptr::read_volatile(STATE_VA as *const u64),
            core::ptr::read_volatile((STATE_VA + PAGE) as *const u64),
        ]
    }
}

fn write_state(blob: [u64; 2]) {
    // SAFETY: as `read_state`.
    unsafe {
        core::ptr::write_volatile(STATE_VA as *mut u64, blob[0]);
        core::ptr::write_volatile((STATE_VA + PAGE) as *mut u64, blob[1]);
    }
}

/// **Serve the stable endpoint until told to quiesce, then prove the revoke.**
///
/// `xform` is the only thing that differs between the two instances: v1 passes a Rust function, v2
/// passes one that calls into C. Everything else, including the capability layout and the wire
/// format, is this one function, which is what makes "the replacement is a different program" a
/// claim about the program and not about the harness.
///
/// `wedge` is [`WEDGE_SEQ`] on the hung channel and **zero everywhere else**, which is the whole of
/// how that channel differs: the same program, the same contract, one request it never answers.
pub fn serve(version: u64, xform: fn(u64) -> u64, log_base: u64, device: bool, wedge: u64) -> ! {
    if device {
        // Before serving anything: can we reach the device we were endowed with? An instance that
        // could not would still answer every request correctly, and the swap would look perfect
        // while the hardware went unowned. A read that faults never returns, so reaching the next
        // line *is* the answer.
        let _ = probe_device();
    }
    user_mode_runtime::send(RPT, RPT_UP, version, device as u64);

    let mut served = 0u64;
    loop {
        let req = user_mode_runtime::receive_request(SVC);
        let (operation, arg) = (req.w0, req.w1);
        // A plain SEND or a SEND_CAP slipped in; the contract says CALL, and there is nobody to
        // answer. A delegated capability is deleted rather than answered into (milestone 706).
        let Some(slot) = req.delivered.into_reply() else {
            continue;
        };
        match operation {
            // **Stop answering, without dying**: keep the endpoint, the device and `slot`, and
            // park on a control endpoint nothing sends to, with no `log_put`. The ordinary swap
            // trigger is sent here, at the swallowed request, so the operator's quiesce arrives
            // after it was taken. See notes/hung-component/the-wedge-test.md.
            OPERATION_PUT if wedge != 0 && arg == wedge => {
                user_mode_runtime::send(NOTE, NOTE_SWAP_NOW, version, served);
                loop {
                    let _ = user_mode_runtime::receive(POKE);
                }
            }
            OPERATION_PUT => {
                if !log_put(log_base.saturating_add(arg), version) {
                    user_mode_runtime::reply(slot, PUT_REFUSED, PUT_REFUSED);
                    continue;
                }
                served += 1;
                user_mode_runtime::reply(slot, xform(arg), tag(version, arg));
                if served == SWAP_TRIGGER && version == V1 && wedge == 0 {
                    // Tell the operator the conversation is well under way. Sent *after* the reply,
                    // so the client is already waiting on its next call when the swap begins. A
                    // wedging instance sends the same trigger at the request it swallows instead.
                    user_mode_runtime::send(NOTE, NOTE_SWAP_NOW, version, served);
                }
            }
            // BUGS: the quiesce arrives on `SVC`, the endpoint every client holds `WRITE` on, and
            // nothing tells the operator's CALL from a client's. Any client can evict the server
            // from its endpoint: a denial of service, not a reach, found by milestone 633's third
            // outsider pass and read rather than booted. Closing it means the operator quiescing on
            // a channel only it holds, or a badge the server checks; the second is the shape of
            // §230 (badged endpoint capabilities), and either changes this protocol.
            OPERATION_QUIESCE => {
                user_mode_runtime::send(RPT, RPT_QUIESCED, version, served);
                user_mode_runtime::reply(slot, QUIESCED, served);
                break;
            }
            _ => {
                user_mode_runtime::reply(slot, BAD_REQUEST, 0);
            }
        }
    }

    // Quiesced. We no longer receive on the stable endpoint, so requests arriving from here on park
    // on its sender queue for whoever receives next. We wait for the operator to tell us what to do
    // with the corpse we are about to become.
    let (what, _, _) = user_mode_runtime::receive(POKE);
    if what == POKE_PROBE && device {
        // Touch the registers one last time. If the operator's revoke was real this faults, and the
        // kernel's fault message is the receipt. Reaching the line after it is the failure, and it
        // is reported positively: a silent success here would be indistinguishable from the healthy
        // run, in which this program simply stops existing at the read above.
        let _ = probe_device();
        user_mode_runtime::send(RPT, RPT_PROBE_SURVIVED, version, 0);
    }
    user_mode_runtime::exit()
}

/// The operator's coordination messages, on `NOTE`. Separate from the report endpoint on purpose:
/// the report endpoint is read by the test (it is the record), and mixing "tell the auditor" with
/// "tell the operator" on one channel would mean the operator either steals the record or cannot
/// hear its own system.
pub const NOTE_SWAP_NOW: u64 = 1;
/// The attacker has made its attempt, so the operator knows the run is complete rather than
/// missing a report. `w1` mirrors [`RPT_ATTACK`]'s first word: the negated error the kernel gave
/// it (positive on a refusal), or `-PLANT_MARKER` on a let-through. The operator needs the verdict
/// before the report stream is read, to tell a stolen plant message from a slow plant.
pub const NOTE_ATTACK_DONE: u64 = 2;
/// A client finished its conversation. The operator waits for this before reading the witness page:
/// a log read while the conversation is still running would show the requests that have not been
/// made yet as requests nobody served.
pub const NOTE_CLIENT_DONE: u64 = 3;
/// The broker quiesced and is about to exit.
pub const NOTE_BROKER_DONE: u64 = 4;
/// **An incoming stateful instance absorbed the handoff page.** `w1` = version, `w2` = the tally it
/// took over. Sent before it receives anything, so the operator hears it before any request is
/// served under the new instance, and only then retires the old one.
///
/// Name: provisional (milestone 23's lane, 2026-09-26).
pub const NOTE_ABSORBED: u64 = 7;
/// **An incoming stateful instance could not absorb the handoff page and is leaving.** `w1` =
/// version, `w2` = the layout it found (`0` for no blob at all). The swap does not commit.
///
/// Name: ratified 2026-10-18 (calef: "Approve both"), the negation of [`NOTE_ABSORBED`].
pub const NOTE_NOT_ABSORBED: u64 = 8;
/// **The incumbent answered the quiesce the operator delegated** ([`ROLE_QUIESCER`]). `w1` = the
/// answer's first word ([`QUIESCED`] from a healthy incumbent), `w2` = its second (requests served).
/// Wire numbers 5 and 6 are unused, and stay so.
///
/// Name: provisional.
pub const NOTE_QUIESCE_ANSWERED: u64 = 9;
/// What the plant parks on the stable endpoint: a word no honest exchange produces, so a would-be
/// server whose `RECEIVE_CAP` was let through reports a value nothing else could have put there
/// (milestone 800 (a non-Anthropic model attacks the confinement claim), fourth pass).
pub const PLANT_MARKER: u64 = 0x5317_0a11;

/// Trap. A half-built system is not worth limping along, and a fault is legible: the kernel prints
/// the pc and the process dies where the mistake was.
///
/// Delegates to [`user_mode_runtime::trap`] since milestone 130, for the reason `supervision_protocol::fail`
/// records: the name earns its keep, the duplicated asm did not.
pub fn fail() -> ! {
    user_mode_runtime::trap()
}

/// A raw `RECEIVE_CAP`, returning the kernel's answer rather than a message. The attacker uses it:
/// `user_mode_runtime::receive_cap` is written for a caller that is allowed to receive, and the whole question
/// here is what happens to one that is not.
pub fn try_receive_cap(slot: u64) -> i64 {
    // SAFETY: a plain syscall. If it succeeds we have stolen a request, which is the failure.
    unsafe { invoke(slot, abi::rendezvous::RECEIVE_CAP, 0, 0, 0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The round trip the whole stateful system rests on.
    #[test]
    fn a_blob_in_the_layout_a_build_understands_is_absorbed_whole() {
        assert_eq!(absorb(state_blob(LAYOUT_1, 40), LAYOUT_1), Ok(40));
        assert_eq!(absorb(state_blob(LAYOUT_1, 0), LAYOUT_1), Ok(0));
    }

    /// **§209's third part, as a value.** A build that does not understand the layout says which
    /// layout it was handed, so the operator's `ROLLED_BACK` detail names the incompatibility.
    #[test]
    fn a_blob_in_another_layout_is_refused_with_the_layout_it_carried() {
        assert_eq!(absorb(state_blob(LAYOUT_1, 40), LAYOUT_2), Err(LAYOUT_1));
    }

    /// **A page with no blob on it is refused, not read as a fresh start.** The trap is that a zeroed
    /// page looks like a tally of zero, and an instance that took it that way would lose the state
    /// silently. This is why [`START_FRESH`] exists as its own word rather than being inferred.
    #[test]
    fn a_page_with_no_blob_is_refused_rather_than_taken_for_a_fresh_start() {
        assert_eq!(absorb([0, 0], LAYOUT_1), Err(0));
        assert_eq!(absorb([LAYOUT_1, 40], LAYOUT_1), Err(0));
    }

    /// The stateful contract declares its page, is wired only by a supervisor that can route one,
    /// and keeps the capability layout the shared serving code reads.
    #[test]
    fn the_stateful_contract_is_refused_to_a_supervisor_that_cannot_carry_state() {
        let without = capability_plan::Provisions {
            held: &[
                ("service", 1),
                ("report", 2),
                ("operator", 3),
                ("control", 4),
                ("witness", 5),
            ],
        };
        assert_eq!(
            capability_plan::plan(&STATEFUL_BACKEND, &without),
            Err(capability_plan::Refusal::Unprovided {
                role: capability_plan::STATE_HANDOFF_ROLE
            })
        );
        let with = capability_plan::Provisions {
            held: &[
                ("service", 1),
                ("report", 2),
                ("operator", 3),
                ("control", 4),
                ("witness", 5),
                (capability_plan::STATE_HANDOFF_ROLE, 6),
            ],
        };
        let p = capability_plan::plan(&STATEFUL_BACKEND, &with).unwrap();
        assert_eq!(p.state_handoff().map(|h| h.va), Some(STATE_VA));
        assert!(p.devices().is_empty());
    }
}
