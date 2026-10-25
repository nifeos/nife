//! Where the JH7110's peripherals are, read out of the device tree for the drivers that take plain
//! values.
//!
//! The JH7110 device crates (`jh7110_entropy`, `jh7110_clock_and_reset`, `designware_ethernet`,
//! `designware_mobile_storage`) ship on their own, so none of them reads a device tree: each holds
//! what the chip's two trees call its blocks (compatible strings, node and `reg-names` spellings)
//! and small decoders for property values, and takes a register base, a size and a settings value.
//! The walk that finds the node and hands each property to a decoder is this module's, one
//! submodule per crate, so codebase rule 2 holds: a driver gets what it needs passed in. See
//! `notes/machine-discovery.md`.
//!
//! The ruling is calef's, 2026-10-25 (UTC), recorded in milestone 857 (crates released on their
//! own).
//!
//! Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, for the chip whose
//! peripherals it finds, beside `plic`, `gic` and the other readers here. calef has not named it.

pub mod entropy;
