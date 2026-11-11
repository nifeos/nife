//! **Restarting the machine**: the method the reboot object answers, and the cold reset the
//! kernel's own bench paths ask for.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// `REBOOT`: restart the machine, or say why not (§251 (restarting the machine is a kernel object
/// the progenitor hands out), "The method").
///
/// Returns only when the firmware refused, with the refusal already printed; a successful reset
/// does not come back. The caller is `syscall::invoke`'s arm for `Object::Reboot`, and the
/// authority check is the capability itself: §251 asks for no rights bit beyond holding it.
pub(crate) fn restart() -> abi::Error {
    // Out of the ring first: once the reset starts, the drainer never runs again.
    crate::console::enter_reset();
    println!("{MARKER} the kernel was asked to restart the machine");
    let refused = arch::reboot(MARKER);
    println!(
        "{MARKER} the firmware refused the reset ({refused:?}; the lines above say how); the \
         machine keeps running"
    );
    refused
}

/// **The kernel's own cold reset**, for the rebooting soak and the storage bench, which hold no
/// capability: the board's PMIC route first where there is one, then the firmware.
///
/// The order is the one `reboot` keeps, so the bench paths and the prompt reset a board the same
/// way. Returns only when every route was refused, each refusal already on the console. See
/// design/roadmap/0592-radons-reboot-dies-in-opensbis-pmic-write.md.
#[cfg_attr(
    not(any(feature = "reboot_soak_test", feature = "storage_read_matrix")),
    allow(dead_code)
)]
pub(crate) fn cold_reset(marker: &str) -> abi::Error {
    crate::user::power_management_chip_route_service::reset(marker);
    arch::reboot(marker)
}
