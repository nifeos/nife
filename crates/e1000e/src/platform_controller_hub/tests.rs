//! The module's host tests.
//!
//! Name: ratified 2026-11-03 (UTC), #3145. Ruled in the group E1 ruling on e1000e's module names;
//! kept as it stands.

use super::*;

#[test]
fn the_pch_class_is_every_claimed_i219_and_not_qemus_part() {
    assert!(!is_platform_controller_hub_device(0x10d3));
    for &(id, model) in super::super::DEVICE_IDS {
        assert_eq!(
            is_platform_controller_hub_device(id),
            model.starts_with("I219"),
            "{id:#06x} {model}"
        );
    }
}

#[test]
fn the_ulp_request_clears_the_indication_and_enforces() {
    let h = ultra_low_power::register_value_requesting_exit(ultra_low_power::H2ME_ULP | 0x1);
    assert_eq!(h & ultra_low_power::H2ME_ULP, 0);
    assert_ne!(h & ultra_low_power::H2ME_ENFORCE_SETTINGS, 0);
    assert_eq!(h & 1, 1, "other bits survive");
    assert_eq!(
        ultra_low_power::register_value_releasing_request(h)
            & ultra_low_power::H2ME_ENFORCE_SETTINGS,
        0
    );
    assert!(!ultra_low_power::is_configuration_done(
        ultra_low_power::FWSM_ULP_CFG_DONE
    ));
    assert!(ultra_low_power::has_management_engine(
        ultra_low_power::ICH_FWSM_FW_VALID
    ));
}

#[test]
fn the_flush_runs_only_when_asked_and_the_ring_has_length() {
    assert!(flush::is_flush_required(flush::FLUSH_DESC_REQUIRED, 256));
    assert!(!flush::is_flush_required(flush::FLUSH_DESC_REQUIRED, 0));
    assert!(!flush::is_flush_required(0, 256));
    assert_eq!(flush::tail_after_dummy_transmit(15, 16), 0);
    assert_eq!(
        flush::receive_descriptor_control_register_value_for_flush(0xffff_ffff) & 0x3fff,
        0x1f | 1 << 8
    );
    assert_ne!(
        flush::receive_descriptor_control_register_value_for_flush(0) & 0x0100_0000,
        0
    );
    // No end of packet, no report status: the dummy is never a frame anyone waits on.
    assert_eq!(
        (flush::dummy_transmit_descriptor_command_word() >> 24) as u8 & (0x01 | 0x08),
        0
    );
}

/// The flush's register values exactly as FreeBSD writes them: the dummy descriptor is 512
/// bytes with only IFCS (`txd_lower`), the tail moves one slot and wraps, and `RXDCTL` keeps
/// every bit above the two thresholds, including the granularity bit when it was already set.
#[test]
fn the_flush_values_are_freebsds() {
    assert_eq!(flush::dummy_transmit_descriptor_command_word(), 0x0200_0200);
    assert_eq!(flush::tail_after_dummy_transmit(3, 16), 4);
    assert_eq!(flush::tail_after_dummy_transmit(15, 16), 0);
    assert_eq!(
        flush::receive_descriptor_control_register_value_for_flush(0xffff_ffff),
        0xffff_c11f
    );
    assert_eq!(
        flush::receive_descriptor_control_register_value_for_flush(0x0100_0000),
        0x0100_011f
    );
    assert_eq!(
        flush::receive_descriptor_control_register_value_for_flush(0),
        0x0100_011f
    );
}

/// The ME handshake touches only its own two bits: the request sets `ENFORCE_SETTINGS` even
/// when it is already set, and the release clears it and nothing else.
#[test]
fn the_ulp_handshake_changes_only_its_own_bits() {
    assert_eq!(
        ultra_low_power::register_value_requesting_exit(ultra_low_power::H2ME_ENFORCE_SETTINGS)
            & ultra_low_power::H2ME_ENFORCE_SETTINGS,
        ultra_low_power::H2ME_ENFORCE_SETTINGS
    );
    assert_eq!(
        ultra_low_power::register_value_releasing_request(u32::MAX),
        !ultra_low_power::H2ME_ENFORCE_SETTINGS
    );
    assert_eq!(ultra_low_power::register_value_releasing_request(0x1), 0x1);
}

/// A millisecond is a thousand microseconds of the device's clock, and a posted-write flush is
/// one read of `STATUS`, FreeBSD's `E1000_WRITE_FLUSH`.
#[test]
fn a_delay_is_in_milliseconds_and_a_flush_reads_status() {
    let mut s =
        simulator::SimulatedHardware::new(simulator::PhysicalLayerDeviceStartState::Healthy, true);
    delay_ms(&mut s, 3);
    assert_eq!(s.elapsed_microseconds, 3000);
    flush_writes(&mut s);
    assert_eq!(s.reads, [crate::registers::DEVICE_STATUS_REGISTER]);
}
