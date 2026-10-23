//! **The `Rendezvous` methods of the IPC round trip**: `SEND`, `RECEIVE`, `SEND_CAP`,
//! `RECEIVE_CAP` and `CALL`, with `SURVEY` beside them.
//!
//! Every function here is `#[inline(always)]`, and that is the point of the file: a child module
//! can land in another codegen unit, where a plain function is not inlined, and the round trip
//! would pay a call while its bytes left the `syscall_entry` `script/fastpath-footprint` counts.
//! `BADGE` and `REAP` are rare and live out of line in `object_methods`. See
//! notes/syscall-boundary.md.
//!
//! Name: provisional (lane maintainer/long-function-paydown-invoke, 2026-10-12 UTC), for the
//! module, the file and each function in it. calef names things; expect these to change.

use super::*;
use crate::sched::RendezvousId;

/// `Rendezvous::SEND`. SEND takes WRITE, RECEIVE takes READ. The *same* endpoint, handed out with
/// different rights, is a one-way pipe in whichever direction each holder was trusted with.
#[inline(always)]
pub(super) fn rendezvous_send(
    rights: Rights,
    ep: RendezvousId,
    badge: u64,
    words: [u64; 3],
) -> Result<i64, Error> {
    if !rights.allows(Rights::WRITE) {
        return Err(Error::NotPermitted);
    }
    // The badge rides in word 3: a writer's identity is its capability's badge, never its claim.
    sched::ipc_send_badged(ep, words, badge);
    // An abort is a revoked endpoint (`Gone`, not `NoSuchSlot`: a pipe writer acts on the
    // difference, notes/sink-protocol.md) or an interrupt's endpoint refusing the deposit
    // (`NotPermitted`, DECISIONS §101 (notification objects), ruling B). Asked only on abort.
    if sched::take_ipc_aborted() {
        return Err(aborted_send_error());
    }
    Ok(0)
}

/// `Rendezvous::RECEIVE`, which takes READ (see [`rendezvous_send`]).
#[inline(always)]
pub(super) fn rendezvous_receive(
    frame: &mut TrapFrame,
    rights: Rights,
    ep: RendezvousId,
) -> Result<i64, Error> {
    if !rights.allows(Rights::READ) {
        return Err(Error::NotPermitted);
    }
    let msg = sched::ipc_receive(ep);
    if sched::take_ipc_aborted() {
        return Err(Error::Gone); // endpoint revoked; the message is a placeholder
    }
    // Word 0 is the result; the other four words of a message go straight to their registers.
    frame.set_arg(1, msg[1]);
    frame.set_arg(2, msg[2]);
    frame.set_arg(3, msg[3]);
    frame.set_arg(4, msg[4]);
    Ok(msg[0] as i64)
}

/// `Rendezvous::SEND_CAP`: delegation. `slot` is the slot of the capability to pass on, `narrow`
/// the rights to narrow it to, `word` one data word. Two rights are in play and they are different
/// questions: WRITE on *this* endpoint (may I send here?) and GRANT on the *delegated* capability
/// (was I trusted to pass it on?). Without GRANT you may use a thing and not lend it.
#[inline(always)]
pub(super) fn rendezvous_send_capability(
    rights: Rights,
    ep: RendezvousId,
    badge: u64,
    [slot, narrow, word]: [u64; 3],
) -> Result<i64, Error> {
    if !rights.allows(Rights::WRITE) {
        return Err(Error::NotPermitted);
    }
    // The source is read, checked and copied inside `sched`, under the hold that files the
    // copy, so no revocation sweep falls between the read and the filing (`sched::Delegation`).
    #[cfg(feature = "system_tests")]
    crate::delegation_pause::here(); // no lock held
    sched::ipc_delegate_cap(
        ep,
        word,
        sched::Delegation {
            slot,
            rights: Rights::from_bits(narrow as u32),
        },
        badge, // the endpoint we send on may be badged (milestone 599 (a frame per filesystem client channel))
    )?;
    if sched::take_ipc_aborted() {
        // Revoked, or refused (§101 ruling B); either way the delegation did not happen and the
        // capability is still the sender's.
        return Err(aborted_send_error());
    }
    Ok(0)
}

/// `Rendezvous::RECEIVE_CAP`, which takes READ like `RECEIVE`.
#[inline(always)]
pub(super) fn rendezvous_receive_capability(
    frame: &mut TrapFrame,
    rights: Rights,
    ep: RendezvousId,
) -> Result<i64, Error> {
    if !rights.allows(Rights::READ) {
        return Err(Error::NotPermitted);
    }
    let msg = sched::ipc_receive_cap(ep);
    if sched::take_ipc_aborted() {
        return Err(Error::Gone); // endpoint revoked; the message is a placeholder
    }
    // x1 the slot the capability landed in or `NO_CAP`, x2 the second word, x3 the sender's
    // badge. x4 is `BOUND`, `REPLY_DELIVERED` (§245 (a `CALL` server tells a Reply from a
    // delegation)) or 0, and is the one register no sender can write.
    frame.set_arg(1, msg[1]);
    frame.set_arg(2, msg[2]);
    frame.set_arg(3, msg[3]);
    frame.set_arg(4, msg[4]);
    Ok(msg[0] as i64)
}

/// `Rendezvous::CALL`: send two words and block until replied. The kernel mints a one-shot Reply
/// cap naming us into the server (delivered by its `RECEIVE_CAP`); we return here only when the
/// server invokes it. See §12 (Call/Reply IPC: a one-shot reply capability) and
/// notes/ipc-naming.md. Sending needs WRITE, like SEND.
#[inline(always)]
pub(super) fn rendezvous_call(
    frame: &mut TrapFrame,
    rights: Rights,
    ep: RendezvousId,
    badge: u64,
    words: [u64; 2],
) -> Result<i64, Error> {
    if !rights.allows(Rights::WRITE) {
        return Err(Error::NotPermitted);
    }
    let reply = sched::ipc_call_badged(ep, words, badge);
    if sched::take_ipc_aborted() {
        return Err(aborted_send_error()); // revoked or refused; no call, no reply
    }
    frame.set_arg(1, reply[1]); // r1; r0 returns in x0 below
    // Written on every CALL, so a caller reads the kernel's word (a filed slot or `NO_CAP`),
    // never what it left in x2 itself.
    frame.set_arg(2, reply[2]);
    Ok(reply[0] as i64)
}

/// `Rendezvous::SURVEY`: **one entry of the domain this endpoint supervises**, from `cursor` (0
/// to start), as the per-thread fact `record` selects. Writes the tid to x1 and the record's word
/// to x2, and returns the next cursor or `survey::DONE`.
///
/// Scoped by the relationship `REAP` uses, so a supervisor sees exactly the children whose deaths
/// arrive here. A holder without `ENUMERATE` is refused, never shown an empty domain. The
/// selector, the placement record's authority argument and what a placement leaks are in
/// notes/syscall-boundary.md.
#[inline(always)]
pub(super) fn rendezvous_survey(
    frame: &mut TrapFrame,
    rights: Rights,
    ep: RendezvousId,
    cursor: u64,
    record: u64,
) -> Result<i64, Error> {
    // `ENUMERATE`, not `READ`: `READ` also unlocks `RECEIVE` and `REAP`, and a viewer must not
    // be able to reap a child.
    if !rights.allows(Rights::ENUMERATE) {
        return Err(Error::NotPermitted);
    }
    let (next, tid, word) = sched::survey_supervised(ep, cursor, record)?;
    frame.set_arg(1, tid);
    frame.set_arg(2, word);
    Ok(next as i64)
}
