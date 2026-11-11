//! **The VT-d driver: Intel's IOMMU, in front of the PCIe bus.** The register file, the root and
//! context tables and register-based invalidation; the translation tables are the portable seam's,
//! built in [`paging::x86_64::Vtd`]. Every DRHD the DMAR names gets its own root table, all-absent
//! before `GCMD.TE` is set so no device is unconstrained by omission, and `attach` goes to the
//! unit the DMAR says owns the device. A device no unit owns gets no context entry, and
//! [`scope_of`] says so.
//!
//! Where a unit and the platform both offer it, the unit also remaps interrupts: every MSI the
//! kernel grants and every IO APIC line it routes is an entry in the unit's interrupt remapping
//! table, naming the one vector granted and the one requester id allowed to raise it, and
//! compatibility-format messages are blocked. Invalidation is queued on every unit that offers
//! it, since the interrupt entry cache has no register-based invalidation.
//!
//! This is also `x86_64`'s IOMMU front door: every public entry point hands the call to
//! [`super::amd_vi`] when an AMD-Vi unit is up. See notes/iommu.md, which holds the design and
//! the detail of each BUGS entry.

//! # BUGS
//!
//! - Only PCI segment 0 is brought up; a unit elsewhere is reported and refused.
//! - A unit whose scope names only absent devices is still brought up, and may panic at
//!   `initialize`.
//! - An RMRR naming a bridge is mapped only once the device below it is confined.
//! - The graphics unit translates with only its RMRR mapped; scanout needs the RMRR to cover it.
//! - A remapping entry is never freed, like the MSI vector it carries; one table holds 256.
//! - Destinations are 8-bit xAPIC ids (`IRTA.EIME` clear); an x2APIC machine is not handled.
//! - A device behind a PCIe-to-PCI bridge raises its MSIs under the bridge's requester id, which
//!   its remapping entry does not name, so they are refused.
//! - Remapping stays off on a machine whose IO APIC no DMAR scope names.
//! - Invalidation is always global, which is correct and expensive with many devices.
//! - `RWBF` and protected-memory disabling have never run, because QEMU sets neither.
//! - One Fault Recording Register per unit is read, so one device's faults can hide another's.

use machine_discovery::acpi::{
    DmarUnits, Drhd, InterruptRemappingClaims, MAXIMUM_RESERVED_REGION_DEVICES,
    MAXIMUM_TRANSLATION_UNITS, SCOPE_PCI_ENDPOINT,
};
use paging::PageFormat;
use paging::x86_64::Vtd;
use pci::MsiTarget;

use crate::arch::mmu::phys_to_virt;
use crate::sync::{InterruptSafeMutex, rank};

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
// Queued invalidation (section 11.4.9): the queue's head, tail and address, all 64-bit.
const IQH: u64 = 0x80;
const IQT: u64 = 0x88;
const IQA: u64 = 0x90;
// The interrupt remapping table's address and size (section 11.4.10.1), 64-bit.
const IRTA: u64 = 0xb8;

// GCMD (write-only) / GSTS (read-only, same bit positions): translation and root-pointer control.
const GCMD_TE: u32 = 1 << 31; // Translation Enable
const GCMD_SRTP: u32 = 1 << 30; // Set Root Table Pointer
const GCMD_WBF: u32 = 1 << 27; // Write Buffer Flush
const GSTS_TES: u32 = 1 << 31;
const GSTS_RTPS: u32 = 1 << 30;
const GSTS_WBFS: u32 = 1 << 27;
// Queued invalidation enable, interrupt remapping enable, set interrupt remapping table pointer,
// compatibility format interrupts (allowed when set; this driver keeps it clear).
const GCMD_QIE: u32 = 1 << 26;
const GCMD_IRE: u32 = 1 << 25;
const GCMD_SIRTP: u32 = 1 << 24;
const GCMD_CFI: u32 = 1 << 23;
const GSTS_QIES: u32 = 1 << 26;
const GSTS_IRES: u32 = 1 << 25;
const GSTS_IRTPS: u32 = 1 << 24;
const GSTS_CFIS: u32 = 1 << 23;
// Section 11.4.4's mask for turning a GSTS read into a GCMD value: it clears the one-shot command
// bits (SRTP, SFL, WBF, SIRTP) so reissuing the status never repeats one of them.
const GSTS_ONE_SHOT_MASK: u32 = 0x96FF_FFFF;

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

// ECAP.IR (bit 3): the unit supports interrupt remapping. A boot shows the machine's own answer,
// because `-device intel-iommu,intremap=on` is a host-side string; `print_summary` prints it.
const ECAP_IR: u64 = 1 << 3;

// ECAP.QI (bit 1): the unit supports queued invalidation. Section 6.5.2 makes it the only way to
// invalidate the interrupt entry cache, and a unit that reports `IR` must report it too.
const ECAP_QI: u64 = 1 << 1;

// ECAP.C (bit 0): the unit's table walks snoop the processor caches. Clear means the hardware reads
// root, context and second-level entries from DRAM, so a write still in a cache line is invisible
// to it. See `Unit::publish`.
const ECAP_C: u64 = 1 << 0;

/// The `clflush` granule. CPUID leaf 1 reports it (EBX bits 15:8, in 8-byte units) and every x86_64
/// part this kernel has met reports 64; a smaller real line would leave lines unflushed, a larger
/// one only flushes some lines twice, so 64 is safe only while nothing reports less. Checked at
/// `initialize`, where a mismatch panics rather than walking stale tables.
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
// FSTS.IQE: the unit refused a descriptor in the invalidation queue and stopped reading it.
const FSTS_IQE: u32 = 1 << 4;

// Invalidation descriptors (section 6.5.2), 128 bits each; only the low qword carries fields here
// except the wait descriptor's status address. The type is bits 3:0.
const DESCRIPTOR_CONTEXT_CACHE_GLOBAL: u64 = 0x1 | 1 << 4; // granularity 01: global
const DESCRIPTOR_IOTLB_GLOBAL: u64 = 0x2 | 1 << 4; // granularity 01: global
const DESCRIPTOR_INTERRUPT_ENTRY_CACHE: u64 = 0x4; // G clear: global
const DESCRIPTOR_INTERRUPT_ENTRY_CACHE_INDEX: u64 = 0x4 | 1 << 4; // G set: the index in 47:32
const DESCRIPTOR_WAIT: u64 = 0x5 | 1 << 5 | 1 << 6; // status write, fence
/// The value a wait descriptor writes to its status word when every earlier descriptor is done.
const WAIT_DONE: u32 = 1;
/// The queue is one frame of 128-bit descriptors: `IQA.QS` = 0 is 256 of them.
const QUEUE_DESCRIPTORS: u64 = page_frames::FRAME_SIZE / 16;
/// The interrupt remapping table is one frame of 128-bit entries, `IRTA.S` = 7 (2^(S+1) = 256).
const REMAPPING_ENTRIES: u16 = (page_frames::FRAME_SIZE / 16) as u16;
const IRTA_SIZE_256: u64 = 7;

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
    /// The invalidation queue, on a unit that offers one (`ECAP.QI`). While it is on, every
    /// invalidation goes through it; register-based invalidation is then forbidden (section
    /// 6.5.2) and QEMU's model refuses it.
    queue: Option<InvalidationQueue>,
    /// The interrupt remapping table, once `GSTS.IRES` reads set. `None` is a unit that remaps
    /// nothing, and its devices' messages are delivered in compatibility format.
    remapping: Option<RemappingTable>,
    /// `ECAP.C`: this unit's table walks snoop the CPU caches. When clear, every table line the
    /// CPU writes must be written back before the unit is told to look ([`Unit::publish`]).
    coherent: bool,
}

/// **A unit's invalidation queue** (VT-d 4.1 section 6.5.2): one frame of descriptors the unit reads
/// from `IQH` up to the `IQT` software writes, and a status word a wait descriptor writes back.
/// Every batch ends with a wait, so the queue is empty between batches and never wraps onto
/// itself.
struct InvalidationQueue {
    base: u64,
    /// The next free slot, in descriptors.
    tail: u64,
    /// A frame whose first word the wait descriptor writes [`WAIT_DONE`] to.
    status: u64,
}

/// **A unit's interrupt remapping table** (VT-d 4.1 section 9.10): one frame of 256 entries, each
/// naming a vector, a destination and the requester id allowed to raise it.
struct RemappingTable {
    base: u64,
    /// The next entry to hand out. Entries are never freed (this module's BUGS).
    next: u16,
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

    /// Put one descriptor in the invalidation queue, without telling the unit. [`Unit::queue_wait`]
    /// moves `IQT` past it.
    fn queue_submit(&mut self, low: u64, high: u64) {
        let queue = self
            .queue
            .as_mut()
            .expect("a queued invalidation on a unit with no queue");
        let at = queue.base + queue.tail * 16;
        queue.tail = (queue.tail + 1) % QUEUE_DESCRIPTORS;
        // SAFETY: `at` is one of the 256 16-byte slots of the kernel-owned queue frame.
        unsafe {
            core::ptr::write_volatile(phys_to_virt(at) as *mut u64, low);
            core::ptr::write_volatile(phys_to_virt(at + 8) as *mut u64, high);
        }
        self.publish(at, 16);
    }

    /// **Hand the unit every descriptor submitted since the last wait, and wait until it has done
    /// them all**: a wait descriptor whose status write lands only after every earlier descriptor
    /// completes (`FN` set), then the tail, then a bounded poll of the status word. A refused
    /// descriptor sets `FSTS.IQE` and stops the queue, and that is a bug in this driver, so it
    /// panics with the head the unit stopped at.
    fn queue_wait(&mut self) {
        let status = self
            .queue
            .as_ref()
            .expect("a queued invalidation on a unit with no queue")
            .status;
        // SAFETY: `status` is a kernel-owned frame; its first word is this queue's status word.
        unsafe { core::ptr::write_volatile(phys_to_virt(status) as *mut u32, 0) };
        self.publish(status, 4);
        self.queue_submit(DESCRIPTOR_WAIT | u64::from(WAIT_DONE) << 32, status);
        // The descriptors, and the cleared status word, reach memory before the tail tells the
        // unit to read them.
        mfence();
        crate::arch::direct_memory_access_write_barrier();
        let tail = self.queue.as_ref().map_or(0, |q| q.tail);
        w64(self.base, IQT, tail << 4);
        let mut spins = 0u32;
        loop {
            // A unit that cannot see the caches wrote memory, not the line this loop reads.
            self.publish(status, 4);
            // SAFETY: as above; the unit writes this word, so it is read volatile.
            if unsafe { core::ptr::read_volatile(phys_to_virt(status) as *const u32) } == WAIT_DONE
            {
                return;
            }
            assert!(
                r32(self.base, FSTS) & FSTS_IQE == 0,
                "VT-d unit {:#x} refused an invalidation descriptor (FSTS.IQE, IQH {:#x})",
                self.base,
                r64(self.base, IQH),
            );
            spins += 1;
            assert!(
                spins < 1_000_000,
                "VT-d unit {:#x} never finished its invalidation queue",
                self.base
            );
        }
    }

    /// Invalidate the interrupt entry cache: every entry when `index` is `None`, else that one
    /// (section 6.5.2.7). Hardware may cache an entry from the moment the table pointer is set, so
    /// every write to the table is followed by one of these.
    fn invalidate_interrupt_entries(&mut self, index: Option<u16>) {
        let low = match index {
            None => DESCRIPTOR_INTERRUPT_ENTRY_CACHE,
            Some(i) => DESCRIPTOR_INTERRUPT_ENTRY_CACHE_INDEX | u64::from(i) << 32,
        };
        self.queue_submit(low, 0);
        self.queue_wait();
    }

    /// **Write remapping entry `handle` and invalidate it**, the requester id half first and the
    /// half holding the present bit last, so the unit never reads a present entry naming the
    /// previous requester. A rewrite of a present entry changes only the low half (an IO APIC line
    /// moved to another vector or core keeps its requester id), and one 64-bit store is atomic.
    fn write_remapping_entry(&mut self, handle: u16, entry: [u64; 2]) {
        let Some(table) = &self.remapping else {
            return;
        };
        let at = table.base + u64::from(handle) * 16;
        // SAFETY: `handle` is below `REMAPPING_ENTRIES`, so `at` is one of the 256 16-byte entries
        // of the kernel-owned table frame.
        unsafe {
            let p = phys_to_virt(at) as *mut u64;
            if core::ptr::read_volatile(p) == entry[0]
                && core::ptr::read_volatile(p.add(1)) == entry[1]
            {
                return;
            }
            core::ptr::write_volatile(p.add(1), entry[1]);
            crate::arch::direct_memory_access_write_barrier();
            core::ptr::write_volatile(p, entry[0]);
        }
        self.publish(at, 16);
        mfence();
        self.invalidate_interrupt_entries(Some(handle));
    }

    /// The next free remapping entry, or `None` when all 256 are handed out.
    fn allocate_remapping_entry(&mut self) -> Option<u16> {
        let table = self.remapping.as_mut()?;
        if table.next >= REMAPPING_ENTRIES {
            return None;
        }
        table.next += 1;
        Some(table.next - 1)
    }
}

// --- The interrupt remapping encodings (VT-d 4.1 sections 5.1 and 9.10). Pure functions of their
// arguments, so the proofs below can hold every input. ---

// Remapping table entry, low qword: present, trigger mode, vector, destination. Delivery mode
// (bits 7:5) is 000 Fixed, destination mode (bit 2) 0 physical, the format bit (15) 0 remapped
// rather than posted, and fault processing (bit 1) 0, so a refused message is recorded.
const ENTRY_PRESENT: u64 = 1 << 0;
const ENTRY_LEVEL_SHIFT: u64 = 4;
const ENTRY_VECTOR_SHIFT: u64 = 16;
// The destination field is bits 63:32; with `IRTA.EIME` clear the xAPIC id sits in its bits 15:8.
const ENTRY_XAPIC_DESTINATION_SHIFT: u64 = 40;
// High qword: the source id in 15:0, `SQ` in 17:16 (00: compare all sixteen bits) and `SVT` in
// 19:18 (01: verify the requester id against the source id).
const ENTRY_VERIFY_REQUESTER_ID: u64 = 0b01 << 18;

// A remappable-format message address (section 5.1.5.2): 0xfee in 31:20, handle bits 14:0 in
// 19:5, the format bit 4 set, `SHV` (bit 3) set so the data word is a subhandle, handle bit 15 in
// bit 2. The data word is the subhandle, which is zero.
const MESSAGE_ADDRESS_BASE: u64 = 0xfee0_0000;
const MESSAGE_REMAPPABLE_FORMAT: u64 = 1 << 4;
const MESSAGE_SUBHANDLE_VALID: u64 = 1 << 3;

// A remappable-format IO APIC redirection entry (section 5.1.5.1): handle bits 14:0 in 63:49, the
// format bit 48 set, handle bit 15 in bit 11, and the vector, which must equal the remapping
// entry's so an end of interrupt finds the line.
const REDIRECTION_REMAPPABLE_FORMAT: u64 = 1 << 48;
const REDIRECTION_ACTIVE_LOW: u64 = 1 << 13;
const REDIRECTION_LEVEL: u64 = 1 << 15;

/// **The remapping table entry that grants `vector` on the core whose xAPIC id is `destination`,
/// to requester `source_id` alone.** Present, remapped format, fixed delivery to one physical
/// destination, faults recorded, and the requester id verified on all sixteen bits, so a message
/// any other function raises with this entry's handle is refused. `level` is the trigger mode,
/// which an IO APIC line's entry must match. See notes/iommu.md.
const fn remapped_entry(vector: u8, destination: u8, source_id: u16, level: bool) -> [u64; 2] {
    [
        ENTRY_PRESENT
            | (level as u64) << ENTRY_LEVEL_SHIFT
            | (vector as u64) << ENTRY_VECTOR_SHIFT
            | (destination as u64) << ENTRY_XAPIC_DESTINATION_SHIFT,
        source_id as u64 | ENTRY_VERIFY_REQUESTER_ID,
    ]
}

/// **The message a device writes to raise remapping entry `handle`**: an address that names the
/// entry and nothing else, and a zero subhandle. The vector and destination are the entry's, so a
/// component that can write the device's MSI-X table can choose only which entry to name.
const fn remappable_message(handle: u16) -> MsiTarget {
    let handle = handle as u64;
    MsiTarget {
        address: MESSAGE_ADDRESS_BASE
            | (handle & 0x7fff) << 5
            | MESSAGE_REMAPPABLE_FORMAT
            | MESSAGE_SUBHANDLE_VALID
            | (handle >> 15) << 2,
        data: 0,
    }
}

/// **The IO APIC redirection entry that raises remapping entry `handle`**, unmasked, with the
/// line's own polarity and trigger mode and the entry's vector.
const fn remappable_redirection_entry(
    handle: u16,
    vector: u8,
    active_low: bool,
    level: bool,
) -> u64 {
    let handle = handle as u64;
    ((handle & 0x7fff) << 49)
        | REDIRECTION_REMAPPABLE_FORMAT
        | ((handle >> 15) << 11)
        | ((level as u64) * REDIRECTION_LEVEL)
        | ((active_low as u64) * REDIRECTION_ACTIVE_LOW)
        | vector as u64
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
    slots: [Slot; MAXIMUM_TRANSLATION_UNITS],
    /// Per unit, one context-table root per PCI bus, allocated the first time a device on that
    /// bus is attached. `None` is the default-deny state: a bus with no table here has no
    /// root-table entry pointing at one either, so every device on it faults.
    ctx: [[Option<u64>; 256]; MAXIMUM_TRANSLATION_UNITS],
    /// The unit the IO APIC's lines reach and the requester id its messages carry, from the
    /// DMAR's IO APIC scope. `None` on a machine with no IO APIC, or one whose IO APIC no scope
    /// names, where no unit remaps.
    io_apic: Option<(usize, u16)>,
    /// The remapping entry each redirection entry raises, by redirection index, or
    /// [`NO_HANDLE`] before [`remap_io_apic_line`] first routes it.
    io_apic_handles: [u16; 256],
}

/// The value in [`Units::io_apic_handles`] for a line with no remapping entry yet.
const NO_HANDLE: u16 = u16::MAX;

impl Units {
    fn iter(&self) -> core::slice::Iter<'_, Slot> {
        self.slots.iter()
    }
}

static IOMMU: InterruptSafeMutex<Units> = InterruptSafeMutex::new(
    rank::IOMMU,
    Units {
        slots: [const { Slot::Absent }; MAXIMUM_TRANSLATION_UNITS],
        ctx: [[None; 256]; MAXIMUM_TRANSLATION_UNITS],
        io_apic: None,
        io_apic_handles: [NO_HANDLE; 256],
    },
);

/// Every unit, device scope and RMRR the DMAR described, so [`attach`] can route a device to its
/// owner and [`scope_of`] can say which unit that is. Recorded by [`initialize`]; never held at the
/// same time as [`IOMMU`], because resolving a scope path reads configuration space.
static DMAR: InterruptSafeMutex<Option<DmarUnits>> = InterruptSafeMutex::new(rank::IOMMU, None);

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
    let mut unit = Unit {
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
        queue: None,
        remapping: None,
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
    // The same for interrupt remapping and the invalidation queue, whose tables the kernel did
    // not build either. The queue goes off only once the unit has read all of it (section
    // 6.5.2), and register-based invalidation below needs it off.
    if r32(base, GSTS) & GSTS_IRES != 0 {
        global_command(base, GCMD_IRE, false);
        wait_gsts(base, "interrupt remapping disable", |v| v & GSTS_IRES == 0);
    }
    if r32(base, GSTS) & GSTS_QIES != 0 {
        let mut spins = 0u32;
        while r64(base, IQH) != r64(base, IQT) && r32(base, FSTS) & FSTS_IQE == 0 {
            spins += 1;
            assert!(
                spins < 1_000_000,
                "IOMMU firmware's invalidation queue never drained"
            );
        }
        global_command(base, GCMD_QIE, false);
        wait_gsts(base, "queued invalidation disable", |v| v & GSTS_QIES == 0);
    }

    w64(base, RTADDR, unit.root);
    global_command(base, GCMD_SRTP, true);
    wait_gsts(base, "set root table pointer", |v| v & GSTS_RTPS != 0);
    invalidate_all(&mut unit);

    // Queued invalidation from here on, wherever the unit offers it: the interrupt entry cache
    // has no other invalidation, and once the queue is on, register-based invalidation is
    // forbidden. The queue is empty, so the tail starts at zero.
    if ecap & ECAP_QI != 0 {
        let queue = InvalidationQueue {
            base: zeroed_page_frame("invalidation queue"),
            tail: 0,
            status: zeroed_page_frame("invalidation status"),
        };
        unit.publish(queue.base, page_frames::FRAME_SIZE);
        mfence();
        w64(base, IQT, 0);
        // `IQA.DW` clear (128-bit descriptors) and `IQA.QS` zero (one frame, 256 of them).
        w64(base, IQA, queue.base);
        global_command(base, GCMD_QIE, true);
        wait_gsts(base, "queued invalidation enable", |v| v & GSTS_QIES != 0);
        unit.queue = Some(queue);
    }
    Ok(unit)
}

/// **Turn interrupt remapping on in one unit** (VT-d 4.1 section 9.10 and 11.4.4): an all-absent
/// table, its pointer set, the entry cache invalidated (the step section 11.4.4 requires after
/// `SIRTP`), compatibility-format messages blocked, then `GCMD.IRE`. From here a message reaches a
/// core only through an entry [`remap_message`] or [`remap_io_apic_line`] wrote. `Err` is a unit
/// that cannot, with the reason the tour prints.
fn remapping_up(u: &mut Unit) -> Result<(), &'static str> {
    if !u.interrupt_remapping {
        return Err("ECAP.IR is clear");
    }
    if u.queue.is_none() {
        return Err("ECAP.QI is clear, so the interrupt entry cache cannot be invalidated");
    }
    let table = zeroed_page_frame("interrupt remapping table");
    u.publish(table, page_frames::FRAME_SIZE);
    mfence();
    // `IRTA.EIME` clear: destinations are xAPIC ids (this module's BUGS).
    w64(u.base, IRTA, table | IRTA_SIZE_256);
    global_command(u.base, GCMD_SIRTP, true);
    wait_gsts(u.base, "set interrupt remapping table pointer", |v| {
        v & GSTS_IRTPS != 0
    });
    u.invalidate_interrupt_entries(None);
    if r32(u.base, GSTS) & GSTS_CFIS != 0 {
        global_command(u.base, GCMD_CFI, false);
        wait_gsts(u.base, "compatibility format block", |v| v & GSTS_CFIS == 0);
    }
    global_command(u.base, GCMD_IRE, true);
    wait_gsts(u.base, "interrupt remapping enable", |v| v & GSTS_IRES != 0);
    u.remapping = Some(RemappingTable {
        base: table,
        next: 0,
    });
    Ok(())
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
/// an RMRR device, anywhere outside its reserved regions). Last, every unit the platform and its
/// own `ECAP.IR` say can remap turns remapping on, provided the DMAR names the scope of
/// `io_apic_id`, the MADT's IO APIC. That is milestone 1068 (the kernel turns interrupt
/// remapping on where an IOMMU offers it).
///
/// Name: provisional. Spelled out under the `init` ruling of 2026-10-26 (UTC), in
/// design/naming/acronyms-and-abbreviations.md.
pub fn initialize(dmar: &DmarUnits, io_apic_id: Option<u8>) {
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
    let mut done = [0u32; MAXIMUM_RESERVED_REGION_DEVICES];
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
    drop(g);

    // Step 4. Resolving the IO APIC's scope path reads configuration space, so before the lock.
    let io_apic = io_apic_id.map(|id| {
        (
            id,
            dmar.interrupt_controller_source(id, &mut crate::pci::bridge_bus_range),
        )
    });
    let mut remapping = [Err("not tried"); MAXIMUM_TRANSLATION_UNITS];
    let mut g = IOMMU.lock();
    g.io_apic = io_apic.and_then(|(_, found)| found);
    for (i, s) in g.slots.iter_mut().enumerate() {
        let Slot::Up(u) = s else { continue };
        remapping[i] = match io_apic {
            _ if !dmar.platform_interrupt_remapping => Err("the DMAR's INTR_REMAP is clear"),
            Some((_, None)) => {
                Err("no DMAR scope names the IO APIC, so its messages cannot be verified")
            }
            _ => remapping_up(u),
        };
    }
    let n = dmar.units().len();
    for (i, d) in dmar.units().iter().enumerate() {
        let what = if d.include_pci_all {
            "the catch-all"
        } else {
            "named devices only"
        };
        match &g.slots[i] {
            Slot::Up(u) => {
                crate::println!(
                    "  vt-d        : drhd {:#x} up ({} of {n}, {what}), translation enabled \
                     (gsts.tes confirmed), {} rmrr device(s) identity-mapped",
                    d.register_base,
                    i + 1,
                    u.reserved_devices,
                );
                match remapping[i] {
                    Ok(()) => crate::println!(
                        "  vt-d        : drhd {:#x} remaps interrupts (gsts.ires confirmed), \
                         compatibility format blocked",
                        d.register_base,
                    ),
                    Err(why) => crate::println!(
                        "  vt-d        : drhd {:#x} does not remap interrupts: {why}",
                        d.register_base,
                    ),
                }
            }
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

/// Copy the recorded DMAR into `out` under the lock, so the caller reads configuration space with
/// the lock released. The caller borrows the one copy it owns; returning the table by value would
/// make an unoptimised build keep a whole-table temporary at each step of the hand-off.
#[inline(never)]
fn copy_recorded_dmar(out: &mut Option<DmarUnits>) {
    *out = *DMAR.lock();
}

/// The index of the unit that owns `rid`, when that unit is one this kernel could ask. `None` is
/// a device no unit owns, or a DMAR recorded only in part. Resolving a scope path reads
/// configuration space, so this runs with neither lock held across it.
fn owner_index(rid: u32) -> Option<usize> {
    let mut recorded = None;
    copy_recorded_dmar(&mut recorded);
    let units = recorded.as_ref()?;
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
    let mut up = [false; MAXIMUM_TRANSLATION_UNITS];
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

/// **Does every VT-d unit that is up remap interrupts?** `None` when no unit is up, which is a
/// different answer from "a unit that does not". Every unit, because a unit that does not leaves
/// its own devices' messages unremapped. The confinement tests ask it before they forge a message.
///
/// Name: provisional (milestone 1068, 2026-11-11 UTC).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn interrupt_remapping_enabled() -> Option<bool> {
    if super::amd_vi::is_active() {
        return Some(false);
    }
    let g = IOMMU.lock();
    let mut any = false;
    let mut all = true;
    for s in g.iter() {
        if let Slot::Up(u) = s {
            any = true;
            all &= u.remapping.is_some();
        }
    }
    any.then_some(all)
}

/// **The message requester `rid` must write to deliver `vector` to the core whose xAPIC id is
/// `destination`** (milestone 1068). `Ok(Some)` is a remappable message naming a fresh remapping
/// entry that only `rid` may raise; `Ok(None)` is a requester no remapping unit owns, whose
/// messages no unit sees, so the caller writes the compatibility format; `Err` is a remapping
/// table with no entry left.
///
/// Name: provisional (milestone 1068, 2026-11-11 UTC).
pub(crate) fn remap_message(
    rid: u32,
    vector: u8,
    destination: u8,
) -> Result<Option<MsiTarget>, &'static str> {
    if super::amd_vi::is_active() {
        return Ok(None);
    }
    // Before the lock: `owner_index` reads configuration space.
    let Some(i) = owner_index(rid) else {
        return Ok(None);
    };
    let mut g = IOMMU.lock();
    let Slot::Up(u) = &mut g.slots[i] else {
        return Ok(None);
    };
    if u.remapping.is_none() {
        return Ok(None);
    }
    let handle = u
        .allocate_remapping_entry()
        .ok_or("every interrupt remapping entry on this unit is handed out")?;
    u.write_remapping_entry(
        handle,
        remapped_entry(vector, destination, rid as u16, false),
    );
    Ok(Some(remappable_message(handle)))
}

/// **The IO APIC redirection entry that delivers `vector` to xAPIC id `destination` for
/// redirection index `index`**, with the line's polarity and trigger mode (milestone 1068).
/// `Ok(Some)` is a remappable-format entry; `Ok(None)` is a machine where the IO APIC's unit
/// remaps nothing, so the caller writes the compatibility format. Each line keeps one remapping
/// entry for good, rewritten when the line moves, since `enable` routes a line again at every
/// acknowledgement.
///
/// Name: provisional (milestone 1068, 2026-11-11 UTC).
pub(crate) fn remap_io_apic_line(
    index: u32,
    vector: u8,
    destination: u8,
    active_low: bool,
    level: bool,
) -> Result<Option<u64>, &'static str> {
    if super::amd_vi::is_active() {
        return Ok(None);
    }
    let mut g = IOMMU.lock();
    let Units {
        slots,
        io_apic,
        io_apic_handles,
        ..
    } = &mut *g;
    let Some((unit, source_id)) = *io_apic else {
        return Ok(None);
    };
    let Slot::Up(u) = &mut slots[unit] else {
        return Ok(None);
    };
    if u.remapping.is_none() {
        return Ok(None);
    }
    let Some(handle) = io_apic_handles.get_mut(index as usize) else {
        return Err("the redirection index is past the 256 lines this driver tracks");
    };
    if *handle == NO_HANDLE {
        *handle = u
            .allocate_remapping_entry()
            .ok_or("every interrupt remapping entry on this unit is handed out")?;
    }
    let handle = *handle;
    u.write_remapping_entry(
        handle,
        remapped_entry(vector, destination, source_id, level),
    );
    Ok(Some(remappable_redirection_entry(
        handle, vector, active_low, level,
    )))
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
                    "  iommu            : VT-d drhd at {:#018x}, root table default-deny, translating, \
                     {}",
                    u.base,
                    InterruptRemappingClaims {
                        unit: u.interrupt_remapping,
                        platform,
                        enabled: u.remapping.is_some(),
                    },
                );
            }
            Slot::Refused(why) => {
                any = true;
                crate::println!("  iommu            : VT-d unit refused: {why}");
            }
            Slot::Absent => {}
        }
    }
    if !any {
        crate::println!(
            "  iommu            : none (this machine's ACPI names neither a DMAR nor an IVRS); device \
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

/// Global invalidation: the context cache first (a stale context entry would still point
/// translation at the previous domain), then the IOTLB. Queued where the unit has a queue, else a
/// write-and-poll on each one's own register. Both use global granularity; see notes/iommu.md's
/// BUGS for the cost.
fn invalidate_all(s: &mut Unit) {
    if s.rwbf {
        global_command(s.base, GCMD_WBF, true);
        wait_gsts(s.base, "write buffer flush", |v| v & GSTS_WBFS == 0);
    }
    if s.queue.is_some() {
        s.queue_submit(DESCRIPTOR_CONTEXT_CACHE_GLOBAL, 0);
        s.queue_submit(DESCRIPTOR_IOTLB_GLOBAL, 0);
        s.queue_wait();
        return;
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
    let Units { slots, ctx, .. } = &mut *g;
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

/// Proofs that a remapping entry grants exactly what the kernel asked for, and that a message
/// names exactly one entry (milestone 1068). They run on an `x86_64` host, where `crate::arch` is
/// this architecture's. See notes/iommu.md.
#[cfg(kani)]
mod proofs {
    use super::*;

    /// **A remapping entry delivers exactly the vector granted, to exactly the core named, and
    /// only for the requester named.** Present, remapped rather than posted, fixed delivery,
    /// physical destination, faults recorded, and every reserved bit zero (QEMU's model faults an
    /// entry with any set, and so may silicon). `SVT` = 01 with `SQ` = 00 compares all sixteen
    /// bits of the requester id, so no other function, not even another function of the same
    /// device, passes the check.
    ///
    /// Falsification: replayable `kernel/falsifications/arch.x86_64.iommu.proofs.a_remapping_entry_grants_exactly_one_vector_to_exactly_one_requester.patch`
    #[kani::proof]
    fn a_remapping_entry_grants_exactly_one_vector_to_exactly_one_requester() {
        let vector: u8 = kani::any();
        let destination: u8 = kani::any();
        let source_id: u16 = kani::any();
        let level: bool = kani::any();
        let [low, high] = remapped_entry(vector, destination, source_id, level);

        assert!(low & 1 == 1, "present");
        assert!(
            low >> 1 & 1 == 0,
            "fault processing enabled: a refusal is recorded"
        );
        assert!(low >> 2 & 1 == 0, "physical destination mode");
        assert!(
            low >> 3 & 1 == 0,
            "no redirection hint: the one core named, not a lowest-priority pick"
        );
        assert!(low >> 4 & 1 == level as u64, "the trigger mode asked for");
        assert!(
            low >> 5 & 0b111 == 0,
            "fixed delivery: no NMI, SMI, INIT or ExtINT"
        );
        assert!(low >> 15 & 1 == 0, "remapped format, not posted");
        assert!(low >> 16 & 0xff == vector as u64, "the vector granted");
        assert!(
            low >> 32 == (destination as u64) << 8,
            "the xAPIC destination in DST[15:8], nothing else"
        );
        assert!(
            low & 0xff00_7000 == 0,
            "the low half's reserved bits are zero"
        );
        assert!(high & 0xffff == source_id as u64, "the requester id named");
        assert!(
            high >> 16 & 0b11 == 0,
            "SQ: all sixteen requester-id bits compared"
        );
        assert!(
            high >> 18 & 0b11 == 0b01,
            "SVT: the requester id is verified"
        );
        assert!(high >> 20 == 0, "the high half's reserved bits are zero");
    }

    /// **A message names one remapping entry and no vector, and two entries never share a
    /// message.** Every message is in remappable format at the interrupt address window, with the
    /// subhandle valid and zero and the high address half zero, so the handle it decodes to is the
    /// one granted; two handles give two addresses. An IO APIC line's entry is remappable, decodes
    /// to its own handle, carries the entry's vector (an end of interrupt matches on it) and
    /// arrives unmasked with no delivery-mode bits.
    ///
    /// Falsification: replayable `kernel/falsifications/arch.x86_64.iommu.proofs.a_message_names_exactly_one_remapping_entry.patch`
    #[kani::proof]
    fn a_message_names_exactly_one_remapping_entry() {
        let a: u16 = kani::any();
        let b: u16 = kani::any();
        let message = remappable_message(a);
        let address = message.address;
        assert!(
            address >> 20 == 0xfee,
            "inside the interrupt address window, high half zero"
        );
        assert!(
            address >> 4 & 1 == 1,
            "remappable format: never the compatibility format"
        );
        assert!(
            address >> 3 & 1 == 1 && message.data == 0,
            "subhandle valid and zero"
        );
        let decoded = (address >> 5 & 0x7fff | (address >> 2 & 1) << 15) as u16;
        assert!(decoded == a, "the address names the handle granted");
        if a != b {
            assert!(
                remappable_message(b).address != address,
                "two entries, two messages"
            );
        }

        let vector: u8 = kani::any();
        let active_low: bool = kani::any();
        let level: bool = kani::any();
        let entry = remappable_redirection_entry(a, vector, active_low, level);
        assert!(
            entry >> 48 & 1 == 1,
            "the IO APIC entry is in remappable format"
        );
        assert!(
            (entry >> 49 | (entry >> 11 & 1) << 15) as u16 == a,
            "it names its own handle"
        );
        assert!(
            entry & 0xff == vector as u64,
            "it carries the entry's vector"
        );
        assert!(
            entry >> 8 & 0b111 == 0 && entry >> 16 & 1 == 0,
            "fixed delivery, unmasked"
        );
        assert!(entry >> 15 & 1 == level as u64 && entry >> 13 & 1 == active_low as u64);
        assert!(
            entry & 0x0000_ffff_fffe_5000 == 0,
            "reserved and read-only bits zero"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Interrupt remapping is on in every unit whose hardware and platform offer it, and off in
    /// every other** (milestone 1068, the opposite of the claim milestone 317 (the
    /// interrupt-remapping flags) held). Where it is on,
    /// compatibility-format messages are blocked (`GSTS.CFIS` clear) and the table the unit walks
    /// is the one this driver built. On the default runner the unit and the DMAR both offer it, so
    /// a unit that stayed off there fails rather than passing as "not offered"; under
    /// `NIFE_INTREMAP=off` the same test checks the other half.
    #[test_case]
    fn interrupt_remapping_is_on_wherever_it_is_offered() {
        let Some(platform) = DMAR.lock().map(|d| d.platform_interrupt_remapping) else {
            crate::testing::skip!("this machine's ACPI names no DMAR, so there is no unit to ask");
        };
        let g = IOMMU.lock();
        let io_apic_named = g.io_apic.is_some();
        for slot in g.iter() {
            let Slot::Up(u) = slot else { continue };
            let status = r32(u.base, GSTS);
            let offered = u.interrupt_remapping && platform && u.queue.is_some();
            if !offered {
                assert_eq!(
                    status & GSTS_IRES,
                    0,
                    "drhd {:#x} remaps interrupts though it or the platform does not offer it",
                    u.base
                );
                continue;
            }
            assert!(
                io_apic_named,
                "drhd {:#x} offers remapping, and no DMAR scope named the IO APIC",
                u.base
            );
            assert_ne!(
                status & GSTS_IRES,
                0,
                "drhd {:#x} offers remapping and GSTS.IRES is clear",
                u.base
            );
            assert_eq!(
                status & GSTS_CFIS,
                0,
                "drhd {:#x} lets compatibility-format interrupts through",
                u.base
            );
            let table = u.remapping.as_ref().map(|t| t.base);
            assert_eq!(
                Some(r64(u.base, IRTA) & !0xfff),
                table,
                "drhd {:#x} walks a remapping table this driver did not build",
                u.base
            );
        }
    }

    /// **A granted MSI names a remapping entry, and the entry names the vector and the requester**
    /// (milestone 1068). A fresh grant for the first virtio disk reads back from the table it was
    /// written to, present and bound to that disk's requester id, and its message is in
    /// remappable format, so the address carries no vector at all.
    #[test_case]
    fn a_granted_message_names_an_entry_bound_to_its_requester() {
        if interrupt_remapping_enabled() != Some(true) {
            crate::testing::skip!("no VT-d unit remaps interrupts on this machine");
        }
        let Some(d) = crate::pci::find_block_device() else {
            crate::testing::skip!("no PCI disk to grant a message to");
        };
        let vector = 0xc3;
        let target = remap_message(d.rid, vector, 0)
            .expect("a remapping entry was free")
            .expect("the disk's unit remaps, so its message is remapped");
        assert_ne!(target.address & MESSAGE_REMAPPABLE_FORMAT, 0);
        let handle = ((target.address >> 5) & 0x7fff | ((target.address >> 2) & 1) << 15) as u16;
        let i = owner_index(d.rid).expect("the disk has an owner");
        let g = IOMMU.lock();
        let Slot::Up(u) = &g.slots[i] else {
            panic!("the disk's unit is not up");
        };
        let base = u.remapping.as_ref().expect("the unit remaps").base;
        // SAFETY: `handle` came from this table's allocator, so the entry is inside its frame.
        let entry = unsafe {
            let p = phys_to_virt(base + u64::from(handle) * 16) as *const u64;
            [
                core::ptr::read_volatile(p),
                core::ptr::read_volatile(p.add(1)),
            ]
        };
        assert_eq!(entry, remapped_entry(vector, 0, d.rid as u16, false));
    }

    /// The fault reason a unit records for a remappable message whose entry names another
    /// requester (VT-d 4.1 section 7.1.3, "invalid source-id").
    const FAULT_SOURCE_ID: u32 = 0x26;
    /// ... and for a compatibility-format message while `CFIS` is clear.
    const FAULT_COMPATIBILITY_FORMAT: u32 = 0x25;

    /// The fault the unit recorded for requester `rid`, polled a bounded while.
    fn fault_for(rid: u32) -> Option<Fault> {
        for _ in 0..200_000 {
            match take_fault() {
                Some(f) if f.rid == rid => return Some(f),
                Some(_) => {}
                None => core::hint::spin_loop(),
            }
        }
        None
    }

    /// One MSI-X table entry's address and data, read or written as the device sees them.
    fn read_message(entry: u64) -> (u64, u32) {
        // SAFETY: `entry` is an MSI-X table entry inside a placed BAR, which the direct map covers
        // device-typed; the first three words are its address and data.
        unsafe {
            let p = phys_to_virt(entry) as *const u32;
            (
                u64::from(core::ptr::read_volatile(p))
                    | u64::from(core::ptr::read_volatile(p.add(1))) << 32,
                core::ptr::read_volatile(p.add(2)),
            )
        }
    }
    fn write_message(entry: u64, address: u64, data: u32) {
        // SAFETY: as `read_message`.
        unsafe {
            let p = phys_to_virt(entry) as *mut u32;
            core::ptr::write_volatile(p, address as u32);
            core::ptr::write_volatile(p.add(1), (address >> 32) as u32);
            core::ptr::write_volatile(p.add(2), data);
        }
    }

    /// **A message forged in one device's MSI-X table, naming a vector granted to another, is
    /// refused** (milestone 1068 step 5, in the shape of the confinement tests of
    /// milestone 202 (every confinement test is a ritual until somebody breaks the confinement)). The
    /// forger is the second virtio disk and the grant the first's: the test plays the component
    /// that holds the forger's MSI-X table page, which is what a userspace bus server holds.
    ///
    /// With interrupts masked, so the local APIC's request register is the witness, it shows
    /// three things in order. The forger's own message lands on the forger's own vector, so the
    /// request really raises an interrupt. The first disk's message, copied into the forger's
    /// table, does not land, and the unit records an invalid-source-id fault for the forger. And a
    /// compatibility-format message aimed straight at the first disk's vector is blocked, with its
    /// own fault reason. QEMU's model (`vtd_interrupt_remap_msi`, QEMU 10.2) passes the
    /// compatibility format whatever `GCMD.CFI` says, so that last half is asserted only on bare
    /// metal (notes/iommu.md's BUGS); a guest says which it is in `CPUID.1:ECX[31]`.
    #[test_case]
    fn a_message_forged_from_another_devices_grant_is_refused() {
        if interrupt_remapping_enabled() != Some(true) {
            crate::testing::skip!("no VT-d unit remaps interrupts on this machine");
        }
        // Quiesce first: finding a disk brings it up again, which grants it a fresh vector and
        // rewrites its MSI-X entry, so the two disks below are found after the others are found.
        let Some(forger_rid) = crate::pci::find_block_device_n(1).map(|d| d.rid) else {
            crate::testing::skip!("two PCI disks are needed, one to grant and one to forge");
        };
        crate::virtio::quiesce_block_devices_except(forger_rid);
        let (Some(granted), Some(forger)) = (
            crate::pci::find_block_device_n(0),
            crate::pci::find_block_device_n(1),
        ) else {
            crate::testing::skip!("two PCI disks are needed, one to grant and one to forge");
        };
        assert_ne!(granted.rid, forger.rid);
        let granted_entry =
            crate::pci::msix_table_entry(granted.bdf).expect("the first disk has MSI-X");
        let forger_entry =
            crate::pci::msix_table_entry(forger.bdf).expect("the second disk has MSI-X");
        let (granted_address, granted_data) = read_message(granted_entry);
        let (own_address, own_data) = read_message(forger_entry);

        let dma = crate::memory::alloc_zeroed().expect("no DMA frame").addr();
        let id = crate::virtio::register(
            crate::virtio::Transport::pci(&forger),
            dma,
            page_frames::FRAME_SIZE,
            Some(forger.rid),
        );
        let was = super::super::interrupts::disable();
        while take_fault().is_some() {}
        // Whether `vector` lands within a bounded wait: the message follows the request's
        // completion, possibly on another host thread, so one read could miss a message that
        // lands, and a refusal is only believed after the whole wait.
        let pending = |vector: u32| {
            (0..200_000).any(|_| {
                core::hint::spin_loop();
                super::super::interrupt_controller::is_vector_pending(vector)
            })
        };
        assert!(
            !pending(granted.intid),
            "the first disk's vector was pending before the test"
        );

        assert!(
            crate::virtio::provoke_completion(id),
            "the second disk did not complete"
        );
        assert!(
            pending(forger.intid),
            "the second disk's own message did not land on its own vector {:#x}: the request \
             raises no interrupt, and the refusals below would prove nothing",
            forger.intid,
        );

        write_message(forger_entry, granted_address, granted_data);
        assert!(
            crate::virtio::provoke_completion(id),
            "the second disk did not complete"
        );
        assert!(
            !pending(granted.intid),
            "the second disk raised the first disk's vector {:#x} by copying its message: \
             interrupt remapping does not confine it",
            granted.intid,
        );
        let fault =
            fault_for(forger.rid).expect("the unit recorded no fault for the forged message");
        assert_eq!(
            fault.code, FAULT_SOURCE_ID,
            "the forged message faulted for another reason"
        );

        let destination = u64::from(super::super::interrupt_controller::local_apic_id());
        write_message(
            forger_entry,
            MESSAGE_ADDRESS_BASE | destination << 12,
            granted.intid,
        );
        assert!(
            crate::virtio::provoke_completion(id),
            "the second disk did not complete"
        );
        let landed = pending(granted.intid);
        let guest = core::arch::x86_64::__cpuid(1).ecx & 1 << 31 != 0;
        if guest && landed {
            crate::println!(
                "    this guest's VT-d model delivered a compatibility-format message with CFIS \
                 clear; bare metal refuses it (notes/iommu.md)"
            );
        } else {
            assert!(
                !landed,
                "a compatibility-format message reached vector {:#x} with remapping on",
                granted.intid,
            );
            let fault =
                fault_for(forger.rid).expect("no fault for the compatibility-format message");
            assert_eq!(fault.code, FAULT_COMPATIBILITY_FORMAT);
        }

        write_message(forger_entry, own_address, own_data);
        while take_fault().is_some() {}
        super::super::interrupts::restore(was);
        crate::virtio::unregister(id);
        crate::memory::free(page_frames::PageFrame::from_addr(dma));
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
