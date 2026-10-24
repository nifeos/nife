//! **aarch64: the firmware's device tree, and the Linux arm64 boot state.**
//!
//! The kernel's entry contract is U-Boot's `booti` (Linux's Documentation/arch/arm64/booting.rst,
//! which `kernel/src/arch/aarch64/boot.s` cites): entered at the image's first byte with **the MMU
//! off**, `x0` = the device tree's physical address and `x1`..`x3` zero, at EL2 or EL1. UEFI hands
//! over MMU and caches on with an identity map. So the loader copies the firmware's device tree,
//! adding `/chosen/linux,initrd-*` (`uefi_loader::device_tree_patch`); exits boot services; cleans
//! and invalidates the data cache to the point of coherency over everything the kernel reads with
//! its caches off; and turns the MMU and data cache off at its exception level and branches
//! (`leave_uefi.s`). See notes/x86-uefi-boot/loader-design.md.

//! # BUGS
//!
//! - **An ACPI machine is described second-hand** ([`uefi_loader::device_tree_from_acpi`]): the
//!   tree written from its tables loses what only AML holds (no PCI bus, no virtio-mmio, no
//!   `fw_cfg`). That module's BUGS is the list.
//! - **The kernel is linked for `0x4008_0000`** (`kernel/link-aarch64.ld`), QEMU `virt`'s RAM;
//!   argon's starts at `0x8000_0000`, so the loader refuses there with the memory map printed.
//! - **Everything the kernel reads early must be below 2 GiB** ([`ALLOCATION_CEILING`]): `boot.s`
//!   maps one 1 GiB block of RAM at `0x4000_0000`. A device tree kept higher is copied down.

use core::ptr;

use device_tree_blob::Region;
use machine_discovery::acpi;
use machine_discovery::gic::Gic;
use r_efi::efi::{self, BootServices, Handle, SystemTable};
use uefi_loader::{device_tree_from_acpi, device_tree_patch};

use crate::{
    MAP_SLACK_DESCRIPTORS, PAGE, Placed, allocate_below, boot_services, configuration_table,
    copy_device_tree, exit_boot_services, say, say_decimal, say_hex,
};

core::arch::global_asm!(include_str!("leave_uefi.s"));

unsafe extern "C" {
    /// `leave_uefi.s`: MMU and data cache off at the current EL, then `x0` = `device_tree`, branch.
    fn aarch64_leave_uefi(entry: u64, device_tree: u64) -> !;
}

/// **The ceiling on everything the kernel reads before its own page tables are up**: the device
/// tree and the archive. `boot.s`'s boot map covers RAM with a single 1 GiB block at
/// `0x4000_0000`, so the last byte it reaches is `0x7fff_ffff`.
pub(crate) const ALLOCATION_CEILING: u64 = 0x7fff_ffff;

/// The firmware's device tree, found before anything moves.
pub struct Found {
    device_tree: *const u8,
}

/// **Find, or failing that write, the tree the kernel is handed.**
///
/// The firmware's own device tree whenever it offers one, and ACPI only when it does not: the
/// richer description first. A device tree names the PCI bus with its BAR windows and interrupt
/// map, the virtio-mmio transports and `fw_cfg`, none of which exist outside AML, so a tree written
/// from ACPI ([`uefi_loader::device_tree_from_acpi`]) is strictly a subset of one the firmware
/// hands over. See notes/x86-uefi-boot/loader-design.md.
pub fn discover(table: &SystemTable, services: &BootServices) -> Result<Found, &'static str> {
    if let Some(entry) = configuration_table(table)
        .iter()
        .find(|entry| entry.vendor_guid == efi::DTB_TABLE_GUID)
    {
        return Ok(Found {
            device_tree: entry.vendor_table.cast(),
        });
    }
    let rsdp = find_rsdp(table).ok_or(
        "the firmware offers neither a device tree nor ACPI, so nothing describes this machine",
    )?;
    Ok(Found {
        device_tree: tree_from_acpi(table, services, rsdp)?,
    })
}

/// Clean and invalidate the data cache to the point of coherency over `[start, end)`, by virtual
/// address, which under UEFI's identity map is the physical one.
fn clean_to_coherency(start: u64, end: u64) {
    let ctr: u64;
    // SAFETY: reading the cache type register has no side effects.
    unsafe { core::arch::asm!("mrs {}, ctr_el0", out(reg) ctr, options(nomem, nostack)) };
    // DminLine, bits 19:16: log2 of the smallest data cache line, in words.
    let line = 4u64 << ((ctr >> 16) & 0xf);
    let mut at = start & !(line - 1);
    while at < end {
        // SAFETY: `dc civac` on an address the firmware mapped for us; it moves no data this
        // program can observe, only where the current copy lives.
        unsafe { core::arch::asm!("dc civac, {}", in(reg) at, options(nostack)) };
        at += line;
    }
    // SAFETY: a barrier.
    unsafe { core::arch::asm!("dsb sy", options(nostack)) };
}

/// Exit boot services and enter the kernel. Returns only on failure, with a console.
// The argument list is the x86_64 `hand_over`'s, kept in step on purpose: one signature per arch
// arm of the same seam, with the parameters this architecture does not use yet carried under
// their own names (see the two `None`s below).
#[allow(clippy::too_many_arguments)]
pub fn hand_over(
    handle: Handle,
    table: &SystemTable,
    services: &BootServices,
    found: Found,
    kernel: &Placed,
    module: Option<(u64, u64)>,
    // **Always `None` here**, and that is a recorded gap rather than an unused parameter.
    // `main.rs`'s `place_boot_file` reads this loader's own file so the running system can install
    // itself, and hands it over as a second PVH module. A device-tree handoff has `/chosen` with
    // one initial ramdisk and no second slot, so there is nowhere to put it; see that function's
    // BUGS and milestone 198 (a package manager, and the trivial install that makes a second
    // customer possible)'s rung 2a, which is an x86_64 claim for this reason.
    _boot_file: Option<(u64, u64)>,
    // **Always `None` here**, and that is a scope note (DECISIONS
    // §19 (architectural parity is a tenet; the targets are aarch64, riscv64, and x86_64)) rather than an unused parameter. The chooser that would set it is
    // `uefi_loader::chooser`, which is x86_64 only for the same reason the install offer is: an
    // installed machine on this architecture is what rung 2b of milestone 198 built, and the
    // device-tree architectures have no installed disk to choose slots on yet. When they do, this
    // parameter is where the number arrives.
    _from_slot: Option<u8>,
    // The boot medium token (`boot_slot::medium`) is x86_64 only, like the chooser.
    _on_nvme: bool,
) -> Result<(), &'static str> {
    if kernel.end > ALLOCATION_CEILING + 1 {
        return Err("the kernel is linked above what its own boot map reaches");
    }
    let device_tree = copy_device_tree(services, found.device_tree, module)?;
    say(
        table,
        "uefi_loader: kernel placed, exiting boot services\r\n",
    );
    exit_boot_services(handle, services)?;

    // --- The firmware is gone. No console from here. ---
    clean_to_coherency(kernel.start, kernel.end);
    if let Some((start, len)) = module {
        clean_to_coherency(start, start + len);
    }
    // The tree's own header says how long it is; the copy was written with that length.
    // SAFETY: `device_tree` is the copy made above and its header was written by us.
    let len = unsafe { u32::from_be(ptr::read_unaligned((device_tree + 4) as *const u32)) } as u64;
    clean_to_coherency(device_tree, device_tree + len);

    // SAFETY: everything the kernel reads is placed, cleaned to coherency, and below the ceiling;
    // `leave_uefi.s` documents the state it leaves, which is the arm64 boot protocol's.
    unsafe { aarch64_leave_uefi(kernel.entry, device_tree) }
}

/// Park the core with `wfi`, which QEMU turns into a real host-thread sleep (AGENTS.md).
pub fn park() -> ! {
    loop {
        // SAFETY: waits for an interrupt; no other effect.
        unsafe { core::arch::asm!("wfi", options(nomem, nostack)) };
    }
}

// ---------------------------------------------------------------------------------------------
// The other description: ACPI, for a machine that offers no device tree.
// ---------------------------------------------------------------------------------------------

/// **Find the ACPI root pointer**, the 2.0 entry first because its RSDP carries the 64-bit XSDT.
/// Identical in shape to the x86_64 loader's `find_rsdp`, and deliberately a separate four lines
/// rather than a shared helper: the two architectures reach it through the same configuration table
/// but do entirely different things with it.
fn find_rsdp(table: &SystemTable) -> Option<u64> {
    let mut fallback = None;
    for entry in configuration_table(table) {
        if entry.vendor_guid == efi::ACPI_20_TABLE_GUID {
            return Some(entry.vendor_table as u64);
        }
        if entry.vendor_guid == efi::ACPI_10_TABLE_GUID {
            fallback = Some(entry.vendor_table as u64);
        }
    }
    fallback
}

/// A system descriptor table, as the bytes its own header says it is.
///
/// # Safety
///
/// `phys` must be a physical address the firmware described as holding an ACPI table. The loader
/// runs under UEFI's identity map, so the physical address is the one to read.
unsafe fn table_at(phys: u64) -> Option<&'static [u8]> {
    if phys == 0 {
        return None;
    }
    // SAFETY: the caller's contract; the header's length field is at offset 4 and is read before
    // anything past the 36 bytes every table is guaranteed to have.
    let header = unsafe { core::slice::from_raw_parts(phys as *const u8, acpi::SDT_HEADER_LEN) };
    let len = acpi::parse_sdt_header(header).ok()?.length as usize;
    if len < acpi::SDT_HEADER_LEN {
        return None;
    }
    // SAFETY: as above, now for the length the header states.
    let bytes = unsafe { core::slice::from_raw_parts(phys as *const u8, len) };
    // **The checksum is the whole reason to bother**, and it is cheap. A table that does not sum to
    // zero is one this loader will not describe a machine from.
    acpi::is_checksum_ok(bytes).then_some(bytes)
}

/// The first table in the root list with this signature, checksum already checked.
///
/// # Safety
///
/// `rsdp` must be the firmware's ACPI root pointer.
unsafe fn find_table(rsdp: u64, signature: &[u8; 4]) -> Option<&'static [u8]> {
    // SAFETY: the caller's contract. An RSDP is 36 bytes at most and `parse_rsdp` checks both
    // checksums before any field is believed.
    let bytes = unsafe { core::slice::from_raw_parts(rsdp as *const u8, 36) };
    let rsdp = acpi::parse_rsdp(bytes).ok()?;
    let (root, wide) = rsdp.root_table();
    // SAFETY: the address the RSDP states, read as the table it says it is.
    let root = unsafe { table_at(root) }?;
    let body = &root[acpi::SDT_HEADER_LEN..];
    let count = acpi::root_entry_count(root.len() as u32, wide);
    for i in 0..count {
        let Some(at) = acpi::root_entry(body, i, wide) else {
            continue;
        };
        // SAFETY: an entry of the root table, which is a list of table addresses by definition.
        let Some(table) = (unsafe { table_at(at) }) else {
            continue;
        };
        if &table[..4] == signature {
            return Some(table);
        }
    }
    None
}

/// **Read the firmware's memory map into [`device_tree_from_acpi::Ram`] runs.**
///
/// Adjacent descriptors of RAM-ish types are merged, which on every machine this can describe
/// collapses a few dozen descriptors into one or two runs, the same shape the firmware's own device
/// tree would have stated. The map is read here rather than at hand-over because the tree has to be
/// written while allocation still works; nothing later widens or narrows RAM, so a map read now
/// describes the same machine.
fn read_ram(
    services: &BootServices,
    out: &mut [device_tree_from_acpi::Ram; device_tree_from_acpi::MAX_RAM],
) -> Result<usize, &'static str> {
    // The walk below strides by the firmware's own `descriptor_size`; see `memory_map_size`.
    let (bytes, descriptor_size) =
        boot_services::memory_map_size(services).ok_or("the firmware reported no memory map")?;
    let capacity = bytes + MAP_SLACK_DESCRIPTORS * descriptor_size;
    let buffer = boot_services::allocate_pool(services, capacity)
        .ok_or("no memory for the firmware's memory map")?;
    // SAFETY: `buffer` is `capacity` bytes from the firmware's pool, used for nothing else.
    let Some(map) = (unsafe { boot_services::memory_map(services, buffer, capacity) }) else {
        // SAFETY: `buffer` came from `allocate_pool` above and nothing holds it.
        unsafe { boot_services::free_pool(services, buffer) };
        return Err("the firmware's memory map would not fit the buffer it asked for");
    };
    let (got, descriptor_size) = (map.bytes, map.descriptor_size);

    let mut count = 0usize;
    for i in 0..got / descriptor_size {
        // SAFETY: `buffer` holds `got` bytes of descriptors and `i` is inside that count. Read
        // unaligned because the firmware chooses `descriptor_size` and nothing promises the stride
        // keeps 8-byte alignment.
        let d = unsafe {
            ptr::read_unaligned(
                (buffer as usize + i * descriptor_size) as *const efi::MemoryDescriptor,
            )
        };
        // **What counts as RAM the kernel may have.** Conventional memory, plus the two
        // boot-services types (free the instant `ExitBootServices` returns), plus the loader's own
        // allocations, which is where the kernel image and the archive already sit and which the
        // kernel carves out for itself. Deliberately NOT runtime services, ACPI reclaim or NVS: the
        // tables this tree was written from live there, and handing them to the frame allocator
        // would be handing out the description of the machine.
        let usable = matches!(
            d.r#type,
            efi::CONVENTIONAL_MEMORY
                | efi::BOOT_SERVICES_CODE
                | efi::BOOT_SERVICES_DATA
                | efi::LOADER_CODE
                | efi::LOADER_DATA
        );
        if !usable || d.number_of_pages == 0 {
            continue;
        }
        let start = d.physical_start;
        let len = d.number_of_pages * PAGE;
        // The map is sorted by address, so a descriptor that begins where the previous run ended
        // extends it. That is what turns a firmware's several dozen entries into the one or two
        // runs a device tree would have stated.
        if count > 0 && out[count - 1].start + out[count - 1].len == start {
            out[count - 1].len += len;
            continue;
        }
        if count == device_tree_from_acpi::MAX_RAM {
            break;
        }
        out[count] = device_tree_from_acpi::Ram { start, len };
        count += 1;
    }
    // SAFETY: `buffer` came from `allocate_pool` above, and the walk that read it is done.
    unsafe { boot_services::free_pool(services, buffer) };
    if count == 0 {
        return Err("the firmware's memory map describes no usable RAM");
    }
    Ok(count)
}

/// **Describe this machine from its ACPI tables, and write the device tree the kernel reads.**
///
/// Returns the physical address of the blob, which from the caller's side is indistinguishable from
/// one the firmware offered: everything downstream (`copy_device_tree`, the initial ramdisk patch,
/// the handover) is the path a device-tree machine already takes.
fn tree_from_acpi(
    table: &SystemTable,
    services: &BootServices,
    rsdp: u64,
) -> Result<*const u8, &'static str> {
    // SAFETY: `rsdp` came from the firmware's own configuration table, and every table reached
    // from it is checksummed before a field is read.
    let madt = unsafe { find_table(rsdp, b"APIC") }
        .ok_or("this machine's ACPI has no MADT, so nothing describes its interrupt controller")?;
    let body = &madt[acpi::SDT_HEADER_LEN..];

    // --- The interrupt controller, which is the one thing a boot cannot do without ---
    let entries = read_madt(body);
    let gic = gic_from_madt(&entries)?;

    // --- The rest, every one of which a machine may legitimately not have ---
    let (timer, psci_hvc, uart) = optional_tables(rsdp);

    let mut ram = [device_tree_from_acpi::Ram::default(); device_tree_from_acpi::MAX_RAM];
    let ram_count = read_ram(services, &mut ram)?;

    let machine = device_tree_from_acpi::Machine {
        gic,
        cpus: entries.cpus,
        cpu_count: entries.cpu_count,
        psci_hvc,
        timer,
        uart,
        ram,
        ram_count,
    };
    let base = write_tree(services, &machine)?;
    say_acpi_machine(table, &machine, &entries);
    Ok(base as *const u8)
}

/// What the MADT's entries say about the interrupt controller and the cores.
struct Madt {
    distributor: u64,
    version: u8,
    redistributors: Region,
    cpu_interface: u64,
    per_cpu_redistributor: u64,
    cpus: [device_tree_from_acpi::Cpu; device_tree_from_acpi::MAX_CPUS],
    cpu_count: usize,
    /// The first GIC ITS entry's frame, and how many the MADT states. Reported, not written into
    /// the tree: see `device_tree_from_acpi`'s BUGS.
    first_its: u64,
    its_count: u32,
}

/// Walk the MADT's entries once, keeping the GIC's frames and up to `MAX_CPUS` cores.
fn read_madt(body: &[u8]) -> Madt {
    let mut distributor = 0u64;
    let mut version = 0u8;
    let mut redistributors = Region { start: 0, size: 0 };
    let mut cpu_interface = 0u64;
    let mut per_cpu_redistributor = 0u64;
    let mut cpus = [device_tree_from_acpi::Cpu::default(); device_tree_from_acpi::MAX_CPUS];
    let mut cpu_count = 0usize;
    let mut first_its = 0u64;
    let mut its_count = 0u32;
    for entry in acpi::madt_entries(body) {
        match entry {
            acpi::MadtEntry::GenericDistributor {
                address,
                version: v,
            } => {
                distributor = address;
                version = v;
            }
            acpi::MadtEntry::GenericRedistributor { address, length } => {
                redistributors = Region {
                    start: address,
                    size: u64::from(length),
                };
            }
            acpi::MadtEntry::GenericInterruptController {
                mpidr,
                enabled,
                online_capable,
                cpu_interface: gicc,
                redistributor,
                ..
            } => {
                if gicc != 0 {
                    cpu_interface = gicc;
                }
                if redistributor != 0 && per_cpu_redistributor == 0 {
                    per_cpu_redistributor = redistributor;
                }
                if cpu_count < device_tree_from_acpi::MAX_CPUS {
                    cpus[cpu_count] = device_tree_from_acpi::Cpu {
                        mpidr,
                        // `online_capable` is a socket firmware would start later, and the kernel's
                        // `is_startable` predicate has no third state: it is not a core to call
                        // `CPU_ON` on now, so the node is written disabled.
                        enabled,
                    };
                    let _ = online_capable;
                    cpu_count += 1;
                }
            }
            acpi::MadtEntry::GicIts { address, .. } => {
                if its_count == 0 {
                    first_its = address;
                }
                its_count = its_count.saturating_add(1);
            }
            _ => {}
        }
    }
    Madt {
        distributor,
        version,
        redistributors,
        cpu_interface,
        per_cpu_redistributor,
        cpus,
        cpu_count,
        first_its,
        its_count,
    }
}

/// Decide the GIC's version and frames from what the MADT stated, or refuse to guess.
fn gic_from_madt(madt: &Madt) -> Result<Gic, &'static str> {
    let version = madt.version;
    let cpu_interface = madt.cpu_interface;
    let per_cpu_redistributor = madt.per_cpu_redistributor;
    let mut redistributors = madt.redistributors;
    if madt.distributor == 0 {
        return Err("this machine's MADT names no GIC distributor");
    }
    // **A version byte of zero means "work it out"**, and the evidence is which spelling the CPU
    // entries used: a redistributor (per-core or as a range) exists only on a GICv3, and a
    // memory-mapped CPU interface only on a GICv2. A machine that states neither is one this loader
    // will not guess about.
    let v3 = version >= 3 || redistributors.size != 0 || per_cpu_redistributor != 0;
    let v2 = version == 2 || (version == 0 && cpu_interface != 0);
    let distributor = Region {
        start: madt.distributor,
        size: device_tree_from_acpi::GICD_LEN,
    };
    let gic = if v3 {
        if redistributors.size == 0 {
            // The per-core spelling: each GICC entry carries its own frame and the MADT states no
            // range. Two 64 KiB frames per core is the GICv3 architecture's own layout, and the
            // frames are contiguous from the first, which is what `GICR_TYPER`'s Last bit then
            // confirms when the kernel walks them.
            redistributors = Region {
                start: per_cpu_redistributor,
                size: 0x2_0000 * madt.cpu_count.max(1) as u64,
            };
        }
        Gic::V3 {
            distributor,
            redistributors,
        }
    } else if v2 {
        Gic::V2 {
            distributor,
            cpu_interface: Region {
                start: cpu_interface,
                size: device_tree_from_acpi::GICC_LEN,
            },
        }
    } else {
        return Err("this machine's MADT names a GIC whose version this loader cannot work out");
    };
    Ok(gic)
}

/// The timer, PSCI and console tables, each of which a machine may legitimately not have.
fn optional_tables(
    rsdp: u64,
) -> (
    Option<acpi::Gtdt>,
    Option<bool>,
    Option<device_tree_from_acpi::Uart>,
) {
    // SAFETY: `rsdp` came from the firmware's own configuration table, and every table reached
    // from it is checksummed before a field is read.
    let timer = unsafe { find_table(rsdp, b"GTDT") }
        .and_then(|t| acpi::parse_gtdt(&t[acpi::SDT_HEADER_LEN..]).ok());
    // SAFETY: as above.
    let psci_hvc = unsafe { find_table(rsdp, b"FACP") }
        .and_then(|t| acpi::parse_arm_boot(&t[acpi::SDT_HEADER_LEN..]).ok())
        .and_then(|arm| arm.psci.then_some(arm.hvc));
    // SAFETY: as above.
    let uart = unsafe { find_table(rsdp, b"SPCR") }
        .and_then(|t| acpi::parse_spcr(&t[acpi::SDT_HEADER_LEN..]).ok())
        .filter(|spcr| {
            // System memory only, and only the two PL011 spellings: the kernel's console driver is
            // a PL011 and a node claiming to be one over a 16550 would be a lie this loader told.
            spcr.address_space == 0
                && spcr.address != 0
                && matches!(spcr.interface, acpi::SPCR_PL011 | acpi::SPCR_SBSA_UART)
        })
        .map(|spcr| device_tree_from_acpi::Uart {
            address: spcr.address,
            interrupt: spcr.interrupt,
        });
    (timer, psci_hvc, uart)
}

/// Allocate pages below the kernel's reach and write the device tree for `machine` into them.
fn write_tree(
    services: &BootServices,
    machine: &device_tree_from_acpi::Machine,
) -> Result<u64, &'static str> {
    let pages = (device_tree_from_acpi::output_len() as u64).div_ceil(PAGE) as usize;
    let base = allocate_below(services, pages, efi::LOADER_DATA)
        .ok_or("no memory below the kernel's reach for a device tree written from ACPI")?;
    // SAFETY: `pages` pages at `base` were just granted exclusively.
    let out = unsafe { core::slice::from_raw_parts_mut(base as *mut u8, pages * PAGE as usize) };
    device_tree_from_acpi::build(machine, out).map_err(device_tree_patch::Error::reason)?;
    Ok(base)
}

/// Say what the ACPI tables described, and which optional pieces the kernel will do without.
fn say_acpi_machine(table: &SystemTable, machine: &device_tree_from_acpi::Machine, madt: &Madt) {
    say(
        table,
        "uefi_loader: no device tree; describing this machine from ACPI\r\n",
    );
    say(table, "uefi_loader:   ");
    say_decimal(table, machine.cpu_count as u32);
    say(
        table,
        match machine.gic {
            Gic::V2 { .. } => " core(s), GICv2 at ",
            Gic::V3 { .. } => " core(s), GICv3 at ",
        },
    );
    say_hex(table, madt.distributor);
    say(table, ", ");
    say_decimal(table, machine.ram_count as u32);
    say(table, " RAM region(s)\r\n");
    // **Said, so that a boot shows the MADT's ITS was read**, and said to be left out, so that
    // nobody reads this line as the kernel having MSIs through it.
    if madt.its_count != 0 {
        say(table, "uefi_loader:   GIC ITS at ");
        say_hex(table, madt.first_its);
        if madt.its_count > 1 {
            say(table, " and ");
            say_decimal(table, madt.its_count - 1);
            say(table, " more");
        }
        say(
            table,
            "; not described to the kernel, which drives no ITS\r\n",
        );
    }
    if machine.uart.is_none() {
        say(
            table,
            "uefi_loader:   no SPCR names a PL011; the kernel will fall back for its console irq\r\n",
        );
    }
    if machine.psci_hvc.is_none() {
        say(
            table,
            "uefi_loader:   the FADT does not claim PSCI; the kernel will run one core\r\n",
        );
    }
}
