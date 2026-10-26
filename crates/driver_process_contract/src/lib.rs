//! **What the kernel and a device-driver process agree on**: where the process finds each window
//! the kernel maps for it, and the spawn words that say what it may touch. One module per driver.
//!
//! The device crates hold only device logic and ship on their own; the process layout around them
//! is nife's, so it lives here, linked by the kernel that spawns each driver and by the component
//! that is the driver. Ruled 2026-10-25 (UTC), recorded in milestone 857 (crates released on
//! their own).
//!
//! Name: ratified 2026-10-26 (UTC), on #2749, for now: it is expected to break up as components
//! move into programs and crates of their own.

#![cfg_attr(not(test), no_std)]

pub mod designware_ethernet;
pub mod designware_mobile_storage;
pub mod e1000e;
pub mod extensible_host_controller_interface;
pub mod non_volatile_memory_express;
