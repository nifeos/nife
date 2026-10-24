//! **`user_mode_runtime`**: the user-mode runtime every nife userspace program links, on all three
//! architectures (`EL0` on aarch64, U-mode on riscv64, ring 3 on `x86_64`).
//!
//! [`invoke`] is the one syscall wrapper. [`send`], [`receive`], [`call`] and [`exit`] are built on
//! it, and [`require`] is how a program says no. [`panic_handler!`] expands to each binary's
//! handler over [`trap`]. Opt-in modules ([`mapped_window`], [`initial_ramdisk`], [`virtio`]) hold
//! what only some programs need. Nothing here runs on the host, so the examples are `no_run`. The
//! signatures and `abi` constants are identical on all three ABIs (§124 (ratify the x86_64 syscall
//! ABI) for `x86_64`'s). See notes/user-mode-runtime.md.
//!
//! Name: ratified 2026-09-13 (calef), replacing `user_rt`: `rt` is an abbreviation
//! that needs a decoder, and in this tree `user` alone means a person, so `user_mode_` names the
//! privilege level. Refused `user_runtime` (ambiguous between a person and a privilege level) and
//! `el0_runtime` (correct on one architecture and wrong on the other two).

#![no_std]

pub mod child_stub;
#[cfg(feature = "virtio-drivers")]
pub mod confined_virtio;
pub mod entropy;
pub mod initial_ramdisk;
pub mod mapped_window;
pub mod socket_frame;
pub mod virtio;

mod clock;
mod devices;
mod inter_process_communication;
mod objects;
mod process;
mod syscall;

pub use abi::cycle_counter::CycleMeaning;
pub use clock::*;
pub use devices::*;
pub use inter_process_communication::*;
pub use objects::*;
pub use process::*;
pub use syscall::*;
