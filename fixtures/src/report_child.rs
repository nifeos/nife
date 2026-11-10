//! **Reports one word on the one capability it was born holding** (milestone 19d (run a real
//! workload)). `report_child_builder` builds this program out of an ELF it parsed in userspace and
//! grants it a report endpoint in slot 0. Sending
//! [`capability_witness_contract::REPORT_CHILD_WORD`] there is the observable proof that the
//! userspace build produced a running thread. The two words after it are the address and length of
//! the window it finds on its startup page ([`capability_witness_contract::REPORT_CHILD_WINDOW`]),
//! or zero for both where the spawner listed none.
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

user_mode_runtime::program_entry!(start);

fn start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    // SAFETY: both spawners of this program write a startup page and map it read-only at its fixed
    // address: the kernel's loader, and `process_builder` for `report_child_builder`.
    let window = unsafe { startup_page_contract::StartupPage::mapped() }
        .and_then(|page| page.window(capability_witness_contract::REPORT_CHILD_WINDOW.kind));
    let (address, pages) = window.map_or((0, 0), |w| (w.virtual_address, u64::from(w.pages)));
    send(
        REPORT,
        capability_witness_contract::REPORT_CHILD_WORD,
        address,
        pages,
    );
    exit();
}

user_mode_runtime::panic_handler!();
