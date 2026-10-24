//! The CoreMark workload, at EL0 (milestone 19e: "run a real workload").
//!
//! It runs the CoreMark-derived kernel in `crates/coremark` (a linked-list sort, a small-matrix
//! multiply and a state machine) for a fixed iteration count, so the run's CRC is a fixed value
//! (`coremark::PINNED_CRC_64`) that init and the kernel test check. It times itself with
//! `user_mode_runtime::now` and SENDs `[crc, ticks, freq]` on slot 0, its one capability, so the
//! receiver computes iterations per second without a second syscall. Under HVF the ticks are real
//! nanoseconds; under TCG they are icount fiction, and only the CRC means anything. The compute is
//! a portable crate, so the identical code runs on macOS and Linux too (notes/benchmarks.md).
//!
//! Name: ratified 2026-08-01 (calef, milestone 63) as a proper noun, EEMBC's industry benchmark. It
//! shares its crate's name for the reason DECISIONS §63 gives: the crate is the logic, the program
//! is the authority.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{cntfrq, exit, now, send};

/// The endpoint init grants us (slot 0): we SEND `[crc, ticks, freq]` here, then exit.
const RESULT: u64 = 0;

#[unsafe(no_mangle)]
pub extern "C" fn _start(_x0: u64, _x1: u64, _x2: u64) -> ! {
    let start = now();
    let crc = coremark::run(coremark::PINNED_ITERS);
    let ticks = now().wrapping_sub(start);
    send(RESULT, crc as u64, ticks, cntfrq());
    exit();
}

user_mode_runtime::panic_handler!();
