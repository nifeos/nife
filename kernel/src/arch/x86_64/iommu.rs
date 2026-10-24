//! **The VT-d driver: Intel's IOMMU, in front of the PCIe bus.** The register file, the root and
//! context tables and register-based invalidation; the translation tables are the portable seam's,
//! built in [`paging::x86_64::Vtd`]. Every DRHD the DMAR names gets its own root table, all-absent
//! before `GCMD.TE` is set so no device is unconstrained by omission, and `attach` goes to the
//! unit the DMAR says owns the device. A device no unit owns gets no context entry, and
//! [`scope_of`] says so.
//!
//! This is also `x86_64`'s IOMMU front door: every public entry point hands the call to
//! [`super::amd_vi`] when an AMD-Vi unit is up. See notes/iommu.md, which holds the design and
//! the detail of each BUGS entry.

//! # BUGS
//!
//! - Only PCI segment 0 is brought up; a unit elsewhere is reported and refused.
//! - A unit whose scope names only absent devices is still brought up, and may panic at `init`.
//! - An RMRR naming a bridge is mapped only once the device below it is confined.
//! - The graphics unit translates with only its RMRR mapped; scanout needs the RMRR to cover it.
//! - No interrupt remapping: `ECAP.IR` is only reported, and no `IRTE` is ever written.
//! - Invalidation is always global, which is correct and expensive with many devices.
//! - `RWBF` and protected-memory disabling have never run, because QEMU sets neither.
//! - One Fault Recording Register per unit is read, so one device's faults can hide another's.

use machine_discovery::acpi::{
    DmarUnits, Drhd, InterruptRemappingClaims, MAX_DRHDS, MAX_RMRR_SCOPES, SCOPE_PCI_ENDPOINT,
};
use paging::PageFormat;
use paging::x86_64::Vtd;

use crate::arch::mmu::phys_to_virt;
use crate::sync::{IrqSafeMutex, rank};

// --- Register file (offsets from the DRHD's register base; Intel VT-d spec chapter 10, and
// QEMU's hw/i386/intel_iommu_internal.h, which is this driver's ground truth for what q35's
// emulation actually checks). ---
const CAP: u64 = 0x08; // 64-bit
const ECAP: u64 = 0x10; // 64-bit
const GCMD: u64 = 0x18;
const GSTS: u64 = 0x1c;
const RTADDR: u64 = 0x20; // 64-bit
const CCMD: u64 = 0x28; // 64-bit
const FSTS: u64 = 0x34;
const PMEN: u64 = 0x64; // 32-bit, section 11.4.8.1

// GCMD (write-only) / GSTS (read-only, same bit positions): translation and root-pointer control.
const GCMD_TE: u32 = 1 << 31; // Translation Enable
const GCMD_SRTP: u32 = 1 << 30; // Set Root Table Pointer
const GCMD_WBF: u32 = 1 << 27; // Write Buffer Flush
const GSTS_TES: u32 = 1 << 31;
const GSTS_RTPS: u32 = 1 << 30;
const GSTS_WBFS: u32 = 1 << 27;
// Section 11.4.4's mask for turning a GSTS read into a GCMD value: it clears the one-shot command
// bits (SRTP, SFL, WBF, SIRTP) so reissuing the status never repeats one of them.
const GSTS_ONE_SHOT_MASK: u32 = 0x96FF_FFFF;
// IRES: interrupt remapping is ENABLED. Read only by the test that asserts it is clear; this
// driver has no `GCMD_IRE` constant to pair it with, deliberately, because there is nothing here
// that should be one typo away from turning interrupt remapping on. See notes/iommu.md's BUGS.
#[cfg(any(test, feature = "system_tests"))]
const GSTS_IRES: u32 = 1 << 25;

// CAP fields this driver reads. SAGAW is a bitmap (bit N means "AGAW level N is supported"), not
// an index; bit 2 of the 5-bit field (so bit 10 of the register) is the 48-bit/4-level width
// `Vtd`'s four-level walk needs, the same width `AW_48BIT` below selects in a context entry.
const CAP_SAGAW_48BIT: u64 = 1 << 10;
const CAP_RWBF: u64 = 1 << 4;
const CAP_PLMR: u64 = 1 << 5; // protected low-memory region supported
const CAP_PHMR: u64 = 1 << 6; // protected high-memory region supported
const CAP_ND_MASK: u64 = 7; // number of domain ids: 2^(4 + 2 * ND)
// PMEN: EPM enables the protected memory regions, PRS reports them in force.
const PMEN_EPM: u32 = 1 << 31;
const PMEN_PRS: u32 = 1 << 0;
const CAP_FRO_SHIFT: u64 = 24; // 10-bit field, in 16-byte units
const CAP_FRO_MASK: u64 = 0x3ff;

// ECAP fields: where the IOTLB invalidate register lives, since unlike the fault-recording bank
// it is not at a fixed offset by specification (QEMU happens to put it at a fixed offset; a real
// unit is not required to).
const ECAP_IRO_SHIFT: u64 = 8; // 10-bit field, in 16-byte units
const ECAP_IRO_MASK: u64 = 0x3ff;

// ECAP.IR (bit 3): the unit supports interrupt remapping. Read to report, never to act on (this
// driver does not set `GCMD.IRE`; see notes/iommu.md's BUGS). A boot must show the machine's own
// answer, because `-device intel-iommu,intremap=on` is a host-side string. `print_summary` prints
// it.
const ECAP_IR: u64 = 1 << 3;

// ECAP.C (bit 0): the unit's table walks snoop the processor caches. Clear means the hardware reads
// root, context and second-level entries from DRAM, so a write still in a cache line is invisible
// to it. See `Unit::publish`.
const ECAP_C: u64 = 1 << 0;

/// The `clflush` granule. CPUID leaf 1 reports it (EBX bits 15:8, in 8-byte units) and every x86_64
/// part this kernel has met reports 64; a smaller real line would leave lines unflushed, a larger
/// one only flushes some lines twice, so 64 is safe only while nothing reports less. Checked at
/// `init`, where a mismatch panics rather than walking stale tables.
const FLUSH_LINE: u64 = 64;

// CCMD_REG: context-cache invalidation, register-based (the legacy, non-queued interface every
// VT-d unit supports). ICC is set to start, cleared by hardware on completion; CIRG selects
// global granularity, the only one this driver uses.
const CCMD_ICC: u64 = 1 << 63;
const CCMD_CIRG_GLOBAL: u64 = 1 << 61;

// The IOTLB invalidate register, same shape as CCMD: IVT starts it, hardware clears it, IIRG
// selects granularity.
const IOTLB_IVT: u64 = 1 << 63;
const IOTLB_IIRG_GLOBAL: u64 = 1 << 60;

// FSTS: fault status. PPF says a record is waiting; PFO says a fault arrived with no free record, and
// while it is set the unit records nothing at all (write 1 to clear).
const FSTS_PFO: u32 = 1 << 0;
const FSTS_PPF: u32 = 1 << 1;

// Root entry (one page, 256 x 16 bytes, one per PCI bus): lower qword only, in legacy mode.
const ROOT_ENTRY_P: u64 = 1 << 0;
const ROOT_ENTRY_CTP_MASK: u64 = 0x000f_ffff_ffff_f000; // bits 63:12

// Context entry (one page, 256 x 16 bytes, one per device/function on a bus). Lower qword:
// present, translation type (00 = second-level-only, the shape every attach here writes), and the
// second-level page-table pointer. Upper qword: address width and domain id.
const CTX_ENTRY_P: u64 = 1 << 0;
const CTX_TT_MULTI_LEVEL: u64 = 0; // bits 3:2, value 0
const CTX_SLPTPTR_MASK: u64 = 0x000f_ffff_ffff_f000; // bits 63:12
const CTX_AW_48BIT: u64 = 2; // bits 2:0 of the upper qword: 010 = 48-bit AGAW, 4 levels
const CTX_DID_SHIFT: u64 = 8; // bits 23:8 of the upper qword

/// One recorded fault, in portable terms. `code` is the Fault Reason byte the spec defines (e.g.
/// `0x02` is a write past the second-level page table's write permission; `0x07` is no entry for
/// the address at all). Read today only by the confinement test; a production fault handler is
/// future work, the same posture the other two drivers take.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
#[derive(Debug, Clone, Copy)]
pub struct Fault {
    pub rid: u32,
    pub code: u32,
    /// The device address that faulted, or `None` when the unit did not say. VT-d always says.
    /// AMD-Vi's event record has the field, and QEMU 11.1.1's model never fills it: its
    /// `amdvi_setevent_bits` builds the address's mask with a shift of 64, undefined in C and
    /// zero in the Homebrew build (fixed upstream in QEMU commit 4adfb431c0, 2026-08-14, in no
    /// release as of 2026-10-05). So [`super::amd_vi::take_fault`] reports a zero address as
    /// `None`, and a test that wants to tie a fault to a frame has to use another witness.
    pub addr: Option<u64>,
}

/// One VT-d unit this kernel brought up.
struct Unit {
    base: u64,
    root: u64,
    rwbf: bool,
    frcd: u64,
    /// The IOTLB invalidate register's offset, from `ECAP.IRO` (not fixed by the specification).
    iotlb: u64,
    /// `ECAP.IR`: does this unit offer interrupt remapping? Recorded, never acted on. See the
    /// constant's comment and notes/iommu.md's BUGS for why a read-only field earns its place.
    interrupt_remapping: bool,
    /// The next domain id to hand out, and the largest `CAP.ND` allows. See [`Unit::domain_id`].
    next_did: u32,
    last_did: u32,
    /// How many devices an RMRR had pre-attached here before translation turned on.
    reserved_devices: u32,
    /// `ECAP.C`: this unit's table walks snoop the CPU caches. When clear, every table line the
    /// CPU writes must be written back before the unit is told to look ([`Unit::publish`]).
    coherent: bool,
}

/// Order every `clflush` before it against every later store, the MMIO write that starts an
/// invalidation included. `sfence`, which `direct_memory_access_write_barrier` is, does not order
/// `clflush` (the SDM orders it only by `mfence` or a locked or serializing instruction).
fn mfence() {
    // SAFETY: a fence; no operands, no state but ordering.
    unsafe { core::arch::asm!("mfence", options(nostack, preserves_flags)) };
}

impl Unit {
    /// **Write the lines `pa..pa + len` back to memory, if this unit cannot see the CPU caches.**
    /// VT-d 4.1 section 6.1 leaves cache maintenance of the remapping structures to software when
    /// `ECAP.C` is clear, as on xenon's catch-all unit. `clflush` per 64-byte line rather than
    /// `wbinvd`, which would write back every cache in the package for a few bytes of table. The
    /// caller fences (`mfence`) once after its last flush and before any register write that makes
    /// the unit read. See notes/iommu.md.
    fn publish(&self, pa: u64, len: u64) {
        if self.coherent {
            return;
        }
        let mut line = pa & !(FLUSH_LINE - 1);
        while line < pa + len {
            // SAFETY: `line` is inside a kernel-owned table frame reachable through the direct map;
            // clflush reads nothing and writes back a line the CPU may hold, changing no value.
            unsafe {
                core::arch::asm!("clflush [{}]", in(reg) phys_to_virt(line), options(nostack, preserves_flags));
            }
            line += FLUSH_LINE;
        }
    }

    /// [`Unit::publish`] every table frame of the second-level domain rooted at `root`: the walk
    /// follows present, non-leaf entries down to the last level, whose frames hold leaves. A
    /// domain is a handful of frames (the NVMe's is four), so a whole-frame flush of each is a few
    /// hundred lines.
    fn publish_domain(&self, table: u64, level: usize) {
        if self.coherent {
            return;
        }
        self.publish(table, page_frames::FRAME_SIZE);
        if level == 1 {
            return;
        }
        for i in 0..512 {
            // SAFETY: `table` is a kernel-owned page-table frame of 512 entries; read only.
            let e = unsafe { core::ptr::read_volatile((phys_to_virt(table) as *const u64).add(i)) };
            if Vtd::is_present(e) && !Vtd::is_block(e) {
                self.publish_domain(Vtd::entry_pa(e), level - 1);
            }
        }
    }

    /// **A fresh domain id, inside the width this unit implements** (milestone 594). Section 9.3:
    /// a unit with fewer than 16-bit domain ids treats the unused high bits of a context entry's
    /// `DID` as reserved, so a wider value is a reserved-field fault on every DMA the device
    /// makes. The portable seam passes the requester id as the tag, which QEMU's 16-bit model
    /// accepts and the 7040's two units (`CAP.ND` = 2, 8-bit ids) would reject for any device off
    /// bus 0: the NVMe at 01:00.0 would have been domain 0x100. Zero is never used, since
    /// section 9.3 reserves it when `CAP.CM` is set and nothing is lost by skipping it always.
    fn domain_id(&mut self) -> u16 {
        assert!(
            self.next_did <= self.last_did,
            "VT-d unit {:#x} has handed out all {} of its domain ids (CAP.ND)",
            self.base,
            self.last_did,
        );
        let did = self.next_did as u16;
        self.next_did += 1;
        did
    }
}

/// A DMAR unit's slot: brought up, refused with a reason, or not described at all.
enum Slot {
    Absent,
    Up(Unit),
    Refused(&'static str),
}

/// Every unit, indexed as `machine_discovery::acpi::DmarUnits::units` lists them, and each
/// unit's context tables beside it rather than inside [`Unit`], so a unit is a few dozen bytes to
/// build on the boot stack and only this static holds the 4 KiB of table pointers per unit.
struct Units {
    slots: [Slot; MAX_DRHDS],
    /// Per unit, one context-table root per PCI bus, allocated the first time a device on that
    /// bus is attached. `None` is the default-deny state: a bus with no table here has no
    /// root-table entry pointing at one either, so every device on it faults.
    ctx: [[Option<u64>; 256]; MAX_DRHDS],
}

impl Units {
    fn iter(&self) -> core::slice::Iter<'_, Slot> {
        self.slots.iter()
    }
}

static IOMMU: IrqSafeMutex<Units> = IrqSafeMutex::new(
    rank::IOMMU,
    Units {
        slots: [const { Slot::Absent }; MAX_DRHDS],
        ctx: [[None; 256]; MAX_DRHDS],
    },
);

/// Every unit, device scope and RMRR the DMAR described, so [`attach`] can route a device to its
/// owner and [`scope_of`] can say which unit that is. Recorded by [`init`]; never held at the same
/// time as [`IOMMU`], because resolving a scope path reads configuration space.
static DMAR: IrqSafeMutex<Option<DmarUnits>> = IrqSafeMutex::new(rank::IOMMU, None);

fn r32(base: u64, off: u64) -> u32 {
    // SAFETY: the DRHD's register file lies inside the direct map (it is ordinary MMIO below the
    // 4 GiB line on every machine this driver has run against), mapped device-typed by
    // `mmu::map_everything`; these reads are side-effect-free registers.
    unsafe { core::ptr::read_volatile(phys_to_virt(base + off) as *const u32) }
}
fn w32(base: u64, off: u64, v: u32) {
    // SAFETY: as above.
    unsafe { core::ptr::write_volatile(phys_to_virt(base + off) as *mut u32, v) }
}
fn r64(base: u64, off: u64) -> u64 {
    // SAFETY: as above; the 64-bit registers are 8-byte aligned.
    unsafe { core::ptr::read_volatile(phys_to_virt(base + off) as *const u64) }
}
fn w64(base: u64, off: u64, v: u64) {
    // SAFETY: as above.
    unsafe { core::ptr::write_volatile(phys_to_virt(base + off) as *mut u64, v) }
}

fn zeroed_page_frame(what: &str) -> u64 {
    let pa = crate::memory::alloc()
        .unwrap_or_else(|| panic!("no frame for the IOMMU {what}"))
        .addr();
    // SAFETY: a fresh frame, reachable through the direct map, owned by this module from here on.
    unsafe {
        core::ptr::write_bytes(
            phys_to_virt(pa) as *mut u8,
            0,
            page_frames::FRAME_SIZE as usize,
        );
    }
    pa
}

/// Wait for `cond(gsts)` to hold, polling `GSTS_REG`. Every global-command write this driver makes
/// is confirmed this way, the same write-then-poll shape `CCMD`/IOTLB invalidation use on their
/// own registers below.
fn wait_gsts(base: u64, what: &str, cond: impl Fn(u32) -> bool) {
    let mut spins = 0u32;
    loop {
        let v = r32(base, GSTS);
        if cond(v) {
            return;
        }
        spins += 1;
        assert!(spins < 1_000_000, "IOMMU {what} never completed ({v:#x})");
    }
}

/// **Issue one global command, leaving every other one as it is** (VT-d 4.1 section 11.4.4).
/// `GCMD_REG` is write-only and every bit in it is a command, so writing a lone bit also writes
/// zero to every other one, and a zero `TE` turns translation off. The section's recipe is to
/// read the status, clear the one-shot bits (`0x96FF_FFFF`), then set or clear exactly one bit.
/// Until milestone 594 this driver wrote lone bits, which was harmless only because `WBF`, the
/// one command issued with translation on, never ran.
fn global_command(base: u64, bit: u32, on: bool) {
    let status = r32(base, GSTS) & GSTS_ONE_SHOT_MASK;
    w32(base, GCMD, if on { status | bit } else { status & !bit });
}

/// **Install an all-absent root table on one unit, without turning translation on.** `Err` is a
/// unit this driver will not drive, with the reason the tour prints.
fn root_up(d: &Drhd) -> Result<Unit, &'static str> {
    if d.segment != 0 {
        return Err("not on PCI segment 0, the only configuration space this kernel reads");
    }
    let base = d.register_base;
    let cap = r64(base, CAP);
    if cap & CAP_SAGAW_48BIT == 0 {
        return Err(
            "no 48-bit/4-level second-level translation (CAP.SAGAW bit 2), which Vtd needs",
        );
    }
    let ecap = r64(base, ECAP);
    // ND: 0 means 4-bit ids and each step adds two bits, up to 16 at 6; 7 is reserved.
    let nd = (cap & CAP_ND_MASK).min(6);
    let unit = Unit {
        base,
        root: zeroed_page_frame("root table"),
        rwbf: cap & CAP_RWBF != 0,
        frcd: ((cap >> CAP_FRO_SHIFT) & CAP_FRO_MASK) << 4,
        iotlb: (((ecap >> ECAP_IRO_SHIFT) & ECAP_IRO_MASK) << 4) + 8,
        interrupt_remapping: ecap & ECAP_IR != 0,
        next_did: 1,
        last_did: (1u32 << (4 + 2 * nd)) - 1,
        reserved_devices: 0,
        coherent: ecap & ECAP_C != 0,
    };
    if !unit.coherent {
        let line = u64::from((core::arch::x86_64::__cpuid(1).ebx >> 8) & 0xff) * 8;
        assert!(
            line >= FLUSH_LINE,
            "clflush line is {line} bytes, smaller than the {FLUSH_LINE} this driver flushes by"
        );
    }
    // The all-absent root goes to memory before the unit is pointed at it. Zero is what the
    // walk must read; whatever the frame held before is what it would read otherwise.
    unit.publish(unit.root, page_frames::FRAME_SIZE);
    mfence();

    // Firmware may hand over with translation already on (a pre-boot DMA protection setting, or
    // a kexec). Linux turns it off before reprogramming (`init_dmars`), since a root pointer the
    // kernel did not build is not one it can reason about; so does this.
    if r32(base, GSTS) & GSTS_TES != 0 {
        global_command(base, GCMD_TE, false);
        wait_gsts(base, "translation disable", |v| v & GSTS_TES == 0);
    }

    w64(base, RTADDR, unit.root);
    global_command(base, GCMD_SRTP, true);
    wait_gsts(base, "set root table pointer", |v| v & GSTS_RTPS != 0);
    invalidate_all(&unit);
    Ok(unit)
}

/// **Bring every VT-d unit up, each translating the devices it owns** (milestone 594 (every VT-d
/// unit translates its own devices), provisional number; VT-d 4.1 sections 3.16, 8.3 and 8.4), in
/// three steps whose order is the point:
///
/// 1. Every unit gets an all-absent root table, with translation still off.
/// 2. Every device an RMRR names is confined to a domain that is exactly its reserved regions.
/// 3. Translation turns on unit by unit, and protected memory regions firmware left are disabled.
///
/// Then every device a running unit owns faults until `attach` writes its context entry (or, for
/// an RMRR device, anywhere outside its reserved regions).
pub fn init(dmar: &DmarUnits) {
    *DMAR.lock() = Some(*dmar);
    {
        let mut g = IOMMU.lock();
        assert!(
            g.iter().all(|s| matches!(s, Slot::Absent)),
            "IOMMU initialized twice"
        );
        for (i, d) in dmar.units().iter().enumerate() {
            g.slots[i] = match root_up(d) {
                Ok(u) => Slot::Up(u),
                Err(why) => Slot::Refused(why),
            };
        }
    }

    // Step 2. A device named by two scopes (two RMRRs, say) is confined once, with both regions:
    // `confine` asks `for_each_reserved_region` for all of them.
    let mut done = [0u32; MAX_RMRR_SCOPES];
    let mut done_count = 0;
    for s in &dmar.rmrr_scopes[..dmar.rmrr_scope_count] {
        let region = dmar.rmrrs[s.unit as usize];
        if region.segment != 0 || s.kind != SCOPE_PCI_ENDPOINT {
            continue; // see the VT-d BUGS in the IOMMU note on sub-hierarchy RMRR scopes
        }
        let Some((bus, dev, func)) = s.resolve(&mut crate::pci::bridge_bus_range) else {
            continue; // the path names hardware that is not on this machine
        };
        let rid = (bus as u32) << 8 | (dev as u32) << 3 | func as u32;
        if done[..done_count].contains(&rid) {
            continue;
        }
        done[done_count] = rid;
        done_count += 1;
        if let Some(i) = owner_index(rid) {
            crate::iommu::confine(rid, &[]);
            if let Slot::Up(u) = &mut IOMMU.lock().slots[i] {
                u.reserved_devices += 1;
            }
        }
    }

    // Step 3.
    let mut g = IOMMU.lock();
    for s in g.slots.iter_mut() {
        let Slot::Up(u) = s else { continue };
        global_command(u.base, GCMD_TE, true);
        wait_gsts(u.base, "translation enable", |v| v & GSTS_TES != 0);
        if r64(u.base, CAP) & (CAP_PLMR | CAP_PHMR) != 0 {
            w32(u.base, PMEN, r32(u.base, PMEN) & !PMEN_EPM);
            let mut spins = 0u32;
            while r32(u.base, PMEN) & PMEN_PRS != 0 {
                spins += 1;
                assert!(spins < 1_000_000, "IOMMU protected-memory disable hung");
            }
        }
    }
    let n = dmar.units().len();
    for (i, d) in dmar.units().iter().enumerate() {
        let what = if d.include_pci_all {
            "the catch-all"
        } else {
            "named devices only"
        };
        match &g.slots[i] {
            Slot::Up(u) => crate::println!(
                "  vt-d        : drhd {:#x} up ({} of {n}, {what}), translation enabled (gsts.tes \
                 confirmed), {} rmrr device(s) identity-mapped",
                d.register_base,
                i + 1,
                u.reserved_devices,
            ),
            Slot::Refused(why) => crate::println!(
                "  vt-d        : drhd {:#x} NOT up ({} of {n}, {what}): {why}; the devices it owns \
                 are not translated",
                d.register_base,
                i + 1,
            ),
            Slot::Absent => {}
        }
    }
}

/// The index of the unit that owns `rid`, when that unit is one this kernel could ask. `None` is
/// a device no unit owns, or a DMAR recorded only in part. Resolving a scope path reads
/// configuration space, so this runs with neither lock held across it.
fn owner_index(rid: u32) -> Option<usize> {
    let units = (*DMAR.lock())?;
    let (bus, dev, func) = ((rid >> 8) as u8, ((rid >> 3) & 0x1f) as u8, (rid & 7) as u8);
    match units.owner_index(0, bus, dev, func, &mut crate::pci::bridge_bus_range) {
        Ok(Some((i, _))) => Some(i),
        _ => None,
    }
}

/// **Every RMRR the firmware declared for requester `rid`**, as the regions a DMA domain for it
/// must also map (VT-d 4.1 section 3.16). [`crate::iommu::confine`] adds them to every domain it
/// builds, so confining a USB controller or the GPU never takes away memory the firmware still
/// DMAs into. Name: provisional (milestone 594).
pub(crate) fn for_each_reserved_region(rid: u32, each: &mut dyn FnMut(paging::domain::DmaRegion)) {
    if super::amd_vi::is_active() {
        return super::amd_vi::for_each_reserved_region(rid, each);
    }
    let Some(units) = *DMAR.lock() else {
        return;
    };
    let (bus, dev, func) = ((rid >> 8) as u8, ((rid >> 3) & 0x1f) as u8, (rid & 7) as u8);
    units.reserved_for(
        0,
        bus,
        dev,
        func,
        &mut crate::pci::bridge_bus_range,
        &mut |r| {
            each(paging::domain::DmaRegion {
                base: r.base,
                size: r.size(),
                // An RMRR carries no permission bits: it is memory firmware both reads and writes
                // through DMA, and VT-d 4.1 section 3.16 asks for it mapped as such.
                writable: true,
            });
        },
    );
}

/// **Which unit, if any, translates requester id `rid`, and why?** (milestone 261 (the NVMe driver leaves the kernel)'s bench
/// rehearsal; VT-d 4.1 section 8.3.) Asked after the bus is up, because a scope path is resolved
/// through the live bridges' bus-number registers.
///
/// This is fatal risk 6's first night-of condition as code. A VT-d unit translates only the
/// requesters its DMAR scope gives it, so "translation is on" and "this device is confined" are
/// different claims. Since milestone 594 every unit is brought up, so the answer is `Owned`
/// whenever the device has an owner and that owner came up; `Elsewhere` now means a unit this
/// kernel refused (`owner: Some`) or no owner at all (`owner: None`).
pub(crate) fn scope_of(rid: u32) -> crate::iommu::Scope {
    use crate::iommu::Scope;
    if super::amd_vi::is_active() {
        return super::amd_vi::scope_of(rid);
    }
    // Copied out rather than held, so the config-space reads that resolve a path run with no
    // lock taken.
    let mut up = [false; MAX_DRHDS];
    let mut translating = None;
    for (i, s) in IOMMU.lock().iter().enumerate() {
        if let Slot::Up(u) = s {
            up[i] = true;
            translating.get_or_insert(u.base);
        }
    }
    let Some(translating) = translating else {
        return Scope::NoIommu;
    };
    let Some(units) = *DMAR.lock() else {
        return Scope::Unknown { translating };
    };
    let (bus, dev, func) = ((rid >> 8) as u8, ((rid >> 3) & 0x1f) as u8, (rid & 7) as u8);
    match units.owner_index(0, bus, dev, func, &mut crate::pci::bridge_bus_range) {
        Ok(Some((i, how))) if up[i] => Scope::Owned {
            unit: units.drhds[i].register_base,
            how,
        },
        Ok(Some((i, _))) => Scope::Elsewhere {
            translating,
            owner: Some(units.drhds[i].register_base),
        },
        Ok(None) => Scope::Elsewhere {
            translating,
            owner: None,
        },
        Err(()) => Scope::Unknown { translating },
    }
}

/// **Does this machine's VT-d offer interrupt remapping (`ECAP.IR`) on every unit that is up?**
/// `None` when no unit is up, which is a different answer from "a unit that says no". Every unit,
/// because remapping that one unit lacks leaves that unit's devices' interrupts unremapped.
///
/// Nothing in this kernel remaps an interrupt. This exists so the question is *askable* from
/// inside the guest, which is what milestone 317 is for: without it, turning
/// `-device intel-iommu,intremap=on` on and watching the suite stay green proves only that the
/// suite does not care. See design/roadmap/0317-interrupt-remapping-flags.md.
// One caller: this module's own test. The comment below said "two callers, print_summary and this
// module's own test", but print_summary inlines `u.interrupt_remapping` and never calls this; the
// CI-warnings lane read the body on 2026-09-30 after mirroring print_summary's allowance set and
// finding the warning still fired. A test-only helper gets the not(test) allowance the tree's
// test-only items carry.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn interrupt_remapping_available() -> Option<bool> {
    let g = IOMMU.lock();
    let mut any = false;
    let mut all = true;
    for s in g.iter() {
        if let Slot::Up(u) = s {
            any = true;
            all &= u.interrupt_remapping;
        }
    }
    any.then_some(all)
}

/// **This machine's IOMMU, for the machine description** (milestone 268).
///
/// One of the eight questions the description answers on every architecture. The vocabulary is this
/// architecture's, because that is what parity means here: the same question, answered in the terms
/// of the hardware that answers it. A machine with no IOMMU says so plainly rather than printing a
/// blank, because a blank is indistinguishable from a line nobody wrote. One line per unit.
// The machine description is the only caller, and it is
// `#[cfg(not(any(test, feature = "bench")))]`: a test boot exits through semihosting and a bench
// boot diverges into `bench::run`, so neither reads a bring-up transcript. Same treatment
// `memory::print_summary` already carries, and for the same reason.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub(crate) fn print_summary() {
    if super::amd_vi::print_summary() {
        return;
    }
    // Copied out before `IOMMU` is taken: both locks share a rank.
    let platform = DMAR
        .lock()
        .as_ref()
        .is_some_and(|d| d.platform_interrupt_remapping);
    let g = IOMMU.lock();
    let mut any = false;
    for s in g.iter() {
        match s {
            Slot::Up(u) => {
                any = true;
                crate::println!(
                    "  iommu           : VT-d drhd at {:#018x}, root table default-deny, translating, \
                     {}",
                    u.base,
                    InterruptRemappingClaims {
                        unit: u.interrupt_remapping,
                        platform,
                    },
                );
            }
            Slot::Refused(why) => {
                any = true;
                crate::println!("  iommu           : VT-d unit refused: {why}");
            }
            Slot::Absent => {}
        }
    }
    if !any {
        crate::println!(
            "  iommu           : none (this machine's ACPI names neither a DMAR nor an IVRS); device \
             DMA is NOT confined",
        );
    }
}

/// Is any VT-d or AMD-Vi unit up? The portable seam asks this to decide whether attaching is
/// possible.
pub fn is_active() -> bool {
    super::amd_vi::is_active() || IOMMU.lock().iter().any(|s| matches!(s, Slot::Up(_)))
}

/// **Build a DMA domain over exactly `regions` in the format this machine's IOMMU walks**, and
/// return its root: [`paging::x86_64::AmdVi`] when an AMD-Vi unit is up, [`Vtd`] otherwise. The two
/// share the CPU's four-level, 4 KiB-leaf shape and nothing else that matters: `Vtd`'s and
/// `AmdVi`'s own docs say why neither encoding, nor `Ia32e`'s, would work in the other's place.
pub(crate) fn build_domain(regions: &[paging::domain::DmaRegion]) -> u64 {
    if super::amd_vi::is_active() {
        crate::iommu::build_identity::<paging::x86_64::AmdVi>(regions)
    } else {
        crate::iommu::build_identity::<Vtd>(regions)
    }
}

/// Register-based, global invalidation: the context cache first (a stale context entry would
/// still point translation at the previous domain), then the IOTLB, each a write-and-poll on its
/// own register. Both use global granularity; see notes/iommu.md's BUGS for the cost.
fn invalidate_all(s: &Unit) {
    if s.rwbf {
        global_command(s.base, GCMD_WBF, true);
        wait_gsts(s.base, "write buffer flush", |v| v & GSTS_WBFS == 0);
    }

    w64(s.base, CCMD, CCMD_ICC | CCMD_CIRG_GLOBAL);
    let mut spins = 0u32;
    while r64(s.base, CCMD) & CCMD_ICC != 0 {
        spins += 1;
        assert!(spins < 1_000_000, "IOMMU context-cache invalidate hung");
    }

    w64(s.base, s.iotlb, IOTLB_IVT | IOTLB_IIRG_GLOBAL);
    let mut spins = 0u32;
    while r64(s.base, s.iotlb) & IOTLB_IVT != 0 {
        spins += 1;
        assert!(spins < 1_000_000, "IOMMU IOTLB invalidate hung");
    }
}

/// Point device `rid` (the PCIe requester id: bus in bits 15:8, device/function in bits 7:0) at
/// the domain rooted at `root` (a [`Vtd`]-format table the portable seam built) **in the unit the
/// DMAR says owns it**, then invalidate that unit. `_tag` is unused: VT-d's tag is the domain id,
/// which the unit hands out itself because the requester id does not fit a real unit's domain-id
/// width ([`Unit::domain_id`]). A device no running unit owns gets no context entry; [`scope_of`]
/// is how a caller learns that.
pub fn attach(rid: u32, root: u64, _tag: u16) {
    if super::amd_vi::is_active() {
        return super::amd_vi::attach(rid, root);
    }
    let Some(i) = owner_index(rid) else {
        return;
    };
    let mut g = IOMMU.lock();
    let Units { slots, ctx } = &mut *g;
    let Slot::Up(s) = &mut slots[i] else {
        return;
    };
    let ctx = &mut ctx[i];
    let did = s.domain_id();

    let bus = (rid >> 8) as usize & 0xff;
    let devfn = (rid & 0xff) as usize;

    let ctp = match ctx[bus] {
        Some(ctp) => ctp,
        None => {
            let ctp = zeroed_page_frame("context table");
            s.publish(ctp, page_frames::FRAME_SIZE);
            // Publish the (still all-absent) context table before the root entry that makes it
            // reachable, so the IOMMU can never walk to a root entry whose context table isn't
            // there yet.
            crate::arch::direct_memory_access_write_barrier();
            let root_entry = phys_to_virt(s.root + bus as u64 * 16) as *mut u64;
            // SAFETY: `s.root` is a kernel-owned page-aligned frame; `bus` is masked to 0..256,
            // which is exactly the 256 entries a one-page root table holds.
            unsafe {
                core::ptr::write_volatile(root_entry, (ctp & ROOT_ENTRY_CTP_MASK) | ROOT_ENTRY_P);
                core::ptr::write_volatile(root_entry.add(1), 0); // upper qword: reserved, legacy mode
            }
            s.publish(s.root + bus as u64 * 16, 16);
            ctx[bus] = Some(ctp);
            ctp
        }
    };

    // The context entry, written back to front: the second-level root and the domain id first,
    // the present bit last, with a barrier between, so the IOMMU can never observe a present
    // entry with a stale second-level root. Same ordering discipline the RISC-V driver's
    // `attach` uses for its device context.
    let ctx_entry = phys_to_virt(ctp + devfn as u64 * 16) as *mut u64;
    let hi = CTX_AW_48BIT | ((did as u64) << CTX_DID_SHIFT);
    let lo = (root & CTX_SLPTPTR_MASK) | CTX_TT_MULTI_LEVEL;
    // SAFETY: `ctp` is a kernel-owned page-aligned frame; `devfn` is masked to 0..256, which is
    // exactly the 256 entries a one-page context table holds.
    unsafe {
        core::ptr::write_volatile(ctx_entry.add(1), hi);
        core::ptr::write_volatile(ctx_entry, lo);
    }
    crate::arch::direct_memory_access_write_barrier();
    // SAFETY: as above.
    unsafe {
        core::ptr::write_volatile(ctx_entry, lo | CTX_ENTRY_P);
    }
    // The domain the seam built, then the entry that points at it, then one fence before the
    // invalidation that lets the unit walk any of them. One line holds the whole 16-byte entry,
    // so the present bit and the root reach memory together.
    s.publish_domain(root, Vtd::LEVELS);
    s.publish(ctp + devfn as u64 * 16, 16);
    mfence();
    crate::arch::direct_memory_access_write_barrier();

    invalidate_all(s);
}

/// Pop one fault, if any unit holds one: `FSTS.PPF` says at least one Fault Recording Register
/// holds an unprocessed record, and (with `CAP.NFR` reporting one register on every unit this
/// driver has met) the first is the only one read. The confinement test drains this to prove a
/// DMA escape was stopped by the hardware, not merely absent.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn take_fault() -> Option<Fault> {
    if super::amd_vi::is_active() {
        return super::amd_vi::take_fault();
    }
    let g = IOMMU.lock();
    for slot in g.iter() {
        let Slot::Up(s) = slot else { continue };
        // An overflow blinds the unit until it is cleared: with PFO set, QEMU drops every later
        // fault, so clearing it here is what lets the next fault land.
        if r32(s.base, FSTS) & FSTS_PFO != 0 {
            w32(s.base, FSTS, FSTS_PFO);
        }
        if r32(s.base, FSTS) & FSTS_PPF == 0 {
            continue;
        }
        let lo = r64(s.base, s.frcd);
        let hi = r64(s.base, s.frcd + 8);
        if hi & (1 << 63) == 0 {
            // PPF was set but this record's own F bit is not: nothing to report from the one
            // register this driver reads. See notes/iommu.md's BUGS on why only one is ever read.
            continue;
        }
        // Write-1-to-clear: writing back exactly what was read clears the F bit (bit 63 of `hi`)
        // and leaves every other field, which the hardware overwrites on the next fault anyway.
        w64(s.base, s.frcd + 8, hi);
        return Some(Fault {
            rid: (hi & 0xffff) as u32,
            code: ((hi >> 32) & 0xff) as u32,
            addr: Some(lo & !0xfff),
        });
    }
    None
}

/// **Bench diagnostic for fatal risk 6's CompletionTimeout on xenon (2026-10-04).** Everything
/// the unit that is up can say about why a device's DMA went nowhere, without clearing anything:
/// CAP and ECAP (ECAP bit 0, `C`, says whether the unit's table walks snoop the CPU caches; this
/// driver never flushes the tables it writes, so `C=0` would mean the hardware reads whatever is
/// in DRAM), GSTS and FSTS, then every Fault Recording Register whose F bit is set, and the root
/// and context entries for `rid` as the CPU sees them. Bounded at about eight lines.
#[cfg(feature = "disk_throughput")]
pub(crate) fn print_faults(rid: u32) {
    if super::amd_vi::is_active() {
        return super::amd_vi::print_faults(rid);
    }
    // Before the lock: `owner_index` reads configuration space and takes `DMAR`.
    let owner = owner_index(rid);
    let g = IOMMU.lock();
    let Some(i) = owner else {
        crate::println!("diag vt-d no running unit owns {rid:#06x}");
        return;
    };
    let Slot::Up(s) = &g.slots[i] else {
        crate::println!("diag vt-d the unit owning {rid:#06x} is not up");
        return;
    };
    let ctx_tables = &g.ctx[i];
    let cap = r64(s.base, CAP);
    let ecap = r64(s.base, ECAP);
    let nfr = ((cap >> 40) & 0xff) + 1;
    crate::println!(
        "diag vt-d {:#x} cap {cap:#x} ecap {ecap:#x} c={} pt={} nfr {nfr}",
        s.base,
        ecap & 1,
        ecap >> 6 & 1
    );
    let fsts = r32(s.base, FSTS);
    crate::println!(
        "diag vt-d gsts {:#010x} fsts {fsts:#x} (ppf {} pfo {} fri {})",
        r32(s.base, GSTS),
        fsts >> 1 & 1,
        fsts & 1,
        fsts >> 8 & 0xff
    );
    let mut shown = 0;
    for i in 0..nfr.min(16) {
        let hi = r64(s.base, s.frcd + i * 16 + 8);
        if hi & (1 << 63) == 0 {
            continue;
        }
        let lo = r64(s.base, s.frcd + i * 16);
        if shown < 6 {
            let sid = hi & 0xffff;
            crate::println!(
                "diag vt-d frcd{i} sid {:02x}:{:02x}.{} reason {:#04x} {} addr {:#x}",
                sid >> 8,
                sid >> 3 & 0x1f,
                sid & 7,
                (hi >> 32) & 0xff,
                if hi >> 62 & 1 == 1 { "read" } else { "write" },
                lo & !0xfff
            );
        }
        shown += 1;
    }
    if shown == 0 {
        crate::println!("diag vt-d no fault record has F set");
    } else if shown > 6 {
        crate::println!("diag vt-d ... {} more fault records not shown", shown - 6);
    }
    print_context_entry(s, &ctx_tables, rid);
}

/// One line: the root and context entries for `rid` as the CPU sees them, with the domain id
/// decoded against the width this unit implements (`CAP.ND`). xenon's second diagnostic boot
/// (2026-10-04) faulted with reason 0x0b on `ctx .. 0x10002`, domain 0x100 on a unit whose
/// `CAP.ND` = 2 allows 255; this line says whether a boot is past that.
#[cfg(feature = "disk_throughput")]
fn print_context_entry(s: &Unit, ctx_tables: &[Option<u64>; 256], rid: u32) {
    let bus = (rid >> 8) as u64 & 0xff;
    let devfn = rid as u64 & 0xff;
    // SAFETY: `s.root` and any context table are kernel-owned table frames; bus and devfn are
    // masked to the 256 entries each holds. Reads only.
    let (re, ce) = unsafe {
        let re = core::ptr::read_volatile(phys_to_virt(s.root + bus * 16) as *const u64);
        let ce = ctx_tables[bus as usize].map(|ctp| {
            let p = phys_to_virt(ctp + devfn * 16) as *const u64;
            (
                core::ptr::read_volatile(p),
                core::ptr::read_volatile(p.add(1)),
            )
        });
        (re, ce)
    };
    match ce {
        Some((lo, hi)) => crate::println!(
            "diag vt-d root[{bus}] {re:#x} ctx {lo:#x} {hi:#x} (did {}, nd allows {}, aw {})",
            (hi >> CTX_DID_SHIFT) & 0xffff,
            s.last_did,
            hi & 7,
        ),
        None => crate::println!("diag vt-d root[{bus}] {re:#x} ctx none (never attached)"),
    }
}

/// The context-entry line on its own, for a bring-up that succeeded: the bench photograph then
/// shows the entry the unit accepted, not only the one it refused.
#[cfg(feature = "disk_throughput")]
pub(crate) fn print_context(rid: u32) {
    if super::amd_vi::is_active() {
        return super::amd_vi::print_faults(rid);
    }
    let owner = owner_index(rid);
    let g = IOMMU.lock();
    let Some(i) = owner else { return };
    if let Slot::Up(s) = &g.slots[i] {
        print_context_entry(s, &g.ctx[i], rid);
    }
}

// A compile-time check that this module and `Vtd` agree on the level count `CTX_AW_48BIT`
// promises the hardware: four levels, the same the context entry's AW field selects.
const _: () = assert!(Vtd::LEVELS == 4);

#[cfg(test)]
mod tests {
    //! Milestone 317's half of the interrupt-remapping question, which is the half a boot can
    //! answer. The other half (does a remapped interrupt land where an `IRTE` says it should)
    //! needs a driver that programs one, and this tree has none.

    use super::*;

    /// **The unit's interrupt-remapping capability is reported, and remapping is still off.**
    ///
    /// The first assertion is a guard: on a runner that attaches `-device intel-iommu`, a `None`
    /// means the DRHD was never found. The second states a limit (notes/confinement-claims.md's
    /// shape): whatever `ECAP.IR` says, `GSTS.IRES` is clear, because this driver never sets
    /// `GCMD.IRE`. It fails the day remapping is enabled without retiring the claim in
    /// design/roadmap/0317-interrupt-remapping-flags.md, and it runs with `NIFE_INTREMAP` and
    /// without, since a test that only ran under the flag would be one nobody runs.
    #[test_case]
    fn interrupt_remapping_is_reported_and_never_enabled() {
        let Some(offered) = interrupt_remapping_available() else {
            crate::testing::skip!("this machine's ACPI names no DMAR, so there is no unit to ask");
        };

        for slot in IOMMU.lock().iter() {
            let Slot::Up(u) = slot else { continue };
            assert_eq!(
                r32(u.base, GSTS) & GSTS_IRES,
                0,
                "GSTS.IRES is set on drhd {:#x}: something enabled interrupt remapping (ECAP.IR \
                 was {offered}), and the claim that this kernel never remaps an interrupt is now \
                 false",
                u.base,
            );
        }
    }

    /// **Every unit the DMAR names is translating** (milestone 594). Before it, one unit came up
    /// and the rest were left off by design; this fails if a unit is silently skipped again. A
    /// refused unit fails it too, which is right under QEMU, whose one unit has nothing to refuse.
    #[test_case]
    fn every_unit_the_dmar_names_is_translating() {
        let Some(units) = *DMAR.lock() else {
            crate::testing::skip!("this machine's ACPI names no DMAR, so there is no unit to ask");
        };
        let g = IOMMU.lock();
        for (i, d) in units.units().iter().enumerate() {
            match &g.slots[i] {
                Slot::Up(u) => assert_ne!(
                    r32(u.base, GSTS) & GSTS_TES,
                    0,
                    "drhd {:#x} is up but GSTS.TES reads clear",
                    u.base
                ),
                Slot::Refused(why) => panic!("drhd {:#x} was refused: {why}", d.register_base),
                Slot::Absent => panic!("drhd {:#x} was never brought up", d.register_base),
            }
        }
    }

    /// **A device no unit owns gets no context entry anywhere** (milestone 594's decision, from
    /// Linux's `-ENODEV`). Before it, `attach` wrote into the one running unit's tables whatever
    /// requester it was handed, so a domain could be "attached" in a table the device's DMA is
    /// never looked up in. Bus 0xfe is empty on every runner; on a DMAR with a catch-all it is
    /// owned all the same, and there is nothing to prove.
    #[test_case]
    fn a_device_no_unit_owns_gets_no_context_entry() {
        let rid = 0xfe << 8;
        match scope_of(rid) {
            crate::iommu::Scope::Elsewhere { owner: None, .. } => {}
            crate::iommu::Scope::NoIommu => {
                crate::testing::skip!("no VT-d unit is up on this machine")
            }
            _ => crate::testing::skip!("bus 0xfe has an owner on this DMAR (a catch-all takes it)"),
        }
        let root = zeroed_page_frame("test domain root");
        attach(rid, root, 0);
        let g = IOMMU.lock();
        for (i, slot) in g.iter().enumerate() {
            let Slot::Up(u) = slot else { continue };
            assert!(
                g.ctx[i][0xfe].is_none(),
                "drhd {:#x} grew a context table for a bus no unit owns",
                u.base
            );
        }
    }
}
