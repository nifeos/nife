//! The module's host tests.
//!
//! Name: ratified 2026-11-03 (UTC), #3145. Ruled in the group E1 ruling on e1000e's module names;
//! kept as it stands.

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
    let h = ultra_low_power::request(ultra_low_power::H2ME_ULP | 0x1);
    assert_eq!(h & ultra_low_power::H2ME_ULP, 0);
    assert_ne!(h & ultra_low_power::H2ME_ENFORCE_SETTINGS, 0);
    assert_eq!(h & 1, 1, "other bits survive");
    assert_eq!(
        ultra_low_power::release(h) & ultra_low_power::H2ME_ENFORCE_SETTINGS,
        0
    );
    assert!(!ultra_low_power::done(ultra_low_power::FWSM_ULP_CFG_DONE));
    assert!(ultra_low_power::has_management_engine(
        ultra_low_power::ICH_FWSM_FW_VALID
    ));
}

#[test]
fn the_flush_runs_only_when_asked_and_the_ring_has_length() {
    assert!(flush::required(flush::FLUSH_DESC_REQUIRED, 256));
    assert!(!flush::required(flush::FLUSH_DESC_REQUIRED, 0));
    assert!(!flush::required(0, 256));
    assert_eq!(flush::tail_after(15, 16), 0);
    assert_eq!(flush::rxdctl(0xffff_ffff) & 0x3fff, 0x1f | 1 << 8);
    assert_ne!(flush::rxdctl(0) & 0x0100_0000, 0);
    // No end of packet, no report status: the dummy is never a frame anyone waits on.
    assert_eq!((flush::transmit_word() >> 24) as u8 & (0x01 | 0x08), 0);
}

/// The flush's register values exactly as FreeBSD writes them: the dummy descriptor is 512
/// bytes with only IFCS (`txd_lower`), the tail moves one slot and wraps, and `RXDCTL` keeps
/// every bit above the two thresholds, including the granularity bit when it was already set.
#[test]
fn the_flush_values_are_freebsds() {
    assert_eq!(flush::transmit_word(), 0x0200_0200);
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
        ultra_low_power::request(ultra_low_power::H2ME_ENFORCE_SETTINGS)
            & ultra_low_power::H2ME_ENFORCE_SETTINGS,
        ultra_low_power::H2ME_ENFORCE_SETTINGS
    );
    assert_eq!(
        ultra_low_power::release(u32::MAX),
        !ultra_low_power::H2ME_ENFORCE_SETTINGS
    );
    assert_eq!(ultra_low_power::release(0x1), 0x1);
}

/// A millisecond is a thousand microseconds of the device's clock, and a posted-write flush is
/// one read of `STATUS`, FreeBSD's `E1000_WRITE_FLUSH`.
#[test]
fn a_delay_is_in_milliseconds_and_a_flush_reads_status() {
    let mut s = simulator::Sim::new(simulator::Start::Healthy, true);
    delay_ms(&mut s, 3);
    assert_eq!(s.elapsed_microseconds, 3000);
    flush_writes(&mut s);
    assert_eq!(s.reads, [crate::registers::STATUS]);
}

/// One step of the flush, as the recording fakes below saw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Read(u64),
    Write(u64, u32),
    Delay(u64),
    Status,
    Descriptor(u16, [u64; 2]),
}

type Log = core::cell::RefCell<std::vec::Vec<Step>>;

/// Registers that answer the flush's reads with fixed values and log every access.
struct Registers<'a> {
    values: std::collections::HashMap<u64, u32>,
    log: &'a Log,
}

/// A ring that answers status reads from a queue and logs every access to the same log.
struct Ring<'a> {
    statuses: std::vec::Vec<u16>,
    log: &'a Log,
}

impl Hardware for Registers<'_> {
    fn read(&mut self, offset: u64) -> u32 {
        self.log.borrow_mut().push(Step::Read(offset));
        self.values.get(&offset).copied().unwrap_or(0)
    }
    fn write(&mut self, offset: u64, value: u32) {
        self.log.borrow_mut().push(Step::Write(offset, value));
    }
    fn read16(&mut self, offset: u64) -> u16 {
        self.read(offset) as u16
    }
    fn delay_microseconds(&mut self, microseconds: u64) {
        self.log.borrow_mut().push(Step::Delay(microseconds));
    }
    fn pci_vendor_id(&mut self) -> u16 {
        0x8086
    }
    fn note(&mut self, _: sequence::Note) {}
}

impl flush::RingAccess for Ring<'_> {
    fn descriptor_ring_status(&mut self) -> u16 {
        self.log.borrow_mut().push(Step::Status);
        self.statuses.remove(0)
    }
    fn write_transmit_descriptor(&mut self, index: u16, words: [u64; 2]) {
        self.log.borrow_mut().push(Step::Descriptor(index, words));
    }
}

const RECEIVE_CONTROL: u32 = 0x0400_8002;

/// Runs the flush against a device whose transmit ring is at `device_ring` with its tail at
/// `tail`, whose status reads return `statuses` in turn, for a caller whose ring is at `ring`.
fn flush_steps(
    statuses: &[u16],
    device_ring: u64,
    tail: u32,
    ring: u64,
) -> (flush::Outcome, std::vec::Vec<Step>) {
    use crate::registers::{RCTL, TCTL, TDBAH, TDBAL, TDLEN, TDT};
    let log = Log::default();
    let values = [
        (registers::FEXTNVM11, 0x1),
        (TDLEN, 256),
        (TDBAL, device_ring as u32),
        (TDBAH, (device_ring >> 32) as u32),
        (TCTL, 0x8),
        (TDT, tail),
        (RCTL, RECEIVE_CONTROL),
        (registers::RXDCTL, 0x0001_0000),
    ]
    .into_iter()
    .collect();
    let mut hardware = Registers { values, log: &log };
    let mut rings = Ring {
        statuses: statuses.to_vec(),
        log: &log,
    };
    let outcome = flush::descriptor_rings(&mut hardware, &mut rings, ring);
    (outcome, log.into_inner())
}

const MULR_FIX_DISABLED: Step = Step::Write(
    registers::FEXTNVM11,
    0x1 | flush::FEXTNVM11_DISABLE_MULR_FIX,
);

/// Without the request bit, the flush only sets `DISABLE_MULR_FIX` and stops.
#[test]
fn the_flush_without_a_request_only_disables_the_mulr_fix() {
    let (outcome, steps) = flush_steps(&[0], 0x1000, 3, 0x1000);
    assert_eq!(outcome, flush::Outcome::NotRequired);
    assert_eq!(
        steps,
        [
            Step::Read(registers::FEXTNVM11),
            MULR_FIX_DISABLED,
            Step::Status,
            Step::Read(crate::registers::TDLEN),
        ]
    );
}

/// A transmit ring the caller does not own is never written, and neither is any register past
/// `FEXTNVM11`.
#[test]
fn the_flush_refuses_a_transmit_ring_that_is_not_the_callers() {
    let (outcome, steps) = flush_steps(&[flush::FLUSH_DESC_REQUIRED], 0x2_0000_1000, 3, 0x1000);
    assert_eq!(
        outcome,
        flush::Outcome::ForeignTransmitRing {
            base: 0x2_0000_1000
        }
    );
    let writes: std::vec::Vec<_> = steps
        .iter()
        .filter(|s| matches!(s, Step::Write(..) | Step::Descriptor(..)))
        .collect();
    assert_eq!(writes, [&MULR_FIX_DISABLED]);
}

/// The full flush in FreeBSD's order: transmit enabled, the dummy at the tail, the tail advanced,
/// 250 µs, then the receive ring disabled, rethresholded, pulsed on for 150 µs and disabled.
#[test]
fn the_flush_runs_both_rings_in_freebsds_order() {
    use crate::registers::{RCTL, STATUS, TCTL, TDBAH, TDBAL, TDLEN, TDT};
    let ring = 0x1_0000_1000;
    let required = flush::FLUSH_DESC_REQUIRED;
    let (outcome, steps) = flush_steps(&[required, required], ring, 15, ring);
    assert_eq!(outcome, flush::Outcome::BothFlushed);
    assert_eq!(
        steps,
        [
            Step::Read(registers::FEXTNVM11),
            MULR_FIX_DISABLED,
            Step::Status,
            Step::Read(TDLEN),
            Step::Read(TDBAL),
            Step::Read(TDBAH),
            Step::Read(TCTL),
            Step::Write(TCTL, 0x8 | 0x2),
            Step::Read(TDT),
            Step::Descriptor(15, [ring, flush::transmit_word()]),
            Step::Write(TDT, 0),
            Step::Delay(250),
            Step::Status,
            Step::Read(RCTL),
            Step::Write(RCTL, RECEIVE_CONTROL & !0x2),
            Step::Read(STATUS),
            Step::Delay(150),
            Step::Read(registers::RXDCTL),
            Step::Write(registers::RXDCTL, flush::rxdctl(0x0001_0000)),
            Step::Write(RCTL, RECEIVE_CONTROL | 0x2),
            Step::Read(STATUS),
            Step::Delay(150),
            Step::Write(RCTL, RECEIVE_CONTROL & !0x2),
        ]
    );
}

/// When the transmit flush clears the request, the receive ring is left alone.
#[test]
fn the_flush_stops_after_the_transmit_ring_when_the_request_clears() {
    let (outcome, steps) = flush_steps(&[flush::FLUSH_DESC_REQUIRED, 0], 0x1000, 0, 0x1000);
    assert_eq!(outcome, flush::Outcome::TransmitFlushed);
    assert!(
        !steps
            .iter()
            .any(|s| matches!(s, Step::Write(o, _) if *o == crate::registers::RCTL))
    );
    assert!(steps.contains(&Step::Descriptor(0, [0x1000, flush::transmit_word()])));
    assert!(steps.contains(&Step::Write(crate::registers::TDT, 1)));
}
