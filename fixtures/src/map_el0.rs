//! **`map_el0`: the cost of mapping a page, measured from EL0** (lmbench's `lat_mmap`, milestone
//! 961 step 1), as a command that takes a small memory budget: `map_el0 --mem 10`.
//!
//! It retypes an address space and one page frame out of its own budget, then calls `MAP_INTO` to
//! alias that frame at [`primitive_benchmark_contract::MAP_ITERATIONS`] fresh addresses after
//! [`primitive_benchmark_contract::MAP_WARMUP`] untimed ones. No data memory is consumed per
//! iteration, only the page-table entry and the revocation record the map path writes anyway,
//! which is what the row times. There is no unmap in the surface yet, so each address is used
//! once. It writes one row, `bench-production: map_el0 <ticks> <iters>`. The bench boot spawns the
//! same program with a budget of [`primitive_benchmark_contract::MAP_BUDGET_PAGES`] and the
//! quiet-boot startup word.
//!
//! # Capability contract
//!
//! | slot | what | why |
//! |---|---|---|
//! | 0 | the output sink, `WRITE` | where the row goes |
//! | 1 | a memory region | the address space, its page tables and the frame come out of it |
//!
//! # EXAMPLES
//!
//! ```text
//! $ map_el0 --mem 10
//!   bench-production: map_el0 285291 500
//! ```
//!
//! # BUGS
//!
//! - The address space is one this program built from its own budget, where the bench boot's older
//!   `map_el0` mapped into one the kernel built. The mapping path is the same, and the quiet row
//!   moved 2.1% under the baseline's, inside `script/bench --check`'s tolerance.
//! - A production-shell row competes with the console, `net_stack` and the shell for the core; it
//!   is recorded, never gated (milestone 961 (the kernel's special boots become commands typed at the production shell), decision D6).
//!
//! Name: provisional (2026-10-31 UTC, lane/961-step-1-bench-commands). The row's own name in
//! `bench/baseline-*.txt`.

#![no_std]
// Program entry points, not the crates/ library surface the missing-docs ratchet tracks: each
// `[[bin]]` is its own crate root with one `_start`, and documenting an OS-facing ABI entry point
// on every one is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use primitive_benchmark_contract::{MAP_ITERATIONS, MAP_WARMUP, Prefix, write_row};
use user_mode_runtime::{
    address_space_map_into, exit, now, retype_into_kernel_object, retype_into_page_frame, send,
};

/// Slot 0: where the row goes.
const OUTPUT: u64 = 0;
/// Slot 1: the memory region everything this program builds comes out of.
const BUDGET: u64 = 1;

const PAGE: u64 = 4096;
const WARM_BASE: u64 = address_space_map::pair_page(0x20_0000);
const TIMED_BASE: u64 = address_space_map::pair_page(0x40_0000);

user_mode_runtime::program_entry!(start);

fn start(startup_word: u64, _a1: u64, _a2: u64) -> ! {
    let space = retype_into_kernel_object(BUDGET, abi::objtype::ADDRESS_SPACE);
    let frame = retype_into_page_frame(BUDGET);
    if space < 0 || frame < 0 {
        // The budget was too small to build the target; a row of zeros says so without a panic.
        write_row(
            sink,
            Prefix::from_startup_word(startup_word),
            "map_el0",
            0,
            0,
        );
        exit();
    }
    let (space, frame) = (space as u64, frame as u64);
    for i in 0..MAP_WARMUP {
        map_one(space, frame, WARM_BASE + i * PAGE);
    }
    let start = now();
    for i in 0..MAP_ITERATIONS {
        map_one(space, frame, TIMED_BASE + i * PAGE);
    }
    let ticks = now().wrapping_sub(start);
    write_row(
        sink,
        Prefix::from_startup_word(startup_word),
        "map_el0",
        ticks,
        MAP_ITERATIONS,
    );
    exit();
}

fn sink(w0: u64, w1: u64, w2: u64) -> byte_sink_contract::Sent {
    byte_sink_contract::classify(send(OUTPUT, w0, w1, w2))
}

/// One `MAP_INTO`: map `frame` read-only into `space` at `va`. Read-only needs only `READ` on the
/// frame. Returns the syscall result: 0 on success, a negative error otherwise.
#[inline(never)]
fn map_one(space: u64, frame: u64, va: u64) -> i64 {
    address_space_map_into(space, va, frame, abi::address_space::MAP_RO)
}

user_mode_runtime::panic_handler!();
