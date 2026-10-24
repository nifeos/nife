//! **Waits for an interrupt it was delegated, then reports** (step 19d.2b of milestone 19d (run a
//! real workload)). `interrupt_child_builder` grants this program a report endpoint (slot 0) and an
//! interrupt capability (slot 1). It blocks in the interrupt's `WAIT` and sends
//! [`capability_witness_contract::INTERRUPT_CHILD_WORD`] once the interrupt arrives as a message.
//! It sends nothing before that, so the kernel's test receiving the word is the interrupt delivered
//! through the delegated capability, and a hang is the negative case.
//!
//! Name: ratified 2026-10-18 (calef), as the child `INTERRUPT_CHILD_BUILDER` builds.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68 (code-quality gates)'s ratchet
// tracks (DECISIONS §107 (`missing_docs` moves to the workspace lints)): each `[[bin]]` is its own
// crate root with one `_start`, and 58 of them documenting an OS-facing ABI entry point is not what
// the lint is for.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{exit, interrupt_wait, send};

const REPORT: u64 = 0;
const INTERRUPT: u64 = 1;

user_mode_runtime::program_entry!(start);

fn start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    let _ = interrupt_wait(INTERRUPT);
    send(
        REPORT,
        capability_witness_contract::INTERRUPT_CHILD_WORD,
        0,
        0,
    );
    exit();
}

user_mode_runtime::panic_handler!();
