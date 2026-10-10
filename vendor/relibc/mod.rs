//! relibc's OS-neutral code, seeded into nife's C library (milestone 835, §265).
//!
//! Seeded from relibc (MIT, `LICENSE` here) at `893a3b9133ac2fb3089f71b02d5b61d145d97968`
//! (2026-10-07), on 2026-10-10 (UTC). This is the module list of relibc's `src/lib.rs` cut to what
//! stage 1 seeds; the crate root that includes it is `c_library/src/lib.rs`, which is nife's.
//! What was taken and left behind, and how nife's edits are marked, is in `vendor/README.md`.

#[macro_use]
pub(crate) mod macros;
pub mod c_str;
pub mod c_vec;
pub mod casting;
pub mod error;
pub mod fs;
pub mod header;
pub mod io;
pub mod iter;
pub mod out;
pub mod plain;
pub mod platform;
pub mod raw_cell;
pub mod sync;
