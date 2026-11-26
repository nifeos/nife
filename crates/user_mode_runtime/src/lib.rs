//! **`user_mode_runtime`**: the user-mode runtime every user-mode program of the kernel links, on all three
//! architectures (`EL0` on aarch64, U-mode on riscv64, ring 3 on `x86_64`).
//!
//! Every wrapper builds its call from `moho_abi::system_call`, which the method table generates, and
//! traps through one six-register round trip; [`invoke`] is that trap for a caller naming the method
//! itself. [`require`] is how a program says no. [`panic_handler!`] expands to each binary's
//! handler over [`trap`]. Opt-in modules ([`mapped_window`], [`initial_ramdisk`], [`virtual_io`]) hold
//! what only some programs need. A system call runs only in a user-mode process under the kernel,
//! so the examples that make one are `no_run`; the pure logic has host tests. The signatures and
//! `moho_abi` constants are identical on all three application binary interfaces (ABIs). The design
//! behind them is in `docs/design.md`, and time on each architecture in `docs/time.md`.
//!
//! Name: ratified 2026-09-13 (calef), replacing `user_rt`: `rt` is an abbreviation
//! that needs a decoder, and in this tree `user` alone means a person, so `user_mode_` names the
//! privilege level. Refused `user_runtime` (ambiguous between a person and a privilege level) and
//! `el0_runtime` (correct on one architecture and wrong on the other two).

#![no_std]

/// The version of the entry registers and link layout a program starts with: 1. A break bumps it,
/// with the dated ledger entry the interface-stability discipline owes; an addition an older peer
/// ignores does not.
///
/// Name: ratified 2026-11-20 (UTC); first minted, minted 2026-11-01 (UTC) by lane lane/956-interface-ids-for-the-owed-formats.
pub const VERSION: u32 = 1;

/// This interface's identity in a program's `INTERFACES` note: 20, naming the entry registers and
/// link layout a program starts with. The number is this interface's for good: no other interface
/// takes it, and a format change is a new version, never a new number. A program declares the
/// pair of this id and [`VERSION`].
///
/// Name: ratified 2026-11-20 (UTC); first minted, minted 2026-11-01 (UTC) by lane lane/956-interface-ids-for-the-owed-formats.
pub const INTERFACE_ID: u64 = 20;

pub mod child_stub;
pub mod entropy;
pub mod initial_ramdisk;
pub mod mapped_window;
pub mod socket_frame;
#[doc(alias = "virtio")]
pub mod virtual_io;

mod clock;
mod devices;
mod inter_process_communication;
mod objects;
mod process;
mod system_call;

pub use clock::*;
pub use devices::*;
pub use inter_process_communication::*;
pub use moho_abi::cycle_counter::CycleMeaning;
pub use objects::*;
pub use process::*;
pub use system_call::*;

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod linker_script;
