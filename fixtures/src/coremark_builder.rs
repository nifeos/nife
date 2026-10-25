//! **Builds the CoreMark workload and lets it report** (milestone 19e (run a real workload)), the
//! first real workload rather than a toy. `kernel::user::spawn_elf_child_builder` enters this
//! program holding a building budget (slot 0) and a report endpoint (slot 1). It builds
//! `coremark` with the report endpoint as its whole authority and starts it with no arguments,
//! since the workload's iteration count is fixed. The workload sends its run's checksum, its tick
//! count and the counter frequency it read, and the kernel's tests assert on all three.
//!
//! A failed lookup, parse or build reports word 0.

//! # BUGS
//!
//! The child is loaded unmeasured (notes/trusted-init.md).
//!
//! Name: ratified 2026-10-18 (calef), as `COREMARK_BUILDER`, written here in program form.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68 (code-quality gates)'s ratchet
// tracks (DECISIONS §107 (`missing_docs` moves to the workspace lints)): each `[[bin]]` is its own
// crate root with one `_start`, and 58 of them documenting an OS-facing ABI entry point is not what
// the lint is for.
#![allow(missing_docs)]
#![no_main]

use supervision_protocol::{ChildEndowment, Retention, build_child, start_child};
use user_mode_runtime::{exit, require, send};

const MEMORY_REGION: u64 = 0;
const REPORT: u64 = 1;
const CHILD: &str = "coremark";

user_mode_runtime::program_entry!(start);

fn start(_arg0: u64, initial_ramdisk_len: u64, _arg2: u64) -> ! {
    // SAFETY: forwarded from user_mode_runtime::initial_ramdisk::bytes's own contract: the kernel
    // mapped the archive at INITIAL_RAMDISK_VA and passed its length in x1.
    let archive = unsafe { user_mode_runtime::initial_ramdisk::bytes(initial_ramdisk_len) };
    let Some(elf) = nifefs::Fs::parse(archive)
        .ok()
        .and_then(|fs| fs.read(CHILD))
        .and_then(|bytes| elf::Elf::parse(bytes).ok())
    else {
        refuse()
    };
    let grants = [(REPORT, abi::rights::WRITE)];
    let endowment = ChildEndowment {
        caps: &grants,
        stack_pages: system_initializer::CHILD_STACK_PAGES,
        // `Retention::Nothing`: this program exits a few lines after `START`, so keeping the
        // child's thread control block capability would be keeping it for nobody (DECISIONS §142
        // (what a spawner retains over a child after `START`)).
        ..ChildEndowment::new(Retention::Nothing)
    };
    let Ok(child) = build_child(MEMORY_REGION, MEMORY_REGION, &elf, &endowment) else {
        refuse()
    };
    require(start_child(child, 0, 0, 0));
    exit();
}

/// Report word 0, which is not CoreMark's checksum, and exit.
fn refuse() -> ! {
    send(REPORT, 0, 0, 0);
    exit();
}

user_mode_runtime::panic_handler!();
