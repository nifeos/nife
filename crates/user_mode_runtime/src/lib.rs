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

/// The version of the entry registers and link layout a program starts with: 1. A break bumps it,
/// with the dated ledger entry the interface-stability discipline owes; an addition an older peer
/// ignores does not.
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane lane/956-interface-ids-for-the-owed-formats.
pub const VERSION: u32 = 1;

/// This interface's identity in a program's `INTERFACES` note: 20, naming the entry registers and
/// link layout a program starts with. The number is this interface's for good: no other interface
/// takes it, and a format change is a new version, never a new number. Milestone 956 (the linked
/// interfaces carry checkable versions) holds the table of numbers, and `script/lint` holds the
/// tree to it. A program declares the pair of this id and [`VERSION`].
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane lane/956-interface-ids-for-the-owed-formats.
pub const INTERFACE_ID: u64 = 20;

pub mod child_stub;
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
