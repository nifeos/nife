//! **The AMD-Vi driver: AMD's IOMMU, in front of the PCIe bus.** VT-d's counterpart on an AMD
//! machine, behind the same calls the portable seam makes, which [`super::iommu`] hands over
//! whenever a unit here is up. Formats are from AMD document 48882 revision 2.62; QEMU 11.1.1 is
//! the only AMD-Vi this has run on.
//!
//! Default deny: `init` fills every device table entry with [`blocked_dte`] before enabling the
//! unit, and the same entry is the quarantine of §163 (where a confined device's IOMMU fault is
//! delivered). See notes/amd-vi.md, which holds the design, QEMU's differences and the BUGS.

use machine_discovery::acpi::ivrs::{IvrsUnits, MAX_IVHDS};

use crate::arch::mmu::phys_to_virt;
use crate::sync::{IrqSafeMutex, rank};

// --- MMIO register offsets (section 3.3.1). ---
const DEVICE_TABLE_BASE: u64 = 0x0000;
const COMMAND_BUFFER_BASE: u64 = 0x0008;
const EVENT_LOG_BASE: u64 = 0x0010;
const CONTROL: u64 = 0x0018;
const EXCLUSION_BASE: u64 = 0x0020;
const EXCLUSION_LIMIT: u64 = 0x0028;
const EXTENDED_FEATURES: u64 = 0x0030;
const COMMAND_HEAD: u64 = 0x2000;
const COMMAND_TAIL: u64 = 0x2008;
const EVENT_HEAD: u64 = 0x2010;
const EVENT_TAIL: u64 = 0x2018;
const STATUS: u64 = 0x2020;

/// **How much of the register file this driver maps**: 16 KiB, which covers every register above
/// (the highest is `STATUS`, at `0x2020`). A unit with performance counters decodes up to 512 KiB
/// (`0x80000`); nothing here reads them. QEMU's model is exactly 16 KiB (`AMDVI_MMIO_SIZE`).
pub(crate) const REGISTER_SIZE: u64 = 0x4000;

// Control register bits this driver sets or clears (section 3.3.1, MMIO 0018h).
const CONTROL_IOMMU_EN: u64 = 1 << 0;
const CONTROL_EVENT_LOG_EN: u64 = 1 << 2;
const CONTROL_EVENT_INT_EN: u64 = 1 << 3;
const CONTROL_COM_WAIT_INT_EN: u64 = 1 << 4;
const CONTROL_CMD_BUF_EN: u64 = 1 << 12;

// Exclusion Base register bits (section 3.3.1, MMIO 0020h): `ExEn` turns the range on, for every
// device whose entry sets `EX` (Table 7, bit 103) or, with `Allow`, for every device whatever its
// entry says.
// The base and the limit (MMIO 0028h) both carry the address in bits 51:12.
const EXCLUSION_EN: u64 = 1 << 0;
const EXCLUSION_ALLOW: u64 = 1 << 1;
const EXCLUSION_ADDRESS: u64 = 0x000f_ffff_ffff_f000;

// Status register bits (section 3.3.1, MMIO 2020h).
const STATUS_EVENT_OVERFLOW: u64 = 1 << 0;
const STATUS_EVENT_LOG_RUN: u64 = 1 << 3;
const STATUS_CMD_BUF_RUN: u64 = 1 << 4;

// Extended Feature Register (MMIO 0030h): `IASup` is bit 6, `HATS` bits 11:10.
const EFR_IA_SUP: u64 = 1 << 6;
const EFR_HATS_SHIFT: u64 = 10;
/// `HATS` = 11b: QEMU writes it when host translation is off (`dma-translation=off`), and revision
/// 2.62 calls it reserved. Either way the unit cannot walk the four levels a domain here has.
const EFR_HATS_NONE: u64 = 0b11;

/// Both rings are one page: 256 entries of 16 bytes, the smallest length the hardware takes
/// (`ComLen` and `EventLen` = 1000b).
const RING_BYTES: u64 = 4096;
const RING_LEN_256: u64 = 0b1000 << 56;

// --- Device table entry, 256 bits as four qwords (section 2.2.2.1, Table 7). The two this driver
// writes are built by `paging::AmdVi`, where Kani proves them; these name fields for the tests. ---
#[cfg(test)]
const DTE_ROOT_MASK: u64 = 0x000f_ffff_ffff_f000;
/// `IV`, bit 128: the interrupt-remapping half of the entry is valid. Never set here.
#[cfg(test)]
const DTE_IV: u64 = 1 << 0;
const DTE_BYTES: u64 = 32;

// Command opcodes, bits 63:60 of the first qword (section 2.4).
const CMD_COMPLETION_WAIT: u64 = 0x1 << 60;
const CMD_INVALIDATE_DEVTAB_ENTRY: u64 = 0x2 << 60;
const CMD_INVALIDATE_IOMMU_PAGES: u64 = 0x3 << 60;
const CMD_INVALIDATE_IOMMU_ALL: u64 = 0x8 << 60;
/// `COMPLETION_WAIT`'s `s` bit: store the second qword at the address in the first.
const COMPLETION_WAIT_STORE: u64 = 1 << 0;
/// `INVALIDATE_IOMMU_PAGES` over a whole domain: `S` and `PDE` set, address `7_FFFF_FFFF_FFFFh`
/// (section 2.4.3's software note says exactly this invalidates everything for the domain).
const INVALIDATE_WHOLE_DOMAIN: u64 = 0x7fff_ffff_ffff_f000 | 0b011;

/// **The device table entry of a device that is confined**: valid, translating through the
/// four-level table at `root`, read and write allowed so the leaves decide, tagged `domain`
/// ([`paging::AmdVi::device_table_entry`], which `no_amd_vi_device_table_entry_sets_a_reserved_bit`
/// proves over every root and domain). A root the entry cannot hold is a panic, not a mask: the
/// mask that was here before provisional milestone 767 would have pointed the device at another
/// table. Every root comes from the frame allocator, so the panic is unreachable on a machine whose
/// memory ends below 2^52.
fn translating_dte(root: u64, domain: u16) -> [u64; 4] {
    paging::AmdVi::device_table_entry(root, domain)
        .unwrap_or_else(|| panic!("AMD-Vi domain root {root:#x} does not fit a device table entry"))
}

/// **The device table entry that denies everything**: the default for every device and §163's
/// quarantine ([`paging::AmdVi::blocked_device_table_entry`]). A four-level walk to `empty_root`,
/// an all-zero table, with read and write both clear; every access target-aborts and is logged.
/// This module's header says why it is not `Mode` 0. Domain 0, which no confined device is given,
/// because every blocked device shares the one root and the specification asks that devices
/// sharing a domain id share their tables.
fn blocked_dte(empty_root: u64) -> [u64; 4] {
    paging::AmdVi::blocked_device_table_entry(empty_root)
        .unwrap_or_else(|| panic!("AMD-Vi empty root {empty_root:#x} does not fit an entry"))
}

/// One fault, in the portable shape VT-d's driver returns.
pub use super::iommu::Fault;

/// One AMD-Vi unit this kernel brought up.
struct Unit {
    base: u64,
    /// The device table: `entries` entries of 32 bytes, physically contiguous.
    device_table: u64,
    entries: u32,
    /// The empty top-level table every blocked entry points at.
    empty_root: u64,
    command_ring: u64,
    /// The offset of the next command slot, as the tail register holds it.
    command_tail: u64,
    event_log: u64,
    /// One 8-byte word the unit writes on `COMPLETION_WAIT`, and the value last asked for.
    semaphore: u64,
    sequence: u64,
    /// `EFR.IASup`: the unit takes `INVALIDATE_IOMMU_ALL`.
    invalidate_all: bool,
    next_domain: u32,
    /// Set once `IommuEn` is: before that the unit reads no commands, so `attach` writes the
    /// entry alone and `init`'s flush covers it.
    enabled: bool,
    /// How many devices an IVMD had attached before the unit was enabled.
    reserved_devices: u32,
    /// What the firmware left in the exclusion registers, which `set_up` cleared.
    exclusion: ExclusionFound,
    /// **Which device holds each alias entry it wrote**, as (entry, device). An entry absent here
    /// that is not blocked is held by the device whose id it is. Kept because the device table
    /// cannot say who wrote an entry, and [`attach`] must refuse to hand one device's alias to
    /// another (provisional milestone 767).
    aliases: [Option<(u32, u32)>; MAX_ALIASES],
}

/// How many alias entries one unit records the holder of. QEMU's machine has none in use; a client
/// AMD board has a handful of PCIe-to-PCI bridges at most. An attach that would need a slot past
/// this fails closed, like a conflict. Name: provisional.
const MAX_ALIASES: usize = 32;

// `Up` is several hundred bytes since the alias holder record (provisional milestone 767) and the
// others are a pointer at most. Boxing it would buy nothing: the slots live in one static array of
// `MAX_IVHDS`, built once at boot and never moved.
#[allow(clippy::large_enum_variant)]
enum Slot {
    Absent,
    Up(Unit),
    Refused(&'static str),
}

static UNITS: IrqSafeMutex<[Slot; MAX_IVHDS]> =
    IrqSafeMutex::new(rank::IOMMU, [const { Slot::Absent }; MAX_IVHDS]);

/// The IVRS as [`init`] read it, kept so `attach` can route a device to its unit. Never held at
/// the same time as [`UNITS`].
static IVRS: IrqSafeMutex<Option<IvrsUnits>> = IrqSafeMutex::new(rank::IOMMU, None);

fn r64(base: u64, off: u64) -> u64 {
    // SAFETY: the unit's register file lies inside the direct map, mapped device-typed by
    // `mmu::map_everything` from `memory::amd_vi_regions`; these are aligned 64-bit registers.
    unsafe { core::ptr::read_volatile(phys_to_virt(base + off) as *const u64) }
}
fn w64(base: u64, off: u64, v: u64) {
    // SAFETY: as above.
    unsafe { core::ptr::write_volatile(phys_to_virt(base + off) as *mut u64, v) }
}

/// Write one device table entry, upper qwords first and the one carrying `V` last, so a unit
/// that reads it between the stores sees the old first qword with new upper halves, never a new
/// first qword with stale ones.
fn write_dte(table: u64, id: u32, dte: [u64; 4]) {
    let at = phys_to_virt(table + id as u64 * DTE_BYTES) as *mut u64;
    // SAFETY: `table` is a kernel-owned contiguous run of `entries` entries and every caller
    // checks `id < entries`; the direct map reaches it.
    unsafe {
        for i in (1..4).rev() {
            core::ptr::write_volatile(at.add(i), dte[i]);
        }
        crate::arch::direct_memory_access_write_barrier();
        core::ptr::write_volatile(at, dte[0]);
    }
}

fn read_dte(table: u64, id: u32) -> [u64; 4] {
    let at = phys_to_virt(table + id as u64 * DTE_BYTES) as *const u64;
    // SAFETY: as in `write_dte`; read only.
    unsafe { core::array::from_fn(|i| core::ptr::read_volatile(at.add(i))) }
}

fn spin_until(what: &str, mut done: impl FnMut() -> bool) {
    for _ in 0..10_000_000u32 {
        if done() {
            return;
        }
        core::hint::spin_loop();
    }
    panic!("AMD-Vi {what} never completed");
}

impl Unit {
    /// Queue one command. Waits for a free slot first: the ring is full when advancing the tail
    /// would make it equal the head.
    fn command(&mut self, first: u64, second: u64) {
        let next = (self.command_tail + 16) % RING_BYTES;
        let base = self.base;
        spin_until("command ring drain", || {
            r64(base, COMMAND_HEAD) & 0x7fff0 != next
        });
        let at = phys_to_virt(self.command_ring + self.command_tail) as *mut u64;
        // SAFETY: `command_ring` is a kernel-owned page and `command_tail` a 16-byte-aligned
        // offset inside it; the unit does not read a slot until the tail register passes it.
        unsafe {
            core::ptr::write_volatile(at, first);
            core::ptr::write_volatile(at.add(1), second);
        }
        // The command is in memory before the tail write that tells the unit to read it.
        crate::arch::direct_memory_access_write_barrier();
        self.command_tail = next;
        w64(self.base, COMMAND_TAIL, next);
    }

    /// **Wait until every command queued so far has finished** (section 2.4.1): a
    /// `COMPLETION_WAIT` that stores a fresh sequence number, then a bounded poll for it.
    fn sync(&mut self) {
        self.sequence += 1;
        let seq = self.sequence;
        self.command(
            CMD_COMPLETION_WAIT | (self.semaphore & 0x000f_ffff_ffff_fff8) | COMPLETION_WAIT_STORE,
            seq,
        );
        let at = phys_to_virt(self.semaphore) as *const u64;
        let base = self.base;
        spin_until("completion wait", || {
            // SAFETY: `semaphore` is a kernel-owned word the unit writes; read only.
            unsafe { core::ptr::read_volatile(at) == seq }
        });
        // A halted command processor (an ILLEGAL_COMMAND_ERROR) never stores, so a timeout above
        // panics; this checks the opposite surprise, a store with the processor stopped.
        assert_ne!(r64(base, STATUS) & STATUS_CMD_BUF_RUN, 0);
    }

    fn invalidate_entry(&mut self, id: u32) {
        self.command(CMD_INVALIDATE_DEVTAB_ENTRY | id as u64, 0);
    }

    fn invalidate_domain(&mut self, domain: u16) {
        self.command(
            CMD_INVALIDATE_IOMMU_PAGES | ((domain as u64) << 32),
            INVALIDATE_WHOLE_DOMAIN,
        );
    }

    fn domain(&mut self) -> u16 {
        assert!(
            self.next_domain <= 0xffff,
            "AMD-Vi unit {:#x} has handed out every domain id",
            self.base
        );
        let d = self.next_domain as u16;
        self.next_domain += 1;
        d
    }

    /// **Which device's DMA entry `id` translates**, or `None` when it is blocked (or past the
    /// table, where the hardware target-aborts).
    fn holder(&self, id: u32) -> Option<u32> {
        if id >= self.entries || read_dte(self.device_table, id) == blocked_dte(self.empty_root) {
            return None;
        }
        Some(
            self.aliases
                .iter()
                .flatten()
                .find(|&&(entry, _)| entry == id)
                .map_or(id, |&(_, owner)| owner),
        )
    }

    fn alias_slot_free(&self) -> bool {
        self.aliases.iter().any(Option::is_none)
    }

    /// Record that `owner` holds alias entry `entry`, replacing any earlier record of it.
    fn record_alias(&mut self, entry: u32, owner: u32) {
        self.forget_alias(entry);
        if let Some(slot) = self.aliases.iter_mut().find(|s| s.is_none()) {
            *slot = Some((entry, owner));
        }
    }

    fn forget_alias(&mut self, entry: u32) {
        for slot in self.aliases.iter_mut() {
            if matches!(slot, Some((e, _)) if *e == entry) {
                *slot = None;
            }
        }
    }

    /// Set entry `id` to the blocked entry and forget who held it. The caller syncs.
    fn block(&mut self, id: u32) {
        self.forget_alias(id);
        let dte = blocked_dte(self.empty_root);
        self.set_entry(id, dte);
    }

    /// Point device table entry `id` at `dte` and, once the unit reads commands, make it forget
    /// both the entry and every translation it cached under the entry's previous domain (section
    /// 2.4.2: `INVALIDATE_DEVTAB_ENTRY` alone leaves the translation cache). The caller syncs. See
    /// notes/amd-vi.md.
    fn set_entry(&mut self, id: u32, dte: [u64; 4]) {
        if id >= self.entries {
            return;
        }
        let previous = read_dte(self.device_table, id)[1] as u16;
        write_dte(self.device_table, id, dte);
        if self.enabled {
            self.invalidate_entry(id);
            self.invalidate_domain(previous);
        }
    }
}

fn zeroed_frames(count: usize, what: &str) -> u64 {
    crate::memory::alloc_contiguous_zeroed(count)
        .unwrap_or_else(|| panic!("no {count} frame(s) for the AMD-Vi {what}"))
        .addr()
}

/// **What a unit's exclusion registers held when this kernel cleared them**: the raw Exclusion Base
/// and Exclusion Range Limit (MMIO 0020h and 0028h). Only [`clear_exclusion_range`] makes one, and
/// [`Unit`] cannot be built without one, so a `set_up` that forgot to clear the range would not
/// compile. Name: provisional (provisional milestone 767 (AMD-Vi hardening before the first AMD
/// boot)).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExclusionFound {
    base: u64,
    limit: u64,
}

impl ExclusionFound {
    /// Was the range in force? `ExEn` is the enable; `Allow` widens it from the devices whose entry
    /// sets `EX` (none here) to every device, and means nothing without `ExEn`.
    fn was_enabled(&self) -> bool {
        self.base & EXCLUSION_EN != 0
    }
}

impl core::fmt::Display for ExclusionFound {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.base == 0 && self.limit == 0 {
            return write!(f, "no firmware exclusion range");
        }
        write!(
            f,
            "FIRMWARE EXCLUSION RANGE {:#x}..={:#x} ({}{}) CLEARED",
            self.base & EXCLUSION_ADDRESS,
            (self.limit & EXCLUSION_ADDRESS) | 0xfff,
            if self.was_enabled() {
                "enabled"
            } else {
                "disabled"
            },
            if self.base & EXCLUSION_ALLOW != 0 {
                ", every device"
            } else {
                ""
            },
        )
    }
}

/// **Switch off the unit's exclusion range, and say what it was** (provisional milestone 767
/// (AMD-Vi hardening before the first AMD boot)).
///
/// Inside an enabled exclusion range every device's access goes to memory untranslated, whatever
/// its entry says (section 3.3.1, MMIO 0020h and 0028h), and nothing clears it at reset. Both
/// registers are zeroed, `ExEn` and `Allow` with them. A range an IVMD names is mapped into its
/// devices' domains instead, as Linux does. QEMU 11.1.1 never consults these registers, so under
/// QEMU this is proved to write them and nothing more. See notes/amd-vi.md.
fn clear_exclusion_range(unit_base: u64) -> ExclusionFound {
    let found = ExclusionFound {
        base: r64(unit_base, EXCLUSION_BASE),
        limit: r64(unit_base, EXCLUSION_LIMIT),
    };
    w64(unit_base, EXCLUSION_BASE, 0);
    w64(unit_base, EXCLUSION_LIMIT, 0);
    found
}

/// **Give one unit an all-blocked device table, a command ring and an event log**, with the unit
/// still disabled. `Err` is a unit this driver will not drive, with the reason the boot prints.
fn set_up(ivrs: &IvrsUnits, i: usize) -> Result<Unit, &'static str> {
    let d = ivrs.units()[i];
    if d.segment != 0 {
        return Err("not on PCI segment 0, the only configuration space this kernel reads");
    }
    let base = d.register_base;
    // The EFR is defined only when the unit says it has one (`IVinfo.EFRSup`, mirroring the
    // capability header's bit); without it, read nothing and assume the 2.62 baseline.
    let efr = if ivrs.ivinfo & 1 != 0 {
        r64(base, EXTENDED_FEATURES)
    } else {
        0
    };
    if (efr >> EFR_HATS_SHIFT) & 0b11 == EFR_HATS_NONE {
        return Err("no host address translation (EFR.HATS = 11b)");
    }

    // Firmware may hand over with the unit on. Turn it off before pointing it anywhere new, the
    // same reasoning VT-d's `root_up` gives for clearing `TE`; the base registers may not be
    // written while their rings run (section 3.3.1, MMIO 0008h to 0018h).
    let control = r64(base, CONTROL);
    let running = CONTROL_IOMMU_EN
        | CONTROL_EVENT_LOG_EN
        | CONTROL_CMD_BUF_EN
        | CONTROL_EVENT_INT_EN
        | CONTROL_COM_WAIT_INT_EN;
    if control & running != 0 {
        w64(base, CONTROL, control & !running);
        spin_until("disable", || {
            r64(base, STATUS) & (STATUS_CMD_BUF_RUN | STATUS_EVENT_LOG_RUN) == 0
        });
    }
    // With the unit off, before anything is pointed at it.
    let exclusion = clear_exclusion_range(base);

    // The device table covers every id the IVRS gives this unit, rounded up to a whole page; the
    // hardware target-aborts any id past its end (section 2.2.2).
    let ids = ivrs.last_device_id(i) as u64 + 1;
    let pages = (ids * DTE_BYTES).div_ceil(page_frames::FRAME_SIZE);
    let entries = (pages * page_frames::FRAME_SIZE / DTE_BYTES) as u32;
    let device_table = zeroed_frames(pages as usize, "device table");
    let empty_root = zeroed_frames(1, "blocked domain root");
    for id in 0..entries {
        write_dte(device_table, id, blocked_dte(empty_root));
    }
    let command_ring = zeroed_frames(1, "command ring");
    let event_log = zeroed_frames(1, "event log");
    let semaphore = zeroed_frames(1, "completion word");
    crate::arch::direct_memory_access_write_barrier();

    // `Size` is pages minus one (section 3.3.1, MMIO 0000h). Writing a ring's base resets its head and tail
    // (section 2.4; QEMU does the same in `amdvi_handle_cmdbase_write`).
    w64(base, DEVICE_TABLE_BASE, device_table | (pages - 1));
    w64(base, COMMAND_BUFFER_BASE, command_ring | RING_LEN_256);
    w64(base, COMMAND_HEAD, 0);
    w64(base, COMMAND_TAIL, 0);
    w64(base, EVENT_LOG_BASE, event_log | RING_LEN_256);
    w64(base, EVENT_HEAD, 0);
    w64(base, EVENT_TAIL, 0);

    Ok(Unit {
        base,
        device_table,
        entries,
        empty_root,
        command_ring,
        command_tail: 0,
        event_log,
        semaphore,
        sequence: 0,
        invalidate_all: efr & EFR_IA_SUP != 0,
        // Domain 0 is the blocked domain every unconfined device shares.
        next_domain: 1,
        enabled: false,
        reserved_devices: 0,
        exclusion,
        aliases: [None; MAX_ALIASES],
    })
}

/// **Turn one unit on and empty its caches.** The command ring and the event log first, then the
/// unit, as Linux orders it (`early_enable_iommu`, from memory); the unit reads no command until
/// `IommuEn` is set, so the flush comes after.
fn enable(u: &mut Unit) {
    let control = r64(u.base, CONTROL) | CONTROL_CMD_BUF_EN | CONTROL_EVENT_LOG_EN;
    w64(u.base, CONTROL, control);
    w64(u.base, CONTROL, control | CONTROL_IOMMU_EN);
    let base = u.base;
    spin_until("enable", || {
        r64(base, STATUS) & (STATUS_CMD_BUF_RUN | STATUS_EVENT_LOG_RUN)
            == STATUS_CMD_BUF_RUN | STATUS_EVENT_LOG_RUN
    });
    u.enabled = true;

    // Every entry, by name: the specification's way to make the unit reload one, and the only
    // way QEMU's model turns a device's translation on at all (notes/amd-vi.md's BUGS).
    for id in 0..u.entries {
        u.invalidate_entry(id);
    }
    if u.invalidate_all {
        u.command(CMD_INVALIDATE_IOMMU_ALL, 0);
    } else {
        // Without `INVALIDATE_IOMMU_ALL`, every domain id firmware might have cached under.
        for domain in 0..=0xffffu16 {
            u.invalidate_domain(domain);
        }
    }
    u.sync();
}

/// **Bring every AMD-Vi unit the IVRS names up, each translating the devices it serves.** Three
/// steps, in the order VT-d's `init` takes and for the same reason:
///
/// 1. Every unit gets an all-blocked device table and its rings, still disabled.
/// 2. Every device an IVMD names (types 21h and 22h) is confined, through
///    [`crate::iommu::confine`] with no grant of its own, to exactly its reserved regions.
/// 3. Each unit is enabled and its caches emptied.
///
/// One `amd-vi` line per unit.
pub fn init(ivrs: &IvrsUnits) {
    *IVRS.lock() = Some(*ivrs);
    {
        let mut g = UNITS.lock();
        assert!(
            g.iter().all(|s| matches!(s, Slot::Absent)),
            "AMD-Vi initialized twice"
        );
        for i in 0..ivrs.units().len() {
            g[i] = match set_up(ivrs, i) {
                Ok(u) => Slot::Up(u),
                Err(why) => Slot::Refused(why),
            };
        }
    }

    // Step 2. `confine` asks `for_each_reserved_region` for every region that names the device,
    // so a device two IVMDs name is confined once with both.
    let mut done = 0u32;
    for r in ivrs.unity_regions() {
        if r.first == 0 && r.last == 0xffff {
            continue; // type 20h: see the BUGS in the AMD-Vi note
        }
        for id in r.first..=r.last {
            if done == 256 {
                break; // a bound, not a rule: see the AMD-Vi note's BUGS on IVMDs
            }
            if let Ok(Some((i, _))) = ivrs.owner_index(id) {
                crate::iommu::confine(id as u32, &[]);
                done += 1;
                if let Slot::Up(u) = &mut UNITS.lock()[i] {
                    u.reserved_devices += 1;
                }
            }
        }
    }

    // Step 3.
    let mut g = UNITS.lock();
    for s in g.iter_mut() {
        if let Slot::Up(u) = s {
            enable(u);
        }
    }
    let n = ivrs.units().len();
    for (i, d) in ivrs.units().iter().enumerate() {
        match &g[i] {
            Slot::Up(u) => crate::println!(
                "  amd-vi      : unit {:#x} up ({} of {n}, ivhd type {:#x}), {} device table \
                 entries all blocked, translation enabled (status confirmed), {} ivmd device(s) \
                 mapped, {}",
                d.register_base,
                i + 1,
                d.kind,
                u.entries,
                u.reserved_devices,
                u.exclusion,
            ),
            Slot::Refused(why) => crate::println!(
                "  amd-vi      : unit {:#x} NOT up ({} of {n}): {why}; the devices it serves are \
                 not translated",
                d.register_base,
                i + 1,
            ),
            Slot::Absent => {}
        }
    }
}

/// Is any AMD-Vi unit up?
pub fn is_active() -> bool {
    UNITS.lock().iter().any(|s| matches!(s, Slot::Up(_)))
}

/// The unit that serves `rid`, when it is one this kernel brought up.
fn owner_index(rid: u32) -> Option<usize> {
    let ivrs = (*IVRS.lock())?;
    match ivrs.owner_index(rid as u16) {
        Ok(Some((i, _))) => Some(i),
        _ => None,
    }
}

/// **Every device table entry device `rid`'s DMA can arrive under**: its own and, when an IVRS
/// alias entry covers it, the source id its transactions actually carry (section 5.2.2.1's alias
/// entries, types 42h and 43h). The second is `None` when there is no alias.
fn entries_of(rid: u32) -> (u32, Option<u32>) {
    let source = match *IVRS.lock() {
        Some(ivrs) => ivrs.source_id(rid as u16) as u32,
        None => rid,
    };
    (rid, (source != rid).then_some(source))
}

/// **Confine device `rid` to the domain rooted at `root`** (an [`paging::x86_64::AmdVi`] table
/// the portable seam built), in the unit the IVRS says serves it, and make that unit forget what
/// it cached. It writes every entry [`entries_of`] names: the device's own and its alias's, where a
/// device behind a PCIe-to-PCI bridge is translated. A device no unit serves gets no entry.
///
/// An alias already held by another device is never shared (provisional milestone 767 (AMD-Vi
/// hardening before the first AMD boot)): the attach fails closed, setting every entry either
/// device's DMA arrives under to the blocked entry, and the boot prints the pair in capitals.
pub fn attach(rid: u32, root: u64) {
    let Some(i) = owner_index(rid) else {
        return;
    };
    let (own, alias) = entries_of(rid);
    let mut g = UNITS.lock();
    let Slot::Up(u) = &mut g[i] else {
        return;
    };
    let ids = [Some(own), alias];
    let conflict = ids
        .into_iter()
        .flatten()
        .find_map(|x| u.holder(x).filter(|&h| h != rid).map(|h| (x, h)));
    // A new alias needs a slot to record its holder in; without one, fail closed as for a conflict.
    let out_of_slots = alias.is_some_and(|a| u.holder(a) != Some(rid) && !u.alias_slot_free());
    if conflict.is_some() || out_of_slots {
        for x in ids.into_iter().flatten() {
            u.block(x);
        }
        if u.enabled {
            u.sync();
        }
        drop(g);
        match conflict {
            Some((shared, other)) => crate::println!(
                "  amd-vi      : DEVICES {rid:#06x} AND {other:#06x} BOTH REACH THE UNIT AS \
                 {shared:#06x}; they cannot be confined apart, so every entry of {rid:#06x} is \
                 BLOCKED (fail closed)"
            ),
            None => crate::println!(
                "  amd-vi      : NO SLOT TO RECORD {rid:#06x}'s ALIAS ({MAX_ALIASES} in use), so \
                 every entry of {rid:#06x} is BLOCKED (fail closed)"
            ),
        }
        return;
    }
    let domain = u.domain();
    let dte = translating_dte(root, domain);
    for x in ids.into_iter().flatten() {
        u.set_entry(x, dte);
    }
    if let Some(a) = alias {
        u.record_alias(a, rid);
    }
    if u.enabled {
        u.sync();
    }
}

/// **Switch device `rid` to the blocked entry** (§163's quarantine; milestone 102 is its caller).
/// Every entry the device's DMA can arrive under is reset, its alias's included (provisional
/// milestone 767): a quarantine that left the alias translating left the DMA of a device behind a
/// bridge exactly where it was. If another device shares the alias, it loses its DMA too, which is
/// unavoidable: the unit cannot tell the two apart.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn quarantine(rid: u32) {
    let Some(i) = owner_index(rid) else {
        return;
    };
    let (own, alias) = entries_of(rid);
    let mut g = UNITS.lock();
    let Slot::Up(u) = &mut g[i] else {
        return;
    };
    for x in [Some(own), alias].into_iter().flatten() {
        u.block(x);
    }
    if u.enabled {
        u.sync();
    }
}

/// **Every IVMD the firmware declared for `rid`**, as regions its domain must also map.
pub(crate) fn for_each_reserved_region(rid: u32, each: &mut dyn FnMut(paging::domain::DmaRegion)) {
    let Some(ivrs) = *IVRS.lock() else {
        return;
    };
    ivrs.unity_for(rid as u16, &mut |r| {
        each(paging::domain::DmaRegion {
            base: r.base,
            size: r.size,
            writable: r.device_may_write(),
        });
    });
}

/// **Which unit, if any, translates `rid`, and why?** The same answer VT-d's `scope_of` gives,
/// from the IVRS's device ids rather than the DMAR's paths.
pub(crate) fn scope_of(rid: u32) -> crate::iommu::Scope {
    use crate::iommu::Scope;
    let mut up = [None; MAX_IVHDS];
    let mut translating = None;
    for (i, s) in UNITS.lock().iter().enumerate() {
        if let Slot::Up(u) = s {
            up[i] = Some(u.base);
            translating.get_or_insert(u.base);
        }
    }
    let Some(translating) = translating else {
        return Scope::NoIommu;
    };
    let Some(ivrs) = *IVRS.lock() else {
        return Scope::Unknown { translating };
    };
    match ivrs.owner_index(rid as u16) {
        Ok(Some((i, how))) => match up[i] {
            Some(unit) => Scope::Owned { unit, how },
            None => Scope::Elsewhere {
                translating,
                owner: Some(ivrs.units()[i].register_base),
            },
        },
        Ok(None) => Scope::Elsewhere {
            translating,
            owner: None,
        },
        Err(()) => Scope::Unknown { translating },
    }
}

/// **Pop one event, if any unit logged one** (section 2.5). The record's device id, its event
/// code (bits 63:60 of the first qword; 2 is `IO_PAGE_FAULT`) and its address. An overflowed log
/// records nothing until software clears `EventOverflow` and restarts logging (section 3.3.1,
/// status register), so that happens here first, as VT-d's `take_fault` clears `PFO`.
pub fn take_fault() -> Option<Fault> {
    let g = UNITS.lock();
    for slot in g.iter() {
        let Slot::Up(u) = slot else { continue };
        if r64(u.base, STATUS) & STATUS_EVENT_OVERFLOW != 0 {
            w64(u.base, STATUS, STATUS_EVENT_OVERFLOW);
            let control = r64(u.base, CONTROL);
            w64(u.base, CONTROL, control & !CONTROL_EVENT_LOG_EN);
            w64(u.base, CONTROL, control | CONTROL_EVENT_LOG_EN);
        }
        let head = r64(u.base, EVENT_HEAD) & 0x7fff0;
        if head == r64(u.base, EVENT_TAIL) & 0x7fff0 {
            continue;
        }
        let at = phys_to_virt(u.event_log + head) as *const u64;
        // SAFETY: `event_log` is a kernel-owned page and `head` a 16-byte offset inside it, behind
        // the tail the unit has published.
        let (first, address) = unsafe {
            (
                core::ptr::read_volatile(at),
                core::ptr::read_volatile(at.add(1)),
            )
        };
        w64(u.base, EVENT_HEAD, (head + 16) % RING_BYTES);
        return Some(Fault {
            rid: (first & 0xffff) as u32,
            code: (first >> 60) as u32,
            // A zero address is QEMU 11.1.1's model writing none (see `Fault::addr`); a real fault
            // at device address 0 is reported as unknown with it, which no test aims at.
            addr: (address != 0).then_some(address & !0xfff),
        });
    }
    None
}

/// **This machine's AMD-Vi, for the machine description.** `true` when there was a unit to
/// describe, so VT-d's `print_summary` knows not to print its own "none".
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub(crate) fn print_summary() -> bool {
    let g = UNITS.lock();
    let mut any = false;
    for s in g.iter() {
        match s {
            Slot::Up(u) => {
                any = true;
                crate::println!(
                    "  iommu           : AMD-Vi unit at {:#018x}, device table default-deny, \
                     translating, interrupt remapping not enabled",
                    u.base,
                );
            }
            Slot::Refused(why) => {
                any = true;
                crate::println!("  iommu           : AMD-Vi unit refused: {why}");
            }
            Slot::Absent => {}
        }
    }
    any
}

/// Bench diagnostic, the AMD-Vi side of VT-d's `print_faults`: the unit's status, the entry for
/// `rid`, and how many events are waiting. Clears nothing.
#[cfg(feature = "disk_throughput")]
pub(crate) fn print_faults(rid: u32) {
    let owner = owner_index(rid);
    let g = UNITS.lock();
    let Some(Slot::Up(u)) = owner.map(|i| &g[i]) else {
        crate::println!("diag amd-vi no running unit serves {rid:#06x}");
        return;
    };
    let head = r64(u.base, EVENT_HEAD);
    let tail = r64(u.base, EVENT_TAIL);
    crate::println!(
        "diag amd-vi {:#x} control {:#x} status {:#x} events {}",
        u.base,
        r64(u.base, CONTROL),
        r64(u.base, STATUS),
        ((tail + RING_BYTES - head) % RING_BYTES) / 16,
    );
    if rid < u.entries {
        let e = read_dte(u.device_table, rid);
        crate::println!(
            "diag amd-vi dte[{rid:#06x}] {:#x} {:#x} {:#x} {:#x}",
            e[0],
            e[1],
            e[2],
            e[3]
        );
    }
}

#[cfg(test)]
mod tests {
    //! What a boot on an AMD-Vi machine can say about the unit itself. The confinement claim (a
    //! device's DMA outside its grant is refused) is the escape tests', which run on the same
    //! machine: `virtio::tests` and `system_tests`' NVMe test.

    use super::*;

    /// **Every unit the IVRS names is translating, with both rings running**, read back from the
    /// unit's own status register rather than from what this driver wrote.
    #[test_case]
    fn every_unit_the_ivrs_names_is_translating() {
        let Some(ivrs) = *IVRS.lock() else {
            crate::testing::skip!("this machine's ACPI names no IVRS, so there is no unit to ask");
        };
        let g = UNITS.lock();
        for (i, d) in ivrs.units().iter().enumerate() {
            match &g[i] {
                Slot::Up(u) => {
                    assert_ne!(r64(u.base, CONTROL) & CONTROL_IOMMU_EN, 0);
                    let s = r64(u.base, STATUS);
                    assert_eq!(
                        s & (STATUS_CMD_BUF_RUN | STATUS_EVENT_LOG_RUN),
                        STATUS_CMD_BUF_RUN | STATUS_EVENT_LOG_RUN,
                        "unit {:#x} status {s:#x}: a ring is not running",
                        u.base
                    );
                }
                Slot::Refused(why) => panic!("unit {:#x} was refused: {why}", d.register_base),
                Slot::Absent => panic!("unit {:#x} was never brought up", d.register_base),
            }
        }
    }

    /// **A device nobody confined has the blocked entry**: the IOMMU's own function, which the
    /// IVRS names and nothing ever attaches. This is the default-deny half of the claim, read
    /// from the table the unit walks.
    #[test_case]
    fn a_device_nobody_confined_is_blocked() {
        let Some(ivrs) = *IVRS.lock() else {
            crate::testing::skip!("this machine's ACPI names no IVRS");
        };
        let g = UNITS.lock();
        for (i, d) in ivrs.units().iter().enumerate() {
            let Slot::Up(u) = &g[i] else { continue };
            let id = d.device_id as u32;
            assert!(
                id < u.entries,
                "the unit's own id {id:#x} is past its table"
            );
            assert_eq!(read_dte(u.device_table, id), blocked_dte(u.empty_root));
        }
    }

    /// **A device no unit serves gets no entry anywhere** (the VT-d test's twin). Device id
    /// `0xfe00` is on a bus QEMU's runner never populates, and past the table QEMU's IVRS sizes.
    #[test_case]
    fn a_device_no_unit_serves_gets_no_entry() {
        match scope_of(0xfe00) {
            crate::iommu::Scope::Elsewhere { owner: None, .. } => {}
            crate::iommu::Scope::NoIommu => crate::testing::skip!("no AMD-Vi unit is up"),
            _ => crate::testing::skip!("device 0xfe00 has an owner on this IVRS"),
        }
        let root = zeroed_frames(1, "test domain root");
        attach(0xfe00, root);
        for s in UNITS.lock().iter() {
            let Slot::Up(u) = s else { continue };
            if 0xfe00 < u.entries {
                assert_eq!(read_dte(u.device_table, 0xfe00), blocked_dte(u.empty_root));
            }
        }
    }

    /// **Interrupt remapping is never enabled**: no entry sets `IV`. The claim VT-d's
    /// `interrupt_remapping_is_reported_and_never_enabled` makes, on this hardware's terms.
    #[test_case]
    fn no_entry_enables_interrupt_remapping() {
        if !is_active() {
            crate::testing::skip!("no AMD-Vi unit is up");
        }
        for s in UNITS.lock().iter() {
            let Slot::Up(u) = s else { continue };
            for id in 0..u.entries {
                assert_eq!(read_dte(u.device_table, id)[2] & DTE_IV, 0, "entry {id:#x}");
            }
        }
    }

    /// **Quarantine is the blocked entry, and it takes effect**: a device is attached, then
    /// quarantined, and its entry reads as the default. Run on the IOMMU's own function, which no
    /// driver owns, so the boot loses nothing.
    #[test_case]
    fn quarantine_restores_the_blocked_entry() {
        let Some(ivrs) = *IVRS.lock() else {
            crate::testing::skip!("this machine's ACPI names no IVRS");
        };
        let id = ivrs.units()[0].device_id as u32;
        let root = zeroed_frames(1, "test domain root");
        attach(id, root);
        {
            let g = UNITS.lock();
            let Slot::Up(u) = &g[0] else { return };
            assert_eq!(read_dte(u.device_table, id)[0] & DTE_ROOT_MASK, root);
        }
        quarantine(id);
        let g = UNITS.lock();
        let Slot::Up(u) = &g[0] else { return };
        assert_eq!(read_dte(u.device_table, id), blocked_dte(u.empty_root));
    }

    /// Two device ids an IVRS alias range covers, and the source id both carry: the first two ids
    /// of the first alias range with room for two. On the `NIFE_IOMMU=amd` machine that is the
    /// runner's empty `pci-bridge`'s secondary bus, aliased to the bridge.
    fn an_alias_pair() -> Option<(u32, u32, u32)> {
        let ivrs = (*IVRS.lock())?;
        let r = ivrs
            .ranges()
            .iter()
            .find(|r| !r.all && r.alias.is_some() && r.first < r.last)?;
        Some((r.first as u32, r.first as u32 + 1, r.alias? as u32))
    }

    /// The unit `rid` belongs to, its table and the root a blocked entry points at.
    fn table_of(rid: u32) -> (u64, u64) {
        let i = owner_index(rid).expect("no unit serves the alias range");
        let g = UNITS.lock();
        let Slot::Up(u) = &g[i] else {
            panic!("the unit serving {rid:#x} is not up")
        };
        (u.device_table, u.empty_root)
    }

    /// **Two devices behind one alias never share a domain** (provisional milestone 767). The
    /// first confined behind the bridge gets the alias entry; the second cannot be confined apart
    /// from it, so the alias is blocked rather than handed over, and neither root is left in it.
    /// Before 767 the second attach silently moved the first device's DMA into its own domain. Run
    /// on ids nothing occupies (the runner's bridge is empty), so no device loses DMA.
    ///
    /// Falsification: replayable `kernel/falsifications/arch.x86_64.amd_vi.tests.two_devices_behind_one_alias_never_share_a_domain.patch`
    #[test_case]
    fn two_devices_behind_one_alias_never_share_a_domain() {
        let Some((first, second, alias)) = an_alias_pair() else {
            crate::testing::skip!("no IVRS alias range here (the AMD-Vi runner adds a pci-bridge)");
        };
        let (table, empty) = table_of(first);
        let (a, b) = (
            zeroed_frames(1, "test root a"),
            zeroed_frames(1, "test root b"),
        );
        attach(first, a);
        assert_eq!(
            read_dte(table, alias)[0] & DTE_ROOT_MASK,
            a,
            "the alias is not the first's"
        );
        attach(second, b);
        let held = read_dte(table, alias);
        assert_ne!(
            held[0] & DTE_ROOT_MASK,
            b,
            "the second device took the first's alias"
        );
        assert_eq!(
            held,
            blocked_dte(empty),
            "a shared alias was left translating"
        );
        assert_eq!(read_dte(table, second), blocked_dte(empty));
        quarantine(first);
        quarantine(second);
    }

    /// **A quarantine resets the alias as well as the device's own entry** (provisional milestone
    /// 767), and confining the same device again takes its alias back rather than reading as a
    /// conflict with itself.
    ///
    /// Falsification: replayable `kernel/falsifications/arch.x86_64.amd_vi.tests.a_quarantine_blocks_the_alias_too.patch`
    #[test_case]
    fn a_quarantine_blocks_the_alias_too() {
        let Some((first, _, alias)) = an_alias_pair() else {
            crate::testing::skip!("no IVRS alias range here (the AMD-Vi runner adds a pci-bridge)");
        };
        let (table, empty) = table_of(first);
        let (a, b) = (
            zeroed_frames(1, "test root a"),
            zeroed_frames(1, "test root b"),
        );
        attach(first, a);
        attach(first, b);
        assert_eq!(
            read_dte(table, alias)[0] & DTE_ROOT_MASK,
            b,
            "a second confine of the same device lost its own alias"
        );
        quarantine(first);
        assert_eq!(read_dte(table, first), blocked_dte(empty));
        assert_eq!(
            read_dte(table, alias),
            blocked_dte(empty),
            "the quarantined device's alias still translates"
        );
    }

    /// **A firmware exclusion range does not survive `set_up`**: every unit's Exclusion Base and
    /// Limit read zero after boot, and [`clear_exclusion_range`] zeroes a range the test sets as
    /// firmware would (provisional milestone 767 (AMD-Vi hardening before the first AMD boot)).
    /// QEMU 11.1.1 drops every write to the base register, so under QEMU this proves the limit half
    /// and that the boot left both zero; the base half and the effect on DMA need silicon. The
    /// range is the last 4 KiB of the 52-bit space, which no machine backs. See notes/amd-vi.md.
    ///
    /// Falsification: replayable `kernel/falsifications/arch.x86_64.amd_vi.tests.a_firmware_exclusion_range_is_cleared.patch`
    #[test_case]
    fn a_firmware_exclusion_range_is_cleared() {
        if !is_active() {
            crate::testing::skip!("no AMD-Vi unit is up");
        }
        const TOP: u64 = 0x000f_ffff_ffff_f000;
        let g = UNITS.lock();
        for s in g.iter() {
            let Slot::Up(u) = s else { continue };
            assert_eq!(r64(u.base, EXCLUSION_BASE), 0, "unit {:#x}", u.base);
            assert_eq!(r64(u.base, EXCLUSION_LIMIT), 0, "unit {:#x}", u.base);

            w64(u.base, EXCLUSION_LIMIT, TOP);
            w64(u.base, EXCLUSION_BASE, TOP | EXCLUSION_ALLOW | EXCLUSION_EN);
            let found = clear_exclusion_range(u.base);
            assert_eq!(
                found.limit, TOP,
                "the limit the test wrote was not what was found"
            );
            assert_eq!(
                r64(u.base, EXCLUSION_LIMIT),
                0,
                "the limit survived the clear"
            );
            assert_eq!(
                r64(u.base, EXCLUSION_BASE),
                0,
                "the base survived the clear"
            );
        }
    }

    /// The two entries this driver writes, spelled against Table 7's bit positions: `V` (0), `TV`
    /// (1), `Mode` (11:9) = 4, the root (51:12), `IR` (61), `IW` (62), the domain id in the second
    /// qword's low 16 bits, and nothing in the reserved bits QEMU checks (6:2 and 63).
    #[test_case]
    fn the_entries_are_what_table_7_says() {
        let t = translating_dte(0x1234_5000, 7);
        assert_eq!(t, [0x6000_0000_1234_5803, 7, 0, 0]);
        let b = blocked_dte(0x9000);
        assert_eq!(b, [0x0000_0000_0000_9803, 0, 0, 0]);
        for q in [t[0], b[0]] {
            assert_eq!(q & ((0b11111 << 2) | (1 << 63)), 0);
        }
    }
}
