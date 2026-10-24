//! **The swappable component, version 1: the incumbent** (milestone 23, DECISIONS §41).
//!
//! A console-shaped server that owns a device (the UART's registers), serves one stable endpoint,
//! and can be taken out from under its client mid-conversation.
//!
//! The serving loop in `swap.rs` is shared byte for byte with
//! [`c_swappable`](../c_swappable/index.html), the replacement; only the function that computes an
//! answer differs (Rust here, C there), which makes the milestone's claim about the component
//! rather than the harness. See notes/live-replacement.md.

//! # What it holds
//!
//! Four capabilities and two pages, all placed by `swapper` before it ran: the stable service
//! endpoint (READ), a report endpoint (WRITE), the operator's coordination channel (WRITE) and
//! post-revoke poke (READ), the shared log page (read/write), and the device's registers. No
//! initial ramdisk, no budget: a compromised instance is a bad console and nothing else.
//!
//! Name: ratified 2026-08-01 (calef, milestone 61), replacing `conx`. Refused `conx`, whose
//! expansion is recorded nowhere in this tree; that absence is one of the two cases the naming
//! tenet cites as evidence that it needed a rule.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

// This is its own binary. What it shares with the other hot-swap programs comes from the
// `hot_swap_contract` crate.
// Shared code was a `#[path]` module compiled into each binary until rule 7 (2026-08-01) made it
// a crate, so this file needs no dead-code allow under §38 (a suppression is scoped to an item
// and carries a reason).

user_mode_runtime::program_entry!(start);

/// `a0` says whether the operator endowed us with a device, and `a1` where our entries start in the
/// shared witness page. Both come from the operator, because both are facts about the wiring rather
/// than about this program: the incumbent on the direct channel owns a UART, the backend behind the
/// queue broker does not, and neither can tell by looking.
///
/// `a2` is the sequence number to stop answering at, or zero to serve honestly, which is what the
/// two healthy channels pass. It is a start argument rather than a manifest field for the reason
/// notes/component-manifest.md gives: a manifest declares authority, and this carries none. A
/// component lied to about it misbehaves rather than escaping, which is precisely what the hung
/// channel wants it to do.
fn start(a0: u64, log_base: u64, a2: u64) -> ! {
    hot_swap_contract::start(
        hot_swap_contract::V1,
        hot_swap_contract::digest,
        a0,
        log_base,
        a2,
    )
}

user_mode_runtime::panic_handler!();
