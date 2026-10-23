//! PCIe enumeration and virtio-pci bring-up. Not a driver.
//!
//! The kernel walks the bus, finds a device, brings its register blocks up and hands a userspace
//! driver a confined transport capability. The decode logic (ECAM addressing, BAR sizing,
//! capability parsing) lives in the host-tested `pci` crate; this module supplies the volatile
//! accessors into the mapped ECAM window and the policy: which device, where BARs go, which command
//! bits to set. Every window comes through `memory::pci_regions()`, and a machine that names none
//! answers every probe with "nobody home", no MMIO touched. How a function's interrupt is chosen
//! is the machine's answer (notes/pcie.md, "Placing BARs, and routing interrupts"). See
//! notes/pcie.md and notes/pcie-transport-scope.md.

use core::sync::atomic::{AtomicU16, AtomicU64, Ordering};

use paging::PAGE_SIZE;
use pci::{Bar, Bdf, VirtioCap};

use crate::arch::mmu::{self, PCI_BAR_MAPPED, PCI_ECAM_BUSES, PCI_IRQ_BASE};

/// One bus of ECAM configuration space: 32 devices of 8 functions of 4 KiB.
const BUS_BYTES: u64 = 0x10_0000;

/// How many buses of configuration space this kernel has mapped, and may therefore read. Zero
/// means no survey has run and [`ecam_buses`] answers the architecture's compiled-in default.
/// A runtime value because a controller behind a root port needs the bus behind that port mapped,
/// and nothing compiled in knows which bus that is (notes/pcie.md, "More than one bus").
static ECAM_BUSES: AtomicU16 = AtomicU16::new(0);

/// The ECAM window's physical base, cached from `memory::pci_regions` by
/// [`is_host_bridge_present`]. Zero means "not cached yet, or no bridge", and cannot collide with a
/// real value: no machine puts config space at physical zero (that is RAM or a vector table
/// everywhere this runs).
static ECAM_BASE: AtomicU64 = AtomicU64::new(0);

/// The shared bump cursor for kernel-assigned BARs, starting at the discovered 32-bit memory
/// window's base. One cursor that only advances, because every function the kernel places (the
/// virtio devices, and on riscv the IOMMU) draws from it and two cursors would hand out one address
/// twice. Enumeration runs on the boot hart alone; the atomics are not for contention.
static BAR_NEXT: AtomicU64 = AtomicU64::new(0);

/// One past the last byte a BAR may occupy: the discovered window base plus the mapped slice
/// (`mmu::map_everything` maps the same `PCI_BAR_MAPPED`-capped slice, so this limit is exactly
/// what is addressable).
static BAR_LIMIT: AtomicU64 = AtomicU64::new(0);

/// The exclusive bus-number bound every walk in this module passes to `pci::walk`: how many buses
/// of configuration space are mapped, so how many can be read without faulting.
///
/// Returns the compiled-in `PCI_ECAM_BUSES` until [`survey`] has run, and what the survey found
/// afterwards. On both `virt` boards no survey runs and this is 1.
pub(crate) fn ecam_buses() -> u16 {
    match ECAM_BUSES.load(Ordering::Relaxed) {
        0 => PCI_ECAM_BUSES,
        n => n,
    }
}

/// Bytes of ECAM the direct map must cover, for each architecture's `map_everything`.
///
/// The caller still clamps against the window the machine described: a survey cannot report a bus
/// outside it, but `PCI_ECAM_BUSES` on a machine that describes less than one bus could.
///
/// Name: provisional (milestone 320 (every PCI bus the machine has)), every name listed here: this,
/// [`ecam_buses`], [`survey`] and `NIFE_PCIE_ROOT_PORT` in `helpers/qemu-runner-x86_64.sh`. calef
/// names public items.
pub fn ecam_bytes() -> u64 {
    u64::from(ecam_buses()) * BUS_BYTES
}

/// Print every function on every bus the machine has, and record how many buses that is.
/// Returns the number of functions found.
///
/// Runs before the fine map replaces the boot map, while every bus the MCFG describes is readable;
/// called later it would read off the end of the mapped window. It prints the list rather than a
/// count, because a missing disk hides in an ordinary count. The bus range it records is
/// contiguous from 0, not the set found (notes/pcie.md, "More than one bus", and its `BUGS`).
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub(crate) fn survey() -> usize {
    if !is_host_bridge_present() {
        return 0;
    }
    // The whole window the machine described, which is what is readable right now and is never
    // what stays mapped. `pci::walk` clamps this to 256 itself.
    let described =
        crate::memory::pci_regions().map_or(0, |((_, size), _)| (size / BUS_BYTES).min(256) as u16);

    let (mut functions, mut highest) = (0usize, 0u8);
    pci::walk(described, &mut |b, o| cfg_read32(b, o), &mut |f| {
        functions += 1;
        highest = highest.max(f.bdf.bus);
        match f.bridge {
            Some(b) => {
                highest = highest.max(b.subordinate);
                crate::println!(
                    "                {:02x}:{:02x}.{} {:04x}:{:04x} class {:06x} \
                     bridge to buses {:02x}..={:02x}",
                    f.bdf.bus,
                    f.bdf.dev,
                    f.bdf.func,
                    f.vendor,
                    f.device,
                    f.class,
                    b.secondary,
                    b.subordinate,
                );
            }
            None => crate::println!(
                "                {:02x}:{:02x}.{} {:04x}:{:04x} class {:06x}",
                f.bdf.bus,
                f.bdf.dev,
                f.bdf.func,
                f.vendor,
                f.device,
                f.class,
            ),
        }
    });

    // `highest` takes a bridge's SUBORDINATE bus and not only the buses that answered, because a
    // subtree firmware left room in is a subtree a hotplugged device appears in, and because a
    // bridge whose secondary bus is empty still decodes it.
    let buses = u16::from(highest) + 1;
    ECAM_BUSES.store(buses.min(described.max(1)), Ordering::Relaxed);
    crate::println!(
        "                {functions} function(s) over {} bus(es) of the {described} described; \
         mapping {} KiB of config space",
        buses,
        ecam_bytes() / 1024,
    );
    functions
}

/// True when `memory::pci_regions()` named a host bridge, caching its windows on first call.
/// Every public entry point checks this before touching config space, so a machine with nothing
/// recorded (the JH7110's tree, an x86 machine with no MCFG) reports nobody home with no MMIO
/// access. Single-hart boot-path code (see `BAR_NEXT`); the store order (cursor and limit before
/// the base that stands for "present") is documentation, not synchronization.
fn is_host_bridge_present() -> bool {
    if ECAM_BASE.load(Ordering::Relaxed) != 0 {
        return true;
    }
    let Some(((ecam, _), (bar, bar_size))) = crate::memory::pci_regions() else {
        return false;
    };
    BAR_NEXT.store(bar, Ordering::Relaxed);
    BAR_LIMIT.store(bar + PCI_BAR_MAPPED.min(bar_size), Ordering::Relaxed);
    ECAM_BASE.store(ecam, Ordering::Relaxed);
    true
}

/// Place every unassigned BAR of `bdf` at a size-aligned address drawn from the shared cursor,
/// writing the config-space BAR registers. Returns false (after saying so) if the window is
/// exhausted.
///
/// A BAR already inside the window `mmu::map_everything` mapped is left alone; a nonzero one
/// outside it is adopted where it stands ([`adopt`]); only one that is unassigned, or assigned
/// somewhere this kernel cannot honour, is moved. Nonzero alone is not evidence of mapped: see
/// notes/pcie.md, "Placing BARs, and routing interrupts".
fn place_bars(bdf: Bdf, bars: &mut [Option<Bar>; 6]) -> bool {
    // The window `mmu::map_everything` mapped. `BAR_LIMIT` is the same `PCI_BAR_MAPPED` clamp that
    // call computed, so it agrees even where the tree describes a larger window.
    let mapped_lo = crate::memory::pci_regions().map(|(_, (base, _))| base);
    let mapped_hi = BAR_LIMIT.load(Ordering::Relaxed);

    for (i, bar) in bars.iter_mut().enumerate() {
        let Some(bar) = bar.as_mut() else { continue };
        if bar.base != 0
            && let Some(lo) = mapped_lo
            && bar.base >= lo
            && bar.base + bar.size <= mapped_hi
        {
            continue; // already placed, and inside memory this kernel actually mapped
        }
        // The fine map covers only the kernel's own window, so adoption must map the BAR itself.
        if bar.base != 0 && adopt(bdf, i, bar) {
            continue;
        }
        // A BAR's address must be aligned to its size (the writable-bits mask encodes that).
        let align = bar.size.max(0x10);
        let base = loop {
            let cur = BAR_NEXT.load(Ordering::Relaxed);
            let base = cur.next_multiple_of(align);
            if base + bar.size > BAR_LIMIT.load(Ordering::Relaxed) {
                crate::println!("  pci: BAR window exhausted; cannot place BAR{i}");
                return false;
            }
            if BAR_NEXT
                .compare_exchange(cur, base + bar.size, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break base;
            }
        };
        let off = pci::BAR0 + i as u64 * 4;
        cfg_write32(bdf, off, base as u32 | if bar.is_64 { 0b100 } else { 0 });
        if bar.is_64 {
            cfg_write32(bdf, off + 4, (base >> 32) as u32);
        }
        bar.base = base;
    }
    true
}

/// Does `lo..hi` overlap any region the machine called usable RAM? The one safety question
/// adoption turns on (notes/pcie.md, "Placing BARs, and routing interrupts").
fn overlaps_ram(lo: u64, hi: u64) -> bool {
    crate::memory::ram_regions().any(|(start, size)| lo < start + size && hi > start)
}

/// Take a BAR the machine already placed, and map it where it stands. True when it was adopted,
/// false when the caller must place it instead.
///
/// Refuses a BAR that overlaps RAM, and one with a page that fails to map for any reason other than
/// `AlreadyMapped`, which is success here and nowhere else in this tree. Unreachable on the
/// device-tree architectures, where every BAR arrives zero. See notes/pcie.md, "Placing BARs, and
/// routing interrupts".
fn adopt(bdf: Bdf, index: usize, bar: &pci::Bar) -> bool {
    let lo = bar.base & !(PAGE_SIZE - 1);
    let hi = (bar.base + bar.size).next_multiple_of(PAGE_SIZE);
    if overlaps_ram(lo, hi) {
        crate::println!(
            "  pci: {:02x}:{:02x}.{} BAR{index} at {:#x} overlaps RAM; moving it",
            bdf.bus,
            bdf.dev,
            bdf.func,
            bar.base,
        );
        return false;
    }
    let mut pa = lo;
    while pa < hi {
        match mmu::map_page(mmu::phys_to_virt(pa), pa, paging::Flags::device()) {
            Ok(()) | Err(paging::MapError::AlreadyMapped) => {}
            Err(e) => {
                crate::println!(
                    "  pci: {:02x}:{:02x}.{} BAR{index} at {:#x} could not be mapped ({e:?}); \
                     moving it",
                    bdf.bus,
                    bdf.dev,
                    bdf.func,
                    bar.base,
                );
                return false;
            }
        }
        pa += PAGE_SIZE;
    }
    true
}

/// Callers sit behind [`is_host_bridge_present`], which is what makes the cached base nonzero and
/// the window below it device-mapped.
fn cfg_read32(bdf: Bdf, off: u64) -> u32 {
    let va = mmu::phys_to_virt(ECAM_BASE.load(Ordering::Relaxed) + bdf.ecam_offset() + (off & !3));
    // SAFETY: the ECAM window for the buses we enumerate is device-mapped (mmu::map_everything's
    // PCIe step maps it whenever the tree described one), and ECAM config reads are
    // side-effect-free.
    unsafe { core::ptr::read_volatile(va as *const u32) }
}

/// **A bridge's `(secondary, subordinate)` bus numbers**, or `None` when `bus:dev.func` is absent
/// or is not a bridge. The live-bus half of resolving a DMAR device-scope path, which is firmware
/// naming a device by the route to it rather than by its number (milestone 261 (the NVMe driver leaves the kernel)'s bench rehearsal;
/// `machine_discovery::acpi::DmarUnits::owner`).
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub(crate) fn bridge_bus_range(bus: u8, dev: u8, func: u8) -> Option<(u8, u8)> {
    if !is_host_bridge_present() || dev > 31 || func > 7 {
        return None;
    }
    let bdf = Bdf { bus, dev, func };
    if cfg_read32(bdf, pci::VENDOR_ID) & 0xffff == 0xffff {
        return None;
    }
    let header = (cfg_read32(bdf, pci::HEADER_TYPE & !3) >> ((pci::HEADER_TYPE & 3) * 8)) as u8;
    if header & 0x7f != pci::HEADER_TYPE_BRIDGE {
        return None;
    }
    let b = pci::bridge_buses(bdf, &mut |b, o| cfg_read32(b, o));
    Some((b.secondary, b.subordinate))
}

/// Bench diagnostic for fatal risk 6's CompletionTimeout on xenon (2026-10-04). The command
/// and status words of the NVMe function `rid` and of every bridge on bus 0 whose range holds its
/// bus. A root port with Bus-Master clear forwards no upstream DMA, and this kernel sets
/// Bus-Master on the endpoint only; status bits name an abort the endpoint saw. Two short lines,
/// at most, so the photograph holds them.
#[cfg(feature = "disk_throughput")]
pub(crate) fn print_dma_path(rid: u32) {
    let ep = Bdf {
        bus: (rid >> 8) as u8,
        dev: ((rid >> 3) & 0x1f) as u8,
        func: (rid & 7) as u8,
    };
    let w = cfg_read32(ep, pci::COMMAND);
    crate::println!(
        "diag pci  {:02x}:{:02x}.{} cmd {:#06x} sts {:#06x} (bme {})",
        ep.bus,
        ep.dev,
        ep.func,
        w & 0xffff,
        w >> 16,
        w >> 2 & 1
    );
    for dev in 0..32u8 {
        for func in 0..8u8 {
            let Some((sec, sub)) = bridge_bus_range(0, dev, func) else {
                continue;
            };
            if ep.bus == 0 || ep.bus < sec || ep.bus > sub {
                continue;
            }
            let b = Bdf { bus: 0, dev, func };
            let w = cfg_read32(b, pci::COMMAND);
            // Secondary status is the upper half of the dword at 0x1c (I/O base and limit below).
            let sec_sts = cfg_read32(b, 0x1c) >> 16;
            crate::println!(
                "diag pci  bridge 00:{dev:02x}.{func} cmd {:#06x} sts {:#06x} secsts {sec_sts:#06x} (bme {})",
                w & 0xffff,
                w >> 16,
                w >> 2 & 1
            );
        }
    }
}

fn cfg_write32(bdf: Bdf, off: u64, v: u32) {
    let va = mmu::phys_to_virt(ECAM_BASE.load(Ordering::Relaxed) + bdf.ecam_offset() + (off & !3));
    // SAFETY: as above; config writes go to the one function this bdf names.
    unsafe { core::ptr::write_volatile(va as *mut u32, v) }
}

/// A modern virtio device on the PCI bus, brought up: every register block resolved to a
/// physical address, memory decoding and bus mastering enabled, its INTx line known. Device-neutral
/// (a disk and a NIC differ only in the driver above this), so both `find_block_device` and
/// `find_net_device` return it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PciVirtioDevice {
    /// Which function this is, for the boot tours' prints; every other build (tests, the shell
    /// and bench boots) drives the device without ever naming it, so the lint is quieted rather
    /// than chased through four cfg combinations.
    #[allow(dead_code)]
    pub bdf: Bdf,
    /// The virtio common-config block (queue setup, status, features), physical.
    pub common: u64,
    /// The notify region base and the per-queue multiplier; queue N's doorbell is at
    /// `notify_base + queue_notify_off(N) * notify_mult`.
    pub notify_base: u64,
    pub notify_mult: u32,
    /// The ISR byte (read-to-ack), physical.
    pub isr: u64,
    /// The interrupt a driver binds and waits on: the PLIC/GIC input the INTx pin swizzles to (see
    /// `pci::intx_irq`) on the two device-tree boards, the MSI-X vector
    /// `arch::irq::alloc_msi_vector` reserved on `x86_64`. An `Irq` capability names it and the arch
    /// layer resolves it.
    pub intid: u32,
    /// Which MSI-X table entry [`intid`](Self::intid) was programmed into, or
    /// `pci::VIRTIO_MSIX_NO_VECTOR` when this function delivers INTx (both `virt` boards). It
    /// travels with the transport because a virtio queue is told its vector at queue-setup time, in
    /// `virtio::setup_queue`, and a device reset clears it.
    pub msix_vector: u16,
    /// The PCIe requester id (`bus:8 | dev:5 | fn:3`), the id the IOMMU keys its per-device tables
    /// on (milestone 16b). Carried through to `virtio::register` so the device is confined to its
    /// DMA region in hardware.
    pub rid: u32,
    /// The virtio device type (2 = block, 1 = net, 16 = gpu), recovered from the PCI device id
    /// (`0x1040 + type`). The transport answers a driver's `DeviceID` read from it, since PCI config
    /// space has no such register.
    pub device_type: u32,
    /// The device-specific config region (virtio spec 4.1.4.4's `DEVICE_CFG` capability), physical,
    /// when the function carries one. A blk function keeps its capacity there (eight bytes at
    /// offset 0, what `blk::SIZE` answers from); net, rng and console carry `None`. The mmio
    /// transport's config area is at a fixed offset (0x100) instead.
    pub device_config: Option<u64>,
}

/// Find the first modern virtio-blk function on the bus and bring it up. `None` if there is no
/// PCI disk (an empty bus reads all-ones and enumerates nothing).
pub(crate) fn find_block_device() -> Option<PciVirtioDevice> {
    let bdf = find_virtio_bdf(
        pci::VIRTIO_BLK_MODERN,
        Some(pci::VIRTIO_BLK_TRANSITIONAL),
        "virtio-blk",
    )?;
    bring_up(bdf, pci::VIRTIO_TYPE_BLOCK)
}

/// Find the `n`-th (0-based) modern virtio-blk function on the bus and bring it up, in
/// bus:device.function order. `None` if the bus has no such function.
///
/// The PCI half of [`crate::virtio::find_block_device_n`]'s ordering, for `q35`, which has no
/// virtio-mmio bus. [`find_block_device`] stays separate because the PCIe transport tests
/// (`virtio_service::start_role_pci`) want "the PCI disk", not an ordinal across both buses.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // the mmio half answers first on both virt boards
pub(crate) fn find_block_device_n(n: usize) -> Option<PciVirtioDevice> {
    if !is_host_bridge_present() {
        return None;
    }
    let mut seen = 0;
    let mut found: Option<Bdf> = None;
    pci::enumerate(
        ecam_buses(),
        &mut |b, o| cfg_read32(b, o),
        &mut |bdf, vendor, device| {
            if found.is_none() && vendor == pci::VIRTIO_VENDOR && device == pci::VIRTIO_BLK_MODERN {
                if seen == n {
                    found = Some(bdf);
                }
                seen += 1;
            }
        },
    );
    bring_up(found?, pci::VIRTIO_TYPE_BLOCK)
}

/// Find the first modern virtio-net function on the bus and bring it up. `None` if there is no
/// PCI NIC. Same bring-up as the disk; the transport seam and the DMA confinement do not know or
/// care that a NIC sits behind them (milestone 30).
pub(crate) fn find_net_device() -> Option<PciVirtioDevice> {
    let bdf = find_virtio_bdf(
        pci::VIRTIO_NET_MODERN,
        Some(pci::VIRTIO_NET_TRANSITIONAL),
        "virtio-net",
    )?;
    bring_up(bdf, pci::VIRTIO_TYPE_NET)
}

/// Find the first modern virtio-gpu function on the bus and bring it up (milestone 29, the display
/// ladder's first rung). `None` if there is no PCI GPU.
///
/// PCIe only: no configuration we attach puts a virtio-gpu on either `virt` machine's mmio bus.
/// Same bring-up and IOMMU confinement as the disk.
pub(crate) fn find_gpu_device() -> Option<PciVirtioDevice> {
    // No transitional twin exists for virtio-gpu (the legacy id range predates the device), so this
    // asks for the modern id and nothing else rather than warning about a legacy device that cannot
    // be on the bus.
    let bdf = find_virtio_bdf(pci::VIRTIO_GPU_MODERN, None, "virtio-gpu")?;
    bring_up(bdf, pci::VIRTIO_TYPE_GPU)
}

/// Find the first modern virtio-input function on the bus and bring it up (milestone 29's keyboard).
/// `None` if there is no PCI input device.
///
/// PCIe only by choice, so the keyboard sits behind the IOMMU the way the GPU does (notes/pcie.md,
/// "Placing BARs, and routing interrupts").
pub(crate) fn find_input_device() -> Option<PciVirtioDevice> {
    let bdf = find_virtio_bdf(pci::VIRTIO_INPUT_MODERN, None, "virtio-input")?;
    bring_up(bdf, pci::VIRTIO_TYPE_INPUT)
}

/// Find the first modern virtio-rng function on the bus and bring it up (milestone 56's entropy
/// source). `None` if there is no PCI RNG.
///
/// The mmio twin is [`crate::virtio::find_entropy_device`]; the entropy service is wired over
/// whichever the wiring asks for, and the milestone-56 test runs each.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // entropy_service is the caller, and the m56 tests drive it
pub(crate) fn find_rng_device() -> Option<PciVirtioDevice> {
    let bdf = find_virtio_bdf(
        pci::VIRTIO_RNG_MODERN,
        Some(pci::VIRTIO_RNG_TRANSITIONAL),
        "virtio-rng",
    )?;
    bring_up(bdf, pci::VIRTIO_TYPE_ENTROPY)
}

/// How many modern virtio-blk functions are on the bus (milestone 57's roster).
///
/// Reads config space and nothing else: bring-up sizes BARs and enables decoding, which would
/// disturb whichever driver owns the function (`crates/block_roster`).
pub fn count_block_devices() -> usize {
    if !is_host_bridge_present() {
        return 0;
    }
    let mut n = 0;
    pci::enumerate(
        ecam_buses(),
        &mut |b, o| cfg_read32(b, o),
        &mut |_, vendor, device| {
            if vendor == pci::VIRTIO_VENDOR && device == pci::VIRTIO_BLK_MODERN {
                n += 1;
            }
        },
    );
    n
}

/// A read-only census of what the machine left on the bus, provisional name.
///
/// Returns `(functions, stranded)`: how many functions answer, and how many carry a memory BAR this
/// kernel can neither use where it is nor adopt. Zero stranded is the passing answer. Counting
/// only, like [`count_block_devices`] and for its reason. See notes/pcie.md, "Placing BARs, and
/// routing interrupts".
// The x86 boot tour is the only caller: on both `virt` boards every BAR arrives zero.
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub(crate) fn bar_census() -> (usize, usize) {
    if !is_host_bridge_present() {
        return (0, 0);
    }
    let lo = crate::memory::pci_regions().map_or(0, |(_, (base, _))| base);
    let hi = BAR_LIMIT.load(Ordering::Relaxed);
    let (mut functions, mut stranded) = (0usize, 0usize);
    pci::enumerate(
        ecam_buses(),
        &mut |b, o| cfg_read32(b, o),
        &mut |bdf, _, _| {
            functions += 1;
            // The raw BAR registers, not `pci::read_bars`, which sizes them by writing all-ones.
            // Unsized, so only a BAR's first page is checked against RAM; `adopt` refuses the rest.
            let stuck = (0..6).any(|i| {
                let raw = cfg_read32(bdf, pci::BAR0 + i * 4);
                let base = u64::from(raw & !0xf);
                raw & 1 == 0
                    && base != 0
                    && !(lo..hi).contains(&base)
                    && overlaps_ram(base, base + PAGE_SIZE)
            });
            if stuck {
                stranded += 1;
            }
        },
    );
    (functions, stranded)
}

/// Enumerate the bus for the first function matching `modern`, warning (once) if only a
/// `transitional` (legacy) twin is present, since we drive modern only. `kind` names the device for
/// that warning. `transitional` is `None` for a device type that has no legacy id at all
/// (virtio-gpu): there is then no such warning to give, and inventing an id to compare against
/// would be a fact nobody checked.
fn find_virtio_bdf(modern: u16, transitional: Option<u16>, kind: &str) -> Option<Bdf> {
    if !is_host_bridge_present() {
        return None;
    }
    let mut found: Option<Bdf> = None;
    pci::enumerate(
        ecam_buses(),
        &mut |b, o| cfg_read32(b, o),
        &mut |bdf, vendor, device| {
            if found.is_none() && vendor == pci::VIRTIO_VENDOR {
                if device == modern {
                    found = Some(bdf);
                } else if Some(device) == transitional {
                    crate::println!(
                        "  pci: {kind} at {:02x}:{:02x}.{} is transitional (legacy); \
                         we drive modern only",
                        bdf.bus,
                        bdf.dev,
                        bdf.func,
                    );
                }
            }
        },
    );
    found
}

/// Bring an enumerated virtio function up and resolve its transport, in this order:
/// 1. size BARs while memory decoding is off (the sizing dance writes the BARs);
/// 2. assign addresses to unassigned BARs (with `-bios default`, OpenSBI has done no PCI setup,
///    so every BAR arrives zero and the kernel is the firmware here);
/// 3. parse the virtio vendor capabilities and resolve them against the assigned BARs;
/// 4. only then set Memory-Space Enable, and Bus-Master last: DMA permission is granted at the
///    final moment, after the transport the confinement layer owns is fully described.
fn bring_up(bdf: Bdf, device_type: u32) -> Option<PciVirtioDevice> {
    let mut bars = pci::read_bars(bdf, &mut |b, o| cfg_read32(b, o), &mut |b, o, v| {
        cfg_write32(b, o, v);
    });
    if !place_bars(bdf, &mut bars) {
        return None;
    }

    let resolve = |bars: &[Option<Bar>; 6], capability: &VirtioCap| -> Option<u64> {
        let bar = bars.get(capability.bar as usize)?.as_ref()?;
        (u64::from(capability.offset) + u64::from(capability.length) <= bar.size)
            .then(|| bar.base + u64::from(capability.offset))
    };
    let (mut common, mut notify_base, mut notify_mult, mut isr) = (None, None, 0u32, None);
    let mut device_config = None;
    pci::virtio_caps(
        bdf,
        &mut |b, o| cfg_read32(b, o),
        &mut |capability| match capability.cfg_type {
            pci::VIRTIO_CAP_COMMON if common.is_none() => common = resolve(&bars, &capability),
            pci::VIRTIO_CAP_NOTIFY if notify_base.is_none() => {
                notify_base = resolve(&bars, &capability);
                notify_mult = capability.notify_off_multiplier;
            }
            pci::VIRTIO_CAP_ISR if isr.is_none() => isr = resolve(&bars, &capability),
            // Optional per the spec; absent stays `None` rather than failing bring-up.
            pci::VIRTIO_CAP_DEVICE if device_config.is_none() => {
                device_config = resolve(&bars, &capability);
            }
            _ => {}
        },
    );
    let (common, notify_base, isr) = (common?, notify_base?, isr?);

    // The upper half of this dword is STATUS, whose error bits are write-1-to-clear, so it is
    // written as zero.
    let cmd = cfg_read32(bdf, pci::COMMAND) as u16;
    cfg_write32(
        bdf,
        pci::COMMAND,
        (cmd | pci::CMD_MEMORY_SPACE | pci::CMD_BUS_MASTER) as u32,
    );

    let (intid, msix_vector) = function_interrupt(bdf, &bars, false)?;

    Some(PciVirtioDevice {
        bdf,
        common,
        notify_base,
        notify_mult,
        isr,
        intid,
        msix_vector,
        rid: bdf.requester_id(),
        device_type,
        device_config,
    })
}

/// How this function's interrupt reaches a driver: `(intid, msix_vector)`. MSI-X where
/// `arch::irq::alloc_msi_vector` answers `Some`, the INTx swizzle where it answers `None`.
///
/// `msi_fallback` lets a function with MSI and no MSI-X (the xHCI on an Intel PCH) take plain MSI;
/// virtio functions pass `false`, since a virtio queue names its vector by MSI-X table index.
/// `msix_vector` is `pci::VIRTIO_MSIX_NO_VECTOR` whenever the answer was not MSI-X. `None` when no
/// route exists. See notes/pcie.md, "Placing BARs, and routing interrupts".
fn function_interrupt(bdf: Bdf, bars: &[Option<Bar>; 6], msi_fallback: bool) -> Option<(u32, u16)> {
    match crate::arch::irq::alloc_msi_vector() {
        Some((intid, target)) => {
            // No fallback to INTx: on a machine that answered `Some`, `PCI_IRQ_BASE` is 0 and the
            // swizzle would resolve to the PIT's line, arming the driver on the timer.
            if msi_fallback
                && pci::msix_cap(bdf, &mut |b, o| cfg_read32(b, o)).is_none()
                && program_msi(bdf, target)
            {
                return Some((intid, pci::VIRTIO_MSIX_NO_VECTOR));
            }
            let vector = program_msix(bdf, bars, target)?;
            Some((intid, vector))
        }
        None => Some((intx_intid(bdf)?, pci::VIRTIO_MSIX_NO_VECTOR)),
    }
}

/// Point this function's one MSI vector at `target` and enable it. `false`, having touched
/// nothing, when the function has no MSI capability.
///
/// Address and data first, enable last, so no half-written message is ever live; one vector
/// (Multiple Message Enable left 0), because one is what every driver here asks for.
fn program_msi(bdf: Bdf, target: pci::MsiTarget) -> bool {
    let Some(cap) = pci::msi_cap(bdf, &mut |b, o| cfg_read32(b, o)) else {
        return false;
    };
    cfg_write32(bdf, cap.address_low(), target.address as u32);
    if cap.is_64 {
        cfg_write32(bdf, cap.address_low() + 4, (target.address >> 32) as u32);
    }
    // The data register is 16 bits, and the dword holding it has the (64-bit) layout's reserved
    // half or the next register above it; a read-modify-write keeps whichever it is.
    let old = cfg_read32(bdf, cap.data());
    cfg_write32(
        bdf,
        cap.data(),
        (old & 0xffff_0000) | (target.data & 0xffff),
    );
    let head = cfg_read32(bdf, cap.cap_offset);
    let control = ((head >> 16) as u16 & !(0x7 << 4)) | pci::MSI_ENABLE;
    cfg_write32(
        bdf,
        cap.cap_offset,
        (head & 0xffff) | (control as u32) << 16,
    );
    true
}

/// The INTx path: read the function's interrupt pin and swizzle it onto this board's controller.
///
/// `None`, loudly, for a function that declares no pin at all, because on a board that routes INTx
/// a device with no pin has no way to interrupt anybody and a driver bound to whatever
/// `intx_irq(base, dev, 0)` returned would wait forever on a line nothing drives.
fn intx_intid(bdf: Bdf) -> Option<u32> {
    // config 0x3d; 1=INTA..4=INTD, 0=none. The dtb fixture test holds the swizzle against the
    // machine's own interrupt-map.
    let pin = ((cfg_read32(bdf, 0x3c) >> 8) & 0xff) as u8;
    if pin == 0 {
        crate::println!(
            "  pci: the virtio function at {:02x} declares no INTx pin",
            bdf.dev
        );
        return None;
    }
    Some(pci::intx_irq(PCI_IRQ_BASE, bdf.dev, pin))
}

/// Point entry 0 of this function's MSI-X table at `target` and arm it. Returns the table index
/// the device must be told to raise, or `None` (loudly) when the function has no usable table.
///
/// `bars` are the placed BARs: the table lives in the one its BIR names, and one that is unplaced
/// or too small is refused. The entry is written before MSI-X Enable, its mask cleared last, and
/// Function Mask cleared explicitly because some parts set it at reset. See notes/pcie.md,
/// "Placing BARs, and routing interrupts".
fn program_msix(bdf: Bdf, bars: &[Option<Bar>; 6], target: pci::MsiTarget) -> Option<u16> {
    let Some(cap) = pci::msix_cap(bdf, &mut |b, o| cfg_read32(b, o)) else {
        crate::println!(
            "  pci: the function at {:02x}:{:02x}.{} declares no MSI-X, and this machine has no \
             other route for a PCI interrupt",
            bdf.bus,
            bdf.dev,
            bdf.func,
        );
        return None;
    };

    // Entry 0: every device this tree attaches uses a single queue's completion.
    let bar = bars.get(cap.table_bar as usize)?.as_ref()?;
    let entry = u64::from(cap.table_offset);
    if entry + pci::MSIX_ENTRY_BYTES > bar.size {
        crate::println!(
            "  pci: the MSI-X table of the function at {:02x} does not fit its own BAR{}",
            bdf.dev,
            cap.table_bar,
        );
        return None;
    }
    let at = mmu::phys_to_virt(bar.base + entry);

    // SAFETY: `at` is inside BAR `cap.table_bar`, which `place_bars` put inside the window
    // `mmu::map_everything` maps as device memory, and the bounds check above keeps all sixteen
    // bytes inside it. The four words are the specified layout of one MSI-X table entry.
    unsafe {
        core::ptr::write_volatile(at as *mut u32, target.address as u32);
        core::ptr::write_volatile((at + 4) as *mut u32, (target.address >> 32) as u32);
        core::ptr::write_volatile((at + 8) as *mut u32, target.data);
        core::ptr::write_volatile((at + 12) as *mut u32, 0); // unmasked, last
    }

    // Message Control is the upper half of the capability's first dword, so it is written back
    // with the (read-only) id and next-pointer bytes underneath it, which is what a 32-bit config
    // write to a capability header means.
    let head = cfg_read32(bdf, cap.cap_offset);
    let control = ((head >> 16) as u16 | pci::MSIX_ENABLE) & !pci::MSIX_FUNCTION_MASK;
    cfg_write32(
        bdf,
        cap.cap_offset,
        (head & 0xffff) | ((control as u32) << 16),
    );

    Some(0)
}

/// Every function on the bus carrying the NVMe class code, in bus-walk order. One walk for the
/// driver and the roster, so the controller [`find_nvme_device`] brings up is always the one the
/// roster calls ordinal 0.
fn each_non_volatile_memory_express_controller(found: &mut dyn FnMut(Bdf)) {
    if !is_host_bridge_present() {
        return;
    }
    pci::enumerate(
        ecam_buses(),
        &mut |b, o| cfg_read32(b, o),
        &mut |bdf, _, _| {
            if cfg_read32(bdf, pci::CLASS_REVISION) >> 8 == pci::CLASS_NVME {
                found(bdf);
            }
        },
    );
}

/// How many NVMe controllers are on the bus, for milestone 421 (the block roster names an NVMe
/// disk).
///
/// Config space only, for [`count_block_devices`]'s reason: [`find_nvme_device`] places BARs and
/// enables bus mastering, and the kernel's bring-up then resets the controller, none of which a
/// listing may cause.
///
/// Name: provisional. Says controllers rather than disks because that is what the class code
/// identifies; one namespace per controller is what makes the two the same here.
pub(crate) fn count_non_volatile_memory_express_controllers() -> usize {
    let mut n = 0;
    each_non_volatile_memory_express_controller(&mut |_| n += 1);
    n
}

/// An enumerated, brought-up NVMe controller: its register file (BAR0) placed and decoding, bus
/// mastering enabled, requester id known so the caller can confine its DMA before enabling the
/// controller. No INTx line, on purpose: the milestone-53 driver completes by polling the phase
/// tag (`kernel/src/non_volatile_memory_express.rs`), so wiring an interrupt here would record a fact nothing checks.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PciNvmeDevice {
    /// The register file's physical base (BAR0; the doorbells live in it too).
    pub bar0: u64,
    /// The PCIe requester id, the key the IOMMU confines DMA by (as `PciVirtioDevice::rid`).
    pub rid: u32,
}

/// Find the first NVMe controller on the bus and bring its transport up: size and place its BARs,
/// resolve BAR0, and enable memory decoding and bus mastering. `None` if no function on the bus
/// carries the NVMe class code.
///
/// Same bring-up order as [`bring_up`], for the same reasons; what it does not share is the virtio
/// capability walk (NVMe's register layout is fixed by its own spec, not described by vendor
/// capabilities) and the INTx resolution (the driver polls). Bus-Master last, as everywhere: DMA
/// permission is granted at the final moment, and on a machine with an IOMMU the device still
/// cannot reach a byte until `iommu::confine` maps its region (denied by default, milestone 16b).
pub(crate) fn find_nvme_device() -> Option<PciNvmeDevice> {
    let mut found: Option<Bdf> = None;
    each_non_volatile_memory_express_controller(&mut |bdf| {
        if found.is_none() {
            found = Some(bdf);
        }
    });
    let bdf = found?;

    let mut bars = pci::read_bars(bdf, &mut |b, o| cfg_read32(b, o), &mut |b, o, v| {
        cfg_write32(b, o, v);
    });
    if !place_bars(bdf, &mut bars) {
        return None;
    }
    let bar0 = bars[0].as_ref().map(|b| b.base)?;

    let cmd = cfg_read32(bdf, pci::COMMAND) as u16;
    cfg_write32(
        bdf,
        pci::COMMAND,
        (cmd | pci::CMD_MEMORY_SPACE | pci::CMD_BUS_MASTER) as u32,
    );

    Some(PciNvmeDevice {
        bar0,
        rid: bdf.requester_id(),
    })
}

/// An enumerated, brought-up xHCI controller: its register
/// file placed and decoding, bus mastering on, its interrupt resolved, and the byte ranges of BAR0
/// its MSI-X structures occupy, which the driver must never be mapped.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PciXhciDevice {
    /// The register file's physical base (BAR0).
    pub bar0: u64,
    /// BAR0's size in bytes.
    pub bar_bytes: u64,
    /// The PCIe requester id the IOMMU confines DMA by.
    pub rid: u32,
    /// The interrupt a driver binds and waits on, as [`PciVirtioDevice::intid`].
    pub intid: u32,
    /// The MSI-X table and pending-bit array, as byte ranges into BAR0, `(0, 0)` where one is
    /// absent or lives in another BAR. Withheld from the driver's mapping.
    pub withheld: [(u64, u64); 2],
}

/// Find the first xHCI controller on the bus and bring its transport up, the
/// [`find_nvme_device`] shape plus an interrupt: size and place the BARs, enable memory decoding and
/// bus mastering, resolve how its interrupt arrives. `None` if no function carries the xHCI class
/// code, or one does and has no BAR0 or no interrupt this machine can route.
///
/// Bus-Master before confinement, as [`find_nvme_device`] does and for its reason: on a machine
/// with an IOMMU the controller cannot reach a byte until `iommu::confine` maps its region.
pub(crate) fn find_xhci_device() -> Option<PciXhciDevice> {
    if !is_host_bridge_present() {
        return None;
    }
    let mut found: Option<Bdf> = None;
    pci::enumerate(
        ecam_buses(),
        &mut |b, o| cfg_read32(b, o),
        &mut |bdf, _, _| {
            if found.is_none() && cfg_read32(bdf, pci::CLASS_REVISION) >> 8 == pci::CLASS_XHCI {
                found = Some(bdf);
            }
        },
    );
    let bdf = found?;

    let mut bars = pci::read_bars(bdf, &mut |b, o| cfg_read32(b, o), &mut |b, o, v| {
        cfg_write32(b, o, v);
    });
    if !place_bars(bdf, &mut bars) {
        return None;
    }
    let bar = bars[0].as_ref()?;
    let (bar0, bar_bytes) = (bar.base, bar.size);

    let cmd = cfg_read32(bdf, pci::COMMAND) as u16;
    cfg_write32(
        bdf,
        pci::COMMAND,
        (cmd | pci::CMD_MEMORY_SPACE | pci::CMD_BUS_MASTER) as u32,
    );

    let (intid, _) = function_interrupt(bdf, &bars, true)?;

    // The MSI-X table's and pending-bit array's byte ranges in BAR0, whether or not this machine
    // uses MSI-X: a driver that could write the table could enable and aim it, whatever this
    // kernel chose.
    let mut withheld = [(0, 0); 2];
    if let Some(cap) = pci::msix_cap(bdf, &mut |b, o| cfg_read32(b, o)) {
        if cap.table_bar == 0 {
            let at = u64::from(cap.table_offset);
            withheld[0] = (at, at + u64::from(cap.table_size) * pci::MSIX_ENTRY_BYTES);
        }
        if cap.pba_bar == 0 {
            let at = u64::from(cap.pba_offset);
            // One bit per entry, in whole quadwords.
            withheld[1] = (at, at + u64::from(cap.table_size).div_ceil(64) * 8);
        }
    }

    Some(PciXhciDevice {
        bar0,
        bar_bytes,
        rid: bdf.requester_id(),
        intid,
        withheld,
    })
}

/// An enumerated, brought-up `e1000e`-family NIC (milestone 494 (a driver for the network card a
/// PC actually has)): BAR0 placed and decoding, bus mastering on, the requester id known so the
/// caller can confine its DMA before the device is told where any ring is. No INTx line, for the
/// reason [`PciNvmeDevice`] has none: the data plane polls.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PciE1000eDevice {
    /// The register file's physical base (BAR0, 128 KiB on the 82574L).
    pub bar0: u64,
    /// The PCIe requester id, the key the IOMMU confines DMA by.
    pub rid: u32,
    /// The PCI device id, which says which part this is (`e1000e::model`).
    pub device: u16,
    bdf: Bdf,
}

impl PciE1000eDevice {
    /// Read a dword of this function's configuration space. The I219's descriptor-ring flush
    /// reads its status word at `0xe4` (`e1000e::pch::flush`), which is the one reason a NIC
    /// driver here needs configuration space after bring-up.
    pub(crate) fn config_read32(&self, off: u64) -> u32 {
        cfg_read32(self.bdf, off)
    }
}

/// Find the first function `e1000e::is_supported` claims and bring its transport up, in
/// [`find_nvme_device`]'s order and for its reasons: BARs placed, then memory decoding and bus
/// mastering last. `None` if no such function is on the bus.
///
/// The same shape as `find_nvme_device` with a different predicate; a shared
/// `find_function(predicate)` would replace the copies.
pub(crate) fn find_e1000e_device() -> Option<PciE1000eDevice> {
    if !is_host_bridge_present() {
        return None;
    }
    let mut found: Option<(Bdf, u16)> = None;
    pci::enumerate(
        ecam_buses(),
        &mut |b, o| cfg_read32(b, o),
        &mut |bdf, vendor, device| {
            if found.is_none() && ::e1000e::is_supported(vendor, device) {
                found = Some((bdf, device));
            }
        },
    );
    let (bdf, device) = found?;

    let mut bars = pci::read_bars(bdf, &mut |b, o| cfg_read32(b, o), &mut |b, o, v| {
        cfg_write32(b, o, v);
    });
    if !place_bars(bdf, &mut bars) {
        return None;
    }
    let bar0 = bars[0].as_ref().map(|b| b.base)?;

    let cmd = cfg_read32(bdf, pci::COMMAND) as u16;
    cfg_write32(
        bdf,
        pci::COMMAND,
        (cmd | pci::CMD_MEMORY_SPACE | pci::CMD_BUS_MASTER) as u32,
    );

    Some(PciE1000eDevice {
        bar0,
        rid: bdf.requester_id(),
        device,
        bdf,
    })
}

/// The QEMU `riscv-iommu-pci` function's PCI identity (Red Hat vendor, RISC-V IOMMU device id).
/// riscv-only: aarch64's SMMUv3 is a device-tree platform node, not a PCI function, and x86's
/// IOMMUs are discovered through ACPI, so a PCI-function IOMMU is a RISC-V shape today.
#[cfg(target_arch = "riscv64")]
const IOMMU_VENDOR: u16 = 0x1b36;
#[cfg(target_arch = "riscv64")]
const IOMMU_DEVICE: u16 = 0x0014;

/// Bring up the RISC-V IOMMU, if the machine has one. The ratified RISC-V IOMMU is a PCI function:
/// the kernel places its BAR0 (the register file) from the shared cursor, enables memory-space
/// decoding and hands the base to the arch driver's `init`. No such function on the bus, no-op.
/// No Bus-Master: the IOMMU polices DMA and does not initiate it.
#[cfg(target_arch = "riscv64")]
pub(crate) fn init_iommu() {
    if !is_host_bridge_present() {
        return;
    }
    let mut found: Option<Bdf> = None;
    pci::enumerate(
        ecam_buses(),
        &mut |b, o| cfg_read32(b, o),
        &mut |bdf, vendor, device| {
            if found.is_none() && vendor == IOMMU_VENDOR && device == IOMMU_DEVICE {
                found = Some(bdf);
            }
        },
    );
    let Some(bdf) = found else { return };

    let mut bars = pci::read_bars(bdf, &mut |b, o| cfg_read32(b, o), &mut |b, o, v| {
        cfg_write32(b, o, v);
    });
    if !place_bars(bdf, &mut bars) {
        crate::println!("  pci: could not place the IOMMU's BAR; leaving the IOMMU off");
        return;
    }
    let Some(base) = bars[0].as_ref().map(|b| b.base) else {
        crate::println!("  pci: the IOMMU function has no BAR0; leaving the IOMMU off");
        return;
    };

    let cmd = cfg_read32(bdf, pci::COMMAND) as u16;
    cfg_write32(bdf, pci::COMMAND, (cmd | pci::CMD_MEMORY_SPACE) as u32);

    crate::arch::iommu::init(base);
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "x86_64")]
    use super::*;

    /// The discovered PCIe windows are the machine's own, and on QEMU `virt` they equal the old
    /// `PCI_ECAM_BASE` and `PCI_BAR_BASE` constants. A fresh parse of the live DTB must agree with
    /// what `memory::init` recorded. The JH7110's answer (no generic-ECAM node, no window) is
    /// witnessed on the host (`crates/pci/tests/qemu_virt_dtb.rs`). Not on `x86_64`, which has no
    /// device tree; see [`acpi_mcfg_wires_a_real_ecam_window_that_finds_the_host_bridge`].
    #[cfg(not(target_arch = "x86_64"))]
    #[test_case]
    fn the_discovered_pci_windows_are_the_machines_own_and_match_the_old_constants() {
        let Some((ecam, mem32)) = crate::memory::pci_regions() else {
            // The JH7110 has no generic-ECAM node at all (comment above).
            crate::testing::skip!(
                "no pci-host-ecam-generic bridge in the device tree (expected on the JH7110)"
            );
        };

        let dt = crate::device_tree().expect("device tree is unreadable");
        let mut regs = [device_tree_blob::Region { start: 0, size: 0 }; 1];
        let n = dt
            .node_reg_compatible(b"pci-host-ecam-generic", &mut regs)
            .expect("the bridge's reg parses");
        assert_eq!(n, 1, "one host bridge, one register window");
        assert_eq!(
            (regs[0].start, regs[0].size),
            ecam,
            "the recorded ECAM window is not the tree's"
        );
        let ranges = dt
            .node_prop_compatible(b"pci-host-ecam-generic", b"ranges")
            .expect("the bridge's ranges parses")
            .expect("the bridge states its ranges");
        assert_eq!(
            ::pci::mem32_window(ranges),
            Some(mem32),
            "the recorded BAR window is not the tree's"
        );

        #[cfg(target_arch = "riscv64")]
        {
            assert_eq!(ecam, (0x3000_0000, 0x1000_0000), "was PCI_ECAM_BASE");
            assert_eq!(mem32, (0x4000_0000, 0x4000_0000), "was PCI_BAR_BASE");
        }
        #[cfg(target_arch = "aarch64")]
        {
            assert_eq!(ecam, (0x40_1000_0000, 0x1000_0000), "was PCI_ECAM_BASE");
            assert_eq!(mem32, (0x1000_0000, 0x2eff_0000), "was PCI_BAR_BASE");
        }
    }

    /// The window ACPI's MCFG named decodes real PCI configuration space once
    /// `arch::x86_64::machine::enable_pcie_ecam` has run. The host bridge (0:0.0) is on every
    /// x86_64 machine whatever `-device` flags are passed, and before `enable_pcie_ecam` its
    /// address faults (QEMU's monitor, 2026-08-24) rather than reading all-ones, so a wrong address
    /// or a skipped enable reads as "nothing found", not a false pass.
    #[cfg(target_arch = "x86_64")]
    #[test_case]
    fn acpi_mcfg_wires_a_real_ecam_window_that_finds_the_host_bridge() {
        assert!(
            is_host_bridge_present(),
            "ACPI's MCFG did not leave a usable window in memory::pci_regions()"
        );

        let mut found_host_bridge = false;
        let mut count = 0usize;
        pci::enumerate(
            ecam_buses(),
            &mut |b, o| cfg_read32(b, o),
            &mut |bdf, vendor, device| {
                count += 1;
                if bdf.bus == 0 && bdf.dev == 0 && bdf.func == 0 {
                    // The ids are a diagnostic, not a pass condition: a real machine's bridge
                    // answers an id other than q35's.
                    assert_ne!(
                        vendor, 0xffff,
                        "bus 0 device 0 function 0 read back all-ones, which is what an absent \
                         device looks like; the ECAM window is not decoding"
                    );
                    assert_ne!(
                        vendor, 0x0000,
                        "bus 0 device 0 function 0 read back zero, which is neither a device nor \
                         the absent pattern; the window is reading memory rather than config space"
                    );
                    crate::println!("    host bridge: vendor {vendor:#06x}, device {device:#06x}");
                    found_host_bridge = true;
                }
            },
        );
        assert!(
            found_host_bridge,
            "bus 0 device 0 function 0 (the host bridge) was not found; ECAM reads are not \
             reaching real config space"
        );
        assert!(
            count >= 1,
            "enumeration walked the bus and found nothing at all"
        );
    }

    /// A controller behind a PCIe root port is found on the bus behind it.
    ///
    /// Asserts the whole chain, since "an NVMe was found" passes on a flat machine too: the survey
    /// recorded more than one bus, a bridge states a subtree, and the controller is on a bus a
    /// bridge named. It does not bring the controller up: under PVH nothing programs the bridge's
    /// memory window (notes/pcie.md, "More than one bus", `BUGS`; and
    /// `design/roadmap/proposals/a-bridge-window-the-kernel-programs-itself.md`).
    #[cfg(target_arch = "x86_64")]
    #[test_case]
    fn a_controller_behind_a_bridge_is_found_on_the_bus_behind_it() {
        assert!(is_host_bridge_present(), "no ECAM window on this machine");

        // Every bridge's secondary bus, not just the first: the AMD-Vi leg also carries an empty
        // `pci-bridge` for its alias tests.
        let (mut secondaries, mut bridges) = ([0u8; 8], 0usize);
        let mut controller_on = None;
        pci::walk(ecam_buses(), &mut |b, o| cfg_read32(b, o), &mut |f| {
            if let Some(behind) = f.bridge
                && behind.secondary != 0
                && bridges < secondaries.len()
            {
                secondaries[bridges] = behind.secondary;
                bridges += 1;
            }
            if f.class == pci::CLASS_NVME && controller_on.is_none() {
                controller_on = Some(f.bdf.bus);
            }
        });
        let secondaries = &secondaries[..bridges];

        if secondaries.is_empty() {
            // The flat `q35` every other leg boots; the skip names the knob.
            crate::testing::skip!(
                "no configured PCI bridge on this machine: q35 is a flat root complex unless \
                 NIFE_PCIE_ROOT_PORT=1 puts the NVMe controller behind a root port, which \
                 `cargo xtask test --arch x86_64` does on its own leg"
            );
        }
        if controller_on == Some(0) {
            crate::testing::skip!(
                "the NVMe controller is on bus 0 and no bridge leads to it: the bridges here (the \
                 AMD-Vi leg's empty alias bridge) have nothing behind them"
            );
        }

        assert!(
            ecam_buses() >= 2,
            "a bridge states a subtree at bus {} but the survey mapped {} bus(es)",
            secondaries[0],
            ecam_buses(),
        );
        assert!(
            controller_on.is_some_and(|bus| secondaries.contains(&bus)),
            "the NVMe controller ({controller_on:?}) was not on a bus a bridge named \
             ({secondaries:?}); before milestone 320 this read as None, because the walk never \
             left bus 0"
        );
    }
}
