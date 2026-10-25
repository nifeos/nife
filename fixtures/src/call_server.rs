//! The Call/Reply server (milestone 12 (Call/Reply IPC: a one-shot reply capability)): holds
//! `RECEIVE` on a request endpoint (slot 0) and a report endpoint (slot 1), answers one caller it
//! was never individually wired to, then tries the reply capability a second time and reports that
//! the kernel refused.
//!
//! The kernel mints the reply capability at the call, naming the caller, and consumes it on first
//! use, so nothing the server holds outlives the one answer it owed; a pre-wired reply rendezvous
//! cannot give that. The other half is `call_client`; the wiring is
//! `system_tests/src/user/call_service.rs`.
//!
//! Name: ratified 2026-09-15 (calef, this header). Ratified as `call_reply_server`; was provisional
//! `call_server` (milestone 291); qualified to the `call_reply` stem (the round-trip benchmark's
//! name), `reply` naming the one-shot capability this pair proves. File, `[[bin]]` and archive-tuple
//! rename pending the batched 291 sweep. This was `hello`'s `CALL_SERVER` role, number 14, and
//! matches the kernel-side module that wires it. "Call" here is the IPC operation's own name
//! (`user_mode_runtime::call`, `abi`'s `CALL`), which is the "standard term a reader already knows" case
//! AGENTS.md's naming section protects rather than a generic verb.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{exit, invoke, receive_request, reply, require, send};

const ENDPOINT: u64 = 0;
const REPORT: u64 = 1;

user_mode_runtime::program_entry!(start);

fn start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    let req = receive_request(ENDPOINT);
    let (w0, w1) = (req.w0, req.w1);
    // The kernel tagged this delivery as a CALL's Reply (milestone 706 (a `CALL` server can tell a
    // Reply from a delegation)), or this is no CALL.
    let to = req.delivered.into_reply();
    require(to.is_some());
    let Some(to) = to else { exit() };
    let slot = to.slot();
    // Answer the caller: w0 + w1. This consumes the one-shot reply capability.
    require(reply(to, w0 + w1, 0) == 0);
    // A second reply on the same slot must fail: the cap was consumed on first use. Raw, because
    // the typed `reply` already consumed its `Reply`, which is the point being tested one level
    // down: the kernel refuses it too.
    // SAFETY: `svc`/`ecall`; the kernel validates the slot, which is now empty.
    let second = unsafe { invoke(slot, abi::reply::REPLY, 0xBAD, 0, 0) };
    send(REPORT, if second < 0 { 1 } else { 0 }, 0, 0); // 1 = refused (one-shot held), 0 = a hole
    exit()
}

user_mode_runtime::panic_handler!();
