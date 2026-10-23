//! **The DMAR**: where VT-d is, which unit owns which PCI device, and which firmware memory each
//! device still needs. AMD's counterpart is [`ivrs`].
//!
//! Name: provisional (milestone 875's lane, 2026-10-12 UTC).

use super::*;

/// The DMAR's fixed part, before its list of remapping structures: the machine's physical
/// address width and a flags byte, then ten reserved bytes. Verified against QEMU's own table
/// builder (`build_dmar_q35` in `hw/i386/acpi-build.c`), which is the ground truth for what this
/// parser has to read, the same way the MADT and MCFG modules were checked against q35.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dmar {
    /// The machine's physical address width in bits, decoded from the table's
    /// `HostAddressWidth - 1` field (so a table saying 38 means a 39-bit width).
    ///
    /// **A `u16` for a value no machine puts above 64**, because the byte the table holds ranges
    /// over `0..=255` and the width it names therefore ranges over `1..=256`. Narrower than the
    /// field plus one is not a smaller type, it is an addition that can overflow on a firmware
    /// byte of `0xff`. Widening makes the wrong state unrepresentable rather than guarded.
    pub host_address_width: u16,
    /// Bit 0 is `INTR_REMAP`: the platform also supports interrupt remapping. Reported and not
    /// acted on, the same posture the MADT's `PCAT_COMPAT` bit takes; interrupt remapping is not
    /// built here (milestone 161 roadmap item 6 names it a follow-on, not this item).
    pub flags: u8,
}

/// The DMAR flags bit `INTR_REMAP` (VT-d 8.1): the platform's claim that it supports interrupt
/// remapping. Each unit makes its own claim in `ECAP.IR`, and the two need not agree.
const DMAR_INTR_REMAP: u8 = 1 << 0;

impl Dmar {
    /// **Does the platform claim interrupt remapping** (the table's `INTR_REMAP` flag)?
    ///
    /// Name: provisional (milestone 378 (read the DMAR), 2026-10-18 UTC).
    pub const fn claims_interrupt_remapping(&self) -> bool {
        self.flags & DMAR_INTR_REMAP != 0
    }
}

/// **The two interrupt-remapping claims a VT-d machine makes, as the boot line prints them**:
/// the unit's (`ECAP.IR`) and the platform's (the DMAR's `INTR_REMAP`). One line carries both so
/// a boot log shows whether firmware and hardware agree. `(unused)` because nothing in this
/// kernel remaps an interrupt, whatever either says.
///
/// Here rather than in the kernel's print so a host test holds the wording.
///
/// Name: provisional (milestone 378 (read the DMAR), 2026-10-18 UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptRemappingClaims {
    /// The unit's `ECAP.IR`.
    pub unit: bool,
    /// The DMAR's `INTR_REMAP`, [`DmarUnits::platform_interrupt_remapping`].
    pub platform: bool,
}

impl core::fmt::Display for InterruptRemappingClaims {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "interrupt remapping {} (unused), dmar INTR_REMAP {}",
            if self.unit { "offered" } else { "absent" },
            if self.platform { "set" } else { "clear" },
        )
    }
}

/// One byte of host address width, one byte of flags, ten reserved bytes.
pub(super) const DMAR_FIXED_LEN: usize = 12;

/// Decode the DMAR's fixed part. `body` begins after the SDT header.
pub fn parse_dmar(body: &[u8]) -> Result<Dmar, AcpiError> {
    if body.len() < DMAR_FIXED_LEN {
        return Err(AcpiError::Truncated);
    }
    Ok(Dmar {
        host_address_width: body[0] as u16 + 1,
        flags: body[1],
    })
}

/// **One DRHD: one VT-d hardware unit's register file.** A machine can have more than one (one
/// per PCI segment, sometimes one per root port group, and on a client Intel part one for the
/// integrated graphics beside the catch-all). The kernel brings up every one [`DmarUnits`]
/// records (milestone 594, provisional number).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drhd {
    /// Remapping-structure flags bit 0: this unit is the catch-all for every PCI device on its
    /// segment that no other DRHD's device-scope list names. [`DmarUnits::owner`] reads it.
    pub include_pci_all: bool,
    /// The PCI segment group this unit covers. Always 0 on a single-segment machine, which QEMU's
    /// `q35` is.
    pub segment: u16,
    /// The physical address of this unit's MMIO register file (`kernel/src/arch/x86_64/iommu.rs`
    /// reads it as the SMMUv3 driver reads its device-tree base and the RISC-V driver reads its
    /// BAR).
    pub register_base: u64,
    /// **How many bytes the register file spans**, from the DRHD's `Size` byte (VT-d 4.1 section
    /// 8.3: bits 3:0 are N, and the set is 2^N 4 KiB pages). Firmware written before that field
    /// existed left the byte reserved and zero, which decodes to one page, the size every unit
    /// had then. Name: provisional (milestone 594).
    pub register_size: u64,
}

/// The register-file size a DRHD's `Size` byte (offset 5) names: 4 KiB shifted by its low four
/// bits. At most 128 MiB, so the shift cannot overflow.
const fn drhd_register_size(size_byte: u8) -> u64 {
    4096u64 << (size_byte & 0xf)
}

/// How many bytes a DRHD's fixed part occupies before its (unparsed) device-scope list: the
/// 4-byte type/length header shared by every remapping structure, plus flags, reserved, segment
/// and register base.
pub(super) const DRHD_FIXED_LEN: usize = 16;

/// One entry of the DMAR's remapping-structure list. Only DRHD (type 0, the piece this kernel
/// drives) is decoded; the rest keep their type code so a boot print can say what it skipped
/// rather than pretending the list was shorter than it is. The same shape as [`MadtEntry`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmarEntry {
    Drhd(Drhd),
    /// A remapping-structure type this decoder does not act on (RMRR, ATSR, RHSA, ANDD, SATC,
    /// ...), with its type code.
    Other(u16),
}

/// Walks a DMAR's remapping-structure list. Each entry is `[type: u16, length: u16, ...]`, the
/// same self-describing shape the MADT's entries use, and a length of zero is refused the same
/// way: it would never advance and the walk would loop forever.
pub struct DmarStructures<'a> {
    body: &'a [u8],
    at: usize,
}

/// Iterate the DMAR's remapping structures. `body` begins after the SDT header.
pub fn dmar_structures(body: &[u8]) -> DmarStructures<'_> {
    DmarStructures {
        body,
        at: DMAR_FIXED_LEN,
    }
}

impl Iterator for DmarStructures<'_> {
    type Item = DmarEntry;

    fn next(&mut self) -> Option<DmarEntry> {
        if self.at + 4 > self.body.len() {
            return None;
        }
        let kind = u16(self.body, self.at);
        let len = u16(self.body, self.at + 2) as usize;
        if len < 4 || self.at + len > self.body.len() {
            return None;
        }
        let e = &self.body[self.at..self.at + len];
        self.at += len;

        Some(match kind {
            0 if len >= DRHD_FIXED_LEN => DmarEntry::Drhd(Drhd {
                include_pci_all: e[4] & 1 != 0,
                segment: u16(e, 6),
                register_base: u64(e, 8),
                register_size: drhd_register_size(e[5]),
            }),
            other => DmarEntry::Other(other),
        })
    }
}

/// The first DRHD in the list, if any. **No longer the unit the kernel brings up**: since milestone
/// 594 (every VT-d unit translates its own devices) it brings up every unit [`DmarUnits`] records. Kept because in-tree
/// records cite it and its tests pin the table order a client Intel machine uses.
pub fn first_drhd(body: &[u8]) -> Option<Drhd> {
    dmar_structures(body).find_map(|e| match e {
        DmarEntry::Drhd(d) => Some(d),
        DmarEntry::Other(_) => None,
    })
}

// ---------------------------------------------------------------------------------------------
// Which unit owns which device, and which firmware memory each device still needs: the
// device-scope lists and the RMRRs, decoded (milestone 261's bench rehearsal, milestone 594).
// ---------------------------------------------------------------------------------------------

/// How many DRHDs [`DmarUnits`] records. A client Intel part has two (one for the integrated
/// graphics, one catch-all); a two-socket server has one per root complex, a handful. A table with
/// more than this sets [`DmarUnits::truncated`] rather than dropping the rest silently.
pub const MAX_DRHDS: usize = 8;
/// How many PCI device-scope entries [`DmarUnits`] records across every DRHD. Only the two PCI
/// types are kept (endpoint and sub-hierarchy); IOAPIC, HPET and ACPI-namespace entries name no
/// requester id this kernel confines and are skipped.
pub const MAX_SCOPES: usize = 32;
/// The longest path a recorded scope can carry: the device itself plus three bridges above it.
/// Deeper than any machine this tree has met, and a deeper one sets [`DmarUnits::truncated`].
pub const MAX_SCOPE_PATH: usize = 4;
/// How many RMRRs [`DmarUnits`] records. The `OptiPlex` 7040 publishes two (USB, graphics). A
/// ninth sets [`DmarUnits::truncated`]. Name: provisional (milestone 594).
pub const MAX_RMRRS: usize = 8;
/// How many PCI device-scope entries [`DmarUnits`] records across every RMRR. Name: provisional
/// (milestone 594).
pub const MAX_RMRR_SCOPES: usize = 16;

/// Device-scope type 1: a PCI endpoint, named by its path from the start bus.
pub const SCOPE_PCI_ENDPOINT: u8 = 1;
/// Device-scope type 2: a PCI-PCI bridge, and with it **every device below it**.
pub const SCOPE_PCI_SUBHIERARCHY: u8 = 2;

/// The fixed part of a device-scope entry before its path: type, length, flags, reserved,
/// enumeration id, start bus. VT-d 3.x section 8.3.1; QEMU's `insert_scope` writes the same six.
const SCOPE_FIXED_LEN: usize = 6;

/// An RMRR's fixed part before its scope list: the 4-byte header, two reserved bytes, the segment,
/// then the region's base and its last byte (VT-d 4.1 section 8.4).
const RMRR_FIXED_LEN: usize = 24;

/// **One PCI device-scope entry, as the firmware wrote it**: a start bus and a path of
/// (device, function) pairs, each hop but the last a bridge whose secondary bus is where the next
/// hop lives. Resolving the path therefore needs the bus's live bridge registers, which is why
/// [`DmarUnits::owner`] takes a reader rather than doing it here.
///
/// Name: provisional (milestone 261's bench rehearsal). calef names public items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeviceScope {
    /// Index of the structure this entry belongs to: into [`DmarUnits::drhds`] for an entry in
    /// [`DmarUnits::scopes`], into [`DmarUnits::rmrrs`] for one in [`DmarUnits::rmrr_scopes`].
    pub unit: u8,
    /// [`SCOPE_PCI_ENDPOINT`] or [`SCOPE_PCI_SUBHIERARCHY`].
    pub kind: u8,
    pub start_bus: u8,
    /// `(device, function)` per hop, the first on `start_bus`. Only `path[..path_len]` is real.
    pub path: [(u8, u8); MAX_SCOPE_PATH],
    pub path_len: u8,
}

impl DeviceScope {
    /// **The function this entry's path ends at, as `(bus, device, function)` on this machine**,
    /// or `None` when a hop that should be a bridge is not one here (the path describes hardware
    /// that is not present). Every hop but the last is a bridge, and its secondary bus is where the
    /// next hop sits. Name: provisional (milestone 594).
    pub fn resolve(
        &self,
        bridge: &mut dyn FnMut(u8, u8, u8) -> Option<(u8, u8)>,
    ) -> Option<(u8, u8, u8)> {
        let n = self.path_len as usize;
        // `parse` never records an empty or over-long path; the fields are public, so say so
        // here rather than let a hand-built scope underflow `n - 1`.
        if n == 0 || n > MAX_SCOPE_PATH {
            return None;
        }
        let mut at_bus = self.start_bus;
        for &(d, f) in &self.path[..n - 1] {
            at_bus = bridge(at_bus, d, f)?.0;
        }
        let (d, f) = self.path[n - 1];
        Some((at_bus, d, f))
    }

    /// **Does this entry cover `bus:dev.func`, and how?** An endpoint covers the function it
    /// resolves to; a sub-hierarchy covers its bridge and every bus in the bridge's
    /// secondary..=subordinate range (VT-d 4.1 section 8.3.1: "the specified bridge device and all
    /// its downstream devices").
    fn covers(
        &self,
        bus: u8,
        dev: u8,
        func: u8,
        bridge: &mut dyn FnMut(u8, u8, u8) -> Option<(u8, u8)>,
    ) -> Option<Ownership> {
        let (at_bus, d, f) = self.resolve(bridge)?;
        if (at_bus, d, f) == (bus, dev, func) {
            return Some(Ownership::Named);
        }
        if self.kind == SCOPE_PCI_SUBHIERARCHY
            && let Some((secondary, subordinate)) = bridge(at_bus, d, f)
            && (secondary..=subordinate).contains(&bus)
        {
            return Some(Ownership::UnderBridge(at_bus, d, f));
        }
        None
    }
}

/// **How a unit came to own a device**, which is the half of the answer a bench reader needs to
/// believe the other half. The three ways the VT-d specification allows (section 8.3).
///
/// Name: provisional (milestone 261's bench rehearsal).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership {
    /// The unit's scope names this function as an endpoint.
    Named,
    /// The unit's scope names a bridge, and this function's bus is inside the bridge's
    /// secondary..=subordinate range. The bridge is carried as (bus, device, function).
    UnderBridge(u8, u8, u8),
    /// Nothing names the function, and this unit carries `INCLUDE_PCI_ALL` for its segment.
    CatchAll,
}

/// **One RMRR: memory the firmware keeps DMA-ing into after it hands over** (VT-d 4.1 sections
/// 3.16 and 8.4). USB legacy keyboard emulation under SMM and a UMA graphics controller's stolen
/// memory are the two the specification names, and the two the `OptiPlex` 7040 publishes. The
/// specification's instruction is that system software identity-map `base..=limit`, read and
/// write, for every device the region's scope names, before it turns translation on.
///
/// Its devices are the entries of [`DmarUnits::rmrr_scopes`] whose `unit` is this region's index.
///
/// Name: provisional (milestone 594). calef names public items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReservedRegion {
    pub segment: u16,
    /// First byte, 4 KiB aligned.
    pub base: u64,
    /// Last byte (inclusive, as the table writes it), so `limit + 1` is 4 KiB aligned.
    pub limit: u64,
}

impl ReservedRegion {
    /// The region's length in bytes. Total: [`DmarUnits::parse`] records only regions whose
    /// `limit` exceeds `base`.
    pub const fn size(&self) -> u64 {
        self.limit - self.base + 1
    }

    /// The VT-d specification's well-formedness rule for an RMRR (section 8.4: a 4 KiB-aligned
    /// base, a size that is a whole number of 4 KiB pages, a limit above the base), which is also
    /// the check Linux's `rmrr_sanity_check` makes before it trusts one.
    const fn is_well_formed(&self) -> bool {
        self.base.is_multiple_of(4096)
            && self.limit > self.base
            && self.limit != u64::MAX
            && (self.limit + 1).is_multiple_of(4096)
    }
}

/// **Every DRHD, every PCI device scope and every RMRR in one DMAR**, decoded into fixed arrays so
/// the kernel can keep it after the firmware's bytes are out of reach (it is read at boot, before
/// the fine map, and asked after it). `Copy` and free of lifetimes for exactly that reason.
///
/// This exists because [`first_drhd`] answered a question nobody on real hardware asks. xenon's
/// DMAR is 204 bytes and its first DRHD is `0xfed90000`; on the Skylake `OptiPlex` 7040, the same
/// family and the same register addresses, Linux reports that unit with flags `0x0` and a second
/// at `0xfed91000` with flags `0x1` (`INCLUDE_PCI_ALL`), which is the integrated-graphics unit
/// followed by the catch-all. See notes/risk-6-bench-evening.md for the source.
///
/// Name: provisional (milestone 261's bench rehearsal).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmarUnits {
    pub drhds: [Drhd; MAX_DRHDS],
    pub drhd_count: usize,
    pub scopes: [DeviceScope; MAX_SCOPES],
    pub scope_count: usize,
    pub rmrrs: [ReservedRegion; MAX_RMRRS],
    pub rmrr_count: usize,
    pub rmrr_scopes: [DeviceScope; MAX_RMRR_SCOPES],
    pub rmrr_scope_count: usize,
    /// RMRRs the table held and this decoder refused because they break section 8.4's
    /// well-formedness rule. Linux skips such a region with a firmware-bug warning; so does this,
    /// and the count is what lets the boot print say so. Name: provisional (milestone 594).
    pub rmrrs_refused: usize,
    /// Something in the table did not fit the arrays above: a ninth DRHD or RMRR, a scope past its
    /// array, a path deeper than [`MAX_SCOPE_PATH`], or a scope whose length disagrees with its
    /// path. **When set, a device no recorded scope names is answered "unknown" rather than handed
    /// to the catch-all**, because the entry that names it may be the one that was dropped.
    pub truncated: bool,
    /// The fixed part's `INTR_REMAP` flag, [`Dmar::claims_interrupt_remapping`]: the platform's
    /// claim, kept beside the units whose own claim (`ECAP.IR`) the kernel reads from each. False
    /// for a body too short to hold the flags byte. Name: provisional (milestone 378, 2026-10-18).
    pub platform_interrupt_remapping: bool,
}

impl Default for DmarUnits {
    fn default() -> Self {
        Self {
            drhds: [Drhd {
                include_pci_all: false,
                segment: 0,
                register_base: 0,
                register_size: 0,
            }; MAX_DRHDS],
            drhd_count: 0,
            scopes: [DeviceScope::default(); MAX_SCOPES],
            scope_count: 0,
            rmrrs: [ReservedRegion::default(); MAX_RMRRS],
            rmrr_count: 0,
            rmrr_scopes: [DeviceScope::default(); MAX_RMRR_SCOPES],
            rmrr_scope_count: 0,
            rmrrs_refused: 0,
            truncated: false,
            platform_interrupt_remapping: false,
        }
    }
}

/// Decode one scope list into `out`, tagging each entry with `owner`. Total for any input: a
/// malformed list stops and sets `truncated`, and an entry past `out` sets it too.
fn read_scope_list(
    mut list: &[u8],
    owner: u8,
    out: &mut [DeviceScope],
    count: &mut usize,
    truncated: &mut bool,
) {
    while list.len() >= 2 {
        let kind = list[0];
        let len = list[1] as usize;
        if len < SCOPE_FIXED_LEN || len > list.len() || !(len - SCOPE_FIXED_LEN).is_multiple_of(2) {
            *truncated = true;
            return;
        }
        let entry = &list[..len];
        list = &list[len..];
        if kind != SCOPE_PCI_ENDPOINT && kind != SCOPE_PCI_SUBHIERARCHY {
            continue;
        }
        let hops = (len - SCOPE_FIXED_LEN) / 2;
        if hops == 0 || hops > MAX_SCOPE_PATH || *count == out.len() {
            *truncated = true;
            continue;
        }
        let mut s = DeviceScope {
            unit: owner,
            kind,
            start_bus: entry[5],
            path_len: hops as u8,
            ..DeviceScope::default()
        };
        for h in 0..hops {
            s.path[h] = (
                entry[SCOPE_FIXED_LEN + 2 * h],
                entry[SCOPE_FIXED_LEN + 2 * h + 1],
            );
        }
        out[*count] = s;
        *count += 1;
    }
}

impl DmarUnits {
    /// Decode every DRHD, every RMRR and their PCI scopes. `body` begins after the SDT header.
    /// Total for any input: a malformed scope list stops that structure's scopes and sets
    /// [`DmarUnits::truncated`].
    pub fn parse(body: &[u8]) -> DmarUnits {
        let mut u = DmarUnits {
            platform_interrupt_remapping: parse_dmar(body)
                .is_ok_and(|d| d.claims_interrupt_remapping()),
            ..DmarUnits::default()
        };
        let mut at = DMAR_FIXED_LEN;
        while at + 4 <= body.len() {
            let kind = u16(body, at);
            let len = u16(body, at + 2) as usize;
            if len < 4 || at + len > body.len() {
                break;
            }
            let e = &body[at..at + len];
            at += len;
            match kind {
                0 if len >= DRHD_FIXED_LEN => {
                    if u.drhd_count == MAX_DRHDS {
                        u.truncated = true;
                        continue;
                    }
                    let unit = u.drhd_count;
                    u.drhds[unit] = Drhd {
                        include_pci_all: e[4] & 1 != 0,
                        segment: u16(e, 6),
                        register_base: u64(e, 8),
                        register_size: drhd_register_size(e[5]),
                    };
                    u.drhd_count += 1;
                    read_scope_list(
                        &e[DRHD_FIXED_LEN..],
                        unit as u8,
                        &mut u.scopes,
                        &mut u.scope_count,
                        &mut u.truncated,
                    );
                }
                1 if len >= RMRR_FIXED_LEN => {
                    let r = ReservedRegion {
                        segment: u16(e, 6),
                        base: u64(e, 8),
                        limit: u64(e, 16),
                    };
                    if !r.is_well_formed() {
                        u.rmrrs_refused += 1;
                        continue;
                    }
                    if u.rmrr_count == MAX_RMRRS {
                        u.truncated = true;
                        continue;
                    }
                    let index = u.rmrr_count;
                    u.rmrrs[index] = r;
                    u.rmrr_count += 1;
                    read_scope_list(
                        &e[RMRR_FIXED_LEN..],
                        index as u8,
                        &mut u.rmrr_scopes,
                        &mut u.rmrr_scope_count,
                        &mut u.truncated,
                    );
                }
                _ => {}
            }
        }
        u
    }

    /// The recorded DRHDs, in table order.
    pub fn units(&self) -> &[Drhd] {
        &self.drhds[..self.drhd_count]
    }

    /// The recorded RMRRs, in table order. Name: provisional (milestone 594).
    pub fn reserved_regions(&self) -> &[ReservedRegion] {
        &self.rmrrs[..self.rmrr_count]
    }

    /// Which unit owns PCI function `bus:dev.func` on `segment`, by the VT-d specification's rule
    /// (section 8.3): an explicit scope anywhere wins, then the segment's `INCLUDE_PCI_ALL` unit.
    /// `Ok(None)` is a machine where nothing translates this device's DMA; `Err(())` is a table
    /// recorded only in part, where the missing part could be the answer. Unlike Linux's table-order
    /// walk, this agrees with the rule even when the catch-all is not listed last.
    ///
    /// `bridge` returns a bridge's `(secondary, subordinate)` bus numbers from the live bus, or
    /// `None` when `bus:dev.func` is not a bridge; only the live bus turns a path into a bus.
    #[allow(clippy::result_unit_err)]
    pub fn owner(
        &self,
        segment: u16,
        bus: u8,
        dev: u8,
        func: u8,
        bridge: &mut dyn FnMut(u8, u8, u8) -> Option<(u8, u8)>,
    ) -> Result<Option<(Drhd, Ownership)>, ()> {
        Ok(self
            .owner_index(segment, bus, dev, func, bridge)?
            .map(|(i, how)| (self.drhds[i], how)))
    }

    /// [`DmarUnits::owner`], answered as an index into [`DmarUnits::units`]: the unit whose root
    /// table a context entry for this function belongs in. Name: provisional (milestone 594).
    #[allow(clippy::result_unit_err)]
    pub fn owner_index(
        &self,
        segment: u16,
        bus: u8,
        dev: u8,
        func: u8,
        bridge: &mut dyn FnMut(u8, u8, u8) -> Option<(u8, u8)>,
    ) -> Result<Option<(usize, Ownership)>, ()> {
        for s in &self.scopes[..self.scope_count] {
            let unit = s.unit as usize;
            if self.drhds[unit].segment != segment {
                continue;
            }
            if let Some(how) = s.covers(bus, dev, func, bridge) {
                return Ok(Some((unit, how)));
            }
        }
        if self.truncated {
            return Err(());
        }
        Ok(self
            .units()
            .iter()
            .position(|d| d.include_pci_all && d.segment == segment)
            .map(|i| (i, Ownership::CatchAll)))
    }

    /// **Every RMRR the firmware declared for `bus:dev.func` on `segment`**, each passed to `each`
    /// once. A region names a device by an endpoint scope, or by a sub-hierarchy scope above it,
    /// which is the same test Linux's `intel_iommu_get_resv_regions` makes
    /// (`is_downstream_to_pci_bridge`). The kernel adds every one to the device's DMA domain,
    /// because section 3.16 asks for the identity map in *the* translation the device uses, and a
    /// domain built without them would fault the firmware's own DMA the moment it was attached.
    /// Name: provisional (milestone 594).
    pub fn reserved_for(
        &self,
        segment: u16,
        bus: u8,
        dev: u8,
        func: u8,
        bridge: &mut dyn FnMut(u8, u8, u8) -> Option<(u8, u8)>,
        each: &mut dyn FnMut(ReservedRegion),
    ) {
        for (i, r) in self.reserved_regions().iter().enumerate() {
            if r.segment != segment {
                continue;
            }
            let named = self.rmrr_scopes[..self.rmrr_scope_count]
                .iter()
                .filter(|s| s.unit as usize == i)
                .any(|s| s.covers(bus, dev, func, bridge).is_some());
            if named {
                each(*r);
            }
        }
    }
}

/// Machine-checked proofs over the DMAR (milestone 319). Why these tables carry harnesses is on
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

    /// **The DMAR walk terminates and never reads outside the body**, the MADT walk's claim one
    /// table over. Stated twice because the two walks are separate encodings: a one-byte length of
    /// at least two against a `u16` length of at least four. `len < 2` copied from the MADT lets a
    /// four-byte header claiming two re-read its own first half forever.
    /// Falsification: replayable `crates/machine_discovery/falsifications/acpi.dmar.verification.the_dmar_walk_terminates_and_stays_inside_the_body.patch`
    #[kani::proof]
    #[kani::unwind(12)]
    fn the_dmar_walk_terminates_and_stays_inside_the_body() {
        let body: [u8; N] = kani::any();
        let mut it = dmar_structures(&body);
        let mut seen = 0usize;
        let mut last = it.at;
        while it.next().is_some() {
            assert!(it.at >= last + 4);
            assert!(it.at <= body.len());
            last = it.at;
            seen += 1;
        }
        assert!(seen <= N / 4);
    }

    /// **The DMAR's fixed part decodes without arithmetic overflow**, for every `HostAddressWidth`
    /// byte. `kernel/src/arch/x86_64/machine.rs` calls [`parse_dmar`] directly on firmware bytes,
    /// and QEMU always writes 38, so no table a test copies reaches the edge. Milestone 319 records
    /// the defect this found.
    /// Falsification: replayable `crates/machine_discovery/falsifications/acpi.dmar.verification.the_dmar_fixed_part_decodes_without_arithmetic_overflow.patch`
    #[kani::proof]
    fn the_dmar_fixed_part_decodes_without_arithmetic_overflow() {
        let body: [u8; N] = kani::any();
        if let Ok(d) = parse_dmar(&body) {
            assert_eq!(d.host_address_width, body[0] as u16 + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acpi::tests::q35_dmar_body;

    extern crate std;

    /// The fixed part decodes: the address width is the table's field plus one, and the flags
    /// byte round-trips unexamined.
    #[test]
    fn the_dmar_reports_the_address_width_and_flags() {
        let d = parse_dmar(&q35_dmar_body()).expect("well-formed");
        assert_eq!(
            d.host_address_width, 39,
            "the table stores width - 1; q35 reports 38 for a 39-bit width"
        );
        assert_eq!(d.flags, 0, "intremap=off: no interrupt remapping claimed");
    }

    /// **The platform's interrupt-remapping claim survives the parse and reaches the boot line**
    /// (milestone 378), set and clear. Bit 0 alone decides it: the set table also carries bits 1
    /// and 2 (`X2APIC_OPT_OUT`, `DMA_CTRL_PLATFORM_OPT_IN`) and the clear one carries them too,
    /// so a test of the whole byte against zero would pass both and prove nothing. The unit's
    /// claim is set opposite to the platform's in each, so a line that printed one in the other's
    /// place fails.
    #[test]
    fn the_dmar_intr_remap_flag_is_kept_and_printed_beside_the_units() {
        for (flags, platform) in [(0b111, true), (0b110, false)] {
            let mut body = dmar(&[drhd(0, 0xfed9_0000, &[])]);
            body[1] = flags;
            assert_eq!(
                parse_dmar(&body).unwrap().claims_interrupt_remapping(),
                platform
            );
            let u = DmarUnits::parse(&body);
            assert_eq!(u.platform_interrupt_remapping, platform, "flags {flags:#b}");
            let line = std::format!(
                "{}",
                InterruptRemappingClaims {
                    unit: !platform,
                    platform: u.platform_interrupt_remapping,
                }
            );
            let want = if platform {
                "interrupt remapping absent (unused), dmar INTR_REMAP set"
            } else {
                "interrupt remapping offered (unused), dmar INTR_REMAP clear"
            };
            assert_eq!(line, want);
        }
        assert!(
            !DmarUnits::parse(&[0x26]).platform_interrupt_remapping,
            "a body too short for the flags byte claims nothing"
        );
    }

    /// **The whole reason this table gets read**: a DRHD names where VT-d's register file is,
    /// and `first_drhd` is what `kernel/src/arch/x86_64/machine.rs` calls to find it.
    #[test]
    fn the_drhd_gives_the_register_base() {
        let body = q35_dmar_body();
        let d = first_drhd(&body).expect("one DRHD in this table");
        assert_eq!(d.register_base, 0xfed9_0000);
        assert_eq!(d.segment, 0);
        assert!(
            !d.include_pci_all,
            "q35 names its devices explicitly rather than using the catch-all"
        );
    }

    /// The iterator sees the DRHD by type code, alongside whatever else the list might hold, the
    /// same shape [`madt_entries`] proves for the MADT.
    #[test]
    fn dmar_structures_decodes_a_drhd_and_stops_at_the_end() {
        let body = q35_dmar_body();
        let mut it = dmar_structures(&body);
        assert_eq!(
            it.next(),
            Some(DmarEntry::Drhd(Drhd {
                include_pci_all: false,
                segment: 0,
                register_base: 0xfed9_0000,
                register_size: 4096,
            }))
        );
        assert_eq!(it.next(), None);
    }

    /// **Two of the shortest structures the list can hold, both read.**
    ///
    /// Four bytes each, a type and a length and nothing else, and the second one is what proves
    /// the walk advances by the length it read rather than jumping to the end. Every other DMAR
    /// here holds a single sixteen-byte DRHD, where a cursor that advanced wrongly and a walk that
    /// stopped early are indistinguishable from the right answer.
    #[test]
    fn two_shortest_remapping_structures_are_both_read() {
        let mut body = [0u8; DMAR_FIXED_LEN + 8];
        let d = DMAR_FIXED_LEN;
        body[d..d + 2].copy_from_slice(&7u16.to_le_bytes());
        body[d + 2..d + 4].copy_from_slice(&4u16.to_le_bytes());
        body[d + 4..d + 6].copy_from_slice(&8u16.to_le_bytes());
        body[d + 6..d + 8].copy_from_slice(&4u16.to_le_bytes());

        let mut it = dmar_structures(&body);
        assert_eq!(it.next(), Some(DmarEntry::Other(7)));
        assert_eq!(it.next(), Some(DmarEntry::Other(8)));
        assert_eq!(it.next(), None);
    }

    /// **A DRHD too short to hold its own register base is reported by its type**, not decoded.
    /// The guard is the only thing between a malformed table and reading eight bytes that are not
    /// there, and what a decoded one would yield is an MMIO address the IOMMU driver then writes
    /// to.
    #[test]
    fn a_drhd_too_short_for_its_own_fields_is_reported_by_type() {
        let mut body = [0u8; DMAR_FIXED_LEN + 8];
        let d = DMAR_FIXED_LEN;
        body[d..d + 2].copy_from_slice(&0u16.to_le_bytes()); // type 0: DRHD
        body[d + 2..d + 4].copy_from_slice(&8u16.to_le_bytes());

        let mut it = dmar_structures(&body);
        assert_eq!(it.next(), Some(DmarEntry::Other(0)));
        assert_eq!(it.next(), None);
    }

    /// A remapping-structure type this decoder does not act on is reported by its type code
    /// rather than silently dropped, so a boot print can say what it skipped.
    #[test]
    fn an_unrecognized_remapping_structure_is_reported_by_type() {
        let mut body = [0u8; DMAR_FIXED_LEN + 8].to_vec();
        body[DMAR_FIXED_LEN..DMAR_FIXED_LEN + 2].copy_from_slice(&1u16.to_le_bytes()); // RMRR
        body[DMAR_FIXED_LEN + 2..DMAR_FIXED_LEN + 4].copy_from_slice(&8u16.to_le_bytes());
        let mut it = dmar_structures(&body);
        assert_eq!(it.next(), Some(DmarEntry::Other(1)));
        assert_eq!(it.next(), None);
    }

    /// **A malformed structure length ends the walk rather than looping forever**, the same
    /// refusal [`madt_entries`] makes for the same reason: a zero length would never advance.
    #[test]
    fn a_zero_length_remapping_structure_ends_the_walk() {
        let mut body = q35_dmar_body().to_vec();
        body[DMAR_FIXED_LEN + 2] = 0;
        body[DMAR_FIXED_LEN + 3] = 0;
        assert_eq!(dmar_structures(&body).count(), 0);
    }

    /// A DRHD with its scope list, for the device-scope tests below.
    fn drhd(flags: u8, base: u64, scopes: &[&[u8]]) -> std::vec::Vec<u8> {
        let len = DRHD_FIXED_LEN + scopes.iter().map(|s| s.len()).sum::<usize>();
        let mut e = std::vec![0u8; DRHD_FIXED_LEN];
        e[0..2].copy_from_slice(&0u16.to_le_bytes());
        e[2..4].copy_from_slice(&(len as u16).to_le_bytes());
        e[4] = flags;
        e[8..16].copy_from_slice(&base.to_le_bytes());
        for s in scopes {
            e.extend_from_slice(s);
        }
        e
    }

    /// One device-scope entry: type, start bus, and a path of (device, function) hops.
    fn scope(kind: u8, start_bus: u8, path: &[(u8, u8)]) -> std::vec::Vec<u8> {
        let mut s = std::vec![
            kind,
            (SCOPE_FIXED_LEN + 2 * path.len()) as u8,
            0,
            0,
            0,
            start_bus
        ];
        for &(d, f) in path {
            s.push(d);
            s.push(f);
        }
        s
    }

    fn dmar(units: &[std::vec::Vec<u8>]) -> std::vec::Vec<u8> {
        let mut b = std::vec![0u8; DMAR_FIXED_LEN];
        b[0] = 38;
        for u in units {
            b.extend_from_slice(u);
        }
        b
    }

    /// The bus xenon is expected to have: a root port at 00:1d.0 with the NVMe on bus 1 below it.
    fn xenon_bus(bus: u8, dev: u8, func: u8) -> Option<(u8, u8)> {
        ((bus, dev, func) == (0, 0x1d, 0)).then_some((1, 1))
    }

    /// **The layout a client Intel machine publishes, and the reason this decoder exists.** The
    /// integrated-graphics unit first, naming 00:02.0 and nothing else, then the catch-all with its
    /// IOAPIC and HPET scopes. This is the `OptiPlex` 7040's as Linux prints it (flags 0x0 at
    /// 0xfed90000, flags 0x1 at 0xfed91000); xenon is the 7050, the next generation of the same
    /// board. `first_drhd` picks the graphics unit, and the NVMe is not behind it.
    #[test]
    fn on_a_client_intel_layout_the_nvme_belongs_to_the_catch_all_not_the_first_unit() {
        let body = dmar(&[
            drhd(0, 0xfed9_0000, &[&scope(SCOPE_PCI_ENDPOINT, 0, &[(2, 0)])]),
            drhd(
                1,
                0xfed9_1000,
                &[&scope(3, 0xf0, &[(0x1f, 0)]), &scope(4, 0, &[(0x1f, 0)])],
            ),
        ]);
        let u = DmarUnits::parse(&body);
        assert!(
            !u.truncated,
            "IOAPIC and HPET scopes are skipped, not errors"
        );
        assert_eq!(u.units().len(), 2);
        assert_eq!(first_drhd(&body).unwrap().register_base, 0xfed9_0000);
        let (unit, how) = u.owner(0, 1, 0, 0, &mut xenon_bus).unwrap().unwrap();
        assert_eq!(
            (unit.register_base, how),
            (0xfed9_1000, Ownership::CatchAll)
        );
        let (unit, how) = u.owner(0, 0, 2, 0, &mut xenon_bus).unwrap().unwrap();
        assert_eq!((unit.register_base, how), (0xfed9_0000, Ownership::Named));
    }

    /// **A bridge named as a sub-hierarchy owns everything below it**, and a device no scope names
    /// on a machine with no catch-all is owned by nobody. The second half is the verdict the bench
    /// must be able to print, and it is the shape QEMU writes when a host bridge bypasses the IOMMU.
    #[test]
    fn a_subhierarchy_scope_owns_its_buses_and_an_unnamed_device_has_no_owner() {
        let body = dmar(&[drhd(
            0,
            0xfed9_0000,
            &[&scope(SCOPE_PCI_SUBHIERARCHY, 0, &[(0x1d, 0)])],
        )]);
        let u = DmarUnits::parse(&body);
        assert_eq!(
            u.owner(0, 1, 0, 0, &mut xenon_bus).unwrap().unwrap().1,
            Ownership::UnderBridge(0, 0x1d, 0)
        );
        assert_eq!(u.owner(0, 0, 3, 0, &mut xenon_bus), Ok(None));
        assert_eq!(
            u.owner(1, 1, 0, 0, &mut xenon_bus),
            Ok(None),
            "another segment"
        );
    }

    /// **A two-hop path is resolved through the live bridge**: the first hop is 00:1d.0, its
    /// secondary bus is 1, and the endpoint is 01:00.0. A path whose first hop is not a bridge on
    /// this machine names nothing, rather than being read as a bus-0 device.
    #[test]
    fn a_multi_hop_endpoint_path_is_resolved_through_the_bridge_registers() {
        let body = dmar(&[drhd(
            0,
            0xfed9_2000,
            &[&scope(SCOPE_PCI_ENDPOINT, 0, &[(0x1d, 0), (0, 0)])],
        )]);
        let u = DmarUnits::parse(&body);
        assert_eq!(
            u.owner(0, 1, 0, 0, &mut xenon_bus).unwrap().unwrap().1,
            Ownership::Named
        );
        assert_eq!(u.owner(0, 1, 0, 0, &mut |_, _, _| None), Ok(None));
    }

    /// **A table that did not fit answers "unknown", never "the catch-all"**, because the dropped
    /// entry could have been the one naming the device.
    #[test]
    fn a_truncated_table_refuses_to_guess() {
        let many: std::vec::Vec<std::vec::Vec<u8>> = (0..MAX_SCOPES as u8 + 1)
            .map(|i| scope(SCOPE_PCI_ENDPOINT, 0, &[(i, 0)]))
            .collect();
        let refs: std::vec::Vec<&[u8]> = many.iter().map(|s| s.as_slice()).collect();
        let u = DmarUnits::parse(&dmar(&[drhd(1, 0xfed9_1000, &refs)]));
        assert!(u.truncated);
        assert_eq!(u.owner(0, 5, 0, 0, &mut xenon_bus), Err(()));
        assert_eq!(
            u.owner(0, 0, 3, 0, &mut xenon_bus).unwrap().unwrap().1,
            Ownership::Named,
            "a recorded scope still answers"
        );
    }

    /// Hostile scope lengths (zero, odd, past the end) stop the list and mark it, without a panic.
    #[test]
    fn a_malformed_scope_list_is_marked_rather_than_trusted() {
        for bad in [
            &[1u8, 0][..],
            &[1, 7, 0, 0, 0, 0, 0][..],
            &[1, 40, 0, 0, 0, 0, 1, 0][..],
        ] {
            let u = DmarUnits::parse(&dmar(&[drhd(1, 0xfed9_1000, &[bad])]));
            assert!(u.truncated, "{bad:?}");
            assert_eq!(u.units().len(), 1);
        }
        assert_eq!(DmarUnits::parse(&[]).units().len(), 0);
    }

    /// An RMRR with its scope list: base, last byte, then the scopes.
    fn rmrr(base: u64, limit: u64, scopes: &[&[u8]]) -> std::vec::Vec<u8> {
        let len = RMRR_FIXED_LEN + scopes.iter().map(|s| s.len()).sum::<usize>();
        let mut e = std::vec![0u8; RMRR_FIXED_LEN];
        e[0..2].copy_from_slice(&1u16.to_le_bytes());
        e[2..4].copy_from_slice(&(len as u16).to_le_bytes());
        e[8..16].copy_from_slice(&base.to_le_bytes());
        e[16..24].copy_from_slice(&limit.to_le_bytes());
        for s in scopes {
            e.extend_from_slice(s);
        }
        e
    }

    /// **The `OptiPlex` 7040's whole DMAR, rebuilt from what Linux printed about it** (milestone
    /// 594): the graphics unit naming 00:02.0, the catch-all naming its IOAPIC and HPET, an RMRR at
    /// `0xdb5d2000..=0xdb5f1fff` and one at `0xdd800000..=0xdfffffff`. Which device each RMRR names
    /// is not in that log; USB (00:14.0) and graphics (00:02.0) are what section 8.4 says RMRRs are
    /// for and what the two sizes fit (128 KiB of USB buffers, 40 MiB of stolen memory). The
    /// reconstruction is checked by length: the 7040's table header says `0xA8` bytes, and this is
    /// that many, with nothing to spare for a structure the reading left out.
    fn optiplex_7040_dmar() -> std::vec::Vec<u8> {
        dmar(&[
            drhd(0, 0xfed9_0000, &[&scope(SCOPE_PCI_ENDPOINT, 0, &[(2, 0)])]),
            drhd(
                1,
                0xfed9_1000,
                &[&scope(3, 0xf0, &[(0x1f, 0)]), &scope(4, 0, &[(0x1f, 0)])],
            ),
            rmrr(
                0xdb5d_2000,
                0xdb5f_1fff,
                &[&scope(SCOPE_PCI_ENDPOINT, 0, &[(0x14, 0)])],
            ),
            rmrr(
                0xdd80_0000,
                0xdfff_ffff,
                &[&scope(SCOPE_PCI_ENDPOINT, 0, &[(2, 0)])],
            ),
        ])
    }

    fn reserved(u: &DmarUnits, bus: u8, dev: u8, func: u8) -> std::vec::Vec<(u64, u64)> {
        let mut v = std::vec::Vec::new();
        u.reserved_for(0, bus, dev, func, &mut xenon_bus, &mut |r| {
            v.push((r.base, r.limit));
        });
        v
    }

    /// **Every device lands in its own unit's root table** (milestone 594, the proposal's
    /// two-unit host test). `owner_index` is the decision the kernel's `attach` routes on, so this
    /// is the routing proved without the registers: the GPU to the graphics unit (index 0), and
    /// the NVMe behind its root port, the USB controller, the root port itself and the Ethernet
    /// function to the catch-all (index 1). QEMU presents one unit, so this is the only place the
    /// two-unit route runs before xenon.
    #[test]
    fn on_the_7040s_table_each_device_routes_to_the_unit_that_owns_it() {
        let body = optiplex_7040_dmar();
        assert_eq!(SDT_HEADER_LEN + body.len(), 0xa8, "the 7040's DMAR length");
        let u = DmarUnits::parse(&body);
        assert!(!u.truncated);
        assert_eq!(u.units().len(), 2);
        for (bdf, want) in [
            ((0, 2, 0), (0, Ownership::Named)),
            ((1, 0, 0), (1, Ownership::CatchAll)),
            ((0, 0x14, 0), (1, Ownership::CatchAll)),
            ((0, 0x1d, 0), (1, Ownership::CatchAll)),
            ((0, 0x1f, 6), (1, Ownership::CatchAll)),
        ] {
            let (b, d, f) = bdf;
            assert_eq!(
                u.owner_index(0, b, d, f, &mut xenon_bus),
                Ok(Some(want)),
                "{bdf:x?}"
            );
        }
    }

    /// **Each RMRR is handed to exactly the device it names**, and to nothing else: the USB
    /// controller gets its 128 KiB, the GPU its 40 MiB, the NVMe none. The GPU's region is the one
    /// whose absence would blank xenon's screen the instant its unit translated, which is why the
    /// kernel maps it before it sets `GCMD.TE`.
    #[test]
    fn on_the_7040s_table_each_rmrr_goes_to_the_device_it_names() {
        let u = DmarUnits::parse(&optiplex_7040_dmar());
        assert_eq!(u.reserved_regions().len(), 2);
        assert_eq!(u.rmrrs_refused, 0);
        assert_eq!(reserved(&u, 0, 0x14, 0), [(0xdb5d_2000, 0xdb5f_1fff)]);
        assert_eq!(reserved(&u, 0, 2, 0), [(0xdd80_0000, 0xdfff_ffff)]);
        assert_eq!(u.reserved_regions()[1].size(), 40 << 20);
        assert!(reserved(&u, 1, 0, 0).is_empty());
        assert!(reserved(&u, 0, 0x14, 1).is_empty(), "another function");
    }

    /// An RMRR naming a bridge as a sub-hierarchy covers the functions below it, the case Linux's
    /// `is_downstream_to_pci_bridge` handles.
    #[test]
    fn an_rmrr_naming_a_bridge_covers_what_is_below_it() {
        let u = DmarUnits::parse(&dmar(&[
            drhd(1, 0xfed9_1000, &[]),
            rmrr(
                0x7000_0000,
                0x7000_ffff,
                &[&scope(SCOPE_PCI_SUBHIERARCHY, 0, &[(0x1d, 0)])],
            ),
        ]));
        assert_eq!(reserved(&u, 1, 0, 0), [(0x7000_0000, 0x7000_ffff)]);
        assert!(reserved(&u, 0, 2, 0).is_empty());
    }

    /// **An RMRR that breaks section 8.4's shape is counted and dropped**, never mapped: an
    /// unaligned base, a size that is not whole pages, a limit at or below the base, and a limit
    /// of the top address (whose `+ 1` would wrap). Mapping any of them would widen a domain by
    /// memory the firmware never declared.
    #[test]
    fn a_malformed_rmrr_is_refused_rather_than_mapped() {
        let s = scope(SCOPE_PCI_ENDPOINT, 0, &[(2, 0)]);
        for (base, limit) in [
            (0x1000_0800, 0x1000_ffff),
            (0x1000_0000, 0x1000_07ff),
            (0x1000_0000, 0x1000_0000),
            (0x2000_0000, 0x1000_0fff),
            (0xffff_ffff_ffff_f000, u64::MAX),
        ] {
            let u = DmarUnits::parse(&dmar(&[
                drhd(1, 0xfed9_1000, &[]),
                rmrr(base, limit, &[&s]),
            ]));
            assert_eq!(u.rmrrs_refused, 1, "{base:#x}..={limit:#x}");
            assert!(u.reserved_regions().is_empty());
            assert!(!u.truncated, "a refused region is not a truncated table");
        }
    }

    /// A ninth RMRR marks the table rather than vanishing, the same rule a ninth DRHD follows.
    #[test]
    fn a_ninth_rmrr_marks_the_table_truncated() {
        let s = scope(SCOPE_PCI_ENDPOINT, 0, &[(2, 0)]);
        let mut parts = std::vec![drhd(1, 0xfed9_1000, &[])];
        for i in 0..=MAX_RMRRS as u64 {
            parts.push(rmrr(
                0x1000_0000 + i * 0x1000,
                0x1000_0fff + i * 0x1000,
                &[&s],
            ));
        }
        let u = DmarUnits::parse(&dmar(&parts));
        assert_eq!(u.reserved_regions().len(), MAX_RMRRS);
        assert!(u.truncated);
    }

    /// **The register file is as big as the DRHD's `Size` byte says**: zero is one page (firmware
    /// that predates the field), three is eight pages. The kernel maps this much, not one page.
    #[test]
    fn a_drhds_size_byte_gives_its_register_span() {
        let mut body = dmar(&[drhd(1, 0xfed9_0000, &[])]);
        assert_eq!(DmarUnits::parse(&body).units()[0].register_size, 4096);
        body[DMAR_FIXED_LEN + 5] = 3;
        assert_eq!(DmarUnits::parse(&body).units()[0].register_size, 8 * 4096);
        body[DMAR_FIXED_LEN + 5] = 0xf3; // bits 7:4 are reserved and ignored
        assert_eq!(DmarUnits::parse(&body).units()[0].register_size, 8 * 4096);
    }

    // ---- survivor triage of the 2026-10-03 census ---------------------------------------------

    /// A structure of the smallest legal length (4: a type and a length) is skipped, and what comes
    /// after it is still read. A structure that claims more bytes than the table has ends the walk
    /// instead of slicing past it.
    #[test]
    fn dmar_walks_past_a_four_byte_structure_and_stops_at_one_that_overruns() {
        let tiny = [5u8, 0, 4, 0];
        let body = dmar(&[tiny.to_vec(), drhd(1, 0xfed9_1000, &[])]);
        let u = DmarUnits::parse(&body);
        assert_eq!(u.units().len(), 1);
        assert_eq!(u.units()[0].register_base, 0xfed9_1000);

        let mut body = dmar(&[drhd(1, 0xfed9_1000, &[])]);
        body.extend_from_slice(&[5, 0, 100, 0]);
        let u = DmarUnits::parse(&body);
        assert_eq!(u.units().len(), 1);
    }

    /// A DRHD or RMRR too short to hold its fixed part is not read as one. Reading it would index
    /// past what the table gave.
    #[test]
    fn a_dmar_structure_shorter_than_its_fixed_part_is_ignored() {
        let body = dmar(&[
            [0u8, 0, 4, 0].to_vec(),
            [1u8, 0, 4, 0].to_vec(),
            drhd(1, 0xfed9_1000, &[]),
        ]);
        let u = DmarUnits::parse(&body);
        assert_eq!(u.units().len(), 1);
        assert_eq!(u.reserved_regions().len(), 0);
    }

    /// A scope's path is bounded on both sides: none is nothing, four is the longest, and what
    /// `parse` records and `resolve` reads agree on where the limit is.
    #[test]
    fn a_scope_path_resolves_up_to_its_limit_and_no_further() {
        let mut s = DeviceScope {
            start_bus: 3,
            ..DeviceScope::default()
        };
        let direct = &mut |bus: u8, _: u8, _: u8| Some((bus + 1, 0));
        assert_eq!(s.resolve(direct), None, "an empty path names nothing");
        s.path_len = MAX_SCOPE_PATH as u8;
        for (i, hop) in s.path.iter_mut().enumerate() {
            *hop = (i as u8, 0);
        }
        // Three bridges are crossed (3 -> 4 -> 5 -> 6) and the last hop is the device on bus 6.
        assert_eq!(s.resolve(direct), Some((6, MAX_SCOPE_PATH as u8 - 1, 0)));
        s.path_len = MAX_SCOPE_PATH as u8 + 1;
        assert_eq!(s.resolve(direct), None);
    }

    /// A scope list is read hop by hop and keeps everything it was given: the starting bus, the
    /// path at every depth, and a path exactly as long as the limit.
    #[test]
    fn a_scope_keeps_its_start_bus_and_every_hop() {
        let path = [(0x1d, 3), (5, 7), (9, 11), (13, 15)];
        let u = DmarUnits::parse(&dmar(&[drhd(
            1,
            0xfed9_1000,
            &[&scope(SCOPE_PCI_ENDPOINT, 0x09, &path)],
        )]));
        assert!(!u.truncated);
        assert_eq!(u.scope_count, 1);
        assert_eq!(u.scopes[0].start_bus, 0x09);
        assert_eq!(u.scopes[0].path_len as usize, MAX_SCOPE_PATH);
        assert_eq!(u.scopes[0].path, path);
    }

    /// A scope with no hops, or with one more than the limit, is marked and not recorded, and the
    /// entries after it are still read.
    #[test]
    fn a_scope_with_no_hops_or_too_many_is_marked_and_the_list_goes_on() {
        let good = scope(SCOPE_PCI_ENDPOINT, 0, &[(2, 0)]);
        for bad in [
            scope(SCOPE_PCI_ENDPOINT, 0, &[]),
            scope(
                SCOPE_PCI_ENDPOINT,
                0,
                &[(1, 0), (2, 0), (3, 0), (4, 0), (5, 0)],
            ),
        ] {
            let u = DmarUnits::parse(&dmar(&[drhd(1, 0xfed9_1000, &[&bad, &good])]));
            assert!(u.truncated);
            assert_eq!(u.scope_count, 1, "only the good one is kept");
            assert_eq!(u.scopes[0].path[0], (2, 0));
        }
    }
}
