//! A client that receives a capability in a reply (§255 (each socket is its own capability),
//! milestone 649 (every client of a network stack shares its socket numbers)), the other half of
//! `carried_capability_server`. Slot 0: the server's request endpoint (`WRITE`); slot 1: the report
//! endpoint (`WRITE`).
//!
//! It `CALL`s, and the reply must carry a capability; a word sent on it works only if it names the
//! server's probe endpoint. A second `CALL`'s plain reply must carry none. Report, one word of
//! bits: 0, the first reply carried a capability; 1, the send on it succeeded; 2, the second reply
//! carried none; 3, the carried slot was not one of the two this process was spawned with.
//!
//! Name: provisional (lane/649-sockets-are-capabilities, 2026-10-07 UTC).

#![no_std]
// A program entry point, not a library surface: its one `_start` documents itself above.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{call_receiving, exit, send};

const ENDPOINT: u64 = 0;
const REPORT: u64 = 1;
const PROBE_WORD: u64 = 0x5EC0_0C47;

#[unsafe(no_mangle)]
pub extern "C" fn _start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    let mut verdict = 0;
    let (r0, _, carried) = call_receiving(ENDPOINT, 0, 0);
    if let Some(slot) = carried
        && r0 == 1
    {
        verdict |= 0b0001;
        if slot != ENDPOINT && slot != REPORT {
            verdict |= 0b1000;
        }
        if send(slot, PROBE_WORD, 0, 0) == 0 {
            verdict |= 0b0010;
        }
    }
    let (r0, _, carried) = call_receiving(ENDPOINT, 0, 0);
    if r0 == 2 && carried.is_none() {
        verdict |= 0b0100;
    }
    send(REPORT, verdict, 0, 0);
    exit()
}

user_mode_runtime::panic_handler!();
