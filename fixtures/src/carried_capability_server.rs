//! A server that answers a `CALL` with a capability (§255 (each socket is its own capability),
//! milestone 649 (every client of a network stack shares its socket numbers)): the kernel half of
//! what the network stack does on every socket open, proved without a network.
//!
//! The first `CALL` is answered three times, in order: `REPLY_CAPABILITY` carrying slot 4 must be
//! refused (no `GRANT`) and leave the Reply usable, one carrying an empty slot must be refused the
//! same way, and one carrying slot 3 answers. The client's word on it arrives on slot 2. The second
//! `CALL` gets a plain `REPLY`, which must deliver no capability. Report bits: 0, the no-`GRANT`
//! copy was refused; 1, the empty slot was; 2, the carrying reply succeeded; 3, the word arrived.

//! # Capability contract
//!
//! - slot 0: the request endpoint (`READ`), where the client `CALL`s
//! - slot 1: the report endpoint (`WRITE`)
//! - slot 2: the probe endpoint (`READ`): what the carried capability names
//! - slot 3: the probe endpoint again, `WRITE | GRANT`: the copy it carries
//! - slot 4: the probe endpoint again, `WRITE` only: a capability it may use and not pass on
//!
//! The other half is `carried_capability_client`; the wiring is
//! `system_tests/src/user/carried_capability_service.rs`.

//! Name: provisional (lane/649-sockets-are-capabilities, 2026-10-07 UTC).

#![no_std]
// A program entry point, not a library surface: its one `_start` documents itself above.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{exit, receive, receive_request, reply, reply_capability, send};

const ENDPOINT: u64 = 0;
const REPORT: u64 = 1;
const PROBE: u64 = 2;
const PROBE_GRANT: u64 = 3;
const PROBE_NO_GRANT: u64 = 4;
const EMPTY: u64 = 40;

/// The word the client sends on the carried capability.
const PROBE_WORD: u64 = 0x5EC0_0C47;

user_mode_runtime::program_entry!(start);

fn start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    let mut verdict = 0;
    let Some(to) = receive_request(ENDPOINT).delivered.into_reply() else {
        user_mode_runtime::trap()
    };
    let to = match reply_capability(to, 1, 0, PROBE_NO_GRANT) {
        Err((to, _)) => {
            verdict |= 0b0001;
            to
        }
        Ok(_) => finish(verdict),
    };
    let to = match reply_capability(to, 1, 0, EMPTY) {
        Err((to, _)) => {
            verdict |= 0b0010;
            to
        }
        Ok(_) => finish(verdict),
    };
    if reply_capability(to, 1, 0, PROBE_GRANT) == Ok(true) {
        verdict |= 0b0100;
    }
    let (word, ..) = receive(PROBE);
    if word == PROBE_WORD {
        verdict |= 0b1000;
    }
    // The second CALL, answered with a plain REPLY: the client checks it carried nothing.
    if let Some(to) = receive_request(ENDPOINT).delivered.into_reply() {
        reply(to, 2, 0);
    }
    finish(verdict)
}

fn finish(verdict: u64) -> ! {
    send(REPORT, verdict, 0, 0);
    exit()
}

user_mode_runtime::panic_handler!();
