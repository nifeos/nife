//! RISC-V virtual memory: the Sv39 half of the `arch` page-table contract.
//!
//! The descriptor format is `paging::Sv39` (DECISIONS §17 (the second architecture: RISC-V)). This
//! module composes `satp`, builds the kernel's W^X tables, maps user pages through `Mapper<_, _,
//! Sv39>`, shares the kernel half into every process root, and does TLB maintenance with
//! `sfence.vma`.
//!
//! See notes/riscv-port/memory.md.

use core::ffi::c_void;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use paging::{Flags, Half, MapError, Mapper, PAGE_SIZE, PageSize, PageTable};

use super::instructions;
use crate::memory;

/// This architecture's page-table format, which portable code names `arch::mmu::Format`.
///
/// Per machine: the TH1520 build uses `Sv39Mae`, whose leaves carry T-Head memory types because
/// firmware turns `XTheadMae` on. `arch::isa::init` refuses a hart that disagrees.
#[cfg(not(feature = "board_th1520"))]
pub type Format = paging::Sv39;
#[cfg(feature = "board_th1520")]
pub(crate) type Format = paging::Sv39Mae;

/// The console UART, mapped as device memory so printing survives the switch off the boot table.
const UART_BASE: u64 = super::machine::CONSOLE_UART_PHYS;
const UART_SIZE: u64 = 0x1000;

/// The `satp` MODE field value for Sv39 (bits 63:60).
const SATP_MODE_SV39: u64 = 8 << 60;
/// `satp.ASID` sits at bits 59:44 on rv64 Sv39.
const SATP_ASID_SHIFT: u64 = 44;
/// `satp.PPN` is the low 44 bits.
const SATP_PPN_MASK: u64 = (1 << 44) - 1;

/// The kernel's fine-map root, saved by [`init`] so a secondary hart can adopt it.
static KERNEL_ROOT: AtomicU64 = AtomicU64::new(0);

/// Read `satp`. One instruction, in [`instructions`], so a proof can stub it (see `mod proofs`).
fn read_satp() -> u64 {
    instructions::read_satp()
}

/// Install a whole address space (kernel high half and user low half) by writing `satp`.
///
/// Flushes the whole TLB only when [`asid_tagging_is_trusted`] is false. See
/// notes/riscv-tlb-shootdown.md.
///
/// # Safety
/// `satp` must name a well-formed, live Sv39 root that contains the kernel's high half. It is the
/// whole address space on this ISA, so a root missing the kernel half faults on the very next
/// instruction fetch, and a root whose frames have been freed hands this hart somebody else's
/// memory. The table must stay live until some later write to `satp` replaces it.
#[inline(always)] // on the context switch's hot path; see `switch_user_root`'s note
unsafe fn write_satp(satp: u64) {
    // SAFETY: this function's own `# Safety` contract is exactly the one this write needs; it
    // forwards, it does not weaken.
    unsafe { instructions::write_satp(satp) };

    if !asid_tagging_is_trusted() {
        // The tag cannot keep two spaces apart here, so only a full sweep makes the switch safe.
        instructions::sfence_vma_all();
    }
}

/// The physical address of the currently-installed root page table (`satp.PPN << 12`).
fn current_root_pa() -> u64 {
    (read_satp() & SATP_PPN_MASK) << 12
}

/// The base of the kernel's virtual address space, the Sv39 high half (bits 63:38 all one). Must
/// match `KERNEL_VA_BASE` in link-riscv64.ld.
pub const KERNEL_VA_BASE: u64 = 0xffff_ffc0_0000_0000;

/// Where kernel thread stacks live, virtually (`thread.rs`'s `STACK_AREA`): 64 GiB up, far above
/// the direct map, so a stack address never collides with a physical address's virtual name.
///
/// Name: provisional (milestone 161 (the `x86_64` kernel port)): this was a portable expression in
/// `thread.rs` (`KERNEL_VA_BASE | 0x10_0000_0000`) until `x86_64` arrived, where the expression is
/// not merely wrong but a no-op -- `KERNEL_VA_BASE` there already has every bit of `0x10_0000_0000`
/// set, so the OR yielded the kernel image's own base and every kernel thread stack would have been
/// mapped over `.text`. Rule 1 says an architecture's addresses live under `arch/`; this is that.
pub const THREAD_STACK_AREA: u64 = KERNEL_VA_BASE | 0x0000_0010_0000_0000;

/// The device window's physical base: the top gigabyte of a 40-bit physical space, named at the
/// top gigabyte of the Sv39 high half ([`DEVICE_WINDOW_VA`], root index 511).
///
/// It names devices above the direct map's 255 GiB, such as the TH1520's at `0xff_d800_0000`. See
/// notes/riscv-port/memory.md.
pub(crate) const DEVICE_WINDOW_PA: u64 = 0xff_c000_0000;
/// One gigabyte: one root entry.
pub(crate) const DEVICE_WINDOW_SIZE: u64 = 1 << 30;
/// Root index 511, the last gigabyte of the high half.
pub(crate) const DEVICE_WINDOW_VA: u64 = 0xffff_ffff_c000_0000;
/// The first physical address the direct map does not reach: 255 GiB, so that `pa +
/// KERNEL_VA_BASE` stops one gigabyte short of [`DEVICE_WINDOW_VA`].
pub(crate) const DIRECT_MAP_LIMIT: u64 = DEVICE_WINDOW_VA - KERNEL_VA_BASE;

// The arithmetic the two translations rest on, checked by the compiler rather than by a reader.
const _: () = {
    assert!(DEVICE_WINDOW_VA == KERNEL_VA_BASE + 255 * (1 << 30));
    assert!(DIRECT_MAP_LIMIT == 255 << 30);
    assert!(DEVICE_WINDOW_PA + DEVICE_WINDOW_SIZE == 1 << 40);
    assert!(DEVICE_WINDOW_PA >= DIRECT_MAP_LIMIT);
    // The window round-trips at both edges, and the direct map at its last page.
    assert!(virt_to_phys(phys_to_virt(DEVICE_WINDOW_PA)) == DEVICE_WINDOW_PA);
    assert!(phys_to_virt(DEVICE_WINDOW_PA) == DEVICE_WINDOW_VA);
    assert!(
        virt_to_phys(phys_to_virt(DEVICE_WINDOW_PA + DEVICE_WINDOW_SIZE - 1))
            == DEVICE_WINDOW_PA + DEVICE_WINDOW_SIZE - 1
    );
    assert!(virt_to_phys(phys_to_virt(DIRECT_MAP_LIMIT - 1)) == DIRECT_MAP_LIMIT - 1);
    assert!(phys_to_virt(DIRECT_MAP_LIMIT - 1) < DEVICE_WINDOW_VA);
    // Every console address a machine can choose is nameable.
    assert!(
        UART_BASE < DIRECT_MAP_LIMIT
            || (UART_BASE >= DEVICE_WINDOW_PA && UART_BASE < DEVICE_WINDOW_PA + DEVICE_WINDOW_SIZE)
    );
};

/// The boot page table: one Sv39 root of RWX gigapage leaves, identity-mapped and aliased at
/// `KERNEL_VA_BASE`, that lasts until `mmu::init` installs the W^X tables.
///
/// `boot.s` loads its physical address into `satp`, so it is a real static in `.data`. See
/// notes/riscv-port/memory.md for which gigapages it maps and why.
#[repr(C, align(4096))]
struct BootTable([u64; paging::ENTRIES]);

const fn boot_table() -> BootTable {
    // Sv39 gigapage leaf: V R W X A D set.
    const LEAF: u64 = (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 6) | (1 << 7);
    // With `XTheadMae` on every leaf names a T-Head memory type (Linux's `_PAGE_PMA_THEAD` and
    // `_PAGE_IO_THEAD`). Elsewhere bits 63:59 are reserved, and a set one is a page fault.
    #[cfg(feature = "board_th1520")]
    const MEMORY: u64 = 0b0111 << 60;
    #[cfg(feature = "board_th1520")]
    const DEVICE: u64 = 0b1001 << 60;
    #[cfg(not(feature = "board_th1520"))]
    const MEMORY: u64 = 0;
    #[cfg(not(feature = "board_th1520"))]
    const DEVICE: u64 = 0;
    // A 1 GiB-aligned physical base, as a gigapage PTE (PPN at bits 53:10).
    const fn giga(pa: u64) -> u64 {
        let memory_type = if pa >= DEVICE_WINDOW_PA {
            DEVICE
        } else {
            MEMORY
        };
        ((pa >> 12) << 10) | LEAF | memory_type
    }
    let mut t = [0u64; paging::ENTRIES];
    t[0] = giga(0x0000_0000); // identity: 0..1 GiB, covers the UART at 0x1000_0000
    t[1] = giga(0x4000_0000); // identity: 1..2 GiB, the VF2's low DRAM and U-Boot's default DTB
    t[2] = giga(0x8000_0000); // identity: 2..3 GiB, covers the kernel/RAM at 0x8020_0000
    t[256] = t[0]; // high alias of index 0 (KERNEL_VA_BASE adds 256 to the top-level index)
    t[257] = t[1]; // high alias of index 1
    t[258] = t[2]; // high alias of index 2
    // The TH1520 maps all 16 GiB of its RAM: its U-Boot may place the device tree anywhere.
    #[cfg(feature = "board_th1520")]
    {
        let mut g = 3;
        while g < 16 {
            t[g] = giga(g as u64 * (1 << 30));
            t[256 + g] = t[g];
            g += 1;
        }
    }
    // The device window, high alias only, so a console there can print before `mmu::init`.
    t[511] = giga(DEVICE_WINDOW_PA);
    BootTable(t)
}

/// The boot table instance `boot.s` points `satp` at, unmangled so the assembly can name it.
#[unsafe(no_mangle)]
static BOOT_PAGE_TABLE: BootTable = boot_table();

/// The physical base of the virtio-mmio transport window on QEMU's `virt` machine.
pub const VIRTIO_MMIO_BASE: u64 = 0x1000_1000;
/// The size of the virtio-mmio window (8 slots of 0x1000).
pub(crate) const VIRTIO_MMIO_SIZE: u64 = 8 * 0x1000;
/// The PLIC interrupt id of the first virtio-mmio slot on `virt`; the driver adds the slot index.
pub(crate) const VIRTIO_IRQ_BASE: u32 = 1;
/// The distance between virtio-mmio slots on RISC-V's `virt`, which `virtio::find_block_device`
/// walks.
pub(crate) const VIRTIO_SLOT_STRIDE: u64 = 0x1000;
pub(crate) const VIRTIO_SLOTS: u64 = 8;

/// Whether `map_everything` mapped the virtio-mmio window, which it does only when the device tree
/// names a `virtio,mmio` node (milestone 89). Written once on the primary hart before any probe.
static VIRTIO_MMIO_MAPPED: AtomicBool = AtomicBool::new(false);

/// How many virtio-mmio slots the probe may read: [`VIRTIO_SLOTS`] when the device tree names the
/// bus, else zero. On the JH7110 and the TH1520 that address is a UART and DRAM.
///
/// Name: provisional, milestone 89 (Scaleway EM-RV1)'s lane, 2026-10-06 (UTC). The same name on all three architectures.
pub fn virtio_slots() -> u64 {
    if VIRTIO_MMIO_MAPPED.load(Ordering::Relaxed) {
        VIRTIO_SLOTS
    } else {
        0
    }
}

/// How many PCIe ECAM buses the kernel maps before a survey: bus 0 only. The window itself comes
/// from the device tree (`memory::pci_regions`).
///
/// `pci::ecam_buses()` is what is mapped; no survey runs on this architecture, so it stays 1. See
/// notes/riscv-port/memory.md.
pub(crate) const PCI_ECAM_BUSES: u16 = 1;

/// How much of the 32-bit PCI memory window the kernel maps and assigns BARs from: 2 MB, room for
/// dozens of 16 KB virtio BARs. OpenSBI programs no BAR, so kernel/src/pci.rs places them.
pub(crate) const PCI_BAR_MAPPED: u64 = 0x20_0000;

/// The PLIC input for INTx line A on `virt`'s root complex; B, C and D follow, by the standard
/// swizzle in `pci::intx_irq`.
pub(crate) const PCI_IRQ_BASE: u32 = 32;

/// Physical to kernel-virtual: `pa + KERNEL_VA_BASE`, or [`DEVICE_WINDOW_VA`] onward inside the
/// device window, by one branchless mask.
///
/// `pa` must be below [`DIRECT_MAP_LIMIT`] or inside the window; any other address aliases another
/// frame. [`refuse_unnameable_ram`] and `direct_map` check that at boot. See
/// notes/riscv-port/memory.md.
pub const fn phys_to_virt(pa: u64) -> u64 {
    (pa & ((1 << 38) - 1)) | KERNEL_VA_BASE
}

/// Whether [`phys_to_virt`] names `[pa_start, pa_end)` truthfully: the range lies wholly in the
/// direct map (below [`DIRECT_MAP_LIMIT`]) or wholly in the device window. Anywhere else the mask
/// aliases it onto some other physical address. Checked when mappings are built, never per call.
const fn is_nameable(pa_start: u64, pa_end: u64) -> bool {
    pa_end <= DIRECT_MAP_LIMIT
        || (pa_start >= DEVICE_WINDOW_PA && pa_end <= DEVICE_WINDOW_PA + DEVICE_WINDOW_SIZE)
}

/// Panic if any RAM region reaches [`DIRECT_MAP_LIMIT`], where [`phys_to_virt`] would alias it.
///
/// Called first in [`init`], before any frame is named through the direct map. A machine with that
/// much RAM needs a second window or Sv48.
fn refuse_unnameable_ram() {
    for (start, size) in memory::ram_regions() {
        let end = start.saturating_add(size);
        assert!(
            end <= DIRECT_MAP_LIMIT,
            "RAM {start:#x}..{end:#x} reaches past the 255 GiB the Sv39 direct map can name; \
             phys_to_virt would alias it (arch/riscv64/mmu.rs, DIRECT_MAP_LIMIT)"
        );
    }
}

/// Kernel-virtual to physical. The inverse of [`phys_to_virt`].
pub const fn virt_to_phys(va: u64) -> u64 {
    if va >= DEVICE_WINDOW_VA {
        va - DEVICE_WINDOW_VA + DEVICE_WINDOW_PA
    } else {
        va - KERNEL_VA_BASE
    }
}

/// A physical page-table address as a kernel pointer through the direct map.
pub fn phys_to_ptr(pa: u64) -> *mut PageTable {
    phys_to_virt(pa) as *mut PageTable
}

/// Build the kernel's W^X Sv39 tables, switch `satp` to them, and probe the ASID width.
///
/// The fine tables map the same kernel addresses to the same frames as the boot table, so the
/// switch is seamless. Runs once, on the primary hart, before any secondary or user space exists.
pub fn init() {
    refuse_unnameable_ram();
    let root = memory::alloc()
        .expect("no frame for the root page table")
        .addr();
    // SAFETY: a fresh frame; zero it before the hardware can ever walk it.
    unsafe {
        (*phys_to_ptr(root)).entries = [0; paging::ENTRIES];
    }

    // SAFETY: `root` is zeroed and page-aligned; `phys_to_ptr` is valid because the boot table's
    // direct-map gigapages cover all of RAM (so every frame the mapper allocates is addressable).
    let mut mapper = unsafe {
        Mapper::<_, _, Format>::new(
            root,
            Half::High,
            || memory::alloc().map(|f| f.addr()),
            phys_to_ptr,
        )
    };

    map_everything(&mut mapper).expect("failed to build the kernel page tables");
    verify(&mapper);

    // SAFETY: the fine map covers this function's code, its stack, and the UART; we checked.
    unsafe { install(root) };

    KERNEL_ROOT.store(root, Ordering::Relaxed);

    // Probe after `install`, against the real kernel root.
    ASID_BITS.store(probe_asid_bits(), Ordering::Relaxed);

    // Settled before any secondary or user space exists, so every `write_satp` reads one value.
    ASID_TAGGING_TRUSTED.store(
        ASID_BITS.load(Ordering::Relaxed) >= ASID_BITS_NEEDED as usize,
        Ordering::Relaxed,
    );
    // Through the accessor, so its "was it probed" assertion fires here if the order moves. The
    // boot's `isa` line reports it.
    super::isa::record_asid_bits(asid_bits());
}

/// Switch `satp` to the Sv39 tables rooted at physical `root`, and flush the TLB.
///
/// # Safety
/// `root` must be a complete Sv39 kernel map covering the currently-executing code, stack, and any
/// memory touched before the next `sfence`; otherwise the instruction after the `csrw` faults.
unsafe fn install(root: u64) {
    let satp = SATP_MODE_SV39 | (root >> 12);
    // Bracketed by `sfence.vma` so no stale boot-table entry survives. None of the three is
    // `nomem`, so no memory access moves into the gaps.
    instructions::sfence_vma_all();
    // SAFETY: caller's contract.
    unsafe { instructions::write_satp(satp) };
    instructions::sfence_vma_all();
}

/// How many `satp.ASID` bits this hardware actually implements, discovered at boot.
///
/// `usize::MAX` until [`probe_asid_bits`] runs, so a read before the probe is loud rather than a
/// plausible zero.
static ASID_BITS: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(usize::MAX);

/// The widest ASID the architecture allows in Sv39: `satp` bits 59:44.
const SATP_ASID_WIDTH: u32 = 16;

/// Return how many `satp.ASID` bits this hart implements, which `crates/address_space_identifier`
/// needs to be at least [`ASID_BITS_NEEDED`].
///
/// `satp.ASID` is WARL and may be zero bits wide. The probe writes ones into the live `satp`'s ASID
/// field, reads back what stuck, and restores `satp` exactly; only the tag moves. Code the compiler
/// places between the writes runs on the same root under another tag, and none of the five
/// wrappers is `nomem`, so no memory access moves past the closing `sfence.vma`. `mod proofs`
/// proves both promises. See notes/riscv-tlb-shootdown.md and notes/riscv-port/memory.md.
fn probe_asid_bits() -> usize {
    let original = read_satp();
    let all_ones = original | (((1u64 << SATP_ASID_WIDTH) - 1) << SATP_ASID_SHIFT);
    // SAFETY: MODE and PPN are carried over from the live `satp`, so this installs the same root
    // page table under a different ASID tag. Bracketed by `sfence.vma` so no entry tagged with the
    // probe value outlives the probe.
    unsafe { instructions::write_satp(all_ones) };
    instructions::sfence_vma_all();
    let readback = read_satp();
    // SAFETY: `original` is the value the hardware was walking a moment ago.
    unsafe { instructions::write_satp(original) };
    instructions::sfence_vma_all();
    let implemented = (readback >> SATP_ASID_SHIFT) & ((1 << SATP_ASID_WIDTH) - 1);
    // WARL bits need not be contiguous, so count them rather than assume a low-bit mask.
    implemented.count_ones() as usize
}

/// How many `satp.ASID` bits this hardware implements. Panics if read before [`init`] probed.
pub(crate) fn asid_bits() -> usize {
    let n = ASID_BITS.load(core::sync::atomic::Ordering::Relaxed);
    assert_ne!(n, usize::MAX, "asid_bits() read before mmu::init probed it");
    n
}

/// How many `satp.ASID` bits the allocator needs to hold its largest tag,
/// `address_space_identifier::ASIDS - 1`. Derived, so raising `ASIDS` moves the gate.
///
/// `arch::riscv64::isa`'s record test checks [`asid_tagging_is_trusted`] against it.
pub(crate) const ASID_BITS_NEEDED: u32 = (address_space_identifier::ASIDS as u64 - 1).ilog2() + 1;

/// Whether two live address spaces are guaranteed distinct in this hart's TLB. False until [`init`]
/// probes, so every earlier `satp` write flushes.
static ASID_TAGGING_TRUSTED: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Whether the context switch may skip its TLB flush: `satp.ASID` is wide enough to hold every
/// number the allocator hands out, so two spaces can never share a hardware tag.
///
/// Relaxed: it is written once at boot, before any secondary hart or user space exists.
pub(crate) fn asid_tagging_is_trusted() -> bool {
    ASID_TAGGING_TRUSTED.load(core::sync::atomic::Ordering::Relaxed)
}

/// Adopt the kernel's W^X map on a secondary hart, switching from the boot table to the shared
/// `KERNEL_ROOT` the primary built. Nothing per hart is built.
pub(crate) fn init_secondary() {
    let root = KERNEL_ROOT.load(Ordering::Relaxed);
    // SAFETY: the primary built this root and is running on it; it covers this code, this hart's
    // stack (mapped in the kernel image), and the UART, so the switch is seamless.
    unsafe { install(root) };
}

/// Build every mapping the kernel needs in `m`: the direct map of RAM, the W^X kernel sections, the
/// stacks, and the devices the device tree names. See notes/riscv-port/memory.md.
fn map_everything<A, P>(m: &mut Mapper<A, P, Format>) -> Result<(), MapError>
where
    A: FnMut() -> Option<u64>,
    P: Fn(u64) -> *mut PageTable,
{
    // 1. The direct map of RAM, less the kernel image: the mapper refuses to overwrite, and the
    //    image's sections get tighter permissions below.
    let image_lo = virt_to_phys(image_start());
    let image_hi = virt_to_phys(image_end());
    for (start, size) in memory::ram_regions() {
        let end = start + size;
        direct_map(m, start, image_lo.min(end), Flags::kernel_data())?;
        direct_map(m, image_hi.max(start), end, Flags::kernel_data())?;
    }

    // 2. The kernel image, section by section, at its linked VAs. W^X.
    map_range(m, text_start(), text_end(), Flags::kernel_code())?;
    map_range(m, rodata_start(), rodata_end(), Flags::kernel_rodata())?;
    map_range(m, data_start(), bss_end(), Flags::kernel_data())?;

    // 3. The guard page stays unmapped. 4. The stack.
    map_range(m, stack_bottom(), stack_top(), Flags::kernel_data())?;

    // 4b. Secondary stacks one slot at a time, so each slot's bottom page stays a guard hole.
    for id in 0..crate::cpu::MAX_CPUS {
        let (bottom, top) = crate::smp::secondary_stack_span(id);
        map_range(m, bottom, top, Flags::kernel_data())?;
    }

    // 4c. Interrupt stacks, slot by slot for 4b's reason.
    for id in 0..crate::cpu::MAX_CPUS {
        let (bottom, top) = crate::interrupt_stack::span(id);
        map_range(m, bottom, top, Flags::kernel_data())?;
    }

    // 5. The UART.
    direct_map(m, UART_BASE, UART_BASE + UART_SIZE, Flags::device())?;

    // 6. The PLIC, where the device tree names one.
    if let Some((start, size)) = memory::plic_region() {
        direct_map(m, start, start + size, Flags::device())?;
    }

    // 6b. The JH7110's STG clock and reset generator, which ungates the TRNG.
    if let Some(crg) = memory::jh7110_clock_and_reset() {
        direct_map(m, crg.base, crg.base + crg.size, Flags::device())?;
    }

    // 6c. The JH7110's SYS clock and reset generator: I2C5 for the PMIC reset, and gmac0's clocks.
    if let Some(sys) = memory::jh7110_sys_window() {
        direct_map(m, sys.base, sys.base + sys.size, Flags::device())?;
    }

    // 6c'. The SYS syscon (the PLL words the I2C clock reads) and the PMIC bus's I2C controller.
    if let Some((_sys, bus)) = memory::jh7110_pmic_bus() {
        direct_map(
            m,
            jh7110_clock_and_reset::SYS_SYSCON_BASE,
            jh7110_clock_and_reset::SYS_SYSCON_BASE + jh7110_clock_and_reset::SYS_SYSCON_SIZE,
            Flags::device(),
        )?;
        if let Some((base, size)) = bus.controller {
            direct_map(m, base, base + size, Flags::device())?;
        }
    }

    // 6d. The JH7110's first Ethernet port, its AON clock-and-reset window and AON syscon page.
    if let Some(e) = memory::jh7110_ethernet() {
        direct_map(m, e.port.base, e.port.base + e.port.size, Flags::device())?;
        direct_map(m, e.aon.base, e.aon.base + e.aon.size, Flags::device())?;
        direct_map(
            m,
            e.syscon.base,
            e.syscon.base + e.syscon.size,
            Flags::device(),
        )?;
    }

    // 6e. The JH7110's watchdog, the registers the driver touches, in a soak that arms it.
    #[cfg(soak_watchdog)]
    if let Some((w, _sys)) = memory::jh7110_watchdog() {
        let used = jh7110_watchdog::WINDOW_USED.min(w.size);
        direct_map(m, w.base, w.base + used, Flags::device())?;
    }

    // 7. QEMU's `sifive_test` finisher, which `arch::semihosting::exit` writes. Boards have none.
    #[cfg(not(feature = "board"))]
    direct_map(m, 0x10_0000, 0x10_1000, Flags::device())?;

    // 8. The virtio-mmio window, only where the device tree names the bus: elsewhere that address
    //    may be RAM already mapped in step 1 (notes/visionfive2.md).
    let named = crate::device_tree().is_ok_and(|dt| {
        let mut slot = [device_tree_blob::Region { start: 0, size: 0 }; 1];
        matches!(dt.node_reg_compatible(b"virtio,mmio", &mut slot), Ok(n) if n >= 1)
    });
    if named {
        direct_map(
            m,
            VIRTIO_MMIO_BASE,
            VIRTIO_MMIO_BASE + VIRTIO_MMIO_SIZE,
            Flags::device(),
        )?;
        VIRTIO_MMIO_MAPPED.store(true, Ordering::Relaxed);
    }

    // 9. The PCIe ECAM and BAR windows, only where the device tree names a host, for 8's reason.
    if let Some(((ecam, ecam_size), (bar, bar_size))) = memory::pci_regions() {
        direct_map(
            m,
            ecam,
            ecam + crate::pci::ecam_bytes().min(ecam_size),
            Flags::device(),
        )?;
        direct_map(m, bar, bar + PCI_BAR_MAPPED.min(bar_size), Flags::device())?;
    }

    // 10. The JH7110's SD/MMC controllers, the registers and data FIFO the driver touches.
    if let Some(slots) = memory::jh7110_storage() {
        for s in slots.iter().flatten() {
            let used = u64::from(designware_mobile_storage::regs::WINDOW_USED).min(s.size);
            direct_map(m, s.base, s.base + used, Flags::device())?;
        }
    }

    Ok(())
}

/// Map a range of *virtual* addresses to the physical ones they were linked against.
fn map_range<A, P>(
    m: &mut Mapper<A, P, Format>,
    va_start: u64,
    va_end: u64,
    flags: Flags,
) -> Result<(), MapError>
where
    A: FnMut() -> Option<u64>,
    P: Fn(u64) -> *mut PageTable,
{
    if va_end <= va_start {
        return Ok(());
    }
    let pages = (va_end - va_start).div_ceil(PAGE_SIZE);
    m.map_range(va_start, virt_to_phys(va_start), pages, flags)
}

/// Map physical `[pa_start, pa_end)` into the direct map with `flags`. Panics on a range
/// [`phys_to_virt`] cannot name.
///
/// RAM goes in the largest leaf that fits; devices stay in 4 KiB pages.
fn direct_map<A, P>(
    m: &mut Mapper<A, P, Format>,
    pa_start: u64,
    pa_end: u64,
    flags: Flags,
) -> Result<(), MapError>
where
    A: FnMut() -> Option<u64>,
    P: Fn(u64) -> *mut PageTable,
{
    if pa_end <= pa_start {
        return Ok(());
    }
    // Boot time only, never on the IPC path.
    assert!(
        is_nameable(pa_start, pa_end),
        "{pa_start:#x}..{pa_end:#x} is outside the direct map and the device window; phys_to_virt would alias it"
    );
    let len = (pa_end - pa_start).next_multiple_of(PAGE_SIZE);
    let largest = if flags.is_device() {
        PageSize::Size4KiB
    } else {
        PageSize::Size1GiB
    };
    m.map_span(phys_to_virt(pa_start), pa_start, len, flags, largest)
}

/// Walk `m` in software and panic on anything that would kill the machine at the `satp` switch:
/// this code not executable, the UART unmapped, or a guard page mapped.
fn verify<A, P>(m: &Mapper<A, P, Format>)
where
    A: FnMut() -> Option<u64>,
    P: Fn(u64) -> *mut PageTable,
{
    let here = init as *const () as u64;
    let (pa, flags) = m
        .translate(here)
        .expect("the code switching tables is not mapped: we would die on the next fetch");
    assert_eq!(pa, virt_to_phys(here), "our .text maps to the wrong frame");
    assert!(
        flags.is_kernel_executable(),
        "our own .text is not executable"
    );
    assert!(
        !flags.is_writable(),
        "our own .text is writable (W^X violated)"
    );

    assert!(
        m.translate(phys_to_virt(UART_BASE)).is_some(),
        "the UART is not mapped: the machine would go silent"
    );

    assert!(
        m.translate(stack_guard()).is_none(),
        "the guard page IS mapped: stack overflow protection is off"
    );

    for id in 0..crate::cpu::MAX_CPUS {
        assert!(
            m.translate(crate::smp::secondary_stack_guard(id)).is_none(),
            "a secondary's guard page IS mapped: its stack overflow protection is off"
        );
    }

    for id in 0..crate::cpu::MAX_CPUS {
        assert!(
            m.translate(crate::interrupt_stack::guard(id)).is_none(),
            "an interrupt stack's guard page IS mapped: its overflow protection is off"
        );
    }
}

macro_rules! linker_symbol {
    ($name:ident, $sym:ident) => {
        // `pub`: `stack::guard_page_at` asks for the boot stack's guard page on every ISA.
        pub fn $name() -> u64 {
            unsafe extern "C" {
                static $sym: c_void;
            }
            (&raw const $sym) as u64
        }
    };
}

linker_symbol!(image_start, __image_start);
linker_symbol!(image_end, __image_end);
linker_symbol!(text_start, __text_start);
linker_symbol!(text_end, __text_end);
linker_symbol!(rodata_start, __rodata_start);
linker_symbol!(rodata_end, __rodata_end);
linker_symbol!(data_start, __data_start);
linker_symbol!(bss_end, __bss_end);
linker_symbol!(stack_guard, __stack_guard);
linker_symbol!(stack_bottom, __stack_bottom);
linker_symbol!(stack_top, __stack_top);

/// Compose the full `satp` value naming the space at `root` with tag `asid`: Sv39 mode, ASID, PPN.
///
/// Named like aarch64's so portable `user.rs` is shared. `root` must contain the kernel high half
/// ([`share_kernel_half`]).
pub(crate) fn ttbr0_value(root: u64, asid: u16) -> u64 {
    SATP_MODE_SV39 | ((asid as u64) << SATP_ASID_SHIFT) | (root >> 12)
}

/// Read the ASID back out of a composed [`ttbr0_value`], for portable tests.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the tests are its only caller; the kernel composes, never decomposes
pub fn asid_of(satp: u64) -> u16 {
    ((satp >> SATP_ASID_SHIFT) & 0xffff) as u16
}

/// Discard every TLB entry tagged with `asid` on every online hart. Only after it returns may the
/// number tag another space.
///
/// Local `sfence.vma x0, asid` first, which leaves the global kernel half alone, then an SBI RFENCE
/// that returns once every other online hart has run it. See notes/riscv-tlb-shootdown.md.
pub fn flush_asid(asid: u16) {
    // Local first: it orders this hart's page-table writes before the others are told.
    instructions::sfence_vma_asid(asid as u64);

    let others = crate::smp::online_harts_mask() & !(1usize << crate::cpu::id());
    if others != 0 {
        super::sbi_remote_sfence_vma_asid(others, asid);
    }
}

/// Let S-mode load and store through `U` pages (`sstatus.SUM`) when `allowed`, else forbid it;
/// returns whether it was permitted before.
///
/// Test builds only, so a shipping kernel that strays into the low half faults. See
/// notes/riscv-port/memory.md.
#[cfg(any(test, feature = "system_tests"))]
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the system tests call it; a unit-test boot on some ISAs does not
pub fn permit_kernel_access_to_user_pages(allowed: bool) -> bool {
    const SSTATUS_SUM: u64 = 1 << 18;
    let previous = if allowed {
        instructions::read_and_set_sstatus(SSTATUS_SUM)
    } else {
        instructions::read_and_clear_sstatus(SSTATUS_SUM)
    };
    previous & SSTATUS_SUM != 0
}

/// Install a user address space by writing its composed `satp`.
///
/// # Safety
/// `satp` must name a well-formed Sv39 root that includes the kernel high-half (else the next
/// instruction fetch faults); the caller owns that invariant, as on aarch64.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the system tests are the callers, as on the other two
pub unsafe fn activate_user(satp: u64) {
    // SAFETY: this function's own `# Safety` contract is exactly the one this call needs; it
    // forwards, it does not weaken.
    unsafe { write_satp(satp) };
}

/// Remove the user address space from this hart: fall back to the kernel-only reserved root.
pub fn deactivate_user() {
    // SAFETY: `reserved_root()` composes `KERNEL_ROOT`, the fine map `init` built and stored, which
    // is `'static` and contains the kernel high half by construction. Its low half is empty, so
    // every user address faults, which is the point.
    unsafe { switch_user_root(reserved_root()) };
}

/// The leaf the hardware would find for `va` on this hart, in whichever half `va` names.
///
/// [`translate_user`] walks only the low half, so it answers `None` for every kernel address.
fn translate_in_either_half(va: u64) -> Option<(u64, Flags)> {
    let root = current_root_pa();
    // SAFETY: `root` is the live installed root; the direct map makes `phys_to_ptr` valid; a
    // translate allocates nothing, so the `|| None` allocator is never called.
    let half = |h| unsafe { Mapper::<_, _, Format>::new(root, h, || None, phys_to_ptr) };
    half(Half::Low)
        .translate(va)
        .or_else(|| half(Half::High).translate(va))
}

/// Whether U-mode may read `va` in the installed space, by walking both halves and checking `U`.
///
/// RISC-V has no `AT S1E0R`. The system tests are the only caller
/// (`the_page_tables_say_u_mode_cannot_read_the_kernels_memory`).
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn user_can_read(va: u64) -> bool {
    translate_in_either_half(va).is_some_and(|(_, f)| f.is_user_accessible())
}

/// Whether U-mode may write `va`: user-accessible and writable. Same disposition, same test.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn user_can_write(va: u64) -> bool {
    translate_in_either_half(va).is_some_and(|(_, f)| f.is_user_accessible() && f.is_writable())
}

/// The physical root of the currently installed address space (`satp.PPN << 12`).
pub fn current_user_root() -> u64 {
    current_root_pa()
}

/// Unmap one user page at `va` in the space rooted at `root`, invalidate the TLB, and return the
/// frame it named.
pub(crate) fn unmap_user_at(root: u64, va: u64) -> Option<u64> {
    // SAFETY: `root` is a live low-half-owning root; `unmap` allocates nothing; the direct map makes
    // `phys_to_ptr` valid.
    let mut mapper = unsafe { Mapper::<_, _, Format>::new(root, Half::Low, || None, phys_to_ptr) };
    let (pa, flush) = mapper.unmap(va).ok()?;
    flush.flush(flush_tlb);
    Some(pa)
}

/// Unmap many user pages of the space at `root` and invalidate once, by `asid`, for
/// [`crate::revoke::revoke_region`]. Name provisional (lane 2451, 2026-10-20 UTC).
///
/// `each` calls the unmap it is handed once per address; unmapped pages are skipped. The caller
/// must not free any frame before this returns. See notes/riscv-port/memory.md.
pub(crate) fn unmap_user_pages(root: u64, asid: u16, each: impl FnOnce(&mut dyn FnMut(u64))) {
    // SAFETY: as `unmap_user_at`.
    let mut mapper = unsafe { Mapper::<_, _, Format>::new(root, Half::Low, || None, phys_to_ptr) };
    let mut unmapped = false;
    each(&mut |va| {
        if let Ok((_, flush)) = mapper.unmap(va) {
            flush.flush(|_| {}); // owed to, and paid by, the one flush_asid below
            unmapped = true;
        }
    });
    if unmapped {
        flush_asid(asid);
    }
}

/// Cut the page table at `table` out of the walk to `va` in the space at `root`, and flush tag
/// `asid` on every core. Name provisional (the page-tables-outlive-destroy lane, 2026-10-05 UTC).
///
/// Returns the span that went with it as `(base, size)`, or `None` if the walk no longer passes
/// through `table`. By tag, because the walk caches above the leaves must go too.
pub(crate) fn cut_user_table(root: u64, va: u64, table: u64, asid: u16) -> Option<(u64, u64)> {
    // SAFETY: `root` is a live low-half table (the registry forgets a root before its space frees
    // it); the direct map makes `phys_to_ptr` valid; a cut allocates nothing.
    let mut mapper = unsafe { Mapper::<_, _, Format>::new(root, Half::Low, || None, phys_to_ptr) };
    let (base, span, flush) = mapper.unlink_table(va, table)?;
    flush.flush(|_| flush_asid(asid));
    Some((base, span))
}

/// Translate `va` in the space rooted at physical `root`.
pub fn translate_at(root: u64, va: u64) -> Option<(u64, Flags)> {
    // SAFETY: `root` is a page table; the direct map makes `phys_to_ptr` valid; no allocation.
    let mapper = unsafe { Mapper::<_, _, Format>::new(root, Half::Low, || None, phys_to_ptr) };
    mapper.translate(va)
}

/// A run of user pages mapped into the current space, invalidated once when it drops, for
/// `PageFrame::MAP`. Name provisional (lane 2451, 2026-10-20 UTC).
///
/// Holds the span mapped so far. The drop fences each page locally and every other hart once. See
/// notes/riscv-port/memory.md.
pub(crate) struct UserMapBatch {
    lo: u64,
    hi: u64,
}

impl UserMapBatch {
    pub(crate) const fn new() -> Self {
        Self {
            lo: u64::MAX,
            hi: 0,
        }
    }

    /// Map one page as [`map_current_user_page_frame`] does, leaving its invalidate to the drop.
    pub(crate) fn map(
        &mut self,
        va: u64,
        phys: u64,
        flags: Flags,
        alloc: impl FnMut() -> Option<u64>,
    ) -> Result<(), MapError> {
        let root = current_root_pa();
        // SAFETY: as `map_current_user_page_frame`.
        let mut mapper =
            unsafe { Mapper::<_, _, Format>::new(root, Half::Low, alloc, phys_to_ptr) };
        mapper.map(va, phys, flags)?;
        self.lo = self.lo.min(va);
        self.hi = self.hi.max(va + PAGE_SIZE);
        Ok(())
    }
}

impl Drop for UserMapBatch {
    fn drop(&mut self) {
        if self.lo >= self.hi {
            return;
        }
        for va in (self.lo..self.hi).step_by(PAGE_SIZE as usize) {
            instructions::sfence_vma_page(va);
        }
        let others = crate::smp::online_harts_mask() & !(1usize << crate::cpu::id());
        if others != 0 {
            super::sbi_remote_sfence_vma(others, self.lo as usize, (self.hi - self.lo) as usize);
        }
    }
}

/// Map one user page at `va` onto the owned frame `phys` in the current space, drawing tables from
/// `alloc`, then flush `va`: RISC-V may need an `sfence.vma` to see a freshly valid leaf.
pub(crate) fn map_current_user_page_frame(
    va: u64,
    phys: u64,
    flags: Flags,
    alloc: impl FnMut() -> Option<u64>,
) -> Result<(), MapError> {
    let root = current_root_pa();
    // SAFETY: `root` is the live installed root; the direct map makes `phys_to_ptr` valid.
    let mut mapper = unsafe { Mapper::<_, _, Format>::new(root, Half::Low, alloc, phys_to_ptr) };
    mapper.map(va, phys, flags)?;
    flush_tlb(va);
    Ok(())
}

/// The `satp` a thread with no user space runs on: the kernel root, whose empty low half faults
/// every user address.
pub(crate) fn reserved_root() -> u64 {
    ttbr0_value(KERNEL_ROOT.load(Ordering::Relaxed), 0)
}

/// Install the address space named by the composed `satp`, unless the live `satp` already is it.
///
/// `#[inline(always)]`, with [`write_satp`], so both stay in `.text.hot` with the context switch.
/// See notes/riscv-port/memory.md.
///
/// # Safety
/// `satp` must be a value [`ttbr0_value`] composed over a **live** `AddressSpace`'s root, or
/// [`reserved_root`]. Same contract as [`write_satp`]; see the aarch64 twin for why the liveness
/// half of it cannot be carried by a type instead.
#[inline(always)]
pub(crate) unsafe fn switch_user_root(satp: u64) {
    if read_satp() == satp {
        return;
    }
    // SAFETY: this function's own `# Safety` contract is exactly the one this call needs; it
    // forwards, it does not weaken. `satp` is already a composed value (from `ttbr0_value` or
    // `reserved_root`), so it is written directly.
    unsafe { write_satp(satp) };
}

/// Serializes edits to the kernel's live tables across harts, at aarch64's lock rank.
static KERNEL_MMU: crate::sync::IrqSafeMutex<()> =
    crate::sync::IrqSafeMutex::new(crate::sync::rank::KERNEL_MMU, ());

/// The kernel's live page tables, as a `Mapper` rooted at `KERNEL_ROOT`, which every hart shares.
///
/// Context: call only while holding [`KERNEL_MMU`].
#[allow(clippy::type_complexity)]
fn kernel_mapper() -> Mapper<impl FnMut() -> Option<u64>, fn(u64) -> *mut PageTable, Format> {
    let root = KERNEL_ROOT.load(Ordering::Relaxed);
    // SAFETY: `root` is the fine kernel table built by `init`; the direct map makes `phys_to_ptr`
    // valid for every table frame.
    unsafe {
        Mapper::new(
            root,
            Half::High,
            || memory::alloc().map(|f| f.addr()),
            phys_to_ptr,
        )
    }
}

/// Map one page into the kernel's own (high-half) address space.
///
/// Context: takes [`KERNEL_MMU`].
pub(crate) fn map_page(va: u64, pa: u64, flags: Flags) -> Result<(), MapError> {
    let _guard = KERNEL_MMU.lock(); // exclusive: two harts must not mutate the tables at once
    kernel_mapper().map(va, pa, flags)
}

/// Remove one page at `va` from the kernel's space, invalidate it on every hart, and return the
/// frame, which the caller frees.
///
/// Context: takes [`KERNEL_MMU`].
pub(crate) fn unmap_page(va: u64) -> Result<u64, MapError> {
    let _guard = KERNEL_MMU.lock(); // exclusive: see map_page
    let (pa, flush) = kernel_mapper().unmap(va)?;
    flush.flush(flush_tlb);
    Ok(pa)
}

/// Invalidate `va`'s translation, for every ASID, on every online hart. Discharges a
/// `paging::TlbFlush`.
///
/// `sfence.vma` orders the preceding page-table write and completes locally, so no barrier is
/// needed. Other harts get an SBI RFENCE once they are online.
pub(crate) fn flush_tlb(va: u64) {
    instructions::sfence_vma_page(va);

    // No hardware broadcast: every other online hart may cache it, so shoot down through SBI.
    let others = crate::smp::online_harts_mask() & !(1usize << crate::cpu::id());
    if others != 0 {
        super::sbi_remote_sfence_vma(others, va as usize, PAGE_SIZE as usize);
    }
}

/// Whether paging is on: `satp`'s MODE field is not Bare (0). True from `boot.s`'s Sv39 switch on.
pub(crate) fn is_enabled() -> bool {
    read_satp() >> 60 != 0
}

/// Translate a kernel virtual address through the live kernel tables.
///
/// Context: takes [`KERNEL_MMU`].
pub fn translate(va: u64) -> Option<(u64, Flags)> {
    let _guard = KERNEL_MMU.lock();
    kernel_mapper().translate(va)
}

/// Translate a user virtual address through the currently installed address space.
pub fn translate_user(va: u64) -> Option<(u64, Flags)> {
    translate_at(current_root_pa(), va)
}

/// The live `satp` and the leaf a walk of it finds for `va` now, for the user-fault report.
///
/// A `satp` that is not the thread's own says the wrong space was live; a leaf that permits the
/// access says the refusal came from something cached. Read after the fault (`classify`'s BUGS).
///
/// Name: provisional (lane maintainer/ntp-c906-flake, 2026-10-13 UTC).
pub fn installed_satp_and_leaf(va: u64) -> (u64, Option<Flags>) {
    (read_satp(), translate_in_either_half(va).map(|(_, f)| f))
}

/// Whether `va` has any translation in the space installed on this hart, in either half, for the
/// fault classifier.
///
/// Both halves, because a user thread reaching for kernel memory names a high-half address, which
/// the process root carries ([`share_kernel_half`]).
pub(crate) fn is_mapped_in_current_space(va: u64) -> bool {
    let root = current_root_pa();
    // SAFETY: `root` is the live installed root; the direct map makes `phys_to_ptr` valid; a
    // translate allocates nothing, so the `|| None` allocator is never called.
    let half = |h| unsafe { Mapper::<_, _, Format>::new(root, h, || None, phys_to_ptr) };
    half(Half::Low).translate(va).is_some() || half(Half::High).translate(va).is_some()
}

/// Whether kernel address `va` is mapped, asked without taking [`KERNEL_MMU`], for fault handlers
/// whose lock holder may be the thread that just died.
///
/// Walks this hart's installed root, which carries the kernel half ([`share_kernel_half`]).
///
/// Name: provisional (2026-08-17): calef has not ruled on it.
pub fn is_mapped(va: u64) -> bool {
    let root = current_root_pa();
    // SAFETY: `root` is the live installed root; the direct map makes `phys_to_ptr` valid; a
    // translate allocates nothing, so the `|| None` allocator is never called.
    let mapper = unsafe { Mapper::<_, _, Format>::new(root, Half::High, || None, phys_to_ptr) };
    mapper.translate(va).is_some()
}

/// Copy the kernel root's top-level entries 256..512 into the fresh process root at `root`, so one
/// `satp` sees the user low half and the whole shared kernel high half.
///
/// Called by `user::AddressSpace` right after it allocates a root. See notes/riscv-port/memory.md.
pub(crate) fn share_kernel_half(root: u64) {
    let kernel_root = KERNEL_ROOT.load(Ordering::Relaxed);
    // SAFETY: both are page-aligned root tables reachable through the direct map. We copy only the
    // high-half entries; the low half stays zero for the process's own user mappings.
    unsafe {
        let dst = &mut (*phys_to_ptr(root)).entries;
        let src = &(*phys_to_ptr(kernel_root)).entries;
        dst[256..paging::ENTRIES].copy_from_slice(&src[256..paging::ENTRIES]);
    }
}

/// Print a human summary of the kernel's mapping for the machine description: `satp`, the kernel
/// root and the live root.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub(crate) fn print_summary() {
    crate::println!(
        "  mmu             : Sv39 {}, one satp, kernel high half at {:#018x}",
        if is_enabled() {
            "on, fine-grained W^X tables installed"
        } else {
            "OFF (still on the boot map)"
        },
        KERNEL_VA_BASE,
    );
    crate::println!(
        "                  : kernel root {:#018x}, live satp root {:#018x}",
        KERNEL_ROOT.load(Ordering::Relaxed),
        current_root_pa(),
    );
}

#[cfg(test)]
mod tests;

/// Proofs of the logic around `satp`, against stubs of `csrr satp`, `csrw satp` and `sfence.vma`.
///
/// The stubs model the hardware from the privileged specification, unchecked against silicon, and
/// the proofs hold only under that model. See notes/riscv-port/memory.md and
/// notes/kernel-proofs/stubbing-an-instruction.md.
#[cfg(kani)]
mod proofs;
