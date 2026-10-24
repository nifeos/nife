//! The delegation demo, the half that takes (milestone 10 (a shell at EL0, and processes spawned
//! on command)); the other half is `delegation_granter`.
//!
//! Holds the channel (slot 0), a report endpoint (slot 1), and a loopback endpoint (slot 2) it
//! uses only to *attempt* re-delegation. It reports on three things: the capability arrives, it
//! carries real authority when a process that did not mint it invokes it, and the kernel refuses
//! to let it travel further because it was handed over without `GRANT`.
//!
//! Name: provisional (milestone 291). This was `hello`'s `RECEIVER` role, number 10. See
//! `delegation_granter` for why the qualifier: `receiver` alone could name almost anything that
//! holds the read end of a channel.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use abi::rendezvous;
use user_mode_runtime::{exit, receive_cap, send, send_capability};

const CHANNEL: u64 = 0;
const REPORT: u64 = 1;
const LOOPBACK: u64 = 2;

#[unsafe(no_mangle)]
pub extern "C" fn _start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    // Receive the delegated capability. It lands in a fresh slot of our own capability table;
    // RECEIVE_CAP tells us which one. We were never told the slot in advance: the kernel chose it and
    // named it to us.
    let (_data, got, _) = receive_cap(CHANNEL);
    let received = got != rendezvous::NO_CAP;

    // Use it. A SEND on the received capability rendezvous with whoever holds the other end, which
    // proves a capability minted for us by another process carries real authority.
    if received {
        send(got, capability_witness_contract::USED_WORD, 0, 0);
    }

    // Try to pass it on. We hold it WITHOUT grant, so the kernel refuses before any rendezvous, and
    // the invoke returns an error. LOOPBACK needs no receiver: the refusal happens at the check.
    let redelegate = send_capability(LOOPBACK, got, abi::rights::WRITE, 0);
    let refused = redelegate < 0;

    // Verdict: bit 0 we received a capability, bit 1 re-delegation was refused. 0b11 is the story.
    let code = (received as u64) | ((refused as u64) << 1);
    send(REPORT, code, 0, 0);

    exit() // one-shot: reported, so we leave and the kernel reaps us
}

user_mode_runtime::panic_handler!();
