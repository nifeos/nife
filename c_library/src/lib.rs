//! nife's C library, stage 1: files, clock and memory (milestone 835 (a C library, stage 1: files,
//! clock and memory), §265 (a C library started from relibc, whose Rust platform layer holds the
//! capabilities)).
//!
//! Two halves, kept in two places so a reader and a gate can tell them apart:
//!
//! - **relibc's code**, seeded once at a recorded commit into `vendor/relibc/` and owned by nife
//!   from then on (no upstream tracking; fixes are ported by hand). Its header modules (`stdio`,
//!   `string`, `stdlib`, `time` and the rest) call the operating system only through its `Pal`
//!   trait. Every place nife changed it says `nife:` where the change is. `vendor/README.md` lists
//!   what was taken, what was left behind and why.
//! - **nife's code**, here: the platform layer (`platform/nife.rs`, `Pal` implemented on nife's
//!   Rust `std`), `malloc` on `std`'s allocator (`platform/allocator.rs`), and the start of a C
//!   program (`start.rs`). Every gate this tree runs reads these files.
//!
//! No C translation unit makes a syscall and neither does this crate: every call that leaves the
//! process goes through `std` (§31 (the foreign-language seam) rule 1 as amended by §265).
//! `c_library/README.md` is the guide.
//!
//! The crate is `no_std` so the seeded files keep relibc's `core::` and `alloc::` paths unchanged;
//! `std` is linked for the platform layer.
//!
//! Name: provisional 2026-10-10 (UTC), milestone 835's lane, after `notes/c-library.md`. calef has
//! not ruled; §265 leaves the crate's name to him.

#![no_std]
#![feature(core_intrinsics)]
#![feature(linkage)]
#![feature(macro_derive)]
#![feature(ptr_as_uninit)]
#![feature(slice_ptr_get)]
#![feature(stmt_expr_attributes)]
#![feature(sync_unsafe_cell)]
// relibc's `printf` and `strto*` use compiler intrinsics, which is an internal feature.
#![allow(internal_features)]

#[macro_use]
extern crate alloc;
extern crate std;

// relibc's code under relibc's own lint settings (its `Cargo.toml` `[workspace.lints]`): POSIX
// names are lower case and C-shaped, and a seeded header carries functions no stage-1 program calls
// (`dead_code`), which the next stage's consumers will.
#[path = "../../vendor/relibc/mod.rs"]
#[macro_use]
#[allow(
    dead_code,
    non_camel_case_types,
    non_upper_case_globals,
    non_snake_case,
    unexpected_cfgs,
    unreachable_code
)]
mod relibc;

// relibc's modules at the crate root, where its own `crate::` paths expect them.
pub(crate) use relibc::macros;
pub use relibc::{
    c_str, c_vec, casting, error, fs, header, io, iter, out, plain, platform, raw_cell, sync,
};

#[cfg(target_arch = "x86_64")]
mod long_double;
pub mod start;
