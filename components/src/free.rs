//! **`free`: the machine's memory, and this prompt's share of it** (milestone 126 (the `procps` package), DECISIONS §225
//! (`free` sees the machine and your share), `crates/free`).
//!
//! Two reads and two sinks; what to print, with examples, is `crates/free`. Slot 0 is the output
//! sink and slot 8 the diagnostics sink (both `WRITE`). Slot 11 is the machine statistics page
//! (`READ`, also mapped read-only at `machine_statistics_contract::PAGE_VA`, written by the kernel
//! alone) for the `Mem:` line. Slot 12 is the job pool behind `Yours:`, asked `MemoryRegion::USAGE`
//! twice through a view narrowed to `ENUMERATE`, which answers what was spent and cannot spend,
//! split or destroy. Neither view can change anything.

//! # BUGS
//!
//! `crates/free`'s, all of them: no cache columns because the kernel keeps none, no `Swap:` line
//! because nife refuses paging out (pull request #1356), `Yours:` counts `free` itself, and
//! `Yours:` is the machine's whole job pool, shared by every prompt and session, not one prompt's.
//!
//! Name: ratified 2026-09-27 (calef, #1360's table), as milestone 126's `free` lane minted it.

#![no_std]
// Program entry points, not the crates/ library surface the ratchet of milestone 68 (code-quality
// gates: one lint policy) tracks (DECISIONS §107 (`missing_docs` moves to `workspace.lints.rust`)).
#![allow(missing_docs)]
#![no_main]

use core::sync::atomic::{AtomicBool, Ordering};

use free::{MachineRefusal, Share, ShareRefusal};
use machine_statistics_contract::{PAGE_VA, Snapshot};
use user_mode_runtime::{exit, is_granted, memory_region_usage, send};

const REPORT: u64 = 0;
const DIAG_SLOT: u64 = grant_plan::DIAGNOSTICS_SLOT;
const MACHINE_SLOT: u64 = grant_plan::MACHINE_SLOT;
const SHARE_SLOT: u64 = grant_plan::SHARE_SLOT;

static HAS_DIAG: AtomicBool = AtomicBool::new(false);

#[unsafe(no_mangle)]
pub extern "C" fn _start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    HAS_DIAG.store(is_granted(DIAG_SLOT), Ordering::Relaxed);

    let page = if is_granted(MACHINE_SLOT) {
        // SAFETY: the capability at `MACHINE_SLOT` is granted only alongside a read-only mapping of
        // the same frame at `PAGE_VA` (`system_initializer`'s spawn service does both from one
        // declaration), and that mapping lives as long as this process.
        unsafe { Snapshot::read(PAGE_VA) }.ok_or(MachineRefusal::Unrecognized)
    } else {
        Err(MachineRefusal::Withheld)
    };
    let machine = page.as_ref().map_err(|e| *e);
    let share = if !is_granted(SHARE_SLOT) {
        Err(ShareRefusal::NotHeld)
    } else {
        usage(abi::usage::SIZE).and_then(|pages| {
            usage(abi::usage::COMMITTED).map(|committed| Share { pages, committed })
        })
    };

    free::write_diagnostics(machine, share, &mut |b| write_on(diag_slot(), b));
    if HAS_DIAG.load(Ordering::Relaxed) {
        send(DIAG_SLOT, byte_sink_contract::eof(), 0, 0);
    }
    free::write_report(machine, share, &mut |b| write_on(REPORT, b));
    send(REPORT, byte_sink_contract::eof(), 0, 0);
    exit();
}

/// One `MemoryRegion::USAGE` question about the job budget.
fn usage(record: u64) -> Result<u64, ShareRefusal> {
    let r = memory_region_usage(SHARE_SLOT, record);
    if r < 0 {
        Err(ShareRefusal::Refused(r))
    } else {
        Ok(r as u64)
    }
}

fn diag_slot() -> u64 {
    if HAS_DIAG.load(Ordering::Relaxed) {
        DIAG_SLOT
    } else {
        REPORT
    }
}

/// Write bytes to an endpoint under the sink contract, sixteen at a time.
fn write_on(slot: u64, bytes: &[u8]) {
    let mut rest = bytes;
    while !rest.is_empty() {
        let (w0, w1, w2, n) = byte_sink_contract::pack(rest);
        send(slot, w0, w1, w2);
        rest = &rest[n..];
    }
}

user_mode_runtime::panic_handler!();
