//! `job_sleeper`: a program that waits for a given number of milliseconds, or for ever, and then
//! says so (milestone 933 (every job is built the way a supervised job is)).
//!
//! The prompt's job-control lines need an ordinary program that is still there when the next line
//! is typed, and that can be made to end on its own, blocked, or by a fault. This one is blocked in
//! the kernel on a timer, or on a notification nobody will signal, so a `kill -9` that tears it
//! down is tearing down a thread that cannot run, and a `wait` that sees it exit sees a clean exit
//! after a known delay. It is not interruptible and keeps no frame: it holds the report sink and
//! its budget, as every `--mem` program does.
//!
//! # What this program holds
//!
//! - slot 0: the output sink (`WRITE`, the sink contract's framing).
//! - slot 1: the delegated `--mem` budget, the pages its timer and notification are retyped from.
//!
//! The argument is the wait in milliseconds. `0` waits for ever.
//!
//! # EXAMPLES
//!
//! ```text
//! $ job_sleeper 50 --mem 4
//!   the sleeper woke
//! $ job_sleeper 0 --mem 4 &
//!   [1] job_sleeper running in the background
//! $ kill -9 %1
//! ```
//!
//! Name: provisional (milestone 933's lane, 2026-10-27 UTC). It names what it does, as `sleep` does.
//! Refused `sleep` for the program table because a fixture should not take the name a package may
//! want, and `blocker` because the timed case is the common one.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{
    cntfrq, exit, now, notification_wait, retype_sleeper, send, sleep_until,
};

/// Slot 0: where the one line goes.
const REPORT: u64 = 0;
/// Slot 1: the budget the timer and notification come from.
const BUDGET: u64 = 1;

user_mode_runtime::program_entry!(start);

fn start(_a0: u64, milliseconds: u64, _a2: u64) -> ! {
    // Slots 0 and 1 are held, which is what `retype_sleeper` checks before it retypes.
    let Some((timer, notification)) = retype_sleeper(BUDGET, 2) else {
        say(b"the sleeper has no budget to sleep on\n");
        done();
    };
    if milliseconds == 0 {
        // Nothing signals it and nothing arms the timer: blocked until its region is destroyed.
        let _ = notification_wait(notification);
    } else {
        let deadline = now() + cntfrq() / 1000 * milliseconds;
        let _ = sleep_until(timer, notification, deadline);
    }
    say(b"the sleeper woke\n");
    done();
}

/// Send `bytes` in the sink contract's framing.
fn say(bytes: &[u8]) {
    let mut off = 0usize;
    while off < bytes.len() {
        let (w0, w1, w2, took) = byte_sink_contract::pack(&bytes[off..]);
        if !matches!(
            byte_sink_contract::classify(send(REPORT, w0, w1, w2)),
            byte_sink_contract::Sent::Ok
        ) {
            return;
        }
        off += took;
    }
}

/// End the stream and exit.
fn done() -> ! {
    send(REPORT, byte_sink_contract::eof(), 0, 0);
    exit();
}

user_mode_runtime::panic_handler!();
