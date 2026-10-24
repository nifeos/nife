//! The two Kani attribute macros, as pass-throughs, so `script/lint` can compile the proof
//! harnesses without Kani installed (milestone 113 (the proofs' own unsafe code is ungated)). Half
//! of the shim; `kani.rs` beside it re-exports these and supplies `any`, `assume` and `cover!`. An
//! attribute macro can only come from a proc-macro crate, hence two crates
//! (notes/unsafe-obligations/kani-gate.md). It is built by a plain `rustc` and is deliberately not
//! a workspace package: a dependency named `kani` in a harness crate's manifest would collide with
//! the real library `cargo kani` injects.
//!
//! **Names are provisional** (CLAUDE.md: calef names crates, programs and modules); unlike `kani`,
//! this one is internal to the shim and could be called anything.

extern crate proc_macro;

use proc_macro::TokenStream;

/// `#[kani::proof]`: keep the function, and silence the dead-code warning it would otherwise earn.
///
/// A harness is called by nothing (Kani finds it by attribute), so under `-D warnings` every one of
/// the 108 harnesses in the tree would fail the lint pass as dead code. The cost of the blanket is
/// stated plainly: this pass cannot see a genuinely unused *harness* either. It can still see an
/// unused helper beside one, which is the case worth catching.
#[proc_macro_attribute]
pub fn proof(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut out: TokenStream = "#[allow(dead_code)]".parse().unwrap();
    out.extend(item);
    out
}

/// `#[kani::unwind(N)]`: the loop bound, which only the model checker has any use for.
#[proc_macro_attribute]
pub fn unwind(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
