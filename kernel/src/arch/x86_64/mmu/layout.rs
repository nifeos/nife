//! **Where things are**: the kernel image's base, the direct map and its two translations,
//! the thread-stack area, and the fixed physical addresses of the devices every PC has.
//!
//! Cut out of `mmu.rs` unchanged by lane `maintainer/last-three-over-2000`, which left the
//! parent an index (§282 (when a file is split, the parent becomes an index)) and under §266 (a
//! Rust source file stays under 2,000 lines).
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

/// The base of the kernel **image**'s virtual addresses.
///
/// **Fixed at 0xffffffff80000000 by the target's code model**, not chosen for taste: see the module
/// header and link-x86_64.ld. It decomposes as PML4[511], PDPT[510], PD[0], which is why `boot.s`
/// can alias the first gigabyte into the high half with a single PDPT entry.
///
/// This is deliberately **not** where physical memory is mapped; see [`DIRECT_MAP_BASE`].
pub const KERNEL_VA_BASE: u64 = 0xffff_ffff_8000_0000;

/// **The base of the direct map: where physical address `pa` is readable as `pa + this`.**
///
/// `0xffff888000000000`, Linux's `page_offset_base`. It must be canonically high, clear of
/// PML4[511] where the image lives and of the low half ring 3 gets, with room above it for all of
/// physical memory (it has 64 TiB). `boot.s` installs its PML4 entry before any Rust runs, so the
/// arithmetic below is valid from the first instruction. See notes/x86-port/kernel-page-tables.md.
///
/// Name: provisional (milestone 161 (the `x86_64` kernel port)): calef names the constants, and
/// this one was minted by a lane. The tree's analogous name is [`KERNEL_VA_BASE`], which is why
/// this is `_BASE` too.
pub const DIRECT_MAP_BASE: u64 = 0xffff_8880_0000_0000;

/// **Where kernel thread stacks live, virtually** (`thread.rs`'s `STACK_AREA`): PML4[402], which
/// is nothing else's (the image is PML4[511] and the direct map PML4[273]).
///
/// `0xffff_c900_0000_0000` is Linux's `VMALLOC_START`. The other architectures' expression,
/// `KERNEL_VA_BASE | 0x10_0000_0000`, is the identity here, because this `KERNEL_VA_BASE` already
/// has those bits set, and would put every stack over `.text`. See
/// notes/x86-port/kernel-page-tables.md.
///
/// Name: provisional (milestone 161, roadmap item 4).
pub const THREAD_STACK_AREA: u64 = 0xffff_c900_0000_0000;

/// The top-level (PML4) index the direct map occupies, **duplicated in `boot.s`** because a 32-bit
/// assembler cannot compute it from the constant above. This is the gate that keeps the two in
/// agreement: change [`DIRECT_MAP_BASE`] without changing `boot.s` and the kernel does not build.
const DIRECT_MAP_PML4_INDEX: u64 = 273;

const _: () = {
    assert!(
        DIRECT_MAP_BASE >> 47 == 0x1_ffff,
        "the direct map base must be canonically high, or every address in it faults"
    );
    assert!(
        (DIRECT_MAP_BASE >> 39) & 0x1ff == DIRECT_MAP_PML4_INDEX,
        "boot.s writes PML4[273]: change both or neither"
    );
    assert!(
        (KERNEL_VA_BASE >> 39) & 0x1ff != DIRECT_MAP_PML4_INDEX,
        "the image and the direct map must not share a PML4 entry"
    );
};

/// The kernel's view of physical address `pa`: its address in the direct map.
///
/// **Not for the kernel image's own frames.** The direct map skips them on purpose (a second,
/// writable alias of `.text` would undo W^X; `claims::direct_map_claims` step 1), so the result for
/// an image frame is unmapped and a read through it faults.
pub const fn phys_to_virt(pa: u64) -> u64 {
    pa + DIRECT_MAP_BASE
}

/// The physical address behind kernel virtual address `va`.
///
/// **Two branches, because this architecture has two kernel bases** (see the module header). An
/// address at or above [`KERNEL_VA_BASE`] is part of the kernel image, linked there by
/// link-x86_64.ld; anything else in the high half is a direct-map address. Both callers exist:
/// `memory::image_start` hands this a linker symbol, and `sched`/`kmem` hand it pointers that came
/// out of [`phys_to_virt`]. Linux's `__pa()` makes the same distinction for the same reason.
///
/// It is the exact inverse of [`phys_to_virt`] over the direct map, now for **all** of physical
/// memory rather than the low 2 GiB the single-base arithmetic could reach.
pub const fn virt_to_phys(va: u64) -> u64 {
    if va >= KERNEL_VA_BASE {
        va - KERNEL_VA_BASE
    } else {
        va - DIRECT_MAP_BASE
    }
}

/// COM1's I/O port on every x86 machine since the PC/AT, and on QEMU's `q35`. Not an address in the
/// memory space at all; see `arch/x86_64/port.rs` for what that means and why the console reaches it
/// through a different accessor than the other two architectures' UARTs.
pub(crate) const COM1_PORT: usize = 0x3f8;

/// The local APIC's default physical base. Relocatable through `IA32_APIC_BASE`, and nothing
/// relocates it; the constant is where the machine puts it at reset.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) const LOCAL_APIC_PHYS: u64 = 0xfee0_0000;

/// The IO APIC's default physical base on a PC-compatible machine, used only when the ACPI MADT
/// did not say. `irq::io_apic_phys()` is what the machine actually reported, and q35 reports
/// exactly this.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) const IO_APIC_PHYS: u64 = 0xfec0_0000;

/// **Where q35 puts the PCIe ECAM window**, QEMU's q35 default. Not mapped directly: the real
/// window comes from ACPI's MCFG through `memory::pci_regions()`. It stays as the value
/// `machine::print_acpi_summary` prints the MCFG's answer against, which on x86 is no equality
/// invariant: firmware places the window (OVMF at `0xe000_0000`, xenon at `0xf000_0000`), so the
/// two agree only on q35 under `SeaBIOS`. See notes/x86-port/kernel-page-tables.md.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) const PCI_ECAM_PHYS: u64 = 0xb000_0000;

/// How many PCI buses of the ECAM window the kernel maps **before it has asked the machine**. One
/// bus is 1 MiB of configuration space, and the MCFG on the first real machine this port met
/// describes a hundred and twenty-eight of them.
///
/// **The floor, not the answer, since milestone 320.** `pci::ecam_buses()` is what the kernel maps
/// and reads: this value until `pci::survey` has run, and the topology the survey found afterwards.
/// The survey runs from `kernel_main` while the boot tables still cover the low 4 GiB
/// indiscriminately, which is the only window in the boot where every bus the MCFG describes is
/// readable; `map_everything` runs after it and maps what it found.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) const PCI_ECAM_BUSES: u16 = 1;

/// A page table at physical address `pa`, reached through the direct map.
pub fn phys_to_ptr(pa: u64) -> *mut paging::PageTable {
    phys_to_virt(pa) as *mut paging::PageTable
}
