//! A thread reading its own CPU from user mode, with no syscall at all.
//!
//! calef ruled on 2026-09-21 that a thread observing itself reads a page rather than making a
//! crossing, because the consumer is a memory allocator asking on every allocation. This proves the
//! chain from the side that matters: `crates/current_cpu_contract` lays the page out, the kernel
//! maps it read-only and writes it at every switch-in, and `user_mode_runtime::current_cpu` reads
//! it with an ordinary load. It reads twice across a `yield_now`, as `cycle_counter_reader` does:
//! the first read proves a thread learns its core before its first instruction, and the second that
//! the switch keeps writing. A yield does not guarantee a switch, so the second read is evidence;
//! the kernel-side test's direct read of a space's page is the proof.

//! # Report
//!
//! Both reads, plus [`current_cpu_contract::CPU_ID_BOUND`], the one number in this contract two
//! binaries must agree on: the kernel's `cpu::MAX_CPUS` is that constant, and carrying it back
//! across the boundary would catch them drifting. A CPU it could not learn is reported as
//! `u64::MAX`, the protocol's `UNSCHEDULED`, never a small number: zero is a real core, and a wrong
//! number is worse than no number.

//! # Bugs
//!
//! **Nothing here asserts the two reads agree or differ**, and neither would be right. A thread
//! that stayed on one core reports the same id twice and a thread the scheduler moved reports two,
//! and both are correct behaviour. The kernel-side test asserts what is actually invariant: that
//! each id names a core that is online.
//!
//! Name: provisional (this lane, 2026-09-21). Formed on `cycle_counter_reader`, the fixture it is
//! shaped after: the thing read, then `_reader`. calef names the programs.

#![no_std]
// Program entry points, not the crates/ library surface tracked by the ratchet of
// milestone 68 (code-quality gates: one lint policy): each `[[bin]]` is its own crate root with
// one `_start`, and §107 (`missing_docs` moves to `workspace.lints.rust`) is what would otherwise
// ask each for a doc comment.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{current_cpu, exit, send, yield_now};

const REPORT: u64 = 0;

/// `None` on the wire. The protocol's own sentinel rather than a second one.
const UNKNOWN: u64 = current_cpu_contract::UNSCHEDULED;

user_mode_runtime::program_entry!(start);

fn start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    let first = current_cpu().map_or(UNKNOWN, |c| c as u64);
    yield_now();
    let second = current_cpu().map_or(UNKNOWN, |c| c as u64);
    send(
        REPORT,
        first,
        second,
        current_cpu_contract::CPU_ID_BOUND as u64,
    );
    exit()
}

user_mode_runtime::panic_handler!();
