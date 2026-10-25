//! **The Call/Reply client**, milestone 12.
//!
//! Holds `WRITE` on the request endpoint (slot 0) and a report endpoint (slot 1). It calls with
//! two words and reports the reply. It was never wired to the particular server that answers, and
//! it holds no capability naming that server's thread: the endpoint is the whole of the
//! introduction.
//!
//! The other half is `call_server`.
//!
//! Name: ratified 2026-09-15 (calef, this header). Ratified as `call_reply_client`; was provisional
//! `call_client` (milestone 291); `call` alone read as too generic, so it takes the `call_reply`
//! stem the round-trip benchmark already carries, `reply` naming the one-shot capability this pair
//! proves. File, `[[bin]]` and archive-tuple rename pending the batched 291 sweep. This was
//! `hello`'s `CALL_CLIENT` role, number 15.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{call, exit, send};

const ENDPOINT: u64 = 0;
const REPORT: u64 = 1;

user_mode_runtime::program_entry!(start);

fn start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    let (r0, _r1) = call(ENDPOINT, 40, 2); // expect 42 back
    send(REPORT, r0, 0, 0);
    exit()
}

user_mode_runtime::panic_handler!();
