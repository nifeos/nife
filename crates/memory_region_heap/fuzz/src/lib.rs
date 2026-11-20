//! Drive talc the way `memory_region_heap` does, from a byte string, and check the heap after
//! every step.
//!
//! [`run`] reads the bytes as a header and a sequence of operations, applies each to a copy of
//! `memory_region_heap`'s policy over a host arena ([`policy`]), and panics on the first broken
//! invariant ([`sequence`]). The fuzz target and the seed replay test both call it. [`sequence`]
//! documents the input format and the invariants.
//!
//! Name: provisional, minted 2026-11-14 (UTC) by lane talc-fuzz-and-miri, matching the directory cargo-fuzz finds.

// `cfg(sanitize)` lets the page source mark unmapped pages for AddressSanitizer; the fuzz build
// already needs nightly.
#![feature(cfg_sanitize)]

mod policy;
mod sequence;

pub use sequence::run;
