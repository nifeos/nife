//! **A program that does not know what it is writing to** (milestone 50, split out of the `sink`
//! binary by milestone 292; notes/sink-protocol.md).
//!
//! It writes [`byte_sink_contract::fixture::TRANSCRIPT`] to whatever is in slot 0 and reports how
//! the last `SEND` classified, by value: "gone" and "never had one" must be two numbers to test
//! milestone 50's one behaviour change. **The negative is the point**: it holds one writable
//! endpoint and a report endpoint, nothing that names a file, a directory, a terminal or the FS
//! server. `kernel::user::sink_tests` puts three things in slot 0 (an endpoint the kernel drains,
//! `file_sink` over a real RedoxFS file, `terminal_sink_caretaker` before a real terminal) and the
//! same bytes come out of all three.

//! # The wiring
//!
//! | slot 0 | slot 1 | `a0` |
//! |---|---|---|
//! | the byte sink, `WRITE` (or **empty**) | the report, `WRITE` | how many times to write the transcript |
//!
//! `a0 = 0` means "until the sink stops taking it", for the destroyed-sink test: like a producer in
//! `yes | head`, it cannot know when its reader will die. An empty slot 0 is a case ("you were
//! never given one"): it runs on and classifies [`byte_sink_contract::Sent::NoSink`], as a process
//! whose stdout is closed runs to completion anywhere.

//! # EXAMPLES
//!
//! The one the tests assert, in the kernel's vocabulary (`system_tests/src/user/sink_tests.rs`):
//!
//! ```text
//! let report = spawn_writer(image, Some(endpoint), 1);
//! let [class, total, ..] = ipc_receive(report);
//! assert_eq!(class, fixture::code(Sent::Ok));
//! assert_eq!(total as usize, fixture::TRANSCRIPT.len());
//! ```

//! # BUGS
//!
//! The report is a single `send` of three words, so a caller that never drains it parks this
//! process forever at `exit`'s doorstep: drain the report or do not spawn it.
//!
//! Name: recorded (design/roadmap/0292-the-sink-contract-ends-are-three-programs.md and
//! notes/sink-protocol.md). **Provisional: calef has not ratified it.** Named for the contract it
//! speaks (`byte_sink_contract`, which design/naming/vocabulary-rulings.md keeps) and the pinned
//! transcript it writes. Refused `writer` (generic). Refused `indifferent_writer` (names the
//! property being proved, not the program). Refused `sink_writer` (reads as the thing that writes
//! a sink, and says nothing about what it writes).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use byte_sink_contract::fixture;
use user_mode_runtime::{exit, send};

/// The byte sink, `WRITE`, and the whole of what this program holds towards its output: no page,
/// no acknowledgement channel, nothing else. Empty is a legal state and a tested one.
const SINK: u64 = 0;
/// Where the classification and the byte total go.
const REPORT: u64 = 1;

/// Write the transcript `repeat` times, or forever if `repeat` is 0, and report how the last
/// `SEND` classified plus how many bytes got through.
///
/// A clean finish announces end of stream; a failed one does not, because there is nothing left to
/// announce it to.
#[unsafe(no_mangle)]
pub extern "C" fn _start(repeat: u64, _a1: u64, _a2: u64) -> ! {
    let mut total = 0u64;
    let mut rounds = 0u64;
    let mut last = byte_sink_contract::Sent::Ok;

    'outer: loop {
        let mut off = 0usize;
        while off < fixture::TRANSCRIPT.len() {
            let (w0, w1, w2, n) = byte_sink_contract::pack(&fixture::TRANSCRIPT[off..]);
            last = byte_sink_contract::classify(send(SINK, w0, w1, w2));
            if !matches!(last, byte_sink_contract::Sent::Ok) {
                break 'outer;
            }
            off += n;
            total += n as u64;
        }
        rounds += 1;
        if repeat != 0 && rounds >= repeat {
            break;
        }
    }

    if matches!(last, byte_sink_contract::Sent::Ok) {
        let _ = send(SINK, byte_sink_contract::eof(), 0, 0);
    }
    send(REPORT, fixture::code(last), total, rounds);
    exit();
}

user_mode_runtime::panic_handler!();
