//! Where the JH7110's peripherals are, read out of the device tree for the drivers that take plain
//! values.
//!
//! The JH7110 device crates (`jh7110_entropy`, `jh7110_clock_and_reset`, `designware_ethernet`,
//! `designware_mobile_storage`) ship on their own, so none of them reads a device tree: each takes
//! a register base, a size and a settings value. The two chip crates hold what the chip's trees
//! call their blocks (compatible strings, node and `reg-names` spellings) and small decoders for
//! property values. The two `DesignWare` crates know no chip at all, so every JH7110 fact they need
//! (addresses, interrupts, tree spellings, bus settings) is in their submodules here. The walk
//! that finds the node and hands each property to a decoder is this module's, one submodule per
//! crate, so codebase rule 2 holds: a driver gets what it needs passed in. See
//! `notes/machine-discovery.md`.
//!
//! Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, for the chip whose
//! peripherals it finds, beside `plic`, `gic` and the other readers here. calef has not named it.

pub mod clock_and_reset;
pub mod designware_ethernet;
pub mod designware_mobile_storage;
pub mod entropy;
