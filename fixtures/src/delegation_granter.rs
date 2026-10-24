//! **The delegation demo, the half that gives**, milestone 10.
//!
//! Holds a channel to send over (slot 0) and a resource capability held `WRITE | GRANT` (slot 1).
//! It passes the resource on, narrowed to `WRITE` so the receiver can use it but cannot lend it
//! further. The whole point of a capability system, in four lines: authority a process holds,
//! handed to another process, at runtime, with less power than it arrived with.
//!
//! The other half is `delegation_receiver`; the wiring and the assertions are
//! `system_tests/src/user/delegation_service.rs` and `kernel::user::tests`.
//!
//! Name: provisional (milestone 291). This was `hello`'s `GRANTER` role, number 9. Qualified with
//! `delegation_` because `granter` alone is one of AGENTS.md's generic words: half this system
//! grants something to something. Refused `capability_granter`: every capability in this tree is a
//! capability, so the word distinguishes nothing, where `delegation` names the specific act
//! (`SEND_CAP` over a channel) this program exists to perform.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

const CHANNEL: u64 = 0;
const RESOURCE: u64 = 1;

user_mode_runtime::program_entry!(start);

fn start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    // Delegate RESOURCE, narrowed to WRITE (dropping GRANT), over CHANNEL.
    user_mode_runtime::send_capability(CHANNEL, RESOURCE, abi::rights::WRITE, 0);

    user_mode_runtime::exit() // one-shot: our authority is passed on, so we leave and the kernel reaps us
}

user_mode_runtime::panic_handler!();
