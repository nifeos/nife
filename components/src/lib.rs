#![no_std]
// A library test build for a bare-metal target has no libtest and no C runtime to call `main`, so it
// takes the kernel's way out: a runner that has nothing to run, because the host tests are gated off.
#![cfg_attr(all(test, target_os = "none"), no_main)]
#![cfg_attr(all(test, target_os = "none"), feature(custom_test_frameworks))]
#![cfg_attr(
    all(test, target_os = "none"),
    test_runner(crate::bare_metal_test_image::bare_metal_runner)
)]
#![cfg_attr(
    all(test, target_os = "none"),
    reexport_test_harness_main = "test_main"
)]
//! The one library target of the `components` package: the logic of programs that has host tests
//! and proofs, kept out of the program's own binary because a binary of this package builds with
//! `test = false` and cannot carry them.
//!
//! Name: provisional.

pub mod block_driver_roles;

#[cfg(all(test, target_os = "none"))]
mod bare_metal_test_image;
