//! The IVRS: where AMD-Vi is, the AMD counterpart of the DMAR (provisional module name, lane
//! `amd-vi`). Every field is from AMD document 48882 revision 2.62, chapter 5.
//!
//! An IVHD describes one IOMMU and the device ids it serves; an IVMD, memory a device keeps
//! DMA-ing into. Firmware lists one IOMMU once per IVHD type it supports, so
//! [`IvrsUnits::parse`] decodes only the highest type it understands, as Linux does. Ownership is by
//! 16-bit device id, so [`IvrsUnits::owner_index`] needs no bus. See `notes/amd-vi.md` and
//! `notes/machine-discovery.md` ("ACPI").

//! # BUGS
//!
//! - ACPI HID device entries (type F0h) are skipped: they name devices with no PCI identity, and
//!   this kernel drives none.
//! - A write-only IVMD is mapped readable too, since no DMA page format says "write, not read"
//!   (milestone 767 (AMD-Vi hardening)). No machine met so far publishes an IVMD.
//! - An exclusion-range IVMD becomes a read-write identity map, not the unit's exclusion registers.
//! - Only segment 0 is described, as revision 2.62 requires; a unit on another is refused.

use super::{u16, u32, u64};
use crate::acpi::Ownership;

/// The most IVHDs (of the chosen type) recorded. A client AMD part has one IOMMU; a two-socket
/// server can have eight. Name: provisional.
pub const MAX_IVHDS: usize = 8;
/// The most device-id ranges recorded across every unit, a select entry counting as a range of
/// one. QEMU's `q35` with every device this tree's runner attaches writes about a dozen. Name:
/// provisional.
pub const MAX_IVHD_RANGES: usize = 64;
/// The most IVMDs recorded. Name: provisional.
pub const MAX_IVMDS: usize = 8;

/// Four bytes of `IVinfo`, eight reserved.
const IVRS_FIXED_LEN: usize = 12;
/// The fixed part of a type 10h IVHD (Table 80): type, flags, length, device id, capability
/// offset, register base, segment, IOMMU info, feature reporting.
const IVHD_10_HEADER_LEN: usize = 24;
/// Types 11h and 40h add the EFR image and eight reserved bytes (Tables 85 and 89).
const IVHD_11_HEADER_LEN: usize = 40;
/// An IVMD is fixed at 32 bytes (Table 102).
const IVMD_LEN: usize = 32;

/// **One IVHD: one AMD IOMMU.** Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ivhd {
    /// The block type this record came from: 10h, 11h or 40h.
    pub kind: u8,
    /// The IVHD flags byte (Table 82): firmware's recommended control-register settings.
    pub flags: u8,
    /// The IOMMU's own PCI device id. The unit is a PCI function as well as an MMIO register file.
    pub device_id: u16,
    /// The PCI segment group this unit and every device it serves are on.
    pub segment: u16,
    /// The physical address of the unit's MMIO register file.
    pub register_base: u64,
    /// The Extended Feature Register image firmware copied into a type 11h or 40h block, `None`
    /// for type 10h. The kernel reads the live register itself; this is what the boot print shows.
    pub efr_image: Option<u64>,
}

/// **The device ids one unit serves, as one entry of an IVHD described them.** A select entry is a
/// range of one; an "all" entry is `0..=0xffff` with `all` set. Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeviceRange {
    /// Index into [`IvrsUnits::units`].
    pub unit: u8,
    pub first: u16,
    /// Inclusive, as the table writes ranges.
    pub last: u16,
    /// **The device id these devices' transactions actually carry**, for an alias entry (types
    /// 42h and 43h, or a special device's source id): a device behind a PCIe-to-PCI bridge is seen
    /// by the IOMMU as the bridge. `None` when each device carries its own.
    pub alias: Option<u16>,
    /// Set for the type-1 "all" entry, which [`IvrsUnits::owner_index`] consults only after every
    /// explicit entry, the same order a DMAR's catch-all takes.
    pub all: bool,
}

impl DeviceRange {
    const fn covers(&self, id: u16) -> bool {
        self.first <= id && id <= self.last
    }
}

/// **One IVMD: memory a device keeps DMA-ing into.** AMD's counterpart of an RMRR (section
/// 5.2.2.2). `first..=last` are the device ids it applies to, all of them for type 20h. `base` is
/// rounded down and `size` up to 4 KiB, as the table allows ("system software may round up to
/// 4-Kbyte boundary"). Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnityRegion {
    pub first: u16,
    pub last: u16,
    pub base: u64,
    pub size: u64,
    pub read: bool,
    pub write: bool,
    /// The IVMD's `ExclusionRange` flag. See this module's BUGS.
    pub exclusion: bool,
}

impl UnityRegion {
    /// **Whether the device may write this region through its domain** (provisional milestone 767
    /// (AMD-Vi hardening before the first AMD boot); name provisional). `IW` (flags bit 2, section
    /// 5.2.2.2, Table 103) says so, and so does `ExclusionRange` (bit 3): an exclusion range is
    /// memory the device reaches untranslated on a unit that honours it, so this kernel, which never
    /// programs the exclusion registers, maps it read-write as the nearest thing (this module's BUGS).
    /// An IVMD with `IR` alone is read-only and stays so. Read is not a separate answer: a region
    /// with `IR` = `IW` = 0 and no exclusion flag is never recorded, and the domain format grants
    /// read with every present page.
    pub const fn device_may_write(&self) -> bool {
        self.write || self.exclusion
    }
}

/// **Every AMD IOMMU, every device-id range and every IVMD in one IVRS**, decoded into fixed
/// arrays so the kernel can keep it after the firmware's bytes are out of reach. `Copy` and free
/// of lifetimes for the reason [`DmarUnits`](super::DmarUnits) is. Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IvrsUnits {
    /// The `IVinfo` word (Table 78): `EFRSup` in bit 0, the physical address size in bits 14:8.
    pub ivinfo: u32,
    /// The IVHD type decoded: the highest of 10h, 11h and 40h the table carries. Zero when there
    /// is no IVHD.
    pub ivhd_type: u8,
    pub ivhds: [Ivhd; MAX_IVHDS],
    pub ivhd_count: usize,
    pub ranges: [DeviceRange; MAX_IVHD_RANGES],
    pub range_count: usize,
    pub unity: [UnityRegion; MAX_IVMDS],
    pub unity_count: usize,
    /// IVMDs refused as malformed (zero length, or a range that wraps), counted so the boot print
    /// can say so.
    pub unity_refused: usize,
    /// Something did not fit, or an entry could not be decoded. **When set, a device no recorded
    /// entry names is answered "unknown" rather than "unowned"**, because the entry that names it
    /// may be the one that was dropped. The same rule the DMAR decoder follows.
    pub truncated: bool,
}

impl Default for IvrsUnits {
    fn default() -> Self {
        Self {
            ivinfo: 0,
            ivhd_type: 0,
            ivhds: [Ivhd::default(); MAX_IVHDS],
            ivhd_count: 0,
            ranges: [DeviceRange::default(); MAX_IVHD_RANGES],
            range_count: 0,
            unity: [UnityRegion::default(); MAX_IVMDS],
            unity_count: 0,
            unity_refused: 0,
            truncated: false,
        }
    }
}

/// Every IVDB in `body` past the fixed part, as `(type, bytes)`. Stops at the first block whose
/// length is too short to be a block or runs past the body, which is total for any input.
fn blocks(body: &[u8]) -> impl Iterator<Item = (u8, &[u8])> {
    let mut at = IVRS_FIXED_LEN;
    core::iter::from_fn(move || {
        if at + 4 > body.len() {
            return None;
        }
        let kind = body[at];
        let len = u16(body, at + 2) as usize;
        if len < 4 || at + len > body.len() {
            return None;
        }
        let b = &body[at..at + len];
        at += len;
        Some((kind, b))
    })
}

const fn is_ivhd(kind: u8) -> bool {
    matches!(kind, 0x10 | 0x11 | 0x40)
}

impl IvrsUnits {
    /// Decode the IVRS. `body` begins after the SDT header. Total for any input: a block that does
    /// not fit stops the walk, and an entry this decoder cannot size sets
    /// [`IvrsUnits::truncated`].
    pub fn parse(body: &[u8]) -> IvrsUnits {
        let mut u = IvrsUnits::default();
        if body.len() < IVRS_FIXED_LEN {
            return u;
        }
        u.ivinfo = u32(body, 0);
        u.ivhd_type = blocks(body)
            .map(|(k, _)| k)
            .filter(|&k| is_ivhd(k))
            .max()
            .unwrap_or(0);

        for (kind, b) in blocks(body) {
            if is_ivhd(kind) && kind == u.ivhd_type {
                u.read_ivhd(kind, b);
            } else if matches!(kind, 0x20..=0x22) {
                u.read_ivmd(kind, b);
            }
        }
        u
    }

    fn read_ivhd(&mut self, kind: u8, b: &[u8]) {
        let header = if kind == 0x10 {
            IVHD_10_HEADER_LEN
        } else {
            IVHD_11_HEADER_LEN
        };
        if b.len() < header {
            self.truncated = true;
            return;
        }
        if self.ivhd_count == MAX_IVHDS {
            self.truncated = true;
            return;
        }
        let unit = self.ivhd_count;
        self.ivhds[unit] = Ivhd {
            kind,
            flags: b[1],
            device_id: u16(b, 4),
            segment: u16(b, 16),
            register_base: u64(b, 8),
            efr_image: (kind != 0x10).then(|| u64(b, 24)),
        };
        self.ivhd_count += 1;
        self.read_device_entries(unit as u8, &b[header..]);
    }

    /// Walk one IVHD's device entries (Tables 93 to 100). A start-of-range entry (3h, 43h or 47h)
    /// is held until the end-of-range (4h) that must follow it.
    fn read_device_entries(&mut self, unit: u8, mut e: &[u8]) {
        // The pending start of a range, and the alias its devices carry if it was a 43h.
        let mut start: Option<(u16, Option<u16>)> = None;
        while !e.is_empty() {
            let kind = e[0];
            let len = match kind {
                0x00..=0x3f => 4,
                0x40..=0x7f => 8,
                // The one variable-length entry this revision defines: 22 bytes, plus the UID
                // whose length byte is at offset 21 (Table 100).
                0xf0 if e.len() >= 22 => 22 + e[21] as usize,
                _ => {
                    self.truncated = true;
                    return;
                }
            };
            if len > e.len() {
                self.truncated = true;
                return;
            }
            let entry = &e[..len];
            e = &e[len..];
            let id = u16(entry, 1);
            match kind {
                // "All": every device id this unit controls.
                0x01 => self.push(DeviceRange {
                    unit,
                    first: 0,
                    last: 0xffff,
                    alias: None,
                    all: true,
                }),
                // Select and extended select.
                0x02 | 0x46 => self.push(DeviceRange {
                    unit,
                    first: id,
                    last: id,
                    alias: None,
                    all: false,
                }),
                // Start of range, extended start of range.
                0x03 | 0x47 => start = Some((id, None)),
                // Alias start of range: the devices carry DevIDb, at bytes 5 and 6.
                0x43 => start = Some((id, Some(u16(entry, 5)))),
                0x04 => match start.take() {
                    Some((first, alias)) if first <= id => self.push(DeviceRange {
                        unit,
                        first,
                        last: id,
                        alias,
                        all: false,
                    }),
                    _ => self.truncated = true,
                },
                // Alias select: DevIDa uses DevIDb as its source id.
                0x42 => self.push(DeviceRange {
                    unit,
                    first: id,
                    last: id,
                    alias: Some(u16(entry, 5)),
                    all: false,
                }),
                // Special device (IOAPIC or HPET): no device id of its own on the bus, only the
                // source id at bytes 5 and 6 its interrupts carry. Recorded as a select of that
                // id, so the unit is known to own it.
                0x48 => {
                    let source = u16(entry, 5);
                    self.push(DeviceRange {
                        unit,
                        first: source,
                        last: source,
                        alias: None,
                        all: false,
                    });
                }
                // Reserved codes and the ACPI HID entry: sized above, skipped here.
                _ => {}
            }
        }
        if start.is_some() {
            // A start of range with no end: the table promised a range and did not finish it.
            self.truncated = true;
        }
    }

    fn push(&mut self, r: DeviceRange) {
        if self.range_count == MAX_IVHD_RANGES {
            self.truncated = true;
            return;
        }
        self.ranges[self.range_count] = r;
        self.range_count += 1;
    }

    fn read_ivmd(&mut self, kind: u8, b: &[u8]) {
        if b.len() < IVMD_LEN {
            self.truncated = true;
            return;
        }
        let flags = b[1];
        let (first, last) = match kind {
            0x20 => (0, 0xffff),
            0x21 => (u16(b, 4), u16(b, 4)),
            _ => (u16(b, 4), u16(b, 6)),
        };
        let start = u64(b, 16);
        let length = u64(b, 24);
        let base = start & !0xfff;
        let end = start.checked_add(length).and_then(|e| e.checked_add(0xfff));
        let (Some(end), true) = (end, length != 0 && first <= last) else {
            self.unity_refused += 1;
            return;
        };
        let r = UnityRegion {
            first,
            last,
            base,
            size: (end & !0xfff) - base,
            read: flags & (1 << 1) != 0,
            write: flags & (1 << 2) != 0,
            exclusion: flags & (1 << 3) != 0,
        };
        // "IR = 0 and IW = 0 ... the memory range is not to be mapped" (section 5.2.2.2): a
        // region firmware forbids rather than reserves. Nothing to map, so nothing to record.
        if !(r.read || r.write || r.exclusion) {
            return;
        }
        // The same region under a 10h and an 11h duplicate of one unit is recorded once.
        if self.unity[..self.unity_count].contains(&r) {
            return;
        }
        if self.unity_count == MAX_IVMDS {
            self.truncated = true;
            return;
        }
        self.unity[self.unity_count] = r;
        self.unity_count += 1;
    }

    /// The recorded IOMMUs, in table order.
    pub fn units(&self) -> &[Ivhd] {
        &self.ivhds[..self.ivhd_count]
    }

    /// The recorded device-id ranges.
    pub fn ranges(&self) -> &[DeviceRange] {
        &self.ranges[..self.range_count]
    }

    /// The recorded IVMDs.
    pub fn unity_regions(&self) -> &[UnityRegion] {
        &self.unity[..self.unity_count]
    }

    /// **Which unit serves device id `id`, and how the table said so.** An explicit entry (select,
    /// range, alias) anywhere wins over an "all" entry, which answers [`Ownership::CatchAll`].
    /// `Ok(None)` is a device no unit serves. `Err(())` is a table recorded only in part, where
    /// the missing part could be the answer.
    #[allow(clippy::result_unit_err)]
    pub fn owner_index(&self, id: u16) -> Result<Option<(usize, Ownership)>, ()> {
        if let Some(r) = self.ranges().iter().find(|r| !r.all && r.covers(id)) {
            return Ok(Some((r.unit as usize, Ownership::Named)));
        }
        if self.truncated {
            return Err(());
        }
        Ok(self
            .ranges()
            .iter()
            .find(|r| r.all)
            .map(|r| (r.unit as usize, Ownership::CatchAll)))
    }

    /// **The device id `id`'s transactions carry**: its alias when an alias entry covers it, and
    /// itself otherwise. The device table entry that confines a device is the one at this id.
    pub fn source_id(&self, id: u16) -> u16 {
        self.ranges()
            .iter()
            .find(|r| !r.all && r.covers(id))
            .and_then(|r| r.alias)
            .unwrap_or(id)
    }

    /// **The highest device id unit `unit` serves**, which sizes its device table: an id past the
    /// table's end is target-aborted by the hardware (section 2.2.2), so a table that stops at the
    /// last id the firmware named denies everything beyond it.
    pub fn last_device_id(&self, unit: usize) -> u16 {
        self.ranges()
            .iter()
            .filter(|r| r.unit as usize == unit)
            .map(|r| r.alias.map_or(r.last, |a| a.max(r.last)))
            .max()
            .unwrap_or(0)
    }

    /// **Every IVMD that applies to device id `id`**, each passed to `each` once.
    pub fn unity_for(&self, id: u16, each: &mut dyn FnMut(UnityRegion)) {
        for r in self.unity_regions() {
            if r.first <= id && id <= r.last {
                each(*r);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec::Vec;

    use super::*;

    /// One device entry, 4 bytes: type, device id, DTE setting.
    fn entry4(kind: u8, id: u16) -> [u8; 4] {
        let [lo, hi] = id.to_le_bytes();
        [kind, lo, hi, 0]
    }

    /// One 8-byte entry with a second device id at bytes 5 and 6 (alias and special entries).
    fn entry8(kind: u8, id: u16, b: u16) -> [u8; 8] {
        let [lo, hi] = id.to_le_bytes();
        let [blo, bhi] = b.to_le_bytes();
        [kind, lo, hi, 0, 0, blo, bhi, 0]
    }

    /// An IVHD of `kind` (10h or 11h) for the unit at `base`, followed by `entries`, the way
    /// `build_amd_iommu` writes one.
    fn ivhd(kind: u8, base: u64, entries: &[u8]) -> Vec<u8> {
        let header = if kind == 0x10 { 24 } else { 40 };
        let mut b = std::vec![0u8; header];
        b[0] = kind;
        b[1] = 0xd1;
        b[2..4].copy_from_slice(&((header + entries.len()) as u16).to_le_bytes());
        b[4..6].copy_from_slice(&0x0010u16.to_le_bytes());
        b[6..8].copy_from_slice(&0x40u16.to_le_bytes());
        b[8..16].copy_from_slice(&base.to_le_bytes());
        if kind != 0x10 {
            b[24..32].copy_from_slice(&0x0000_0000_0000_29d3u64.to_le_bytes());
        }
        b.extend_from_slice(entries);
        b
    }

    fn ivmd(kind: u8, flags: u8, first: u16, last: u16, start: u64, len: u64) -> Vec<u8> {
        let mut b = std::vec![0u8; IVMD_LEN];
        b[0] = kind;
        b[1] = flags;
        b[2..4].copy_from_slice(&(IVMD_LEN as u16).to_le_bytes());
        b[4..6].copy_from_slice(&first.to_le_bytes());
        b[6..8].copy_from_slice(&last.to_le_bytes());
        b[16..24].copy_from_slice(&start.to_le_bytes());
        b[24..32].copy_from_slice(&len.to_le_bytes());
        b
    }

    fn body(blocks: &[Vec<u8>]) -> Vec<u8> {
        // IVinfo as QEMU writes it: EFRSup, PASize 40.
        let mut b = std::vec![0u8; IVRS_FIXED_LEN];
        b[0..4].copy_from_slice(&(1u32 | (40 << 8)).to_le_bytes());
        for blk in blocks {
            b.extend_from_slice(blk);
        }
        b
    }

    /// **The table QEMU's `q35` writes for `-device amd-iommu`**, rebuilt from `build_amd_iommu`:
    /// one select entry per function on bus 0 (the host bridge, the IOMMU's own function, a disk,
    /// the NVMe), the IOAPIC special entry interrupt remapping adds, and the whole list twice, once
    /// under a 10h header and once under an 11h. Decoded once, from the 11h.
    fn q35_amd_iommu() -> Vec<u8> {
        let mut e = Vec::new();
        for id in [
            0x0000u16, 0x0008, 0x0010, 0x0018, 0x0020, 0x00f8, 0x00fa, 0x00fb,
        ] {
            e.extend_from_slice(&entry4(0x02, id));
        }
        // Special device: IOAPIC, handle 0, source id 00:14.0. QEMU writes it as one u64:
        // (1 << 56) | (0xa0 << 40) | 0x48.
        e.extend_from_slice(&0x0100_a000_0000_0048u64.to_le_bytes());
        body(&[ivhd(0x10, 0xfed8_0000, &e), ivhd(0x11, 0xfed8_0000, &e)])
    }

    #[test]
    fn one_unit_described_twice_is_decoded_once_from_the_highest_type() {
        let u = IvrsUnits::parse(&q35_amd_iommu());
        assert_eq!(u.ivhd_type, 0x11);
        assert_eq!(
            u.units().len(),
            1,
            "a 10h and an 11h for one unit are one unit"
        );
        let unit = u.units()[0];
        assert_eq!(unit.register_base, 0xfed8_0000);
        assert_eq!(unit.efr_image, Some(0x29d3));
        assert_eq!(unit.device_id, 0x0010);
        assert!(!u.truncated);
        assert_eq!(u.ivinfo & 1, 1, "EFRSup");
    }

    #[test]
    fn a_selected_device_is_owned_and_an_unnamed_one_is_not() {
        let u = IvrsUnits::parse(&q35_amd_iommu());
        assert_eq!(u.owner_index(0x0020), Ok(Some((0, Ownership::Named))));
        assert_eq!(
            u.owner_index(0x00a0),
            Ok(Some((0, Ownership::Named))),
            "the IOAPIC's source id, from the special entry"
        );
        assert_eq!(u.owner_index(0xfe00), Ok(None), "no entry and no 'all'");
        assert_eq!(u.last_device_id(0), 0x00fb);
        assert_eq!(u.source_id(0x0020), 0x0020);
    }

    /// **A range, an alias range and an "all" entry**, and the order they are asked in: explicit
    /// entries first, the catch-all last, the same rule a DMAR's catch-all follows.
    #[test]
    fn ranges_aliases_and_the_catch_all() {
        let mut e = Vec::new();
        e.extend_from_slice(&entry4(0x01, 0));
        e.extend_from_slice(&entry4(0x03, 0x0100));
        e.extend_from_slice(&entry4(0x04, 0x01ff));
        e.extend_from_slice(&entry8(0x43, 0x0200, 0x0018));
        e.extend_from_slice(&entry4(0x04, 0x02ff));
        e.extend_from_slice(&entry8(0x42, 0x0300, 0x0028));
        let u = IvrsUnits::parse(&body(&[ivhd(0x10, 0xfeb8_0000, &e)]));
        assert!(!u.truncated);
        assert_eq!(u.owner_index(0x0150), Ok(Some((0, Ownership::Named))));
        assert_eq!(u.owner_index(0x0250), Ok(Some((0, Ownership::Named))));
        assert_eq!(u.owner_index(0x0400), Ok(Some((0, Ownership::CatchAll))));
        assert_eq!(u.source_id(0x0250), 0x0018, "behind a PCIe-to-PCI bridge");
        assert_eq!(u.source_id(0x0300), 0x0028);
        assert_eq!(u.source_id(0x0150), 0x0150);
        assert_eq!(
            u.last_device_id(0),
            0xffff,
            "'all' sizes the table to every id"
        );
    }

    /// **Two units each own what their own IVHD names**, so an attach is routed to the right one.
    #[test]
    fn two_units_own_their_own_devices() {
        let a = ivhd(0x11, 0xfd20_0000, &entry4(0x02, 0x0008));
        let b = ivhd(0x11, 0xfd30_0000, &entry4(0x02, 0x4008));
        let u = IvrsUnits::parse(&body(&[a, b]));
        assert_eq!(u.units().len(), 2);
        assert_eq!(u.owner_index(0x0008), Ok(Some((0, Ownership::Named))));
        assert_eq!(u.owner_index(0x4008), Ok(Some((1, Ownership::Named))));
        assert_eq!(u.last_device_id(1), 0x4008);
    }

    #[test]
    fn an_ivmd_is_rounded_to_pages_and_found_for_its_devices() {
        let e = entry4(0x02, 0x0008);
        let u = IvrsUnits::parse(&body(&[
            ivhd(0x11, 0xfed8_0000, &e),
            ivmd(0x21, 0b0111, 0x0008, 0, 0x7f00_0800, 0x1000),
            ivmd(0x22, 0b0110, 0x0100, 0x01ff, 0x7e00_0000, 0x2000),
            // IR = IW = 0 and not an exclusion range: forbidden, not reserved.
            ivmd(0x20, 0b0000, 0, 0, 0x7d00_0000, 0x1000),
            // Zero length: refused.
            ivmd(0x21, 0b0111, 0x0008, 0, 0x7c00_0000, 0),
        ]));
        assert_eq!(u.unity_regions().len(), 2);
        assert_eq!(u.unity_refused, 1);
        let mut seen = Vec::new();
        u.unity_for(0x0008, &mut |r| seen.push((r.base, r.size)));
        assert_eq!(
            seen,
            [(0x7f00_0000, 0x2000)],
            "a page-straddling region covers both"
        );
        let mut seen = Vec::new();
        u.unity_for(0x0180, &mut |r| seen.push((r.base, r.size)));
        assert_eq!(seen, [(0x7e00_0000, 0x2000)]);
    }

    /// **An IVMD's `IW` bit decides whether the device may write it**, and `ExclusionRange` counts
    /// as write (Table 103's bits 1, 2 and 3). The read-only case is the one milestone 633 (an
    /// outside agent attacks the confinement claim)'s second pass found mapped writable.
    #[test]
    fn an_ivmd_without_iw_is_not_writable() {
        let e = entry4(0x02, 0x0008);
        let u = IvrsUnits::parse(&body(&[
            ivhd(0x11, 0xfed8_0000, &e),
            ivmd(0x21, 0b0011, 0x0008, 0, 0x7f00_0000, 0x1000),
            ivmd(0x21, 0b0111, 0x0008, 0, 0x7e00_0000, 0x1000),
            ivmd(0x21, 0b1001, 0x0008, 0, 0x7d00_0000, 0x1000),
        ]));
        let mut seen = Vec::new();
        u.unity_for(0x0008, &mut |r| seen.push((r.base, r.device_may_write())));
        assert_eq!(
            seen,
            [
                (0x7f00_0000, false),
                (0x7e00_0000, true),
                (0x7d00_0000, true)
            ]
        );
    }

    /// **A table this decoder recorded only in part answers "unknown" for what it did not see**:
    /// an unterminated range, and an entry code it cannot size.
    #[test]
    fn an_unfinished_table_is_unknown_not_unowned() {
        let u = IvrsUnits::parse(&body(&[ivhd(0x10, 0xfed8_0000, &entry4(0x03, 0x0100))]));
        assert!(u.truncated);
        assert_eq!(u.owner_index(0x0150), Err(()));
        let u = IvrsUnits::parse(&body(&[ivhd(0x10, 0xfed8_0000, &[0x81, 0, 0, 0])]));
        assert!(u.truncated);
    }

    /// **A block that fits but is too short for its type is reported, not skipped.** A four-byte
    /// block is the smallest the walk accepts, and one ending the table exactly is still walked.
    #[test]
    fn a_block_too_short_for_its_type_marks_the_table_truncated() {
        for kind in [0x10u8, 0x11, 0x21] {
            let stub = std::vec![kind, 0, 4, 0];
            let u = IvrsUnits::parse(&body(&[stub]));
            assert!(u.truncated, "a 4-byte {kind:#x} block");
            assert!(u.units().is_empty() && u.unity_regions().is_empty());
        }
    }

    /// The fixed part alone is a table with no blocks, and its `IVinfo` is still read.
    #[test]
    fn a_table_of_only_the_fixed_part_still_has_its_ivinfo() {
        let u = IvrsUnits::parse(&body(&[]));
        assert_eq!(u.ivinfo, 1 | (40 << 8));
        assert!(!u.truncated);
    }

    /// An IVHD of exactly its header names a unit with no devices: recorded, not truncated.
    #[test]
    fn an_ivhd_of_only_its_header_is_a_unit() {
        let u = IvrsUnits::parse(&body(&[ivhd(0x11, 0xfed8_0000, &[])]));
        assert_eq!(u.units().len(), 1);
        assert!(!u.truncated);
    }

    /// **The variable-length ACPI HID entry is sized and stepped over**, so the select after it
    /// is still read.
    #[test]
    fn the_acpi_hid_entry_is_skipped_by_its_own_length() {
        let mut e = std::vec![0u8; 22];
        e[0] = 0xf0;
        e[21] = 3;
        e.extend_from_slice(b"uid");
        e.extend_from_slice(&entry4(0x02, 0x00a0));
        let u = IvrsUnits::parse(&body(&[ivhd(0x10, 0xfed8_0000, &e)]));
        assert!(!u.truncated);
        assert_eq!(u.owner_index(0x00a0), Ok(Some((0, Ownership::Named))));
    }

    /// A range whose end is below its start names no device, and the table is reported as read
    /// only in part rather than as owning an inverted range.
    #[test]
    fn a_range_that_ends_before_it_starts_is_refused() {
        let mut e = Vec::new();
        e.extend_from_slice(&entry4(0x03, 0x0200));
        e.extend_from_slice(&entry4(0x04, 0x0100));
        let u = IvrsUnits::parse(&body(&[ivhd(0x10, 0xfed8_0000, &e)]));
        assert!(u.truncated);
        assert!(u.ranges().is_empty());
    }

    /// An IVMD of type 20h applies to every device, whatever its device-id bytes hold.
    #[test]
    fn a_type_20h_ivmd_covers_every_device() {
        let u = IvrsUnits::parse(&body(&[ivmd(0x20, 1 << 1, 0, 0, 0x1000, 0x1000)]));
        let r = u.unity_regions()[0];
        assert_eq!((r.first, r.last), (0, 0xffff));
    }

    /// **Every prefix of a real table decodes without panicking**, and never invents a unit the
    /// bytes do not hold. Firmware hands this parser whatever it wrote, and the walk runs on the
    /// boot path.
    #[test]
    fn every_prefix_and_every_corruption_is_survived() {
        let full = q35_amd_iommu();
        for n in 0..=full.len() {
            let u = IvrsUnits::parse(&full[..n]);
            assert!(u.units().len() <= 1);
        }
        for i in 0..full.len() {
            for v in [0x00u8, 0x01, 0x03, 0x43, 0x80, 0xf0, 0xff] {
                let mut b = full.clone();
                b[i] = v;
                let _ = IvrsUnits::parse(&b);
            }
        }
    }
}
