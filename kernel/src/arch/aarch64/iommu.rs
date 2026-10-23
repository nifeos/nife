//! The SMMUv3 driver: aarch64's IOMMU, in front of the PCIe bus.
//!
//! The SMMU translates every address a PCIe device emits through per-stream tables the kernel
//! programs (DECISIONS §20). The page tables are VMSAv8-64, built by the portable seam
//! (`paging::domain` via `crate::iommu`); this module owns the register file, the stream table,
//! the context descriptors, and the command and event queues the SMMU is driven by.
//!
//! Default deny: `init` sets `GBPA.ABORT`, installs an all-invalid stream table and only then sets
//! `SMMUEN`, so every stream aborts until `attach` writes its STE. See notes/iommu.md, "The two
//! arch drivers, structural twins", and its honest limits.

use crate::arch::mmu::phys_to_virt;
use crate::sync::{IrqSafeMutex, rank};

// --- Register file (offsets from the SMMU page; Arm IHI 0070, and QEMU's smmuv3-common.h) ---
const IDR1: u64 = 0x04;
const CR0: u64 = 0x20;
const CR0ACK: u64 = 0x24;
const CR1: u64 = 0x28;
const CR2: u64 = 0x2c;
const GBPA: u64 = 0x44;
const GERROR: u64 = 0x60;
const STRTAB_BASE: u64 = 0x80; // 64-bit
const STRTAB_BASE_CFG: u64 = 0x88;
const CMDQ_BASE: u64 = 0x90; // 64-bit
const CMDQ_PROD: u64 = 0x98;
const CMDQ_CONS: u64 = 0x9c;
const EVENTQ_BASE: u64 = 0xa0; // 64-bit
const EVENTQ_PROD: u64 = 0xa8;
const EVENTQ_CONS: u64 = 0xac;

// CR0 bits. Each write must be confirmed by the same bit appearing in CR0ACK.
const CR0_SMMUEN: u32 = 1 << 0;
const CR0_EVENTQEN: u32 = 1 << 2;
const CR0_CMDQEN: u32 = 1 << 3;

// GBPA: what happens to traffic while SMMUEN is 0. ABORT makes that window fail closed.
const GBPA_ABORT: u32 = 1 << 20;
const GBPA_UPDATE: u32 = 1 << 31;

// Queue geometry. A queue's base register carries LOG2SIZE in bits [4:0]; PROD/CONS carry an
// index plus a wrap bit at bit LOG2SIZE. 256 commands of 16 bytes and 128 events of 32 bytes are
// each exactly one frame.
const CMDQ_LOG2: u32 = 8;
const CMD_BYTES: u64 = 16;
const EVTQ_LOG2: u32 = 7;
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // used by take_fault, which only the confinement test calls
const EVT_BYTES: u64 = 32;

// The linear stream table: 64 STEs of 64 bytes, one frame. StreamID on this board is the PCIe
// requester id (the DTB's iommu-map is identity), so 64 covers bus 0's 32 devices twice over.
const STRTAB_LOG2: u32 = 6;
const STE_BYTES: u64 = 64;

// Commands (opcode in byte 0 of word 0).
const CMD_CFGI_STE: u32 = 0x03;
const CMD_CFGI_CD: u32 = 0x05;
const CMD_TLBI_NSNH_ALL: u32 = 0x30;
const CMD_SYNC: u32 = 0x46;

/// One recorded fault, in portable terms. `code` is the SMMU event type (0x10 is `F_TRANSLATION`).
/// Read only by the confinement test.
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

struct Smmu {
    base: u64,
    strtab: u64,
    /// One frame of CDs, one 64-byte CD per stream table entry index. Re-attaching a stream
    /// rewrites its CD in place and invalidates, so repeated test runs converge instead of
    /// leaking a frame per attach.
    cds: u64,
    cmdq: u64,
    cmdq_prod: u32,
    // Read by take_fault (the confinement test); no production fault handler yet.
    #[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
    evtq: u64,
    #[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
    evtq_cons: u32,
}

static SMMU: IrqSafeMutex<Option<Smmu>> = IrqSafeMutex::new(rank::IOMMU, None);

fn r32(base: u64, off: u64) -> u32 {
    // SAFETY: the SMMU register page is device-mapped (mmu::map_everything region 9, gated on the
    // DTB node); reads of these registers are side-effect-free.
    unsafe { core::ptr::read_volatile(phys_to_virt(base + off) as *const u32) }
}
fn w32(base: u64, off: u64, v: u32) {
    // SAFETY: as above.
    unsafe { core::ptr::write_volatile(phys_to_virt(base + off) as *mut u32, v) }
}
fn w64(base: u64, off: u64, v: u64) {
    // SAFETY: as above; the 64-bit registers are 8-byte aligned and QEMU accepts 8-byte stores.
    unsafe { core::ptr::write_volatile(phys_to_virt(base + off) as *mut u64, v) }
}

/// Allocate one zeroed frame and return its physical address. Queue memory, tables.
fn zeroed_page_frame(what: &str) -> u64 {
    let pa = crate::memory::alloc()
        .unwrap_or_else(|| panic!("no frame for the SMMU {what}"))
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

/// Set bits in CR0 and wait for CR0ACK to confirm. The ack register exists because CR0 changes
/// have global effect and the SMMU may complete them asynchronously; on QEMU the ack is
/// immediate, but polling is the architectural contract.
fn cr0_set(base: u64, bits: u32) {
    let cur = r32(base, CR0);
    w32(base, CR0, cur | bits);
    let mut spins = 0u32;
    while r32(base, CR0ACK) & bits != bits {
        spins += 1;
        assert!(
            spins < 1_000_000,
            "SMMU never acknowledged CR0 |= {bits:#x}"
        );
    }
}

/// Bring the SMMU up: queues live, stream table installed all-invalid, translation enabled.
/// `base` is the register page from the device tree. Everything aborts until `attach`.
pub fn init(base: u64) {
    let mut g = SMMU.lock();
    assert!(g.is_none(), "SMMU initialized twice");

    // Fail closed even before SMMUEN: without this, the reset state of GBPA lets every
    // transaction bypass translation while SMMUEN is 0. UPDATE latches the new value.
    w32(base, GBPA, GBPA_UPDATE | GBPA_ABORT);
    let mut spins = 0u32;
    while r32(base, GBPA) & GBPA_UPDATE != 0 {
        spins += 1;
        assert!(spins < 1_000_000, "SMMU GBPA update never completed");
    }

    let idr1 = r32(base, IDR1);
    let cmdqs_max = (idr1 >> 21) & 0x1f;
    let evtqs_max = (idr1 >> 16) & 0x1f;
    assert!(
        cmdqs_max >= CMDQ_LOG2 && evtqs_max >= EVTQ_LOG2,
        "SMMU queues smaller than expected (IDR1 {idr1:#x})"
    );

    let strtab = zeroed_page_frame("stream table");
    let cds = zeroed_page_frame("context descriptors");
    let cmdq = zeroed_page_frame("command queue");
    let evtq = zeroed_page_frame("event queue");

    // Linear stream table (FMT 0), 2^6 entries. An all-zero STE is invalid, and an invalid STE
    // aborts the transaction and records C_BAD_STE: the default-deny posture.
    w64(base, STRTAB_BASE, strtab);
    w32(base, STRTAB_BASE_CFG, STRTAB_LOG2);

    w64(base, CMDQ_BASE, cmdq | CMDQ_LOG2 as u64);
    w32(base, CMDQ_PROD, 0);
    w32(base, CMDQ_CONS, 0);
    w64(base, EVENTQ_BASE, evtq | EVTQ_LOG2 as u64);
    w32(base, EVENTQ_PROD, 0);
    w32(base, EVENTQ_CONS, 0);

    // Table/queue access attributes; zero (non-cacheable) is always legal and QEMU ignores it.
    w32(base, CR1, 0);
    w32(base, CR2, 0);

    cr0_set(base, CR0_CMDQEN);
    cr0_set(base, CR0_EVENTQEN);
    cr0_set(base, CR0_SMMUEN);

    *g = Some(Smmu {
        base,
        strtab,
        cds,
        cmdq,
        cmdq_prod: 0,
        evtq,
        evtq_cons: 0,
    });
}

/// **This machine's IOMMU, for the machine description** (milestone 268).
///
/// In this architecture's vocabulary; a machine with no IOMMU says so rather than printing a blank.
// The machine description and the boot self-test are the only callers, and both are
// `#[cfg(not(any(test, feature = "bench")))]`: a test boot exits through semihosting and a bench
// boot diverges into `bench::run`, so neither reads a bring-up transcript. Same treatment
// `memory::print_summary` already carries, and for the same reason.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub(crate) fn print_summary() {
    match SMMU.lock().as_ref() {
        Some(s) => crate::println!(
            "  iommu           : SMMUv3 at {:#018x}, stream table default-deny, translating",
            s.base,
        ),
        None => crate::println!(
            "  iommu           : none (this machine's device tree names no smmuv3 node)",
        ),
    }
}

/// **Build a DMA domain over exactly `regions`, in the format this IOMMU walks**, and return its
/// root. The SMMUv3's stage 1 translates with VMSAv8-64, the CPU's own format. One function per
/// architecture rather than a type alias since lane `amd-vi`, because `x86_64` has two IOMMUs that
/// walk two formats and only a running machine says which.
pub(crate) fn build_domain(regions: &[paging::domain::DmaRegion]) -> u64 {
    crate::iommu::build_identity::<paging::Aarch64>(regions)
}

/// Is the SMMU up? The portable seam asks this to decide whether attaching is possible.
pub fn is_active() -> bool {
    SMMU.lock().is_some()
}

/// **Whether requester `rid` is behind this IOMMU** (milestone 261 (the NVMe driver leaves the kernel)'s bench rehearsal): always, when
/// it is up. This tree brings the unit up against a device tree whose `iommu-map` is an identity
/// over the whole bus, so there is no second unit for a device to belong to. VT-d is the
/// architecture where that stops being true; see `arch::x86_64::iommu::scope_of`.
pub(crate) fn scope_of(_rid: u32) -> crate::iommu::Scope {
    if is_active() {
        crate::iommu::Scope::WholeBus
    } else {
        crate::iommu::Scope::NoIommu
    }
}

/// **Firmware-reserved DMA regions for requester `rid`: none on this architecture as this tree
/// brings it up** (milestone 594 (every VT-d unit translates its own devices)). The VT-d driver reports its RMRRs here so
/// [`crate::iommu::confine`] can map them into every domain. The SMMUv3 counterpart is an IORT RMR node or a device tree `reserved-memory` region with `iommu-addresses`; this tree reads neither, and no machine it boots publishes one.
pub(crate) fn for_each_reserved_region(
    _rid: u32,
    _each: &mut dyn FnMut(paging::domain::DmaRegion),
) {
}

/// Push one 16-byte command and (for our uses) wait for the SMMU to consume it. The queue is far
/// larger than any burst we issue, so treating every push as synchronous keeps the driver simple;
/// QEMU consumes on the PROD write.
fn cmd_push(s: &mut Smmu, cmd: [u32; 4]) {
    let idx = (s.cmdq_prod & ((1 << CMDQ_LOG2) - 1)) as u64;
    let slot = phys_to_virt(s.cmdq + idx * CMD_BYTES) as *mut u32;
    // SAFETY: the command queue frame is kernel-owned; idx is masked to the queue.
    unsafe {
        for (i, w) in cmd.iter().enumerate() {
            core::ptr::write_volatile(slot.add(i), *w);
        }
    }
    // The SMMU reads the command by DMA from its own viewpoint: publish the words before PROD.
    crate::arch::direct_memory_access_write_barrier();
    s.cmdq_prod = (s.cmdq_prod + 1) & ((1 << (CMDQ_LOG2 + 1)) - 1);
    w32(s.base, CMDQ_PROD, s.cmdq_prod);

    let mut spins = 0u32;
    while r32(s.base, CMDQ_CONS) & ((1 << (CMDQ_LOG2 + 1)) - 1) != s.cmdq_prod {
        spins += 1;
        assert!(
            spins < 1_000_000,
            "SMMU stopped consuming commands (GERROR {:#x}, CONS {:#x})",
            r32(s.base, GERROR),
            r32(s.base, CMDQ_CONS),
        );
    }
}

/// **Where stream `rid`'s two tables sit**, as a byte offset into the frame that holds them.
///
/// One function for both tables, which share the index and the 64-byte stride, so the two cannot
/// disagree and land one stream's entry on another's. Panics past the linear table; `mod proofs`
/// holds the bound against the stride.
fn table_offset(rid: u32) -> u64 {
    assert!(
        (rid as u64) < (1 << STRTAB_LOG2),
        "StreamID {rid} beyond the linear stream table"
    );
    rid as u64 * STE_BYTES
}

/// **The context descriptor for one stream**: the SMMU's `TTBR0_EL1` + `TCR_EL1` + `MAIR_EL1`,
/// as the eight 32-bit words the hardware reads out of memory.
///
/// Geometry mirrors the CPU's `TCR_EL1` (48-bit IOVAs, 4 KiB granule, no TTB1), because the same
/// paging code builds the domain. `ttb` is TTB0 `[51:4]` in dword 1, split across two words; the
/// high word is zero on this board, which is why `mod proofs` checks it.
const fn context_descriptor(ttb: u64, asid: u16) -> [u32; 8] {
    [
        16                           // T0SZ: 48-bit IOVA space, matching Aarch64::SPLIT_SHIFT
            | (0b01 << 8)            // IR0: walks are write-back cacheable (ignored by QEMU)
            | (0b01 << 10)           // OR0
            | (0b11 << 12)           // SH0: inner shareable
            | (1 << 30)              // EPD1: no TTB1, the high half does not exist for devices
            | (1 << 31), //             V
        0b101                        // IPS: 48-bit output
            | (1 << 9)               // AA64
            | (1 << 13)              // R: record faults from this stream to the event queue
            | (1 << 14)              // A: and abort the faulting transaction
            | ((asid as u32) << 16),
        ttb as u32 & 0xffff_fff0, //  TTB0[31:4]; the low flag bits stay zero
        (ttb >> 32) as u32 & 0xf_ffff, // TTB0[51:32]
        0,                        // TTB1 does not exist
        0,
        // MAIR0/MAIR1: the same palette mmu::init programs into MAIR_EL1 (slot 0 device, slot 1
        // normal write-back). QEMU does not model attributes; real silicon reads these.
        paging::aarch64::mair::VALUE as u32,
        (paging::aarch64::mair::VALUE >> 32) as u32,
    ]
}

/// **The stream table entry for one stream**: valid, stage-1 translate, one linear context
/// descriptor at `ctxptr`, as the sixteen 32-bit words the hardware reads.
///
/// Word 0 holds V, CONFIG, S1FMT and the low half of `S1ContextPtr`, so a misaligned `ctxptr`
/// could turn CONFIG into bypass. `S1CDMAX` is 0: one CD, no substreams.
const fn stream_table_entry(ctxptr: u64) -> [u32; 16] {
    let mut ste = [0u32; 16];
    ste[0] = 1 | (0b101 << 1) | (ctxptr as u32 & 0xffff_ffc0);
    ste[1] = (ctxptr >> 32) as u32 & 0xf_ffff;
    ste
}

/// Point stream `rid` at the domain rooted at `ttb` (a VMSAv8-64 table the portable seam built),
/// tagged `asid`, then invalidate so the SMMU drops any cached STE/CD/TLB entry for the previous
/// domain. After this returns, the device can reach exactly what the domain maps.
pub fn attach(rid: u32, ttb: u64, asid: u16) {
    let mut g = SMMU.lock();
    let s = g.as_mut().expect("SMMU attach before init");
    let off = table_offset(rid);

    let cd = phys_to_virt(s.cds + off) as *mut u32;
    // SAFETY: cds is a kernel-owned frame; `table_offset` bounds the offset to it.
    unsafe {
        for (i, w) in context_descriptor(ttb, asid).iter().enumerate() {
            core::ptr::write_volatile(cd.add(i), *w);
        }
    }

    let ctxptr = s.cds + off;
    let ste = phys_to_virt(s.strtab + off) as *mut u32;
    // SAFETY: strtab is a kernel-owned frame; `table_offset` bounds the offset to it.
    unsafe {
        for (i, w) in stream_table_entry(ctxptr).iter().enumerate() {
            core::ptr::write_volatile(ste.add(i), *w);
        }
    }

    // The tables are read by the SMMU, a separate observer: publish before telling it to look.
    crate::arch::direct_memory_access_write_barrier();

    // Drop whatever the SMMU cached for this stream: its STE, its CD, and every TLB entry (a
    // re-attach reuses the ASID with a brand-new table, so the broad TLBI is the correct one).
    cmd_push(s, [CMD_CFGI_STE, rid, 1, 0]); // leaf=1: just this STE
    cmd_push(s, [CMD_CFGI_CD, rid, 1, 0]);
    cmd_push(s, [CMD_TLBI_NSNH_ALL, 0, 0, 0]);
    cmd_push(s, [CMD_SYNC, 0, 0, 0]);
}

/// Pop one fault from the event queue, if any. The confinement test drains this to prove a DMA
/// escape was stopped by the hardware, not merely absent.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn take_fault() -> Option<Fault> {
    let mut g = SMMU.lock();
    let s = g.as_mut()?;
    let wrap_mask = (1u32 << (EVTQ_LOG2 + 1)) - 1;
    let prod = r32(s.base, EVENTQ_PROD) & wrap_mask;
    if prod == s.evtq_cons {
        return None;
    }
    let idx = (s.evtq_cons & ((1 << EVTQ_LOG2) - 1)) as u64;
    let evt = phys_to_virt(s.evtq + idx * EVT_BYTES) as *const u32;
    // SAFETY: the event queue frame is kernel-owned; the SMMU wrote this entry before advancing
    // PROD, and idx is masked to the queue.
    let (code, rid, lo, hi) = unsafe {
        (
            core::ptr::read_volatile(evt) & 0xff,
            core::ptr::read_volatile(evt.add(1)),
            core::ptr::read_volatile(evt.add(4)),
            core::ptr::read_volatile(evt.add(5)),
        )
    };
    s.evtq_cons = (s.evtq_cons + 1) & wrap_mask;
    w32(s.base, EVENTQ_CONS, s.evtq_cons);
    Some(Fault {
        rid,
        code,
        addr: Some((hi as u64) << 32 | lo as u64),
    })
}

/// **Proofs over the word-building that decides which physical addresses a device may touch**
/// (`design/fatal-risks/README.md` risk 7).
///
/// The register accessors are unreachable to a harness, so `init`, `cmd_push`, `attach` and
/// `take_fault` are unproved; so are the register offsets and bit constants, which only Arm IHI
/// 0070 states. Nothing here covers the RISC-V IOMMU or VT-d. See notes/iommu.md, "What is proved,
/// as against tested", and notes/kernel-proofs.md, "The stub boundary, enumerated".
#[cfg(kani)]
mod proofs {
    use super::*;

    /// A physical address is at most 52 bits on VMSAv8-64, which is also the width of both fields
    /// below. Spelled once so the harnesses and the assumptions cannot drift apart.
    const PA_BITS: u32 = 52;

    /// **The SMMU is handed exactly the tables the kernel built, and nothing else.**
    ///
    /// Both entries split an address across two words and share the low word with control bits:
    /// a lost high half walks another table, and an address bit on CONFIG sets the stream to
    /// bypass. QEMU's RAM is below 4 GiB, so no test sees either. Assumes a page-frame root and a
    /// 64-byte-aligned descriptor, which `attach`'s callers establish.
    ///
    /// Falsification: replayable `kernel/falsifications/arch.aarch64.iommu.proofs.the_smmu_is_handed_exactly_the_tables_the_kernel_built.patch`
    #[kani::proof]
    fn the_smmu_is_handed_exactly_the_tables_the_kernel_built() {
        let ttb: u64 = kani::any();
        let ctxptr: u64 = kani::any();
        let asid: u16 = kani::any();

        // `ttb` is the root of a domain built by `paging::domain`, so a page frame.
        kani::assume(ttb % page_frames::FRAME_SIZE == 0);
        kani::assume(ttb < 1 << PA_BITS);
        // `ctxptr` is `cds + table_offset(rid)`: a page frame plus a 64-byte stride.
        kani::assume(ctxptr % STE_BYTES == 0);
        kani::assume(ctxptr < 1 << PA_BITS);

        let cd = context_descriptor(ttb, asid);
        let ste = stream_table_entry(ctxptr);

        // Over the whole dword, so a bit gained fails as surely as a bit lost.
        let ttb_dword = ((cd[3] as u64) << 32) | cd[2] as u64;
        assert!(
            ttb_dword == ttb,
            "the CD's TTB0 is the table root entire, high half included"
        );

        // The pointer survives, and no address bit reached V, CONFIG or S1FMT.
        let ste_dword = ((ste[1] as u64) << 32) | ste[0] as u64;
        assert!(
            ste_dword & 0x000f_ffff_ffff_ffc0 == ctxptr,
            "the STE's S1ContextPtr is the context descriptor entire, high half included"
        );
        assert!(
            ste_dword & 0x3f == 0b00_101_1,
            "V is set, CONFIG is stage-1 translate, S1FMT is linear: no address bit reached them"
        );

        assert!(cd[0] >> 31 == 1, "the context descriptor is valid");
        assert!(cd[0] & 0x3f == 16, "T0SZ: a 48-bit IOVA space");
        assert!((cd[0] >> 6) & 0b11 == 0, "TG0: a 4 KiB granule");
        assert!(
            cd[1] >> 16 == asid as u32 && cd[1] & 0x6000 == 0x6000,
            "the ASID is this stream's, and faults are still recorded and aborted"
        );
    }

    /// **No stream can reach another stream's tables.**
    ///
    /// `table_offset`'s `assert!` is all that stands between a device-supplied 16-bit `StreamID`
    /// and a raw write. Proves everything it admits fits the frame and no two ids overlap.
    ///
    /// Falsification: replayable `kernel/falsifications/arch.aarch64.iommu.proofs.no_stream_can_reach_another_streams_tables.patch`
    #[kani::proof]
    fn no_stream_can_reach_another_streams_tables() {
        let a: u32 = kani::any();
        let b: u32 = kani::any();
        // The requester id a PCIe function stamps on its transactions is 16 bits wide
        // (`pci::Bdf::requester_id`), and `virt`'s `iommu-map` is the identity.
        kani::assume(a <= u16::MAX as u32 && b <= u16::MAX as u32);
        // Anything `table_offset` refuses never reaches the arithmetic, and a refusal is a panic
        // rather than a silent wrong answer. Restrict to what it admits.
        kani::assume((a as u64) < (1 << STRTAB_LOG2) && (b as u64) < (1 << STRTAB_LOG2));

        let (oa, ob) = (table_offset(a), table_offset(b));

        assert!(
            oa + STE_BYTES <= page_frames::FRAME_SIZE,
            "every entry the bound admits lies inside the single frame that holds the table"
        );
        assert!(
            a == b || oa + STE_BYTES <= ob || ob + STE_BYTES <= oa,
            "two different streams never share a byte of either table"
        );
    }
}
