//! Enough of Kani's API for `clippy` to type-check the proof harnesses (milestone 113).
//!
//! `script/lint` compiles this with `rustc` and runs a clippy pass with `--cfg kani` and
//! `--extern kani=` pointing here, so `#[cfg(kani)]` code meets clippy at all: `cargo kani` drives
//! a rustc, where no `clippy::` lint can fire. **It is not Kani, and a clean lint pass is not a
//! proof.** It covers the five items the harnesses use ([`any`], [`assume`], `cover!`, `proof`,
//! `unwind`) with the semantics deleted, and its `any` takes any `T` where Kani's requires
//! `T: Arbitrary`, because a lint gate must never reject code the model checker accepts.
//! notes/unsafe-obligations/kani-gate.md has the measurement and the reasons.

//! # BUGS
//!
//! **A harness that reaches for Kani API this does not have breaks the lint pass, not the proof.**
//! `kani::proof_for_contract`, `#[derive(kani::Arbitrary)]` and `kani::any_where` are real and
//! absent here. The compile error names the missing item; add the item rather than drop the pass.
//!
//! **Names are provisional**, per CLAUDE.md's naming tenet, except for `kani` itself: the harnesses
//! write `kani::any`, so the crate has exactly one possible name.

#![no_std]

pub use kani_attributes::{proof, unwind};

/// A symbolic value. Under Kani this is the whole point; here it only has to type-check.
///
/// Unbounded in `T` on purpose, so the lint never rejects what Kani accepts (see the module doc).
pub fn any<T>() -> T {
    unimplemented!("the lint shim's `kani::any` is never executed")
}

/// A constraint on the symbolic state. Under Kani it prunes the search; here it does nothing.
pub fn assume(_condition: bool) {}

/// `kani::cover!(cond)`: an assertion that the state is *reachable*, which is how the harnesses
/// check that a bound has not quietly made a proof vacuous. Evaluated and discarded here, so the
/// expression is still type-checked and its variables still count as used.
#[macro_export]
macro_rules! cover {
    ($condition:expr $(, $message:literal)? $(,)?) => {
        let _ = $condition;
    };
}
