//! **Drains `login`'s audit trail so the service never blocks on it** (milestone 49).
//!
//! `components/src/login.rs` sends one [`login_contract::ATTRIBUTED`] message per successful login
//! on its `AUDIT` endpoint, a plain blocking rendezvous: with no receiver, `login`'s first
//! successful login would park its whole thread inside that `send` forever. This process is the
//! receiver, on `job_undertaker`'s pattern, and it **discards** each message: printing the record
//! would need a `WRITE` view of the terminal, which `login` holds single-session.
//!
//! - slot [`AUDIT`]: `READ` on `login`'s own `AUDIT` endpoint. Nothing else: this process cannot
//!   build anything, allocate a page, or reach any other component's memory.

//! # BUGS
//!
//! - **The audit trail this program drains is not surfaced anywhere.** DECISIONS §109
//!   (attribution is a property of a channel) names a server logging which channel a request
//!   arrived on; this program makes `login` safe to leave running, and a deployment that wants the
//!   trail read needs a real consumer in its place.
//!
//! Name: ratified 2026-09-13 (calef, working the unratified worklist), replacing `audit_sink`.
//! `login_audit_recorder` is the named successor: *"We would probably rename to
//! `login_audit_recorder` when it does record the login audit."* Whoever makes it keep records
//! performs that rename in the same change. Refused `audit_sink` (an audit trail that is discarded
//! is not an audit trail). Refused `login_audit_discarder` (names the disposal, not the role).
//! Refused `login_audit_sink` (keeps the promise that something is audited). Refused `audit_drain`
//! ("drain" is spent on the merge-drain helper).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::receive;

/// `login`'s own `AUDIT` endpoint, `READ`.
const AUDIT: u64 = 0;

#[unsafe(no_mangle)]
pub extern "C" fn _start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    loop {
        receive(AUDIT);
    }
}

user_mode_runtime::panic_handler!();
