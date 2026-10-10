//! **RISC-V virtual memory.** The Sv39 page-table half of the `arch` contract.
//!
//! The kernel lives in the Sv39 high half (`boot.s` does the higher-half transition on a coarse boot
//! table; [`init`] then builds the fine-grained W^X kernel tables and switches `satp` to them). The
//! page-table *format* (Sv39 descriptor bits, three levels) lives in `paging::Sv39` behind the
//! `PageFormat` trait (HAL leak #2, DECISIONS §17); this module is the RISC-V glue: `satp`
//! composition, the kernel and user mapping surface via `Mapper<_, _, Sv39>`, the single-`satp`
//! address-space model (`share_kernel_half`), and TLB maintenance (`sfence.vma`). See
//! notes/riscv-port.md.

use core::ffi::c_void;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use paging::{Flags, Half, MapError, Mapper, PAGE_SIZE, PageSize, PageTable};

use super::instructions;
use crate::memory;

/// This architecture's page-table format. Portable code names it as `arch::mmu::Format` (see the
/// aarch64 module's alias for why), so the user-VA gate and the user `Mapper` land on Sv39 here.
///
/// **Per machine** since milestone 89 (Scaleway EM-RV1): on the T-Head TH1520 the leaves also carry
/// T-Head's memory types, because firmware has turned `XTheadMae` on and plain Sv39's zeros there are
/// undefined. `arch::isa::init` refuses a hart that disagrees with the format chosen here.
#[cfg(not(feature = "board_th1520"))]
pub type Format = paging::Sv39;
#[cfg(feature = "board_th1520")]
pub type Format = paging::Sv39Mae;

/// The UART, mapped as device memory in the direct map. Without it the machine goes silent the
/// instant we switch off the coarse boot table. Per machine, so it is [`super::machine`]'s.
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

/// Install a whole address space (kernel high half + user low half) by writing `satp`.
/// **No TLB flush, on hardware whose ASID field is wide enough to be trusted.**
///
/// # Where the sledgehammer went (milestone 58)
///
/// This used to be `csrw satp` followed by a bare `sfence.vma`: every translation this hart had
/// cached, kernel entries included, discarded on every context switch. It had to be, because
/// nothing else made a dying address space's entries stop matching a new one, and RISC-V permits
/// `satp.ASID` to be **zero bits wide**, in which case every space really does share tag 0 and their
/// entries really would alias.
///
/// Three things had to exist before it could go, and now do. Every user mapping is non-global
/// (`paging::Sv39` sets `G` only for the kernel's half), so its TLB entries carry the ASID that was
/// live during the walk. Each address space owns one ASID for life (`crates/address_space_identifier`), freed only after
/// [`flush_asid`] has swept it **from every hart**. And [`probe_asid_bits`] measures the field the
/// hardware actually implements, so this decision is made against the machine rather than against
/// the specification's permission.
///
/// On a core that implements too few bits, [`asid_tagging_is_trusted`] is false and the flush stays.
/// That is the honest degradation: correct and slow beats fast and silently wrong, and a panic would
/// refuse to boot a machine that works.
///
/// # Safety
/// `satp` must name a well-formed, live Sv39 root that contains the kernel's high half. It is the
/// whole address space on this ISA, so a root missing the kernel half faults on the very next
/// instruction fetch, and a root whose frames have been freed hands this hart somebody else's
/// memory. The table must stay live until some later write to `satp` replaces it.
///
/// **This is `unsafe fn` because aarch64's `set_ttbr0` is** (milestone 112). It used to be a safe
/// fn carrying a `// SAFETY:` comment that discharged onto "the caller",
/// which imposed the obligation on nobody: the same register write, the one that installs a user
/// address space, was a contract on one architecture and an ordinary call on the other. Rule 5 is
/// about capabilities shipping on every ISA; a *rule about the code* that differs by ISA is the same
/// defect one level up.
#[inline(always)] // on the context switch's hot path; see `switch_user_root`'s note
unsafe fn write_satp(satp: u64) {
    // SAFETY: this function's own `# Safety` contract is exactly the one this write needs; it
    // forwards, it does not weaken.
    unsafe { instructions::write_satp(satp) };

    if !asid_tagging_is_trusted() {
        // The full sweep is what makes the switch safe when the tag cannot be relied on to keep two
        // spaces apart.
        instructions::sfence_vma_all();
    }
}

/// The physical address of the currently-installed root page table (`satp.PPN << 12`).
fn current_root_pa() -> u64 {
    (read_satp() & SATP_PPN_MASK) << 12
}

/// The base of the kernel's virtual address space: the Sv39 high half (bits 63:38 all one, the sign
/// extension of bit 38 = 1). Chosen exactly like aarch64's base so `VA = PA | KERNEL_VA_BASE` is
/// exact and a kernel VA shares its physical address's page-table indices. Matched to
/// `KERNEL_VA_BASE` in link-riscv64.ld, and the kernel runs here from `boot.s`'s high-half jump on.
pub const KERNEL_VA_BASE: u64 = 0xffff_ffc0_0000_0000;

/// **Where kernel thread stacks live, virtually** (`thread.rs`'s `STACK_AREA`).
///
/// Deliberately far above the direct map, so a stack address can never collide with the virtual
/// *name* of a physical one. 64 GiB up: RAM will not reach there for a while.
///
/// Name: provisional (milestone 161 (the `x86_64` kernel port)): this was a portable expression in
/// `thread.rs` (`KERNEL_VA_BASE | 0x10_0000_0000`) until `x86_64` arrived, where the expression is
/// not merely wrong but a no-op -- `KERNEL_VA_BASE` there already has every bit of `0x10_0000_0000`
/// set, so the OR yielded the kernel image's own base and every kernel thread stack would have been
/// mapped over `.text`. Rule 1 says an architecture's addresses live under `arch/`; this is that.
pub const THREAD_STACK_AREA: u64 = KERNEL_VA_BASE | 0x0000_0010_0000_0000;

/// **The device window** (milestone 89 (Scaleway EM-RV1)): the top gigabyte of a 40-bit physical
/// space, named at the top gigabyte of the Sv39 high half.
///
/// `pa + KERNEL_VA_BASE` is the whole direct map on every other machine this tree boots, and under
/// Sv39 it can only name the first 256 GiB of physical space: the high half *is* 256 GiB. The T-Head
/// TH1520 puts every peripheral at `0xff_d800_0000` and up (PLIC, CLINT, UARTs), 1 TiB up, so for it
/// `phys_to_virt` overflows. Rather than grow a virtual-address allocator for devices (Linux's
/// `ioremap`), this kernel names one fixed gigabyte: physical `0xff_c000_0000..0x100_0000_0000`
/// appears at virtual `0xffff_ffff_c000_0000`, root index 511. Everything else stays the direct map,
/// now capped at 255 GiB so the two cannot meet ([`DIRECT_MAP_LIMIT`]).
///
/// What it costs: one compare in [`phys_to_virt`] and [`virt_to_phys`], and one boot-table entry. On
/// QEMU `virt` and on radon nothing lives in that gigabyte, so the entry is never walked. Devices in
/// the window are mapped page by page by `direct_map` exactly as low ones are; the window decides
/// only *where* they are named. If a machine ever puts devices in two distant gigabytes, this
/// verdict changes (milestone 89's seventh question says so too).
pub const DEVICE_WINDOW_PA: u64 = 0xff_c000_0000;
/// One gigabyte: one root entry.
pub const DEVICE_WINDOW_SIZE: u64 = 1 << 30;
/// Root index 511, the last gigabyte of the high half.
pub const DEVICE_WINDOW_VA: u64 = 0xffff_ffff_c000_0000;
/// The first physical address the direct map does not reach: 255 GiB, so that `pa +
/// KERNEL_VA_BASE` stops one gigabyte short of [`DEVICE_WINDOW_VA`].
pub const DIRECT_MAP_LIMIT: u64 = DEVICE_WINDOW_VA - KERNEL_VA_BASE;

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

/// The boot page table: a single Sv39 root that maps the low physical range (to survive turning
/// paging on) and its high-half alias (where the kernel is linked). Six gigapage (1 GiB) leaves are
/// enough to run, print, and read the device tree: indices 0..=2 identity-map the UART region
/// (gigapage 0), the VisionFive 2's first gigabyte of DRAM (gigapage 1), and the kernel/RAM region
/// (gigapage 2); 256..=258 are the same three at `KERNEL_VA_BASE` (adding 256 to the top-level
/// index). It is RWX everywhere, like aarch64's coarse boot map: it exists to survive ~twenty
/// instructions until `mmu::init` builds the real W^X tables. See boot.s and notes/riscv-port.md.
///
/// Gigapage 1 (`0x4000_0000..0x8000_0000`) is there for the board, not for QEMU. The JH7110's DRAM
/// starts at `0x4000_0000` and U-Boot's default `fdt_addr_r` is `0x4600_0000`, so without this entry a
/// DTB left at the default address faults on the first read, before the trap path can print
/// (notes/visionfive2.md). On QEMU `virt` this range is the 32-bit PCI window, which nothing
/// touches while the boot table is live, so mapping it is inert there. A DTB near the top of an
/// 8 GB board's RAM (`$fdtcontroladdr`, above 4 GiB) is still out of reach; the bench runbook's
/// `fdt move` remains the answer for that case, on the record in notes/visionfive2.md.
///
/// `boot.s` reads its **physical** address (PC-relative) to load `satp`, so it must be a real static
/// with a stable symbol. It is `.data` (initialized), loaded at its low physical address.
#[repr(C, align(4096))]
struct BootTable([u64; paging::ENTRIES]);

const fn boot_table() -> BootTable {
    // Sv39 gigapage leaf: V R W X A D set. RWX is deliberate and temporary (see above).
    const LEAF: u64 = (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 6) | (1 << 7);
    // **T-Head's memory type** (milestone 89): on a hart with `XTheadMae` on, every leaf names one,
    // including these. Memory is cacheable, bufferable and shareable; the device window is strongly
    // ordered and shareable. Linux's `_PAGE_PMA_THEAD` and `_PAGE_IO_THEAD`, and the same values
    // `paging::Sv39Mae` writes into the fine tables. Zero on every other build, where bits 63:59
    // are reserved and a set one is a page fault.
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
    // **The TH1520's whole 16 GiB** (milestone 89). Its DRAM starts at 0, and Scaleway's U-Boot
    // places the device tree and the archive wherever `bootm` decides, which nothing here has seen.
    // radon learned that a tree above the boot table's reach dies before the trap path can print,
    // and fixed it with `fdt move` at the U-Boot prompt; the RV1 has no prompt anyone is promised.
    // So this build names all of RAM from the first instruction. Not done for radon or QEMU,
    // whose maps stay byte-identical, though it would be as inert there.
    #[cfg(feature = "board_th1520")]
    {
        let mut g = 3;
        while g < 16 {
            t[g] = giga(g as u64 * (1 << 30));
            t[256 + g] = t[g];
            g += 1;
        }
    }
    // The device window (milestone 89), so a machine whose console is up there can print before
    // `mmu::init`. High alias only: nothing touches a device while the PC is still low. Inert on
    // QEMU `virt` and radon, which have nothing in that gigabyte.
    t[511] = giga(DEVICE_WINDOW_PA);
    BootTable(t)
}

/// The boot table instance `boot.s` points `satp` at. `#[unsafe(no_mangle)]` so the assembly can
/// name it; `pub` and read from asm, so not actually dead despite appearances.
#[unsafe(no_mangle)]
static BOOT_PAGE_TABLE: BootTable = boot_table();

/// The physical address of the virtio-mmio transport window on QEMU's `virt` machine. The `virt`
/// board lays out 8 virtio-mmio slots of 0x1000 each starting at `0x1000_1000`, growing downward by
/// slot; this base and the count below describe that window for the driver layer.
pub const VIRTIO_MMIO_BASE: u64 = 0x1000_1000;
/// The size of the virtio-mmio window (8 slots of 0x1000).
pub const VIRTIO_MMIO_SIZE: u64 = 8 * 0x1000;
/// The first PLIC interrupt id for the virtio-mmio slots on the `virt` machine (irq 1 is the first
/// virtio slot; the driver adds the slot index).
pub const VIRTIO_IRQ_BASE: u32 = 1;
/// RISC-V's `virt` lays out 8 virtio-mmio slots 0x1000 apart (aarch64's are 32, 0x200 apart). The
/// probe (`virtio::find_block_device`) walks them.
pub const VIRTIO_SLOT_STRIDE: u64 = 0x1000;
pub const VIRTIO_SLOTS: u64 = 8;

/// Whether `map_everything` mapped the virtio-mmio window, which it does only when the device tree
/// names a `virtio,mmio` node (milestone 89). Written once on the primary hart before any probe.
static VIRTIO_MMIO_MAPPED: AtomicBool = AtomicBool::new(false);

/// How many virtio-mmio slots the probe may read: [`VIRTIO_SLOTS`] on QEMU `virt`, and zero on a
/// machine whose tree names no such bus. radon's JH7110 has UART0's register block at this address
/// and the TH1520 has DRAM, so reading "slots" there was at best noise and at worst a RAM word that
/// happened to spell the magic, handed to userspace as a device.
///
/// Name: provisional, milestone 89 (Scaleway EM-RV1)'s lane, 2026-10-06 (UTC). The same name on all three architectures.
pub fn virtio_slots() -> u64 {
    if VIRTIO_MMIO_MAPPED.load(Ordering::Relaxed) {
        VIRTIO_SLOTS
    } else {
        0
    }
}

/// How much of the PCIe ECAM window the kernel maps: **bus 0 only** (4 KB per function, 1 MB per
/// bus). The window's *base and size* come from the device tree (`memory::pci_regions`, the
/// `pci-host-ecam-generic` node's `reg`); this cap is kernel policy, because QEMU `virt` is a
/// flat root complex with every device on bus 0, and a 4 KB-page map of all 256 buses would cost
/// 64K PTEs for space that reads all-ones. Widening is one constant if a bridge topology ever
/// appears. This base *was* a QEMU constant here (`PCI_ECAM_BASE = 0x3000_0000`), which is the
/// DECISIONS §43 class the first VisionFive 2 boot paid for; the kernel test in pci.rs holds the
/// discovered value to the old one on QEMU.
/// **The floor, not the answer, since milestone 320.** `pci::ecam_buses()` is what the kernel maps
/// and reads: this value until a survey has run, and the machine's own topology afterwards. On this
/// architecture no survey runs, so it stays 1 and is the truth here.
pub const PCI_ECAM_BUSES: u16 = 1;

/// How much of the 32-bit PCI memory window the kernel maps and assigns BARs from. The window
/// itself comes from the device tree (`memory::pci_regions`, the bridge's `ranges`); with
/// `-bios default` nobody has programmed a BAR before us (OpenSBI does no PCI), so the kernel
/// places them itself, bumping from the discovered base (kernel/src/pci.rs). A 2 MB slice: a
/// virtio function's register BAR is 16 KB, so this bounds the kernel's page-table spend while
/// leaving room for dozens of devices.
pub const PCI_BAR_MAPPED: u64 = 0x20_0000;

/// The PLIC input for INTx line A on the `virt` board's root complex; B, C, D follow. A device's
/// line is `PCI_IRQ_BASE + ((dev + pin - 1) % 4)`, the standard swizzle (`pci::intx_irq`); the
/// dtb fixture test walks the machine's own `interrupt-map` and asserts the formula matches.
pub const PCI_IRQ_BASE: u32 = 32;

/// Physical to kernel-virtual: `pa + KERNEL_VA_BASE`, except in the [device window](DEVICE_WINDOW_PA).
///
/// **One formula for both, with no branch**, because this is inlined all over the IPC fastpath and
/// `script/fastpath-footprint` measured a compare-and-branch here at +122 bytes on `ipc_call_reply`
/// (milestone 89 (Scaleway EM-RV1)). The window was placed so that this works: its physical base,
/// `0xff_c000_0000`, is 255 GiB modulo 256 GiB, which is exactly where its virtual base sits above
/// `KERNEL_VA_BASE`. So keeping the low 38 bits and OR-ing in the high half maps RAM below 255 GiB
/// to `pa + KERNEL_VA_BASE` (the OR is the add, since the low bits never carry into the base) and
/// the window to [`DEVICE_WINDOW_VA`].
///
/// **The assumption, and it is load-bearing:** every physical address handed here is below 255 GiB
/// or inside the window. Any other one, anything at or above 2^38 outside the window, silently
/// aliases onto a different frame. It holds because Sv39 gives the kernel only 256 GiB to name
/// things in, and every machine this tree boots keeps its RAM in the first 16 GiB. It is enforced
/// where it can be checked without a cost per call: [`refuse_unnameable_ram`] at the top of
/// [`init`] for RAM, and `direct_map` for every device mapping.
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

/// **Refuse a machine whose RAM [`phys_to_virt`] would alias**, before the first frame is named
/// through the direct map (milestone 89 (Scaleway EM-RV1)).
///
/// The mask in [`phys_to_virt`] keeps only the low 38 bits, so a RAM region reaching 255 GiB or
/// beyond would land on the device window or wrap onto low RAM, and the kernel would write one
/// frame while believing it wrote another. That cannot happen on any machine this tree boots: Sv39
/// itself caps the kernel's half at 256 GiB, QEMU `virt` here has at most a few GiB from
/// `0x8000_0000`, radon has 8 GiB from `0x4000_0000`, and the TH1520 16 GiB from 0. A machine with
/// more needs a second window or Sv48, and this makes it say so at boot instead of corrupting
/// memory. Called first thing in [`init`]: before it, the boot table maps only the low gigabytes,
/// so a high frame would fault rather than alias.
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

/// A physical page-table address as a kernel pointer. Identity in bare mode; the direct map makes it
/// valid once the Sv39 step maps all of RAM into the high-half. Same role as the aarch64 helper.
pub fn phys_to_ptr(pa: u64) -> *mut PageTable {
    phys_to_virt(pa) as *mut PageTable
}

/// Build the kernel's fine-grained Sv39 tables and switch `satp` to them, replacing the coarse RWX
/// boot table (`BOOT_PAGE_TABLE`) that `boot.s` installed. The new tables are W^X: `.text` executable
/// and read-only, `.rodata` read-only, everything else non-executable, the guard page unmapped.
///
/// We are already running in the high half on the boot table; the fine table maps the same kernel
/// VAs to the same frames, so the `csrw satp` is seamless (the next instruction fetch resolves
/// identically). This is the RISC-V counterpart of the aarch64 `mmu::init`, one register instead of
/// the TTBR0/TTBR1 pair.
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

    // Probe after `install`, so the ASID tag is exercised against the real kernel root rather than
    // the coarse boot table. See `probe_asid_bits`: this validates the assumption `crates/address_space_identifier`
    // is already built on, and it is the gate on ever removing the flush in `write_satp`.
    ASID_BITS.store(probe_asid_bits(), Ordering::Relaxed);

    // And the decision the probe exists to make (milestone 58): may a context switch stop flushing?
    // Set here, once, on the primary hart, before any secondary is started and before any user
    // address space is created, so every `write_satp` in the machine's life reads a settled value.
    ASID_TAGGING_TRUSTED.store(
        ASID_BITS.load(Ordering::Relaxed) >= ASID_BITS_NEEDED as usize,
        Ordering::Relaxed,
    );
    // Read back through the accessor rather than the local, so the store/load path is exercised on
    // every boot and `asid_bits`'s "was it probed" assertion fires here if the ordering ever moves,
    // instead of at some later caller.
    //
    // Handed to the ISA record rather than printed here (milestone 60). It is still reported on
    // every boot, in the `isa` line a few steps later, which is what a number whoever brings up a
    // real board wants without running the suite: it says whether the ASID allocator's numbers are
    // distinguishable by this hardware at all. The test enforces >= 8.
    super::isa::record_asid_bits(asid_bits());
}

/// Switch `satp` to the Sv39 tables rooted at physical `root`, and flush the TLB.
///
/// # Safety
/// `root` must be a complete Sv39 kernel map covering the currently-executing code, stack, and any
/// memory touched before the next `sfence`; otherwise the instruction after the `csrw` faults.
unsafe fn install(root: u64) {
    let satp = SATP_MODE_SV39 | (root >> 12);
    // sfence.vma before and after brackets the switch so no stale boot-table entry survives. Three
    // `asm!` blocks rather than one since 2026-09-25, so each is a stubbable instruction; none is
    // `nomem`, so the compiler cannot move a memory access into the gap between them either way.
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

/// **Discover how many `satp.ASID` bits exist, because `crates/address_space_identifier` assumes at least eight.**
///
/// `satp.ASID` is **WARL**: an implementation may hardwire any number of those bits to zero,
/// *including all of them*. That is not a hypothetical corner of the spec; it is the cheap option
/// for a small core, and the VisionFive 2's U74 has not been checked.
///
/// It matters because [`crates/address_space_identifier`](address_space_identifier) is built on an assumption it states out loud: 255 usable
/// numbers, "below even the smallest hardware ASID space (8-bit, 256)". That holds on aarch64, where
/// the architecture *mandates* at least 8 bits. RISC-V mandates none. On a machine with zero
/// implemented bits, every one of the 160 address spaces would carry ASID 0 in hardware and their
/// TLB entries would **alias**: one process reading another's memory, with nothing to signal it.
///
/// This probe is what the context switch's silence is bought with (milestone 58). [`write_satp`]
/// used to follow every `csrw satp` with an unconditional `sfence.vma`, throwing the whole TLB away
/// on each switch, so no entry ever survived long enough to alias; that flush was load-bearing for
/// correctness rather than merely slow. It is now conditional on this number, through
/// [`asid_tagging_is_trusted`]: a machine that implements fewer bits than `crates/address_space_identifier` needs keeps
/// the sweep and pays for it. See notes/riscv-tlb-shootdown.md.
///
/// The probe writes ones into the ASID field of the *current* `satp`, leaving MODE and PPN alone, and
/// reads back which bits stuck. The address space is unchanged throughout (only the tag moves), so
/// the worst case is TLB misses that re-walk the same page table and find the same mappings.
///
/// **Rust around five one-instruction wrappers since 2026-09-25, not one `asm!` block**, so
/// `mod proofs` can prove the two things this function promises: the count is the implemented bits,
/// and `satp` is put back exactly. The proposal that priced this work classed the probe as
/// must-stay-asm; it need not be. Code the compiler places between the writes runs on the same root
/// under a different tag, which is the state the probe was always in, and none of the five blocks is
/// `nomem`, so no memory access can be moved out past the closing `sfence.vma`.
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
    // WARL bits need not be contiguous in principle; count what is set rather than assuming a
    // low-bit mask, so a strange implementation is reported honestly instead of rounded.
    implemented.count_ones() as usize
}

/// How many `satp.ASID` bits this hardware implements. Panics if read before [`init`] probed.
pub fn asid_bits() -> usize {
    let n = ASID_BITS.load(core::sync::atomic::Ordering::Relaxed);
    assert_ne!(n, usize::MAX, "asid_bits() read before mmu::init probed it");
    n
}

/// How many `satp.ASID` bits the allocator's numbers need: enough to hold `address_space_identifier::ASIDS - 1`, the
/// largest tag `crates/address_space_identifier` can hand out. Derived rather than written as `8`, so that raising
/// `ASIDS` moves the gate with it instead of leaving a constant behind that used to be right.
///
/// `pub(crate)` rather than private: `arch::riscv64::isa`'s own record test checks that
/// [`asid_tagging_is_trusted`] agrees with this threshold on whatever width the bench measures,
/// not just on the QEMU width, so it needs the same number `init` gates on.
pub(crate) const ASID_BITS_NEEDED: u32 = (address_space_identifier::ASIDS as u64 - 1).ilog2() + 1;

/// **Whether two live address spaces are guaranteed to be distinguishable in this hart's TLB.**
///
/// False until [`init`] has probed, which is the safe direction: every `satp` write before the probe
/// (and every one after it on a core that implements too narrow a field) carries the full
/// `sfence.vma` that [`write_satp`] used to do unconditionally.
static ASID_TAGGING_TRUSTED: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Whether the context switch may skip its TLB flush: `satp.ASID` is wide enough to hold every
/// number the allocator hands out, so two spaces can never share a hardware tag.
///
/// Read on the context-switch path, so it is a relaxed load of a value written once at boot, before
/// any secondary hart exists and before any user address space does.
pub fn asid_tagging_is_trusted() -> bool {
    ASID_TAGGING_TRUSTED.load(core::sync::atomic::Ordering::Relaxed)
}

/// Adopt the kernel's fine-grained Sv39 map on a secondary hart (SMP). `secondary_boot` brought this
/// hart up on the coarse `BOOT_PAGE_TABLE`, which reaches only the first few gigapages of the high
/// half, not the thread-stack area far above the direct map; switching `satp` to the shared
/// `KERNEL_ROOT` the primary built and verified gives this hart the same W^X map every other hart
/// runs on. All harts share the one kernel root, so there is nothing per-hart to build. The RISC-V
/// counterpart of the aarch64 `init_secondary`.
pub fn init_secondary() {
    let root = KERNEL_ROOT.load(Ordering::Relaxed);
    // SAFETY: the primary built this root and is running on it; it covers this code, this hart's
    // stack (mapped in the kernel image), and the UART, so the switch is seamless.
    unsafe { install(root) };
}

/// Build every mapping the kernel needs: the direct map of RAM, the W^X kernel sections, the stack,
/// and the UART. Mirrors the aarch64 `map_everything`.
fn map_everything<A, P>(m: &mut Mapper<A, P, Format>) -> Result<(), MapError>
where
    A: FnMut() -> Option<u64>,
    P: Fn(u64) -> *mut PageTable,
{
    // 1. The direct map: all of RAM at `pa | KERNEL_VA_BASE`, read/write, never executable, so the
    //    kernel can touch any frame the allocator hands it. Skip the kernel image, whose sections
    //    get tighter permissions below (the mapper refuses to overwrite, turning an ordering mistake
    //    into an error rather than a silently-wrong permission).
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

    // 3. The guard page is deliberately NOT mapped (stack-overflow trap). Skip it.

    // 4. The stack.
    map_range(m, stack_bottom(), stack_top(), Flags::kernel_data())?;

    // 4b. The per-CPU secondary stacks, one slot at a time so the bottom page of each slot stays a
    // hole (milestone 90). A loop rather than one range is the whole point, and it is why the
    // stacks moved out of `.bss`, which step 2 maps wholesale. Mirrors the aarch64 map; see
    // notes/stack-high-water.md.
    for id in 0..crate::cpu::MAX_CPUS {
        let (bottom, top) = crate::smp::secondary_stack_span(id);
        map_range(m, bottom, top, Flags::kernel_data())?;
    }

    // 4c. THE PER-CPU INTERRUPT STACKS (milestone 124), the same slot-at-a-time loop as 4b and for
    // the same reason: each slot's bottom page must stay a hole. These are where a trap taken on
    // kernel code builds its handler frames, so that a preemption is not charged to the thread it
    // interrupted. See kernel/src/interrupt_stack.rs.
    for id in 0..crate::cpu::MAX_CPUS {
        let (bottom, top) = crate::interrupt_stack::span(id);
        map_range(m, bottom, top, Flags::kernel_data())?;
    }

    // 5. The UART, device memory, in the direct map. Silence otherwise, the instant we switch.
    direct_map(m, UART_BASE, UART_BASE + UART_SIZE, Flags::device())?;

    // 6. The PLIC, device memory (milestone 20). Its base and size come from the device tree
    // (memory::init parsed it before this ran). Device-typed like the UART; the interrupt handler
    // and the boot demo reach it through the direct map. Absent on aarch64, so skip if unknown.
    if let Some((start, size)) = memory::plic_region() {
        direct_map(m, start, start + size, Flags::device())?;
    }

    // 6b. The JH7110's STG clock and reset generator (milestone 220), device memory. Present
    // only on a JH7110; `memory::jh7110_clock_and_reset` is None everywhere else, including on every
    // machine CI boots. Without this the kernel cannot ungate the TRNG's clocks, which is why
    // that device's whole register file read back as zeros on radon on 2026-09-04.
    if let Some(crg) = memory::jh7110_clock_and_reset() {
        direct_map(m, crg.base, crg.base + crg.size, Flags::device())?;
    }

    // 6c. The JH7110's SYS clock and reset generator (milestone 592 (radon's cold reboot dies in OpenSBI's PMIC write), provisional),
    // device memory,
    // under 6b's guard and for two callers: the rebooting soak ungates I2C5 and releases its reset
    // just before SBI SRST, because radon's OpenSBI resets the board with an I2C write to the PMIC;
    // and the Ethernet bring-up (6d) ungates gmac0's transmit and PTP clocks there.
    if let Some(sys) = memory::jh7110_sys_window() {
        direct_map(m, sys.base, sys.base + sys.size, Flags::device())?;
    }

    // 6c-prime. Option B's two windows (milestone 592, 2026-10-10): the SYS syscon, where the PLL
    // words the I2C input-clock computation reads live, and the I2C controller the PMIC's bus node
    // names. Guarded on the PMIC plan itself, so neither is mapped on any machine CI boots.
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

    // 6d. The JH7110's first Ethernet port (milestone 53 (the board's own peripherals: network and
    // storage on real silicon)), device memory: the controller's 64 KiB, the AON clock-and-reset
    // window that holds its bus clocks and resets, and the AON syscon page that holds its
    // interface select. The SYS window it also needs is 6c's, now mapped for either user. Present
    // only when the tree names the port, so never on a machine CI boots.
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

    // 7. The `sifive_test` finisher (0x10_0000), device memory: the MMIO word the test harness writes
    // to exit QEMU (arch::semihosting::exit). One page. Only QEMU `virt` has this device; the
    // VisionFive 2 has nothing at 0x10_0000, so mapping it there is a mapping to a nonexistent
    // address. Under the `board` feature the finisher exit is replaced by a UART marker + SBI SRST,
    // and this page is not mapped. The boot tour halts with `wfi` and never touches it in any build.
    #[cfg(not(feature = "board"))]
    direct_map(m, 0x10_0000, 0x10_1000, Flags::device())?;

    // 8. The virtio-mmio transport window (milestone 9 / parity C), device memory. The kernel probes
    // these slots for a block device (virtio::find_block_device) and owns the transport; the DMA
    // rings live in the driver's own region (notes/dma.md). Absent hardware here just reads as "no
    // device", so mapping it is harmless when no disk is attached.
    //
    //    **Only when the device tree names the bus** (milestone 89 (Scaleway EM-RV1)). This window
    //    was mapped unconditionally from QEMU's constants, the same class as the PCI windows below:
    //    on the T-Head TH1520 `0x1000_1000` is DRAM, already in the direct map from step 1, and the
    //    mapper's overwrite refusal would end the boot here exactly as the PCI window ended radon's
    //    first. [`virtio_slots`] reads the answer so the probe never reads an unmapped window.
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

    // 9. The PCIe windows (the PCIe transport): bus 0's ECAM config space, and the slice of the
    // 32-bit PCI memory window the kernel assigns BARs from, both straight from the device tree
    // (memory::init read the `pci-host-ecam-generic` node; no node, no mapping). Device memory
    // both. An absent *device* reads all-ones in ECAM ("nobody home"), so mapping the windows is
    // harmless without a PCI device; mapping them unconditionally from QEMU's constants was not,
    // and it is what stopped the first VisionFive 2 boot: the JH7110 states no such node, and
    // QEMU's BAR window (0x4000_0000) is that board's DRAM base, already direct-mapped by step 1,
    // so the mapper's overwrite refusal killed the boot right here (DECISIONS §43,
    // notes/visionfive2.md).
    if let Some(((ecam, ecam_size), (bar, bar_size))) = memory::pci_regions() {
        direct_map(
            m,
            ecam,
            ecam + crate::pci::ecam_bytes().min(ecam_size),
            Flags::device(),
        )?;
        direct_map(m, bar, bar + PCI_BAR_MAPPED.min(bar_size), Flags::device())?;
    }

    // 10. The JH7110's SD/MMC controllers (milestone 53 (the board's own peripherals: network and
    // storage on real silicon)), device memory: the first page of each, which holds every
    // register the driver touches and the data FIFO (`designware_mobile_storage::regs::
    // WINDOW_USED`), not the whole 64 KiB window the tree names. Present only when
    // `memory::init` found a JH7110 and its tree describes the controller, so never on a machine
    // CI boots. Their clocks and resets are in the SYS window, which 6c maps under the same guard.
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

/// Map a range of *physical* addresses into the direct map at `pa | KERNEL_VA_BASE`.
///
/// **Memory in blocks, devices in pages** (milestone 161). RAM goes in the largest leaf that fits
/// (`paging::Mapper::map_span` puts a 2 MiB or 1 GiB leaf only where it lies wholly inside the
/// range, so exactly the same pages are mapped, in a fraction of the table frames). Device windows
/// stay in 4 KiB pages: they are a handful of pages each, and keeping them small is the same
/// choice the x86 port makes for its own reasons (`arch/x86_64/mmu.rs`'s BUGS on the MTRRs).
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
    // A device the tree places outside both nameable ranges would be mapped at an aliased VA.
    // Boot-time only: this runs while the tables are built, never on the IPC path.
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

/// Walk the tables in software and check the things that would kill us, before the hardware bets the
/// machine on them. The RISC-V counterpart of the aarch64 `verify`.
fn verify<A, P>(m: &Mapper<A, P, Format>)
where
    A: FnMut() -> Option<u64>,
    P: Fn(u64) -> *mut PageTable,
{
    // The code we are executing right now must be mapped executable, or the instruction after the
    // `csrw satp` never gets fetched.
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

    // The UART, so `println!` keeps working across the switch.
    assert!(
        m.translate(phys_to_virt(UART_BASE)).is_some(),
        "the UART is not mapped: the machine would go silent"
    );

    // The guard page must NOT be mapped, or the stack-overflow protection is silently off and we
    // would only find out during an overflow, which is when it is no use. The aarch64 `verify` has
    // always checked this; riscv reached the same layout (link-riscv64.ld reserves the page) without
    // ever asserting it.
    assert!(
        m.translate(stack_guard()).is_none(),
        "the guard page IS mapped: stack overflow protection is off"
    );

    // Each secondary hart's guard page, the same check per slot (milestone 90). The harts are not
    // up yet; this runs on the primary, on the map every hart will adopt in `init_secondary`.
    for id in 0..crate::cpu::MAX_CPUS {
        assert!(
            m.translate(crate::smp::secondary_stack_guard(id)).is_none(),
            "a secondary's guard page IS mapped: its stack overflow protection is off"
        );
    }

    // And every core's interrupt-stack guard page (milestone 124), on the same grounds: a stack
    // whose guard is mapped has silently lost its protection, and the next thing to find out would
    // be a handler running off the bottom into whatever the linker put below.
    for id in 0..crate::cpu::MAX_CPUS {
        assert!(
            m.translate(crate::interrupt_stack::guard(id)).is_none(),
            "an interrupt stack's guard page IS mapped: its overflow protection is off"
        );
    }
}

macro_rules! linker_symbol {
    ($name:ident, $sym:ident) => {
        // `pub`, matching the aarch64 twin: `stack::guard_page_at` asks both ISAs where the boot
        // stack's guard page is, and a portable caller cannot reach a private one. Parity (§19)
        // wants the two macros identical rather than one of them widened by one symbol.
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

/// Compose the `satp` value naming an address space: Sv39 mode, ASID, and the root PPN. The RISC-V
/// analog of aarch64's `ttbr0_value`, kept under that name so portable `user.rs` does not change.
/// **Returns a full `satp` value** (not a bare root), so `switch_user_root`/`activate_user` write it
/// directly; the `root` a process stores must itself contain the kernel high-half (see the note on
/// the single-`satp` model at `switch_user_root`).
pub fn ttbr0_value(root: u64, asid: u16) -> u64 {
    SATP_MODE_SV39 | ((asid as u64) << SATP_ASID_SHIFT) | (root >> 12)
}

/// Read the ASID back out of a composed [`ttbr0_value`]. The inverse of the line above, and it
/// exists so a portable test can ask "which tag is this space wearing?" without knowing that this
/// ISA keeps it in `satp[59:44]` and aarch64 keeps it in `TTBR0_EL1[63:48]`.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the tests are its only caller; the kernel composes, never decomposes
pub fn asid_of(satp: u64) -> u16 {
    ((satp >> SATP_ASID_SHIFT) & 0xffff) as u16
}

/// Discard every TLB entry tagged with `asid`, **on every online hart**. The teardown half of the
/// ASID contract (`crates/address_space_identifier`): after this, and only after this, the number may tag someone else.
/// The aarch64 twin of this function is one instruction, and the gap between them is milestone 58.
///
/// # What each half guarantees
///
/// `sfence.vma x0, asid` invalidates every address for one ASID **on the hart that executes it**,
/// and orders this hart's own earlier page-table writes ahead of it. It deliberately does *not*
/// touch global mappings, which is right: the kernel's high half is `G`, shared by every address
/// space, and must survive a process dying. Only user mappings wear a tag.
///
/// The remote half is an SBI RFENCE ([`sbi_remote_sfence_vma_asid`](super::sbi_remote_sfence_vma_asid)),
/// which returns only once every other online hart has run the same instruction. Without it, a hart
/// that ran a thread of the dying space keeps its translations, and the next space to be handed this
/// number reads them: **one process reading another's memory, with no fault to announce it.** That
/// is not a latent hazard, it is the hazard that the unconditional flush in [`write_satp`] was
/// covering up until this milestone; see notes/riscv-tlb-shootdown.md.
///
/// Skipped when this is the only hart online, exactly as [`flush_tlb`] skips it: there is nobody to
/// shoot down, and single-hart boot tears down address spaces before the secondaries exist.
pub fn flush_asid(asid: u16) {
    // Local first, so this hart's own page-table writes are ordered before anyone is told to look.
    instructions::sfence_vma_asid(asid as u64);

    let others = crate::smp::online_harts_mask() & !(1usize << crate::cpu::id());
    if others != 0 {
        super::sbi_remote_sfence_vma_asid(others, asid);
    }
}

/// **Test-only: let S-mode load and store through pages marked `U`** (`sstatus.SUM`), returning
/// whether it was already permitted so the caller can put it back.
///
/// **RISC-V forbids this by default and aarch64 permits it**, which is a parity difference nothing
/// in this tree had written down until milestone 58 went looking for why a ported test faulted.
/// S-mode reading a `U` page raises a load page fault unless `sstatus.SUM` is set; EL1 reading an
/// EL0 page is simply allowed, because this kernel does not enable `PAN`. So a test that reads
/// through a *user* virtual address to see what the TLB is holding, which is the only way to observe
/// a TLB from software, needs this on one ISA and nothing on the other.
///
/// It is `#[cfg(test)]` on purpose. No syscall in this ABI dereferences a user pointer (see
/// [`user_can_read`]), so the running kernel never needs to touch a user page, and leaving `SUM`
/// clear in a shipping build means a kernel bug that strays into the low half faults instead of
/// succeeding quietly. Making that reachable outside tests would trade a real protection for
/// nothing.
#[cfg(any(test, feature = "system_tests"))]
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the system tests call it; a unit-test boot on some ISAs does not
pub fn permit_kernel_access_to_user_pages(allowed: bool) -> bool {
    const SSTATUS_SUM: u64 = 1 << 18;
    // Widening what S-mode may touch is a permission change, not a memory-safety one; the kernel's
    // own mappings are unaffected.
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

/// The leaf the hardware would find for `va` on this hart, **in whichever half `va` names**.
///
/// [`translate_user`] cannot answer this and it is not a near miss: `translate_at` builds its
/// `Mapper` with `Half::Low`, always, so a high-half address comes back `None` before any leaf is
/// read. [`is_mapped_in_current_space`] already carried the fix in its own body and said why in its
/// doc comment ("a user thread reaching for the *kernel's* memory names a high-half address ...
/// `translate_user` alone would say 'not mapped' for it and turn the most interesting case into the
/// wrong answer"), and [`user_can_read`] went on calling `translate_user` anyway.
///
/// **Milestone 305 found that by falsification, which is the only way it could have been found.**
/// A patch that removed the `U` check from `user_can_read` entirely left
/// `the_page_tables_say_u_mode_cannot_read_the_kernels_memory` **green**, because the answer was
/// never coming from the `U` bit: the walk stopped at the half. The test's headline assertion was
/// vacuous and had been since milestone 41.
fn translate_in_either_half(va: u64) -> Option<(u64, Flags)> {
    let root = current_root_pa();
    // SAFETY: `root` is the live installed root; the direct map makes `phys_to_ptr` valid; a
    // translate allocates nothing, so the `|| None` allocator is never called.
    let half = |h| unsafe { Mapper::<_, _, Format>::new(root, h, || None, phys_to_ptr) };
    half(Half::Low)
        .translate(va)
        .or_else(|| half(Half::High).translate(va))
}

/// Whether U-mode may read `va` in the installed address space. RISC-V has no address-translation
/// instruction like aarch64's `AT S1E0R`, so we walk the current tables and check the U bit.
///
/// No syscall in the ABI dereferences a user pointer, so this has no caller in the running kernel;
/// see the aarch64 twin for the full disposition. It is proved rather than merely allowed, by
/// `the_page_tables_say_u_mode_cannot_read_the_kernels_memory` (milestone 41, which is when this
/// ISA got the confused-deputy test aarch64 had had all along).
///
/// **The walk is [`translate_in_either_half`] rather than [`translate_user`], and that is the whole
/// of milestone 305's correction here.** With `translate_user` this function answered "no" for
/// every kernel address by refusing to look, so the one assertion it exists for could not fail.
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
pub fn unmap_user_at(root: u64, va: u64) -> Option<u64> {
    // SAFETY: `root` is a live low-half-owning root; `unmap` allocates nothing; the direct map makes
    // `phys_to_ptr` valid.
    let mut mapper = unsafe { Mapper::<_, _, Format>::new(root, Half::Low, || None, phys_to_ptr) };
    let (pa, flush) = mapper.unmap(va).ok()?;
    flush.flush(flush_tlb);
    Some(pa)
}

/// **Cut the page table at `table` out of the walk that reaches `va` in the space rooted at
/// `root`**, and flush that space's tag. Returns the span of addresses that went with it, as
/// `(base, size)`, or `None` if the walk to `va` no longer passes through `table`. Name
/// provisional (the page-tables-outlive-destroy lane, 2026-10-05 UTC).
///
/// `revoke::revoke_region` calls this for each table a region paid for before the region's pages
/// go back: see `paging::Mapper::unlink_table` for why the cut is one entry. The flush is by ASID
/// rather than by page because the span is up to a whole table's reach and the walk caches above
/// the leaves have to go too, and a by-tag flush reaches both on every core.
pub fn cut_user_table(root: u64, va: u64, table: u64, asid: u16) -> Option<(u64, u64)> {
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

/// Map one user page at `va` onto the already-owned physical frame `phys` in the current address
/// space, drawing any intermediate tables from `alloc`, then flush the TLB for `va` (RISC-V may
/// require an `sfence.vma` to make a freshly-valid leaf visible, unlike aarch64).
pub fn map_current_user_page_frame(
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

/// The root a thread with no user address space runs on. On RISC-V the whole address space is one
/// `satp`, so "the reserved user root" is simply the kernel root: its low half is empty, so any user
/// address faults, which is exactly right for a kernel thread. (On aarch64 this is a separate empty
/// `TTBR0` table; the single-`satp` model folds it into the kernel root.)
pub fn reserved_root() -> u64 {
    ttbr0_value(KERNEL_ROOT.load(Ordering::Relaxed), 0)
}

/// Install the address space named by the composed `satp` value, **unless it is already installed**.
///
/// **This is the RISC-V single-`satp` model.** aarch64 has separate `TTBR0` (user) and `TTBR1`
/// (kernel), so switching a process swaps only `TTBR0` and leaves the kernel mapped. RISC-V has one
/// `satp` for the whole address space, so a process's root table must *itself* contain the kernel's
/// high-half entries (shared at address-space creation), and switching threads rewrites the whole
/// `satp`.
///
/// The early return matches aarch64's, and it earns more here than it does there. Every switch
/// between two kernel threads names [`reserved_root`], and until milestone 58 each of those wrote
/// `satp` and threw away the hart's whole TLB for a switch that changed nothing. The comparison
/// reads the register back, so it is against what the hardware is walking rather than against our
/// record of it; `satp` carries the ASID in the same word, so "same address space" is one compare.
///
/// # Safety
/// `satp` must be a value [`ttbr0_value`] composed over a **live** `AddressSpace`'s root, or
/// [`reserved_root`]. Same contract as [`write_satp`]; see the aarch64 twin for why the liveness
/// half of it cannot be carried by a type instead.
///
/// `#[inline(always)]`, with [`write_satp`] beneath it, because both are on the context switch's
/// hot path and must land inside `.text.hot` with their caller. LLVM had inlined them by choice
/// until milestone 139 (drive the unsafe count down) round 9 gave the riscv64 boot tour a call
/// through `AddressSpace::while_installed`. Then it outlined this function, and with only it
/// forced, it outlined `write_satp` and merged it with the identical `activate_user`. Both times
/// `script/fastpath-footprint` caught a hot symbol outside `.text.hot` (2026-10-08 UTC). Forcing
/// the two hot ones keeps the hot path independent of how many cold callers exist.
#[inline(always)]
pub unsafe fn switch_user_root(satp: u64) {
    if read_satp() == satp {
        return;
    }
    // SAFETY: this function's own `# Safety` contract is exactly the one this call needs; it
    // forwards, it does not weaken. `satp` is already a composed value (from `ttbr0_value` or
    // `reserved_root`), so it is written directly.
    unsafe { write_satp(satp) };
}

/// Serializes edits to the kernel's live tables: two harts must not mutate them at once. Same role
/// (and lock rank) as the aarch64 module's `KERNEL_MMU`.
static KERNEL_MMU: crate::sync::IrqSafeMutex<()> =
    crate::sync::IrqSafeMutex::new(crate::sync::rank::KERNEL_MMU, ());

/// The kernel's live page tables, as a `Mapper` rooted at the saved fine-table root. Reads
/// `KERNEL_ROOT` rather than `satp` back because both harts share one kernel root; **call only while
/// holding [`KERNEL_MMU`].**
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
pub fn map_page(va: u64, pa: u64, flags: Flags) -> Result<(), MapError> {
    let _guard = KERNEL_MMU.lock(); // exclusive: two harts must not mutate the tables at once
    kernel_mapper().map(va, pa, flags)
}

/// Remove one page from the kernel's address space, invalidate the TLB, and return the physical
/// frame (the caller's to free; the mapper never owned it). The `TlbFlush` obligation is discharged
/// here with a real `sfence.vma`; dropping one un-discharged panics.
pub fn unmap_page(va: u64) -> Result<u64, MapError> {
    let _guard = KERNEL_MMU.lock(); // exclusive: see map_page
    let (pa, flush) = kernel_mapper().unmap(va)?;
    flush.flush(flush_tlb);
    Ok(pa)
}

/// Invalidate the TLB entry for one virtual address. This is what discharges a `paging::TlbFlush`;
/// the `paging` crate is pure logic and emits no instructions.
///
/// `sfence.vma rs1, rs2` with `rs1` = the address and `rs2` = `x0` (all ASIDs) invalidates that
/// page's translation. Unlike aarch64's `tlbi`, `sfence.vma` also orders the preceding page-table
/// write and completes locally, so no separate barrier is needed.
pub fn flush_tlb(va: u64) {
    // Local first. Getting TLB maintenance wrong means a stale translation, which is the
    // memory-unsafety that matters here, not Rust unsafety.
    instructions::sfence_vma_page(va);

    // Then the other online harts (SMP shootdown). The kernel root is shared, so a page mapped or
    // unmapped here must be sfence'd on every hart that might run a thread touching it, or a migrated
    // thread faults on a translation this hart already invalidated but the others still cache. RISC-V
    // has no hardware TLB broadcast, so we IPI via SBI RFENCE. Skipped entirely until a second hart is
    // online (single-hart boot maps a great many pages; there is no one to shoot down).
    let others = crate::smp::online_harts_mask() & !(1usize << crate::cpu::id());
    if others != 0 {
        super::sbi_remote_sfence_vma(others, va as usize, PAGE_SIZE as usize);
    }
}

/// Whether paging is on: `satp`'s MODE field is not Bare (0). True from `boot.s`'s Sv39 switch on.
pub fn is_enabled() -> bool {
    read_satp() >> 60 != 0
}

/// Translate a kernel virtual address through the live kernel tables.
pub fn translate(va: u64) -> Option<(u64, Flags)> {
    let _guard = KERNEL_MMU.lock();
    kernel_mapper().translate(va)
}

/// Translate a user virtual address through the currently installed address space.
pub fn translate_user(va: u64) -> Option<(u64, Flags)> {
    translate_at(current_root_pa(), va)
}

/// Does `va` have a translation at all in the address space installed on this hart, in **either**
/// half?
///
/// This exists for the fault classifier, and it is the one question RISC-V's `scause` refuses to
/// answer: it says "load page fault" whether the leaf was absent or present-and-forbidden, so the
/// only way to tell a permission refusal from a missing mapping is to walk the tables the hardware
/// just walked. See `exceptions::user_fault` for the caveat that carries.
///
/// Both halves, because a user thread reaching for the *kernel's* memory names a high-half address,
/// and the process root carries the kernel half ([`share_kernel_half`]). [`translate_user`] alone
/// would say "not mapped" for it and turn the most interesting case into the wrong answer.
pub fn is_mapped_in_current_space(va: u64) -> bool {
    let root = current_root_pa();
    // SAFETY: `root` is the live installed root; the direct map makes `phys_to_ptr` valid; a
    // translate allocates nothing, so the `|| None` allocator is never called.
    let half = |h| unsafe { Mapper::<_, _, Format>::new(root, h, || None, phys_to_ptr) };
    half(Half::Low).translate(va).is_some() || half(Half::High).translate(va).is_some()
}

/// **Is this kernel address mapped, asked without taking a lock?**
///
/// For fault handlers, and the "without a lock" is the whole reason it exists rather than
/// `translate`: [`translate`] takes `KERNEL_MMU`, and a handler that has already lost the machine
/// may not block on a lock whose holder might be the thread that just died. aarch64 has the same
/// function for the same caller (`stack::print_text_words`), where it is a one-line wrapper because
/// `TTBR1_EL1` needs no lock at all; the asymmetry is RISC-V's single-root design, not a difference
/// in intent.
///
/// Walks the root installed on this hart, which carries the kernel half
/// ([`share_kernel_half`]), so a high-half address resolves whichever process is current.
///
/// Name: provisional (2026-08-17): calef has not ruled on it.
pub fn is_mapped(va: u64) -> bool {
    let root = current_root_pa();
    // SAFETY: `root` is the live installed root; the direct map makes `phys_to_ptr` valid; a
    // translate allocates nothing, so the `|| None` allocator is never called.
    let mapper = unsafe { Mapper::<_, _, Format>::new(root, Half::High, || None, phys_to_ptr) };
    mapper.translate(va).is_some()
}

/// Populate a fresh process root's **high half** with the kernel's, so a single `satp` pointing at
/// it sees both the process's user pages (low half) and the whole kernel (high half).
///
/// This is the RISC-V single-`satp` requirement with no aarch64 counterpart: aarch64 keeps the
/// kernel in a separate `TTBR1` that every process shares implicitly, but RISC-V has one root per
/// address space, so every process root must carry copies of the kernel root's top-level entries.
/// The kernel high half is the top 256 entries (index 256..512, `KERNEL_VA_BASE`'s top-level index
/// and up); they point at shared kernel intermediate tables, so copying the entries shares the whole
/// kernel map. Called by `user::AddressSpace` right after it allocates a root.
pub fn share_kernel_half(root: u64) {
    let kernel_root = KERNEL_ROOT.load(Ordering::Relaxed);
    // SAFETY: both are page-aligned root tables reachable through the direct map. We copy only the
    // high-half entries; the low half stays zero for the process's own user mappings.
    unsafe {
        let dst = &mut (*phys_to_ptr(root)).entries;
        let src = &(*phys_to_ptr(kernel_root)).entries;
        dst[256..paging::ENTRIES].copy_from_slice(&src[256..paging::ENTRIES]);
    }
}

/// **Print a human summary of the kernel's mapping**, for the machine description.
///
/// **It was `unimplemented!()` until milestone 268**, and nothing had ever called it: the RISC-V
/// arm of `main.rs` printed its own `paging` line inline instead, so this function was a
/// panic waiting for its first caller. The machine description calls the same name on all three
/// architectures (that is what "the same questions answered" means), so it found this the first
/// time it ran, as a `[PANIC]` in the middle of the block it had just printed.
///
/// The shape is aarch64's, said in this architecture's vocabulary: one root register rather than a
/// TTBR pair, so the line names `satp` and the root it holds rather than a split.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub fn print_summary() {
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
mod tests {
    //! Tests for the Sv39 MMU: the live page tables, W^X, the guard page, and TLB invalidation.
    //!
    //! These are the RISC-V twins of the aarch64 module's, written against the same *properties*
    //! rather than the same mechanisms (DECISIONS §19). Where the two ISAs differ, the difference is
    //! stated where it matters: one `satp` instead of the TTBR0/TTBR1 pair, three levels instead of
    //! four, a single `X` bit whose privilege is decided by `U` instead of the PXN/UXN pair, and a
    //! software RSW bit standing in for a memory type base Sv39 has no encoding for.
    //!
    //! `translate` walks the live kernel root, so these inspect the tables the hardware is
    //! **actually walking**, not a copy of what we intended.

    /// **A physical address certainly outside every RAM region this machine described**, for
    /// tests that need a spare page to map and unmap without colliding with the direct map.
    ///
    /// Two tests here used to hardcode `0x1_0000_0000` / `0x1_0100_0000` as "not RAM on QEMU
    /// virt", true when written and false on the VisionFive 2 (bench, 2026-08-21): 8 GiB of RAM
    /// starting at `0x4000_0000` reaches past `0x2_4000_0000`, so both addresses land inside the
    /// direct map and the very first `map_page` in each test failed with `AlreadyMapped` before
    /// the test's own logic ever ran. Reading the machine's own RAM regions and picking one gap
    /// above the top of the highest region is the fix that works on any machine's memory map,
    /// not just a wider guess.
    fn a_physical_address_outside_every_ram_region() -> u64 {
        let top = crate::memory::ram_regions()
            .map(|(start, size)| start + size)
            .max()
            .expect("a booted kernel has at least one RAM region");
        // One gigabyte clear of the top: page-aligned by construction (RAM regions are), and far
        // enough that a region reported with generous rounding still leaves room.
        top + (1 << 30)
    }

    /// **The trust flag agrees with the measured width, whatever that width is.**
    ///
    /// This used to assert `bits >= 8` outright, named for `crates/address_space_identifier`'s own justification:
    /// "below even the smallest hardware ASID space (8-bit, 256)", true of aarch64 (which
    /// mandates 8 bits) and **not guaranteed by RISC-V at all**, which permits zero. The
    /// VisionFive 2's U74 measures exactly zero implemented bits (bench, 2026-08-21: the boot
    /// summary reads `satp.ASID 0 bits measured`), which is not a hypothetical this test can
    /// treat as unreachable; it is the live case [`asid_tagging_is_trusted`] exists to handle.
    ///
    /// With `bits < ASID_BITS_NEEDED`, every address space would carry ASID 0 in hardware and
    /// their TLB entries would alias, which is one process reading another's memory, EXCEPT that
    /// [`asid_tagging_is_trusted`] being `false` keeps the unconditional `sfence.vma` in
    /// [`write_satp`] doing the flush a narrow machine still needs. So the invariant this test
    /// owes is not "the hardware is wide enough": it is "the kernel correctly knows whether the
    /// hardware is wide enough", which is checkable on any width, including zero.
    #[test_case]
    fn the_hardware_has_at_least_the_asid_bits_the_allocator_assumes() {
        let bits = super::asid_bits();
        assert!(
            bits <= super::SATP_ASID_WIDTH as usize,
            "satp.ASID reported {bits} implemented bits, wider than the architectural 16",
        );
        assert_eq!(
            super::asid_tagging_is_trusted(),
            bits >= super::ASID_BITS_NEEDED as usize,
            "satp.ASID implements {bits} bits, address_space_identifier::ASIDS needs {}: the trust flag must agree \
             with whether this width holds every tag the allocator can hand out. If it does not, \
             either the flush stays where it should have been dropped, or address spaces would \
             alias in the TLB with no flush catching it.",
            address_space_identifier::ASIDS,
        );
    }

    /// Paging is on, and we are alive to say so.
    ///
    /// Weaker than it looks on this ISA, and worth saying so: the kernel runs at `KERNEL_VA_BASE`,
    /// which does not exist unless Sv39 is on, so a machine that reached this line has paging. What
    /// the assertion adds is that [`is_enabled`](super::is_enabled) reads the right field: `satp`'s
    /// MODE is bits 63:60, and a helper that looked at the wrong bits would answer "paging is off"
    /// on a paging machine, which is the sort of quiet wrongness that only shows up in whatever
    /// decides to trust it.
    #[test_case]
    fn mmu_is_enabled() {
        assert!(crate::arch::mmu::is_enabled(), "satp.MODE reads as Bare");
    }

    /// The kernel is running in the Sv39 high half.
    ///
    /// The reason is the same as aarch64's, arrived at differently. There the kernel lives in
    /// `TTBR1`, which a process switch never touches. Here there is one `satp` per address space and
    /// every process root carries a **copy of the kernel's top-level entries** (`share_kernel_half`),
    /// so the kernel is reachable from every space. Either way the kernel must be in the half that
    /// is not the process's, or the first switch into userspace deletes the kernel.
    #[test_case]
    fn the_kernel_lives_in_the_high_half() {
        use crate::arch::mmu::KERNEL_VA_BASE;

        // Our own code.
        let pc = crate::kernel_main as *const () as u64;
        assert!(
            pc >= KERNEL_VA_BASE,
            "kernel .text is at {pc:#x}, not in the high half"
        );

        // Our stack.
        let sp = crate::arch::current_sp();
        assert!(
            sp >= KERNEL_VA_BASE,
            "the stack is at {sp:#x}, not in the high half"
        );

        // And a static.
        static IN_BSS: u64 = 0;
        let addr = (&raw const IN_BSS) as u64;
        assert!(
            addr >= KERNEL_VA_BASE,
            "a static is at {addr:#x}, not in the high half"
        );
    }

    /// **The low half is empty when no process is running**, and on RISC-V that is a stronger claim
    /// than on aarch64.
    ///
    /// aarch64 can point `TTBR0` at an empty reserved table, so "no user space installed" is a
    /// separate register. RISC-V has *one* `satp`: the kernel runs on the kernel root, and that root's
    /// own low half is what a low address resolves through. Nothing but discipline stops
    /// `map_everything` from leaving something down there, and if it did, a stray low pointer in the
    /// kernel would silently succeed instead of faulting, and a process would inherit the mapping
    /// through `share_kernel_half`'s copy path the moment the entry moved up a level.
    ///
    /// The addresses are all inside Sv39's low half (`va >> 38 == 0`). That is deliberate: the
    /// `Mapper` returns `None` for anything outside its half *before it walks anything*, so a test
    /// address above 2^38 would pass without a single page-table read and prove nothing at all.
    #[test_case]
    fn a_low_address_does_not_translate_when_no_process_is_running() {
        use crate::arch::mmu::translate_user;

        for va in [
            0x1000u64,
            0x8020_0000, // where OpenSBI loaded us: the identity map, if it survived
            0x0000_003f_ffff_f000, // the top page of the Sv39 low half
        ] {
            assert!(
                translate_user(va).is_none(),
                "{va:#x} translates through the live satp: the boot table's identity gigapages \
                 may still be live",
            );
        }
    }

    /// The direct map: every physical address is nameable at `pa + KERNEL_VA_BASE`.
    ///
    /// This is how the kernel touches a frame the allocator just handed it. Without it, a physical
    /// address the kernel cannot NAME is a physical address it cannot use.
    #[test_case]
    fn the_direct_map_reaches_physical_memory() {
        use crate::arch::mmu::{phys_to_virt, virt_to_phys};

        let frame = crate::memory::alloc().expect("out of memory");
        let va = phys_to_virt(frame.addr());

        assert_eq!(
            virt_to_phys(va),
            frame.addr(),
            "the transform is not reversible"
        );

        let (pa, flags) = crate::arch::mmu::translate(va).expect("frame is NOT in the direct map");
        assert_eq!(pa, frame.addr());
        assert!(flags.is_writable());

        // And it is real memory: write through the virtual name, read it back.
        // SAFETY: the allocator just gave us this frame exclusively.
        unsafe {
            core::ptr::write_volatile(va as *mut u64, 0xfeed_face_cafe_f00d);
            assert_eq!(
                core::ptr::read_volatile(va as *const u64),
                0xfeed_face_cafe_f00d
            );
        }

        crate::memory::free(frame);
    }

    /// **The guard page must not be mapped.** That is its entire job.
    ///
    /// link-riscv64.ld has reserved the page since the port landed, and `map_everything` skips it by
    /// mapping `.data..bss_end` and `stack_bottom..stack_top` as two ranges with a hole between
    /// them. Nothing asserted the hole was where it was supposed to be, so a linker-script edit that
    /// moved `__stack_guard` would have left the stack still mapped and the protection silently
    /// gone. `verify` now checks it at boot as well, as aarch64's always has.
    #[test_case]
    fn the_guard_page_is_a_hole() {
        use crate::arch::mmu;
        assert_eq!(
            mmu::translate(mmu::stack_guard()),
            None,
            "the guard page IS mapped: a stack overflow would silently eat .bss"
        );

        // And the pages either side of it must be mapped, or the hole is in the wrong place and is
        // protecting nothing.
        assert!(
            mmu::translate(mmu::stack_guard() - 4096).is_some(),
            "below the guard"
        );
        assert!(
            mmu::translate(mmu::stack_bottom()).is_some(),
            "the stack itself"
        );
    }

    /// W^X, checked against the tables the hardware is actually walking.
    ///
    /// **The `!is_user_executable` line is weaker here than on aarch64, and deliberately kept
    /// anyway.** aarch64 has two independent execute-never bits (PXN and UXN), so asserting both is
    /// two claims. Sv39 has one `X` bit whose privilege is decided by `U`, and `Sv39::leaf_flags`
    /// reports kernel-exec or user-exec accordingly, so given kernel-exec the user-exec assertion
    /// cannot fail. It stays because the *property* is what the kernel cares about and the format
    /// under it may change (Svpbmt, or a future format with separate bits); it is not carrying
    /// weight today.
    #[test_case]
    fn kernel_text_is_executable_and_not_writable() {
        use crate::arch::mmu;

        let (pa, flags) = mmu::translate(mmu::text_start()).expect(".text is not mapped");
        assert_eq!(
            pa,
            mmu::virt_to_phys(mmu::text_start()),
            ".text maps to the wrong frame"
        );

        assert!(flags.is_kernel_executable(), ".text is not executable");
        assert!(!flags.is_writable(), ".text is WRITABLE: W^X is broken");
        assert!(!flags.is_user_executable(), ".text is executable by U-mode");
    }

    /// Constants are read-only, and not executable by anyone.
    #[test_case]
    fn kernel_rodata_is_read_only_and_not_executable() {
        use crate::arch::mmu;

        let (_, flags) = mmu::translate(mmu::rodata_start()).expect(".rodata is not mapped");
        assert!(!flags.is_writable(), ".rodata is writable");
        assert!(!flags.is_kernel_executable(), ".rodata is executable");
    }

    /// The stack is writable and NOT executable.
    #[test_case]
    fn the_stack_is_writable_and_not_executable() {
        use crate::arch::mmu;

        let (_, flags) = mmu::translate(mmu::stack_bottom()).expect("stack is not mapped");
        assert!(flags.is_writable());
        assert!(
            !flags.is_kernel_executable(),
            "the stack is EXECUTABLE: data on the stack could be run as code"
        );
    }

    /// The UART is device-typed.
    ///
    /// **And the honest caveat, because this is the one place the two ISAs are not the same claim.**
    /// On aarch64 the device type is an architectural PTE field, and getting it wrong lets the CPU
    /// cache, reorder, merge and *speculatively read* MMIO; a speculative read of a UART FIFO
    /// register consumes the byte. Base Sv39 has no such field (that is the Svpbmt extension), so
    /// `paging::Sv39` carries the flag in an RSW software bit and QEMU's `virt` derives the real
    /// memory type from the physical address instead. So this asserts the kernel's *bookkeeping* is
    /// right, not that the hardware was told. It still fails for the mistake worth catching (mapping
    /// the UART with `Flags::kernel_data()`), and on a board with Svpbmt the same flag is what would
    /// drive the architectural bits. See notes/page-tables.md and crates/paging/src/sv39.rs.
    #[test_case]
    fn the_uart_is_mapped_as_device_memory() {
        use crate::arch::mmu;

        // The UART lives in the direct map, like every other physical address the kernel names: its
        // raw physical address stopped existing when the boot table's identity gigapages went away.
        let (_, flags) =
            mmu::translate(mmu::phys_to_virt(super::UART_BASE)).expect("the UART is not mapped");

        assert!(flags.is_device(), "the UART is not device memory");
        assert!(flags.is_writable(), "we do need to write to it");
        assert!(!flags.is_kernel_executable());
    }

    /// A frame from the allocator is still real, writable memory *through the MMU*.
    ///
    /// Proves the direct map covers everything the allocator can hand out. With paging on, a
    /// physical address the kernel cannot name is a physical address it cannot use, and the riscv
    /// direct map is built from `memory::ram_regions()` with the kernel image punched out of it,
    /// which is exactly the kind of arithmetic that can leave a hole nobody notices.
    #[test_case]
    fn an_allocated_frame_is_reachable_through_the_mmu() {
        use crate::arch::mmu;

        let frame = crate::memory::alloc().expect("out of memory");
        let va = mmu::phys_to_virt(frame.addr());
        let (pa, flags) = mmu::translate(va).expect("allocated frame is NOT MAPPED");

        assert_eq!(pa, frame.addr());
        assert!(flags.is_writable());
        assert!(!flags.is_kernel_executable(), "RAM is executable");

        crate::memory::free(frame);
    }

    /// **Prove the TLB is actually invalidated on unmap.**
    ///
    /// The landmine, and it is the same landmine on both ISAs even though the instruction differs
    /// (`sfence.vma` here, `tlbi` there). Change a mapping without discharging the flush and the CPU
    /// keeps using the *cached* translation: memory reads back as the previous owner's data. It is a
    /// security hole and it is close to undebuggable, because the page tables **in memory are
    /// correct**; the lie lives in a CPU structure you cannot inspect.
    ///
    /// So we make it observable:
    ///
    ///   1. map a spare VA to frame A, which holds 0xAAAA...
    ///   2. **read it**, which is what populates the TLB
    ///   3. unmap, and invalidate
    ///   4. map the *same VA* to frame B, which holds 0xBBBB...
    ///   5. read it again
    ///
    /// If step 5 returns 0xAAAA, the TLB is stale and we have exactly the bug. It must return
    /// 0xBBBB.
    #[test_case]
    fn unmap_invalidates_the_tlb() {
        use paging::Flags;

        use crate::arch::mmu::{self, phys_to_virt};

        const PATTERN_A: u64 = 0xaaaa_aaaa_aaaa_aaaa;
        const PATTERN_B: u64 = 0xbbbb_bbbb_bbbb_bbbb;

        // A high-half address well clear of everything the kernel maps: outside every RAM region
        // this machine described, and above every device window on both QEMU virt and the
        // VisionFive 2. Sv39's high half is 256 GiB, so it is a nameable address.
        let test_va = mmu::phys_to_virt(a_physical_address_outside_every_ram_region());
        assert_eq!(
            mmu::translate(test_va),
            None,
            "test address is already in use"
        );

        let a = crate::memory::alloc().expect("out of memory");
        let b = crate::memory::alloc().expect("out of memory");

        // SAFETY: two frames the allocator just gave us exclusively, reached via the direct map.
        unsafe {
            core::ptr::write_volatile(phys_to_virt(a.addr()) as *mut u64, PATTERN_A);
            core::ptr::write_volatile(phys_to_virt(b.addr()) as *mut u64, PATTERN_B);
        }

        mmu::map_page(test_va, a.addr(), Flags::kernel_data()).expect("map A");

        // SAFETY: just mapped, writable.
        let seen = unsafe { core::ptr::read_volatile(test_va as *const u64) };
        assert_eq!(seen, PATTERN_A, "the mapping didn't take");
        // ^ that read is the point: it pulls the translation into the TLB.

        let returned = mmu::unmap_page(test_va).expect("unmap");
        assert_eq!(returned, a.addr(), "unmap returned the wrong frame");

        mmu::map_page(test_va, b.addr(), Flags::kernel_data()).expect("map B");

        // SAFETY: mapped again, to a different frame.
        let seen = unsafe { core::ptr::read_volatile(test_va as *const u64) };

        assert_eq!(
            seen, PATTERN_B,
            "STALE TLB: the same virtual address still reads the OLD frame's data. \
             This is the bug that reads back another process's memory."
        );

        mmu::unmap_page(test_va).expect("cleanup");
        crate::memory::free(a);
        crate::memory::free(b);
    }

    /// Changing a mapping is forced through break-before-make.
    ///
    /// aarch64 needs this because a valid -> valid change can raise a TLB conflict abort. RISC-V is
    /// more forgiving about the hardware consequence, but the API is the same on both because the
    /// *software* hazard is the same: overwrite a leaf in place and the old frame is leaked with no
    /// record that it ever existed. The mapper makes it unrepresentable rather than merely unwise.
    #[test_case]
    fn the_kernel_mapper_refuses_to_overwrite() {
        use paging::{Flags, MapError};

        use crate::arch::mmu;

        let va = mmu::phys_to_virt(a_physical_address_outside_every_ram_region());
        let f = crate::memory::alloc().unwrap();

        mmu::map_page(va, f.addr(), Flags::kernel_data()).unwrap();

        assert_eq!(
            mmu::map_page(va, f.addr(), Flags::kernel_data()),
            Err(MapError::AlreadyMapped)
        );

        mmu::unmap_page(va).unwrap();
        crate::memory::free(f);
    }

    /// **`satp` carries the address space's ASID, in the field the hardware reads.**
    ///
    /// This is as much of aarch64's `asid_tagging_keeps_address_spaces_apart_without_flushes` as is
    /// true on RISC-V today, and the gap is worth naming rather than papering over.
    ///
    /// That test proves two things: distinct spaces get distinct tags, *and* switching between them
    /// flushes nothing, so their TLB entries coexist. The second half cannot be proved here, because
    /// `write_satp` follows every `csrw satp` with a bare `sfence.vma`, which discards the whole TLB
    /// on every user switch. So the ASID is written and then immediately made irrelevant: an
    /// isolation test would pass with the tagging removed entirely, which makes it a test that
    /// cannot fail for its stated reason, and we do not ship those.
    ///
    /// What is left is real and untested until now: `ttbr0_value` must place the ASID at bits 59:44,
    /// where `satp` keeps it. It sits directly above the PPN, so a wrong shift either corrupts the
    /// root pointer (loud) or drops the tag on the floor (silent, and about to matter). Dropping the
    /// unconditional `sfence` is a change to the switching model, not a test fix; see
    /// notes/riscv-arch-tests.md.
    #[test_case]
    fn the_satp_carries_the_address_spaces_asid() {
        use crate::user::AddressSpace;

        let a = AddressSpace::new(2).expect("no space A");
        let b = AddressSpace::new(2).expect("no space B");

        // satp.ASID is bits 59:44 (16 bits) on rv64 Sv39.
        let asid_of = |satp: u64| (satp >> 44) & 0xffff;
        let (asid_a, asid_b) = (asid_of(a.ttbr0()), asid_of(b.ttbr0()));

        assert_ne!(asid_a, asid_b, "two live spaces share an ASID");
        assert_ne!(asid_a, 0, "a user space got the kernel's ASID 0");
        assert_ne!(asid_b, 0, "a user space got the kernel's ASID 0");

        // And the tag did not eat the rest of the word. `satp` packs MODE (63:60), ASID (59:44) and
        // PPN (43:0) with no slack between them, so a shift that is off by four in either direction
        // lands the ASID in the mode field or in the root pointer. Both other fields must still be
        // what they were: Sv39 mode, and two distinct root tables.
        for satp in [a.ttbr0(), b.ttbr0()] {
            assert_eq!(satp >> 60, 8, "satp.MODE is not Sv39: {satp:#x}");
        }
        assert_ne!(
            a.ttbr0() & super::SATP_PPN_MASK,
            b.ttbr0() & super::SATP_PPN_MASK,
            "two address spaces share a root table",
        );
    }
}

/// Proofs of the logic around `satp`, reachable since 2026-09-25 because the instructions it calls
/// are one-instruction functions in [`instructions`] that a harness can stub.
///
/// # What the stubs assume, which is what these proofs are conditional on
///
/// Each harness replaces `csrr satp`, `csrw satp` and `sfence.vma` with the model below. **The model
/// is a claim about the hardware, written from the privileged specification and not checked against
/// any silicon:**
///
/// - `satp` holds what was last written, except that its ASID field is WARL: bits the hart does not
///   implement read as zero (`IMPLEMENTED_ASID`, chosen by the harness). MODE and PPN are modelled as
///   fully implemented, which is true of Sv39 on every hart this kernel boots on and is not something
///   a hart is required to do.
/// - `sfence.vma` is counted and has no other effect; the model has no TLB. What is proved about a
///   sweep is **whether it is issued**, never what it does.
/// - Nothing else runs: no interrupt, no other hart.
///
/// What would falsify the model rather than the code: a hart whose `satp` write is not visible to the
/// next `csrr`, or whose WARL behaviour depends on anything but the bit written. The QEMU and board
/// suites (`the_hardware_has_at_least_the_asid_bits_the_allocator_assumes`, and the TLB tests of
/// milestone 58 (RISC-V TLB shootdown)) are what test the model; these harnesses only test the code
/// against it. See notes/kernel-proofs/stubbing-an-instruction.md.
#[cfg(kani)]
mod proofs {
    use core::sync::atomic::Ordering::Relaxed;
    use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};

    use super::*;

    /// The modelled `satp`.
    static SATP: AtomicU64 = AtomicU64::new(0);
    /// Which of the 16 ASID bits the modelled hart implements, as a mask in ASID position 0.
    static IMPLEMENTED_ASID: AtomicU64 = AtomicU64::new(0);
    /// How many `csrw satp` the code under proof executed.
    static WRITES: AtomicU32 = AtomicU32::new(0);
    /// How many full `sfence.vma` sweeps it executed.
    static SWEEPS: AtomicU32 = AtomicU32::new(0);
    /// Whether a `csrw satp` has happened that no sweep has followed yet.
    static UNSWEPT: AtomicBool = AtomicBool::new(false);

    /// `satp[59:44]`.
    const ASID_FIELD: u64 = 0xffff << SATP_ASID_SHIFT;

    /// Model of `csrr satp`.
    #[allow(dead_code)] // named only by `#[kani::stub]`, which rustc cannot see
    fn read_satp_model() -> u64 {
        SATP.load(Relaxed)
    }

    /// Model of `csrw satp`: WARL on the ASID field, exact everywhere else.
    ///
    /// # Safety
    /// None of its own: `unsafe` only so its signature matches the `unsafe fn` it stands in for.
    /// It touches nothing but the model's statics.
    #[allow(dead_code)] // named only by `#[kani::stub]`, which rustc cannot see
    unsafe fn write_satp_model(satp: u64) {
        let kept = IMPLEMENTED_ASID.load(Relaxed) << SATP_ASID_SHIFT;
        SATP.store((satp & !ASID_FIELD) | (satp & kept), Relaxed);
        WRITES.fetch_add(1, Relaxed);
        UNSWEPT.store(true, Relaxed);
    }

    /// Model of `sfence.vma` with no operands: counted, nothing else.
    #[allow(dead_code)] // named only by `#[kani::stub]`, which rustc cannot see
    fn sfence_vma_all_model() {
        SWEEPS.fetch_add(1, Relaxed);
        UNSWEPT.store(false, Relaxed);
    }

    /// A physical address Sv39 can name as a table root: a page frame below 2^56.
    fn any_root() -> u64 {
        let root: u64 = kani::any();
        kani::assume(root % PAGE_SIZE == 0);
        kani::assume(root < 1 << 56);
        root
    }

    /// **The root the MMU is walking is the root that was installed, and so is the tag.**
    ///
    /// `current_root_pa` is what `translate_user`, `is_mapped_in_current_space` and the fault
    /// classifier all start their walk from, so a mask or shift wrong here sends every one of them
    /// down somebody else's tables. It was unreachable by any harness until `read_satp` became a
    /// stubbable function: the `csrr` sat in the same call graph. Stated as a round trip through
    /// [`ttbr0_value`], the composer every address space is installed with, so it is not the
    /// formula restated: a composer and a decoder that disagree about a field fail it.
    ///
    /// Falsification: attested 2026-09-25. On patagonia with the patched Kani (pull request #1287, not yet merged).
    /// The shift in `current_root_pa` changed from 12 to 10: this goes red and its two siblings stay
    /// green. `attested` rather than `replayable` for the reason `script/falsifications` gives for
    /// every architecture-specific harness: the sweep compiles for its own host, which never
    /// compiles this file.
    #[kani::proof]
    #[kani::stub(super::super::instructions::read_satp, read_satp_model)]
    fn the_live_root_is_the_root_that_was_installed() {
        let root = any_root();
        let asid: u16 = kani::any();
        SATP.store(ttbr0_value(root, asid), Relaxed);

        assert!(
            current_root_pa() == root,
            "the walk starts at a different root"
        );
        assert!(
            asid_of(read_satp()) == asid,
            "the live tag is not the one installed"
        );
        assert!(is_enabled(), "a composed satp reads as Bare");
    }

    /// **The ASID probe reports exactly the bits the hardware kept, and leaves `satp` as it found
    /// it, swept.**
    ///
    /// The probe is what milestone 58's skipped TLB flush is bought with: on a hart that implements
    /// too few ASID bits, [`asid_tagging_is_trusted`] must say no, and the only input it has is this
    /// count. Overcounting is the dangerous direction, since two address spaces would share a tag
    /// with no flush between them. It was one `asm!` block until 2026-09-25, so none of this was
    /// provable; every implemented-bit pattern is covered here, including the zero-bit hart the
    /// specification permits and no machine this tree has booted on has.
    ///
    /// Falsification: attested 2026-09-25. On patagonia with the patched Kani, twice and one claim
    /// at a time: the readback shifted by `SATP_ASID_SHIFT - 1` (a miscount), and the restoring
    /// `csrw satp` and its sweep deleted. Each turns this red and leaves both siblings green.
    /// `attested` for the same reason as the harness above.
    #[kani::proof]
    #[kani::stub(super::super::instructions::read_satp, read_satp_model)]
    #[kani::stub(super::super::instructions::write_satp, write_satp_model)]
    #[kani::stub(super::super::instructions::sfence_vma_all, sfence_vma_all_model)]
    fn the_asid_probe_counts_the_implemented_bits_and_puts_satp_back() {
        let implemented = u64::from(kani::any::<u16>());
        IMPLEMENTED_ASID.store(implemented, Relaxed);
        // A live `satp` can only hold ASID bits the hart implements; the hardware put it there.
        let live: u64 = kani::any();
        kani::assume(live & ASID_FIELD & !(implemented << SATP_ASID_SHIFT) == 0);
        SATP.store(live, Relaxed);

        let counted = probe_asid_bits();

        assert!(
            counted == implemented.count_ones() as usize,
            "the probe miscounted"
        );
        assert!(
            SATP.load(Relaxed) == live,
            "the probe did not put satp back"
        );
        assert!(
            WRITES.load(Relaxed) == 2,
            "the probe wrote satp other than twice"
        );
        assert!(
            !UNSWEPT.load(Relaxed),
            "the probe's last satp write was never swept"
        );
    }

    /// **A switch to the kernel-only root sweeps the TLB exactly when the ASID cannot be trusted,
    /// and writes nothing when that root is already live.**
    ///
    /// This is milestone 58's safety argument as one statement: the unconditional flush on every
    /// `satp` write was removed, and what keeps address spaces apart without it is that the flush
    /// comes back on any hart whose tag is too narrow (and before the probe has run, when the flag
    /// is still false). Reached through [`deactivate_user`], the safe entry every switch to a
    /// kernel thread takes, so it covers [`switch_user_root`]'s early return and [`write_satp`]'s
    /// condition together. Before 2026-09-25 both conditions sat beside a `csrw` and a
    /// `sfence.vma` in the same function and no harness could reach either.
    ///
    /// Falsification: attested 2026-09-25. On patagonia with the patched Kani, twice: `write_satp`'s
    /// condition inverted (sweep only when trusted), and `switch_user_root`'s early return deleted.
    /// Each turns this red and leaves both siblings green. `attested` as above.
    #[kani::proof]
    #[kani::stub(super::super::instructions::read_satp, read_satp_model)]
    #[kani::stub(super::super::instructions::write_satp, write_satp_model)]
    #[kani::stub(super::super::instructions::sfence_vma_all, sfence_vma_all_model)]
    fn a_switch_sweeps_the_tlb_exactly_when_the_asid_cannot_be_trusted() {
        KERNEL_ROOT.store(any_root(), Relaxed);
        let trusted: bool = kani::any();
        ASID_TAGGING_TRUSTED.store(trusted, Relaxed);
        IMPLEMENTED_ASID.store(0xffff, Relaxed);
        let live: u64 = kani::any();
        SATP.store(live, Relaxed);

        deactivate_user();

        let target = reserved_root();
        assert!(
            SATP.load(Relaxed) == target,
            "the kernel-only root is not installed"
        );
        if live == target {
            assert!(
                WRITES.load(Relaxed) == 0,
                "rewrote a satp that was already live"
            );
            assert!(
                SWEEPS.load(Relaxed) == 0,
                "swept for a switch that changed nothing"
            );
        } else {
            assert!(
                WRITES.load(Relaxed) == 1,
                "installed the root other than once"
            );
            assert!(
                UNSWEPT.load(Relaxed) == trusted,
                "swept a trusted switch, or left an untrusted one unswept"
            );
        }
    }
}
