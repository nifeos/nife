//! **The one region the ACPI walk may read**, and the one checked read every firmware-supplied
//! address and length goes through (milestone 423 (a checked direct-map reader for the x86 ACPI walk), ruled in §192 (does the ACPI walk's direct-map
//! read take a bound, and is the bound per-read or a region it holds?)).
//!
//! The walk is handed a [`DirectMapRegion`] once, at boot, and holds it. [`DirectMapRegion::read`]
//! is the only way the walk turns a physical address and a length into bytes, so a number firmware
//! wrote cannot name memory outside the extent the kernel handed over. The table and root-pointer
//! readers below ([`DirectMapRegion::table`], [`DirectMapRegion::find_rsdp`]) are built on it and
//! sit here, beside the proof, rather than in the kernel, so a hostile table is a host test.
//!
//! The device-tree precedent is `DeviceTreeBlob::from_ptr` and `from_bytes`: one unsafe constructor
//! over a raw pointer ([`DirectMapRegion::from_ptr`]) and one safe constructor over a slice
//! ([`DirectMapRegion::from_bytes`]) that every host test and every proof uses.
//!
//! BUGS
//! - The region says which addresses are inside the extent. It cannot say they are memory rather
//!   than a device register block; the kernel chooses an extent that is memory.
//! - Nothing here reads the boot handoff or the command line; those walks are not ACPI's.
//!
//! Name: provisional, minted 2026-10-27 (UTC) by milestone 423's lane (a checked direct-map reader
//! for the x86 ACPI walk). §192 leaves the accessor's name to `script/names`; an architect names things.

use super::root::RSDP_V2_LEN;
use super::*;

/// The BIOS area the RSDP is required to be in when it is not in the EBDA: `0xe0000..0x100000`,
/// scanned on 16-byte boundaries.
///
/// Name: provisional, minted 2026-10-27 (UTC) by milestone 423's lane.
pub const BIOS_AREA: core::ops::Range<u64> = 0x000e_0000..0x0010_0000;

/// Where the BIOS Data Area records the Extended BIOS Data Area's segment. The EBDA's first
/// kilobyte is the other place an RSDP is allowed to be.
///
/// Name: provisional, minted 2026-10-27 (UTC) by milestone 423's lane.
pub const EBDA_SEGMENT_PTR: u64 = 0x0000_040e;

/// Why a read was refused.
///
/// Name: provisional, minted 2026-10-27 (UTC) by milestone 423's lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionError {
    /// The address was zero, which firmware uses for "no pointer".
    Null,
    /// Some byte of `at..at + len` lies outside the region, or the sum wraps.
    OutsideRegion,
}

/// Why a table was not returned.
///
/// Name: provisional, minted 2026-10-27 (UTC) by milestone 423's lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableRefusal {
    /// The header's own bytes lie outside the region. The boot map needs widening, which is not
    /// the same finding as a table to distrust.
    HeaderOutsideRegion,
    /// The header decoded, and the length it states runs past the region.
    LengthOutsideRegion(u32),
    /// The header did not decode.
    BadHeader(AcpiError),
    /// The whole table's bytes do not sum to zero.
    BadChecksum,
}

/// A table whose header decoded, whose stated length lies inside the region, and whose checksum
/// holds.
///
/// Name: provisional, minted 2026-10-27 (UTC) by milestone 423's lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Table<'a> {
    pub header: SdtHeader,
    /// The bytes after the header, `header.length - SDT_HEADER_LEN` of them.
    pub body: &'a [u8],
}

/// An extent of physical addresses the walk may read, and the bytes at them.
///
/// Holds `bytes`, which are the memory at physical `first..first + bytes.len()`. Nothing outside
/// that range is reachable through it, whatever a table says.
///
/// Name: provisional, minted 2026-10-27 (UTC) by milestone 423's lane.
#[derive(Debug, Clone, Copy)]
pub struct DirectMapRegion<'a> {
    first: u64,
    bytes: &'a [u8],
}

impl<'a> DirectMapRegion<'a> {
    /// A region over bytes already in hand, standing for physical `first..first + bytes.len()`.
    /// This is what every host test and proof uses.
    ///
    /// Name: provisional, minted 2026-10-27 (UTC) by milestone 423's lane.
    pub const fn from_bytes(first: u64, bytes: &'a [u8]) -> Self {
        Self { first, bytes }
    }

    /// A region over `len` bytes at `ptr`, standing for physical `first..first + len`.
    ///
    /// # Safety
    ///
    /// `ptr..ptr + len` must be readable for `'a` and hold the memory at physical `first`. The
    /// kernel passes the direct map's own address for physical `first` and the extent the boot
    /// page tables map, once, before the walk starts.
    ///
    /// Name: provisional, minted 2026-10-27 (UTC) by milestone 423's lane.
    pub const unsafe fn from_ptr(first: u64, ptr: *const u8, len: usize) -> Self {
        Self {
            first,
            // SAFETY: the caller's contract: `len` bytes at `ptr` are readable for `'a`.
            bytes: unsafe { core::slice::from_raw_parts(ptr, len) },
        }
    }

    /// **The checked read.** The `len` bytes at physical `at`, or a refusal when any of them lies
    /// outside the region. `at` and `len` are firmware's numbers and are never trusted: the sum is
    /// checked for wrap, and the offset into the region is computed before any slice is taken.
    ///
    /// Name: provisional, minted 2026-10-27 (UTC) by milestone 423's lane.
    pub fn read(&self, at: u64, len: u64) -> Result<&'a [u8], RegionError> {
        if at == 0 {
            return Err(RegionError::Null);
        }
        let end = at.checked_add(len).ok_or(RegionError::OutsideRegion)?;
        let offset = at
            .checked_sub(self.first)
            .ok_or(RegionError::OutsideRegion)?;
        let offset_end = end - self.first;
        if offset_end > self.bytes.len() as u64 {
            return Err(RegionError::OutsideRegion);
        }
        // Both offsets are at most `bytes.len()`, which is a `usize`, so the casts cannot truncate.
        Ok(&self.bytes[offset as usize..offset_end as usize])
    }

    /// **Read the table at physical `at`**, verifying its checksum over the whole table.
    ///
    /// The header is read first, and the length it states is a claim checked against the region
    /// before a byte past the header is touched. A corrupt MADT would hand out an APIC address and
    /// nothing downstream could notice, so a table that fails any check is refused rather than used.
    ///
    /// Name: provisional, minted 2026-10-27 (UTC) by milestone 423's lane.
    pub fn table(&self, at: u64) -> Result<Table<'a>, TableRefusal> {
        let header_bytes = self
            .read(at, SDT_HEADER_LEN as u64)
            .map_err(|_| TableRefusal::HeaderOutsideRegion)?;
        let header = parse_sdt_header(header_bytes).map_err(TableRefusal::BadHeader)?;
        let whole = self
            .read(at, header.length as u64)
            .map_err(|_| TableRefusal::LengthOutsideRegion(header.length))?;
        if !is_checksum_ok(whole) {
            return Err(TableRefusal::BadChecksum);
        }
        Ok(Table {
            header,
            body: &whole[SDT_HEADER_LEN..],
        })
    }

    /// Find the ACPI root pointer: where the loader said (PVH has a field, which QEMU leaves
    /// zero), then the first kilobyte of the EBDA, then the BIOS area. The last two scan for an
    /// eight-byte string, so [`parse_rsdp`]'s checksum is all that separates a hit from chance.
    /// Returns the RSDP's physical address and its decoding.
    ///
    /// Name: provisional, minted 2026-10-27 (UTC) by milestone 423's lane.
    pub fn find_rsdp(&self, hint: u64) -> Option<(u64, Rsdp)> {
        if let Some(rsdp) = self.rsdp_at(hint) {
            return Some((hint, rsdp));
        }
        // A SEGMENT, not an address: the BDA holds it in real-mode form, so it is shifted left four
        // to become the physical address. Reading it as an address directly would land in the first
        // 64 KiB and find nothing, quietly.
        if let Ok(bda) = self.read(EBDA_SEGMENT_PTR, 2) {
            let ebda = (u16::from_le_bytes([bda[0], bda[1]]) as u64) << 4;
            if ebda >= 0x400
                && let Some(found) = self.scan_for_rsdp(ebda..ebda + 0x400)
            {
                return Some(found);
            }
        }
        self.scan_for_rsdp(BIOS_AREA)
    }

    /// Scan `range` on 16-byte boundaries for a well-formed RSDP.
    fn scan_for_rsdp(&self, range: core::ops::Range<u64>) -> Option<(u64, Rsdp)> {
        let mut at = range.start & !0xf;
        while at + RSDP_V2_LEN as u64 <= range.end {
            if let Some(rsdp) = self.rsdp_at(at) {
                return Some((at, rsdp));
            }
            at += 16;
        }
        None
    }

    fn rsdp_at(&self, at: u64) -> Option<Rsdp> {
        parse_rsdp(self.read(at, RSDP_V2_LEN as u64).ok()?).ok()
    }
}

#[cfg(kani)]
mod verification;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acpi::tests::{rsdp_v2, sdt};

    const BASE: u64 = 0x1000;

    /// One kilobyte of zeroed memory, standing for physical `BASE..BASE + 0x400`.
    fn memory() -> [u8; 0x400] {
        [0u8; 0x400]
    }

    /// A read of exactly the whole region is accepted, and one byte more is refused: the boundary
    /// that no malformed input can expose.
    #[test]
    fn a_read_is_checked_against_both_ends_of_the_region() {
        let bytes = memory();
        let region = DirectMapRegion::from_bytes(BASE, &bytes);
        assert_eq!(region.read(BASE, 0x400).map(<[u8]>::len), Ok(0x400));
        assert_eq!(region.read(BASE, 0x401), Err(RegionError::OutsideRegion));
        assert_eq!(region.read(BASE + 0x3ff, 1).map(<[u8]>::len), Ok(1));
        assert_eq!(
            region.read(BASE + 0x400, 1),
            Err(RegionError::OutsideRegion)
        );
        assert_eq!(region.read(BASE - 1, 1), Err(RegionError::OutsideRegion));
        assert_eq!(region.read(0, 1), Err(RegionError::Null));
    }

    /// A length near `u64::MAX` wraps a plain addition to a small number that looks inside.
    #[test]
    fn a_length_that_wraps_the_address_is_refused() {
        let bytes = memory();
        let region = DirectMapRegion::from_bytes(BASE, &bytes);
        assert_eq!(
            region.read(BASE + 8, u64::MAX - 4),
            Err(RegionError::OutsideRegion)
        );
        assert_eq!(region.read(u64::MAX, 2), Err(RegionError::OutsideRegion));
    }

    /// **The hostile root table.** An RSDT whose `length` field says 64 KiB, in a region of 1 KiB,
    /// is refused and its body is never read. This is the case §192 exists for.
    #[test]
    fn an_rsdt_whose_length_runs_past_the_region_is_refused() {
        let mut bytes = memory();
        let rsdt = sdt(b"RSDT", &[0u8; 8]);
        bytes[0..rsdt.len()].copy_from_slice(&rsdt);
        bytes[4..8].copy_from_slice(&0x1_0000u32.to_le_bytes());
        let region = DirectMapRegion::from_bytes(BASE, &bytes);
        assert_eq!(
            region.table(BASE),
            Err(TableRefusal::LengthOutsideRegion(0x1_0000))
        );
    }

    /// A table whose header straddles the end of the region is refused as an unreadable header.
    #[test]
    fn a_header_that_straddles_the_end_of_the_region_is_refused() {
        let bytes = memory();
        let region = DirectMapRegion::from_bytes(BASE, &bytes);
        assert_eq!(
            region.table(BASE + 0x400 - 10),
            Err(TableRefusal::HeaderOutsideRegion)
        );
        assert_eq!(region.table(0), Err(TableRefusal::HeaderOutsideRegion));
    }

    /// A root-table entry that points outside the region is refused, however well-formed the root.
    #[test]
    fn an_entry_that_points_outside_the_region_is_refused() {
        let mut bytes = memory();
        let rsdt = sdt(b"RSDT", &0x7ffe_0000u32.to_le_bytes());
        bytes[..rsdt.len()].copy_from_slice(&rsdt);
        let region = DirectMapRegion::from_bytes(BASE, &bytes);
        let root = region.table(BASE).expect("the root table itself is inside");
        let entry = root_entry(root.body, 0, false).expect("one entry");
        assert_eq!(region.table(entry), Err(TableRefusal::HeaderOutsideRegion));
    }

    /// A header shorter than itself is refused by the header decoder, not read through.
    #[test]
    fn a_length_shorter_than_the_header_is_refused_as_a_bad_header() {
        let mut bytes = memory();
        let t = sdt(b"APIC", &[]);
        bytes[..t.len()].copy_from_slice(&t);
        bytes[4..8].copy_from_slice(&10u32.to_le_bytes());
        let region = DirectMapRegion::from_bytes(BASE, &bytes);
        assert_eq!(
            region.table(BASE),
            Err(TableRefusal::BadHeader(AcpiError::BadLength(10)))
        );
    }

    /// A well-formed table inside the region is returned, with its body, and a corrupt one is not.
    #[test]
    fn a_well_formed_table_is_returned_and_a_corrupt_one_is_not() {
        let mut bytes = memory();
        let t = sdt(b"MCFG", &[7u8; 16]);
        let len = SDT_HEADER_LEN + 16;
        bytes[0x40..0x40 + len].copy_from_slice(&t[..len]);
        let region = DirectMapRegion::from_bytes(BASE, &bytes);
        let table = region.table(BASE + 0x40).expect("well-formed");
        assert_eq!(table.header.signature_str(), Some("MCFG"));
        assert_eq!(table.body, &[7u8; 16]);

        let mut corrupt = bytes;
        corrupt[0x40 + SDT_HEADER_LEN] ^= 1;
        let region = DirectMapRegion::from_bytes(BASE, &corrupt);
        assert_eq!(region.table(BASE + 0x40), Err(TableRefusal::BadChecksum));
    }

    /// The RSDP is found at the loader's hint, and by scanning the BIOS area when the hint is zero,
    /// outside the region, or not an RSDP.
    #[test]
    fn the_root_pointer_is_found_by_hint_and_by_scan() {
        let mut bytes = [0u8; 0x2_0000];
        let rsdp = rsdp_v2();
        bytes[0x1230..0x1230 + 36].copy_from_slice(&rsdp);
        let region = DirectMapRegion::from_bytes(0xe_0000, &bytes);
        let hinted = region.find_rsdp(0xe_1230).expect("the hint names an RSDP");
        assert_eq!(hinted.0, 0xe_1230);
        for hint in [0, 0x10, 0xe_1000, u64::MAX] {
            let scanned = region.find_rsdp(hint).expect("the scan finds it");
            assert_eq!(scanned.0, 0xe_1230);
            assert_eq!(scanned.1, hinted.1);
        }
        let empty = [0u8; 0x2_0000];
        let region = DirectMapRegion::from_bytes(0xe_0000, &empty);
        assert_eq!(region.find_rsdp(0xe_0000), None);
    }

    /// An RSDP that ends one byte past the region is not read, so the scan never reaches past it.
    #[test]
    fn a_root_pointer_that_ends_past_the_region_is_not_read() {
        let mut bytes = [0u8; 0x100];
        let rsdp = rsdp_v2();
        bytes[0xe0..0x100].copy_from_slice(&rsdp[..0x20]);
        let region = DirectMapRegion::from_bytes(0xe_0000, &bytes);
        assert_eq!(region.find_rsdp(0xe_00e0), None);
    }

    /// Memory from physical 0, with the BDA's EBDA segment set to `segment`.
    fn low_memory(segment: u16) -> [u8; 0x1000] {
        let mut bytes = [0u8; 0x1000];
        bytes[0x40e..0x410].copy_from_slice(&segment.to_le_bytes());
        bytes
    }

    /// The EBDA's first kilobyte is searched, at the address the BDA's segment names: the segment is
    /// shifted left four, and the search covers `ebda..ebda + 0x400` and no more.
    #[test]
    fn the_extended_bios_data_area_is_searched_where_the_segment_says() {
        let rsdp = rsdp_v2();
        let mut bytes = low_memory(0x0080);
        bytes[0x840..0x840 + 36].copy_from_slice(&rsdp);
        let region = DirectMapRegion::from_bytes(0, &bytes);
        let found = region.find_rsdp(0).expect("inside the EBDA's kilobyte");
        assert_eq!(found.0, 0x840);

        // The last 16-byte slot that still fits 36 bytes before the kilobyte ends.
        let mut bytes = low_memory(0x0080);
        bytes[0xbd0..0xbd0 + 36].copy_from_slice(&rsdp);
        let region = DirectMapRegion::from_bytes(0, &bytes);
        assert_eq!(region.find_rsdp(0).map(|f| f.0), Some(0xbd0));

        // Just past the kilobyte, and below it, nothing is searched (the BIOS area is outside this
        // region, so a hit there would be a read the region refuses).
        for at in [0xbf0usize, 0xc00, 0x7f0] {
            let mut bytes = low_memory(0x0080);
            bytes[at..at + 36].copy_from_slice(&rsdp);
            let region = DirectMapRegion::from_bytes(0, &bytes);
            assert_eq!(region.find_rsdp(0), None, "at {at:#x}");
        }
    }

    /// A segment below 0x40 would put the EBDA inside the interrupt vector table and the BDA, so it
    /// is not searched.
    #[test]
    fn a_segment_that_names_the_vector_table_is_not_searched() {
        let rsdp = rsdp_v2();
        let mut bytes = low_memory(0x0030);
        bytes[0x300..0x300 + 36].copy_from_slice(&rsdp);
        let region = DirectMapRegion::from_bytes(0, &bytes);
        assert_eq!(region.find_rsdp(0), None);
        // And the segment is not read as an address: 0x0080 as an address finds nothing here.
        let mut bytes = low_memory(0x0040);
        bytes[0x80..0x80 + 36].copy_from_slice(&rsdp);
        let region = DirectMapRegion::from_bytes(0, &bytes);
        assert_eq!(region.find_rsdp(0), None);
    }

    /// The scan starts on a 16-byte boundary at or below its start and stops before a structure
    /// that would run past the end of the range, even when the region goes on.
    #[test]
    fn the_scan_aligns_its_start_and_respects_the_end_of_its_range() {
        let rsdp = rsdp_v2();
        let mut bytes = [0u8; 0x1000];
        bytes[0x800..0x800 + 36].copy_from_slice(&rsdp);
        let region = DirectMapRegion::from_bytes(0, &bytes);
        assert_eq!(region.scan_for_rsdp(0x801..0xc00).map(|f| f.0), Some(0x800));

        let mut bytes = [0u8; 0x1000];
        bytes[0xbf0..0xbf0 + 36].copy_from_slice(&rsdp);
        let region = DirectMapRegion::from_bytes(0, &bytes);
        assert_eq!(region.scan_for_rsdp(0x800..0xc00), None);
        assert_eq!(region.scan_for_rsdp(0x800..0xc14).map(|f| f.0), Some(0xbf0));
    }
}
