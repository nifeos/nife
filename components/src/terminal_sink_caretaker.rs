//! **The terminal, as a sink** (milestone 50 (pipes and redirection: one sink protocol, and `|`
//! turns out to be an endpoint)'s last remainder; DECISIONS §67). A caretaker: it holds the
//! terminal endpoint and hands out a sink, speaking the sink contract to its clients and
//! `OPERATION_PRINT` `CALL`s to `line_editor`, so a program whose output slot holds this process
//! writes to the screen and cannot tell. It keeps the endpoint back because it also carries
//! `OPERATION_READLINE`, and a sink capability that can read the keyboard is not a sink capability.
//! `OPERATION_PRINT` carries the bytes in the request's own words, so it maps no page. See
//! notes/sink-protocol.md, "The terminal's sink adapter", for why it is a process and why it needed
//! that opcode.

//! # BUGS
//!
//! - Eight bytes per call (the terminal contract's request shape), so a sixteen-byte sink
//!   message is two round trips.
//! - No back pressure of its own: flow control is the chain of rendezvous, with nothing buffered.
//! - `OPERATION_EOF` ends a writer, not the terminal; nothing notices a client dead mid-message.
//!
//! Name: ratified 2026-08-03 (calef), replacing `terminal_sink`. It holds the terminal endpoint,
//! which also carries `OPERATION_READLINE`, and hands out a sink that cannot read, which is the caretaker
//! shape exactly; naming it one also proves that shape generalizes beyond `fs_`, where every other
//! member lives, so a reader stops taking caretaker for a filesystem concept. The full form keeps
//! both facts a shorter name would have forced a choice between: the client learns it gets a sink,
//! the auditor learns what it holds.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use line_editor::proto;
use user_mode_runtime::{call, receive};

/// Slot 0: the sink endpoint, held `READ`. Its clients hold `WRITE` on the same object and nothing
/// else, which is the whole point: what they can reach is this process, and this process only ever
/// prints.
const SINK: u64 = 0;

/// Slot 1: the terminal, held `WRITE` (which on an endpoint is the right to `CALL`). **Never
/// delegated onward**, and that is what a caretaker is for.
const TERM: u64 = 1;

#[unsafe(no_mangle)]
pub extern "C" fn _start(_x0: u64, _x1: u64, _x2: u64) -> ! {
    loop {
        let (w0, w1, w2) = receive(SINK);
        let mut buf = [0u8; byte_sink_contract::INLINE_MAX];
        match byte_sink_contract::unpack(w0, w1, w2, &mut buf) {
            byte_sink_contract::Msg::Bytes(n) => print(&buf[..n]),
            // One endpoint serves every client in turn, so a writer's end is not the terminal's.
            byte_sink_contract::Msg::Eof => {}
            // Said aloud, or the client believes it printed.
            byte_sink_contract::Msg::Malformed => {
                print(b"terminal_sink_caretaker: unreadable message\n");
            }
        }
    }
}

/// Print through the terminal, eight bytes a call. The reply means the bytes are on the wire.
fn print(bytes: &[u8]) {
    for chunk in bytes.chunks(8) {
        let mut w1 = 0u64;
        for (i, &b) in chunk.iter().enumerate() {
            w1 |= (b as u64) << (8 * i);
        }
        call(
            TERM,
            proto::req(proto::OPERATION_PRINT, chunk.len() as u64),
            w1,
        );
    }
}

user_mode_runtime::panic_handler!();
