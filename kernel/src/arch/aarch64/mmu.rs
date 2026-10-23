//! The kernel's page tables, the user address-space switch, and TLB maintenance.
//!
//! The instruction after a table switch is fetched through the new tables, so a map that misses
//! the running code stops the machine without a message; `verify` walks the tables first.
//! `cargo xtask gdb` is the tool for this moment. See notes/mmu.md, "What
//! `kernel/src/arch/aarch64/mmu.rs` holds to", notes/page-tables.md and notes/higher-half.md.

use core::ffi::c_void;
use core::sync::atomic::{AtomicU64, Ordering};

use aarch64_cpu::asm::barrier;
use aarch64_cpu::registers::{MAIR_EL1, SCTLR_EL1, TCR_EL1, TTBR0_EL1, TTBR1_EL1};
use paging::aarch64::mair;
use paging::{Aarch64, Flags, Half, MapError, Mapper, PAGE_SIZE, PageSize, PageTable};

use crate::{memory, println};

/// This architecture's page-table format. Portable code that must name the format (the user-VA gate
/// in syscall.rs, the user `Mapper` in user.rs) refers to it as `arch::mmu::Format`, so the choice
/// of format lives in `arch/`, not in the portable kernel. See notes/riscv-port.md (leak #2).
pub type Format = Aarch64;
use tock_registers::interfaces::{Readable, Writeable};

/// Where the kernel lives, virtually.
///
/// Only bits 63:48, which are not translated: `VA = PA | KERNEL_VA_BASE` is exact and reversible,
/// and a kernel address has its physical address's table indices, so boot.s can point both TTBRs
/// at one table. See notes/higher-half.md.
pub const KERNEL_VA_BASE: u64 = 0xffff_0000_0000_0000;

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

/// A physical address, as the kernel names it.
///
/// The direct map: every byte of physical memory at `pa | KERNEL_VA_BASE`, permanently.
pub const fn phys_to_virt(pa: u64) -> u64 {
    pa | KERNEL_VA_BASE
}

pub const fn virt_to_phys(va: u64) -> u64 {
    va & !KERNEL_VA_BASE
}

/// The PL011. Mapped as **device** memory, and that word is load-bearing.
///
/// As normal memory the CPU may speculatively read it, and reading a FIFO register consumes the
/// byte. See notes/page-tables.md.
const UART_BASE: u64 = 0x0900_0000;
const UART_SIZE: u64 = 0x1000;

/// How the kernel turns a physical address into something it can dereference.
///
/// The direct map, valid from the first instruction of Rust: boot.s established it and the fine
/// tables preserve it.
pub fn phys_to_ptr(pa: u64) -> *mut PageTable {
    phys_to_virt(pa) as *mut PageTable
}

/// Build the kernel's page tables and turn the MMU on.
///
/// Replaces boot.s's coarse map with the fine one in TTBR1, verified first, and points TTBR0 at
/// the empty reserved table. Called already running at high virtual addresses.
pub fn init() {
    let root = memory::alloc()
        .expect("no frame for the root page table")
        .addr();

    // SAFETY: a fresh frame. Zero it before the hardware can ever walk it: a page table
    // full of whatever was in RAM is a set of pointers to nowhere, followed at speed.
    unsafe {
        (*phys_to_ptr(root)).entries = [0; paging::ENTRIES];
    }

    // SAFETY: `root` is zeroed and page-aligned. `phys_to_ptr` is the identity, which is
    // correct while the MMU is off and stays correct afterwards because the map we are
    // about to build is an identity map.
    // Half::High: these tables go in TTBR1_EL1. The mapper refuses a low address, which is
    // the check that would have caught a whole class of ghost bugs.
    let mut mapper = unsafe {
        Mapper::<_, _, Aarch64>::new(
            root,
            Half::High,
            || memory::alloc().map(|f| f.addr()),
            phys_to_ptr,
        )
    };

    map_everything(&mut mapper).expect("failed to build the kernel page tables");

    // The last chance to find a wrong map while we can still print.
    verify(&mapper);

    // SAFETY: the map covers this function's code, its stack, and the UART. We checked.
    unsafe { install(root) };

    // For `init_secondary`: the coarse boot map does not reach the thread-stack area.
    KERNEL_ROOT.store(root, Ordering::Relaxed);

    // And give TTBR0 an empty table to walk, so a stray low address faults.
    install_reserved_ttbr0();
}

/// The kernel's fine-map root (TTBR1), saved by [`init`] so secondaries can adopt it.
static KERNEL_ROOT: AtomicU64 = AtomicU64::new(0);

/// Bring a secondary core onto the kernel's own tables: the shared fine TTBR1 map, and the shared
/// empty TTBR0 so a stray low address faults. The boot core must have run [`init`] first.
///
/// A secondary starts on the coarse boot map, which does not reach the thread-stack area.
pub(crate) fn init_secondary() {
    let root = KERNEL_ROOT.load(Ordering::Relaxed);
    debug_assert!(root != 0, "init_secondary before init");

    // Not `install`: this core's TCR and MAIR already match the fine map, so only TTBR1 moves.
    //
    // SAFETY: the fine map covers this core's current code (kernel `.text`), its boot stack (in
    // `.bss`), and the UART. The `isb` after the TTBR write makes it effective before the next
    // fetch; the local `tlbi vmalle1` drops the coarse translations this core cached.
    unsafe {
        TTBR1_EL1.set_baddr(root);
        core::arch::asm!(
            "dsb ish",
            "isb",
            "tlbi vmalle1",
            "dsb ish",
            "isb",
            options(nostack),
        );
    }

    // TTBR0 still holds `secondary_boot`'s identity map; a low address must fault instead.
    // SAFETY: RESERVED_TTBR0 is a zeroed L0 table the boot core allocated in `install_reserved_ttbr0`.
    unsafe { set_ttbr0(ttbr0_value(RESERVED_TTBR0.load(Ordering::Relaxed), 0)) };
}

/// Identity-map everything the kernel needs, each region with the tightest permissions that
/// still let it work.
fn map_everything<A, P>(m: &mut Mapper<A, P, Aarch64>) -> Result<(), MapError>
where
    A: FnMut() -> Option<u64>,
    P: Fn(u64) -> *mut PageTable,
{
    // 1. The direct map, never executable, skipping the kernel image. The mapper refuses to
    //    overwrite, so an ordering mistake is an error rather than a wrong permission.
    let image_lo = virt_to_phys(image_start());
    let image_hi = virt_to_phys(image_end());

    for (start, size) in memory::ram_regions() {
        let end = start + size;
        direct_map(m, start, image_lo.min(end), Flags::kernel_data())?;
        direct_map(m, image_hi.max(start), end, Flags::kernel_data())?;
    }

    // 2. The kernel image, section by section, at its LINKED virtual addresses. This is W^X.
    map_range(m, text_start(), text_end(), Flags::kernel_code())?;
    map_range(m, rodata_start(), rodata_end(), Flags::kernel_rodata())?;
    map_range(m, data_start(), bss_end(), Flags::kernel_data())?;

    // 3. THE GUARD PAGE IS NOT MAPPED. That is its entire job.
    // See notes/stack.md.

    // 4. The stack.
    map_range(m, stack_bottom(), stack_top(), Flags::kernel_data())?;

    // 4b. The secondary stacks, slot by slot so each slot's bottom page stays a guard. They are
    // outside `.bss` because step 2 maps that in one range. See notes/stack-high-water.md.
    for id in 0..crate::cpu::MAX_CPUS {
        let (bottom, top) = crate::smp::secondary_stack_span(id);
        map_range(m, bottom, top, Flags::kernel_data())?;
    }

    // 4c. The interrupt stacks, slot by slot as in 4b. See kernel/src/interrupt_stack.rs.
    for id in 0..crate::cpu::MAX_CPUS {
        let (bottom, top) = crate::interrupt_stack::span(id);
        map_range(m, bottom, top, Flags::kernel_data())?;
    }

    // 5. The UART, as device memory.
    direct_map(m, UART_BASE, UART_BASE + UART_SIZE, Flags::device())?;

    // 6. The GIC's two blocks from the device tree, at full length: a GICv3's redistributor
    // frames say which core owns each.
    if let Some(found) = memory::gic_regions() {
        let (gicd, second) = (found.distributor(), found.second_region());
        direct_map(m, gicd.start, gicd.start + gicd.size, Flags::device())?;
        direct_map(m, second.start, second.start + second.size, Flags::device())?;
    }

    // 7. The virtio-mmio window, mapped only to enumerate it; a driver gets its own slot. See
    // kernel/src/virtio.rs.
    direct_map(
        m,
        VIRTIO_MMIO_BASE,
        VIRTIO_MMIO_BASE + VIRTIO_MMIO_SIZE,
        Flags::device(),
    )?;

    // 8. Bus 0's ECAM and the BAR slice of the PCI window (§18 (the PCIe transport)), from the
    // device tree's `pci-host-ecam-generic` node: no node, no mapping (§19 (architectural parity)).
    if let Some(((ecam, ecam_size), (bar, bar_size))) = memory::pci_regions() {
        direct_map(
            m,
            ecam,
            ecam + crate::pci::ecam_bytes().min(ecam_size),
            Flags::device(),
        )?;
        direct_map(m, bar, bar + PCI_BAR_MAPPED.min(bar_size), Flags::device())?;
    }

    // 9. The SMMUv3's registers, only when the device tree has the node.
    if let Some((smmu, smmu_size)) = memory::smmu_region() {
        direct_map(m, smmu, smmu + smmu_size, Flags::device())?;
    }

    Ok(())
}

/// The virtio-mmio device window on QEMU `virt`: 32 slots of 0x200 bytes at 0x0a000000, with
/// interrupts SPI 16..47 (INTID 48..79). Fixed by the machine, like [`UART_BASE`].
pub const VIRTIO_MMIO_BASE: u64 = 0x0a00_0000;
pub(crate) const VIRTIO_MMIO_SIZE: u64 = 32 * 0x200;
/// SPI 16 becomes INTID 48 (SPIs start at 32). Slot `i` uses INTID `VIRTIO_IRQ_BASE + i`.
pub(crate) const VIRTIO_IRQ_BASE: u32 = 48;
/// aarch64's `virt` lays out 32 virtio-mmio slots 0x200 apart (RISC-V's are 8, 0x1000 apart). The
/// probe (`virtio::find_block_device`) walks them.
pub(crate) const VIRTIO_SLOT_STRIDE: u64 = 0x200;
pub(crate) const VIRTIO_SLOTS: u64 = 32;
/// How many slots the probe walks. All of them: aarch64 `virt` is the only machine this
/// architecture maps the window on. A function because riscv64's answer depends on the device tree.
pub(crate) const fn virtio_slots() -> u64 {
    VIRTIO_SLOTS
}

/// How many ECAM buses the kernel maps: bus 0, 1 MB. The window comes from the device tree's
/// `reg`, which on QEMU's `virt` is the highmem ECAM at `0x40_1000_0000` whatever the node's name
/// says. `pci::ecam_buses()` is what the kernel reads; no survey runs here, so it stays 1.
pub(crate) const PCI_ECAM_BUSES: u16 = 1;

/// How much of the 32-bit PCI memory window the kernel maps and assigns BARs from; the window
/// itself (base `0x1000_0000` on QEMU `virt`) comes from the machine's `ranges` via
/// `memory::pci_regions`. Nobody has programmed a BAR before us (we boot without EDK2, so there
/// is no PCI firmware), and the kernel places them itself, exactly as on riscv. A 2 MB mapped
/// slice bounds the page-table spend.
pub(crate) const PCI_BAR_MAPPED: u64 = 0x20_0000;

/// PCI INTx line A routes to GIC SPI 3 (INTID 32 + 3 = 35); B, C, D follow, by the standard
/// swizzle (`pci::intx_irq`). The pci crate's fixture test walks the machine's own
/// `interrupt-map` and asserts the formula matches all sixteen entries.
pub(crate) const PCI_IRQ_BASE: u32 = 35;

/// Map a range of *virtual* addresses to the physical ones they were linked against.
///
/// Kernel sections: their VA is what the linker gave them, and the PA is that minus the base.
fn map_range<A, P>(
    m: &mut Mapper<A, P, Aarch64>,
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
/// RAM in the largest leaf that fits wholly inside the range; device windows in 4 KiB pages.
fn direct_map<A, P>(
    m: &mut Mapper<A, P, Aarch64>,
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
    let len = (pa_end - pa_start).next_multiple_of(PAGE_SIZE);
    let largest = if flags.is_device() {
        PageSize::Size4KiB
    } else {
        PageSize::Size1GiB
    };
    m.map_span(phys_to_virt(pa_start), pa_start, len, flags, largest)
}

/// Walk the tables in software and check the things that would kill us.
///
/// Panics, while it can still print, on a map the switch would not survive.
fn verify<A, P>(m: &Mapper<A, P, Aarch64>)
where
    A: FnMut() -> Option<u64>,
    P: Fn(u64) -> *mut PageTable,
{
    // The code we are executing right now. If this isn't mapped executable, the instruction
    // after `msr sctlr_el1` never gets fetched.
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
        "our own .text is writable: W^X is broken"
    );

    // The stack. The first thing after the switch is a function return.
    let sp: u64;
    // SAFETY: reads a register.
    unsafe { core::arch::asm!("mov {}, sp", out(reg) sp, options(nomem, nostack)) };
    let (pa, flags) = m.translate(sp).expect("the stack is not mapped");
    assert_eq!(pa, virt_to_phys(sp), "the stack maps to the wrong frame");
    assert!(flags.is_writable(), "the stack is not writable");

    // The UART. Without it we cannot say anything, including why we died.
    let uart = phys_to_virt(UART_BASE);
    let (pa, flags) = m.translate(uart).expect("the UART is not mapped");
    assert_eq!(pa, UART_BASE);
    assert!(flags.is_writable());

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

/// Switch TTBR1 to the new tables, and take TTBR0 away.
///
/// The switch is live: the instruction after `msr ttbr1_el1` is fetched through the new tables.
///
/// # Safety
///
/// The tables at `root` must map, at minimum: the code executing this function, its stack,
/// and the UART. `verify` checks all three.
unsafe fn install(root: u64) {
    // The table walker is a separate observer: the table stores must reach it first.
    barrier::dsb(barrier::SY);

    // boot.s set the same value; this file owns the definition, and a disagreement would map the
    // UART as cacheable.
    MAIR_EL1.set(mair::VALUE);

    // TG0 and TG1 encode 4 KiB differently (0b00, 0b10), both spelled `KiB_4` by the crate. IPS
    // larger than the part supports is UNPREDICTABLE, so it is the part's own `PARange`.
    TCR_EL1.write(
        TCR_EL1::T0SZ.val(16)
            + TCR_EL1::TG0::KiB_4
            + TCR_EL1::SH0::Inner
            + TCR_EL1::ORGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
            + TCR_EL1::IRGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
            // Enabled: TTBR0 points at an empty table, so no switch rewrites TCR (RESERVED_TTBR0).
            + TCR_EL1::EPD0::EnableTTBR0Walks
            + TCR_EL1::T1SZ.val(16)
            + TCR_EL1::TG1::KiB_4
            + TCR_EL1::SH1::Inner
            + TCR_EL1::ORGN1::WriteBack_ReadAlloc_WriteAlloc_Cacheable
            + TCR_EL1::IRGN1::WriteBack_ReadAlloc_WriteAlloc_Cacheable
            + TCR_EL1::EPD1::EnableTTBR1Walks
            + TCR_EL1::IPS.val(super::isa::get().pa_range as u64),
    );

    TTBR1_EL1.set_baddr(root);

    // The system register writes must take effect before the TLB work below.
    barrier::isb(barrier::SY);

    // SAFETY: TLB maintenance is always sound.
    unsafe {
        core::arch::asm!(
            "tlbi vmalle1", // invalidate all EL1 translations
            "dsb ish",      // wait for it to finish, inner shareable domain
            "isb",
            options(nostack),
        );
    }

    // The TLB may hold translations from the boot tables still in .bss.
    //
    // SAFETY: TLB maintenance is always sound.
    unsafe {
        core::arch::asm!("tlbi vmalle1", "dsb ish", "isb", options(nostack),);
    }
}

/// An empty L0 table, installed in `TTBR0_EL1` whenever no user address space is active.
///
/// A low address from EL1 walks it and faults; swapping a user space is one `msr ttbr0_el1`. See
/// notes/mmu.md.
static RESERVED_TTBR0: AtomicU64 = AtomicU64::new(0);

/// Allocate and install the empty TTBR0 root. Called at the end of `init`.
fn install_reserved_ttbr0() {
    let root = memory::alloc()
        .expect("no frame for the reserved TTBR0 root")
        .addr();

    // SAFETY: a fresh frame, and nothing walks it until the `msr` below.
    unsafe {
        (*phys_to_ptr(root)).entries = [0; paging::ENTRIES];
    }

    RESERVED_TTBR0.store(root, Ordering::Relaxed);

    // SAFETY: the table is zeroed, so every translation through it faults. That is the point.
    unsafe { set_ttbr0(ttbr0_value(root, 0)) };
}

/// Compose the value `TTBR0_EL1` actually holds: the table root in the low bits, the ASID in
/// bits 63:48. This composed value is what travels through the switch path, so the register
/// write and the "already installed?" comparison are both one operation on one word.
pub(crate) fn ttbr0_value(root: u64, asid: u16) -> u64 {
    root | (asid as u64) << 48
}

/// Read the ASID back out of a composed [`ttbr0_value`]. The inverse of the line above, and it
/// exists so a portable test can ask "which tag is this space wearing?" without knowing that this
/// ISA keeps it in bits 63:48 and RISC-V keeps it in `satp[59:44]`.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the tests are its only caller; the kernel composes, never decomposes
pub fn asid_of(ttbr: u64) -> u16 {
    (ttbr >> 48) as u16
}

/// **Test-only: let EL1 load and store through pages marked EL0-accessible.** A no-op here, and the
/// no-op is the finding.
///
/// EL1 may already read an EL0 page, because this kernel never sets `PSTATE.PAN`; RISC-V needs
/// `sstatus.SUM`, and its twin carries the explanation.
///
/// Returns the previous state (always `true`) so the two arch modules have one signature.
#[cfg(feature = "system_tests")]
pub fn permit_kernel_access_to_user_pages(_allowed: bool) -> bool {
    true
}

/// Share the kernel into a fresh process root. **A no-op on aarch64:** the kernel lives in a
/// separate `TTBR1` that every process shares implicitly, so a process's `TTBR0` root carries only
/// user mappings. It exists so portable `user::AddressSpace` can call it unconditionally; on RISC-V,
/// with one `satp` per address space, it copies the kernel high-half into the root. See DECISIONS §17.
pub(crate) fn share_kernel_half(_root: u64) {}

/// Point `TTBR0_EL1` at a composed root-plus-ASID value ([`ttbr0_value`]). **No TLB flush.**
///
/// Every user mapping is tagged with its space's ASID, so the old space's entries stop matching.
/// See notes/address-space-identifiers.md.
///
/// # Safety
/// The root inside `ttbr` must be a page-aligned L0 table whose descriptors are all valid or
/// all zero, and the ASID must be the one that owns those mappings.
unsafe fn set_ttbr0(ttbr: u64) {
    // The table walker is a separate observer: the table stores must reach it first.
    barrier::dsb(barrier::SY);

    TTBR0_EL1.set(ttbr);
    barrier::isb(barrier::SY);
}

/// Discard every TLB entry tagged with `asid`, on every core. The teardown half of the ASID
/// contract (`crates/address_space_identifier`): after this, and only after this, the number may tag someone else.
///
/// The leading `dsb ishst` makes earlier table stores visible to every walker before the
/// invalidate is broadcast; the trailing pair completes it before return.
pub fn flush_asid(asid: u16) {
    let arg = (asid as u64) << 48; // tlbi aside1is takes the ASID in bits 63:48
    // SAFETY: TLB maintenance is always sound.
    unsafe {
        core::arch::asm!(
            "dsb ishst",
            "tlbi aside1is, {arg}",
            "dsb ish",
            "isb",
            arg = in(reg) arg,
            options(nostack),
        );
    }
}

/// Install a user address space. The low half of memory now means *that process*.
///
/// The unconditional install, for the system tests; the context switch uses [`switch_user_root`].
///
/// # Safety
/// `ttbr` must compose ([`ttbr0_value`]) a live L0 table built by a `Mapper` with `Half::Low`
/// and the ASID that owns it, and the table must outlive every instruction executed at EL0
/// afterwards.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub unsafe fn activate_user(ttbr: u64) {
    // SAFETY: this function's own `# Safety` contract is exactly the one this call needs; it forwards, it does not weaken.
    unsafe { set_ttbr0(ttbr) };
}

/// Uninstall whatever user address space was active. The low half means nothing again.
pub fn deactivate_user() {
    let reserved = RESERVED_TTBR0.load(Ordering::Relaxed);
    debug_assert!(reserved != 0, "mmu::init has not run");

    // SAFETY: the reserved table is zeroed, so this makes every low address fault. ASID 0: the
    // kernel's own tag, permanently reserved by the allocator, never a user's.
    unsafe { set_ttbr0(ttbr0_value(reserved, 0)) };
}

/// **Could EL0 read this address?** Asked of the silicon with `AT S1E0R`, not of the kernel's own
/// permission model. Only the system tests call it: no syscall dereferences a user pointer. See
/// notes/mmu.md, "Asking the silicon whether EL0 could read an address".
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn user_can_read(va: u64) -> bool {
    // SAFETY: address translation has no side effects beyond PAR_EL1.
    unsafe { can_el0_access(va, false) }
}

/// As [`user_can_read`], for a write. `AT S1E0W`. Same disposition, same test.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn user_can_write(va: u64) -> bool {
    // SAFETY: as above.
    unsafe { can_el0_access(va, true) }
}

/// # Safety
/// Sound for any address. The mapping may change after the answer.
///
/// Name: provisional, minted 2026-10-18 (UTC) by maintainer/names-comply-question-shapes, as the
/// yes-or-no worklist recommended.
/// Refused `translate_as_el0` (named for the `AT` instruction it issues, where the answer is about
/// access).
unsafe fn can_el0_access(va: u64, write: bool) -> bool {
    // PAR_EL1 is one shared register: a switch between the `at` and the `mrs` could leave another
    // thread's result in it.
    let was_enabled = crate::arch::interrupts::disable();

    let par: u64;
    // SAFETY: `at` has no effect but to write PAR_EL1, which we read immediately.
    unsafe {
        if write {
            core::arch::asm!(
                "at s1e0w, {va}",
                "isb",
                "mrs {par}, par_el1",
                va = in(reg) va,
                par = out(reg) par,
                options(nostack),
            );
        } else {
            core::arch::asm!(
                "at s1e0r, {va}",
                "isb",
                "mrs {par}, par_el1",
                va = in(reg) va,
                par = out(reg) par,
                options(nostack),
            );
        }
    }

    crate::arch::interrupts::restore(was_enabled);

    // PAR_EL1.F, bit 0: the translation faulted.
    par & 1 == 0
}

/// The root of whatever user address space is currently installed, read back from the CPU.
pub fn current_user_root() -> u64 {
    TTBR0_EL1.get_baddr()
}

/// Unmap one page at `va` from the user address space rooted at `root`, discharging the TLB
/// obligation. Returns the physical frame it pointed at, or `None` if nothing was mapped there.
///
/// Revocation (§13 (capability revocation)) calls it for every holder's space, so `root` is explicit; the registry forgets
/// a root before its tables are freed, so it is always live.
pub(crate) fn unmap_user_at(root: u64, va: u64) -> Option<u64> {
    // SAFETY: `root` is a live L0 table built with `Half::Low`; the direct map makes `phys_to_ptr`
    // valid; `unmap` allocates nothing.
    let mut mapper = unsafe { Mapper::<_, _, Aarch64>::new(root, Half::Low, || None, phys_to_ptr) };
    let (pa, flush) = mapper.unmap(va).ok()?;
    flush.flush(flush_tlb);
    Some(pa)
}

/// **Unmap many user pages of one space and invalidate once, by its tag**, for a caller that takes
/// back a whole run at a time ([`crate::revoke::revoke_region`]). `each` is handed an unmap and
/// calls it once per address; pages not mapped are skipped. Name provisional (lane 2451, 2026-10-20
/// UTC).
///
/// One [`flush_asid`] after the last unmap replaces a broadcast `tlbi` per page (683 for the
/// scanout). The caller must not hand any frame back before this returns.
pub(crate) fn unmap_user_pages(root: u64, asid: u16, each: impl FnOnce(&mut dyn FnMut(u64))) {
    // SAFETY: as `unmap_user_at`.
    let mut mapper = unsafe { Mapper::<_, _, Aarch64>::new(root, Half::Low, || None, phys_to_ptr) };
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

/// **Cut the page table at `table` out of the walk that reaches `va` in the space rooted at
/// `root`**, and flush that space's tag. Returns the span of addresses that went with it, as
/// `(base, size)`, or `None` if the walk to `va` no longer passes through `table`. Name
/// provisional (the page-tables-outlive-destroy lane, 2026-10-05 UTC).
///
/// `revoke::revoke_region` calls this for each table a region paid for before the region's pages
/// go back: see `paging::Mapper::unlink_table` for why the cut is one entry. The flush is by ASID
/// rather than by page because the span is up to a whole table's reach and the walk caches above
/// the leaves have to go too, and a by-tag flush reaches both on every core.
pub(crate) fn cut_user_table(root: u64, va: u64, table: u64, asid: u16) -> Option<(u64, u64)> {
    // SAFETY: `root` is a live low-half table (the registry forgets a root before its space frees
    // it); the direct map makes `phys_to_ptr` valid; a cut allocates nothing.
    let mut mapper = unsafe { Mapper::<_, _, Aarch64>::new(root, Half::Low, || None, phys_to_ptr) };
    let (base, span, flush) = mapper.unlink_table(va, table)?;
    flush.flush(|_| flush_asid(asid));
    Some((base, span))
}

/// Ask the user page tables rooted at `root` what `va` maps to. Like [`translate_user`], but for an
/// arbitrary root rather than the installed one, so revocation (and its tests) can inspect another
/// address space, and (milestone 126, `pmap`, DECISIONS §114) so `abi::address_space::LIST` can turn a
/// `va` `revoke::list_mapping` names into the permission bits a listing prints. Reads the tables
/// in memory; touches no register.
pub fn translate_at(root: u64, va: u64) -> Option<(u64, Flags)> {
    // SAFETY: `root` is an L0 table; the direct map makes `phys_to_ptr` valid.
    let mapper = unsafe { Mapper::<_, _, Aarch64>::new(root, Half::Low, || None, phys_to_ptr) };
    mapper.translate(va)
}

/// **Map a run of user pages into the current space and invalidate once, when the batch drops**,
/// for `PageFrame::MAP`. Name provisional (lane 2451, 2026-10-20 UTC).
///
/// A newly valid leaf needs a barrier, not an invalidate; the batch issues it once, in `Drop`, so
/// no early return skips it.
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
        let root = TTBR0_EL1.get_baddr();
        // SAFETY: as `map_current_user_page_frame`.
        let mut mapper =
            unsafe { Mapper::<_, _, Aarch64>::new(root, Half::Low, alloc, phys_to_ptr) };
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
        // The page-table writes must be visible to the table walker before the process touches
        // any page of the run.
        // SAFETY: a barrier is always sound.
        unsafe { core::arch::asm!("dsb ishst", "isb", options(nostack, nomem, preserves_flags)) };
    }
}

/// Map an **already-owned** physical page `phys` at `va` in the caller's address space, drawing
/// only the intermediate page tables from `alloc`.
///
/// The `PageFrame::MAP` and `MemoryRegion::MAP` path. The leaf is not allocated here: it is the
/// page the frame capability names, or the page `MemoryRegion::MAP` has just retyped. `alloc` supplies page-table nodes only, so a caller can point them at an untyped and
/// keep the kernel out of the allocation entirely.
pub(crate) fn map_current_user_page_frame(
    va: u64,
    phys: u64,
    flags: Flags,
    alloc: impl FnMut() -> Option<u64>,
) -> Result<(), MapError> {
    let root = TTBR0_EL1.get_baddr();

    // SAFETY: `root` is the live low-half table this thread owns; Half::Low refuses a high
    // address; the direct map makes `phys_to_ptr` valid for any page `alloc` returns.
    let mut mapper = unsafe { Mapper::<_, _, Aarch64>::new(root, Half::Low, alloc, phys_to_ptr) };
    mapper.map(va, phys, flags)?;

    // The page-table writes must be visible to the table walker before the process touches `va`.
    // SAFETY: a barrier is always sound.
    unsafe { core::arch::asm!("dsb ishst", "isb", options(nostack, nomem, preserves_flags)) };
    Ok(())
}

/// The empty table that means "no process is running", composed with the kernel's ASID 0,
/// ready for [`switch_user_root`].
pub(crate) fn reserved_root() -> u64 {
    ttbr0_value(RESERVED_TTBR0.load(Ordering::Relaxed), 0)
}

/// Install `ttbr` (a composed root + ASID) as the live low half, **unless already installed**.
///
/// Called on every switch. The register carries the ASID, so "same process" is one comparison
/// and skips two barriers; a switch that happens flushes nothing.
///
/// # Safety
/// `ttbr` must be a value [`ttbr0_value`] composed over a **live** `AddressSpace`'s L0 root and the
/// ASID that owns its mappings, or [`reserved_root`]. The table must outlive every instruction
/// executed at EL0 until some later write to `TTBR0_EL1` replaces it. Why this is a contract and
/// not a type is in notes/mmu.md.
pub(crate) unsafe fn switch_user_root(ttbr: u64) {
    if TTBR0_EL1.get() == ttbr {
        return;
    }

    // SAFETY: this function's own `# Safety` contract is exactly the one this call needs; it
    // forwards, it does not weaken.
    unsafe { set_ttbr0(ttbr) };
}

/// Serializes mutation of the kernel's page tables across cores.
///
/// Two cores spawning threads map into the shared TTBR1 tables at once. Ranked below `IPC_TABLES`
/// and above the allocators it calls (`sync::rank::KERNEL_MMU`).
static KERNEL_MMU: crate::sync::IrqSafeMutex<()> =
    crate::sync::IrqSafeMutex::new(crate::sync::rank::KERNEL_MMU, ());

/// The kernel's live page tables, as a `Mapper`.
///
/// Reads `TTBR1_EL1` back out of the hardware, so this walks what the CPU is actually walking,
/// not a copy of what we intended. **Call only while holding [`KERNEL_MMU`].**
// The concrete Mapper type (an opaque closure plus a fn pointer plus the format) is unavoidably
// verbose and cannot be a `type` alias without TAIT; the shape is the point, not a problem.
#[allow(clippy::type_complexity)]
fn kernel_mapper() -> Mapper<impl FnMut() -> Option<u64>, fn(u64) -> *mut PageTable, Aarch64> {
    let root = TTBR1_EL1.get_baddr();

    // SAFETY: TTBR1_EL1 holds the root we installed, and the direct map makes `phys_to_ptr`
    // valid for any frame.
    unsafe {
        Mapper::<_, _, Aarch64>::new(
            root,
            Half::High,
            || memory::alloc().map(|f| f.addr()),
            phys_to_ptr,
        )
    }
}

/// Map one page into the kernel's address space.
///
/// Refuses to overwrite an existing mapping (`MapError::AlreadyMapped`), which is what forces
/// break-before-make: to *change* a mapping you must [`unmap_page`] first.
pub(crate) fn map_page(va: u64, pa: u64, flags: Flags) -> Result<(), MapError> {
    let _guard = KERNEL_MMU.lock(); // exclusive: two cores must not mutate the tables at once
    kernel_mapper().map(va, pa, flags)
}

/// Remove one page from the kernel's address space, and **invalidate the TLB**.
///
/// Returns the physical frame, which is the caller's to free: the mapper never owned it.
///
/// The `TlbFlush` obligation is discharged here, properly, with a real `tlbi`. It cannot be
/// forgotten: dropping one un-discharged panics.
pub(crate) fn unmap_page(va: u64) -> Result<u64, MapError> {
    let _guard = KERNEL_MMU.lock(); // exclusive: see map_page
    let (pa, flush) = kernel_mapper().unmap(va)?;
    flush.flush(flush_tlb);
    Ok(pa)
}

/// Invalidate the TLB entry for one virtual address.
///
/// Discharges a `paging::TlbFlush`: `tlbi vaae1is` (by VA, all ASIDs, EL1, inner shareable) on
/// the page number. TLB maintenance is not synchronous, hence the `dsb ish` and `isb` after.
pub(crate) fn flush_tlb(va: u64) {
    // SAFETY: TLB maintenance is always sound. Getting it wrong means a stale translation, not
    // memory unsafety in the Rust sense; but a stale translation IS memory unsafety in the
    // sense that matters here.
    unsafe {
        core::arch::asm!(
            "dsb ishst",             // our page table write must land first
            "tlbi vaae1is, {page}",  // then forget the translation
            "dsb ish",               // wait for every core to have done so
            "isb",                   // and discard anything fetched through the old mapping
            page = in(reg) va >> 12,
            options(nostack),
        );
    }
}

pub(crate) fn is_enabled() -> bool {
    SCTLR_EL1.is_set(SCTLR_EL1::M)
}

/// Ask the live **kernel** page tables what a high virtual address maps to.
///
/// Reads `TTBR1_EL1` back out of the hardware, so tests check the tables the CPU walks.
pub fn translate(va: u64) -> Option<(u64, Flags)> {
    let root = TTBR1_EL1.get_baddr();

    // SAFETY: TTBR1_EL1 holds the root we installed, and the direct map makes `phys_to_ptr`
    // valid.
    let mapper = unsafe { Mapper::<_, _, Aarch64>::new(root, Half::High, || None, phys_to_ptr) };
    mapper.translate(va)
}

/// **Is this kernel address mapped, asked without taking a lock?**
///
/// For fault handlers. [`translate`] already takes no lock here, so this is a rename of its
/// question rather than a different walk; it exists because RISC-V's `translate` **does** take one
/// and its callers are handlers that may not block. Same name, same meaning, one signature on both
/// architectures: `stack::print_text_words` is written once and calls this.
///
/// Name: provisional (2026-08-17): calef has not ruled on it.
pub fn is_mapped(va: u64) -> bool {
    translate(va).is_some()
}

/// Ask the live **user** page tables what a low virtual address maps to.
///
/// Reads `TTBR0_EL1`, so it answers for whichever address space is installed right now, which
/// is either a process's or the empty reserved table.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // the address-space tests are the only callers
pub fn translate_user(va: u64) -> Option<(u64, Flags)> {
    let root = TTBR0_EL1.get_baddr();

    // SAFETY: TTBR0_EL1 holds a root we installed, and the direct map makes `phys_to_ptr` valid.
    let mapper = unsafe { Mapper::<_, _, Aarch64>::new(root, Half::Low, || None, phys_to_ptr) };
    mapper.translate(va)
}

/// The boot banner's MMU line. Its only caller is the banner in `main.rs`, which the test build and
/// the `bench` boot mode both compile out (`cfg(not(any(test, feature = "bench")))`), so it has no
/// caller in exactly those two configurations.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub(crate) fn print_summary() {
    println!(
        "  mmu             : {}, kernel in TTBR1 at {:#018x}, TTBR0 free for userspace",
        if is_enabled() { "on" } else { "OFF" },
        KERNEL_VA_BASE,
    );
    println!(
        "  stack guard     : {:#018x} (unmapped; a stack overflow faults here)",
        stack_guard(),
    );
}

// Section boundaries, page-aligned by link-aarch64.ld so each section carries its own permissions.

macro_rules! linker_symbol {
    ($name:ident, $sym:ident) => {
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

#[cfg(test)]
mod tests {
    //! Tests for the MMU: the live page tables, W^X, the guard page, and TLB invalidation.
    //!
    //! `translate` reads `TTBR1_EL1` back out of the hardware, so these inspect the tables the CPU
    //! is *actually walking*, not a copy of what we intended.

    /// The MMU is on, and we are still alive to say so.
    #[test_case]
    fn mmu_is_enabled() {
        assert!(crate::arch::mmu::is_enabled(), "SCTLR_EL1.M is not set");
    }

    /// The kernel is running at high virtual addresses, out of TTBR1.
    ///
    /// So TTBR0 can be swapped per process without unmapping the kernel.
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
        let sp: u64;
        // SAFETY: reads a register.
        unsafe { core::arch::asm!("mov {}, sp", out(reg) sp, options(nomem, nostack)) };
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

    /// **TTBR0 is free.** Nothing of ours lives at a low address any more.
    ///
    /// Asserts the property, a low address does not translate, not the mechanism that provides it.
    #[test_case]
    fn a_low_address_does_not_translate_when_no_process_is_running() {
        use crate::arch::mmu::translate_user;

        for va in [0x1000u64, 0x4008_0000, 0x0000_ffff_ffff_f000] {
            assert!(
                translate_user(va).is_none(),
                "{va:#x} translates through TTBR0: a stale identity map may still be live",
            );
        }
    }

    /// The direct map: every physical address is nameable at `pa | KERNEL_VA_BASE`.
    ///
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
    /// Also checked at boot by `mmu::verify`. See notes/stack.md.
    #[test_case]
    fn the_guard_page_is_a_hole() {
        use crate::arch::mmu;
        assert_eq!(
            mmu::translate(mmu::stack_guard()),
            None,
            "the guard page IS mapped: a stack overflow would silently eat .bss again"
        );

        // And the pages either side of it must be mapped, or we've put the hole in the
        // wrong place and are protecting nothing.
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
    /// `translate` reads `TTBR1_EL1` back out of the CPU and walks from there.
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
        assert!(!flags.is_user_executable(), ".text is executable by EL0");
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
    /// See `UART_BASE` and notes/page-tables.md.
    #[test_case]
    fn the_uart_is_mapped_as_device_memory() {
        use crate::arch::mmu;

        let (_, flags) =
            mmu::translate(mmu::phys_to_virt(0x0900_0000)).expect("the UART is not mapped");

        // The MAIR-slot encoding is checked in paging::aarch64; this is the portable property.
        assert!(flags.is_device(), "the UART is not device memory");

        assert!(flags.is_writable(), "we do need to write to it");
        assert!(!flags.is_kernel_executable());
    }

    /// A frame from the allocator is still real, writable memory *through the MMU*.
    ///
    /// The direct map covers everything the allocator can hand out.
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
    /// Maps a VA to frame A and reads it into the TLB, unmaps, maps the same VA to frame B and
    /// reads again. A stale TLB returns A's data while the tables in memory are correct.
    #[test_case]
    fn unmap_invalidates_the_tlb() {
        use paging::Flags;

        use crate::arch::mmu::{self, phys_to_virt};

        const PATTERN_A: u64 = 0xaaaa_aaaa_aaaa_aaaa;
        const PATTERN_B: u64 = 0xbbbb_bbbb_bbbb_bbbb;

        // A high-half address well away from the direct map: physical 0xff00_0000 is not RAM
        // (RAM is 0x4000_0000..0x5000_0000), so nothing is mapped here.
        let test_va = mmu::KERNEL_VA_BASE | 0xff00_0000;
        assert_eq!(
            mmu::translate(test_va),
            None,
            "test address is already in use"
        );

        let a = crate::memory::alloc().expect("out of memory");
        let b = crate::memory::alloc().expect("out of memory");

        // SAFETY: two frames the allocator just gave us exclusively, reached via the direct
        // map.
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
    #[test_case]
    fn the_kernel_mapper_refuses_to_overwrite() {
        use paging::{Flags, MapError};

        use crate::arch::mmu;

        let va = mmu::KERNEL_VA_BASE | 0xfe00_0000;
        let f = crate::memory::alloc().unwrap();

        mmu::map_page(va, f.addr(), Flags::kernel_data()).unwrap();

        // aarch64 does not permit valid -> valid directly: it can raise a TLB conflict abort.
        // The API makes it unrepresentable.
        assert_eq!(
            mmu::map_page(va, f.addr(), Flags::kernel_data()),
            Err(MapError::AlreadyMapped)
        );

        mmu::unmap_page(va).unwrap();
        crate::memory::free(f);
    }
}
