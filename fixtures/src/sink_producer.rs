//! **`sink_producer`: the writing half of `sink_producer | sink_consumer`**, the pipe throughput
//! benchmark as a command (milestone 961 (the kernel's special boots become commands typed at the production shell) step 1).
//!
//! It packs sixteen bytes into the sink contract's three words and `SEND`s them,
//! [`primitive_benchmark_contract::SINK_MESSAGES`] plus the warmup count of times, then ends the
//! stream. No buffer, no flush, no knowledge of who is reading: behind its output is the
//! `sink_consumer` that times the crossing, and the pipe between them is an endpoint and nothing
//! else, so every sixteen bytes is a rendezvous (notes/pipes.md). The warmup messages are the
//! contract with the consumer, which discards that many before it starts timing.
//!
//! # Capability contract
//!
//! | slot | what | why |
//! |---|---|---|
//! | 0 | the output sink, `WRITE` | the pipe into the consumer |
//!
//! # EXAMPLES
//!
//! ```text
//! $ sink_producer | sink_consumer
//!   bench-production: sink_throughput 2097152 80000
//! ```
//!
//! # BUGS
//!
//! - Typed alone, it writes 80 000 bytes of `x` to the console. Its manifest cannot say "only into
//!   a pipe": the output declaration has no such kind.
//!
//! Name: provisional (milestone 961 step 1, 2026-10-31 UTC). The two halves of a pipeline name the
//! contract they speak (`sink_`) and their side of it.

#![no_std]
// Program entry points, not the crates/ library surface the missing-docs ratchet tracks: each
// `[[bin]]` is its own crate root with one `_start`, and documenting an OS-facing ABI entry point
// on every one is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use primitive_benchmark_contract::{SINK_MESSAGES, SINK_WARMUP_MESSAGES};
use user_mode_runtime::{exit, send};

/// Slot 0: the pipe into the consumer.
const OUTPUT: u64 = 0;

user_mode_runtime::program_entry!(start);

fn start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    let payload = [b'x'; byte_sink_contract::INLINE_MAXIMUM];
    for _ in 0..(SINK_MESSAGES + SINK_WARMUP_MESSAGES) {
        let (w0, w1, w2, _) = byte_sink_contract::pack(&payload);
        send(OUTPUT, w0, w1, w2);
    }
    send(OUTPUT, byte_sink_contract::eof(), 0, 0);
    exit();
}

user_mode_runtime::panic_handler!();
