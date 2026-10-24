//! **Minting a rendezvous out of our own memory**, milestone 19a.
//!
//! Holds a memory region (slot 0), a channel (slot 1), and nothing else. It retypes a page of its
//! own region into a brand-new rendezvous object (`RETYPE_OBJ`), one no kernel wiring created, then
//! delegates a READ view of it over the channel and SENDs a word into it. If the object works, a
//! peer this program has never met receives that word over a rendezvous that did not exist a moment
//! ago; the `SEND` blocks until the peer receives, so reaching `exit` is itself half the proof.
//!
//! The other half is `rendezvous_peer`; the wiring is `system_tests/src/user/retype_ep_service.rs`.

//! Name: provisional (milestone 291). This was `hello`'s `EP_MAKER` role, number 17. `ep` is an
//! abbreviation that needs a decoder, which is the first of AGENTS.md's three naming failure
//! modes, and the object's own name in this tree is `RENDEZVOUS` (`abi::objtype::RENDEZVOUS`,
//! `rendezvous_cap`). Refused `rendezvous_maker`: "minting" is the word this tree already uses for
//! creating a capability out of a budget, and it appears in the kernel test's own sentence.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{exit, require, retype_into_kernel_object, send, send_capability};

const MEMORY_REGION: u64 = 0;
const CHANNEL: u64 = 1;

#[unsafe(no_mangle)]
pub extern "C" fn _start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    // Retype one page of our budget into a rendezvous; the kernel returns the slot where our
    // full-rights capability to it landed.
    let rendezvous = retype_into_kernel_object(MEMORY_REGION, abi::objtype::RENDEZVOUS);
    require(rendezvous >= 0);
    let rendezvous = rendezvous as u64;

    // Delegate a READ-only view (receive, never send) to whoever is on the channel; we keep WRITE.
    require(send_capability(CHANNEL, rendezvous, abi::rights::READ, 0) == 0);

    // Speak first through our own creation: blocks until the peer receives, which is the proof.
    require(send(rendezvous, 0x77, 0, 0) == 0);
    exit()
}

user_mode_runtime::panic_handler!();
