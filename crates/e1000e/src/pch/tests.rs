//! The module's host tests.

use super::*;

#[test]
fn the_pch_class_is_every_claimed_i219_and_not_qemus_part() {
    assert!(!is_pch(0x10d3));
    for &(id, model) in super::super::DEVICE_IDS {
        assert_eq!(is_pch(id), model.starts_with("I219"), "{id:#06x} {model}");
    }
}

#[test]
fn the_ulp_request_clears_the_indication_and_enforces() {
    let h = ulp::request(ulp::H2ME_ULP | 0x1);
    assert_eq!(h & ulp::H2ME_ULP, 0);
    assert_ne!(h & ulp::H2ME_ENFORCE_SETTINGS, 0);
    assert_eq!(h & 1, 1, "other bits survive");
    assert_eq!(ulp::release(h) & ulp::H2ME_ENFORCE_SETTINGS, 0);
    assert!(!ulp::done(ulp::FWSM_ULP_CFG_DONE));
    assert!(ulp::has_me(ulp::FWSM_FW_VALID));
}

#[test]
fn the_flush_runs_only_when_asked_and_the_ring_has_length() {
    assert!(flush::required(flush::REQUIRED, 256));
    assert!(!flush::required(flush::REQUIRED, 0));
    assert!(!flush::required(0, 256));
    assert_eq!(flush::tail_after(15, 16), 0);
    assert_eq!(flush::rxdctl(0xffff_ffff) & 0x3fff, 0x1f | 1 << 8);
    assert_ne!(flush::rxdctl(0) & 0x0100_0000, 0);
    // No end of packet, no report status: the dummy is never a frame anyone waits on.
    assert_eq!((flush::tx_word() >> 24) as u8 & (0x01 | 0x08), 0);
}

/// The flush's register values exactly as FreeBSD writes them: the dummy descriptor is 512
/// bytes with only IFCS (`txd_lower`), the tail moves one slot and wraps, and `RXDCTL` keeps
/// every bit above the two thresholds, including the granularity bit when it was already set.
#[test]
fn the_flush_values_are_freebsds() {
    assert_eq!(flush::tx_word(), 0x0200_0200);
    assert_eq!(flush::tail_after(3, 16), 4);
    assert_eq!(flush::tail_after(15, 16), 0);
    assert_eq!(flush::rxdctl(0xffff_ffff), 0xffff_c11f);
    assert_eq!(flush::rxdctl(0x0100_0000), 0x0100_011f);
    assert_eq!(flush::rxdctl(0), 0x0100_011f);
}

/// The ME handshake touches only its own two bits: the request sets `ENFORCE_SETTINGS` even
/// when it is already set, and the release clears it and nothing else.
#[test]
fn the_ulp_handshake_changes_only_its_own_bits() {
    assert_eq!(
        ulp::request(ulp::H2ME_ENFORCE_SETTINGS) & ulp::H2ME_ENFORCE_SETTINGS,
        ulp::H2ME_ENFORCE_SETTINGS
    );
    assert_eq!(ulp::release(u32::MAX), !ulp::H2ME_ENFORCE_SETTINGS);
    assert_eq!(ulp::release(0x1), 0x1);
}

/// A millisecond is a thousand microseconds of the device's clock, and a posted-write flush is
/// one read of `STATUS`, FreeBSD's `E1000_WRITE_FLUSH`.
#[test]
fn a_delay_is_in_milliseconds_and_a_flush_reads_status() {
    let mut s = sim::Sim::new(sim::Start::Healthy, true);
    delay_ms(&mut s, 3);
    assert_eq!(s.elapsed_us, 3000);
    flush_writes(&mut s);
    assert_eq!(s.reads, [crate::regs::STATUS]);
}
