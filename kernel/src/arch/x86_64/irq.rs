//! The x86_64 interrupt controller (aarch64's GIC, RISC-V's PLIC): the per-CPU local APIC (EOI,
//! timer, IPIs, the INIT and STARTUP that start a secondary) and the IO APIC, which routes device
//! lines to vectors; also the 8259 mask, legacy routing, MSI. See notes/x86-port/traps.md.
//!
//! # BUGS
//! - One IO APIC: [`route_gsi`] refuses a GSI a second owns (proved, unit-tested, never booted).
//! - Every line and MSI targets the core that armed it, in physical mode; nothing is distributed.
//! - Revoking a driver's `Irq` does not [`mask_gsi`], so a dead driver's COM1 line stays armed.
//! - An MSI vector is never handed back ([`alloc_msi_vector`] bumps), out of a band of 63.
//! - Interrupt remapping is assumed off; a VT-d unit with it on rejects these MSI messages.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use machine_discovery::acpi::{ISA_IRQ_COUNT, IsaIrqRouting};

use super::mmu::phys_to_virt;
use super::port::{in8, out8};

/// The local APIC's registers, as offsets from its base. 32-bit, and **all of them must be accessed
/// as aligned 32-bit words**: an 8- or 16-bit access to an APIC register is undefined.
mod reg {
    /// This CPU's local APIC id, in bits 31:24.
    pub(crate) const ID: u64 = 0x020;
    /// Version, and in bits 23:16 the number of LVT entries minus one.
    pub(crate) const VERSION: u64 = 0x030;
    /// Task Priority. Zero means "accept every vector"; anything else silently drops interrupts
    /// below that priority class, which is a very quiet way to lose them.
    pub(crate) const TPR: u64 = 0x080;
    /// End Of Interrupt. Written (with zero) to acknowledge; until then the local APIC will not
    /// deliver another interrupt of the same or lower priority.
    pub(crate) const EOI: u64 = 0x0b0;
    /// Spurious Interrupt Vector. Bit 8 is the **software enable**, and the low eight bits are the
    /// vector a spurious interrupt arrives on.
    pub(crate) const SPURIOUS: u64 = 0x0f0;
    /// Interrupt Command Register, low word: the vector, the delivery mode, and the destination
    /// shorthand. **Writing this word is what sends the IPI**, so the high word must already be in
    /// place. See [`super::send_ipi`].
    pub(crate) const ICR_LOW: u64 = 0x300;
    /// Interrupt Command Register, high word: the destination local APIC id, in bits 31:24.
    pub(crate) const ICR_HIGH: u64 = 0x310;
    /// Local Vector Table entry for the timer: the vector, the mask bit, and the mode.
    pub(crate) const LVT_TIMER: u64 = 0x320;
    /// What the timer counts down from.
    pub(crate) const TIMER_INITIAL: u64 = 0x380;
    /// What it is at now.
    pub(crate) const TIMER_CURRENT: u64 = 0x390;
    /// How much the timer divides the bus clock by.
    pub(crate) const TIMER_DIVIDE: u64 = 0x3e0;
    /// The Interrupt Request Register, 256 bits as eight 32-bit words 0x10 apart: bit `v` is set
    /// while vector `v` has been accepted by this local APIC and not yet delivered to the core.
    pub(crate) const IRR: u64 = 0x200;
}

/// Bit 8 of the spurious-interrupt register: the local APIC's software enable.
const SPURIOUS_ENABLE: u32 = 1 << 8;

/// **The vector a spurious interrupt arrives on.** 0xff by convention, and the low four bits used to
/// be required to be one on older parts, which is why every kernel picks a vector ending in 0xf.
pub(crate) const SPURIOUS_VECTOR: u8 = 0xff;

/// **The vector the local APIC timer raises.** 0x20, the first vector after the 32 the architecture
/// reserves for exceptions. Not a hardware fact: it is our choice, written into the LVT.
pub(crate) const TIMER_VECTOR: u8 = 0x20;

/// **The vector a reschedule inter-processor interrupt arrives on**, and this architecture's
/// counterpart of aarch64's `sched::RESCHED_SGI` and RISC-V's SBI software interrupt.
///
/// 0x21, immediately after the timer and well below [`GSI_VECTOR_BASE`], so the local APIC's own
/// sources stay grouped in 0x20..0x2f the way [`gsi_vector`]'s comment promises.
///
/// Name: provisional (milestone 161, roadmap item 4): calef names public items.
pub const RESCHEDULE_VECTOR: u8 = 0x21;

/// **The vector `raise_self_interrupt` uses for the scheduler's own interrupt-delivery tests.**
///
/// 0x22, in the same local-APIC band. It is a *test* fixture rather than a device line, and it has
/// to be one: see [`raise_self_interrupt`] for why x86 cannot use its console UART the way RISC-V
/// does, and why a self-IPI is the honest analog of aarch64's software-generated interrupt.
///
/// Name: provisional (milestone 161, roadmap item 4).
pub const SELF_TEST_VECTOR: u8 = 0x22;
/// A second test vector, so two tests cannot see each other's routes (aarch64's two SGIs).
///
/// Name: provisional (milestone 161, roadmap item 4).
pub const SELF_TEST_VECTOR_B: u8 = 0x23;

/// LVT bit 16: masked. Set on every entry at reset, which is why an unmasked entry is a deliberate
/// act.
const LVT_MASKED: u32 = 1 << 16;
/// LVT timer mode bits 18:17 = 01: periodic. The timer reloads its initial count and fires again,
/// rather than firing once.
const LVT_TIMER_PERIODIC: u32 = 1 << 17;

/// Timer divide configuration for "divide by 16". The encoding is not the number: bits 3, 1 and 0
/// form the value with **bit 2 skipped**, which is one of the more gratuitous layouts in the
/// architecture, so it is written as a constant rather than computed.
const TIMER_DIVIDE_16: u32 = 0b0011;

/// The 8259 PICs' command and data ports.
const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_COMMAND: u16 = 0xa0;
const PIC2_DATA: u16 = 0xa1;

/// Where this CPU's local APIC is, as a virtual address. Zero until [`init_local_apic`] runs.
///
/// A static rather than a parameter because every accessor below needs it and the alternative is
/// threading it through the whole interrupt path. It is written once during single-threaded boot.
///
/// **It is a direct-map address and stays valid across `mmu::init`'s `CR3` switch**, which is why
/// this can be computed once. That is not free: `boot.s` installs the direct map at the same base
/// the fine map uses precisely so that `phys_to_virt` never changes meaning (`arch/x86_64/mmu.rs`).
/// What the fine map *does* change is the memory type, from the boot map's cacheable to device.
static LOCAL_APIC: AtomicU64 = AtomicU64::new(0);

/// The **physical** address the local APIC was found at, kept beside the virtual one so that
/// `mmu::init` can map exactly that page device-typed rather than mapping the architectural default
/// and hoping firmware left it there. Zero until [`init_local_apic`] runs.
static LOCAL_APIC_PHYS: AtomicU64 = AtomicU64::new(0);

/// Where this machine's local APIC is, physically, or `None` if ACPI has not said yet.
///
/// Name: provisional (milestone 161 (the kernel port)): `mmu::LOCAL_APIC_PHYS` is the
/// architectural *default* constant and this is what the machine actually reported, which is a
/// distinction worth a better pair of names than these two.
pub fn local_apic_phys() -> Option<u64> {
    match LOCAL_APIC_PHYS.load(Ordering::Relaxed) {
        0 => None,
        base => Some(base),
    }
}

/// Read a local APIC register.
fn read(offset: u64) -> u32 {
    let base = LOCAL_APIC.load(Ordering::Relaxed);
    debug_assert!(base != 0, "the local APIC has not been located");
    // SAFETY: an aligned 32-bit MMIO read of a register the APIC defines, at the address the ACPI
    // MADT stated, reached through the direct map (see `mmu::phys_to_virt`).
    unsafe { core::ptr::read_volatile((base + offset) as *const u32) }
}

/// Write a local APIC register.
fn write(offset: u64, value: u32) {
    let base = LOCAL_APIC.load(Ordering::Relaxed);
    debug_assert!(base != 0, "the local APIC has not been located");
    // SAFETY: as `read`.
    unsafe { core::ptr::write_volatile((base + offset) as *mut u32, value) };
}

/// **Mask every line on both 8259 PICs.**
///
/// Their power-on vector base overlaps the CPU's own exception vectors (a PIC IRQ 0 arrives as
/// vector 8, which is the double fault), so an interrupt from one of these is not merely unowned, it
/// is *misread as a fault the kernel takes seriously*. Masking is enough because nothing here wants
/// to use them; a kernel that did would remap them to 0x20..0x30 first.
///
/// Done before the local APIC is enabled, deliberately: the reverse order leaves a window in which
/// the IDT is live and these are not masked.
fn mask_the_8259s() {
    // SAFETY: writing all-ones to both PICs' interrupt mask registers. The only effect is that they
    // stop raising interrupts, which is the entire intent.
    unsafe {
        out8(PIC1_DATA, 0xff);
        out8(PIC2_DATA, 0xff);
        // A read of the command port, purely to give the (emulated or real) chip a bus cycle to
        // settle. Harmless, and the traditional way to space two 8259 writes.
        let _ = in8(PIC1_COMMAND);
        let _ = in8(PIC2_COMMAND);
    }
}

/// Bring up this CPU's local APIC at physical address `base`.
///
/// `base` comes from the ACPI MADT rather than from a constant, which is the point: the address is
/// architecturally relocatable through `IA32_APIC_BASE`, and reading the table is how the kernel
/// finds out where firmware left it rather than assuming the reset default.
///
/// # Safety
/// `base` must be this machine's real local APIC address. The direct map covers it from `boot.s`
/// on, and `mmu::init` re-maps that page device-typed rather than moving it.
pub(crate) unsafe fn init_local_apic(base: u64) {
    mask_the_8259s();

    LOCAL_APIC_PHYS.store(base, Ordering::Relaxed);
    LOCAL_APIC.store(phys_to_virt(base), Ordering::Relaxed);

    // Accept every priority class. The reset value is already zero on every part this has run on,
    // and writing it is one instruction against a failure (interrupts silently dropped by priority)
    // that looks exactly like a controller that is not wired up.
    write(reg::TPR, 0);

    // Software-enable, with a spurious vector the IDT has a gate for. Until this bit is set the
    // local APIC delivers nothing at all, which is the state the CPU boots in.
    write(reg::SPURIOUS, SPURIOUS_ENABLE | SPURIOUS_VECTOR as u32);
}

/// Acknowledge the interrupt being handled. **Every handler must, and the failure mode is a hang
/// rather than an error**: until the EOI is written the local APIC will not deliver another
/// interrupt of the same or lower priority, so a missed EOI means the timer ticks exactly once.
pub(crate) fn end_of_interrupt() {
    write(reg::EOI, 0);
}

/// This CPU's local APIC id, which is its hardware name and need not be its logical cpu id.
pub(crate) fn local_apic_id() -> u8 {
    (read(reg::ID) >> 24) as u8
}

/// The local APIC's version register's low byte. Printed at boot; an integrated APIC reports 0x10 or
/// higher, an external 82489DX reports below 0x10 and would mean a machine older than this kernel
/// supports.
pub(crate) fn local_apic_version() -> u8 {
    read(reg::VERSION) as u8
}

/// Is the local APIC up? False until [`init_local_apic`] has run, which is what the timer checks
/// before trying to arm anything.
pub(crate) fn is_local_apic_ready() -> bool {
    LOCAL_APIC.load(Ordering::Relaxed) != 0
}

/// Start the timer counting down from `count`, **masked**, so it can be measured without delivering
/// anything. This is what calibration uses: the timer is a free-running counter until something
/// unmasks it.
pub(crate) fn start_timer_for_calibration(count: u32) {
    write(reg::TIMER_DIVIDE, TIMER_DIVIDE_16);
    write(reg::LVT_TIMER, LVT_MASKED | TIMER_VECTOR as u32);
    write(reg::TIMER_INITIAL, count);
}

/// What the timer's countdown is at now.
pub(crate) fn timer_current_count() -> u32 {
    read(reg::TIMER_CURRENT)
}

/// Arm the timer to fire every `count` of its own ticks, on [`TIMER_VECTOR`], forever.
pub(crate) fn arm_periodic_timer(count: u32) {
    write(reg::TIMER_DIVIDE, TIMER_DIVIDE_16);
    write(reg::TIMER_INITIAL, count);
    // Unmasked and periodic. Written last, so the count is already loaded when delivery begins.
    write(reg::LVT_TIMER, LVT_TIMER_PERIODIC | TIMER_VECTOR as u32);
}

/// **Is the timer's vector raised and waiting in this local APIC** (its bit in the IRR)? True from
/// the moment the countdown expires until the core accepts the interrupt, which with `IF` clear is
/// not until interrupts are unmasked. See `timer::is_tick_pending`, the caller.
///
/// Name: ratified 2026-09-24 (calef, the Rust predicate-naming rule in design/naming.md). Refused
/// `timer_pending` (a bare participle reads as a getter, and Rust asks the question with `is_`).
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn is_timer_pending() -> bool {
    let v = TIMER_VECTOR as u64;
    read(reg::IRR + (v / 32) * 0x10) & (1 << (v % 32)) != 0
}

/// Stop the timer delivering.
pub(crate) fn mask_timer() {
    write(reg::LVT_TIMER, LVT_MASKED | TIMER_VECTOR as u32);
}

// ---------------------------------------------------------------------------------------------
// The IO APIC: where a *device* line becomes a vector.
// ---------------------------------------------------------------------------------------------

/// The IO APIC's registers: an index register and a data window, not a flat array. To touch
/// register `n`, write `n` to [`IOREGSEL`] and then read or write [`IOWIN`], so every access is a
/// pair and the device is stateful.
///
/// # BUGS
/// Nothing serializes the pair. A driver's `Irq::ACK` (`enable`) and the trap handler (`disable`)
/// can run on two cores at once, and each would read or write the other's register.
mod io_reg {
    /// The index register, at offset 0. Write the number of the register you want.
    pub(crate) const IOREGSEL: u64 = 0x00;
    /// The data window, at offset 0x10. Reads and writes land on whatever [`IOREGSEL`] last named.
    pub(crate) const IOWIN: u64 = 0x10;

    /// Register 0: this IO APIC's id, in bits 27:24.
    pub(crate) const ID: u32 = 0x00;
    /// Register 1: the version in bits 7:0, and in bits 23:16 **the number of redirection entries
    /// minus one**. That "minus one" is the field's definition, not an off-by-one: a 24-entry part
    /// reports 23.
    pub(crate) const VERSION: u32 = 0x01;
    /// Register 0x10: the first word of redirection entry 0. Each entry is **two** consecutive
    /// registers, low word first, so entry `n` is at `0x10 + 2 * n`.
    pub(crate) const REDIRECTION_BASE: u32 = 0x10;
}

/// Redirection entry bit 16: masked. Every entry powers on masked, which is why arming one is a
/// deliberate act and why [`init_io_apic`] masking them all again is belt and braces rather than
/// necessity.
const REDIR_MASKED: u32 = 1 << 16;
/// Redirection entry bit 15: level triggered rather than edge triggered.
const REDIR_LEVEL: u32 = 1 << 15;
/// Redirection entry bit 13: the input pin is asserted low rather than high.
const REDIR_ACTIVE_LOW: u32 = 1 << 13;

/// The first vector the IO APIC's lines are routed to: 0x30, leaving 0x20..0x2f for the local
/// APIC's own sources (the timer, the IPIs, and the thermal, performance and error LVT entries).
///
/// A GSI's vector is this plus its redirection index within the owning IO APIC (milestone 308 (a
/// GSI reaches its vector by redirection index)), so a stray vector names its entry by subtraction;
/// the GSI needs the IO APIC's base too, and nothing inverts that. The flat map gives 0x30..0x47
/// two priority classes (x86 priority is the vector's top four bits) and no say over which line is
/// in which; nothing has a priority policy yet.
///
/// Name: provisional (milestone 161), along with [`gsi_vector`] and the IO APIC entry points
/// below.
pub const GSI_VECTOR_BASE: u8 = 0x30;

/// The first vector a PCI function's MSI-X message delivers on (milestone 215): 0xc0, above every
/// redirection entry and below [`SPURIOUS_VECTOR`]. The three bands are disjoint by construction:
/// 0x20..0x30 the local APIC's own sources, [`GSI_VECTOR_BASE`]..0xc0 the IO APIC's lines, and
/// 0xc0..0xff MSI. So an intid means one thing: a legacy IRQ for an IO APIC line, and the vector
/// itself for a local APIC source or an MSI, which the device writes straight to the local APIC.
///
/// Name: provisional (milestone 215 (a PCI function's interrupt reaches nothing)): calef
/// names public items.
pub const MSI_VECTOR_BASE: u8 = 0xc0;

/// The most redirection entries this kernel will use: what fits between [`GSI_VECTOR_BASE`] and
/// [`MSI_VECTOR_BASE`]. Real parts have 24 (the 82093AA, q35, the ICH-era chipsets) and the field
/// could report 256. It bounds [`is_device_vector`] and [`init_io_apic`]'s mask loop, and keeps
/// [`gsi_vector`]'s base plus index inside the band, the claim
/// `proofs::an_owned_gsi_routes_inside_the_io_apic_band` checks.
const MAX_REDIRECTION_ENTRIES: u32 = (MSI_VECTOR_BASE - GSI_VECTOR_BASE) as u32;

/// The next MSI vector to hand out, as an offset from [`MSI_VECTOR_BASE`]. A bump counter, never
/// returned: nothing in this tree releases a device, and a freed-vector list with no free path is
/// machinery with no caller (`design/decisions/` §46's posture applied to code we would write
/// ourselves).
static MSI_NEXT: AtomicU32 = AtomicU32::new(0);

/// How many MSI vectors this band holds: 0xc0..0xff, stopping short of [`SPURIOUS_VECTOR`].
const MSI_VECTORS: u32 = SPURIOUS_VECTOR as u32 - MSI_VECTOR_BASE as u32;

/// Reserve a vector a PCI function may deliver an MSI-X message on, and say where to send it.
///
/// Returns the intid a driver binds, which here is the vector, and the message; `None` when the
/// band is spent (aarch64 and riscv64 always answer `None`, and `kernel/src/pci.rs` uses INTx). The
/// address is `0xfee0_0000 | (local APIC id << 12)` with physical destination and no redirection;
/// the data is the vector, `Fixed` delivery, edge trigger, so `enable` and a driver's `Irq::ACK`
/// have nothing to do. With VT-d interrupt remapping on, these messages are rejected (this
/// module's BUGS). See notes/x86-port/traps.md.
///
/// Name: provisional (milestone 215).
pub fn alloc_msi_vector() -> Option<(u32, pci::MsiTarget)> {
    let offset = MSI_NEXT.fetch_add(1, Ordering::Relaxed);
    if offset >= MSI_VECTORS {
        // Do not hand back a vector outside the band: it would land on the spurious vector or on
        // an exception, and the failure would appear as a fault in an unrelated driver.
        return None;
    }
    let vector = MSI_VECTOR_BASE as u32 + offset;
    Some((
        vector,
        pci::MsiTarget {
            address: LOCAL_APIC_MSI_BASE | ((local_apic_id() as u64) << 12),
            data: vector,
        },
    ))
}

/// The base of the local APIC's message address space, which is where an MSI write goes.
const LOCAL_APIC_MSI_BASE: u64 = 0xfee0_0000;

/// **Is `vector` one this kernel handed out for an MSI?** **Provisional name** (milestone 215).
/// The trap handler asks so that an MSI can
/// become a message the way a self-IPI already does. Bounded by what has actually been allocated,
/// so an unclaimed vector in the band is still counted as spurious rather than routed.
pub(crate) fn is_msi_vector(vector: u64) -> bool {
    let base = MSI_VECTOR_BASE as u64;
    vector >= base && vector < base + MSI_NEXT.load(Ordering::Relaxed).min(MSI_VECTORS) as u64
}

/// Where this machine's IO APIC is, as a virtual address. Zero until [`init_io_apic`] runs. The
/// same direct-map reasoning as [`LOCAL_APIC`].
static IO_APIC: AtomicU64 = AtomicU64::new(0);
/// The **physical** address the IO APIC was found at, kept for `mmu::init` for the reason
/// [`LOCAL_APIC_PHYS`] is.
static IO_APIC_PHYS: AtomicU64 = AtomicU64::new(0);
/// The first global system interrupt this IO APIC owns. Usually 0, and a machine with several
/// parts divides the space by giving each a different base.
static IO_APIC_GSI_BASE: AtomicU32 = AtomicU32::new(0);
/// How many redirection entries it has, read from its version register. Zero until then.
static IO_APIC_ENTRIES: AtomicU32 = AtomicU32::new(0);

/// **Which intid armed each redirection entry** (milestone 505 (an x86_64 input driver that never
/// lets the core idle)), so the trap handler can turn the vector it took back into the number a
/// driver bound. [`NO_OWNER`] until [`enable`] arms the entry.
///
/// A table rather than arithmetic, because the arithmetic has no unique answer: the vector gives the
/// redirection index and so the GSI, but a GSI is not a legacy IRQ. The MADT's overrides can send two
/// legacy numbers to one GSI (IRQ 0 and IRQ 2 both resolve to GSI 2 on most PCs), and GSI 0 has no
/// legacy owner at all. The intid that armed the line is the one a driver bound, which is the
/// question the handler asks. Written by `enable`, read in interrupt context, so atomics and no lock.
static GSI_OWNER: [AtomicU32; MAX_REDIRECTION_ENTRIES as usize] =
    [const { AtomicU32::new(NO_OWNER) }; MAX_REDIRECTION_ENTRIES as usize];

/// The value in [`GSI_OWNER`] that means "nothing armed this entry".
const NO_OWNER: u32 = u32::MAX;

/// **The sixteen legacy ISA IRQs as the MADT resolved them**, packed one to a word so the table can
/// live in a static without a lock: the GSI in bits 15:0, "active low" in bit 16, "level triggered"
/// in bit 17. [`NO_ROUTING`] means [`record_isa_routing`] has not run.
///
/// Packed rather than held as a `[IsaIrqRouting; 16]` behind a mutex because it is written once,
/// during single-threaded boot, and read from interrupt-controller code where taking a lock would
/// be the more surprising thing.
static ISA_ROUTING: [AtomicU32; ISA_IRQ_COUNT] =
    [const { AtomicU32::new(NO_ROUTING) }; ISA_IRQ_COUNT];

/// The value in [`ISA_ROUTING`] that means "the MADT has not been read".
const NO_ROUTING: u32 = u32::MAX;

/// Bit 16 of a packed [`ISA_ROUTING`] word.
const PACKED_ACTIVE_LOW: u32 = 1 << 16;
/// Bit 17 of a packed [`ISA_ROUTING`] word.
const PACKED_LEVEL: u32 = 1 << 17;

/// Write the IO APIC's index register, then its data window.
fn io_apic_write(index: u32, value: u32) {
    let base = IO_APIC.load(Ordering::Relaxed);
    debug_assert!(base != 0, "the IO APIC has not been located");
    // SAFETY: two aligned 32-bit MMIO writes to the two registers this device has, at the address
    // the ACPI MADT stated, reached through the direct map. The index write must land before the
    // data write, which `write_volatile` guarantees against another volatile access and TSO
    // guarantees against the device.
    unsafe {
        core::ptr::write_volatile((base + io_reg::IOREGSEL) as *mut u32, index);
        core::ptr::write_volatile((base + io_reg::IOWIN) as *mut u32, value);
    }
}

/// Write the IO APIC's index register, then read its data window.
fn io_apic_read(index: u32) -> u32 {
    let base = IO_APIC.load(Ordering::Relaxed);
    debug_assert!(base != 0, "the IO APIC has not been located");
    // SAFETY: as `io_apic_write`.
    unsafe {
        core::ptr::write_volatile((base + io_reg::IOREGSEL) as *mut u32, index);
        core::ptr::read_volatile((base + io_reg::IOWIN) as *const u32)
    }
}

/// **Bring up the IO APIC at physical address `base`, owning global system interrupts from
/// `gsi_base` up.** Both come from the ACPI MADT.
///
/// Every redirection entry is masked on the way in. They power on masked, so this changes nothing
/// on a cold boot; it matters on a warm one, where firmware may have armed a line for its own use
/// and left it armed, and an inherited interrupt arriving at a vector this kernel has not assigned
/// is a puzzle with no clue in it.
///
/// # Safety
/// `base` must be this machine's real IO APIC address, and the direct map must cover it.
pub(crate) unsafe fn init_io_apic(base: u64, gsi_base: u32) {
    IO_APIC_PHYS.store(base, Ordering::Relaxed);
    IO_APIC.store(phys_to_virt(base), Ordering::Relaxed);
    IO_APIC_GSI_BASE.store(gsi_base, Ordering::Relaxed);

    // The version register's bits 23:16 are the entry count *minus one*.
    let entries = (((io_apic_read(io_reg::VERSION) >> 16) & 0xff) + 1).min(MAX_REDIRECTION_ENTRIES);
    IO_APIC_ENTRIES.store(entries, Ordering::Relaxed);

    for entry in 0..entries {
        let index = io_reg::REDIRECTION_BASE + 2 * entry;
        io_apic_write(index + 1, 0);
        io_apic_write(index, REDIR_MASKED);
    }
}

/// Where this machine's IO APIC is, physically, or `None` if [`init_io_apic`] has not run. Read by
/// `mmu::init` so the fine map covers the address the machine reported rather than the constant.
pub(crate) fn io_apic_phys() -> Option<u64> {
    match IO_APIC_PHYS.load(Ordering::Relaxed) {
        0 => None,
        base => Some(base),
    }
}

/// This IO APIC's id, from its own register rather than from the MADT. Printed at boot because a
/// disagreement between the two is a firmware bug worth seeing rather than averaging over.
pub(crate) fn io_apic_id() -> u8 {
    ((io_apic_read(io_reg::ID) >> 24) & 0x0f) as u8
}

/// Its version register's low byte. 0x11 on the discrete 82093AA and on QEMU's q35, 0x20 on the
/// ICH-era parts.
pub(crate) fn io_apic_version() -> u8 {
    io_apic_read(io_reg::VERSION) as u8
}

/// How many redirection entries it has. Zero until [`init_io_apic`] has run.
pub(crate) fn io_apic_entries() -> u32 {
    IO_APIC_ENTRIES.load(Ordering::Relaxed)
}

/// The vector a global system interrupt is routed to: [`GSI_VECTOR_BASE`] plus its redirection
/// index, not plus the GSI (milestone 308). `None` when this IO APIC does not own the GSI, the
/// partiality [`redirection_index`] has. `proofs::an_owned_gsi_routes_inside_the_io_apic_band`
/// checks the result stays in the band whatever the IO APIC's global interrupt base.
///
/// Name: provisional, and this change makes it worse rather than better (milestone 161 marked it
/// provisional; milestone 308 is the lane that noticed): the function now maps an *index* into the
/// vector space and the GSI is what it takes, not what it adds. `gsi_vector` still describes the
/// question a caller asks, so it is not wrong, but a name naming the index would be more honest.
/// calef names public items; this lane proposes rather than renames.
pub fn gsi_vector(gsi: u32) -> Option<u8> {
    let index = redirection_index(gsi)?;
    // `init_io_apic` clamps the entry count to the band's width and `redirection_index` admits
    // only an index below it, so this addition neither wraps nor truncates.
    debug_assert!(
        index < MAX_REDIRECTION_ENTRIES,
        "redirection index {index} is outside the vector band the entry-count cap reserves"
    );
    Some(GSI_VECTOR_BASE + index as u8)
}

/// Is `vector` one of the IO APIC's? The trap handler asks, so a device interrupt is counted as
/// routed. The band is by redirection index, as [`gsi_vector`] assigns it, and
/// `proofs::an_owned_gsi_routes_inside_the_io_apic_band` asserts the two agree.
pub(crate) fn is_device_vector(vector: u64) -> bool {
    let base = GSI_VECTOR_BASE as u64;
    vector >= base && vector < base + io_apic_entries() as u64
}

/// The redirection-table index for `gsi`, or `None` when this IO APIC does not own that GSI.
///
/// **Not the same number as the GSI**, on a machine with more than one IO APIC: each owns a slice
/// of the global space starting at its own `gsi_base`, and the index is the offset into that slice.
fn redirection_index(gsi: u32) -> Option<u32> {
    let base = IO_APIC_GSI_BASE.load(Ordering::Relaxed);
    let entries = IO_APIC_ENTRIES.load(Ordering::Relaxed);
    let index = gsi.checked_sub(base)?;
    (index < entries).then_some(index)
}

/// Route `gsi` to `vector` on the local APIC `dest_apic_id`, and unmask it.
///
/// Fixed delivery and physical destination: exactly the named local APIC, correct on any number of
/// CPUs; distributing interrupts is a policy nothing here has. The high word goes first and the low
/// word (with the mask bit) last, so the destination is in place when the line goes live.
///
/// # Panics
/// If this IO APIC does not own `gsi`: an unarmed line looks exactly like a silent device.
pub(crate) fn route_gsi(
    gsi: u32,
    vector: u8,
    active_low: bool,
    level_triggered: bool,
    dest_apic_id: u8,
) {
    let index = redirection_index(gsi).unwrap_or_else(|| {
        panic!(
            "gsi {gsi} is outside the IO APIC's range (base {}, {} entries)",
            IO_APIC_GSI_BASE.load(Ordering::Relaxed),
            IO_APIC_ENTRIES.load(Ordering::Relaxed),
        )
    });
    let at = io_reg::REDIRECTION_BASE + 2 * index;

    let mut low = vector as u32;
    if active_low {
        low |= REDIR_ACTIVE_LOW;
    }
    if level_triggered {
        low |= REDIR_LEVEL;
    }

    io_apic_write(at + 1, (dest_apic_id as u32) << 24);
    io_apic_write(at, low);
}

/// Mask `gsi` at the IO APIC, leaving the rest of its entry alone.
///
/// # Panics
/// As [`route_gsi`]: a GSI this part does not own is a caller bug, not a no-op.
pub(crate) fn mask_gsi(gsi: u32) {
    let index = redirection_index(gsi).unwrap_or_else(|| {
        panic!("gsi {gsi} is outside the IO APIC's range");
    });
    let at = io_reg::REDIRECTION_BASE + 2 * index;
    io_apic_write(at, io_apic_read(at) | REDIR_MASKED);
}

/// **Record how the MADT's interrupt source overrides resolve the sixteen legacy ISA IRQs**, so
/// that [`enable`] can take a legacy number the way the arch contract's callers do.
///
/// Called once, after ACPI has been walked and before any line is armed. Without it [`enable`]
/// falls back to the identity map, which is wrong for the timer on every PC ever built.
pub(crate) fn record_isa_routing(table: &[IsaIrqRouting; ISA_IRQ_COUNT]) {
    for (irq, routing) in table.iter().enumerate() {
        let mut packed = routing.gsi & 0xffff;
        if routing.active_low {
            packed |= PACKED_ACTIVE_LOW;
        }
        if routing.level_triggered {
            packed |= PACKED_LEVEL;
        }
        ISA_ROUTING[irq].store(packed, Ordering::Relaxed);
    }
}

/// How legacy IRQ `irq` reaches this machine's IO APIC, falling back to the ISA bus's own defaults
/// when [`record_isa_routing`] has not run or the number is not a legacy IRQ.
pub(crate) fn isa_routing(irq: u32) -> IsaIrqRouting {
    let fallback = IsaIrqRouting::isa_default(irq as u8);
    let Some(slot) = ISA_ROUTING.get(irq as usize) else {
        return fallback;
    };
    match slot.load(Ordering::Relaxed) {
        NO_ROUTING => fallback,
        packed => IsaIrqRouting {
            gsi: packed & 0xffff,
            active_low: packed & PACKED_ACTIVE_LOW != 0,
            level_triggered: packed & PACKED_LEVEL != 0,
        },
    }
}

// ---------------------------------------------------------------------------------------------
// The portable arch contract's names.
// ---------------------------------------------------------------------------------------------

/// **The boot CPU's local APIC, at the address the MADT gave** (milestone 890 (riscv64 and
/// `x86_64` join the shared boot path)): this architecture's half of the step riscv64 fills with its
/// PLIC contexts, in the same window of `bring_up_machine`.
///
/// It runs before `timer::init_frequency`, which calibrates the local APIC's own counter and panics
/// if the APIC is not up, and before `mmu::init`, through the boot map that reaches the low 4 GiB.
/// The fine map then maps the same page device-typed (`mmu::claims`, from the same MADT). The
/// pointer argument is the arch contract's; this one reads the walk `memory::init` kept.
pub(crate) fn init_contexts(boot_info_pointer: usize) {
    let _ = boot_info_pointer;
    let Some(apic) = super::discovery::local_apic_phys() else {
        crate::println!("  apic        : skipped, the MADT did not say where the local apic is");
        return;
    };
    // SAFETY: the address came from the machine's own ACPI MADT, and the boot map still covers it.
    unsafe { init_local_apic(apic) };
    crate::println!(
        "  apic        : local apic {apic:#x} up, id {}, version {:#x}, 8259s masked",
        local_apic_id(),
        local_apic_version(),
    );
}

/// Bring up the interrupt controller: the IO APIC the MADT names, its legacy routing, and one
/// device interrupt through it, before the shared `interrupts_init` arms the tick and unmasks.
///
/// The PIT's line proves an interrupt from outside the CPU arrives, and its IRQ 0 arrives as GSI 2,
/// which tests the override table (notes/x86-port/traps.md). It is here because every build reaches
/// it and the window it needs, the scheduler up and the tick not yet armed, exists only here.
/// `cargo xtask uefi-boot` reads the `device irq` line it prints.
pub fn init() {
    let (io_apic, isa_irqs) = super::discovery::with_acpi(|a| (a.io_apic, a.isa_irqs));
    let Some((io_id, io_addr, gsi_base)) = io_apic.filter(|_| is_local_apic_ready()) else {
        crate::println!("  io apic     : skipped, the MADT did not describe one");
        return;
    };
    // SAFETY: the address and global-interrupt base came from the machine's own ACPI MADT, and
    // `mmu::init` mapped that page device-typed (`mmu::claims`, from the same table).
    unsafe { init_io_apic(io_addr as u64, gsi_base) };
    record_isa_routing(&isa_irqs);
    let chip_id = io_apic_id();
    crate::println!(
        "  io apic     : id {chip_id} at {io_addr:#x} up, version {:#x}, {} redirection entries, gsi base {gsi_base}",
        io_apic_version(),
        io_apic_entries(),
    );
    if chip_id != io_id {
        // Worth a line rather than an average: the MADT and the chip disagreeing about which IO
        // APIC this is means one of the two is describing a different machine.
        crate::println!(
            "                the MADT calls it id {io_id}; the register above is the chip's own answer"
        );
    }
    prove_a_device_line();
}

/// Route the PIT through the IO APIC, take its interrupts for about 0.2 s with nothing else
/// unmasked, and print the count. See [`init`] for why this device.
fn prove_a_device_line() {
    use super::{exceptions, interrupts, timer};

    // Masked so the count is the PIT's alone; `timer::init` rewrites the LVT unmasked right after.
    mask_timer();
    let pit = isa_routing(timer::PIT_IRQ);
    let hz = timer::start_pit_ticking(timer::TICK_HZ);
    enable(timer::PIT_IRQ);
    // Read after `enable` rather than before, which is what makes the `expect` honest: `enable`
    // has just routed this GSI and panics with a named reason if the IO APIC does not own it. It
    // also makes the line's field the vector that was programmed, not one computed beside it.
    let vector =
        gsi_vector(pit.gsi).expect("enable routed this gsi a line ago, so the io apic owns it");

    exceptions::enable_external();
    interrupts::enable();
    let start = timer::now();
    while timer::now().wrapping_sub(start) < timer::frequency() / 5 {
        super::wait_for_interrupt();
    }
    interrupts::disable();
    mask_gsi(pit.gsi);

    let delivered = exceptions::DEVICE_IRQS.load(Ordering::Relaxed);
    DEVICE_LINE_INTERRUPTS.store(delivered as u64, Ordering::Relaxed);
    crate::println!(
        "  device irq  : pit irq {} -> gsi {} on vector {vector:#x}: {delivered} interrupts in ~0.2s at {hz} Hz",
        timer::PIT_IRQ,
        pit.gsi,
    );
}

/// What [`prove_a_device_line`] counted, or `u64::MAX` before it ran (or on a machine whose MADT
/// named no IO APIC, where it never does).
static DEVICE_LINE_INTERRUPTS: AtomicU64 = AtomicU64::new(u64::MAX);

/// **How many interrupts a device line delivered at bring-up**: the PIT's, routed through the IO
/// APIC by [`init`]. `None` when no line was proved, which on this port means the MADT named no
/// IO APIC. `self_test`'s `deviceirq` check reads it, because the window the proof needs (the
/// scheduler up, the tick not yet armed) exists only inside `init`.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)] // self_test reads it
pub(crate) fn device_line_interrupts() -> Option<u64> {
    match DEVICE_LINE_INTERRUPTS.load(Ordering::Relaxed) {
        u64::MAX => None,
        n => Some(n),
    }
}

/// Bring up this CPU's local interrupt interface: a secondary's own [`init_local_apic`], at the
/// physical address the boot core found in the MADT. Every core's local APIC answers at that one
/// address (`IA32_APIC_BASE` resets alike on every core), so re-masking the 8259s and rewriting
/// [`LOCAL_APIC`] with the same value are idempotent, not a race. A no-op with no local APIC, which
/// `arch::can_start_secondaries()` rules out before any secondary starts.
pub(crate) fn init_this_cpu() {
    if let Some(apic) = local_apic_phys() {
        // SAFETY: the same physical address the boot core already validated (it is where
        // `read_acpi` found the MADT's local APIC entry) and mapped device-typed in `mmu::init`'s
        // fine map, which every core shares (x86 has one kernel root; see `mmu::init_secondary`).
        unsafe { init_local_apic(apic) };
    }
}

/// Unmask interrupt `intid` at the controller.
///
/// A local APIC source ([`RESCHEDULE_VECTOR`], [`SELF_TEST_VECTOR`] and its `_B` twin) or an MSI
/// vector names itself by vector and has no controller input, so for it this is a no-op. Any other
/// intid is a legacy IRQ, 0..15: [`isa_routing`] turns it into the GSI the MADT says it arrives on,
/// with polarity and trigger mode, and the entry is routed to [`gsi_vector`]'s vector on this core.
/// IRQ 0 becomes GSI 2. The ranges cannot collide, since local sources start at 0x20. See
/// notes/x86-port/traps.md.
pub fn enable(intid: u32) {
    if is_local_apic_source(intid) || is_msi_vector(intid as u64) {
        return;
    }
    let routing = isa_routing(intid);
    // Refused here rather than in `route_gsi`, so the message names the caller's `intid`.
    let Some(vector) = gsi_vector(routing.gsi) else {
        panic!(
            "intid {intid} resolves to gsi {}, which is outside the IO APIC's range (base {}, {} entries)",
            routing.gsi,
            IO_APIC_GSI_BASE.load(Ordering::Relaxed),
            IO_APIC_ENTRIES.load(Ordering::Relaxed),
        )
    };
    if let Some(index) = redirection_index(routing.gsi) {
        GSI_OWNER[index as usize].store(intid, Ordering::Relaxed);
    }
    route_gsi(
        routing.gsi,
        vector,
        routing.active_low,
        routing.level_triggered,
        local_apic_id(),
    );
}

/// **The intid whose line raised `vector`**, or `None` when `vector` is not an IO APIC vector or
/// nothing armed its entry (milestone 505 (an x86_64 input driver that never lets the core idle)).
/// The inversion of [`enable`], read from [`GSI_OWNER`]; that table's doc has why it is not
/// arithmetic.
///
/// Name: provisional (milestone 505's lane); calef names public functions.
pub fn intid_of_vector(vector: u64) -> Option<u32> {
    if !is_device_vector(vector) {
        return None;
    }
    let index = (vector - GSI_VECTOR_BASE as u64) as usize;
    match GSI_OWNER.get(index)?.load(Ordering::Relaxed) {
        NO_OWNER => None,
        intid => Some(intid),
    }
}

/// Hold a routed line off until its driver ACKs, undone by [`enable`]; aarch64's `irq::disable`.
///
/// Only a level-triggered line is masked: an IO APIC drops an edge that arrives on a masked entry,
/// where a GIC keeps it pending. COM1's edge-triggered IRQ 4 stays high until the 16550 is drained,
/// so a masked COM1 would lose a byte arriving before the ACK and never interrupt again.
///
/// Name: recorded (aarch64's `arch::irq::disable`): the arch contract's name for the same step, which
/// is why it is not named for the edge/level split. Provisional on this architecture (milestone 505's
/// lane).
pub fn disable(intid: u32) {
    let routing = isa_routing(intid);
    if routing.level_triggered {
        mask_gsi(routing.gsi);
    }
}

/// **Does this intid name a local APIC source rather than a controller input?** See [`enable`].
///
/// The range is `TIMER_VECTOR..GSI_VECTOR_BASE`, spelled from those two constants rather than as
/// `0x20..0x30`, so that moving either one moves this with it. A hard-coded pair here would be the
/// third place the same two numbers are written down.
fn is_local_apic_source(intid: u32) -> bool {
    (TIMER_VECTOR as u32..GSI_VECTOR_BASE as u32).contains(&intid)
}

/// **ICR delivery mode `Fixed`**, bits 10:8 = 000: deliver `vector` to the destination, exactly as
/// if a device line had raised it. The other modes below (INIT, STARTUP) are SMP bring-up's.
const ICR_FIXED: u32 = 0b000 << 8;
/// **ICR delivery mode `INIT`**, bits 10:8 = 101: the first step of INIT-SIPI-SIPI (milestone 161's
/// SMP item). Resets the target CPU into a wait-for-SIPI state; the vector field is unused (left
/// zero) because INIT does not say where to start, only that the target should stop and wait.
const ICR_INIT: u32 = 0b101 << 8;
/// **ICR delivery mode `NMI`**, bits 10:8 = 100: deliver a non-maskable interrupt (vector 2) to the
/// destination. The vector field is unused, as it is for INIT, because the vector is architectural.
///
/// **This is the only message on this architecture that `cli` cannot suppress**, which is the whole
/// reason the TLB shootdown uses it: see `mmu::shoot_down_others`, whose target is routinely a core
/// spinning for `KERNEL_MMU` with interrupts masked.
const ICR_NMI: u32 = 0b100 << 8;
/// **ICR delivery mode `Startup`** (a STARTUP inter-processor interrupt, "SIPI"), bits 10:8 = 110:
/// the vector field names a *physical page below 1 MiB* the target begins executing at, in 16-bit
/// real mode, with `cs` = the vector and `ip` = 0 (so the physical address is `vector << 12`).
const ICR_STARTUP: u32 = 0b110 << 8;
/// ICR bit 14, the level bit. Must be 1 for every delivery mode except INIT de-assert, which no
/// modern part uses; a zero here is silently ignored on some steppings and not on others.
const ICR_ASSERT: u32 = 1 << 14;
/// ICR destination shorthand bits 19:18 = 01: **self**, no destination field consulted.
const ICR_SELF: u32 = 0b01 << 18;
/// ICR bit 12, delivery status: set by the APIC while a previous IPI is still being sent. Read-only,
/// and polled before writing a new one.
const ICR_PENDING: u32 = 1 << 12;

/// Wait for any previous IPI to be accepted. The ICR is one register per local APIC, so writing it
/// while a send is outstanding loses one of the two.
fn wait_for_ipi_delivery() {
    while read(reg::ICR_LOW) & ICR_PENDING != 0 {
        core::hint::spin_loop();
    }
}

/// **Send `vector` to the local APIC whose id is `dest_apic_id`.**
///
/// The write to [`reg::ICR_LOW`] is what sends it, so the destination goes in the high word first;
/// a version of this that wrote them the other way round would send to whoever the previous IPI
/// named, which is a bug that only appears once there is a second CPU to get it wrong about.
///
/// Name: provisional (milestone 161, roadmap item 4).
pub fn send_ipi(dest_apic_id: u8, vector: u8) {
    wait_for_ipi_delivery();
    write(reg::ICR_HIGH, (dest_apic_id as u32) << 24);
    write(reg::ICR_LOW, ICR_FIXED | ICR_ASSERT | vector as u32);
}

/// Raise `vector` on the CPU executing this, through the local APIC's self shorthand.
///
/// `sched`'s interrupt-delivery tests use it. The interrupt takes the hardware's path (ICR, IRR,
/// ISR, EOI), as aarch64's software-generated interrupt does; RISC-V can raise nothing and asserts
/// its console UART's line instead. A device line here could fail for reasons unrelated to the
/// kernel. See notes/x86-port/traps.md.
///
/// Name: provisional (milestone 161, roadmap item 4).
pub fn raise_self_interrupt(vector: u8) {
    wait_for_ipi_delivery();
    // No destination word: the `self` shorthand tells the APIC to ignore it, and writing one would
    // be a claim about which CPU this is that the shorthand exists to avoid making.
    write(
        reg::ICR_LOW,
        ICR_SELF | ICR_FIXED | ICR_ASSERT | vector as u32,
    );
}

/// Send a reschedule IPI to `target_cpu`, whose handler drains its inbox and serves any work-steal
/// request (`sched::drain_inbox`, `sched::serve_steal_request`); aarch64's reschedule SGI.
///
/// The destination is `smp::hwid(target_cpu)`, the local APIC id the MADT gave that core's roster
/// slot (`smp::seat_cpus_from_acpi`), not the logical id: the two are independent.
///
/// # Panics
/// If `target_cpu` names no seated core. Every caller in `sched` first checks the target is an
/// online core (`smp::online_cpus`), and an online core is by construction a seated one.
pub(crate) fn send_reschedule(target_cpu: usize) {
    debug_assert!(
        is_local_apic_ready(),
        "a reschedule IPI before the local APIC is up has nothing to send it with"
    );
    let apic_id = crate::smp::hwid(target_cpu).unwrap_or_else(|| {
        panic!("send_reschedule: cpu {target_cpu} is not in the roster (no local apic id)")
    });
    send_ipi(apic_id as u8, RESCHEDULE_VECTOR);
}

/// **Send a non-maskable interrupt to the local APIC whose id is `dest_apic_id`.**
///
/// The target takes vector 2 at its next instruction boundary **whether or not its interrupts are
/// enabled**, which is the property `mmu::shoot_down_others` is built on and the reason this exists
/// beside [`send_ipi`] rather than being one more vector through it.
///
/// No acknowledgement is written by the receiver's local APIC: an NMI sets no in-service bit, so
/// there is no EOI to owe, unlike every vector [`send_ipi`] delivers.
///
/// Name: provisional (milestone 161's SMP item): calef names public items.
pub fn send_nmi(dest_apic_id: u8) {
    wait_for_ipi_delivery();
    write(reg::ICR_HIGH, (dest_apic_id as u32) << 24);
    write(reg::ICR_LOW, ICR_NMI | ICR_ASSERT);
}

/// **Send an INIT inter-processor interrupt** to the local APIC whose id is `dest_apic_id`: the
/// first step of INIT-SIPI-SIPI. Resets the target into a wait-for-SIPI state; it does not say
/// where to start, which is what the two STARTUP IPIs that follow are for.
pub(crate) fn send_init(dest_apic_id: u8) {
    wait_for_ipi_delivery();
    write(reg::ICR_HIGH, (dest_apic_id as u32) << 24);
    write(reg::ICR_LOW, ICR_INIT | ICR_ASSERT);
}

/// **Send a STARTUP inter-processor interrupt ("SIPI")** to the local APIC whose id is
/// `dest_apic_id`, naming the page it should begin executing at: physical address
/// `(vector as u64) << 12`. The target starts there in 16-bit real mode, with `cs` = `vector << 8`
/// and `ip` = 0 (`cs * 16 + ip` is exactly `vector << 12`).
pub(crate) fn send_startup(dest_apic_id: u8, vector: u8) {
    wait_for_ipi_delivery();
    write(reg::ICR_HIGH, (dest_apic_id as u32) << 24);
    write(reg::ICR_LOW, ICR_STARTUP | ICR_ASSERT | vector as u32);
}

/// **This machine's interrupt controller, for the machine description** (milestone 268).
///
/// One of the eight questions the description answers on every architecture, in this
/// architecture's own vocabulary, and on x86 the answer is two chips rather than one. The local
/// APIC is per-core and is what delivers; the IO APIC is per-machine and is what routes a device's
/// line to a vector. A machine with the first and not the second takes its own timer interrupt and
/// no device interrupt at all, so the two are printed apart.
// The machine description is the only caller, and it is
// `#[cfg(not(any(test, feature = "bench")))]`: a test boot exits through semihosting and a bench
// boot diverges into `bench::run`, so neither reads a bring-up transcript. Same treatment
// `memory::print_summary` already carries, and for the same reason.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub(crate) fn print_summary() {
    match (local_apic_phys(), io_apic_phys()) {
        (Some(lapic), Some(ioapic)) => crate::println!(
            "  interrupts      : local apic {lapic:#018x} (id {}), io apic {ioapic:#018x} ({} entries)",
            local_apic_id(),
            io_apic_entries(),
        ),
        (Some(lapic), None) => crate::println!(
            "  interrupts      : local apic {lapic:#018x} (id {}), no io apic (the MADT described none)",
            local_apic_id(),
        ),
        (None, _) => crate::println!(
            "  interrupts      : none (the MADT did not say where the local apic is)",
        ),
    }
}

/// Proofs about the vector space (milestone 304 (`cargo kani -p kernel` only ever compiled one
/// architecture)), over numbers firmware chose: the MADT's GSI base, the version register's entry
/// count, an override's GSI. Each picks an interrupt's vector, an index into the IDT, so a wrong
/// one runs the page-fault handler. See notes/kernel-proofs.md and
/// design/roadmap/0304-prover-one-architecture.md.
#[cfg(kani)]
mod proofs {
    use super::*;

    /// The three vector bands never overlap, whatever the version register says: the claim
    /// [`MSI_VECTOR_BASE`]'s doc makes, over every entry count and MSI allocation state, asked of
    /// the predicates the trap handler calls rather than of the constants.
    ///
    /// Falsification: attested 2026-09-16. `MAX_REDIRECTION_ENTRIES` raised by one, on cordoba
    /// (x86_64 Linux): the device band then reaches `MSI_VECTOR_BASE` and this goes red, along
    /// with its sibling. One character in the constant whose own doc makes the disjointness claim.
    ///
    /// **`attested` rather than `replayable`, and the reason is this milestone's subject.** A patch
    /// under `kernel/falsifications/` is replayed by `script/falsifications --sweep kernel`, which
    /// runs one named harness on whatever host it is on; an x86_64 harness does not exist on an
    /// aarch64 host, so a `replayable` record here would fail the sweep on the dev Mac and on every
    /// CI runner but the new one. See that script's BUGS, where the same fact is recorded from the
    /// sweep's side, and note it has always been true of the `arch.aarch64.iommu` patches facing
    /// the other way.
    #[kani::proof]
    fn no_vector_belongs_to_two_bands() {
        // What `init_io_apic` stores: the version register's entry field is eight bits, plus one,
        // then clamped. Anything the hardware can say, put through the real expression.
        let reported: u32 = kani::any();
        kani::assume(reported <= 0xff);
        let entries = (reported + 1).min(MAX_REDIRECTION_ENTRIES);
        IO_APIC_ENTRIES.store(entries, Ordering::Relaxed);

        // Every state the bump counter can reach, including past the end of the band.
        let allocated: u32 = kani::any();
        MSI_NEXT.store(allocated, Ordering::Relaxed);

        let vector: u64 = kani::any();
        kani::assume(vector <= 0xff);

        let device = is_device_vector(vector);
        let msi = is_msi_vector(vector);
        let local = is_local_apic_source(vector as u32);

        assert!(
            !(device && msi),
            "a vector cannot be both an IO APIC line and an MSI"
        );
        assert!(
            !(device && local),
            "a vector cannot be both an IO APIC line and a local APIC source"
        );
        assert!(
            !(msi && local),
            "a vector cannot be both an MSI and a local APIC source"
        );
        assert!(
            !(msi && vector == SPURIOUS_VECTOR as u64),
            "the MSI band must stop short of the spurious vector"
        );
        assert!(
            !(device && vector == SPURIOUS_VECTOR as u64),
            "the IO APIC band must stop short of the spurious vector"
        );
    }

    /// A GSI this IO APIC owns routes to a vector inside its band, whatever base the MADT gave it,
    /// and [`is_device_vector`] counts that vector: the guard ([`redirection_index`]) and the map
    /// agree over every `base`, entry count and `gsi`.
    ///
    /// Falsification: attested 2026-09-16. On cordoba (x86_64 Linux), and the patch is written
    /// down, which is unusual for an `attested` record and is the point:
    /// `kernel/falsifications/arch.x86_64.irq.tests.a_gsi_on_a_second_io_apic_routes_inside_the_band.patch`
    /// restores `GSI_VECTOR_BASE.wrapping_add(gsi as u8)` and turns **both** this harness and the
    /// kernel test it names red. Here it fires **both** assertions in the `if let` below, the one
    /// whose message begins "a GSI the IO APIC owns" and the one beginning "a vector this kernel
    /// routed". Named rather than numbered because the lines move: they were `irq.rs:1164` and
    /// `irq.rs:1168` when this was measured. The second red is that assertion earning its place:
    /// the two functions really did disagree, rather than merely looking as though they might.
    /// It is filed against the test rather than against this, because a
    /// `#[kani::proof]` compiles for the HOST and `kernel/src/arch/mod.rs` selects its subtree by
    /// `#[cfg(target_arch)]`: a `replayable` record here would turn `script/falsifications --sweep
    /// kernel` red on every machine in this project but cordoba, while a kernel test names its
    /// architecture and boots QEMU. See `script/falsifications`' BUGS, where the same constraint is
    /// recorded from the sweep's side, and note this is the third x86_64 harness it applies to.
    #[kani::proof]
    fn an_owned_gsi_routes_inside_the_io_apic_band() {
        // Unconstrained: the MADT states this and nothing in this kernel bounds or refuses it.
        let base: u32 = kani::any();
        IO_APIC_GSI_BASE.store(base, Ordering::Relaxed);

        let reported: u32 = kani::any();
        kani::assume(reported <= 0xff);
        let entries = (reported + 1).min(MAX_REDIRECTION_ENTRIES);
        IO_APIC_ENTRIES.store(entries, Ordering::Relaxed);

        // Unconstrained too. `record_isa_routing` packs a GSI into sixteen bits, so the reachable
        // set is smaller than this; stating it over the whole type costs nothing and means the
        // property does not quietly depend on that packing.
        let gsi: u32 = kani::any();

        // The guard and the map agree about which GSIs exist. The old signature could not say
        // this, because it answered for every GSI whether or not the part owned one.
        assert_eq!(
            gsi_vector(gsi).is_some(),
            redirection_index(gsi).is_some(),
            "a GSI has a vector exactly when this IO APIC has an entry for it"
        );

        if let Some(vector) = gsi_vector(gsi) {
            assert!(
                (GSI_VECTOR_BASE as u32..MSI_VECTOR_BASE as u32).contains(&(vector as u32)),
                "a GSI the IO APIC owns was mapped onto a vector outside the IO APIC band"
            );
            assert!(
                is_device_vector(vector as u64),
                "a vector this kernel routed a device line to is not counted as a device vector"
            );
        }
    }
}

/// **The second IO APIC nobody has, executed** (milestone 308).
///
/// The proof above states the map's property over every input a model checker can reach, and the
/// module's BUGS is honest that the machine it describes does not exist here. This is the middle
/// ground: one concrete plausible part, the one from the proposal, put through the real functions
/// on a real boot. It is not evidence that a second IO APIC works, and nothing in this tree is; it
/// is evidence that the arithmetic a second IO APIC would need is compiled, linked and running,
/// which an `#[cfg(kani)]` harness on an aarch64 dev machine is not.
#[cfg(test)]
mod tests {
    use super::*;

    /// A GSI owned by an IO APIC based at global interrupt 200 routes inside the device band, and
    /// the trap handler counts its vector as a device vector. GSI 210 is entry 10 of a 24-entry
    /// part; a GSI-based map sends it to `0x30.wrapping_add(210)`, which is 2, the NMI. The two
    /// statics are this machine's real IO APIC state, so they are saved and put back.
    ///
    /// Falsification: replayable `kernel/falsifications/arch.x86_64.irq.tests.a_gsi_on_a_second_io_apic_routes_inside_the_band.patch`
    #[test_case]
    fn a_gsi_on_a_second_io_apic_routes_inside_the_band() {
        let saved_base = IO_APIC_GSI_BASE.load(Ordering::Relaxed);
        let saved_entries = IO_APIC_ENTRIES.load(Ordering::Relaxed);

        IO_APIC_GSI_BASE.store(200, Ordering::Relaxed);
        IO_APIC_ENTRIES.store(24, Ordering::Relaxed);

        let vector = gsi_vector(210).expect("gsi 210 is entry 10 of a part based at 200");
        assert_eq!(
            vector,
            GSI_VECTOR_BASE + 10,
            "the vector is the base plus the redirection index, not plus the GSI"
        );
        assert_ne!(vector, 2, "the pre-308 map sent this GSI to the NMI");
        assert!(
            is_device_vector(vector as u64),
            "the trap handler must count a line this kernel routed as a device vector"
        );

        // Both ends of the part's range, so the guard is exercised rather than assumed.
        assert_eq!(gsi_vector(199), None, "199 is below this part's base");
        assert_eq!(gsi_vector(200), Some(GSI_VECTOR_BASE), "the first entry");
        assert_eq!(
            gsi_vector(223),
            Some(GSI_VECTOR_BASE + 23),
            "the last entry"
        );
        assert_eq!(gsi_vector(224), None, "one past the last entry");

        IO_APIC_GSI_BASE.store(saved_base, Ordering::Relaxed);
        IO_APIC_ENTRIES.store(saved_entries, Ordering::Relaxed);
    }
}
