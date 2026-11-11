//! **The window kinds this crate mints**, held apart: each driver module names its own windows'
//! kinds, and [`ALL`] lists them so one test sees every number.
//!
//! Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.

use startup_page_contract::WindowKind;

/// **Every window kind this crate mints**, one number each across all its drivers, so a page that
/// some day lists two drivers' windows in one space stays unambiguous.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/startup-page-spawners.
pub const ALL: [WindowKind; 18] = [
    crate::non_volatile_memory_express::REGISTER_KIND,
    crate::non_volatile_memory_express::ADMINISTRATION_QUEUE_KIND,
    crate::e1000e::DATA_PLANE_KIND,
    crate::e1000e::RECEIVE_QUEUE_KIND,
    crate::e1000e::TRANSMIT_QUEUE_KIND,
    crate::designware_ethernet::DATA_PLANE_KIND,
    crate::designware_ethernet::DIRECT_MEMORY_ACCESS_REGISTER_PAGE_KIND,
    crate::designware_mobile_storage::REGISTER_KIND,
    crate::designware_mobile_storage::TRANSFER_KIND,
    crate::extensible_host_controller_interface::REGISTER_KIND,
    crate::extensible_host_controller_interface::WORK_KIND,
    crate::jh7110_clock_and_reset::SYSTEM_TOP_GROUP_KIND,
    crate::jh7110_clock_and_reset::SYSTEM_KIND,
    crate::jh7110_clock_and_reset::ALWAYS_ON_KIND,
    crate::jh7110_clock_and_reset::SYSTEM_CONTROLLER_KIND,
    crate::power_management_chip_route::CONTROLLER_KIND,
    crate::virtual_input_output::REGISTER_KIND,
    crate::virtual_input_output::DATA_PLANE_KIND,
];

#[cfg(test)]
mod tests {
    use super::ALL;

    /// Every kind's number is distinct and none is zero, which names no window.
    #[test]
    fn every_window_kind_has_its_own_nonzero_number() {
        for (n, kind) in ALL.iter().enumerate() {
            assert_ne!(kind.0, 0);
            assert!(
                ALL[..n].iter().all(|earlier| earlier != kind),
                "{kind:?} repeats"
            );
        }
    }
}
