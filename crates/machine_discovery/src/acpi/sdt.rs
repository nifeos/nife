//! **What every table shares**: the System Description Table header, the checksum, and
//! [`AcpiError`], the one error every parser here returns.
//!
//! Name: provisional (milestone 875's lane, 2026-10-12 UTC).

use super::*;

/// Every system descriptor table begins with this many bytes of common header.
pub const SDT_HEADER_LEN: usize = 36;

/// Why an ACPI structure could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpiError {
    /// The signature is not the one expected.
    BadSignature,
    /// The bytes end before the structure does.
    Truncated,
    /// The bytes sum to something other than zero. **The most important error here**, because the
    /// RSDP is found by scanning for a string and this is the only thing that separates a real one
    /// from a coincidence.
    BadChecksum,
    /// A length field says something impossible (shorter than its own header).
    BadLength(u32),
}

/// Do these bytes sum to zero in eight-bit arithmetic? That is ACPI's checksum for every structure
/// it defines.
pub fn is_checksum_ok(bytes: &[u8]) -> bool {
    bytes.iter().fold(0u8, |acc, b| acc.wrapping_add(*b)) == 0
}

/// The header every system descriptor table begins with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SdtHeader {
    /// Four ASCII characters: `APIC`, `MCFG`, `DMAR`, `FACP`.
    pub signature: [u8; 4],
    /// The table's total length **including** this header.
    pub length: u32,
    pub revision: u8,
    /// Six ASCII characters naming the firmware vendor. Read only for the boot print, and worth
    /// printing: it is the one place a firmware identifies itself before anything else runs.
    pub oem_id: [u8; 6],
}

impl SdtHeader {
    /// The signature as a string, for printing. `None` if it is not ASCII, which would mean the
    /// bytes are not a table.
    pub fn signature_str(&self) -> Option<&str> {
        core::str::from_utf8(&self.signature).ok()
    }

    /// How many bytes follow this header in the table.
    pub const fn body_len(&self) -> usize {
        self.length as usize - SDT_HEADER_LEN
    }
}

/// Decode a table header, without checking the checksum (which needs the whole table, and the
/// caller has to read `length` bytes before it can have them).
pub fn parse_sdt_header(bytes: &[u8]) -> Result<SdtHeader, AcpiError> {
    if bytes.len() < SDT_HEADER_LEN {
        return Err(AcpiError::Truncated);
    }
    let length = u32(bytes, 4);
    if (length as usize) < SDT_HEADER_LEN {
        return Err(AcpiError::BadLength(length));
    }
    Ok(SdtHeader {
        signature: [bytes[0], bytes[1], bytes[2], bytes[3]],
        length,
        revision: bytes[8],
        // Offset 10, not 9: byte 8 is the revision and byte 9 is the table's own checksum. Getting
        // this off by one reads the checksum as the first letter of the vendor's name, which prints
        // as garbage and is the kind of mistake that survives review because nothing depends on it.
        oem_id: [
            bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ],
    })
}

/// Machine-checked proofs over the table header (milestone 319). Why these tables carry harnesses is on
/// `acpi::root`'s `verification`.
///
/// Names: provisional (milestone 319 (the crate that parses firmware had no proofs)). An architect
/// names things.
#[cfg(kani)]
mod verification {
    use super::*;

    /// Long enough to hold a v2 RSDP, a table header, and a few entries of either walk. Every
    /// harness here is linear in this, so it is the whole cost knob.
    const N: usize = 36;

    /// **A header this parser accepts can always be asked how long its body is.**
    /// `SdtHeader::body_len` is `length - SDT_HEADER_LEN` with no guard of its own, so the guard in
    /// [`parse_sdt_header`] is the only thing standing between a table claiming `length = 10` and a
    /// `usize` underflow on the boot path. This proves the two are exactly matched: every header the
    /// parser returns has a body length, and it is the length less the header.
    ///
    /// Could plausibly have been false: `<=` in place of `<` is fine here, but `length > 0`, or
    /// dropping the check because "firmware would not do that", is the shape of a change a reader
    /// makes while tidying, and the crate's own tests only exercise `length = 10` and `length = 60`.
    /// Falsification: replayable `crates/machine_discovery/falsifications/acpi.sdt.verification.a_table_header_this_parser_accepts_always_has_a_body_length.patch`
    #[kani::proof]
    fn a_table_header_this_parser_accepts_always_has_a_body_length() {
        let bytes: [u8; N] = kani::any();
        if let Ok(h) = parse_sdt_header(&bytes) {
            assert_eq!(h.length as usize, SDT_HEADER_LEN + h.body_len());
            kani::cover!(h.body_len() > 0, "a table with a body is accepted");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acpi::tests::sdt;

    /// A table header's length must at least cover its own header, or the body length underflows.
    #[test]
    fn a_length_shorter_than_the_header_is_refused() {
        let mut t = sdt(b"APIC", &[]);
        t[4..8].copy_from_slice(&10u32.to_le_bytes());
        assert_eq!(parse_sdt_header(&t), Err(AcpiError::BadLength(10)));
    }

    /// The header's fields land where the specification puts them.
    #[test]
    fn a_table_header_decodes() {
        let t = sdt(b"MCFG", &[0u8; 24]);
        let h = parse_sdt_header(&t).expect("well-formed");
        assert_eq!(h.signature_str(), Some("MCFG"));
        assert_eq!(h.length as usize, SDT_HEADER_LEN + 24);
        assert_eq!(h.body_len(), 24);
        assert_eq!(
            &h.oem_id, b"BOCHS ",
            "the vendor name starts at offset 10, after the checksum byte"
        );
        assert!(
            is_checksum_ok(&t[..h.length as usize]),
            "a sealed table sums to zero"
        );
    }
}
