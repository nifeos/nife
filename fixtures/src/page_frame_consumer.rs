//! **The frame demo, the half that receives**, milestone 19.
//!
//! Receives the delegated frame, maps the same physical page read-only, reads the producer's
//! sentinel back (proof the memory is shared), and confirms it cannot map the page writable,
//! because it was handed the frame with `READ` alone.
//!
//! The verdict it reports is two bits: bit 0 the sentinel was read, bit 1 a writable mapping was
//! refused. `kernel::user::tests` asserts both, separately, so a failure says which half broke. The
//! other half is `page_frame_producer`.
//!

//! Name: provisional (milestone 291). This was `hello`'s `PAGE_FRAME_CONSUMER` role, number 12.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use abi::rendezvous;
use user_mode_runtime::{exit, map_page_frame, receive_cap, send};

const CHANNEL: u64 = 0; // RECEIVE_CAP the frame here
const MEMORY_REGION: u64 = 1; // page tables for our own mappings come from here
const REPORT: u64 = 2; // report the verdict here
const PAGE_FRAME_VA: u64 = address_space_map::pair_page(0x0000_0000_00A0_0000);
const RW_VA: u64 = address_space_map::pair_page(0x0000_0000_00B0_0000);

user_mode_runtime::program_entry!(start);

fn start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    let (_data, frame, _) = receive_cap(CHANNEL);
    let received = frame != rendezvous::NO_CAP;

    let mut read_ok = false;
    let mut rw_refused = false;
    if received {
        // Map the shared page read-only and read the producer's sentinel through it.
        let mapped = map_page_frame(frame, PAGE_FRAME_VA, false, MEMORY_REGION);
        if mapped {
            // SAFETY: PAGE_FRAME_VA is now a mapped, readable page.
            let seen = unsafe { core::ptr::read_volatile(PAGE_FRAME_VA as *const u64) };
            read_ok = seen == capability_witness_contract::PAGE_FRAME_SENTINEL;
        }

        // Try to map it read/write. We hold it READ only, so the kernel refuses before mapping.
        rw_refused = !map_page_frame(frame, RW_VA, true, MEMORY_REGION);
    }

    // Verdict: bit 0 we read the shared sentinel, bit 1 a writable mapping was refused.
    let code = (read_ok as u64) | ((rw_refused as u64) << 1);
    send(REPORT, code, 0, 0);
    exit()
}

user_mode_runtime::panic_handler!();
