//! **The root pointer and the root table**: the RSDP, found by scanning for a string, and the
//! RSDT or XSDT it points at, whose entries are every other table's physical address.
//!
//! Name: provisional (milestone 875's lane, 2026-10-12 UTC).

use super::*;

/// The eight bytes that begin an RSDP. Note the trailing space; it is part of the signature.
pub const RSDP_SIGNATURE: &[u8; 8] = b"RSD PTR ";

/// The length of the ACPI 1.0 RSDP, which is what its checksum covers.
pub(super) const RSDP_V1_LEN: usize = 20;
/// The length of the ACPI 2.0 RSDP, which carries a second checksum over the whole thing.
pub(super) const RSDP_V2_LEN: usize = 36;

/// The root pointer, decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rsdp {
    /// 0 for ACPI 1.0 (RSDT only), 2 or more for ACPI 2.0+ (XSDT present).
    pub revision: u8,
    /// The 32-bit root table's physical address. Always present.
    pub rsdt: u32,
    /// The 64-bit root table's physical address, or 0 on a revision-0 RSDP.
    pub xsdt: u64,
}

impl Rsdp {
    /// **Which root table to walk.** The XSDT when there is one, because a machine with memory
    /// above 4 GiB can have tables the RSDT's 32-bit pointers cannot name, and a firmware that
    /// publishes both is not required to list the same tables in each. Returns
    /// `(physical_address, entries_are_64_bit)`.
    pub const fn root_table(&self) -> (u64, bool) {
        if self.revision >= 2 && self.xsdt != 0 {
            (self.xsdt, true)
        } else {
            (self.rsdt as u64, false)
        }
    }
}

/// Decode an RSDP from bytes beginning at its signature.
///
/// Both checksums are checked when the revision says there are two. The first covers the first 20
/// bytes (which is the whole ACPI 1.0 structure, and is why the field order was never allowed to
/// change); the second covers all 36.
pub fn parse_rsdp(bytes: &[u8]) -> Result<Rsdp, AcpiError> {
    if bytes.len() < RSDP_V1_LEN {
        return Err(AcpiError::Truncated);
    }
    if &bytes[0..8] != RSDP_SIGNATURE {
        return Err(AcpiError::BadSignature);
    }
    if !is_checksum_ok(&bytes[..RSDP_V1_LEN]) {
        return Err(AcpiError::BadChecksum);
    }
    let revision = bytes[15];
    let rsdt = u32(bytes, 16);

    if revision < 2 {
        return Ok(Rsdp {
            revision,
            rsdt,
            xsdt: 0,
        });
    }

    if bytes.len() < RSDP_V2_LEN {
        return Err(AcpiError::Truncated);
    }
    // The extended structure's own length field, at offset 20. Trusted only as far as the second
    // checksum, which is computed over exactly what it claims.
    let length = u32(bytes, 20) as usize;
    if !(RSDP_V2_LEN..=bytes.len()).contains(&length) {
        return Err(AcpiError::BadLength(length as u32));
    }
    if !is_checksum_ok(&bytes[..length]) {
        return Err(AcpiError::BadChecksum);
    }
    Ok(Rsdp {
        revision,
        rsdt,
        xsdt: u64(bytes, 24),
    })
}

/// How many table pointers a root table of `length` bytes holds.
pub const fn root_entry_count(length: u32, entries_are_64_bit: bool) -> usize {
    let body = (length as usize).saturating_sub(SDT_HEADER_LEN);
    if entries_are_64_bit {
        body / 8
    } else {
        body / 4
    }
}

/// The physical address of root-table entry `index`. `body` begins **after** the SDT header.
///
/// Split from the header this way because the kernel reads the header first to learn the length,
/// then reads the body; handing this the whole table would mean the caller doing the same offset
/// arithmetic twice, in two places, with one chance to get it wrong.
pub fn root_entry(body: &[u8], index: usize, entries_are_64_bit: bool) -> Option<u64> {
    let width = if entries_are_64_bit { 8 } else { 4 };
    let at = index.checked_mul(width)?;
    if body.len() < at.checked_add(width)? {
        return None;
    }
    Some(if entries_are_64_bit {
        u64(body, at)
    } else {
        u32(body, at) as u64
    })
}

/// Machine-checked proofs over the ACPI tables, the x86 boot path's untrusted input (milestone
/// 319). The leaves and the two self-describing entry walks are what bounded model checking can
/// reach; the whole-table walk from the RSDP needs a symbolic pointer into memory this crate never
/// holds. Milestone 319 records the two defects these harnesses found.
///
/// Names: provisional (milestone 319 (the crate that parses firmware had no proofs)). An architect
/// names things.
#[cfg(kani)]
mod verification {
    use super::*;

    /// Long enough to hold a v2 RSDP, a table header, and a few entries of either walk. Every
    /// harness here is linear in this, so it is the whole cost knob.
    const N: usize = 36;

    /// **An RSDP is accepted only when its bytes sum to zero**, for every 36-byte input. The RSDP
    /// is found by scanning memory for a string, so the checksum is all that stands between a
    /// coincidence and an address the kernel follows. Checking over `bytes.len()` rather than
    /// `RSDP_V1_LEN`, or after the revision branch, passes every test and lets 20 bytes through.
    /// Falsification: replayable `crates/machine_discovery/falsifications/acpi.root.verification.an_rsdp_is_accepted_only_when_its_bytes_sum_to_zero.patch`
    #[kani::proof]
    fn an_rsdp_is_accepted_only_when_its_bytes_sum_to_zero() {
        let bytes: [u8; N] = kani::any();
        if let Ok(r) = parse_rsdp(&bytes) {
            assert!(is_checksum_ok(&bytes[..RSDP_V1_LEN]));
            if r.revision >= 2 {
                // The extended checksum covers the structure's own length field, which is the part
                // a caller cannot compute for itself.
                let length = u32(&bytes, 20) as usize;
                assert!((RSDP_V1_LEN..=N).contains(&length));
                assert!(is_checksum_ok(&bytes[..length]));
            }
            // Not vacuous: an accepted RSDP exists.
            kani::cover!(true, "some byte pattern is a valid RSDP");
        }
    }

    /// **The root table's entry count and its entry reader agree, exactly**, for every length and
    /// index at both pointer widths. The kernel bounds a loop over [`root_entry`] by
    /// [`root_entry_count`], so a disagreement at the last entry half-walks a table or ends it
    /// early. The `+ 1` that makes an inclusive count must be absent from exactly one of them.
    /// Falsification: replayable `crates/machine_discovery/falsifications/acpi.root.verification.the_root_tables_entry_count_and_its_entry_reader_agree.patch`
    #[kani::proof]
    fn the_root_tables_entry_count_and_its_entry_reader_agree() {
        let body: [u8; N] = kani::any();
        let body_len: usize = kani::any();
        kani::assume(body_len <= N);
        let body = &body[..body_len];
        let entries_are_64_bit: bool = kani::any();
        let index: usize = kani::any();

        let count = root_entry_count((body_len + SDT_HEADER_LEN) as u32, entries_are_64_bit);
        assert_eq!(
            root_entry(body, index, entries_are_64_bit).is_some(),
            index < count,
            "the count bounds the reader with nothing left over and nothing missing"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acpi::tests::{rsdp_v2, seal};

    /// **The checksum is the whole defence.** The RSDP is found by scanning memory for an
    /// eight-byte string, so without this any sixteen bytes spelling `RSD PTR ` would be believed
    /// and the kernel would follow a pointer out of somebody's data.
    #[test]
    fn a_string_that_spells_the_signature_but_does_not_checksum_is_refused() {
        let mut b = rsdp_v2();
        b[17] ^= 0xff; // corrupt the RSDT pointer without fixing the checksum
        assert_eq!(parse_rsdp(&b), Err(AcpiError::BadChecksum));
    }

    /// A revision-2 RSDP has a second checksum over all 36 bytes, and it is checked too.
    #[test]
    fn the_extended_checksum_is_checked_as_well_as_the_short_one() {
        let mut b = rsdp_v2();
        b[30] ^= 0xff; // inside the extended part only; the 20-byte checksum still passes
        assert!(
            is_checksum_ok(&b[..RSDP_V1_LEN]),
            "the short one still passes"
        );
        assert_eq!(parse_rsdp(&b), Err(AcpiError::BadChecksum));
    }

    /// A wrong signature is refused before anything else is read.
    #[test]
    fn a_wrong_signature_is_refused() {
        let mut b = rsdp_v2();
        b[7] = b'!';
        assert_eq!(parse_rsdp(&b), Err(AcpiError::BadSignature));
    }

    /// **The XSDT wins when there is one.** A machine with memory above 4 GiB can have tables the
    /// RSDT's 32-bit pointers cannot name, and firmware is not required to list the same tables in
    /// both.
    #[test]
    fn the_xsdt_is_preferred_and_its_entries_are_64_bit() {
        let r = parse_rsdp(&rsdp_v2()).expect("well-formed");
        assert_eq!(r.root_table(), (0x7ffe_1b00, true));
    }

    /// A revision-0 RSDP has no XSDT, so the RSDT is the only answer and its entries are 32-bit.
    /// The extended fields are not read at all, because on a 20-byte structure they do not exist.
    #[test]
    fn a_revision_0_rsdp_uses_the_rsdt_and_never_reads_the_extended_fields() {
        let mut b = rsdp_v2();
        b[15] = 0;
        // Leave garbage where the XSDT would be, to prove it is not read.
        b[24..32].copy_from_slice(&0xdead_beef_dead_beefu64.to_le_bytes());
        seal(&mut b, 8, RSDP_V1_LEN);
        let r = parse_rsdp(&b).expect("a 20-byte RSDP is still an RSDP");
        assert_eq!(r.xsdt, 0);
        assert_eq!(r.root_table(), (0x7ffe_1a40, false));
    }

    /// Root-table entries are 4 bytes in an RSDT and 8 in an XSDT, and running off the end is
    /// `None` rather than a garbage pointer.
    #[test]
    fn root_entries_are_four_or_eight_bytes_and_the_end_is_none() {
        let body32 = [0x40u8, 0x1a, 0xfe, 0x7f, 0x00, 0x1b, 0xfe, 0x7f];
        assert_eq!(root_entry(&body32, 0, false), Some(0x7ffe_1a40));
        assert_eq!(root_entry(&body32, 1, false), Some(0x7ffe_1b00));
        assert_eq!(root_entry(&body32, 2, false), None);
        assert_eq!(root_entry_count(SDT_HEADER_LEN as u32 + 8, false), 2);
        assert_eq!(root_entry_count(SDT_HEADER_LEN as u32 + 8, true), 1);
    }

    /// **A revision-2 RSDP whose XSDT pointer is zero still uses the RSDT.**
    ///
    /// Both halves of the choice have to hold: firmware that claims ACPI 2.0 and then writes no
    /// extended pointer has published exactly one root table, and following the zero would send
    /// the kernel to physical address 0. Every other fixture here has both fields set, where the
    /// revision alone decides and the second half of the condition is never asked.
    #[test]
    fn a_revision_2_rsdp_with_no_xsdt_falls_back_to_the_rsdt() {
        let mut b = rsdp_v2();
        b[24..32].copy_from_slice(&0u64.to_le_bytes());
        seal(&mut b, 32, RSDP_V2_LEN);
        let r = parse_rsdp(&b).expect("well-formed, just without an XSDT");
        assert_eq!(r.revision, 2);
        assert_eq!(r.xsdt, 0);
        assert_eq!(
            r.root_table(),
            (0x7ffe_1a40, false),
            "a null XSDT is not a root table, whatever the revision claims",
        );
    }
}
