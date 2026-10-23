//! Reading what the loader said, `x86_64`: the kernel side of `machine_discovery::x86_64`.
//!
//! The crate decides what each byte means, host-tested; this file turns the physical address the
//! loader put in `ebx` into bytes through the direct map, as `console.rs` and `memory.rs` do with
//! `crates/device_tree_blob`. `memory::init` reaches it through `arch::x86_64::discovery`, which
//! owns the order: the handoff, ACPI, the device windows, PCI, then [`bring_up_memory`].
//!
//! # BUGS
//! - The CMOS RTC crosses no seam: it is two I/O ports, not a page, so `memory::RTC_REGION` has
//!   nowhere to put it. See notes/x86-port/acpi-and-pci.md and `port.rs`'s doc comment.

use machine_discovery::framebuffer::Framebuffer;
use machine_discovery::x86_64::{
    BootInfo, MEMMAP_ENTRY_LEN, MODULE_ENTRY_LEN, MemoryEntry, Module, memory_entry, module,
};

use super::mmu::phys_to_virt;

/// The most memory-map entries the boot print will walk. QEMU's `q35` produces four with `-m 256M`;
/// a real machine with an assortment of firmware reservations produces a couple of dozen. The cap
/// exists because this walks a structure a loader wrote, and a corrupt entry count must bound the
/// loop rather than hang the boot before the console has said anything useful.
const MAX_ENTRIES: usize = 64;

/// Read the `hvm_start_info` at physical address `at`.
///
/// `None` when the magic does not match or the structure is unreadable, which covers the case that
/// matters: this kernel booted some other way and `ebx` held something else. Reporting `None` lets
/// the caller say so; reading on would produce a plausible memory map out of unrelated bytes.
pub(crate) fn boot_info(at: usize) -> Option<BootInfo> {
    if at == 0 {
        return None;
    }
    // SAFETY: `at` is the physical address the PVH loader put in `ebx`, reached through the direct
    // map, which covers the low gigabyte and so covers the low megabyte the loader places this in.
    // 56 bytes is the whole version-1 structure; the parser checks the magic before believing any
    // of them, and refuses a truncated read.
    let bytes = unsafe { core::slice::from_raw_parts(phys_to_virt(at as u64) as *const u8, 56) };
    BootInfo::parse(bytes).ok()
}

/// **The longest boot command line this kernel will read**, in bytes.
///
/// A NUL-terminated string at a physical address a previous stage chose, so the terminator is a
/// claim rather than a guarantee, and the loop below has to stop somewhere whether or not it finds
/// one. `Framebuffer::MAX_LEN` is the only thing written there today and is well under this.
const MAX_CMDLINE: usize = 256;

/// The boot command line the loader wrote, or `None` when this boot chain did not write one.
///
/// One reader for every token: the screen ([`attach_screen`], milestone 243 (a machine with no
/// serial port has no way to say anything)) and, on an installed machine, the boot slot
/// ([`boot_slot`]), so a machine with no framebuffer still gets its slot. QEMU's PVH loader writes
/// a zero command line, so `script/test --arch x86_64` gets `None`.
fn cmdline(info: &BootInfo) -> Option<&'static str> {
    if info.cmdline == 0 {
        return None;
    }
    // SAFETY: a physical address the loader wrote into its own handoff, reached through the direct
    // map the boot page tables installed over the low 4 GiB. The length is capped rather than
    // trusted, so an unterminated string reads `MAX_CMDLINE` bytes of the loader's own page and
    // stops; the bytes are only ever compared against ASCII.
    let bytes = unsafe {
        core::slice::from_raw_parts(phys_to_virt(info.cmdline) as *const u8, MAX_CMDLINE)
    };
    let len = bytes.iter().position(|b| *b == 0).unwrap_or(MAX_CMDLINE);
    core::str::from_utf8(&bytes[..len]).ok()
}

/// **Which boot slot started this image**, from the token `uefi_loader`'s chooser wrote
/// (`boot_slot::cmdline`).
///
/// `None` on every boot no chooser started: a stick, a `-kernel` boot, and a chooser's own
/// fallback to the image in its file. A kernel that gets `None` confirms nothing, which is the
/// state the whole tree was in before this: safe, and upgrades do not stick.
///
/// **Read here rather than trusted from anywhere else.** The number decides which partition entry
/// a later write marks good, and a wrong one is a machine keeping an image that never ran, so the
/// parser refuses anything it cannot read exactly rather than guessing a slot.
fn boot_slot(info: &BootInfo) -> Option<u8> {
    boot_slot::cmdline::parse(cmdline(info)?)
}

/// Put the boot tour on a screen, if the boot handoff described one (milestone 243): read the
/// command line, find the framebuffer token, record the aperture so the fine map carries it, and
/// hand the console its direct-map address.
///
/// Returns what was found and the grid it produced, for the boot line; `None` when the machine said
/// nothing about a screen, as under QEMU's PVH loader. Must run before `arch::mmu::init`, and runs
/// first in the tour; see `memory::record_framebuffer`.
pub(crate) fn attach_screen(at: usize) -> Option<(Framebuffer, u32, u32)> {
    let info = boot_info(at)?;
    let cmdline = cmdline(&info)?;
    let found = Framebuffer::parse(cmdline)?;
    let span = found.span()? as u64;

    // The screen-hold handshake a gate may ask for (milestone 445 (the screen check stops sampling
    // and starts asking)), armed only once a screen is found, so a machine with none cannot wait.
    // See `console::hold_screen_at_handover`.
    if machine_discovery::framebuffer::has_screen_hold(cmdline) {
        crate::console::hold_screen_at_handover();
    }

    // Before the console is armed, not after: this is what makes the mapping survive `mmu::init`,
    // and a console armed against a window nothing will map is a fault inside `println!`.
    crate::memory::record_framebuffer(found.base, span);

    // SAFETY: the aperture is `span` bytes at `found.base`, which the loader read out of the
    // firmware's own `EFI_GRAPHICS_OUTPUT_PROTOCOL` and checked against the aperture size the
    // firmware reported. It is mapped now by the boot tables (which cover the low 4 GiB) and after
    // `mmu::init` by the entry the line above just arranged. Nothing else in the kernel writes
    // there: the display service's framebuffers are virtio-gpu surfaces in RAM, not this.
    let (cols, rows) = unsafe { crate::console::attach_screen(found, phys_to_virt(found.base))? };
    Some((found, cols, rows))
}

/// Read memory-map entry `index` of the map `info` describes.
pub(crate) fn memory_map_entry(info: &BootInfo, index: usize) -> Option<MemoryEntry> {
    if info.memmap == 0 || index >= info.memmap_entries as usize {
        return None;
    }
    let at = phys_to_virt(info.memmap) + (index * MEMMAP_ENTRY_LEN) as u64;
    // SAFETY: a physical address the loader gave us, reached through the direct map, for exactly
    // one entry's worth of bytes. The index is bounded by the count the loader stated above.
    let bytes = unsafe { core::slice::from_raw_parts(at as *const u8, MEMMAP_ENTRY_LEN) };
    memory_entry(bytes, 0)
}

/// The initial ramdisk, if the loader put one in RAM: x86's `/chosen/linux,initrd-start`.
///
/// QEMU's PVH loader turns `-initrd FILE` into an entry in the module list `hvm_start_info` points
/// at, and `uefi_loader` writes the same. Module 0 is the archive and module 1 the boot file
/// ([`boot_file`]), an order both ends state (`uefi_loader/src/arch/x86_64/mod.rs`'s `hand_over`).
/// A third module is ignored.
///
/// Name: ratified 2026-10-12 (calef, #1998, milestone 139 (drive the unsafe count down) round 11).
/// His words: "B for #1998." Refused `initrd` (the foreign spelling, which stops at the boundary
/// where it arrives).
pub fn initial_ramdisk(info: &BootInfo) -> Option<Module> {
    nth_module(info, 0)
}

/// A copy of the file this machine was booted from, if the loader could read one back: module 1.
///
/// An installer writes it to `\EFI\BOOT\BOOTX64.EFI` on a disk's EFI system partition; nothing else
/// keeps the PE file once the loader has placed its contents. The kernel and archive are sealed in
/// it as a set (`uefi_loader/build.rs`'s `refuse_an_unsealed_pair`), so an installer cannot copy
/// one without the other and halt at `MEASURED BOOT REFUSED`. `None` on a boot no loader can
/// re-read, QEMU's `-kernel` PVH path included.
pub(crate) fn boot_file(info: &BootInfo) -> Option<Module> {
    nth_module(info, 1)
}

/// Module `index`, if the loader stated one with bytes in it.
///
/// A module with no bytes is not a payload, and reporting one as an archive would have the kernel
/// parse an empty slice and report a corrupt filesystem rather than an absent one.
fn nth_module(info: &BootInfo, index: usize) -> Option<Module> {
    if info.modules == 0 || info.module_count as usize <= index {
        return None;
    }
    let at = phys_to_virt(info.modules) + (index * MODULE_ENTRY_LEN) as u64;
    // SAFETY: a physical address the loader gave us, reached through the direct map, for exactly one
    // entry's worth of bytes at an index the stated count covers.
    let bytes = unsafe { core::slice::from_raw_parts(at as *const u8, MODULE_ENTRY_LEN) };
    module(bytes, 0).filter(|m| m.size > 0)
}

/// Print the memory map, one line per entry plus a usable total. The x86 stand-in for the device
/// tree's RAM regions, and for now the only thing that reads the map at all.
pub(crate) fn print_memory_map(info: &BootInfo) {
    if info.memmap_entries == 0 {
        crate::println!(
            "  memory      : the loader described no memory map (version {})",
            info.version
        );
        return;
    }
    let shown = (info.memmap_entries as usize).min(MAX_ENTRIES);
    let mut usable = 0u64;
    crate::println!(
        "  memory      : {} regions from the PVH handoff, rsdp {:#x}",
        info.memmap_entries,
        info.rsdp,
    );
    for i in 0..shown {
        let Some(e) = memory_map_entry(info, i) else {
            break;
        };
        if e.is_usable_ram() {
            usable += e.size;
        }
        crate::println!(
            "                {:#014x}..{:#014x}  {}",
            e.addr,
            e.end(),
            e.kind.name(),
        );
    }
    if shown < info.memmap_entries as usize {
        crate::println!(
            "                ... and {} more",
            info.memmap_entries as usize - shown
        );
    }
    crate::println!("                usable ram: {} KiB", usable / 1024);
}

// ---------------------------------------------------------------------------------------------
// ACPI: finding the tables, and reading the two the kernel needs first.
//
// Everything that decides what a byte means is in `machine_discovery::acpi`, host-tested. What is
// here is finding the root pointer and turning physical addresses into slices, neither of which a
// host test can do.
// ---------------------------------------------------------------------------------------------

use machine_discovery::acpi::ivrs::IvrsUnits;
use machine_discovery::acpi::{
    self, DmarUnits, ISA_IRQ_COUNT, IsaIrqRouting, MADT_PCAT_COMPAT, MadtEntry, Rsdp, SdtHeader,
    isa_irq_table, madt_entries, mcfg_entry, parse_dmar, parse_fadt_reset, parse_madt, parse_rsdp,
    parse_sdt_header, root_entry, root_entry_count,
};

/// The BIOS area the RSDP is required to be in when it is not in the EBDA: `0xe0000..0x100000`,
/// scanned on 16-byte boundaries.
const BIOS_AREA: core::ops::Range<u64> = 0x000e_0000..0x0010_0000;

/// Where the BIOS Data Area records the Extended BIOS Data Area's segment. The EBDA's first
/// kilobyte is the other place an RSDP is allowed to be.
const EBDA_SEGMENT_PTR: u64 = 0x0000_040e;

/// The most root-table entries the walk will follow. A machine has a dozen or two tables; the cap
/// exists because the count comes from a length field firmware wrote.
const MAX_TABLES: usize = 64;

/// The most MADT entries the walk will report, for the same reason.
const MAX_MADT_ENTRIES: usize = 256;

/// The most `LocalApic` entries [`Acpi::cpus`] records (milestone 161's SMP item). Bounded well
/// past `cpu::MAX_CPUS` on purpose: a MADT can list more processor entries than this kernel can
/// seat (disabled sockets, a bigger machine than this build is sized for), and `smp::
/// seat_cpus_from_acpi` needs the true count to report "N described, only M startable" honestly,
/// the same distinction `read_cpu_list` draws on the device-tree architectures.
const MAX_ACPI_CPUS: usize = 32;

/// What the tables said, kept so the APIC and PCI bring-up can read it rather than re-walking.
///
/// Every field is `Option` because a machine is allowed not to have the table it comes from, and
/// "the firmware did not say" has to be distinguishable from "the firmware said zero". That is the
/// same posture `memory::uart_irq` takes on the other two architectures.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Acpi {
    /// The local APIC's physical address, from the MADT (with any address-override applied).
    pub local_apic: Option<u64>,
    /// The first IO APIC's id, physical address and global-interrupt base.
    pub io_apic: Option<(u8, u32, u32)>,
    /// **The sixteen legacy ISA IRQs, resolved through the MADT's interrupt source overrides.**
    ///
    /// Not an `Option`, unlike everything around it, and for a reason worth stating: a machine with
    /// no overrides at all is not a machine with no answer. The ISA bus's own convention (identity
    /// onto the global interrupt space, active high, edge triggered) *is* the answer in that case,
    /// which is what `IsaIrqRouting::isa_default` encodes.
    pub isa_irqs: [IsaIrqRouting; ISA_IRQ_COUNT],
    /// How many processors the MADT lists as enabled.
    pub enabled_cpus: usize,
    /// How many it lists as present but not enabled.
    pub disabled_cpus: usize,
    /// **Every `LocalApic` entry the MADT listed**: `(local_apic_id, enabled)`, in the table's own
    /// order, past `cpu_count`. Fed to `smp::seat_cpus_from_acpi` (milestone 161's SMP item), the
    /// ACPI analog of `read_cpu_list`'s device-tree walk.
    pub cpus: [(u8, bool); MAX_ACPI_CPUS],
    /// How many entries of [`Acpi::cpus`] are real. May exceed `MAX_ACPI_CPUS` conceptually (the
    /// table can list more than that), in which case this saturates at `MAX_ACPI_CPUS` and
    /// `enabled_cpus`/`disabled_cpus` above are still the true totals.
    pub cpu_count: usize,
    /// True when the machine also has 8259 PICs, which must be masked before the APICs are used.
    pub has_8259: bool,
    /// The PCIe ECAM window, from the MCFG: base, first bus, last bus.
    pub ecam: Option<(u64, u8, u8)>,
    /// **The FADT's reset register** (milestone 249 (the boot lottery is sampled by a person walking to the board)'s `x86_64` half): where firmware says a write
    /// resets the machine, and the value to write. `None` for no FADT, an ACPI 1.0 one, or one
    /// that does not set `RESET_REG_SUP`; the kernel's reboot then starts at port `0xCF9`.
    pub reset: Option<acpi::ResetRegister>,
    /// **Every DRHD, PCI device scope and RMRR the DMAR describes** (milestones 261 and 594). The
    /// kernel brings up every unit here and routes each device to the one that owns it, which it
    /// can only resolve after the bus is up, so the table is kept rather than read once. Empty on a
    /// machine with no DMAR.
    pub dmar: DmarUnits,
    /// **Every AMD-Vi unit, device-id range and IVMD the IVRS describes** (lane `amd-vi`). An AMD
    /// machine's counterpart of [`Acpi::dmar`]; empty on a machine with no IVRS, which is every
    /// Intel machine.
    pub ivrs: IvrsUnits,
}

impl Default for Acpi {
    fn default() -> Self {
        Self {
            local_apic: None,
            io_apic: None,
            isa_irqs: core::array::from_fn(|irq| IsaIrqRouting::isa_default(irq as u8)),
            enabled_cpus: 0,
            disabled_cpus: 0,
            cpus: [(0, false); MAX_ACPI_CPUS],
            cpu_count: 0,
            has_8259: false,
            ecam: None,
            reset: None,
            dmar: DmarUnits::default(),
            ivrs: IvrsUnits::default(),
        }
    }
}

/// Read `len` bytes at physical address `at` through the direct map.
///
/// # Safety
/// `at..at + len` must be inside the boot map's direct map (the low [`BOOT_DIRECT_MAP_LIMIT`],
/// which is what [`reachable`] checks) and must be memory rather than a device register block,
/// since this is an ordinary read.
unsafe fn phys_slice<'a>(at: u64, len: usize) -> &'a [u8] {
    // SAFETY: the caller's obligation, restated: an in-range physical address naming memory.
    unsafe { core::slice::from_raw_parts(phys_to_virt(at) as *const u8, len) }
}

/// How far the boot page tables' direct map reaches: the low 4 GiB.
///
/// `boot.s` builds 2048 2 MiB page-directory entries under a four-entry PDPT that both PML4[0]
/// (identity) and PML4[273] (the direct map `phys_to_virt` names) point at. The ACPI walk runs
/// before `mmu::init`, so this bound, not the fine map's, applies. Firmware puts its tables near
/// the top of RAM, which the suite's `-m 256M` does not exercise; see
/// notes/x86-port/acpi-and-pci.md.
const BOOT_DIRECT_MAP_LIMIT: u64 = 0x1_0000_0000;

/// Is the direct map able to reach this physical address? Firmware supplies these addresses, and
/// one above [`BOOT_DIRECT_MAP_LIMIT`] computes an address that is right and unmapped.
///
/// A refusal is not ignorable: an unreachable table hides the APICs, PCIe window and IOMMU, so
/// `read_acpi` reports each one. Tables above 4 GiB (none seen) need the boot map widened, not this
/// loosened.
fn is_reachable(at: u64, len: usize) -> bool {
    at != 0 && at.saturating_add(len as u64) <= BOOT_DIRECT_MAP_LIMIT
}

/// Find the ACPI root pointer: where the loader said (PVH has a field, which QEMU leaves zero),
/// then the first kilobyte of the EBDA, then the BIOS area `0xe0000..0x100000`. The last two scan
/// for an eight-byte string, so `parse_rsdp`'s checksum is all that separates a hit from chance.
pub(crate) fn find_rsdp(hint: u64) -> Option<(u64, Rsdp)> {
    if is_reachable(hint, 36)
        // SAFETY: `reachable` checked the range is in the direct map, and firmware places this
        // structure in ordinary memory.
        && let Ok(rsdp) = parse_rsdp(unsafe { phys_slice(hint, 36) })
    {
        return Some((hint, rsdp));
    }

    // SAFETY: the BIOS Data Area is at a fixed low physical address on every PC and is memory.
    let bda = unsafe { phys_slice(EBDA_SEGMENT_PTR, 2) };
    // A SEGMENT, not an address: the BDA holds it in real-mode form, so it is shifted left four to
    // become the physical address. Reading it as an address directly would land in the first 64 KiB
    // and find nothing, quietly.
    let ebda = (u16::from_le_bytes([bda[0], bda[1]]) as u64) << 4;
    if ebda >= 0x400
        && let Some(found) = scan_for_rsdp(ebda..ebda + 0x400)
    {
        return Some(found);
    }

    scan_for_rsdp(BIOS_AREA)
}

/// Scan `range` on 16-byte boundaries for a well-formed RSDP.
fn scan_for_rsdp(range: core::ops::Range<u64>) -> Option<(u64, Rsdp)> {
    let mut at = range.start & !0xf;
    while at + 36 <= range.end {
        if is_reachable(at, 36) {
            // SAFETY: `reachable` checked the range; the low megabyte is memory.
            let bytes = unsafe { phys_slice(at, 36) };
            if let Ok(rsdp) = parse_rsdp(bytes) {
                return Some((at, rsdp));
            }
        }
        at += 16;
    }
    None
}

/// Read the header of the table at physical `at`, verifying its checksum over the whole table.
///
/// A table whose checksum fails is reported as absent rather than used, which is the only safe
/// reading: a corrupt MADT would hand out an APIC address, and there is nothing downstream that
/// could notice it was wrong.
fn table_at(at: u64) -> Option<(SdtHeader, &'static [u8])> {
    // Out of reach is reported apart from a bad checksum: one is a table to distrust, the other a
    // boot map to widen.
    if !is_reachable(at, acpi::SDT_HEADER_LEN) {
        crate::println!(
            "                {at:#012x}  outside the boot map's low {} GiB, cannot be read",
            BOOT_DIRECT_MAP_LIMIT / (1024 * 1024 * 1024),
        );
        return None;
    }
    // SAFETY: `reachable` checked the header's range.
    let header = parse_sdt_header(unsafe { phys_slice(at, acpi::SDT_HEADER_LEN) }).ok()?;
    if !is_reachable(at, header.length as usize) {
        crate::println!(
            "                {at:#012x}  {} bytes long, which runs past the boot map",
            header.length,
        );
        return None;
    }
    // SAFETY: `reachable` checked the whole table's range, using the length the header states.
    let whole = unsafe { phys_slice(at, header.length as usize) };
    if !acpi::is_checksum_ok(whole) {
        return None;
    }
    Some((header, &whole[acpi::SDT_HEADER_LEN..]))
}

/// Walk the tables and collect what the kernel will need, printing a line per table as it goes.
///
/// Printing during the walk rather than after is deliberate: this is the first code in the boot that
/// follows pointers firmware wrote, so if one of them is wrong the last line printed says which
/// table was being read.
pub(crate) fn read_acpi(hint: u64) -> Acpi {
    let mut found = Acpi::default();

    let Some((at, rsdp)) = find_rsdp(hint) else {
        crate::println!("  acpi        : no RSDP found (loader said {hint:#x}, and no scan hit)");
        return found;
    };
    let (root, wide) = rsdp.root_table();
    crate::println!(
        "  acpi        : rsdp at {at:#x} (revision {}), root table {root:#x} ({})",
        rsdp.revision,
        if wide { "xsdt" } else { "rsdt" },
    );

    let Some((root_header, root_body)) = table_at(root) else {
        crate::println!("                root table unreadable or failed its checksum");
        return found;
    };
    let count = root_entry_count(root_header.length, wide).min(MAX_TABLES);

    for i in 0..count {
        let Some(pa) = root_entry(root_body, i, wide) else {
            break;
        };
        let Some((header, body)) = table_at(pa) else {
            crate::println!("                {pa:#012x}  unreadable or bad checksum");
            continue;
        };
        let name = header.signature_str().unwrap_or("????");
        crate::println!(
            "                {pa:#012x}  {name} ({} bytes)",
            header.length
        );
        match &header.signature {
            b"APIC" => read_madt(body, &mut found),
            b"MCFG" => read_mcfg(body, &mut found),
            b"DMAR" => read_dmar(body, &mut found),
            b"IVRS" => found.ivrs = IvrsUnits::parse(body),
            b"FACP" => {
                found.reset = parse_fadt_reset(body);
                super::reset::record(found.reset);
            }
            _ => {}
        }
    }

    found
}

fn read_madt(body: &[u8], into: &mut Acpi) {
    let Ok(madt) = parse_madt(body) else {
        return;
    };
    into.local_apic = Some(madt.local_apic as u64);
    into.has_8259 = madt.flags & MADT_PCAT_COMPAT != 0;
    // The overrides in one pass of their own, in the crate, because resolving them is pure logic
    // over bytes and belongs where a host test can prove it. See its `the_timers_irq_0_resolves_to
    // _gsi_2` test for the case this whole table exists for.
    into.isa_irqs = isa_irq_table(body);

    for entry in madt_entries(body).take(MAX_MADT_ENTRIES) {
        match entry {
            MadtEntry::LocalApic {
                apic_id, enabled, ..
            } => {
                if enabled {
                    into.enabled_cpus += 1;
                } else {
                    into.disabled_cpus += 1;
                }
                if into.cpu_count < MAX_ACPI_CPUS {
                    into.cpus[into.cpu_count] = (apic_id, enabled);
                    into.cpu_count += 1;
                }
            }
            MadtEntry::IoApic {
                id,
                address,
                gsi_base,
            } => {
                if into.io_apic.is_none() {
                    into.io_apic = Some((id, address, gsi_base));
                }
            }
            // Overrides the 32-bit address in the fixed part, and comes after it in the list, which
            // is why this is applied here rather than being read first.
            MadtEntry::LocalApicAddressOverride(pa) => into.local_apic = Some(pa),
            _ => {}
        }
    }
}

fn read_mcfg(body: &[u8], into: &mut Acpi) {
    if let Some(e) = mcfg_entry(body, 0) {
        into.ecam = Some((e.base, e.start_bus, e.end_bus));
    }
}

fn read_dmar(body: &[u8], into: &mut Acpi) {
    // The host address width is not kept (`iommu::init` reads `CAP_REG` itself); `INTR_REMAP` is,
    // in `DmarUnits::platform_interrupt_remapping`. Parsing the fixed part proves the table
    // well-formed before its structure list is trusted.
    if parse_dmar(body).is_err() {
        return;
    }
    into.dmar = DmarUnits::parse(body);
}

// ---------------------------------------------------------------------------------------------
// Turning the MCFG's ECAM window on. Reading it is not enough: nobody has told the chipset to
// route it yet.
// ---------------------------------------------------------------------------------------------

/// The legacy PCI configuration-mechanism ports. Present on every PC-compatible machine, reached
/// only by `in`/`out` (see `arch::x86_64::port`), and independent of whether ECAM decode is on,
/// which is exactly why they are the way to turn it on rather than a chicken-and-egg problem.
const CONFIG_ADDRESS: u16 = 0xcf8;
const CONFIG_DATA: u16 = 0xcfc;

/// `CONFIG_ADDRESS` naming bus 0, device 0, function 0 (the host bridge, always present), register
/// `0x60`: the enable bit is set (bit 31), bus/device/function are all zero, and the register
/// offset is `0x60` (`PCIEXBAR`, the host bridge's memory-mapped-config-space base-address
/// register on every Intel-compatible chipset this kernel has run on, q35 included).
const PCIEXBAR_CONFIG_ADDRESS: u32 = 0x8000_0060;

/// What [`enable_pcie_ecam`] found, so the boot line can say which machine it is on rather than
/// claiming credit for work firmware had already done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EcamDecode {
    /// The register already carried the MCFG's own base with the enable bit set, and nothing was
    /// written. Measured on both paths this kernel boots: OVMF programs it while bringing PCIe up,
    /// and QEMU's `q35` arrives at reset with it already set (see [`enable_pcie_ecam`]).
    AlreadyOn,
    /// The register was off, or named somewhere else, and was programmed from the MCFG.
    Programmed,
    /// The MCFG describes a bus count this chipset register cannot encode, so nothing was written
    /// and whatever decoded before still decodes. Loud rather than silent: a wrong length field
    /// redirects a range of physical addresses that belongs to something else.
    Unencodable,
}

/// The `PCIEXBAR` length field (bits 2:1) for a bus count, or `None` for a count the register
/// cannot express. Three sizes are defined and every Intel-compatible chipset this kernel targets
/// encodes them the same way: 256 MiB for 256 buses, 128 MiB for 128, 64 MiB for 64.
fn pciexbar_length_bits(bus_count: u32) -> Option<u32> {
    match bus_count {
        256 => Some(0b00),
        128 => Some(0b01),
        64 => Some(0b10),
        _ => None,
    }
}

/// Turn the MCFG's ECAM window on, unless something already has.
///
/// Firmware programs the host bridge's `PCIEXBAR` (base, length, enable) before an OS runs; PVH is
/// not firmware, so this port asks. It reads before it writes: the same base with another length
/// widens the chipset's decode over whatever sits above, and a locked register drops the write.
/// Both boot paths arrive with the decode on, so the write is unexercised
/// (notes/x86-port/acpi-and-pci.md). The legacy configuration ports are the only way in: nothing
/// can read the ECAM window to turn it on.
pub(crate) fn enable_pcie_ecam(base: u64, bus_count: u32) -> EcamDecode {
    // SAFETY: 0xcf8/0xcfc are the legacy PCI configuration ports, present on every PC-compatible
    // machine independent of ECAM, and reading the host bridge's own configuration space through
    // them is what firmware does before handing control to an OS.
    let current = unsafe {
        super::port::out32(CONFIG_ADDRESS, PCIEXBAR_CONFIG_ADDRESS);
        super::port::in32(CONFIG_DATA)
    };
    // Bit 0 is the enable bit, bits 2:1 are the length, and the rest is the base. This port has
    // never seen an MCFG base above 4 GiB, and truncating to 32 bits is the same assumption
    // `PCI_ECAM_PHYS` already makes.
    let want_base = base as u32 & !0xf;
    if current & 1 != 0 && current & !0xf == want_base {
        return EcamDecode::AlreadyOn;
    }
    let Some(length) = pciexbar_length_bits(bus_count) else {
        return EcamDecode::Unencodable;
    };
    // SAFETY: as above, and this is the register sequence firmware itself runs. The length comes
    // from the MCFG's own bus range rather than being assumed, which is what stops this widening a
    // window firmware sized deliberately.
    unsafe {
        super::port::out32(CONFIG_ADDRESS, PCIEXBAR_CONFIG_ADDRESS);
        super::port::out32(CONFIG_DATA, want_base | (length << 1) | 1);
    }
    EcamDecode::Programmed
}

/// Print what the tables said, after [`read_acpi`] has walked them.
pub(crate) fn print_acpi_summary(found: &Acpi) {
    match found.local_apic {
        Some(pa) => crate::println!(
            "                local apic {pa:#x}, {} cpu(s) enabled, {} disabled, 8259s {}",
            found.enabled_cpus,
            found.disabled_cpus,
            if found.has_8259 {
                "present (must be masked)"
            } else {
                "absent"
            },
        ),
        None => crate::println!("                no MADT: nothing knows where the APICs are"),
    }
    if let Some((id, address, gsi_base)) = found.io_apic {
        crate::println!("                io apic {id} at {address:#x}, gsi base {gsi_base}");
    }
    print_isa_overrides(found);
    match found.ecam {
        // The parenthetical names q35: `PCI_ECAM_PHYS` is q35's default, a second witness and not
        // the source of truth. `base` is the MCFG's answer and is what gets mapped, and firmware
        // places the window, so a different number is no defect (notes/x86-port/acpi-and-pci.md).
        Some((base, lo, hi)) => crate::println!(
            "                pcie ecam {base:#x}, buses {lo}..={hi} (q35's default is {:#x})",
            super::mmu::PCI_ECAM_PHYS,
        ),
        None => crate::println!("                no MCFG: the PCIe window is not described"),
    }
    // **Every unit and every RMRR, as the firmware described them**, because which unit owns the
    // NVMe is fatal risk 6's first night-of condition and the answer starts here. Whether each unit
    // came up is the `vt-d` line's job, later in the tour. `all` is the INCLUDE_PCI_ALL flag.
    for d in found.dmar.units() {
        crate::println!(
            "                vt-d drhd at {:#x}{}",
            d.register_base,
            if d.include_pci_all {
                ", all (the catch-all)"
            } else {
                ", named devices only"
            },
        );
    }
    for r in found.dmar.reserved_regions() {
        crate::println!(
            "                vt-d rmrr {:#x}..={:#x} ({} KiB), identity-mapped for the device(s) it names",
            r.base,
            r.limit,
            r.size() / 1024,
        );
    }
    if found.dmar.rmrrs_refused > 0 {
        crate::println!(
            "                vt-d: {} rmrr(s) refused as malformed (VT-d 8.4), not mapped",
            found.dmar.rmrrs_refused,
        );
    }
    if found.dmar.truncated {
        crate::println!(
            "                vt-d: the DMAR did not fit what this kernel records; scope answers are unknown"
        );
    }
    if found.dmar.units().is_empty() {
        crate::println!("                no DMAR: no VT-d unit described");
    }
    // **And AMD's**, from the IVRS (lane `amd-vi`). One line per unit, then the IVMDs.
    for d in found.ivrs.units() {
        crate::println!(
            "                amd-vi unit at {:#x} (ivhd type {:#x}, iommu {:02x}:{:02x}.{})",
            d.register_base,
            d.kind,
            d.device_id >> 8,
            (d.device_id >> 3) & 0x1f,
            d.device_id & 7,
        );
    }
    for r in found.ivrs.unity_regions() {
        crate::println!(
            "                amd-vi ivmd {:#x}..{:#x} for devices {:#06x}..={:#06x}, identity-mapped",
            r.base,
            r.base + r.size,
            r.first,
            r.last,
        );
    }
    if found.ivrs.truncated {
        crate::println!(
            "                amd-vi: the IVRS did not fit what this kernel records; scope answers are unknown"
        );
    }
    if found.ivrs.units().is_empty() {
        crate::println!("                no IVRS: no AMD-Vi unit described");
    }
    // Printed on every boot, not only a rebooting one, because this is the line a bench log is read
    // for when the question is "what will a reset on this machine write, and where".
    match found.reset {
        Some(r) => crate::println!(
            "                fadt reset register: {:?} {:#x} ({} bits) <- {:#04x}",
            r.space,
            r.address,
            r.bit_width,
            r.value,
        ),
        None => {
            crate::println!(
                "                no FADT reset register: a reboot starts at port 0xcf9"
            );
        }
    }
}

/// **Print every legacy IRQ the MADT rewired**, and only those.
///
/// Sixteen lines of identity mapping would be noise; the ones that moved are the whole reason this
/// table is read, and the first of them is always the timer. A boot that printed nothing here on a
/// PC would itself be the finding.
fn print_isa_overrides(found: &Acpi) {
    let moved = found
        .isa_irqs
        .iter()
        .enumerate()
        .filter(|(irq, r)| **r != IsaIrqRouting::isa_default(*irq as u8));
    for (irq, routing) in moved {
        crate::println!(
            "                isa irq {irq} -> gsi {} ({}, {})",
            routing.gsi,
            if routing.active_low {
                "active low"
            } else {
                "active high"
            },
            if routing.level_triggered {
                "level"
            } else {
                "edge"
            },
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Bringing the frame allocator up from the PVH memory map.
// ---------------------------------------------------------------------------------------------

/// The most RAM regions handed to the allocator. `memory::bring_up_page_frames` indexes a fixed map of
/// this size, so more than this cannot be described; q35 produces three.
const MAX_RAM_REGIONS: usize = 16;

/// **The whole first megabyte is never allocatable**, and this is the one x86 reservation with no
/// counterpart on the other two architectures.
///
/// It holds the real-mode interrupt vector table, the BIOS data area, the extended BIOS data area,
/// the VGA window, option ROM shadow, and (on this boot) the `hvm_start_info` structure and the
/// memory map the loader wrote. Some of that is described as RAM by the memory map, because it
/// physically is; none of it is memory the kernel may hand out. It is also where an SMP real-mode
/// trampoline has to be copied, since a STARTUP IPI's vector can only name a page below this line.
const LOW_MEGABYTE: u64 = 0x0010_0000;

/// **Bring the frame allocator up from what the loader described.**
///
/// The x86 counterpart of `memory::init`'s device-tree front end: it turns the PVH memory map into
/// the two slices `memory::bring_up_page_frames` takes, and nothing else.
///
/// Returns how many RAM regions were used, which the boot print reports; a machine describing more
/// than [`MAX_RAM_REGIONS`] gets the first that many, and saying how many were taken is what makes
/// that visible rather than silent.
pub(crate) fn bring_up_memory(info: &BootInfo) -> usize {
    let mut ram = [device_tree_blob::Region { start: 0, size: 0 }; MAX_RAM_REGIONS];
    let mut count = 0;

    for i in 0..info.memmap_entries as usize {
        if count == MAX_RAM_REGIONS {
            break;
        }
        let Some(e) = memory_map_entry(info, i) else {
            break;
        };
        if !e.is_usable_ram() {
            continue;
        }
        // Clip the low megabyte off rather than reserving it afterwards. Either works; clipping is
        // the one that cannot be undone by a later `mark_free`, and the allocator's own rule is
        // that reserving has to win, which means order would otherwise matter.
        let start = e.addr.max(LOW_MEGABYTE);
        let end = e.end();
        if end <= start {
            continue;
        }
        ram[count] = device_tree_blob::Region {
            start,
            size: end - start,
        };
        count += 1;
    }

    // Each reservation is fatal to miss: the image includes the boot page tables the CPU walks
    // (`.boot_scratch` lies inside its bounds), and the ramdisk sits in RAM the loader left
    // unreserved. The array is sliced to its count, since a zero `Region` would also be taken.
    let mut forbidden = [device_tree_blob::Region { start: 0, size: 0 }; 3];
    forbidden[0] = device_tree_blob::Region {
        start: crate::memory::image_start(),
        size: crate::memory::image_end() - crate::memory::image_start(),
    };
    let mut forbidden_count = 1;
    if let Some(m) = initial_ramdisk(info) {
        forbidden[forbidden_count] = device_tree_blob::Region {
            start: m.addr,
            size: m.size,
        };
        forbidden_count += 1;
        crate::memory::record_initial_ramdisk(m.addr, m.size);
    }
    // The boot file, which an installer will write to a disk. `handoff::e820_kind` already reports
    // the UEFI loader's `LOADER_DATA` as reserved; this covers a less careful loader.
    if let Some(m) = boot_file(info) {
        forbidden[forbidden_count] = device_tree_blob::Region {
            start: m.addr,
            size: m.size,
        };
        forbidden_count += 1;
        crate::memory::record_boot_file(m.addr, m.size);
    }
    // Which slot started this image, recorded before `mmu::init` replaces the boot map this
    // reader uses. `user::install_service::confirm` is its one consumer.
    crate::memory::record_boot_slot(boot_slot(info));
    // And whether the boot came from the NVMe disk, the one fact that lets the internal disk be
    // mounted at all (`boot_slot::medium`; calef's ruling on PR #1652, 2026-10-04 UTC).
    crate::memory::record_booted_from_nvme(cmdline(info).is_some_and(boot_slot::medium::is_nvme));

    crate::memory::bring_up_page_frames(&ram[..count], &forbidden[..forbidden_count]);
    count
}

// ---------------------------------------------------------------------------------------------
// Where low DRAM stops, from the host bridge itself (milestone 256).
// ---------------------------------------------------------------------------------------------

/// `CONFIG_ADDRESS` naming bus 0, device 0, function 0, register `0x00`: the host bridge's vendor
/// and device id. Read before [`top_of_low_dram`] trusts anything else in that configuration
/// space, because the register it goes on to read is one vendor's and not the architecture's.
const HOST_BRIDGE_ID_CONFIG_ADDRESS: u32 = 0x8000_0000;

/// `CONFIG_ADDRESS` naming the same function's register `0xbc`, `TOLUD`.
const TOLUD_CONFIG_ADDRESS: u32 = 0x8000_00bc;

/// PCI's vendor id for Intel, which is whose register [`top_of_low_dram`] reads.
const INTEL_VENDOR: u32 = 0x8086;

/// Where the machine says its low DRAM stops, or `None` when it does not say (milestone 256).
///
/// `TOLUD` is the host bridge's boundary between DRAM and the 32-bit MMIO hole, where a PCI BAR
/// window starts: a register the legacy ports reach, not AML. Bits 31:20; zero is not reported.
///
/// # BUGS
/// - Intel's register only: another vendor's host bridge answers `None`.
/// - Only Core-era `0xbc` is read, not older chipsets' `0xb0`; q35 models neither (`None`).
///
/// Name: provisional. calef names the functions, and this one was minted by a lane. It is the
/// register's own expansion spelled out (`TOLUD` is "top of low usable DRAM"), on the same
/// reasoning that keeps `elf` and `pci` spelled the way the field already spells them.
pub fn top_of_low_dram() -> Option<u64> {
    // SAFETY: 0xcf8/0xcfc are the legacy PCI configuration ports, present on every PC-compatible
    // machine independent of ECAM (see `enable_pcie_ecam`), and reading the host bridge's own
    // configuration space through them is side-effect-free.
    let id = unsafe {
        super::port::out32(CONFIG_ADDRESS, HOST_BRIDGE_ID_CONFIG_ADDRESS);
        super::port::in32(CONFIG_DATA)
    };
    if id & 0xffff != INTEL_VENDOR {
        return None;
    }
    // SAFETY: as above.
    let raw = unsafe {
        super::port::out32(CONFIG_ADDRESS, TOLUD_CONFIG_ADDRESS);
        super::port::in32(CONFIG_DATA)
    };
    let tolud = u64::from(raw & 0xfff0_0000);
    (tolud != 0).then_some(tolud)
}
