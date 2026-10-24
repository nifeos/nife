//! **What this machine is, read once from the loader's handoff and ACPI and kept for the steps
//! after it**: the `x86_64` front end of `memory::init` (milestone 890 (riscv64 and `x86_64` join
//! the shared boot path)).
//!
//! [`init`] reads the PVH `hvm_start_info`, walks the ACPI tables, records the device windows
//! `mmu::init` maps, finds the PCIe topology while the boot map still reaches every bus, and brings
//! the frame allocator up. The walk is kept, because the tables are reachable only through the boot
//! map `mmu::init` replaces. The parsing is `machine.rs`'s; see notes/x86-port/acpi-and-pci.md.
//!
//! Name: provisional (milestone 890's lane, 2026-10-19 UTC).

use super::machine::{self, Acpi};
use crate::println;
use crate::sync::{IrqSafeMutex, rank};

/// The ACPI walk [`init`] made, for the bring-up steps after `mmu::init` that cannot repeat it.
static ACPI: IrqSafeMutex<Option<Acpi>> = IrqSafeMutex::new(rank::ACPI, None);

/// **Read `f`'s answer out of the ACPI walk.** The lock is held only for `f`, so `f` copies out
/// what it needs and must take no lock ranked above [`rank::ACPI`]; the IOMMUs' tables are copied
/// out before their drivers lock anything, for that reason.
///
/// # Panics
/// Before [`init`], which `memory::init` runs ahead of every caller.
pub(crate) fn with_acpi<R>(f: impl FnOnce(&Acpi) -> R) -> R {
    f(ACPI
        .lock()
        .as_ref()
        .expect("arch::discovery::with_acpi before memory::init walked the tables"))
}

/// The local APIC's physical address, as the MADT stated it, or `None` before [`init`] or on a
/// machine whose MADT did not say. Read by the fine map and the BAR window, which both run before
/// `arch::irq` has brought anything up.
pub(crate) fn local_apic_phys() -> Option<u64> {
    ACPI.lock().as_ref().and_then(|a| a.local_apic)
}

/// The IO APIC's physical address, as the MADT stated it, with [`local_apic_phys`]'s `None`s.
pub(crate) fn io_apic_phys() -> Option<u64> {
    ACPI.lock()
        .as_ref()
        .and_then(|a| a.io_apic.map(|(_, at, _)| at as u64))
}

/// **`memory::init` on this machine**: the handoff, the tables, the device windows, PCI, and the
/// frame allocator, in that order.
///
/// The order is load-bearing twice. The walk is stored before PCI so the BAR window can stay clear
/// of the APICs the MADT names (`mmu::memory_mapped_io_window`). And PCI comes before the frame
/// allocator and `mmu::init` because the bridges are read through the boot map, which reaches every
/// bus, and `mmu::init` then maps exactly the buses this found.
pub(crate) fn init() {
    let pointer = crate::DTB.load(core::sync::atomic::Ordering::Relaxed);
    let Some(info) = machine::boot_info(pointer) else {
        println!("  memory      : no PVH boot info at {pointer:#x}; nothing else can be found");
        println!("nife x86_64: early boot cannot continue, halting.");
        crate::arch::halt(crate::arch::HaltReason::before_scheduler());
    };
    machine::print_memory_map(&info);

    // ACPI: what x86 has instead of a device tree. The RSDP is scanned for rather than taken from
    // the handoff, because QEMU's PVH loader leaves that field zero (measured); the checksum inside
    // the parser is what separates a real hit from a coincidence.
    let acpi = machine::read_acpi(info.rsdp);
    machine::print_acpi_summary(&acpi);

    // COM1's interrupt line, as the legacy number 4 and not its GSI: `arch::irq::enable` resolves a
    // legacy IRQ through the MADT's overrides, so recording the GSI would resolve it twice.
    crate::memory::record_uart_irq(crate::user::UART_RX_INTID);

    // The IOMMUs' register windows, recorded now because `mmu::map_everything` maps them
    // device-typed and the BAR window below must not overlap them; the units themselves come up
    // after the fine map exists (`bring_up_iommus`). AMD-Vi's IVHD carries no size, so its window
    // is the 16 KiB every register this kernel reads lives in.
    for d in acpi.dmar.units() {
        crate::memory::record_vtd_region(d.register_base, d.register_size);
    }
    for d in acpi.ivrs.units() {
        crate::memory::record_amd_vi_region(d.register_base, super::amd_vi::REGISTER_SIZE);
    }

    let ecam = acpi.ecam;
    *ACPI.lock() = Some(acpi);
    bring_up_pci(ecam);

    let regions = machine::bring_up_memory(&info);
    println!("  frames      : allocator up over {regions} ram region(s)");
}

/// **The PCIe configuration window, its BAR window, and the topology**, from the MCFG's `ecam`
/// (base, first bus, last bus).
///
/// Recorded where `kernel/src/pci.rs` already looks, `memory::pci_regions()`, the static a
/// device-tree machine fills from its `pci-host-ecam-generic` node. No MCFG is no PCI at all, as
/// for a tree with no such node.
fn bring_up_pci(ecam: Option<(u64, u8, u8)>) {
    match ecam {
        // `pci.rs` addresses a function by absolute bus number from the MCFG base, which is the
        // first bus's: they agree only when that bus is zero. Refused rather than adjusted, since
        // `base - (lo << 20)` lies below the window `mmu::map_everything` maps.
        Some((_, lo, _)) if lo != 0 => {
            println!(
                "  pci         : skipped, the MCFG's first bus is {lo} and this port assumes 0"
            );
        }
        Some((base, lo, hi)) => {
            let buses = hi as u32 - lo as u32 + 1;
            let decode = machine::enable_pcie_ecam(base, buses);
            let ecam = (base, buses as u64 * 0x10_0000);
            // A failure here panics rather than falling back to a constant, which would place
            // device registers over memory the allocator owns. The message is the whole diagnosis:
            // on a machine with no serial cable, a photograph of the screen is the transcript.
            let bar = match super::mmu::memory_mapped_io_window(ecam) {
                Ok(window) => window,
                Err(why) => panic!("no PCI BAR window on this machine: {why}"),
            };
            crate::memory::record_pci_regions(ecam, bar);
            println!(
                "  pci         : ecam at {base:#x} (buses {lo}..={hi}), decode {}",
                match decode {
                    machine::EcamDecode::AlreadyOn => "was already on",
                    machine::EcamDecode::Programmed => "programmed here",
                    machine::EcamDecode::Unencodable =>
                        "LEFT AS FOUND: this bus count has no PCIEXBAR length field",
                }
            );
            println!(
                "                bar window {:#x}..{:#x}, from the firmware map{}",
                bar.0,
                bar.0 + bar.1,
                if machine::top_of_low_dram().is_some() {
                    " and TOLUD, agreeing"
                } else {
                    " alone (this host bridge reports no TOLUD)"
                }
            );
            // The topology, read from the bridges while the boot map still covers the low 4 GiB, so
            // every bus the MCFG describes is readable; `mmu::init` then maps exactly the buses
            // this found. See `pci::survey`.
            crate::pci::survey();
            // Only this architecture's BARs may already be placed by something else when the
            // kernel arrives; both `virt` boards boot with every BAR zero. See `pci::bar_census`
            // for why the second number is the one to watch on real firmware.
            let (functions, outside) = crate::pci::bar_census();
            println!(
                "                {functions} function(s) on the bus, \
                 {outside} with a BAR this kernel can neither use nor adopt",
            );
        }
        // No MCFG means no PCI, with no fallback to the legacy 0xcf8/0xcfc mechanism: its ports
        // see only the first 256 bytes of configuration space, so every extended capability would
        // be absent and a driver would fail somewhere else entirely.
        None => {
            println!("  pci         : skipped, no MCFG: no PCI at all (no legacy fallback)");
        }
    }
}

/// **Every IOMMU the tables name, each translating the devices it owns**: VT-d's units from the
/// DMAR (milestone 594 (every VT-d unit translates its own devices)), or AMD-Vi's from the IVRS.
///
/// The position the SMMUv3 and the RISC-V IOMMU take on the other two architectures: after the fine
/// map (a unit's register file is device-typed MMIO, reachable only through it) and before anything
/// attaches a device. Each unit's line is read back from its own status register, so a unit
/// reported up is the hardware's answer rather than an assumption that a write took.
pub(crate) fn bring_up_iommus() {
    let (dmar, ivrs) = with_acpi(|a| (a.dmar, a.ivrs));
    if !dmar.units().is_empty() {
        super::iommu::init(&dmar);
    } else if !ivrs.units().is_empty() {
        println!("  vt-d        : skipped, no DMAR (this machine's IOMMU is AMD-Vi)");
        super::amd_vi::init(&ivrs);
    } else {
        // Loud, because the alternative is a boot that reads like every other one while nothing
        // stands between a device and all of memory.
        println!("  vt-d        : skipped, no DMAR");
        println!(
            "  iommu       : NONE: this machine's ACPI names neither a DMAR (VT-d) nor an IVRS \
             (AMD-Vi), so no device's DMA is confined; every driver below runs unconfined"
        );
    }
}
