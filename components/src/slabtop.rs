//! **`slabtop`: where this prompt's job budget went, by kind of kernel object** (milestone 126 (the `procps` package),
//! DECISIONS §225 (`free` sees the machine and your share), `crates/slabtop`).
//!
//! Six `MemoryRegion::USAGE` questions about one budget, and a table. Why this is what `slabtop`
//! became on a kernel with no slab is `crates/slabtop`'s module docs.

//! # Capability contract
//!
//! | slot | what | why |
//! |---|---|---|
//! | 0 | the output sink, `WRITE` | where the table goes |
//! | 8 | the diagnostics sink, `WRITE` | where a refusal is said |
//! | 12 | this prompt's job budget, `ENUMERATE` | every figure; it cannot spend, split or destroy |
//!
//! No machine page, which is the difference from `free`: this program describes a budget, not a
//! machine.

//! # EXAMPLES
//! ```text
//! $ slabtop
//! job budget: 80 of 512 pages spent
//!    PAGES      KIB  SPENT ON
//!       60      240  frames
//!        4       16  threads
//!        4       16  address spaces
//!        2        8  rendezvous
//! ```

//! # BUGS
//!
//! `crates/slabtop`'s: the counts say where pages went rather than what is alive, and the table
//! counts `slabtop` itself.
//!
//! Name: ratified 2026-09-27 (calef, #1360's table), as milestone 126's `free` lane minted it.

#![no_std]
// Program entry points, not the crates/ library surface the ratchet of milestone 68 (code-quality
// gates: one lint policy) tracks (DECISIONS §107 (`missing_docs` moves to `workspace.lints.rust`)).
#![allow(missing_docs)]
#![no_main]

use slabtop::Spending;
use user_mode_runtime::{exit, is_granted, memory_region_usage, send};

const REPORT: u64 = 0;
const DIAG_SLOT: u64 = grant_plan::DIAGNOSTICS_SLOT;
const SHARE_SLOT: u64 = grant_plan::SHARE_SLOT;

user_mode_runtime::program_entry!(start);

fn start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    let has_diag = is_granted(DIAG_SLOT);
    let diag = if has_diag { DIAG_SLOT } else { REPORT };
    let found = if is_granted(SHARE_SLOT) {
        read().map_err(Some)
    } else {
        Err(None)
    };
    match found {
        Ok(_) => {}
        Err(None) => write_on(
            diag,
            b"slabtop: this process holds no view of a job budget\n",
        ),
        Err(Some(_)) => write_on(
            diag,
            b"slabtop: the kernel refused to say what the job budget spent\n",
        ),
    }
    if has_diag {
        send(DIAG_SLOT, byte_sink_contract::eof(), 0, 0);
    }
    if let Ok(s) = found {
        slabtop::write_report(&s, &mut |b| write_on(REPORT, b));
    }
    send(REPORT, byte_sink_contract::eof(), 0, 0);
    exit();
}

/// Every figure the table needs, or the first refusal.
fn read() -> Result<Spending, i64> {
    Ok(Spending {
        pages: usage(abi::usage::SIZE)?,
        committed: usage(abi::usage::COMMITTED)?,
        frames: usage(abi::usage::FRAMES)?,
        threads: usage(abi::usage::THREADS)?,
        address_spaces: usage(abi::usage::ADDRESS_SPACES)?,
        rendezvous: usage(abi::usage::RENDEZVOUS)?,
    })
}

fn usage(record: u64) -> Result<u64, i64> {
    let r = memory_region_usage(SHARE_SLOT, record);
    if r < 0 { Err(r) } else { Ok(r as u64) }
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
