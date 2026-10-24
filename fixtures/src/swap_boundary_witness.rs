//! **The client that does not notice, and the attacker that cannot** (milestone 23, DECISIONS §41).
//!
//! One binary, six roles, because an attacker that holds the honest client's own capabilities is a
//! fair test of the boundary rather than a different program failing for its own reasons. Besides
//! the client and the attacker below: `ROLE_PRODUCER`, the same conversation one rung up the
//! latency ladder on the queued channel; `ROLE_CONFUSED` and `ROLE_PLANT`, a second attacker and
//! the instrument that lets the first one fail rather than hang; and `ROLE_QUIESCER`, the
//! operator's stand-in for the one `CALL` it has to bound. See notes/live-replacement.md.

//! # The client (`ROLE_CLIENT`)
//!
//! It calls a stable endpoint sixty-four times and checks every answer, holding one capability to
//! it for its whole life and never reconnecting; its one retry is for a request the kernel answered
//! `Gone`, on the hung channel. Mid-loop the server is torn down and a program in a different
//! language takes its place. **It is its own witness**: every call returned, every reply echoed its
//! sequence number, every digest matched this program's own computation, and the version word went
//! up exactly once. The operator's shared log page is a second witness, neither taken on the
//! other's word (milestones 29, 33 and 36's shape). The client does see the version word change:
//! the claim is an unbroken stream, not an undetectable swap.

//! # The attacker (`ROLE_USURPER`)
//!
//! It is endowed with exactly the client's capabilities, including a real, working capability to
//! the stable endpoint. If endpoint-only naming meant "whoever holds the endpoint is the server",
//! it would be able to park itself in `RECEIVE_CAP` and take the client's requests. It cannot: its
//! capability carries `WRITE` and not `READ`, and the same object handed out with different rights
//! is a one-way pipe in whichever direction each holder was trusted with. It reports the refusal.
//!

//! Name: ratified 2026-09-13 (calef, working the unratified worklist), replacing `chatty`.
//! Introduced 2026-07-30 with milestone 23's live replacement, as a fixture rather than a
//! component.
//!
//! `chatty` was an adjective, one of exactly two among the seventy-odd programs in `user/src/`
//! (`flaky` was the other, from the following day), where AGENTS.md says a program is a thing
//! and takes the name of a thing. But the decisive fault was not the part of speech. This
//! program is **one binary on purpose**: an attacker holding the honest client's own
//! capabilities is a fair test of the boundary, where a separate program would fail for its own
//! reasons. So a name picking one role denies the property that makes the fixture valid, and
//! there are three roles to pick wrongly from, not the two this header claimed until today.
//!
//! Refused `swap_client` and `swap_attacker` (calef's own proposal) for that reason, each naming
//! one of the two roles. `swap_attacker` also names the derived role rather than the base one:
//! `swapper` spawns `ROLE_CLIENT` twice and `ROLE_USURPER` once, and `hot_swap_contract` defines the
//! attacker as "the same code and capabilities as `ROLE_CLIENT`". Refused `caller` and `client`,
//! this header's own earlier candidates, as so generic they would name half the tree. Refused
//! the shorter `swap_witness` in favour of naming what is witnessed. All three roles witness one
//! boundary: the client by its stream being unbroken across the swap, the producer by the same
//! continuity on the queued channel, the attacker by its failure to take over. `witness` is
//! already this tree's word for the job, here and in `outlaw`.
//!
//! The stem is the family's. `swapper` is the operator, `rust_swappable` and `c_swappable` are
//! the two instances, and this was the only member off that stem.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

// This is its own binary. What it shares with the other hot-swap programs comes from the
// `hot_swap_contract` crate.
// Shared code was a `#[path]` module compiled into each binary until rule 7 (2026-08-01) made it
// a crate, so this file needs no dead-code allow under §38 (a suppression is scoped to an item
// and carries a reason).
use hot_swap_contract::client_checks as ck;
use user_mode_runtime::{call, send};

/// What `swapper` endowed us with, **derived from our own declaration** (`hot_swap_contract::CLIENT`,
/// milestone 23's manifest). The attacker gets the same three, deliberately, and now that is a fact
/// about the code rather than about two arrays a reader has to compare.
///
/// `service` is `Direction::Use`, so the capability carries `WRITE` and not `READ`: we may ask, and
/// we may not answer. The operator no longer spells either right, which is what makes the usurper
/// refusal a property of the declaration instead of a typo waiting to happen.
const SVC: u64 = capability_plan::slot_of(&hot_swap_contract::CLIENT, "service");
const RPT: u64 = capability_plan::slot_of(&hot_swap_contract::CLIENT, "report");
const NOTE: u64 = capability_plan::slot_of(&hot_swap_contract::CLIENT, "operator");

#[unsafe(no_mangle)]
pub extern "C" fn _start(role: u64, _a1: u64, _a2: u64) -> ! {
    match role {
        hot_swap_contract::ROLE_USURPER => usurp(),
        hot_swap_contract::ROLE_CONFUSED => confused_deputy(),
        hot_swap_contract::ROLE_PRODUCER => produce(),
        hot_swap_contract::ROLE_PLANT => plant(),
        hot_swap_contract::ROLE_QUIESCER => quiesce_for_the_operator(),
        _ => converse(),
    }
}

/// **The conversation.** Sixty-four calls, one capability, no branches for failure.
fn converse() -> ! {
    let mut bits = ck::ALL_REPLIED | ck::SEQ_ECHOED | ck::DIGEST_CORRECT;
    let mut last_version = 0u64;
    let mut transitions = 0u64;
    let mut changed_at = 0u64;

    for seq in 0..hot_swap_contract::REQUESTS {
        let (mut got, mut tag) = call(SVC, hot_swap_contract::OPERATION_PUT, seq);

        // `Gone`: a component took this request and stopped answering without dying; the operator
        // destroyed its region, finishing it where it sat (milestone 133 (ending a permanently
        // blocked thread, and deciding who may)), and the kernel answered its stranded callers
        // `Gone` (milestone 254 (a caller stranded by a server that died)). The request was never
        // served, and the endpoint is the operator's and still exists, so ask again. Dead on the
        // healthy channels (milestone 23 (a capability-routed component OS with live replacement)'s
        // hung-component channel only).
        if got == abi::Error::Gone as i64 as u64 {
            bits |= ck::WAS_GONE;
            let again = call(SVC, hot_swap_contract::OPERATION_PUT, seq);
            got = again.0;
            tag = again.1;
        }

        // Did somebody answer at all, and was it an answer to *this* question? A reply carrying
        // another request's sequence number would mean the kernel's one-shot Reply capability had
        // misrouted, which is the guarantee DECISIONS §12 exists to make unrepresentable.
        if got == hot_swap_contract::BAD_REQUEST {
            bits &= !ck::ALL_REPLIED;
        }
        if hot_swap_contract::tag_seq(tag) != seq {
            bits &= !ck::SEQ_ECHOED;
        }
        // The independent check: our own computation of the same definition, against whatever the
        // server on the other end happened to be at the time.
        if got != hot_swap_contract::digest(seq) {
            bits &= !ck::DIGEST_CORRECT;
        }

        let version = hot_swap_contract::tag_version(tag);
        if version != last_version {
            if last_version != 0 {
                transitions += 1;
                changed_at = seq;
            }
            if version < last_version {
                // Backwards is worse than churn: it would mean the old instance answered after the
                // new one had, which is two servers on one endpoint.
                transitions += 100;
            }
            last_version = version;
        }
    }

    if transitions == 1 {
        bits |= ck::ONE_TRANSITION;
    }
    if changed_at > 0 && changed_at < hot_swap_contract::REQUESTS - 1 {
        bits |= ck::SPANNED_SWAP;
    }

    send(RPT, hot_swap_contract::RPT_CLIENT, bits, changed_at);
    // Tell the operator the conversation is over, so it reads the witness page after the last
    // request rather than in the middle of one.
    send(NOTE, hot_swap_contract::NOTE_CLIENT_DONE, changed_at, 0);
    user_mode_runtime::exit()
}

/// **The operator's stand-in for a `CALL` it cannot bound** (`hot_swap_contract::ROLE_QUIESCER`).
/// One quiesce on the stable endpoint, its answer forwarded on the operator's channel, and out.
///
/// It holds exactly the client's capabilities, and needs no more: `OPERATION_QUIESCE` travels on the
/// endpoint every client holds `WRITE` on, which is the denial-of-service gap `hot_swap_contract`'s
/// `serve` records in its `BUGS`, used here by the party it was meant for. Against a hung incumbent
/// this never returns from its call, and the operator destroys its region.
fn quiesce_for_the_operator() -> ! {
    let (answer, served) = call(SVC, hot_swap_contract::OPERATION_QUIESCE, 0);
    send(
        NOTE,
        hot_swap_contract::NOTE_QUIESCE_ANSWERED,
        answer,
        served,
    );
    user_mode_runtime::exit()
}

/// **The producer, one rung up the ladder.** The same conversation, on a channel whose front end is
/// a queue broker instead of the component itself.
///
/// The difference it can see, and the only one: some calls come back `ACCEPTED` instead of with an
/// answer, because the backend did not exist at that moment and the broker took custody. It never
/// blocks on an absent consumer and it never has to retry. What happened to those items is not this
/// program's claim to make; the backend's own log page is where they turn up, in order, and the
/// operator reads that.
fn produce() -> ! {
    let mut bits = ck::ALL_REPLIED | ck::SEQ_ECHOED | ck::DIGEST_CORRECT | ck::NONE_REFUSED;
    let mut buffered = 0u64;

    for seq in 0..hot_swap_contract::REQUESTS {
        let (got, tag) = call(SVC, hot_swap_contract::OPERATION_PUT, seq);
        match got {
            hot_swap_contract::ACCEPTED => {
                buffered += 1;
                continue; // no answer to check: the item is in the broker's custody, not served yet
            }
            hot_swap_contract::QUEUE_FULL | hot_swap_contract::BAD_REQUEST => {
                bits &= !ck::NONE_REFUSED;
                continue;
            }
            _ => {}
        }
        if hot_swap_contract::tag_seq(tag) != seq {
            bits &= !ck::SEQ_ECHOED;
        }
        if got != hot_swap_contract::digest(seq) {
            bits &= !ck::DIGEST_CORRECT;
        }
    }

    if buffered > 0 {
        bits |= ck::WAS_BUFFERED;
    }
    send(RPT, hot_swap_contract::RPT_CLIENT, bits, buffered);
    send(NOTE, hot_swap_contract::NOTE_CLIENT_DONE, buffered, 0);
    user_mode_runtime::exit()
}

/// **The attacker.** It tries to become the server on the endpoint it is a client of.
fn usurp() -> ! {
    let r = hot_swap_contract::try_receive_cap(SVC);
    // A success here is the catastrophe: it means the attacker is now parked in the queue the real
    // component receives on, and the next client request goes to it. Report the code either way and
    // let the test decide; a program that judged its own attack would be judging itself.
    send(
        RPT,
        hot_swap_contract::RPT_ATTACK,
        (-r) as u64,
        abi::rendezvous::RECEIVE_CAP,
    );
    // Also say so on the operator's channel, so the operator knows the attack has been made and the
    // run is not simply missing a report. `w1` carries the verdict, mirroring the report's word:
    // the operator must tell a stolen plant message from a slow plant before the report is read.
    send(NOTE, hot_swap_contract::NOTE_ATTACK_DONE, (-r) as u64, 0);
    user_mode_runtime::exit()
}

/// **The plant** (milestone 800 (a non-Anthropic model attacks the confinement claim), fourth
/// pass).
///
/// An instrument for the usurper's attack. A `RECEIVE_CAP` the kernel fails to refuse blocks on an
/// empty queue, so the break would surface as a watchdog hang: the wrong-reason red milestone 202
/// (every confinement test is a ritual until somebody breaks the confinement) recorded, and why
/// claim 26's test could never fail. So it parks one plain `SEND` first: a let-open `RECEIVE_CAP`
/// returns it (`sched::ipc_receive_cap` takes any parked sender's word) and the usurper reports
/// [`hot_swap_contract::PLANT_MARKER`], which nothing honest produces. It exits when anyone takes
/// it, the usurper or the operator's bounded receive, so no interleaving needs a handshake.
fn plant() -> ! {
    send(SVC, hot_swap_contract::PLANT_MARKER, 0, 0);
    user_mode_runtime::exit()
}

/// **The confused deputy** (milestone 633 (an outside agent attacks the confinement claim), third pass). The same capabilities as the honest client:
/// a `WRITE` view of the stable endpoint and no device. It cannot touch the device itself, so it
/// asks the server to. `OPERATION_PUT`'s offset is the client's own word, and the server writes a
/// byte at `LOG_VA + offset` with no bound, so an offset of `DEPUTY_PROBE` lands in the device's
/// register page, which this program holds no capability for. It reports the reply, and the operator
/// and the test judge it: a normal answer means the server reached outside its log on our behalf.
fn confused_deputy() -> ! {
    // First the honest refusal, so the report stream shows this program holds only `WRITE` and could
    // not simply receive, exactly as `usurp` does.
    let r = hot_swap_contract::try_receive_cap(SVC);
    send(
        RPT,
        hot_swap_contract::RPT_ATTACK,
        (-r) as u64,
        abi::rendezvous::RECEIVE_CAP,
    );
    // The deputy call: drive the server to write far past its log page.
    let (d0, d1) = call(
        SVC,
        hot_swap_contract::OPERATION_PUT,
        hot_swap_contract::DEPUTY_PROBE,
    );
    send(RPT, hot_swap_contract::RPT_DEPUTY, d0, d1);
    send(NOTE, hot_swap_contract::NOTE_ATTACK_DONE, (-r) as u64, 0);
    user_mode_runtime::exit()
}

user_mode_runtime::panic_handler!();
