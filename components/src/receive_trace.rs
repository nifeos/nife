//! **The network bench build's receive trace**: `network_stack::receive_trace`'s distributions,
//! fed from the ring, the transmit token, the poll loop and the polled wait, and sent to the
//! kernel when a TCP socket closes. Only a build with `NIFE_NETWORK_BENCH_PEER` set traces, which
//! is the image `cargo xtask network-bench` builds; in every other build each hook is an empty
//! function the compiler removes.
//!
//! Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.

//! # BUGS
//! - A tracing server blocks on its report endpoint at the first TCP close until the kernel reads
//!   it, so the trace build is for the bench boot only, whose kernel reads it after the drain.

use driver_process_contract::network_bench_report::Report;
use network_stack::receive_trace::{Receiver, Text, tcp_segment};

use super::*;

/// Whether this build traces: the bench image's, and no other.
pub(crate) const ON: bool = option_env!("NIFE_NETWORK_BENCH_PEER").is_some();

/// The server's slot 0, its report endpoint to the kernel.
const REPORT: u64 = 0;

/// The trace, in `.bss`.
static mut TRACE: Receiver = Receiver::new();

/// Whether the connection's first data segment has arrived. The trace starts there, since the
/// wait for a peer to begin sending (15 s under QEMU's slirp) is not part of the stream.
static mut STREAMING: bool = false;

/// The one reference to [`TRACE`].
fn trace() -> &'static mut Receiver {
    // SAFETY: the server has one thread, and each caller uses the reference for one call and
    // drops it before the next.
    unsafe { &mut *core::ptr::addr_of_mut!(TRACE) }
}

/// The counter, in microseconds.
pub(crate) fn microseconds() -> u64 {
    if !ON {
        return 0;
    }
    (u128::from(counter_ticks()) * 1_000_000 / u128::from(counter_hertz())) as u64
}

/// A TCP socket is connecting: the trace waits for its first data segment.
pub(crate) fn connecting() {
    if ON {
        // SAFETY: the server has one thread.
        unsafe { STREAMING = false };
    }
}

pub(crate) fn received(frame: &[u8]) {
    if !ON {
        return;
    }
    // SAFETY: the server has one thread.
    let streaming = unsafe { &mut *core::ptr::addr_of_mut!(STREAMING) };
    if !*streaming {
        if !tcp_segment(frame).is_some_and(|s| s.payload_bytes > 0) {
            return;
        }
        *streaming = true;
        *trace() = Receiver::new();
    }
    trace().received(microseconds(), frame);
}

pub(crate) fn transmitted(frame: &[u8]) {
    if ON {
        trace().transmitted(microseconds(), frame);
    }
}

pub(crate) fn polled(started: u64) {
    if ON {
        trace().polled(started, microseconds());
    }
}

pub(crate) fn yielded(started: u64) {
    if ON {
        trace().waited(true, microseconds().saturating_sub(started));
    }
}

pub(crate) fn slept(started: u64) {
    if ON {
        trace().waited(false, microseconds().saturating_sub(started));
    }
}

pub(crate) fn requested() -> u64 {
    let now = microseconds();
    if ON {
        trace().requested(now);
    }
    now
}

pub(crate) fn replied(asked: u64) {
    if ON {
        trace().replied(asked, microseconds());
    }
}

/// A TCP socket closed: send the summary to the kernel, one text report at a time. The send blocks
/// until the kernel takes each report.
pub(crate) fn closed() {
    if !ON {
        return;
    }
    let mut text = Text::<4096>::new();
    let _ = trace().write_summary(&mut text);
    for r in Report::text(text.as_bytes()) {
        let [a, b, c] = r.pack();
        send(REPORT, a, b, c);
    }
    let [a, b, c] = Report::End {
        server_follows: false,
    }
    .pack();
    send(REPORT, a, b, c);
}
