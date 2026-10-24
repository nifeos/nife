//! **The peer on the other end of a minted rendezvous**, milestone 19a.
//!
//! Holds the channel (slot 0) and a report endpoint (slot 1). It receives a capability to a
//! rendezvous some other process minted out of its own memory, listens on it, and reports what
//! arrives.
//!
//! **It never saw a memory region and never asked the kernel to create anything.** Its authority to
//! listen arrived entirely by delegation, the half of milestone 19a the minter cannot demonstrate
//! on its own. The other half is `rendezvous_minter`.

//! Name: provisional (milestone 291). This was `hello`'s `EP_USER` role, number 18. `user` was
//! doubly wrong: it is a generic word, and in this tree it already means "a person with an
//! identity" (`login`, `credentialer`) and "the unprivileged half of the machine" (`kernel/src/user`).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use abi::rendezvous;
use user_mode_runtime::{exit, receive, receive_cap, require, send};

const CHANNEL: u64 = 0;
const REPORT: u64 = 1;

#[unsafe(no_mangle)]
pub extern "C" fn _start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    let (_w, slot, _) = receive_cap(CHANNEL);
    require(slot != rendezvous::NO_CAP);

    let (w0, _, _) = receive(slot); // listen on the minted rendezvous
    send(REPORT, w0, 0, 0); // report the word that crossed it
    exit()
}

user_mode_runtime::panic_handler!();
