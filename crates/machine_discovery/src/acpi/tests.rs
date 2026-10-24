//! Test fixtures more than one table's tests use, and the one test that reads four tables.
//!
//! Name: provisional (milestone 875's lane, 2026-10-12 UTC).

use super::dmar::{DMAR_FIXED_LEN, DRHD_FIXED_LEN};
use super::madt::MADT_FIXED_LEN;
use super::root::{RSDP_V1_LEN, RSDP_V2_LEN};
use super::*;

/// Set the checksum byte at `at` so `bytes[..len]` sums to zero, the way firmware does when it
/// writes a table.
pub(super) fn seal(bytes: &mut [u8], at: usize, len: usize) {
    bytes[at] = 0;
    let sum = bytes[..len].iter().fold(0u8, |a, b| a.wrapping_add(*b));
    bytes[at] = sum.wrapping_neg();
}

pub(super) fn rsdp_v2() -> [u8; RSDP_V2_LEN] {
    let mut b = [0u8; RSDP_V2_LEN];
    b[0..8].copy_from_slice(RSDP_SIGNATURE);
    b[9..15].copy_from_slice(b"BOCHS ");
    b[15] = 2; // revision
    b[16..20].copy_from_slice(&0x7ffe_1a40u32.to_le_bytes()); // rsdt
    b[20..24].copy_from_slice(&(RSDP_V2_LEN as u32).to_le_bytes());
    b[24..32].copy_from_slice(&0x7ffe_1b00u64.to_le_bytes()); // xsdt
    seal(&mut b, 8, RSDP_V1_LEN);
    seal(&mut b, 32, RSDP_V2_LEN);
    b
}

/// Build a well-formed table: signature, length, revision, a sealed checksum at offset 9, and
/// a vendor name at offset 10.
pub(super) fn sdt(signature: &[u8; 4], body: &[u8]) -> [u8; 128] {
    let mut t = [0u8; 128];
    let len = SDT_HEADER_LEN + body.len();
    assert!(len <= 128);
    t[0..4].copy_from_slice(signature);
    t[4..8].copy_from_slice(&(len as u32).to_le_bytes());
    t[8] = 1;
    t[10..16].copy_from_slice(b"BOCHS ");
    t[SDT_HEADER_LEN..len].copy_from_slice(body);
    seal(&mut t, 9, len);
    t
}

/// **Every prefix shorter than a structure's fixed part is refused, and the shortest one that
/// is long enough is accepted.** One loop per parser, over every length from nothing to one
/// byte short.
///
/// Both halves are the property, and the second is the one an "it returns an error" test
/// leaves out. A guard written `<=` rather than `<` refuses a structure that is exactly long
/// enough, and no malformed input can expose that: only the boundary length can. These four
/// parsers read firmware bytes on the x86 boot path, where a guard one byte too loose reads
/// past the buffer and a guard one byte too tight loses a table the machine really has.
pub(super) fn every_short_prefix_is_refused<T: core::fmt::Debug + PartialEq>(
    bytes: &[u8],
    fixed_len: usize,
    parse: impl Fn(&[u8]) -> Result<T, AcpiError>,
) {
    for len in 0..fixed_len {
        assert_eq!(
            parse(&bytes[..len]),
            Err(AcpiError::Truncated),
            "{len} bytes is short of the {fixed_len} this structure needs",
        );
    }
    assert!(
        parse(&bytes[..fixed_len]).is_ok(),
        "{fixed_len} bytes is exactly enough and must not be refused",
    );
}

/// A revision-0 RSDP, which is the whole 20-byte structure and has no extended half.
pub(super) fn rsdp_v1() -> [u8; RSDP_V1_LEN] {
    let mut b = [0u8; RSDP_V1_LEN];
    b[0..8].copy_from_slice(RSDP_SIGNATURE);
    b[9..15].copy_from_slice(b"BOCHS ");
    b[15] = 0; // revision 0: RSDT only
    b[16..20].copy_from_slice(&0x7ffe_1a40u32.to_le_bytes());
    seal(&mut b, 8, RSDP_V1_LEN);
    b
}

#[test]
fn no_parser_reads_past_a_table_that_ends_early() {
    every_short_prefix_is_refused(&rsdp_v1(), RSDP_V1_LEN, parse_rsdp);
    every_short_prefix_is_refused(&rsdp_v2(), RSDP_V2_LEN, parse_rsdp);
    // An empty body, so the header's own length field is exactly the header length: the case
    // that separates `length < SDT_HEADER_LEN` from `length <= SDT_HEADER_LEN`.
    every_short_prefix_is_refused(&sdt(b"APIC", &[]), SDT_HEADER_LEN, parse_sdt_header);
    every_short_prefix_is_refused(&q35_madt_body(), MADT_FIXED_LEN, parse_madt);
    every_short_prefix_is_refused(&q35_dmar_body(), DMAR_FIXED_LEN, parse_dmar);
}

/// The MADT QEMU's `q35` produces for one CPU: a processor, an IO APIC, and the two interrupt
/// source overrides every PC has.
pub(super) fn q35_madt_body() -> [u8; 8 + 8 + 12 + 10 + 10] {
    let mut b = [0u8; 8 + 8 + 12 + 10 + 10];
    b[0..4].copy_from_slice(&0xfee0_0000u32.to_le_bytes());
    b[4..8].copy_from_slice(&MADT_PCAT_COMPAT.to_le_bytes());
    // Local APIC: processor 0, apic id 0, enabled.
    b[8] = 0;
    b[9] = 8;
    b[10] = 0;
    b[11] = 0;
    b[12..16].copy_from_slice(&1u32.to_le_bytes());
    // IO APIC: id 0 at 0xfec00000, gsi base 0.
    b[16] = 1;
    b[17] = 12;
    b[18] = 0;
    b[20..24].copy_from_slice(&0xfec0_0000u32.to_le_bytes());
    b[24..28].copy_from_slice(&0u32.to_le_bytes());
    // Interrupt source override: ISA IRQ 0 arrives as GSI 2.
    b[28] = 2;
    b[29] = 10;
    b[30] = 0; // bus 0 = ISA
    b[31] = 0; // source: IRQ 0
    b[32..36].copy_from_slice(&2u32.to_le_bytes());
    b[36..38].copy_from_slice(&0u16.to_le_bytes());
    // Interrupt source override: ISA IRQ 5, active high, level triggered.
    b[38] = 2;
    b[39] = 10;
    b[40] = 0;
    b[41] = 5;
    b[42..46].copy_from_slice(&5u32.to_le_bytes());
    b[46..48].copy_from_slice(&0x000du16.to_le_bytes());
    b
}

/// The DMAR `-device intel-iommu,intremap=off` produces on `q35`: 39-bit host address width,
/// `INTR_REMAP` clear (QEMU's default, `intremap=auto`, sets it under TCG), one DRHD (not the catch-all, QEMU names devices explicitly) at the address
/// `Q35_HOST_BRIDGE_IOMMU_ADDR` names, `0xfed90000`.
pub(super) fn q35_dmar_body() -> [u8; DMAR_FIXED_LEN + DRHD_FIXED_LEN] {
    let mut b = [0u8; DMAR_FIXED_LEN + DRHD_FIXED_LEN];
    b[0] = 38; // host address width - 1: 39-bit
    b[1] = 0; // flags: no interrupt remapping
    // reserved[2..12] stays zero
    let d = DMAR_FIXED_LEN;
    b[d..d + 2].copy_from_slice(&0u16.to_le_bytes()); // type 0: DRHD
    b[d + 2..d + 4].copy_from_slice(&(DRHD_FIXED_LEN as u16).to_le_bytes());
    b[d + 4] = 0; // flags: not INCLUDE_PCI_ALL
    b[d + 5] = 0; // reserved
    b[d + 6..d + 8].copy_from_slice(&0u16.to_le_bytes()); // segment 0
    b[d + 8..d + 16].copy_from_slice(&0xfed9_0000u64.to_le_bytes());
    b
}
