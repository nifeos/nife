//! **Every physical range the direct map covers, as one enumeration**: RAM, the firmware's
//! reservations and the device windows, each named for the panic that reports a collision.
//!
//! Cut out of `mmu.rs` unchanged by lane `maintainer/last-three-over-2000`, which left the
//! parent an index (§282 (when a file is split, the parent becomes an index)) and under §266 (a
//! Rust source file stays under 2,000 lines). `super::ap_boot`, `super::discovery` and `super::machine`
//! name the `x86_64` items the index imports, as in `mmu.rs`.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// The first megabyte: real-mode legacy that is nonetheless memory, and the only part of the address
/// space this kernel reads by absolute physical address (the BIOS Data Area's EBDA pointer at
/// `0x40e`, the BIOS area the RSDP is scanned out of). The frame allocator clips it rather than
/// managing it, so it is in no RAM region and would otherwise be unmapped.
pub(super) const LOW_MEGABYTE: u64 = 0x10_0000;

/// **One physical range the direct map covers, and what to call it when it goes wrong.**
///
/// Every direct-map range [`map_everything`] builds comes out of [`direct_map_claims`] as one of
/// these, and the failure path walks the *same* enumeration to say what a refused mapping collided
/// with. That is the point of the type: a diagnosis derived from a second, hand-written list of
/// "what we probably map" can drift from what is mapped, and the first time anyone reads it will be
/// on a bench with no debugger.
///
/// Name: provisional. calef names the types, and this one was minted by a lane.
pub(super) struct Claim {
    /// What a reader of [`map_everything`] would call this range, short enough to fit a panic line
    /// on a screen console.
    pub(super) what: &'static str,
    /// The physical range, half-open. Empty (`hi <= lo`) is legal and maps nothing.
    pub(super) lo: u64,
    pub(super) hi: u64,
    pub(super) flags: Flags,
    /// **Map page by page, skipping what is already mapped, instead of refusing.**
    ///
    /// True only for the loader's reserved entries, which may abut or overlap each other and the
    /// RAM regions in the map the firmware wrote; nobody promised that list is disjoint. Everywhere
    /// else a second mapping of a frame is a real defect and [`MapError::AlreadyMapped`] is the
    /// mechanism that says so, so this is deliberately not a convenience anything else may reach
    /// for.
    pub(super) guarded: bool,
}

/// **Every physical range the direct map covers, in the order [`map_everything`] builds them.**
///
/// Split out from the mapping loop so that the failure path can ask the same question the mapping
/// asked. See [`Claim`].
pub(super) fn direct_map_claims(each: &mut dyn FnMut(Claim)) {
    // 1. Every RAM region, except the kernel image's own frames: the image lives at a different
    //    base, so a direct-map entry for them would be a second, writable alias of `.text`. The
    //    image is on the allocator's forbidden list, so nothing needs them by physical address.
    let image_lo = memory::image_start();
    let image_hi = memory::image_end();
    for (start, size) in memory::ram_regions() {
        let end = start + size;
        each(Claim {
            what: "ram",
            lo: start,
            hi: image_lo.min(end),
            flags: Flags::kernel_data(),
            guarded: false,
        });
        each(Claim {
            what: "ram above the image",
            lo: image_hi.max(start),
            hi: end,
            flags: Flags::kernel_data(),
            guarded: false,
        });
    }

    // 1b. The AP trampoline's LMA, the gap link-x86_64.ld opens between `.rodata` and `.data`.
    //     Step 1 skips it with the image, and neither section's mapping reaches it, so nothing
    //     covers it unless this does. `ap_boot::prepare` reads it through the direct map.
    each(Claim {
        what: "ap trampoline",
        lo: super::ap_boot::trampoline_lma(),
        hi: super::ap_boot::trampoline_lma() + super::ap_boot::trampoline_size(),
        flags: Flags::kernel_data(),
        guarded: false,
    });

    // 5. The device registers, device-typed, one window each, before the firmware reservations:
    //    the reservation fill is guarded and these are not, so whichever runs first decides a
    //    shared page's memory type, and a cacheable device window loses writes. The local APIC's
    //    address is the MADT's when ACPI answered, else the reset default.
    let apic = super::discovery::local_apic_phys().unwrap_or(LOCAL_APIC_PHYS);
    each(Claim {
        what: "local apic",
        lo: apic,
        hi: apic + PAGE_SIZE,
        flags: Flags::device(),
        guarded: false,
    });

    // The IO APIC's page, likewise from the MADT when ACPI answered: a machine with two puts the
    // second somewhere the constant does not name.
    let io_apic = super::discovery::io_apic_phys().unwrap_or(IO_APIC_PHYS);
    each(Claim {
        what: "io apic",
        lo: io_apic,
        hi: io_apic + PAGE_SIZE,
        flags: Flags::device(),
        guarded: false,
    });

    // The PCIe windows: bus 0 of the ECAM space the MCFG named, and the BAR window
    // (`memory_mapped_io_window`). No window recorded means no mapping, and `pci.rs` reports nobody
    // home rather than touching unconfirmed MMIO. One ECAM bus, as on the other two architectures:
    // everything QEMU puts here is on bus 0.
    if let Some(((ecam, ecam_size), (bar, bar_size))) = memory::pci_regions() {
        each(Claim {
            what: "pci ecam",
            lo: ecam,
            hi: ecam + crate::pci::ecam_bytes().min(ecam_size),
            flags: Flags::device(),
            guarded: false,
        });
        each(Claim {
            what: "pci bar window",
            lo: bar,
            hi: bar + PCI_BAR_MAPPED.min(bar_size),
            flags: Flags::device(),
            guarded: false,
        });
    }

    // VT-d's register files, one per unit, device-typed, each as large as its DRHD's `Size` byte
    // says, at the addresses the DMAR named. Without this a DRHD read faults the instant the fine
    // map replaces the boot map.
    for (base, size) in memory::vtd_regions().into_iter().flatten() {
        each(Claim {
            what: "vt-d registers",
            lo: base,
            hi: base + size,
            flags: Flags::device(),
            guarded: false,
        });
    }
    // AMD-Vi's register files (lane `amd-vi`), the same claim on a machine whose IVRS named them.
    for (base, size) in memory::amd_vi_regions().into_iter().flatten() {
        each(Claim {
            what: "amd-vi registers",
            lo: base,
            hi: base + size,
            flags: Flags::device(),
            guarded: false,
        });
    }

    // The screen, when the handoff described one: the console writes to it through the boot map
    // from the first line, so an unmapped framebuffer would fault inside `println!`. Mapped
    // write-combining, which is still device memory to every rule here and selects the PAT entry
    // `cpu_init` made WC on every core; the console only writes it, and WC reads are as slow as UC
    // ones. An aperture inside a RAM region is a different problem; see
    // notes/x86-port/kernel-page-tables.md.
    if let Some((base, size)) = memory::framebuffer() {
        each(Claim {
            what: "framebuffer",
            lo: base,
            hi: base + size,
            flags: Flags::write_combining(),
            guarded: false,
        });
    }

    // 2. Memory the loader did not call usable RAM but which is still memory: the first megabyte
    //    (IVT, BDA, EBDA, the RSDP scan area, the AP trampoline page) and the reserved entries
    //    carved out of DRAM (ACPI tables, NVS, firmware scratch), all read through `phys_to_virt`.
    //    Guarded, since nothing promises the firmware's list is disjoint; `AlreadyMapped` is never
    //    treated as success.
    firmware_claims(each);
}

/// The loader-reserved half of [`direct_map_claims`]: the low megabyte, every reserved entry below
/// [`firmware_fill_ceiling`], and the initial ramdisk.
///
/// Physical page 0 is deliberately left out, so that `phys_to_virt(0)` faults instead of quietly
/// naming the interrupt vector table.
///
/// Claims only the low megabyte when the boot structure cannot be re-read. That is the right
/// failure: this kernel got its RAM regions from the same structure, so a machine that reaches here
/// without one had no ACPI tables to lose access to either.
///
/// Name: provisional (milestone 161, renamed from `map_firmware_regions` when it stopped mapping
/// and started describing).
fn firmware_claims(each: &mut dyn FnMut(Claim)) {
    each(Claim {
        what: "low megabyte",
        lo: PAGE_SIZE,
        hi: LOW_MEGABYTE,
        flags: Flags::kernel_data(),
        guarded: true,
    });

    let Some(info) = super::machine::boot_info(crate::DTB.load(Ordering::Relaxed)) else {
        return;
    };
    let ceiling = firmware_fill_ceiling(|span| {
        for i in 0..info.memmap_entries as usize {
            let Some(e) = super::machine::memory_map_entry(&info, i) else {
                break;
            };
            span(
                e.addr & !(PAGE_SIZE - 1),
                e.end().next_multiple_of(PAGE_SIZE),
            );
        }
    });

    for i in 0..info.memmap_entries as usize {
        let Some(e) = super::machine::memory_map_entry(&info, i) else {
            break;
        };
        if e.is_usable_ram() {
            continue; // step 1 already did these, and did them minus the kernel image
        }
        let start = (e.addr & !(PAGE_SIZE - 1)).max(LOW_MEGABYTE);
        let end = e.end().next_multiple_of(PAGE_SIZE).min(ceiling);
        each(Claim {
            what: "firmware reservation",
            lo: start,
            hi: end,
            flags: Flags::kernel_data(),
            guarded: true,
        });
    }

    // The initial ramdisk, by its recorded bounds, however the memmap classified its bytes: the
    // reserved entry above the top of RAM can widen over the ramdisk's tail. `bring_up_memory`
    // already claimed the range as forbidden, and a guarded claim is idempotent. See
    // notes/x86-port/kernel-page-tables.md.
    if let Some((istart, isize)) = memory::initial_ramdisk_region() {
        each(Claim {
            what: "initial ramdisk",
            lo: istart & !(PAGE_SIZE - 1),
            hi: (istart + isize).next_multiple_of(PAGE_SIZE),
            flags: Flags::kernel_data(),
            guarded: true,
        });
    }
}
