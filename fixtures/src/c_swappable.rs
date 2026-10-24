//! The swappable component, version 2: the replacement, written in C (milestone 23 (a
//! capability-routed component OS with live replacement), DECISIONS §41 (the endpoint is the
//! broker), over the seam DECISIONS §31 (the foreign-language seam) built).
//!
//! Identical to [`rust_swappable`](../rust_swappable/index.html) in everything a client can
//! observe: capability layout, wire protocol and serving loop (both compile `swap.rs`). Only the
//! answer differs in origin: `cx_digest` in `fixtures/c/c_swappable.c`, linked into this binary
//! alone. The C holds no capability and makes no syscall; its whole interface is
//! `(uint64_t) -> uint64_t`. A different implementation in another language is what makes "a client
//! did not notice the swap" a claim about the contract. See notes/live-replacement.md.

//! Name: ratified 2026-08-01 (calef, milestone 61), replacing `cconx`. Refused `conx` and `cconx`:
//! no expansion for `conx` is recorded anywhere, not in DECISIONS §41, not in
//! notes/live-replacement.md, not in the commit that introduced it, and that absence is one of the
//! two cases that made naming a ratified act.

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

// The foreign component. One function, one ABI: an integer in, an integer out. No structs, no
// callbacks, no pointers, nothing that needs a header file to agree on.
unsafe extern "C" {
    fn cx_digest(seq: u64) -> u64;
}

/// The Rust side of the seam: a safe wrapper with the signature the serving loop wants. Every call
/// into the foreign component goes through here, which is why there is exactly one `unsafe` block
/// in this program.
fn digest_in_c(seq: u64) -> u64 {
    // SAFETY: `cx_digest` is pure arithmetic on a value passed by register. It dereferences
    // nothing, so there is no pointer for it to get wrong, which is what makes this the narrowest
    // possible version of the C seam.
    unsafe { cx_digest(seq) }
}

user_mode_runtime::program_entry!(start);

fn start(a0: u64, log_base: u64, a2: u64) -> ! {
    hot_swap_contract::start(hot_swap_contract::V2, digest_in_c, a0, log_base, a2)
}

user_mode_runtime::panic_handler!();
