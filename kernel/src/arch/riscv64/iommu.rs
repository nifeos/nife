//! The RISC-V IOMMU (v1.0.1) driver, in front of the PCIe bus: the twin of aarch64's SMMUv3.
//!
//! A device directory keyed by requester id holds each device's context, whose `iosatp` names an
//! Sv39 domain the portable seam built. A command queue carries invalidations and a fault queue
//! records blocked transactions. `init` installs an all-invalid directory (DECISIONS §20).
//!
//! See notes/iommu.md and notes/riscv-port/iommu.md, whose BUGS cover the unused MSI page table
//! and the untested base context format.

use crate::arch::mmu::phys_to_virt;
use crate::sync::{IrqSafeMutex, rank};

// --- Register file (offsets into BAR 0; RISC-V IOMMU spec ch. 5, QEMU's riscv-iommu-bits.h) ---
const CAPS: u64 = 0x00; // 64-bit
const FCTL: u64 = 0x08;
const DDTP: u64 = 0x10; // 64-bit
const CQB: u64 = 0x18; // 64-bit
const CQH: u64 = 0x20;
const CQT: u64 = 0x24;
const FQB: u64 = 0x28; // 64-bit
const FQH: u64 = 0x30;
const FQT: u64 = 0x34;
const CQCSR: u64 = 0x48;
const FQCSR: u64 = 0x4c;

const CAP_SV39: u64 = 1 << 9;
const CAP_MSI_FLAT: u64 = 1 << 22;

// ddtp: mode in bits [3:0], busy at 4, PPN at [53:10]. Mode 2 = one-level device directory.
const DDTP_MODE_1LVL: u64 = 2;
const DDTP_BUSY: u64 = 1 << 4;

// cqcsr/fqcsr: enable at 0, active ("on") at 16, busy at 17; error bits 8..11 for diagnostics.
const QUEUE_EN: u32 = 1 << 0;
const QUEUE_ON: u32 = 1 << 16;
const QUEUE_BUSY: u32 = 1 << 17;

// Queue geometry: the base register's bits [4:0] hold log2(entries) - 1, PPN at [53:10]. One
// frame each: 256 commands of 16 bytes; 128 fault records of 32 bytes.
const CQ_LOG2: u32 = 8;
const CMD_BYTES: u64 = 16;
const FQ_LOG2: u32 = 7;
const FQ_BYTES: u64 = 32;

// Commands: opcode bits [6:0], function bits [9:7] of dword 0.
const CMD_IOTINVAL_VMA: u64 = 1; // func 0
const CMD_IOFENCE_C: u64 = 2; // func 0
const CMD_IODIR_INVAL_DDT: u64 = 3; // func 0
const CMD_DV: u64 = 1 << 33; // IODIR: the DID field is valid
const CMD_DID_SHIFT: u32 = 40;

// Device context fields. tc.V validates the entry; fsc = iosatp: mode Sv39 (8) in [63:60], PPN
// in [43:0]. ta.PSCID in [31:12] tags this domain's entries in the IOMMU's address translation
// cache the way an ASID tags the CPU TLB.
const DC_TC_V: u64 = 1 << 0;
const FSC_MODE_SV39: u64 = 8 << 60;
const TA_PSCID_SHIFT: u32 = 12;

/// One recorded fault, in portable terms. `code` is the specification's CAUSE (15 = store/AMO page
/// fault, no mapping for that IOVA; 258 = invalid device-directory entry). Only tests read it.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
#[derive(Debug, Clone, Copy)]
pub struct Fault {
    pub rid: u32,
    pub code: u32,
    /// The device address that faulted. Always `Some` from this unit; `None` exists for `x86_64`'s
    /// AMD-Vi, whose QEMU model writes no address into its event records (see
    /// `arch/x86_64/amd_vi.rs`'s BUGS), so a test can tell "no address" from "address 0".
    pub addr: Option<u64>,
}

struct Iommu {
    base: u64,
    ddt: u64,
    /// The device context size: 64 bytes with `MSI_FLAT`, else 32, decided once at init.
    dc_bytes: u64,
    /// `CAPS.MSI_FLAT`: whether this unit offers the flat MSI page table, for the machine
    /// description. A capability, kept apart from the layout [`Self::dc_bytes`] it implies.
    msi_flat: bool,
    cq: u64,
    cq_tail: u32,
    // Read by take_fault (the confinement test); no production fault handler yet.
    #[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
    fq: u64,
    #[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
    fq_head: u32,
}

static IOMMU: IrqSafeMutex<Option<Iommu>> = IrqSafeMutex::new(rank::IOMMU, None);

fn r32(base: u64, off: u64) -> u32 {
    // SAFETY: the IOMMU's BAR lies inside the PCI window mapped device-typed by
    // mmu::map_everything; these register reads are side-effect-free.
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

/// A queue base register value: PPN in [53:10], log2(entries)-1 in [4:0].
fn queue_base(pa: u64, log2: u32) -> u64 {
    ((pa >> 12) << 10) | (log2 as u64 - 1)
}

/// Does device `rid` have a context in the one-level device directory, whose `dc_bytes`-long entries
/// fill one frame? This is the only thing between a requester id and a raw write, so it is a
/// function of its own: `no_device_can_reach_another_devices_context` assumes exactly what it
/// admits and proves the stride in `context_offset` agrees with it.
fn is_in_directory(rid: u32, dc_bytes: u64) -> bool {
    (rid as u64) < page_frames::FRAME_SIZE / dc_bytes
}

/// Where device `rid`'s context sits in the device directory, as a byte offset into its frame.
fn context_offset(rid: u32, dc_bytes: u64) -> u64 {
    assert!(
        is_in_directory(rid, dc_bytes),
        "device_id {rid} beyond the one-level device directory"
    );
    rid as u64 * dc_bytes
}

/// A device context's words in directory order: `tc`, `iohgatp`, `ta`, `fsc`, then the extended
/// format's four MSI words. The base (32-byte) format writes only the first four. Pure so that
/// `the_iommu_is_handed_exactly_the_domain_the_kernel_built` can read each field
/// back at the spec's bit positions.
fn device_context(root: u64, pscid: u16) -> [u64; 8] {
    [
        DC_TC_V,                          // tc: valid, faults reported (DTF clear)
        0,                                // iohgatp: Bare
        (pscid as u64) << TA_PSCID_SHIFT, // ta
        FSC_MODE_SV39 | (root >> 12),     // fsc = iosatp
        0,                                // msiptp: MODE Off (notes/riscv-port/iommu.md)
        0,                                // msi_addr_mask
        0,                                // msi_addr_pattern
        0,                                // reserved
    ]
}

/// `IODIR.INVAL_DDT` for exactly device `rid`: opcode 3, function 0, and DV set, so the IOMMU
/// honours the DID field rather than dropping every device's cached context.
fn iodir_inval_ddt(rid: u32) -> u64 {
    CMD_IODIR_INVAL_DDT | CMD_DV | ((rid as u64) << CMD_DID_SHIFT)
}

/// Enable a queue through its control/status register and wait for it to come on.
fn queue_enable(base: u64, csr: u64, what: &str) {
    w32(base, csr, QUEUE_EN);
    let mut spins = 0u32;
    loop {
        let v = r32(base, csr);
        if v & QUEUE_ON != 0 && v & QUEUE_BUSY == 0 {
            break;
        }
        spins += 1;
        assert!(
            spins < 1_000_000,
            "IOMMU {what} queue never came on ({v:#x})"
        );
    }
}

/// Bring the IOMMU up: queues live, an all-invalid device directory installed. From here every
/// device faults until `attach` writes its context.
pub fn init(base: u64) {
    let mut g = IOMMU.lock();
    assert!(g.is_none(), "IOMMU initialized twice");

    let caps = r64(base, CAPS);
    assert!(
        caps & CAP_SV39 != 0,
        "the IOMMU does not support Sv39 first-stage (caps {caps:#x}); the seam depends on it"
    );
    // MSI_FLAT widens the device context to 64 bytes. Silicon without MSI may report 32.
    let msi_flat = caps & CAP_MSI_FLAT != 0;
    let dc_bytes = if msi_flat { 64 } else { 32 };

    // Little-endian, wire-signaled interrupts off (we poll both queues).
    w32(base, FCTL, 0);

    let cq = zeroed_page_frame("command queue");
    w64(base, CQB, queue_base(cq, CQ_LOG2));
    queue_enable(base, CQCSR, "command");

    let fq = zeroed_page_frame("fault queue");
    w64(base, FQB, queue_base(fq, FQ_LOG2));
    queue_enable(base, FQCSR, "fault");

    // One page of invalid contexts: ids 0..63 in the extended format, all of bus 0. From this
    // write on, an unattached device is blocked with DDT_ENTRY_INVALID.
    let ddt = zeroed_page_frame("device directory");
    w64(base, DDTP, DDTP_MODE_1LVL | ((ddt >> 12) << 10));
    let mut spins = 0u32;
    while r64(base, DDTP) & DDTP_BUSY != 0 {
        spins += 1;
        assert!(spins < 1_000_000, "IOMMU ddtp write never completed");
    }

    *g = Some(Iommu {
        base,
        ddt,
        dc_bytes,
        msi_flat,
        cq,
        cq_tail: 0,
        fq,
        fq_head: 0,
    });
}

/// Print this machine's IOMMU line for the machine description, in this architecture's terms; a
/// machine with none says so.
// The machine description is the only caller, and test and bench boots do not print it.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub(crate) fn print_summary() {
    match IOMMU.lock().as_ref() {
        Some(s) => crate::println!(
            "  iommu           : riscv-iommu at {:#018x}, device directory default-deny, \
             translating, MSI page table {} (mode off)",
            s.base,
            if s.msi_flat { "offered" } else { "absent" },
        ),
        None => crate::println!(
            "  iommu           : none (no riscv-iommu-pci function on this machine's bus)",
        ),
    }
}

/// Build a DMA domain over exactly `regions` in Sv39, the format `iosatp` walks, and return its
/// root.
pub(crate) fn build_domain(regions: &[paging::domain::DmaRegion]) -> u64 {
    crate::iommu::build_identity::<paging::Sv39>(regions)
}

/// Whether the IOMMU is up, so the portable seam can attach.
pub fn is_active() -> bool {
    IOMMU.lock().is_some()
}

/// Whether requester `rid` is behind this IOMMU: the whole bus whenever it is up, since the
/// device tree's `iommu-map` is an identity over it.
pub(crate) fn scope_of(_rid: u32) -> crate::iommu::Scope {
    if is_active() {
        crate::iommu::Scope::WholeBus
    } else {
        crate::iommu::Scope::NoIommu
    }
}

/// Firmware-reserved DMA regions for requester `rid`, for [`crate::iommu::confine`]: none here.
///
/// A device-tree `reserved-memory` region with `iommu-addresses` would hold them; this tree reads
/// none.
pub(crate) fn for_each_reserved_region(
    _rid: u32,
    _each: &mut dyn FnMut(paging::domain::DmaRegion),
) {
}

/// Push one 16-byte command, `dword0` and `dword1`, and poll the head until the IOMMU consumes it.
fn cmd_push(s: &mut Iommu, dword0: u64, dword1: u64) {
    let entries = 1u32 << CQ_LOG2;
    let idx = (s.cq_tail % entries) as u64;
    let slot = phys_to_virt(s.cq + idx * CMD_BYTES) as *mut u64;
    // SAFETY: the command queue frame is kernel-owned; idx is masked to the queue.
    unsafe {
        core::ptr::write_volatile(slot, dword0);
        core::ptr::write_volatile(slot.add(1), dword1);
    }
    // The IOMMU reads the command from memory: publish the words before moving the tail.
    crate::arch::direct_memory_access_write_barrier();
    s.cq_tail = (s.cq_tail + 1) % (2 * entries);
    w32(s.base, CQT, s.cq_tail);

    let mut spins = 0u32;
    while r32(s.base, CQH) != s.cq_tail {
        spins += 1;
        assert!(
            spins < 1_000_000,
            "IOMMU stopped consuming commands (cqcsr {:#x})",
            r32(s.base, CQCSR),
        );
    }
}

/// Point device `rid` at the Sv39 domain at `root`, tagged `pscid`, then invalidate whatever the
/// IOMMU cached for its previous domain.
///
/// Context: takes the `IOMMU` lock.
pub fn attach(rid: u32, root: u64, pscid: u16) {
    let mut g = IOMMU.lock();
    let s = g.as_mut().expect("IOMMU attach before init");
    let offset = context_offset(rid, s.dc_bytes);
    let words = device_context(root, pscid);

    // The valid bit last, after a barrier, so the IOMMU never sees a valid entry with a stale root.
    let dc = phys_to_virt(s.ddt + offset) as *mut u64;
    // SAFETY: ddt is a kernel-owned frame; `context_offset` bounds rid so the whole
    // `dc_bytes`-long entry lies inside it.
    unsafe {
        core::ptr::write_volatile(dc.add(1), words[1]); // iohgatp: Bare
        core::ptr::write_volatile(dc.add(2), words[2]); // ta
        core::ptr::write_volatile(dc.add(3), words[3]); // fsc = iosatp
        if s.dc_bytes == 64 {
            for (i, &w) in words.iter().enumerate().skip(4) {
                core::ptr::write_volatile(dc.add(i), w); // msiptp, mask, pattern, reserved
            }
        }
    }
    crate::arch::direct_memory_access_write_barrier();
    // SAFETY: as above.
    unsafe {
        core::ptr::write_volatile(dc, words[0]); // tc: valid, faults reported (DTF clear)
    }
    crate::arch::direct_memory_access_write_barrier();

    // IODIR drops the cached context, IOTINVAL.VMA every translation, and IOFENCE.C orders both
    // before any later transaction.
    cmd_push(s, iodir_inval_ddt(rid), 0);
    cmd_push(s, CMD_IOTINVAL_VMA, 0);
    cmd_push(s, CMD_IOFENCE_C, 0);
}

/// Pop one fault from the fault queue, or `None`. The confinement test drains it to prove the
/// hardware stopped a DMA escape.
///
/// Context: takes the `IOMMU` lock.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn take_fault() -> Option<Fault> {
    let mut g = IOMMU.lock();
    let s = g.as_mut()?;
    let entries = 1u32 << FQ_LOG2;
    let tail = r32(s.base, FQT);
    if tail == s.fq_head {
        return None;
    }
    let idx = (s.fq_head % entries) as u64;
    let rec = phys_to_virt(s.fq + idx * FQ_BYTES) as *const u64;
    // SAFETY: the fault queue frame is kernel-owned; the IOMMU wrote this record before moving
    // the tail, and idx is masked to the queue.
    let (hdr, iotval) = unsafe {
        (
            core::ptr::read_volatile(rec),
            core::ptr::read_volatile(rec.add(2)),
        )
    };
    s.fq_head = (s.fq_head + 1) % (2 * entries);
    w32(s.base, FQH, s.fq_head);
    Some(Fault {
        rid: (hdr >> 40) as u32 & 0xff_ffff, // DID, bits [63:40]
        code: (hdr & 0xfff) as u32,          // CAUSE, bits [11:0]
        addr: Some(iotval),
    })
}

/// Proofs that a device context names exactly the domain the kernel built, and that no requester
/// id's entry or invalidation lands on another's.
///
/// They run on an aarch64 host, where `crate::arch` is aarch64's, so they prove only this file's
/// word arithmetic. See notes/riscv-port/iommu.md and notes/kernel-proofs.md, stub-list item 8.
#[cfg(kani)]
mod proofs {
    use super::*;

    /// A RISC-V physical address is at most 56 bits (a 44-bit PPN of 4 KiB pages), which is also
    /// the width of `iosatp`'s PPN field. Spelled once so the assumption and the field agree.
    const PA_BITS: u32 = 56;

    /// The IOMMU is handed exactly the domain the kernel built, and it is still translating.
    ///
    /// `iosatp.MODE` is Sv39 for every `root`, since MODE 0 is Bare; the PPN, PSCID and other words
    /// are exact for a page-frame root below 2^56. See notes/riscv-port/iommu.md.
    ///
    /// Falsification: replayable `kernel/falsifications/arch.riscv64.iommu.proofs.the_iommu_is_handed_exactly_the_domain_the_kernel_built.patch`
    #[kani::proof]
    fn the_iommu_is_handed_exactly_the_domain_the_kernel_built() {
        let root: u64 = kani::any();
        let pscid: u16 = kani::any();

        let dc = device_context(root, pscid);
        assert!(
            dc[3] >> 60 == 8,
            "iosatp.MODE is Sv39 for every root: no address bit can turn translation off"
        );

        // From `paging::domain`: a page frame in RAM.
        kani::assume(root % page_frames::FRAME_SIZE == 0);
        kani::assume(root < 1 << PA_BITS);

        // On the whole address, so the harness does not repeat the implementation's `>> 12`.
        assert!(
            (dc[3] & ((1 << 44) - 1)) << 12 == root,
            "iosatp.PPN is the table root entire"
        );
        assert!(
            (dc[3] >> 44) & 0xffff == 0,
            "iosatp's reserved bits [59:44] are zero: no address bit reached them"
        );
        assert!(
            (dc[2] >> 12) & 0xf_ffff == pscid as u64 && dc[2] & !(0xf_ffff << 12) == 0,
            "ta.PSCID is this domain's tag, and nothing else in ta is set"
        );
        assert!(
            dc[0] == 1,
            "tc.V is set and nothing else: DTF clear, so faults are reported"
        );
        assert!(
            dc[1] == 0,
            "iohgatp is Bare: no second stage to translate through"
        );
        assert!(
            dc[4..].iter().all(|&w| w == 0),
            "the extended format's MSI words are zero, msiptp.MODE Off"
        );
    }

    /// No device can reach another device's context, or invalidate the wrong one, in either
    /// context format.
    ///
    /// The directory bound agrees with the stride, and `IODIR.INVAL_DDT` decodes to exactly this
    /// device with DV set. See notes/riscv-port/iommu.md.
    ///
    /// Falsification: replayable `kernel/falsifications/arch.riscv64.iommu.proofs.no_device_can_reach_another_devices_context.patch`
    #[kani::proof]
    fn no_device_can_reach_another_devices_context() {
        let a: u32 = kani::any();
        let b: u32 = kani::any();
        let extended: bool = kani::any();
        let dc_bytes: u64 = if extended { 64 } else { 32 };
        // A PCIe requester id is 16 bits (`pci::Bdf::requester_id`).
        kani::assume(a <= u16::MAX as u32 && b <= u16::MAX as u32);
        // Assume through the same predicate, not a restatement, so a drifting bound turns red.
        kani::assume(is_in_directory(a, dc_bytes) && is_in_directory(b, dc_bytes));

        let (oa, ob) = (context_offset(a, dc_bytes), context_offset(b, dc_bytes));
        assert!(
            oa + dc_bytes <= page_frames::FRAME_SIZE,
            "every context the bound admits lies inside the directory's single frame"
        );
        assert!(
            oa % 8 == 0,
            "every context starts on the 8-byte boundary its 64-bit stores need"
        );
        assert!(
            a == b || oa + dc_bytes <= ob || ob + dc_bytes <= oa,
            "two different devices never share a byte of the directory"
        );

        let cmd = iodir_inval_ddt(a);
        assert!(
            cmd & 0x7f == 3 && (cmd >> 7) & 0b111 == 0,
            "the command is IODIR.INVAL_DDT"
        );
        assert!(
            (cmd >> 33) & 1 == 1,
            "DV is set, so the DID field is honoured"
        );
        assert!(
            cmd >> 40 == a as u64,
            "the invalidation names exactly the device just attached"
        );
    }
}
