//! **The MADT** (signature `APIC`): where the interrupt controllers are, and how the legacy ISA
//! IRQs reach them once its interrupt source overrides are applied.
//!
//! Name: provisional (milestone 875's lane, 2026-10-12 UTC).

use super::*;

/// The MADT's fixed part, which precedes its variable-length entry list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Madt {
    /// The local APIC's physical address as the 32-bit field states it. A
    /// [`MadtEntry::LocalApicAddressOverride`] later in the list replaces it, which is why this is
    /// not the final answer on every machine.
    pub local_apic: u32,
    /// Bit 0 (`PCAT_COMPAT`) means the machine also has 8259 PICs. See the `acpi` module's BUGS.
    pub flags: u32,
}

/// True when the machine has legacy 8259 PICs that must be masked before the APICs are used.
pub const MADT_PCAT_COMPAT: u32 = 1 << 0;

/// The eight bytes of the MADT that precede its entries.
pub(super) const MADT_FIXED_LEN: usize = 8;

/// Decode the MADT's fixed part. `body` begins after the SDT header.
pub fn parse_madt(body: &[u8]) -> Result<Madt, AcpiError> {
    if body.len() < MADT_FIXED_LEN {
        return Err(AcpiError::Truncated);
    }
    Ok(Madt {
        local_apic: u32(body, 0),
        flags: u32(body, 4),
    })
}

/// One entry of the MADT's list. Only the kinds this tree acts on are decoded, four for x86 and
/// five for aarch64; the rest keep their type byte so a boot print can say what it skipped rather
/// than pretending the list was shorter than it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MadtEntry {
    /// A CPU, named by its local APIC id. **`enabled` is what decides whether it can be started**:
    /// a processor entry exists for sockets that are empty and for cores the firmware disabled.
    LocalApic {
        processor_id: u8,
        apic_id: u8,
        /// The CPU is usable now.
        enabled: bool,
        /// The CPU is not enabled but could be brought online later (hot-plug). Not the same as
        /// `enabled`, and starting one that is merely online-capable is a different operation.
        online_capable: bool,
    },
    /// An IO APIC: where device interrupts arrive. `gsi_base` is the first global system interrupt
    /// this one owns, which is how a machine with several of them divides the space.
    IoApic { id: u8, address: u32, gsi_base: u32 },
    /// **A legacy IRQ has been rewired.** The ISA interrupt `source` actually arrives as global
    /// system interrupt `gsi`, with `flags` giving polarity and trigger mode. This is the entry that
    /// makes "COM1 is IRQ 4" not mean "COM1 is IO APIC input 4": on almost every machine the timer's
    /// IRQ 0 is remapped, and reading this wrong arms the wrong line.
    InterruptSourceOverride {
        bus: u8,
        source: u8,
        gsi: u32,
        flags: u16,
    },
    /// The local APIC is not at the 32-bit address the fixed part gave; it is here.
    LocalApicAddressOverride(u64),
    /// **An aarch64 core** (type 11, "GIC CPU Interface"), which is this architecture's
    /// [`MadtEntry::LocalApic`]: the per-core entry, one per processor, carrying the id a bring-up
    /// call has to name. There is no local APIC on an Arm machine, so a decoder that knew only the
    /// x86 types read every core of an aarch64 server as [`MadtEntry::Other`] and found none.
    GenericInterruptController {
        /// `MPIDR_EL1[39:0]`, the affinity value PSCI's `CPU_ON` takes. The device tree spells the
        /// same number in a `cpu@` node's `reg`, which is why `cpu_list::Cpu::hwid` needs no second
        /// meaning for this path.
        mpidr: u64,
        /// The processor's ACPI UID, which is what the DSDT's `Processor` objects refer to. Kept
        /// because it is the only handle AML has on this core, not because anything reads it yet.
        uid: u32,
        /// The core is usable now. Same meaning, and same consequence, as
        /// [`MadtEntry::LocalApic`]'s.
        enabled: bool,
        /// Not enabled, but able to be brought online later. See [`MadtEntry::LocalApic`].
        online_capable: bool,
        /// The GICv2 CPU interface (`GICC`) for this core, zero on a GICv3 machine where the CPU
        /// interface is system registers rather than memory.
        cpu_interface: u64,
        /// This core's GICv3 redistributor frame, zero when the machine instead states its
        /// redistributors as [`MadtEntry::GenericRedistributor`] ranges. Both spellings are legal
        /// and QEMU uses the second.
        redistributor: u64,
    },
    /// **The GIC distributor** (type 12), one per machine, and the entry that says which GIC
    /// architecture version this is. A device tree says the same thing through `compatible`, which
    /// is why `version` is the field a tree writer needs.
    GenericDistributor {
        /// `GICD`, physical.
        address: u64,
        /// 0 unspecified, 1 `GICv1`, 2 `GICv2`, 3 `GICv3`, 4 `GICv4`. **Zero is common and means "work it
        /// out"**, which a caller can only do from the other entries.
        version: u8,
    },
    /// **A range of GICv3 redistributor frames** (type 14), the contiguous array `GICR_TYPER` is
    /// then walked over. The alternative spelling is a per-core `redistributor` in
    /// [`MadtEntry::GenericInterruptController`]; a machine states one or the other.
    GenericRedistributor {
        /// The first frame, physical.
        address: u64,
        /// How many bytes of frames, all cores' together.
        length: u32,
    },
    /// **A `GICv2m` MSI frame** (type 13, "GIC MSI Frame"): the block a device writes to raise a
    /// message-signaled interrupt on a GIC that has no ITS, which turns each write into one of a
    /// range of SPIs. A device tree states it as an `arm,gic-v2m-frame` child of the GIC node. QEMU
    /// `virt` offers one on every GICv2 machine and, with `msi=gicv2m`, on a GICv3 one too.
    GicMsiFrame {
        /// The frame's ID, which IORT and the DSDT use to name this frame.
        id: u32,
        /// The frame, physical. Its length is architectural (4 KiB) and the entry does not state it.
        address: u64,
        /// The SPIs this frame raises, when the entry states them. `None` means flag bit 0 is clear,
        /// and **the frame's own `MSI_TYPER` register is the answer**: the entry's count and base
        /// fields are then not meaningful, so they are not offered.
        spis: Option<MsiSpis>,
    },
    /// **A GICv3 Interrupt Translation Service** (type 15, "GIC ITS"): the device that turns a
    /// message-signaled write, tagged with the writer's device ID, into an LPI through tables the
    /// kernel owns. It is where MSI confinement lives on aarch64 (milestone 317 (the
    /// interrupt-remapping flags, and where MSI confinement actually lives)). A device tree states it
    /// as an `arm,gic-v3-its` child of the GIC node. QEMU `virt` offers one with `gic-version=3`
    /// under TCG, and none under HVF or on a GICv2.
    GicIts {
        /// The ITS's ID, which IORT uses to route a device's MSIs to this ITS.
        id: u32,
        /// `GITS_CTLR`'s frame, physical. Its length (two 64 KiB frames) is architectural and the
        /// entry does not state it.
        address: u64,
    },
    /// A kind this decoder does not act on, with its type byte.
    Other(u8),
}

/// The SPI range a [`MadtEntry::GicMsiFrame`] states in place of its `MSI_TYPER` register.
///
/// Name: provisional (the lane of milestone 578 (an ACPI discovery path for aarch64), 2026-10-19
/// UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsiSpis {
    /// The first SPI's INTID, which is at least 32: INTIDs below that are SGIs and PPIs.
    pub base: u16,
    /// How many consecutive SPIs, from `base`.
    pub count: u16,
}

/// Flag bit 0 of a GIC MSI frame entry: its SPI count and base fields override `MSI_TYPER`.
const MSI_FRAME_SPI_SELECT: u32 = 1 << 0;

/// Walks a MADT's entry list. Each entry is `[type, length, ...]`, so the list is self-describing
/// and a length of zero would loop forever; that is refused by ending the walk.
pub struct MadtEntries<'a> {
    walk: EntryWalk<'a>,
}

/// Iterate the MADT's entries. `body` begins after the SDT header.
pub fn madt_entries(body: &[u8]) -> MadtEntries<'_> {
    MadtEntries {
        walk: entry_walk(body),
    }
}

/// The walk without the decode: each entry's type byte and its bytes. It is separate so a reader
/// that wants one kind can skip decoding the others, and that is a proof cost rather than a
/// runtime one: CBMC encodes every arm of [`MadtEntries`]'s `match` at every unwound step, so a
/// harness over [`isa_irq_table`] would otherwise pay for each GIC kind added to it.
struct EntryWalk<'a> {
    body: &'a [u8],
    at: usize,
}

fn entry_walk(body: &[u8]) -> EntryWalk<'_> {
    EntryWalk {
        body,
        at: MADT_FIXED_LEN,
    }
}

impl<'a> Iterator for EntryWalk<'a> {
    type Item = (u8, &'a [u8]);

    fn next(&mut self) -> Option<(u8, &'a [u8])> {
        if self.at + 2 > self.body.len() {
            return None;
        }
        let kind = self.body[self.at];
        let len = self.body[self.at + 1] as usize;
        // A zero (or absurdly short) length is a malformed table, and continuing would either loop
        // forever or read the next entry from the middle of this one. Stop, which reports the
        // entries read so far rather than inventing more.
        if len < 2 || self.at + len > self.body.len() {
            return None;
        }
        let e = &self.body[self.at..self.at + len];
        self.at += len;
        Some((kind, e))
    }
}

impl Iterator for MadtEntries<'_> {
    type Item = MadtEntry;

    fn next(&mut self) -> Option<MadtEntry> {
        let (kind, e) = self.walk.next()?;
        let len = e.len();
        Some(match kind {
            0 if len >= 8 => {
                let flags = u32(e, 4);
                MadtEntry::LocalApic {
                    processor_id: e[2],
                    apic_id: e[3],
                    enabled: flags & 1 != 0,
                    online_capable: flags & 2 != 0,
                }
            }
            1 if len >= 12 => MadtEntry::IoApic {
                id: e[2],
                address: u32(e, 4),
                gsi_base: u32(e, 8),
            },
            2 if len >= 10 => interrupt_source_override(e),
            5 if len >= 12 => MadtEntry::LocalApicAddressOverride(u64(e, 4)),
            // The three aarch64 kinds. Their lengths have grown across ACPI revisions (a GICC
            // entry was 40 bytes in 5.0, 76 in 5.1, 80 in 6.0 and 82 in 6.3), so every field is
            // taken behind a length check for the offset it sits at rather than behind one check
            // of the whole entry: a firmware writing an older, shorter revision still yields the
            // fields it does carry instead of falling off the list as `Other`.
            11 if len >= 16 => MadtEntry::GenericInterruptController {
                uid: u32(e, 8),
                enabled: u32(e, 12) & 1 != 0,
                online_capable: u32(e, 12) & 2 != 0,
                cpu_interface: if len >= 40 { u64(e, 32) } else { 0 },
                redistributor: if len >= 68 { u64(e, 60) } else { 0 },
                mpidr: if len >= 76 { u64(e, 68) } else { 0 },
            },
            12 if len >= 24 => MadtEntry::GenericDistributor {
                address: u64(e, 8),
                version: e[20],
            },
            14 if len >= 16 => MadtEntry::GenericRedistributor {
                address: u64(e, 4),
                length: u32(e, 12),
            },
            // One length in every revision that has the type (ACPI 5.1 onward): 24 bytes.
            13 if len >= 24 => MadtEntry::GicMsiFrame {
                id: u32(e, 4),
                address: u64(e, 8),
                spis: (u32(e, 16) & MSI_FRAME_SPI_SELECT != 0).then(|| MsiSpis {
                    count: u16(e, 20),
                    base: u16(e, 22),
                }),
            },
            // 20 bytes. ACPI 6.0 printed a wrong length for this structure and its Errata A corrected
            // it, which is the revision QEMU cites beside the 20 it writes.
            15 if len >= 20 => MadtEntry::GicIts {
                id: u32(e, 4),
                address: u64(e, 8),
            },
            other => MadtEntry::Other(other),
        })
    }
}

/// Decode a type-2 entry, which the caller has checked is at least 10 bytes.
fn interrupt_source_override(e: &[u8]) -> MadtEntry {
    MadtEntry::InterruptSourceOverride {
        bus: e[2],
        source: e[3],
        gsi: u32(e, 4),
        flags: u16(e, 8),
    }
}

// ---------------------------------------------------------------------------------------------
// Legacy IRQ numbers, and the overrides that make them a lie.
// ---------------------------------------------------------------------------------------------

/// **The polarity field of an MPS INTI flags word**, bits 1:0. `00` means "whatever this bus
/// specifies", `01` active high, `11` active low. `10` is reserved.
pub const INTI_POLARITY_MASK: u16 = 0b11;
/// Polarity `11`: the line is asserted low.
const INTI_POLARITY_ACTIVE_LOW: u16 = 0b11;

/// **The trigger-mode field**, bits 3:2. `00` means "whatever this bus specifies", `01` edge, `11`
/// level. `10` is reserved.
pub const INTI_TRIGGER_MASK: u16 = 0b11 << 2;
/// Trigger `11`: the line stays asserted until the device is serviced.
const INTI_TRIGGER_LEVEL: u16 = 0b11 << 2;

/// The bus number an [`MadtEntry::InterruptSourceOverride`] uses for the ISA bus. It is the only
/// value ACPI defines for that field, which is why the overrides are exactly the legacy IRQs.
pub const ISA_BUS: u8 = 0;

/// How many legacy ISA IRQs there are: two cascaded 8259s, eight lines each.
pub const ISA_IRQ_COUNT: usize = 16;

/// **How one ISA interrupt actually reaches an IO APIC.**
///
/// The whole point of this type is that `gsi` is very often *not* the IRQ number it was looked up
/// by. On essentially every PC the timer's IRQ 0 arrives as global system interrupt 2, because the
/// PIT is wired to the IO APIC's pin 2 while pin 0 carries the 8259 cascade. A kernel that armed
/// redirection entry 0 for "the timer" would arm a line nothing drives, and would see no
/// interrupts and no error.
///
/// Name: provisional (milestone 161 (the kernel port)), along with the two fields and
/// [`isa_irq_table`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IsaIrqRouting {
    /// The global system interrupt this IRQ arrives on.
    pub gsi: u32,
    /// The line is asserted low rather than high.
    pub active_low: bool,
    /// The line is level triggered rather than edge triggered.
    pub level_triggered: bool,
}

impl IsaIrqRouting {
    /// **What an ISA IRQ is when nothing overrides it**: identity-mapped onto the global interrupt
    /// space, active high, edge triggered. That is the ISA bus's own convention, which is what the
    /// `00` ("conforms to the specifications of the bus") encoding in an override's flags means.
    pub const fn isa_default(irq: u8) -> Self {
        Self {
            gsi: irq as u32,
            active_low: false,
            level_triggered: false,
        }
    }

    /// Apply an override's MPS INTI flags word. `00` in either field means "conforms to the bus",
    /// which for the ISA bus is what [`isa_default`](Self::isa_default) already set, so those bits
    /// deliberately change nothing.
    const fn with_flags(mut self, flags: u16) -> Self {
        if flags & INTI_POLARITY_MASK == INTI_POLARITY_ACTIVE_LOW {
            self.active_low = true;
        }
        if flags & INTI_TRIGGER_MASK == INTI_TRIGGER_LEVEL {
            self.level_triggered = true;
        }
        self
    }
}

/// **Resolve all sixteen legacy ISA IRQs through the MADT's interrupt source overrides.** `body`
/// begins after the SDT header.
///
/// Returned as a whole table rather than one lookup at a time because the overrides are a list that
/// has to be walked to answer any single question, and the caller (an interrupt controller being
/// brought up) wants the answers to outlive the table's bytes.
///
/// An override naming a source outside 0..16 is ignored: the field is a legacy IRQ number and there
/// are sixteen of those, so a larger one is a malformed table rather than a seventeenth IRQ.
pub fn isa_irq_table(body: &[u8]) -> [IsaIrqRouting; ISA_IRQ_COUNT] {
    let mut table = core::array::from_fn(|irq| IsaIrqRouting::isa_default(irq as u8));
    // A plain loop over the walk, not `filter`: `Filter::next` loops inside each step, which nests a
    // second loop under the proof's unwind bound and multiplies what CBMC unrolls.
    for (kind, e) in entry_walk(body) {
        if kind != 2 || e.len() < 10 {
            continue;
        }
        let entry = interrupt_source_override(e);
        if let MadtEntry::InterruptSourceOverride {
            bus,
            source,
            gsi,
            flags,
        } = entry
            && bus == ISA_BUS
            && (source as usize) < ISA_IRQ_COUNT
        {
            table[source as usize] = IsaIrqRouting {
                gsi,
                active_low: false,
                level_triggered: false,
            }
            .with_flags(flags);
        }
    }
    table
}

/// Machine-checked proofs over the MADT (milestone 319). Why these tables carry harnesses is on
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

    /// **The MADT walk terminates and never reads outside the body**, for every byte pattern. Each
    /// entry's length byte is firmware telling the parser how far to advance: zero never advances,
    /// and a length past the end reads the next entry out of the middle of this one. The unwind
    /// bound is what proves termination. `len < 1` for `len < 2`, or `>=` for `>` in the bound
    /// check, keeps every test green.
    /// Falsification: replayable `crates/machine_discovery/falsifications/acpi.madt.verification.the_madt_walk_terminates_and_stays_inside_the_body.patch`
    #[kani::proof]
    #[kani::unwind(20)]
    fn the_madt_walk_terminates_and_stays_inside_the_body() {
        let body: [u8; N] = kani::any();
        let mut it = madt_entries(&body);
        let mut seen = 0usize;
        let mut last = it.walk.at;
        while it.next().is_some() {
            // Every step consumes at least the two bytes of its own header, which is what makes the
            // walk finite; without it this loop would not unroll.
            assert!(it.walk.at >= last + 2);
            assert!(it.walk.at <= body.len());
            last = it.walk.at;
            seen += 1;
        }
        assert!(seen <= N / 2);
    }

    /// **No interrupt source override can write outside the sixteen legacy IRQs.**
    ///
    /// [`isa_irq_table`] indexes a fixed sixteen-entry array with `source`, a byte straight out of
    /// the table, and the only thing between firmware writing `source = 200` and an out-of-bounds
    /// store is one `<` in a chained `if let`. This proves it holds for every body.
    ///
    /// Could plausibly have been false: the guard sits in the middle of a four-clause `let`-chain
    /// added in one commit, and a reader rewriting that chain as a `match` on the entry has to carry
    /// it across by hand. The existing test covers `source = 200` and nothing else.
    /// Falsification: replayable `crates/machine_discovery/falsifications/acpi.madt.verification.no_override_writes_outside_the_sixteen_legacy_irqs.patch`
    #[kani::proof]
    #[kani::unwind(20)]
    fn no_override_writes_outside_the_sixteen_legacy_irqs() {
        let body: [u8; N] = kani::any();
        let table = isa_irq_table(&body);
        // The claim as an assertion, because an unsatisfied `cover!` leaves a Kani harness green
        // (Kani 0.67.0, found by the printenv harness): an IRQ no ISA override names keeps the ISA
        // bus's own convention, so no override wrote anywhere it did not name. The out-of-bounds
        // store the doc names is the other half, and CBMC's bounds check on `table[source]` catches
        // it on its own.
        let mut named: u32 = 0;
        for (kind, e) in entry_walk(&body) {
            if kind != 2 || e.len() < 10 {
                continue;
            }
            if let MadtEntry::InterruptSourceOverride { bus, source, .. } =
                interrupt_source_override(e)
                && bus == ISA_BUS
                && (source as usize) < ISA_IRQ_COUNT
            {
                named |= 1 << source;
            }
        }
        for (irq, routing) in table.iter().enumerate() {
            if named & (1 << irq) == 0 {
                assert_eq!(*routing, IsaIrqRouting::isa_default(irq as u8));
            }
        }
        kani::cover!(
            table[0] != IsaIrqRouting::isa_default(0),
            "some body overrides IRQ 0, which is the case every PC exercises"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acpi::tests::q35_madt_body;

    extern crate std;

    /// **An aarch64 MADT is read as cores and a GIC, where an x86-only decoder saw nothing.**
    ///
    /// The body is QEMU `virt`'s shape with `acpi=on`: a GIC distributor stating version 3, a
    /// redistributor range, and two GIC CPU interface entries carrying their MPIDRs. Before this
    /// decoder knew the three types, every one of them came back as [`MadtEntry::Other`], which is
    /// the same answer a machine with no cores and no interrupt controller would give.
    #[test]
    fn an_aarch64_madt_names_its_gic_and_its_cpus() {
        let mut body = [0u8; 128];
        body[0..4].copy_from_slice(&0u32.to_le_bytes()); // no local APIC address
        body[4..8].copy_from_slice(&0u32.to_le_bytes()); // no PCAT_COMPAT
        // GIC distributor (type 12, 24 bytes).
        let d = &mut body[8..32];
        d[0] = 12;
        d[1] = 24;
        d[8..16].copy_from_slice(&0x0800_0000u64.to_le_bytes());
        d[20] = 3;
        // GIC redistributor (type 14, 16 bytes).
        let r = &mut body[32..48];
        r[0] = 14;
        r[1] = 16;
        r[4..12].copy_from_slice(&0x080a_0000u64.to_le_bytes());
        r[12..16].copy_from_slice(&0x00f6_0000u32.to_le_bytes());
        // Two GIC CPU interfaces at ACPI 5.0's length, which is 40 bytes and stops short of the
        // MPIDR field that 5.1 added. A firmware writing this revision is exactly the case the
        // per-offset length guards exist for.
        for (i, at) in [48usize, 88].into_iter().enumerate() {
            let c = &mut body[at..at + 40];
            c[0] = 11;
            c[1] = 40;
            c[8..12].copy_from_slice(&(i as u32).to_le_bytes()); // uid
            c[12..16].copy_from_slice(&1u32.to_le_bytes()); // enabled
        }

        let mut entries = [MadtEntry::Other(0); 8];
        let mut count = 0;
        for e in madt_entries(&body) {
            entries[count] = e;
            count += 1;
        }
        assert_eq!(count, 4, "two controller entries and two cores");
        assert_eq!(
            entries[0],
            MadtEntry::GenericDistributor {
                address: 0x0800_0000,
                version: 3
            }
        );
        assert_eq!(
            entries[1],
            MadtEntry::GenericRedistributor {
                address: 0x080a_0000,
                length: 0x00f6_0000
            }
        );
        for entry in &entries[2..4] {
            let MadtEntry::GenericInterruptController { enabled, mpidr, .. } = entry else {
                panic!("a GIC CPU interface entry, got {entry:?}");
            };
            assert!(enabled, "both cores are enabled");
            // A 40-byte entry is too short to carry the MPIDR field, and the decoder answers zero
            // for the fields the firmware's revision does not have rather than dropping the entry.
            assert_eq!(*mpidr, 0);
        }
    }

    /// **QEMU's own `GICv2m` frame entry, byte for byte**, as `virt` writes it with
    /// `gic-version=3,msi=gicv2m`: the last 24 bytes of QEMU's checked-in
    /// `tests/data/acpi/aarch64/virt/APIC.msi_gicv2m` (QEMU 3f0043f9, read 2026-10-19 UTC), which
    /// `hw/arm/virt-acpi-build.c` builds as frame 0 at `VIRT_GIC_V2M`, flag 1, 64 SPIs from 80.
    /// Written from the specification alone, the count and base fields are the easy pair to swap,
    /// and both are plausible INTIDs.
    #[test]
    fn qemus_gicv2m_frame_entry_decodes_to_its_frame_and_spis() {
        let mut body = std::vec![0u8; MADT_FIXED_LEN];
        body.extend_from_slice(&[
            0x0d, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x08, 0x00, 0x00,
            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x40, 0x00, 0x50, 0x00,
        ]);
        assert_eq!(
            madt_entries(&body).collect::<std::vec::Vec<_>>(),
            std::vec![MadtEntry::GicMsiFrame {
                id: 0,
                address: 0x0802_0000,
                spis: Some(MsiSpis {
                    base: 80,
                    count: 64
                }),
            }]
        );
    }

    /// **QEMU's own ITS entry, byte for byte**, as `virt` writes it with `acpi=on,gic-version=3`
    /// under TCG. None of QEMU's checked-in `tests/data/acpi/aarch64/virt/APIC*` tables holds one,
    /// so this is the MADT that QEMU 11.1.2 installed under its EDK2, read out of guest RAM with
    /// `pmemsave` (2026-10-19 UTC): ITS 0 at `VIRT_GIC_ITS`. The address sits at offset 8, past four
    /// bytes of ID and two reserved, so a decoder that read it at 4 (as the redistributor's is)
    /// would see the ID's zeros and half the address.
    #[test]
    fn qemus_its_entry_decodes_to_its_id_and_frame() {
        let mut body = std::vec![0u8; MADT_FIXED_LEN];
        body.extend_from_slice(&[
            0x0f, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x08, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ]);
        assert_eq!(
            madt_entries(&body).collect::<std::vec::Vec<_>>(),
            std::vec![MadtEntry::GicIts {
                id: 0,
                address: 0x0808_0000,
            }]
        );
    }

    /// With flag bit 0 clear the entry's SPI fields are not the answer, `MSI_TYPER` is, so a frame
    /// that left stale numbers in them must not have them read as its range.
    #[test]
    fn an_msi_frame_without_the_select_flag_offers_no_spis() {
        let mut body = std::vec![0u8; MADT_FIXED_LEN];
        let mut e = [0u8; 24];
        e[0] = 13;
        e[1] = 24;
        e[8..16].copy_from_slice(&0x0802_0000u64.to_le_bytes());
        e[20..24].copy_from_slice(&[0x40, 0x00, 0x50, 0x00]);
        body.extend_from_slice(&e);
        assert!(matches!(
            madt_entries(&body).next(),
            Some(MadtEntry::GicMsiFrame { spis: None, .. })
        ));
    }

    /// A full-length (ACPI 6.0, 80-byte) GIC CPU interface entry yields the two addresses and the
    /// MPIDR, which is what a tree writer needs and what the shorter revisions cannot give.
    #[test]
    fn a_full_length_gic_cpu_interface_entry_yields_its_mpidr_and_frames() {
        let mut body = [0u8; 8 + 80];
        let c = &mut body[8..];
        c[0] = 11;
        c[1] = 80;
        c[12..16].copy_from_slice(&1u32.to_le_bytes());
        c[32..40].copy_from_slice(&0x0801_0000u64.to_le_bytes()); // GICC
        c[60..68].copy_from_slice(&0x080a_0000u64.to_le_bytes()); // GICR
        c[68..76].copy_from_slice(&0x0000_0001_0000_0003u64.to_le_bytes()); // MPIDR with Aff2 set
        assert_eq!(
            madt_entries(&body).next(),
            Some(MadtEntry::GenericInterruptController {
                uid: 0,
                enabled: true,
                online_capable: false,
                cpu_interface: 0x0801_0000,
                redistributor: 0x080a_0000,
                mpidr: 0x0000_0001_0000_0003,
            })
        );
    }

    /// The fixed part, and the PICs the machine still has.
    #[test]
    fn the_madt_reports_the_local_apic_and_whether_the_8259s_are_there() {
        let m = parse_madt(&q35_madt_body()).expect("well-formed");
        assert_eq!(m.local_apic, 0xfee0_0000);
        assert_ne!(
            m.flags & MADT_PCAT_COMPAT,
            0,
            "a PC still has 8259s and they must be masked before the APICs are used"
        );
    }

    /// **The entry list decodes, including the overrides that make legacy IRQ numbers a lie.** On
    /// almost every machine the timer's IRQ 0 arrives as GSI 2, and a kernel that armed input 0
    /// would arm nothing.
    #[test]
    fn the_madt_entries_decode_and_the_overrides_rewire_legacy_irqs() {
        let body = q35_madt_body();
        let mut it = madt_entries(&body);
        assert_eq!(
            it.next(),
            Some(MadtEntry::LocalApic {
                processor_id: 0,
                apic_id: 0,
                enabled: true,
                online_capable: false,
            })
        );
        assert_eq!(
            it.next(),
            Some(MadtEntry::IoApic {
                id: 0,
                address: 0xfec0_0000,
                gsi_base: 0,
            })
        );
        assert_eq!(
            it.next(),
            Some(MadtEntry::InterruptSourceOverride {
                bus: 0,
                source: 0,
                gsi: 2,
                flags: 0,
            }),
            "IRQ 0 is remapped on essentially every PC"
        );
        assert_eq!(
            it.next(),
            Some(MadtEntry::InterruptSourceOverride {
                bus: 0,
                source: 5,
                gsi: 5,
                flags: 0x000d,
            })
        );
        assert_eq!(it.next(), None);
    }

    /// **A malformed entry length ends the walk rather than looping forever.** A length of zero
    /// would never advance, and a length past the end would read the next entry from the middle of
    /// this one.
    #[test]
    fn a_zero_or_overlong_entry_length_ends_the_walk() {
        let mut body = q35_madt_body();
        body[9] = 0; // the first entry claims zero length
        assert_eq!(madt_entries(&body).count(), 0);

        let mut body = q35_madt_body();
        body[17] = 200; // the second entry claims to run past the table
        assert_eq!(
            madt_entries(&body).count(),
            1,
            "the entries before the bad one are still reported"
        );
    }

    /// A CPU that is present but not enabled is not startable, and online-capable is a third state
    /// rather than a synonym for enabled.
    #[test]
    fn a_disabled_processor_is_reported_as_disabled() {
        let mut body = q35_madt_body();
        body[12..16].copy_from_slice(&2u32.to_le_bytes()); // online-capable, not enabled
        let first = madt_entries(&body).next().expect("an entry");
        assert_eq!(
            first,
            MadtEntry::LocalApic {
                processor_id: 0,
                apic_id: 0,
                enabled: false,
                online_capable: true,
            }
        );
    }

    /// **The trap, resolved.** IRQ 0 is the timer and it is not IO APIC input 0; the override says
    /// it arrives as GSI 2. A kernel that armed redirection entry 0 would arm the 8259 cascade and
    /// see nothing at all, with no error anywhere to say why.
    #[test]
    fn the_timers_irq_0_resolves_to_gsi_2() {
        let table = isa_irq_table(&q35_madt_body());
        assert_eq!(
            table[0],
            IsaIrqRouting {
                gsi: 2,
                active_low: false,
                level_triggered: false,
            },
            "IRQ 0 is remapped on essentially every PC"
        );
    }

    /// An IRQ with no override keeps its number and the ISA bus's own polarity and trigger mode.
    #[test]
    fn an_irq_with_no_override_is_identity_mapped_edge_triggered_and_active_high() {
        let table = isa_irq_table(&q35_madt_body());
        for irq in [1usize, 4, 8, 15] {
            assert_eq!(
                table[irq],
                IsaIrqRouting::isa_default(irq as u8),
                "IRQ {irq} has no override in this MADT"
            );
        }
        assert_eq!(table[4].gsi, 4, "COM1 really is GSI 4 on a PC");
    }

    /// **The flags word is two two-bit fields, and reading it as two one-bit flags gets both
    /// wrong.** `0x000d` is `0b1101`: bits 1:0 are `01`, active *high*, and bits 3:2 are `11`,
    /// level triggered. A decoder that tested bit 1 for polarity and bit 3 for trigger would answer
    /// "active low, level" here, which is a redirection entry that never fires.
    #[test]
    fn the_inti_flags_are_two_two_bit_fields_not_two_bits() {
        let table = isa_irq_table(&q35_madt_body());
        assert_eq!(
            table[5],
            IsaIrqRouting {
                gsi: 5,
                active_low: false,
                level_triggered: true,
            },
        );
    }

    /// Active low is `11` in bits 1:0, and it is the encoding a PCI line uses.
    #[test]
    fn active_low_is_the_11_encoding() {
        let mut body = q35_madt_body();
        body[46..48].copy_from_slice(&0b1111u16.to_le_bytes());
        let table = isa_irq_table(&body);
        assert!(table[5].active_low);
        assert!(table[5].level_triggered);
    }

    /// **A `00` field means "conforms to the bus", not "active high, edge" by accident.** The two
    /// happen to coincide on the ISA bus, and the test exists so that a future reader changing the
    /// default cannot do it in only one of the two places.
    #[test]
    fn a_conforms_to_the_bus_override_keeps_the_isa_defaults_but_takes_the_new_gsi() {
        let mut body = q35_madt_body();
        // Rewrite the second override: IRQ 9 -> GSI 9, flags 0 (conforms).
        body[41] = 9;
        body[42..46].copy_from_slice(&9u32.to_le_bytes());
        body[46..48].copy_from_slice(&0u16.to_le_bytes());
        let table = isa_irq_table(&body);
        assert_eq!(table[9], IsaIrqRouting::isa_default(9));
        assert_eq!(
            table[5],
            IsaIrqRouting::isa_default(5),
            "IRQ 5 is back to itself"
        );
    }

    /// An override naming a source outside the sixteen legacy IRQs is a malformed table, not a
    /// seventeenth IRQ, and must not index past the end of the table.
    #[test]
    fn an_override_for_a_source_above_15_is_ignored() {
        let mut body = q35_madt_body();
        body[41] = 200;
        let table = isa_irq_table(&body);
        for (irq, routing) in table.iter().enumerate().skip(1) {
            assert_eq!(*routing, IsaIrqRouting::isa_default(irq as u8));
        }
        assert_eq!(table[0].gsi, 2, "the well-formed override still applied");
    }

    /// **Only a type-2 entry rewires a legacy IRQ.** An IO APIC entry with id 0 has a zero at
    /// both of the offsets an override keeps its bus and source in, so read as an override it
    /// would send IRQ 0 to the IO APIC's address. Id 0 is what a single-IO-APIC PC usually has.
    #[test]
    fn an_io_apic_entry_is_never_read_as_an_override() {
        let mut body = [0u8; MADT_FIXED_LEN + 12];
        body[MADT_FIXED_LEN..].copy_from_slice(&[1, 12, 0, 0, 0x00, 0x00, 0xC0, 0xFE, 0, 0, 0, 0]);
        let table = isa_irq_table(&body);
        for (irq, routing) in table.iter().enumerate() {
            assert_eq!(*routing, IsaIrqRouting::isa_default(irq as u8));
        }
    }

    /// **A type-2 entry too short to hold an override's flags is skipped, not read past.** The
    /// override's last field ends at byte 10; an 8-byte one names a bus and a source but its
    /// flags would come from whatever follows it in the list.
    #[test]
    fn a_short_override_leaves_the_isa_table_alone() {
        let mut body = [0u8; MADT_FIXED_LEN + 8];
        body[MADT_FIXED_LEN..].copy_from_slice(&[2, 8, ISA_BUS, 3, 9, 0, 0, 0]);
        let table = isa_irq_table(&body);
        for (irq, routing) in table.iter().enumerate() {
            assert_eq!(*routing, IsaIrqRouting::isa_default(irq as u8));
        }
    }

    /// **The shortest entry the MADT's list can hold is still read.**
    ///
    /// Two bytes, a type and a length, and the list ends exactly where the body does. Every other
    /// MADT here has entries this decoder acts on, which are eight bytes and up, so the walk's own
    /// bounds have only ever been asked about lengths with room to spare either side.
    #[test]
    fn the_shortest_entry_the_list_can_hold_is_read_to_the_last_byte() {
        let mut body = [0u8; MADT_FIXED_LEN + 2];
        body[MADT_FIXED_LEN] = 9; // a type this decoder does not act on
        body[MADT_FIXED_LEN + 1] = 2; // the shortest self-describing length there is

        let mut it = madt_entries(&body);
        assert_eq!(it.next(), Some(MadtEntry::Other(9)));
        assert_eq!(it.next(), None);
    }

    /// **An entry of a kind this decoder reads, too short to hold that kind's fields, is reported
    /// by its type rather than decoded.**
    ///
    /// The four guards are the only thing between a malformed table and a read past the entry: a
    /// four-byte processor entry has no flags word, and decoding one anyway reads whatever follows
    /// it in the list. Firmware writes these lengths; nothing else checks them.
    #[test]
    fn an_entry_too_short_for_its_own_kind_is_not_decoded() {
        for kind in [0u8, 1, 2, 5] {
            let mut body = [0u8; MADT_FIXED_LEN + 4];
            body[MADT_FIXED_LEN] = kind;
            body[MADT_FIXED_LEN + 1] = 4;

            let mut it = madt_entries(&body);
            assert_eq!(
                it.next(),
                Some(MadtEntry::Other(kind)),
                "a four-byte entry of type {kind} holds none of that type's fields",
            );
            assert_eq!(it.next(), None);
        }
    }

    /// **The local APIC address override decodes**, which is the one entry kind nothing else here
    /// reads. A machine whose local APIC sits above 4 GiB can say so only in this entry, because
    /// the fixed part's field is 32 bits wide; a decoder that reported it as an unrecognised type
    /// would use the 32-bit address and touch memory that is not the APIC.
    #[test]
    fn a_local_apic_address_override_carries_a_64_bit_address() {
        let mut body = [0u8; MADT_FIXED_LEN + 12];
        body[MADT_FIXED_LEN] = 5;
        body[MADT_FIXED_LEN + 1] = 12;
        body[MADT_FIXED_LEN + 4..MADT_FIXED_LEN + 12]
            .copy_from_slice(&0x0000_0001_fee0_0000u64.to_le_bytes());

        let mut it = madt_entries(&body);
        assert_eq!(
            it.next(),
            Some(MadtEntry::LocalApicAddressOverride(0x1_fee0_0000)),
        );
        assert_eq!(it.next(), None);
    }

    /// **An override naming source 16 is ignored**, and 16 is the number that has to be exact.
    /// There are sixteen legacy IRQs, so 16 is the first value that is not one of them, and a
    /// bound read as inclusive writes past the sixteen-entry table. The existing refusal test uses
    /// 200, which any reading of the bound turns away.
    #[test]
    fn an_override_for_the_first_source_past_the_legacy_sixteen_is_ignored() {
        let mut body = q35_madt_body();
        body[41] = 16;
        let table = isa_irq_table(&body);
        for (irq, routing) in table.iter().enumerate().skip(1) {
            assert_eq!(*routing, IsaIrqRouting::isa_default(irq as u8));
        }
        assert_eq!(table[0].gsi, 2, "the well-formed override still applied");
    }

    // ---- survivor triage of the 2026-10-03 census ---------------------------------------------

    /// The same for the MADT's five ARM kinds: a GICC, a distributor, an MSI frame, a
    /// redistributor and an ITS that stop after their header come back as `Other`.
    #[test]
    fn a_short_arm_madt_entry_is_other_and_not_read() {
        for kind in [11u8, 12, 13, 14, 15] {
            let mut body = std::vec![0u8; MADT_FIXED_LEN];
            body.extend_from_slice(&[kind, 4, 0, 0]);
            let got: std::vec::Vec<_> = madt_entries(&body).collect();
            assert_eq!(got, std::vec![MadtEntry::Other(kind)], "kind {kind}");
        }
    }

    /// A GICC's flags word: bit 0 is enabled and bit 1 is online-capable, and each is its own.
    #[test]
    fn a_gicc_reads_enabled_and_online_capable_separately() {
        let mut body = std::vec![0u8; MADT_FIXED_LEN];
        let mut e = std::vec![0u8; 16];
        e[0] = 11;
        e[1] = 16;
        e[12] = 2;
        body.extend_from_slice(&e);
        match madt_entries(&body).next() {
            Some(MadtEntry::GenericInterruptController {
                enabled,
                online_capable,
                ..
            }) => assert!(!enabled && online_capable),
            other => panic!("{other:?}"),
        }
    }
}
