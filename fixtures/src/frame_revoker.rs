//! Revoking a frame across the boundary (milestone 13 (capability revocation and untyped
//! reclamation)).
//!
//! Holds a memory region (slot 0) and a report endpoint (slot 1). It retypes a page, maps it, then
//! `REVOKE`s it: the kernel unmaps the page and deletes every capability to it, this process's own
//! included. It reports 1 if the revoke succeeded and the slot is now empty. It never touches the
//! address again, because reading the unmapped page would fault before it could report and turn a
//! pass into a hang. `kernel/src/revoke.rs` proves the multi-address-space unmapping and safe
//! reclamation; this adds the syscall path from EL0.
//!
//! Name: provisional (milestone 291). This was `hello`'s `REVOKE_DEMO` role, number 16. Refused
//! `revoke_demo`: "demo" says why it was written rather than what it does, and `revoke` alone is a
//! verb where the tree names things with nouns. `frame_revoker` says which object is revoked,
//! which matters because object revocation applies to more than frames.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{
    exit, map_page_frame, require, retype_into_page_frame, revoke_page_frame, send,
};

const MEMORY_REGION: u64 = 0; // retype + page tables
const REPORT: u64 = 1;
const VA: u64 = address_space_map::pair_page(0x0000_0000_00c0_0000);

user_mode_runtime::program_entry!(start);

fn start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    // Retype a page into a PageFrame capability we hold, then map it writable.
    let frame = retype_into_page_frame(MEMORY_REGION);
    require(frame >= 0);
    let frame = frame as u64;
    require(map_page_frame(frame, VA, true, MEMORY_REGION));
    // SAFETY: VA is now a mapped, writable page in our address space.
    unsafe { core::ptr::write_volatile(VA as *mut u64, 0xABCD) };

    // Revoke: unmap the page everywhere and delete every capability to it, ours included. The
    // frame was retyped from a budget holding GRANT (since milestone 824 (a retype mints no right
    // its budget lacks) a frame carries its budget's rights), so we are allowed to.
    let revoked = revoke_page_frame(frame);
    // Our PageFrame capability is gone now: a second operation on that slot must fail
    // (NoSuchSlot). We do NOT touch VA again, which is unmapped and would fault.
    let after = if map_page_frame(frame, VA, true, MEMORY_REGION) {
        0
    } else {
        -1
    };

    send(REPORT, if revoked == 0 && after < 0 { 1 } else { 0 }, 0, 0);
    exit()
}

user_mode_runtime::panic_handler!();
