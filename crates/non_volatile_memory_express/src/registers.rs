//! **Register offsets in BAR0** (section 3.1). All are 4-byte registers or 8-byte registers the
//! caller accesses as two 4-byte halves (the specification permits either for the 64-bit ones).
//!
//! Name: ratified 2026-09-17 (#1806), with the crate.

/// Controller Capabilities (the register the specification calls `CAP`), 64-bit read-only.
/// See [`crate::Capabilities`].
///
/// Name: ratified 2026-10-28 (UTC), renamed from `CAP`: `cap` is ruled to expand. The
/// specification's name for the register stays searchable through the alias `CAP`.
#[doc(alias = "CAP")]
pub const CAPABILITIES: u64 = 0x00;
/// Controller Configuration (the register the specification calls `CC`), 32-bit. See
/// [`crate::enabled_controller_configuration`].
///
/// Name: ratified 2026-11-06 (UTC). Renamed from `CC`, spelled out under the register-constant naming rule, and
/// given the `_REGISTER` ending the naming rule asks of an offset. The specification's name stays
/// searchable through the alias `CC`.
#[doc(alias = "CC")]
pub const CONTROLLER_CONFIGURATION_REGISTER: u64 = 0x14;
/// Controller Status (the register the specification calls `CSTS`), 32-bit read-only. See
/// [`crate::CONTROLLER_STATUS_READY`].
///
/// Name: ratified 2026-11-06 (UTC). Renamed from `CSTS`, spelled out and given the `_REGISTER` ending as
/// [`CONTROLLER_CONFIGURATION_REGISTER`] was. The alias `CSTS` keeps the specification's name.
#[doc(alias = "CSTS")]
pub const CONTROLLER_STATUS_REGISTER: u64 = 0x1c;
/// Admin Queue Attributes (the register the specification calls `AQA`):
/// (ACQ entries - 1) << 16 | (ASQ entries - 1).
///
/// Name: ratified 2026-11-06 (UTC). Renamed from `AQA`, spelled out and given the `_REGISTER` ending as
/// [`CONTROLLER_CONFIGURATION_REGISTER`] was. The alias `AQA` keeps the specification's name.
#[doc(alias = "AQA")]
pub const ADMIN_QUEUE_ATTRIBUTES_REGISTER: u64 = 0x24;
/// Admin Submission Queue Base Address (the register the specification calls `ASQ`), 64-bit,
/// page-aligned physical.
///
/// Name: ratified 2026-11-06 (UTC). Renamed from `ASQ`, spelled out and given the `_REGISTER` ending as
/// [`CONTROLLER_CONFIGURATION_REGISTER`] was. The alias `ASQ` keeps the specification's name.
#[doc(alias = "ASQ")]
pub const ADMIN_SUBMISSION_QUEUE_BASE_ADDRESS_REGISTER: u64 = 0x28;
/// Admin Completion Queue Base Address (the register the specification calls `ACQ`), 64-bit,
/// page-aligned physical.
///
/// Name: ratified 2026-11-06 (UTC). Renamed from `ACQ`, spelled out and given the `_REGISTER` ending as
/// [`CONTROLLER_CONFIGURATION_REGISTER`] was. The alias `ACQ` keeps the specification's name.
#[doc(alias = "ACQ")]
pub const ADMIN_COMPLETION_QUEUE_BASE_ADDRESS_REGISTER: u64 = 0x30;
/// The first doorbell. Doorbell N is at `DOORBELL_BASE + N * (4 << CAP.DSTRD)`; see
/// [`crate::doorbell`].
///
/// Name: ratified 2026-10-25 (#2720).
pub const DOORBELL_BASE: u64 = 0x1000;

/// Controller Status bit 0 (`CSTS.RDY`): the controller has come up (after the enable bit of
/// Controller Configuration is set) or gone down (after it is cleared).
///
/// Name: ratified 2026-11-06 (UTC). Renamed from `CSTS_RDY`, spelled out. The alias `CSTS.RDY` keeps the
/// specification's name.
#[doc(alias = "CSTS.RDY")]
#[doc(alias = "RDY")]
pub const CONTROLLER_STATUS_READY: u32 = 1;
/// Controller Status bit 1 (`CSTS.CFS`, Controller Fatal Status): the controller gave up, and no
/// doorbell will help.
///
/// Name: ratified 2026-11-06 (UTC). Renamed from `CSTS_CFS`, spelled out. The aliases `CSTS.CFS` and `CFS` keep
/// the specification's name.
#[doc(alias = "CSTS.CFS")]
#[doc(alias = "CFS")]
pub const CONTROLLER_STATUS_FATAL: u32 = 1 << 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fatal_bit_is_a_different_bit_from_the_ready_bit() {
        // Nothing in this crate reads Controller Status; the caller's volatile layer does. This
        // transcription check is the only place on the host that would catch a constant copied to
        // the wrong bit (the fatal bit is bit 1, the ready bit is bit 0).
        assert_eq!(CONTROLLER_STATUS_FATAL, 0b10);
        assert_ne!(CONTROLLER_STATUS_FATAL, CONTROLLER_STATUS_READY);
    }
}
