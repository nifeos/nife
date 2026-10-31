//! **`sink_consumer`: the reading half of `sink_producer | sink_consumer`, self-timed**
//! (milestone 961 (the kernel's special boots become commands typed at the production shell) step 1).
//!
//! It discards [`primitive_benchmark_contract::SINK_WARMUP_MESSAGES`] messages, times the next
//! [`primitive_benchmark_contract::SINK_MESSAGES`] with `user_mode_runtime::now`, drains the stream
//! to its end, and writes one row,
//! `bench-production: sink_throughput <ticks> <bytes>`, to its output. It reports **bytes**, not
//! iterations, because that is the number a Unix pipe can be compared against:
//! `bench/host/pipe_throughput.rs` moves the same bytes through a real `pipe(2)`. There is no
//! buffer, so the producer can never run ahead (notes/pipes.md). The bench boot spawns the same
//! program with startup word [`primitive_benchmark_contract::QUIET_BOOT_STARTUP_WORD`] and reads
//! the row as `bench: ...`.
//!
//! # Capability contract
//!
//! | slot | what | why |
//! |---|---|---|
//! | 0 | the output sink, `WRITE` | where the row goes |
//! | 1 | the input source, `READ` | the pipe from the producer |
//!
//! # EXAMPLES
//!
//! ```text
//! $ sink_producer | sink_consumer
//!   bench-production: sink_throughput 2097152 80000
//! $ sink_consumer
//!   sink_consumer: reads an input stream: name a file, redirect with '<', or pipe into it
//! ```
//!
//! # BUGS
//!
//! - Fed anything but `sink_producer`, it times whatever arrives, and the row means nothing. A
//!   stream shorter than the warmup and the timed count ends the timing early and reports the
//!   bytes it saw.
//! - A production-shell row competes with the console, `net_stack` and the shell for the core; it
//!   is recorded, never gated (milestone 961, decision D6).
//!
//! Name: provisional (milestone 961 step 1, 2026-10-31 UTC), the partner of `sink_producer`.

#![no_std]
// Program entry points, not the crates/ library surface the missing-docs ratchet tracks: each
// `[[bin]]` is its own crate root with one `_start`, and documenting an OS-facing ABI entry point
// on every one is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use primitive_benchmark_contract::{Prefix, SINK_MESSAGES, SINK_WARMUP_MESSAGES, write_row};
use user_mode_runtime::{exit, now, receive, send};

/// Slot 0: where the row goes.
const OUTPUT: u64 = 0;
/// Slot 1: the pipe from the producer.
const INPUT: u64 = 1;

user_mode_runtime::program_entry!(start);

fn start(startup_word: u64, _a1: u64, _a2: u64) -> ! {
    let mut bytes = 0u64;
    let mut timed_start = 0u64;
    let mut ticks = 0u64;
    let mut seen = 0u64;
    loop {
        let (w0, w1, w2) = receive(INPUT);
        let mut buf = [0u8; byte_sink_contract::INLINE_MAXIMUM];
        let n = match byte_sink_contract::unpack(w0, w1, w2, &mut buf) {
            byte_sink_contract::Msg::Bytes(n) => n,
            // End of stream, or a message this framing does not know: stop reading either way.
            byte_sink_contract::Msg::Eof | byte_sink_contract::Msg::Malformed => break,
        };
        seen += 1;
        if seen == SINK_WARMUP_MESSAGES {
            timed_start = now();
        } else if seen > SINK_WARMUP_MESSAGES {
            bytes += n as u64;
            if seen == SINK_WARMUP_MESSAGES + SINK_MESSAGES {
                ticks = now().wrapping_sub(timed_start);
            }
        }
    }
    write_row(
        |w0, w1, w2| byte_sink_contract::classify(send(OUTPUT, w0, w1, w2)),
        Prefix::from_startup_word(startup_word),
        "sink_throughput",
        ticks,
        bytes,
    );
    exit();
}

user_mode_runtime::panic_handler!();
