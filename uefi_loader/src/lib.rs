//! **The pure half of the UEFI loader** (milestone 87): the firmware table layouts, the
//! `hvm_start_info` this loader writes, and the ELF reading that places an image by its *physical*
//! addresses.
//!
//! A library for the reason `crates/device_tree_blob` and `machine_discovery` exist: **a structure
//! layout proved only by booting is proved by nothing that runs in milliseconds.** Everything here
//! compiles for the host and is tested there. The binary (`src/main.rs`) calls firmware and leaves
//! long mode, so it cannot be. See notes/x86-uefi-boot.md, and `src/main.rs` for the boot sequence.

#![cfg_attr(not(test), no_std)]

/// Whether the file this loader came from is on a given disk; the chooser's one device-path question.
pub mod device_path;
pub mod device_tree_from_acpi;
pub mod device_tree_patch;
pub mod handoff;
pub mod image;
/// The one line this loader leaves on the screen across the handoff, milestone 243 (a machine with no serial port).
pub mod screen;
