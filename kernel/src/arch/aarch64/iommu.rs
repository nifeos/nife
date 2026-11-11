//! The SMMUv3 driver: aarch64's IOMMU, in front of the PCIe bus.
//!
//! The SMMU translates every address a PCIe device emits through per-stream tables the kernel
//! programs (§20 (IOMMU-backed DMA isolation)). The page tables are VMSAv8-64, built by the
//! portable seam (`paging::domain` via `crate::iommu`); this module owns the register file, the stream table,
//! the context descriptors, and the command and event queues the SMMU is driven by.
//!
//! Default deny: `initialize` sets `GBPA.ABORT`, installs an all-invalid stream table and only then
//! sets `SMMUEN`, so every stream aborts until `attach` writes its STE. See notes/iommu.md, "The
//! two arch drivers, structural twins", and its honest limits.
//!
//! # BUGS
//!
//! - The stream table holds requester ids 0 to 32767, buses 0 to 127. `attach` refuses a device on
//!   bus 128 or above with [`AttachError::BeyondDeviceTable`]; a third of a 16-bit id space needs a
//!   level-one table of two frames, which the frame allocator cannot align.
//! - A leaf, once allocated, is never freed, so the table grows by two frames per 64 requester ids
//!   ever attached and shrinks never.

use crate::arch::mmu::phys_to_virt;
use crate::iommu::AttachError;
use crate::sync::{InterruptSafeMutex, rank};

// --- Register file (offsets from the SMMU page; Arm IHI 0070, and QEMU's smmuv3-common.h) ---
/// `SMMU_IDR0`, the first identification register (Arm IHI 0070, 6.3.1). `ST_LEVEL`, bits `[28:27]`,
/// says which stream table formats the SMMU walks.
///
/// Name: provisional.
#[doc(alias = "SMMU_IDR0")]
const IDENTIFICATION_REGISTER_0: u64 = 0x00;
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

// The two-level stream table. StreamID on these boards is the PCIe requester id (the device
// tree's iommu-map is the identity). A level-one table of 512 descriptors, eight bytes each, fills
// one frame; each names a leaf of 64 STEs of 64 bytes, which fills one frame too. So a requester
// id splits as `rid >> 6` (which descriptor) and `rid & 63` (which entry in its leaf), and the
// table covers 2^15 ids, buses 0 to 127. The context descriptors keep the same shape: one frame of
// 64 per leaf, at the same index, so a stream's CD sits at its STE's offset in a sibling frame.
const STE_BYTES: u64 = 64;

/// `STRTAB_BASE_CFG.SPLIT`, the bit at which a stream id divides into a level-one index above and
/// a leaf index below: 6, so a leaf holds 64 STEs and fills exactly one frame (Arm IHI 0070,
/// 6.3.24). Linux's `STRTAB_SPLIT` is 8, a leaf of four frames.
///
/// Name: provisional.
#[doc(alias = "SPLIT")]
const STREAM_TABLE_SPLIT: u32 = 6;

/// How many streams one leaf holds: `1 << STREAM_TABLE_SPLIT`.
///
/// Name: provisional.
const STREAMS_PER_LEAF: u32 = 1 << STREAM_TABLE_SPLIT;

/// The widest stream id this table covers, in bits: 15, so the level-one table is 512 descriptors
/// and fits one frame. `initialize` takes the smaller of this and the SMMU's own `SIDSIZE`.
///
/// Name: provisional.
const STREAM_ID_BITS_MAXIMUM: u32 = 15;

/// How many level-one descriptors the table has room for.
///
/// Name: provisional.
const LEVEL_ONE_DESCRIPTORS: usize = 1 << (STREAM_ID_BITS_MAXIMUM - STREAM_TABLE_SPLIT);

/// One level-one stream table descriptor (`L1STD`) is a 64-bit word.
///
/// Name: provisional.
#[doc(alias = "L1STD")]
const LEVEL_ONE_DESCRIPTOR_BYTES: u64 = 8;

/// The bits of a level-one descriptor that hold its leaf's address, `L2Ptr` in `[51:6]`.
///
/// Name: provisional.
const LEVEL_ONE_DESCRIPTOR_ADDRESS: u64 = 0x000f_ffff_ffff_ffc0;

/// `STRTAB_BASE_CFG.FMT` = 0b01, a two-level stream table, in bits `[17:16]`.
///
/// Name: provisional.
#[doc(alias = "STRTAB_BASE_CFG.FMT")]
const STREAM_TABLE_FORMAT_TWO_LEVEL: u32 = 0b01 << 16;

// Each table is one frame: the level-one table, a leaf of STEs and a leaf of CDs. A larger split or
// width that no longer fits stops the build here, not at a write past a frame.
const _: () =
    assert!(LEVEL_ONE_DESCRIPTORS as u64 * LEVEL_ONE_DESCRIPTOR_BYTES <= page_frames::FRAME_SIZE);
const _: () = assert!(STREAMS_PER_LEAF as u64 * STE_BYTES <= page_frames::FRAME_SIZE);

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

/// One leaf of the two-level stream table: a frame of 64 STEs and the frame of their 64 CDs.
#[derive(Clone, Copy)]
struct Leaf {
    stream_table_entries: u64,
    context_descriptors: u64,
}

struct Smmu {
    base: u64,
    /// The level-one stream table: one frame of descriptors, each invalid until its leaf exists.
    strtab: u64,
    /// How many stream id bits the table covers: [`STREAM_ID_BITS_MAXIMUM`], or fewer when the SMMU's
    /// `SIDSIZE` is smaller.
    stream_id_bits: u32,
    /// A kernel-owned frame of [`LEVEL_ONE_DESCRIPTORS`] physical addresses: the CD frame of each
    /// leaf, by level-one index, or zero before that leaf exists. The leaf's STE frame needs no such
    /// record, because its level-one descriptor names it. Both live in frames rather than in this
    /// struct, so building the struct costs no stack (script/stack-frame-check). Re-attaching a
    /// stream rewrites its STE and CD in place and invalidates, so repeated test runs converge
    /// instead of leaking a frame per attach.
    ///
    /// Name: provisional.
    context_descriptor_leaves: u64,
    cmdq: u64,
    cmdq_prod: u32,
    // Read by take_fault (the confinement test); no production fault handler yet.
    #[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
    evtq: u64,
    #[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
    evtq_cons: u32,
}

static SMMU: InterruptSafeMutex<Option<Smmu>> = InterruptSafeMutex::new(rank::IOMMU, None);

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
///
/// Name: provisional. Spelled out under the `init` ruling of 2026-10-26 (UTC), in
/// design/naming/acronyms-and-abbreviations.md.
pub fn initialize(base: u64) {
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

    let idr0 = r32(base, IDENTIFICATION_REGISTER_0);
    assert!(
        (idr0 >> 27) & 0b11 == 0b01,
        "SMMU walks only a linear stream table (IDR0 {idr0:#x}); this driver builds two levels"
    );
    let idr1 = r32(base, IDR1);
    let cmdqs_maximum = (idr1 >> 21) & 0x1f;
    let evtqs_maximum = (idr1 >> 16) & 0x1f;
    assert!(
        cmdqs_maximum >= CMDQ_LOG2 && evtqs_maximum >= EVTQ_LOG2,
        "SMMU queues smaller than expected (IDR1 {idr1:#x})"
    );
    // SIDSIZE, bits [5:0]: the widest StreamID the SMMU itself carries.
    let stream_id_bits = (idr1 & 0x3f).min(STREAM_ID_BITS_MAXIMUM);
    assert!(
        stream_id_bits >= STREAM_TABLE_SPLIT,
        "SMMU StreamIDs narrower than one leaf (IDR1 {idr1:#x})"
    );

    let strtab = zeroed_page_frame("stream table");
    let cmdq = zeroed_page_frame("command queue");
    let evtq = zeroed_page_frame("event queue");
    let context_descriptor_leaves = zeroed_page_frame("context descriptor leaf list");

    // Two-level stream table, every level-one descriptor zero. A zero descriptor has SPAN 0, which
    // names no leaf, so every stream aborts until `attach` installs one: the default-deny posture.
    w64(base, STRTAB_BASE, strtab);
    w32(
        base,
        STRTAB_BASE_CFG,
        stream_table_configuration(stream_id_bits),
    );

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
        stream_id_bits,
        context_descriptor_leaves,
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
            "  iommu            : SMMUv3 at {:#018x}, two-level stream table of {} ids, \
             default-deny, translating",
            s.base,
            1u32 << s.stream_id_bits,
        ),
        None => crate::println!(
            "  iommu            : none (this machine's device tree names no smmuv3 node)",
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

/// Where one stream's entries sit in the two-level table: which leaf, and the byte offset into
/// both of that leaf's frames.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StreamPosition {
    /// The level-one descriptor that names the leaf.
    leaf: usize,
    /// The byte offset of the STE in the leaf's STE frame, and of the CD in its CD frame.
    offset: u64,
}

/// **Where stream `rid`'s two tables sit**, in a table that covers `stream_id_bits` bits of
/// stream id, or `None` past it.
///
/// One function for both tables, which share the index and the 64-byte stride, so the two cannot
/// disagree and land one stream's entry on another's. A refusal is a value and never a panic: a
/// requester id comes from the bus. `mod proofs` holds the bound against the stride.
///
/// Name: provisional.
fn stream_position(rid: u32, stream_id_bits: u32) -> Option<StreamPosition> {
    if rid >= 1 << stream_id_bits {
        return None;
    }
    Some(StreamPosition {
        leaf: (rid >> STREAM_TABLE_SPLIT) as usize,
        offset: (rid % STREAMS_PER_LEAF) as u64 * STE_BYTES,
    })
}

/// **`STRTAB_BASE_CFG` for a two-level table over `stream_id_bits` bits of stream id**: `LOG2SIZE`
/// in `[5:0]`, `SPLIT` in `[10:6]`, `FMT` in `[17:16]`.
///
/// Name: provisional.
const fn stream_table_configuration(stream_id_bits: u32) -> u32 {
    STREAM_TABLE_FORMAT_TWO_LEVEL | (STREAM_TABLE_SPLIT << 6) | stream_id_bits
}

/// **The level-one descriptor that names a leaf at `leaf`**: `L2Ptr` in `[51:6]` and `SPAN` in
/// `[4:0]`. `SPAN` is `SPLIT + 1`, so the leaf holds `2^SPLIT` STEs (Arm IHI 0070, 5.1, "Level 1
/// Stream Table Descriptor"); QEMU's walk reads the same field.
///
/// Name: provisional.
#[doc(alias = "L1STD")]
const fn level_one_descriptor(leaf: u64) -> u64 {
    (leaf & LEVEL_ONE_DESCRIPTOR_ADDRESS) | (STREAM_TABLE_SPLIT as u64 + 1)
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
        // MAIR0/MAIR1: the same palette mmu::initialize programs into MAIR_EL1 (slot 0 device, slot
        // 1 normal write-back). QEMU does not model attributes; real silicon reads these.
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

/// The leaf at level-one index `index`, allocating its two frames and installing its descriptor
/// the first time. Returns the leaf and whether it is new, or `None` with no frame free.
fn leaf_at(s: &mut Smmu, index: usize) -> Option<(Leaf, bool)> {
    let descriptor = phys_to_virt(s.strtab + index as u64 * LEVEL_ONE_DESCRIPTOR_BYTES) as *mut u64;
    let recorded =
        phys_to_virt(s.context_descriptor_leaves + index as u64 * LEVEL_ONE_DESCRIPTOR_BYTES)
            as *mut u64;
    // SAFETY: strtab and context_descriptor_leaves are kernel-owned frames of
    // LEVEL_ONE_DESCRIPTORS eight-byte entries, and `index` is below that (`stream_position`, and
    // the build-time assertion that the table fits a frame). Only this function, under the SMMU
    // lock, writes either.
    let context_descriptors = unsafe { core::ptr::read_volatile(recorded) };
    if context_descriptors != 0 {
        // SAFETY: as above.
        let stream_table_entries =
            unsafe { core::ptr::read_volatile(descriptor) } & LEVEL_ONE_DESCRIPTOR_ADDRESS;
        let leaf = Leaf {
            stream_table_entries,
            context_descriptors,
        };
        return Some((leaf, false));
    }
    let stream_table_entries = crate::memory::alloc_zeroed()?.addr();
    let Some(context_descriptors) = crate::memory::alloc_zeroed() else {
        crate::memory::free(page_frames::PageFrame::from_addr(stream_table_entries));
        return None;
    };
    let leaf = Leaf {
        stream_table_entries,
        context_descriptors: context_descriptors.addr(),
    };
    // The leaf's 64 STEs are zero, so invalid: publish them before the descriptor that makes the
    // SMMU walk them.
    crate::arch::direct_memory_access_write_barrier();
    // SAFETY: as above. One aligned 64-bit store, so the SMMU never reads a pointer with a stale
    // half.
    unsafe {
        core::ptr::write_volatile(descriptor, level_one_descriptor(stream_table_entries));
        core::ptr::write_volatile(recorded, leaf.context_descriptors);
    }
    Some((leaf, true))
}

/// Point stream `rid` at the domain rooted at `ttb` (a VMSAv8-64 table the portable seam built),
/// tagged `asid`, then invalidate so the SMMU drops any cached STE/CD/TLB entry for the previous
/// domain. After this returns `Ok`, the device can reach exactly what the domain maps.
///
/// Refuses a stream id past the table with [`AttachError::BeyondDeviceTable`], and a new leaf with
/// no frame free with [`AttachError::NoFrameForTable`]. A refused stream keeps no STE, so it
/// aborts every transaction.
///
/// Context: takes the `SMMU` lock, and allocates a leaf's frames under it.
pub fn attach(rid: u32, ttb: u64, asid: u16) -> Result<(), AttachError> {
    let mut g = SMMU.lock();
    let s = g.as_mut().expect("SMMU attach before initialize");
    let position =
        stream_position(rid, s.stream_id_bits).ok_or(AttachError::BeyondDeviceTable {
            requester_id: rid,
            limit: 1 << s.stream_id_bits,
        })?;
    let (leaf, is_new_leaf) = leaf_at(s, position.leaf).ok_or(AttachError::NoFrameForTable)?;

    let cd = phys_to_virt(leaf.context_descriptors + position.offset) as *mut u32;
    // SAFETY: the leaf's CD frame is kernel-owned; `stream_position` bounds the offset to it.
    unsafe {
        for (i, w) in context_descriptor(ttb, asid).iter().enumerate() {
            core::ptr::write_volatile(cd.add(i), *w);
        }
    }

    let ctxptr = leaf.context_descriptors + position.offset;
    let ste = phys_to_virt(leaf.stream_table_entries + position.offset) as *mut u32;
    // SAFETY: the leaf's STE frame is kernel-owned; `stream_position` bounds the offset to it.
    unsafe {
        for (i, w) in stream_table_entry(ctxptr).iter().enumerate() {
            core::ptr::write_volatile(ste.add(i), *w);
        }
    }

    // The tables are read by the SMMU, a separate observer: publish before telling it to look.
    crate::arch::direct_memory_access_write_barrier();

    // Drop whatever the SMMU cached for this stream: its STE, its CD, and every TLB entry (a
    // re-attach reuses the ASID with a brand-new table, so the broad TLBI is the correct one).
    // Leaf 0 on a new leaf drops a cached level-one descriptor as well, so the SMMU walks the one
    // just written.
    cmd_push(s, [CMD_CFGI_STE, rid, u32::from(!is_new_leaf), 0]);
    cmd_push(s, [CMD_CFGI_CD, rid, 1, 0]);
    cmd_push(s, [CMD_TLBI_NSNH_ALL, 0, 0, 0]);
    cmd_push(s, [CMD_SYNC, 0, 0, 0]);
    Ok(())
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
/// The register accessors are unreachable to a harness, so `initialize`, `leaf_at`, `cmd_push`,
/// `attach` and `take_fault` are unproved; so are the register offsets and bit constants, which
/// only Arm IHI 0070 states. Nothing here covers the RISC-V IOMMU or VT-d. See notes/iommu.md, "What is proved,
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
        // `ctxptr` is a leaf's CD frame plus `stream_position`'s offset: a page frame plus a 64-byte
        // stride.
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

    /// **No stream can reach another stream's tables, and a stream past the table is refused.**
    ///
    /// `stream_position` is all that stands between a requester id from the bus and a raw write.
    /// Over every `u32` and every table width `initialize` can choose, it admits exactly the ids
    /// below the width, places each inside one level-one descriptor and inside its leaf's frame,
    /// and no two ids share a byte of either leaf.
    ///
    /// Falsification: replayable `kernel/falsifications/arch.aarch64.iommu.proofs.no_stream_can_reach_another_streams_tables.patch`
    #[kani::proof]
    fn no_stream_can_reach_another_streams_tables() {
        let a: u32 = kani::any();
        let b: u32 = kani::any();
        let stream_id_bits: u32 = kani::any();
        // What `initialize` can choose: at least one leaf, at most the level-one table's frame.
        kani::assume((STREAM_TABLE_SPLIT..=STREAM_ID_BITS_MAXIMUM).contains(&stream_id_bits));

        let (pa, pb) = (
            stream_position(a, stream_id_bits),
            stream_position(b, stream_id_bits),
        );
        assert!(
            pa.is_some() == (a < 1 << stream_id_bits),
            "exactly the ids below the table's width are placed, and every other is refused"
        );
        let (Some(pa), Some(pb)) = (pa, pb) else {
            return;
        };

        assert!(
            pa.leaf < LEVEL_ONE_DESCRIPTORS,
            "every admitted stream's leaf has a descriptor in the level-one frame"
        );
        assert!(
            pa.offset + STE_BYTES <= page_frames::FRAME_SIZE,
            "every entry the bound admits lies inside its leaf's single frame"
        );
        assert!(
            a == b
                || pa.leaf != pb.leaf
                || pa.offset + STE_BYTES <= pb.offset
                || pb.offset + STE_BYTES <= pa.offset,
            "two different streams never share a byte of either table"
        );
    }

    /// **The level-one table names exactly the leaf the kernel allocated, at the span it holds.**
    ///
    /// The descriptor shares its low word with `SPAN`, so an address bit there would make the SMMU
    /// read a leaf larger than the frame; a lost high half walks another table. The configuration
    /// word must tell the SMMU the same width and split the kernel indexes by.
    ///
    /// Falsification: replayable `kernel/falsifications/arch.aarch64.iommu.proofs.the_level_one_table_names_exactly_the_leaf.patch`
    #[kani::proof]
    fn the_level_one_table_names_exactly_the_leaf() {
        let leaf: u64 = kani::any();
        let stream_id_bits: u32 = kani::any();
        // A leaf is a page frame in RAM.
        kani::assume(leaf % page_frames::FRAME_SIZE == 0);
        kani::assume(leaf < 1 << PA_BITS);
        kani::assume((STREAM_TABLE_SPLIT..=STREAM_ID_BITS_MAXIMUM).contains(&stream_id_bits));

        let descriptor = level_one_descriptor(leaf);
        assert!(
            descriptor & !0x1f == leaf,
            "L2Ptr is the leaf entire, high half included, and nothing else"
        );
        assert!(
            descriptor & LEVEL_ONE_DESCRIPTOR_ADDRESS == leaf,
            "reading the descriptor back gives the leaf it was written with"
        );
        assert!(
            1u64 << ((descriptor & 0x1f) - 1) == STREAMS_PER_LEAF as u64,
            "SPAN says the leaf holds exactly the streams one frame of STEs holds"
        );

        let configuration = stream_table_configuration(stream_id_bits);
        assert!(
            configuration & 0x3f == stream_id_bits,
            "LOG2SIZE is the width the kernel bounds requester ids by"
        );
        assert!(
            (configuration >> 6) & 0x1f == STREAM_TABLE_SPLIT,
            "SPLIT is the bit the kernel divides a requester id at"
        );
        assert!(
            (configuration >> 16) & 0b11 == 0b01 && configuration & !0x3_07ff == 0,
            "FMT is two-level, and no other bit is set"
        );
    }
}
