//! **Spend an untyped budget until it is gone**, milestone 11.
//!
//! This process holds a chunk of raw memory (slot 0) and a report endpoint (slot 1). It maps page
//! after page out of that memory region into its own address space, writes and reads each one to
//! prove it is real, and reports how many it mapped once the region is exhausted.
//!
//! **The point is what the kernel does meanwhile: nothing.** Every page comes out of the region, so
//! the kernel's free-frame count does not move; a process cannot make the kernel allocate, so it
//! cannot exhaust it. `kernel::user::tests` checks exactly that, reading the frame count on either
//! side of the run.

//! # Bugs
//!
//! **It must not exit**, unlike every other one-shot fixture here, and that is a foot gun rather
//! than a design. The test reads the kernel's used-frame count the instant the report lands, and
//! exiting would tear this address space down inside that same window, so the number read would be
//! the teardown's rather than the measurement's. Spinning holds the state still until the
//! assertion has looked at it. The cost is that nothing reaps this thread on its own:
//! `memory_region_service::start` returns the `ThreadId` for that reason, and a caller that drops
//! the name has leaked a spinning thread onto every test that runs after it.
//!
//! Name: provisional (milestone 291). This was `hello`'s `UNTYPED_DEMO` role, number 7, whose
//! function was already called `memory_region_demo` after the untyped-to-memory-region rename.
//! Matches `memory_grant_depleter`, the milestone-31 program that does the same thing to a grant
//! rather than to a region, which is the analogous case this tree already named. Refused
//! `memory_region_demo`: "demo" says why it was written rather than what it does, and every
//! program in this directory is a demonstration of something.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use abi::Error;
use user_mode_runtime::{map_memory_region_page, send};

const MEMORY_REGION: u64 = 0;
const REPORT: u64 = 1;
const BASE_VA: u64 = address_space_map::pair_page(0x0000_0000_00c0_0000);

#[unsafe(no_mangle)]
pub extern "C" fn _start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    // Signal that we are loaded and about to start spending the region. The test measures the
    // kernel's frame count HERE, so it sees only what we do from now on.
    send(REPORT, 0, 0, 0);

    let mut mapped: u64 = 0;
    loop {
        let va = BASE_VA + mapped * 4096;
        // Retype a page out of our region and map it here.
        let r = map_memory_region_page(MEMORY_REGION, va);
        if let Some(e) = Error::from_ret(r) {
            // OutOfMemory means our budget is spent. Any other error is a real bug.
            if e != Error::OutOfMemory {
                user_mode_runtime::trap();
            }
            break;
        }

        // Prove the page is genuinely ours: write a marker, read it back.
        let marker = 0xA11C_0000_0000_0000u64 | mapped;
        // SAFETY: the kernel just mapped this page writable in our address space.
        unsafe {
            core::ptr::write_volatile(va as *mut u64, marker);
            if core::ptr::read_volatile(va as *const u64) != marker {
                user_mode_runtime::trap();
            }
        }

        mapped += 1;
        if mapped > 100_000 {
            user_mode_runtime::trap(); // a bump allocator that never exhausts is a bug
        }
    }

    send(REPORT, mapped, 0, 0);
    // See this module's BUGS: exiting here would destroy the measurement.
    loop {
        core::hint::spin_loop();
    }
}

user_mode_runtime::panic_handler!();
