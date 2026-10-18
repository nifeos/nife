//! **Reports one word on the one capability it was born holding** (milestone 19d (run a real
//! workload)). `report_child_builder` builds this program out of an ELF it parsed in userspace and
//! grants it a report endpoint in slot 0. Sending
//! [`capability_witness_contract::REPORT_CHILD_WORD`] there is the observable proof that the
//! userspace build produced a running thread.
//!
//! Name: ratified 2026-10-18 (calef), as the child `REPORT_CHILD_BUILDER` builds.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68 (code-quality gates)'s ratchet
// tracks (DECISIONS §107 (`missing_docs` moves to the workspace lints)): each `[[bin]]` is its own
// crate root with one `_start`, and 58 of them documenting an OS-facing ABI entry point is not what
// the lint is for.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{exit, send};

const REPORT: u64 = 0;

#[unsafe(no_mangle)]
pub extern "C" fn _start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    send(REPORT, capability_witness_contract::REPORT_CHILD_WORD, 0, 0);
    exit();
}

user_mode_runtime::panic_handler!();
