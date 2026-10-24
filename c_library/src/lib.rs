//! nife's C library, stage 1: files, clock and memory (milestone 835 (a C library, stage 1: files,
//! clock and memory), §265 (a C library started from relibc, whose Rust platform layer holds the
//! capabilities)).
//!
//! relibc's code is seeded once into `vendor/relibc/` and owned by nife from then on; it reaches
//! the operating system only through its `Pal` trait. nife's code is here: the platform layer
//! (`platform/nife.rs`), `malloc` (`platform/allocator.rs`) and a C program's start (`start.rs`).
//! No C translation unit and nothing here makes a syscall (§31 (the foreign-language seam) rule 1
//! as amended by §265). The crate is `no_std` so the seeded files keep relibc's `core::` and
//! `alloc::` paths, and links `std` for the platform layer. `c_library/README.md` is the guide.

//! Name: provisional 2026-10-10 (UTC), milestone 835's lane, after `notes/c-library.md`. calef has
//! not ruled; §265 leaves the crate's name to him.

#![no_std]
#![feature(core_intrinsics)]
#![feature(linkage)]
#![feature(macro_derive)]
// relibc's read-write lock guards are `!Send` (milestone 836 (a C library, stage 2: threads)).
#![feature(negative_impls)]
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
    dropping_copy_types,
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
// POSIX threads on `std`'s, in place of relibc's own module (milestone 836).
pub mod pthread;
pub mod start;
