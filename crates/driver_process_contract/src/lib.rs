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

use startup_page_contract::WindowKind;

/// **Every window kind this crate mints**, one number each across all its drivers, so a page that
/// some day lists two drivers' windows in one space stays unambiguous.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
pub const WINDOW_KINDS: [WindowKind; 11] = [
    non_volatile_memory_express::DATA_PLANE_KIND,
    non_volatile_memory_express::DOORBELL_PAGE_KIND,
    e1000e::DATA_PLANE_KIND,
    e1000e::RECEIVE_QUEUE_KIND,
    e1000e::TRANSMIT_QUEUE_KIND,
    designware_ethernet::DATA_PLANE_KIND,
    designware_ethernet::DIRECT_MEMORY_ACCESS_REGISTER_PAGE_KIND,
    designware_mobile_storage::REGISTER_KIND,
    designware_mobile_storage::TRANSFER_KIND,
    extensible_host_controller_interface::REGISTER_KIND,
    extensible_host_controller_interface::WORK_KIND,
];

#[cfg(test)]
mod tests {
    use super::WINDOW_KINDS;

    /// Every kind's number is distinct and none is zero, which names no window.
    #[test]
    fn every_window_kind_has_its_own_nonzero_number() {
        for (n, kind) in WINDOW_KINDS.iter().enumerate() {
            assert_ne!(kind.0, 0);
            assert!(
                WINDOW_KINDS[..n].iter().all(|earlier| earlier != kind),
                "{kind:?} repeats"
            );
        }
    }
}
