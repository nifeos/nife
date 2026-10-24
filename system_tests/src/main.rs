//! The system-test image: the kernel, linked as a library, with the whole-system suite on top.
//!
//! Name: provisional, minted on 2026-09-27 by the lane for milestone 609 (the system tests leave the
//! kernel crate). calef ruled the shape that day (a system-test image crate that links the kernel as
//! a library, the suite moved unchanged) and called the name provisional. Nothing was refused yet.

//! # What this is
//!
//! `cargo test -p system_tests --target <arch>` builds a kernel whose boot ends by running every
//! `#[test_case]` in this crate, then reports through semihosting exactly as `cargo test -p kernel`
//! does for the kernel's own unit tests. `script/test` runs both images on each architecture. The
//! kernel crate itself links no service or fixture crate; everything the suite names is a
//! dev-dependency here.

//! # How the boot reaches the suite
//!
//! The kernel's `_start` and `kernel_main` are the library's, unchanged. Built with the kernel's
//! `system_tests` feature, `kernel_main` calls `system_tests_main`, defined below, where a
//! `cargo test -p kernel` image calls its own `test_main`. The two crate roots below make the moved
//! files' paths work unchanged: `crate::sched::...` resolves through the kernel's
//! `system_test_access` facade, and `crate::println!` through the kernel's exported macros.

//! # The rule for new tests (calef, 2026-09-27)
//!
//! A new service test is a userspace program, unless it has to observe kernel internals. A test
//! that does, and so belongs here, says in its doc comment which internals it observes and why no
//! syscall could show them. Existing tests that observe nothing internal move to userspace over
//! time; notes/system-tests-and-the-kernel-crate.md keeps that backlog.

//! # BUGS
//!
//! - A plain `cargo build -p system_tests` builds a bootable kernel that runs an empty suite and
//!   halts. It proves the crate links and nothing else.
//! - The facade re-exports whole modules' `pub` items, so a test can reach any public kernel item
//!   without saying so. The rule above is prose, not a gate.
#![no_std]
#![no_main]
#![allow(missing_docs)]
#![feature(custom_test_frameworks)]
#![test_runner(kernel::system_test_access::testing::runner)]
#![reexport_test_harness_main = "test_main"]

// Macros the kernel exports (`println!`, `print!`), for the `crate::println!` the suite writes.
// The kernel modules the suite observes, at the paths it names them by.
#[allow(unused_imports)]
pub use kernel::system_test_access::*;
#[allow(unused_imports)]
pub use kernel::*;

// All of it is the suite, and its crates are dev-dependencies, so it exists only under `cargo test`.
#[cfg(test)]
pub mod user;

mod entry;

pub use entry::*;
