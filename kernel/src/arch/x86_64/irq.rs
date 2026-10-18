//! **The interrupt controller, `x86_64`: the local APIC and the IO APIC**, where aarch64 has a GIC
//! and RISC-V a PLIC. On x86 the controller is a pair, and it is also the SMP bring-up mechanism.
//! The **local APIC** is per-CPU: it receives and acknowledges interrupts (the EOI), owns a timer,
//! and sends inter-processor interrupts, including the INIT and STARTUP that start a secondary
//! ([`send_init`], [`send_startup`]; see `arch::x86_64::ap_boot`). The **IO APIC** routes device
//! lines to vectors; the ACPI MADT says where it is (`machine_discovery::acpi`). Built here too:
//! masking the 8259s, the calibration counter `timer.rs` needs, the redirection table, and MSI
//! vector allocation ([`alloc_msi_vector`]), which is how a PCI function reaches a userspace driver.
//!
//! **The 8259s are still wired** (`PCAT_COMPAT`), raise interrupts on vectors 8..15 (the CPU's own
//! exception vectors), and see every line the IO APIC routes. [`init_local_apic`] masks both before
//! enabling the local APIC, in that order; masked, not remapped, because a live 8259 would deliver
//! a second copy of every routed interrupt on an exception vector.
//!
//! **A legacy IRQ number is not an IO APIC input**: the MADT's overrides rewire them (the PIT's IRQ
//! 0 arrives as GSI 2 on essentially every PC). `machine_discovery::acpi::isa_irq_table` resolves
//! them and [`record_isa_routing`] hands the answer here; a legacy number used as a pin would arm a
//! line nothing drives, silently.
//!
//! # BUGS
//!
//! - One IO APIC, the first the MADT lists; a GSI owned by a second is refused by [`route_gsi`],
//!   so a two-socket server is unimplemented. The nonzero-base routing is proved
//!   (`proofs::an_owned_gsi_routes_inside_the_io_apic_band`) and unit-tested, never booted.
//! - Every entry targets the local APIC of the core that called [`enable`], in physical mode, and
//!   every MSI the core that ran [`alloc_msi_vector`]; nothing is distributed.
//! - Before `mmu::init`, the redirection table is written through the boot map's cacheable alias;
//!   the MTRRs keep the MMIO hole uncacheable regardless.
//! - Nothing masks a routed line on the way out: revoking a driver's `Irq` does not call
//!   [`mask_gsi`], so a dead input driver's COM1 line stays armed into a rendezvous nobody waits on.
//! - An MSI vector is never handed back ([`alloc_msi_vector`] bumps); a device brought up twice
//!   spends two of the band's sixty-three.
//! - Interrupt remapping is assumed off: a VT-d unit with it on rejects the compatibility-format
//!   messages built here. Check this first if MSI stops arriving on a machine whose firmware enables
//!   it.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use machine_discovery::acpi::{ISA_IRQ_COUNT, IsaIrqRouting};

use super::mmu::phys_to_virt;
use super::port::{in8, out8};

/// The local APIC's registers, as offsets from its base. 32-bit, and **all of them must be accessed
/// as aligned 32-bit words**: an 8- or 16-bit access to an APIC register is undefined.
mod reg {
    /// This CPU's local APIC id, in bits 31:24.
    pub const ID: u64 = 0x020;
    /// Version, and in bits 23:16 the number of LVT entries minus one.
    pub const VERSION: u64 = 0x030;
    /// Task Priority. Zero means "accept every vector"; anything else silently drops interrupts
    /// below that priority class, which is a very quiet way to lose them.
    pub const TPR: u64 = 0x080;
    /// End Of Interrupt. Written (with zero) to acknowledge; until then the local APIC will not
    /// deliver another interrupt of the same or lower priority.
    pub const EOI: u64 = 0x0b0;
    /// Spurious Interrupt Vector. Bit 8 is the **software enable**, and the low eight bits are the
    /// vector a spurious interrupt arrives on.
    pub const SPURIOUS: u64 = 0x0f0;
    /// Interrupt Command Register, low word: the vector, the delivery mode, and the destination
    /// shorthand. **Writing this word is what sends the IPI**, so the high word must already be in
    /// place. See [`super::send_ipi`].
    pub const ICR_LOW: u64 = 0x300;
    /// Interrupt Command Register, high word: the destination local APIC id, in bits 31:24.
    pub const ICR_HIGH: u64 = 0x310;
    /// Local Vector Table entry for the timer: the vector, the mask bit, and the mode.
    pub const LVT_TIMER: u64 = 0x320;
    /// What the timer counts down from.
    pub const TIMER_INITIAL: u64 = 0x380;
    /// What it is at now.
    pub const TIMER_CURRENT: u64 = 0x390;
    /// How much the timer divides the bus clock by.
    pub const TIMER_DIVIDE: u64 = 0x3e0;
    /// The Interrupt Request Register, 256 bits as eight 32-bit words 0x10 apart: bit `v` is set
    /// while vector `v` has been accepted by this local APIC and not yet delivered to the core.
    pub const IRR: u64 = 0x200;
}

/// Bit 8 of the spurious-interrupt register: the local APIC's software enable.
const SPURIOUS_ENABLE: u32 = 1 << 8;

/// **The vector a spurious interrupt arrives on.** 0xff by convention, and the low four bits used to
/// be required to be one on older parts, which is why every kernel picks a vector ending in 0xf.
pub const SPURIOUS_VECTOR: u8 = 0xff;

/// **The vector the local APIC timer raises.** 0x20, the first vector after the 32 the architecture
/// reserves for exceptions. Not a hardware fact: it is our choice, written into the LVT.
pub const TIMER_VECTOR: u8 = 0x20;

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
pub unsafe fn init_local_apic(base: u64) {
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
pub fn end_of_interrupt() {
    write(reg::EOI, 0);
}

/// This CPU's local APIC id, which is its hardware name and need not be its logical cpu id.
pub fn local_apic_id() -> u8 {
    (read(reg::ID) >> 24) as u8
}

/// The local APIC's version register's low byte. Printed at boot; an integrated APIC reports 0x10 or
/// higher, an external 82489DX reports below 0x10 and would mean a machine older than this kernel
/// supports.
pub fn local_apic_version() -> u8 {
    read(reg::VERSION) as u8
}

/// Is the local APIC up? False until [`init_local_apic`] has run, which is what the timer checks
/// before trying to arm anything.
pub fn is_local_apic_ready() -> bool {
    LOCAL_APIC.load(Ordering::Relaxed) != 0
}

/// Start the timer counting down from `count`, **masked**, so it can be measured without delivering
/// anything. This is what calibration uses: the timer is a free-running counter until something
/// unmasks it.
pub fn start_timer_for_calibration(count: u32) {
    write(reg::TIMER_DIVIDE, TIMER_DIVIDE_16);
    write(reg::LVT_TIMER, LVT_MASKED | TIMER_VECTOR as u32);
    write(reg::TIMER_INITIAL, count);
}

/// What the timer's countdown is at now.
pub fn timer_current_count() -> u32 {
    read(reg::TIMER_CURRENT)
}

/// Arm the timer to fire every `count` of its own ticks, on [`TIMER_VECTOR`], forever.
pub fn arm_periodic_timer(count: u32) {
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
pub fn mask_timer() {
    write(reg::LVT_TIMER, LVT_MASKED | TIMER_VECTOR as u32);
}

// ---------------------------------------------------------------------------------------------
// The IO APIC: where a *device* line becomes a vector.
// ---------------------------------------------------------------------------------------------

/// **The IO APIC's registers are not a flat array, and that is the first thing to know about it.**
///
/// The whole device is two 32-bit words in the memory map: an index register and a data window.
/// To touch register `n` you write `n` to [`IOREGSEL`] and then read or write [`IOWIN`], which
/// makes every access a *pair* and makes the device stateful. Two CPUs doing this concurrently
/// would interleave and each would read the other's register; nothing here is concurrent yet
/// (single-threaded boot, one CPU), and this note is where the next person finds out that a lock
/// belongs here the moment SMP lands.
mod io_reg {
    /// The index register, at offset 0. Write the number of the register you want.
    pub const IOREGSEL: u64 = 0x00;
    /// The data window, at offset 0x10. Reads and writes land on whatever [`IOREGSEL`] last named.
    pub const IOWIN: u64 = 0x10;

    /// Register 0: this IO APIC's id, in bits 27:24.
    pub const ID: u32 = 0x00;
    /// Register 1: the version in bits 7:0, and in bits 23:16 **the number of redirection entries
    /// minus one**. That "minus one" is the field's definition, not an off-by-one: a 24-entry part
    /// reports 23.
    pub const VERSION: u32 = 0x01;
    /// Register 0x10: the first word of redirection entry 0. Each entry is **two** consecutive
    /// registers, low word first, so entry `n` is at `0x10 + 2 * n`.
    pub const REDIRECTION_BASE: u32 = 0x10;
}

/// Redirection entry bit 16: masked. Every entry powers on masked, which is why arming one is a
/// deliberate act and why [`init_io_apic`] masking them all again is belt and braces rather than
/// necessity.
const REDIR_MASKED: u32 = 1 << 16;
/// Redirection entry bit 15: level triggered rather than edge triggered.
const REDIR_LEVEL: u32 = 1 << 15;
/// Redirection entry bit 13: the input pin is asserted low rather than high.
const REDIR_ACTIVE_LOW: u32 = 1 << 13;

/// **The first vector the IO APIC's lines are routed to.** 0x30, which leaves 0x20..0x2f for the
/// local APIC's own sources (the timer at [`TIMER_VECTOR`], and later the thermal, performance,
/// error and inter-processor vectors, which are LVT entries rather than redirection entries).
///
/// A GSI's vector is this plus its **redirection index**, the offset of its entry within the IO
/// APIC that owns it (milestone 308; it was this plus the GSI until then, which wrapped onto an
/// exception vector on an IO APIC whose global interrupt base is not zero). The map is flat *in the
/// index*, so a stray vector in a fault report names its redirection entry by subtraction, and on
/// every machine this kernel has booted that entry's index is also its GSI because the MADT gives
/// the single IO APIC base zero. **Recovering the GSI on a machine where they differ needs the
/// base**, which [`redirection_index`] reads and nothing inverts; see `exceptions.rs`'s BUGS, where
/// the inversion is recorded as missing and unneeded.
///
/// The flat map costs the ability to prioritise (x86 priority is the vector's top four bits, so a
/// flat map gives 0x30..0x47 two priority classes and no say in which line is in which). Nothing
/// here has a priority policy to express yet.
///
/// Name: provisional (milestone 161), along with [`gsi_vector`] and the IO APIC entry points
/// below.
pub const GSI_VECTOR_BASE: u8 = 0x30;

/// **The first vector a PCI function's MSI-X message delivers on** (milestone 215). 0xc0, above
/// every redirection entry any real part has and below [`SPURIOUS_VECTOR`], so the three vector
/// bands on this architecture are disjoint by construction rather than by anybody remembering:
/// 0x20..0x30 the local APIC's own sources, [`GSI_VECTOR_BASE`]..0xc0 the IO APIC's lines, and
/// 0xc0..0xff MSI.
///
/// **Disjointness is what lets one number mean one thing.** `sched::bind_irq` and
/// `arch::irq::enable` take an `intid`, and on this architecture an intid is a vector for a local
/// APIC source and a legacy IRQ number for an IO APIC line. An MSI vector is a local APIC source
/// in the only sense that matters here (the device writes it straight to the local APIC; there is
/// no controller input and nothing to unmask), so an MSI intid **is** its vector, and the whole
/// vector-to-intid inversion the trap handler used to owe for a device line never arises.
///
/// Name: provisional (milestone 215 (a PCI function's interrupt reaches nothing)): calef
/// names public items.
pub const MSI_VECTOR_BASE: u8 = 0xc0;

/// The most redirection entries this kernel will use. Real parts have 24 (the 82093AA, QEMU's q35,
/// the ICH-era chipsets); the field could report up to 256, and this cap is the number that still
/// fits in the vector space between [`GSI_VECTOR_BASE`] and [`MSI_VECTOR_BASE`]. It bounds
/// [`is_device_vector`] and the mask loop in [`init_io_apic`] against a version register saying
/// something absurd.
///
/// **It bounds the entry count, and until milestone 308 this doc claimed it also stopped
/// [`gsi_vector`] wrapping onto an exception vector or onto an MSI one. It never did.** The cap is
/// a statement about how many redirection entries exist; the old `gsi_vector` added the *GSI*, a
/// number the MADT supplies and nothing here bounds, so a second IO APIC based at global interrupt
/// 200 wrapped GSI 210 onto vector 2. A doc asserting a guarantee the code did not provide is what
/// hid that defect for as long as it existed, which is why the correction is spelled out here
/// rather than quietly deleted. Now that [`gsi_vector`] adds the index instead, this cap **is** the
/// bound that keeps the sum inside the band, which is the claim
/// `proofs::an_owned_gsi_routes_inside_the_io_apic_band` checks.
const MAX_REDIRECTION_ENTRIES: u32 = (MSI_VECTOR_BASE - GSI_VECTOR_BASE) as u32;

/// The next MSI vector to hand out, as an offset from [`MSI_VECTOR_BASE`]. A bump counter, never
/// returned: nothing in this tree releases a device, and a freed-vector list with no free path is
/// machinery with no caller (`design/decisions/` §46's posture applied to code we would write
/// ourselves).
static MSI_NEXT: AtomicU32 = AtomicU32::new(0);

/// How many MSI vectors this band holds: 0xc0..0xff, stopping short of [`SPURIOUS_VECTOR`].
const MSI_VECTORS: u32 = SPURIOUS_VECTOR as u32 - MSI_VECTOR_BASE as u32;

/// **Reserve a vector a PCI function may deliver an MSI-X message on, and say where to send it.**
///
/// The arch contract's answer to "this machine has a PCI function whose interrupt must reach a
/// driver". `None` means this machine routes PCI interrupts some other way, which is what the
/// aarch64 and riscv64 implementations say: both boards' functions arrive as INTx through a
/// swizzle their device tree states, so `kernel/src/pci.rs` falls back to that.
///
/// The returned `u32` is the **intid** a driver binds and `arch::irq::enable`/`ACK` take, and on
/// this architecture it is the vector itself; see [`MSI_VECTOR_BASE`] for why that is a design
/// rather than a coincidence.
///
/// # The message, and the two fields that are not obvious
///
/// The address is `0xfee0_0000 | (destination local APIC id << 12)`. It is not memory: the local
/// APIC claims `0xfee0_0000..0xfef0_0000` on the bus, so a device's ordinary posted write into
/// that range *is* an interrupt, which is the whole trick and the reason MSI needs no routing
/// table. Redirection-hint and destination-mode bits (3 and 2) are left zero: physical
/// destination, no redirection, the same policy [`route_gsi`] takes and for the same reason.
///
/// The data is the vector, with delivery mode `Fixed` (bits 10:8 zero) and edge trigger (bit 15
/// zero). **MSI is edge-triggered by construction**: the message is a write that happens once, so
/// there is no asserted line to hold off, which is why `enable` has nothing to do for one of these
/// and why an `Irq::ACK` from a driver is correctly a no-op.
///
/// # A caveat the emulator cannot show
///
/// **With VT-d interrupt remapping enabled, this address is not delivered as written.** An
/// interrupt-remapping unit reinterprets `0xfee0_0000..0xfef0_0000` writes as an index into a
/// remapping table, and a message built the way this one is would be rejected as a malformed
/// remappable request. `helpers/qemu-runner-x86_64.sh` runs `-device intel-iommu` **without**
/// `intremap=on`, so DMA is translated and interrupt messages pass through; the same is true of
/// every machine whose firmware leaves remapping off, which is the default. Turning it on is its
/// own piece of work and is recorded as one; see this module's BUGS.
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
pub fn is_msi_vector(vector: u64) -> bool {
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
pub unsafe fn init_io_apic(base: u64, gsi_base: u32) {
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
pub fn io_apic_phys() -> Option<u64> {
    match IO_APIC_PHYS.load(Ordering::Relaxed) {
        0 => None,
        base => Some(base),
    }
}

/// This IO APIC's id, from its own register rather than from the MADT. Printed at boot because a
/// disagreement between the two is a firmware bug worth seeing rather than averaging over.
pub fn io_apic_id() -> u8 {
    ((io_apic_read(io_reg::ID) >> 24) & 0x0f) as u8
}

/// Its version register's low byte. 0x11 on the discrete 82093AA and on QEMU's q35, 0x20 on the
/// ICH-era parts.
pub fn io_apic_version() -> u8 {
    io_apic_read(io_reg::VERSION) as u8
}

/// How many redirection entries it has. Zero until [`init_io_apic`] has run.
pub fn io_apic_entries() -> u32 {
    IO_APIC_ENTRIES.load(Ordering::Relaxed)
}

/// **The vector a global system interrupt is routed to**, or `None` when this IO APIC does not own
/// that GSI and so has no entry to route it through.
///
/// The vector is [`GSI_VECTOR_BASE`] plus the GSI's **redirection index**, not plus the GSI
/// (milestone 308). On every machine this kernel has booted the two are the same number, because
/// the MADT gives the single IO APIC global interrupt base zero; they part company on a machine
/// with a second IO APIC, and the version that added the GSI wrapped a GSI of 210 onto vector 2,
/// the NMI. See this module's BUGS for the case, and
/// `proofs::an_owned_gsi_routes_inside_the_io_apic_band` for the statement that it can no longer
/// happen.
///
/// **This is the same partiality [`redirection_index`] has**, and it is the point rather than a
/// side effect: a GSI with no entry has no vector, and the old total signature had to invent one.
///
/// Name: provisional, and this change makes it worse rather than better (milestone 161 marked it
/// provisional; milestone 308 is the lane that noticed): the function now maps an *index* into the
/// vector space and the GSI is what it takes, not what it adds. `gsi_vector` still describes the
/// question a caller asks, so it is not wrong, but a name naming the index would be more honest.
/// calef names public items; this lane proposes rather than renames.
pub fn gsi_vector(gsi: u32) -> Option<u8> {
    let index = redirection_index(gsi)?;
    // `init_io_apic` clamps the entry count to `MAX_REDIRECTION_ENTRIES`, which is exactly the
    // width of the band, and `redirection_index` admits only an index below that count. So this
    // addition lands in `GSI_VECTOR_BASE..MSI_VECTOR_BASE` and neither wraps nor truncates. The
    // assert is the invariant said out loud at the one place that depends on it; the proof named
    // above is what checks it over every entry count a version register can report.
    debug_assert!(
        index < MAX_REDIRECTION_ENTRIES,
        "redirection index {index} is outside the vector band the entry-count cap reserves"
    );
    Some(GSI_VECTOR_BASE + index as u8)
}

/// Is `vector` one of the IO APIC's? The trap handler asks, so that a device interrupt is counted
/// as routed rather than as an unowned vector nothing claimed.
///
/// **This and [`gsi_vector`] agree on what the band is**, which they did not before milestone 308:
/// this predicate has always defined the band by *index* (`GSI_VECTOR_BASE` up to the entry count),
/// while `gsi_vector` assigned by GSI, so on an IO APIC with a nonzero base the same interrupt was
/// routed as a device vector and then not counted as one here.
/// `proofs::an_owned_gsi_routes_inside_the_io_apic_band` now asserts the agreement directly.
pub fn is_device_vector(vector: u64) -> bool {
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

/// **Route `gsi` to `vector` on the local APIC `dest_apic_id`, and unmask it.**
///
/// Fixed delivery mode and physical destination mode: the vector is delivered to exactly the local
/// APIC whose id is named, rather than to a logical group or to whichever CPU is running at the
/// lowest priority. That is the simplest thing that is correct on one CPU and stays correct on
/// several; distributing interrupts is a policy, and there is nothing here to have one yet.
///
/// The high word is written **first**, and the low word (which carries the mask bit) last, so the
/// destination is already in place at the instant the line goes live. The reverse order leaves a
/// window in which an interrupt is delivered to whatever CPU the previous value named.
///
/// # Panics
/// If this IO APIC does not own `gsi`. Silently doing nothing would be an unarmed line that looks
/// exactly like a device that never interrupts.
pub fn route_gsi(gsi: u32, vector: u8, active_low: bool, level_triggered: bool, dest_apic_id: u8) {
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
pub fn mask_gsi(gsi: u32) {
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
pub fn record_isa_routing(table: &[IsaIrqRouting; ISA_IRQ_COUNT]) {
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
pub fn isa_routing(irq: u32) -> IsaIrqRouting {
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

/// Bring up the interrupt controller.
///
/// # BUGS
/// **Unimplemented, and it is the argument rather than the work.** The IO APIC's bring-up is
/// [`init_io_apic`], which takes the address and global-interrupt base the ACPI MADT supplied; this
/// name is the arch contract's no-argument one, written for two architectures that read those facts
/// out of a device tree the shared `memory::init` had already parsed. Wiring it up is the device
/// discovery seam (roadmap item 0), not this module.
#[allow(dead_code)]
pub fn init() {
    unimplemented!("x86_64 irq::init: see init_io_apic, which takes the MADT's address")
}

/// Bring up this CPU's local interrupt interface (milestone 161's SMP item): a secondary's own
/// call to [`init_local_apic`], at the SAME physical address the boot core already found in the
/// MADT and mapped device-typed.
///
/// **Every core's local APIC answers at this identical MMIO address**, unlike a device with one
/// physical instance: the address is architecturally per-core (`IA32_APIC_BASE`, reset to the same
/// value on every core), so a redundant [`mask_the_8259s`] and a redundant write of [`LOCAL_APIC`]/
/// [`LOCAL_APIC_PHYS`] to the SAME value are both harmless idempotent re-statements of what the
/// boot core already recorded, not a race over shared state.
///
/// A no-op if the boot core never found a local APIC at all (`local_apic_phys()` is `None`):
/// unreachable in practice, because `smp::bring_up_secondaries` only starts a core once
/// `arch::can_start_secondaries()` has already required exactly that.
pub fn init_this_cpu() {
    if let Some(apic) = local_apic_phys() {
        // SAFETY: the same physical address the boot core already validated (it is where
        // `read_acpi` found the MADT's local APIC entry) and mapped device-typed in `mmu::init`'s
        // fine map, which every core shares (x86 has one kernel root; see `mmu::init_secondary`).
        unsafe { init_local_apic(apic) };
    }
}

/// **Unmask interrupt `intid` at the controller**, where `intid` is a *legacy IRQ number* the way
/// the arch contract's other two implementations take an INTID or a PLIC source.
///
/// The translation is the whole point of this function: [`isa_routing`] turns the legacy number
/// into the GSI the MADT says it actually arrives on, plus that line's polarity and trigger mode,
/// and only then is a redirection entry written. IRQ 0 becomes GSI 2 here, and a version of this
/// that skipped the step would arm the 8259 cascade and report success.
///
/// The vector is [`gsi_vector`]'s, and the destination is the boot CPU.
///
/// # An intid on this architecture is one of two things
///
/// **A local APIC source names itself by its vector**, because there is no controller input to
/// name: [`RESCHEDULE_VECTOR`], [`SELF_TEST_VECTOR`] and its `_B` twin are raised by writing the
/// ICR, and the ICR takes a vector. There is nothing to unmask, so this is a **no-op** for them,
/// and that is a real answer rather than a shrug: the line is already deliverable the moment the
/// local APIC is enabled, which is what `RFLAGS.IF` then gates.
///
/// **An MSI vector is the same case for a stronger reason** (milestone 215). A PCI function with
/// MSI-X enabled raises its interrupt by *writing* to the local APIC's message address, so there
/// is no controller input anywhere in the path: no redirection entry, no swizzle, no `_PRT`. There
/// is nothing to unmask, and nothing for a driver's `Irq::ACK` to re-arm either, which is correct
/// rather than a gap: the message is edge-delivered and already over by the time the driver runs.
///
/// **Everything else is a legacy IRQ**, 0..15, and goes through the translation above.
///
/// The two ranges cannot collide, which is what makes one function able to take both:
/// [`GSI_VECTOR_BASE`]'s doc reserves 0x20..0x2f for the local APIC's own sources, and a legacy IRQ
/// number never reaches 0x20. **This was a real bug until userspace arrived** (milestone 161, item
/// 4's hand-off): `spawn_elf_child_builder` enables `user::DELEGATED_TEST_INTERRUPT`, which on
/// x86 *is* `SELF_TEST_VECTOR`, and routing 34 as though it were a legacy IRQ panicked with
/// "gsi 34 is outside the IO APIC's range". Nothing had ever called `enable` with a local-APIC
/// number before, because nothing above the arch layer had run.
pub fn enable(intid: u32) {
    if is_local_apic_source(intid) || is_msi_vector(intid as u64) {
        return;
    }
    let routing = isa_routing(intid);
    // A GSI with no redirection entry has no vector, which is the same condition `route_gsi` panics
    // on one line below. It is refused here instead, and not only to satisfy the `Option`: this
    // message names the `intid` the caller passed, which `route_gsi` never sees. The bug quoted
    // above surfaced as "gsi 34 is outside the IO APIC's range" when what the caller said was 34,
    // and reading that took a while precisely because the two numbers had been silently swapped.
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

/// **Hold a routed line off until its driver ACKs**: the trap handler's act for a line it hands to
/// a userspace driver, undone by [`enable`] when the driver ACKs. aarch64's `irq::disable` is the
/// same step.
///
/// **A level-triggered line is masked; an edge-triggered one is left live**, and the difference is
/// the IO APIC's rather than a choice. A GIC keeps an interrupt pending while it is disabled. An IO
/// APIC does not: an edge that arrives on a masked entry is dropped. COM1's IRQ 4 is edge-triggered
/// on every PC, and a 16550 holds its line high until the driver drains it, so masking it would turn
/// a byte that arrives between the driver's last read and its ACK into an input that never wakes
/// anything again: the line is already high when the entry is unmasked, so no new edge comes.
/// Left live, that byte raises a fresh edge after the drain, and the rendezvous counts it.
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

/// **Raise `vector` on the CPU executing this**, through the local APIC's self shorthand.
///
/// This is x86's answer to a question the other two architectures answer very differently, and the
/// asymmetry is worth stating because `sched`'s interrupt-delivery tests depend on it. aarch64 can
/// raise a software-generated interrupt on itself with no device at all. RISC-V cannot raise
/// anything: `sip.SEIP` is read-only to S-mode and the PLIC's pending block is read-only by
/// specification, so its tests assert the console UART's transmit-empty line instead. x86 is
/// aarch64's case rather than RISC-V's: the local APIC will deliver any vector to itself on demand,
/// through a real delivery path (the ICR, the IRR, the ISR, an EOI), so the interrupt is the
/// hardware's and not a function call wearing a handler's name.
///
/// It is deliberately **not** the console UART's line. COM1's interrupt is discoverable here
/// (`Acpi::isa_irqs[4]`) and unwired, and asserting a device this port has no driver for would prove
/// less than this does while being able to fail for reasons unrelated to the kernel.
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

/// Send a reschedule inter-processor interrupt to `target_cpu`, whose handler drains its inbox and
/// serves any outstanding work-steal request (`sched::drain_inbox`, `sched::serve_steal_request`).
/// The x86 counterpart of aarch64's reschedule SGI and RISC-V's SBI IPI.
///
/// **The logical cpu id is looked up in the roster rather than used as the destination local APIC
/// id directly** (fixed, milestone 161's SMP item). The two numbers are independent in general; the
/// MADT states the mapping and `smp::seat_cpus_from_acpi` reads it into the same roster
/// `bring_up_secondaries` uses, seating every core (the boot core included, via
/// `arch::boot_cpu_id`'s own `CPUID` read) at the slot its own local APIC id names, so
/// `smp::hwid(id)` is that id.
///
/// # Panics
/// If `target_cpu` names no seated core. Every caller in `sched` first checks the target is an
/// online core (`smp::online_cpus`), and an online core is by construction a seated one.
pub fn send_reschedule(target_cpu: usize) {
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
pub fn send_init(dest_apic_id: u8) {
    wait_for_ipi_delivery();
    write(reg::ICR_HIGH, (dest_apic_id as u32) << 24);
    write(reg::ICR_LOW, ICR_INIT | ICR_ASSERT);
}

/// **Send a STARTUP inter-processor interrupt ("SIPI")** to the local APIC whose id is
/// `dest_apic_id`, naming the page it should begin executing at: physical address
/// `(vector as u64) << 12`. The target starts there in 16-bit real mode, with `cs` = `vector << 8`
/// and `ip` = 0 (`cs * 16 + ip` is exactly `vector << 12`).
pub fn send_startup(dest_apic_id: u8, vector: u8) {
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
pub fn print_summary() {
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

/// **Proofs about the vector space** (milestone 304), the first properties this tree has ever
/// checked on `arch/x86_64/`: until 304 the prover compiled `arch/aarch64/` and nothing else, so
/// this whole subtree was out of reach for a `cfg` rather than for a construct. See
/// notes/kernel-proofs.md, whose stub list applies here unchanged, and
/// design/roadmap/0304-prover-one-architecture.md for why a second host is what made them runnable.
///
/// Both harnesses are about **numbers firmware chose**. The MADT supplies the IO APIC's GSI base,
/// the version register supplies its entry count, and an interrupt source override supplies the GSI
/// a legacy IRQ resolves to. Every one of those decides which vector an interrupt is delivered on,
/// and a vector is an index into the IDT: getting it wrong does not misroute an interrupt, it runs
/// the page-fault handler.
#[cfg(kani)]
mod proofs {
    use super::*;

    /// **The three vector bands never overlap, whatever the version register says.**
    ///
    /// [`MSI_VECTOR_BASE`]'s doc claims the bands are "disjoint by construction rather than by
    /// anybody remembering", and the construction is [`MAX_REDIRECTION_ENTRIES`] clamping the
    /// entry count in [`init_io_apic`]. This is that claim, stated over every entry count an
    /// eight-bit version field can report and every allocation state the MSI bump counter can be
    /// in, rather than over the two numbers QEMU happens to produce.
    ///
    /// The claim is deliberately not a restatement of the constants: it asks the two *predicates*
    /// the trap handler actually calls, so a change to either one's arithmetic breaks this even if
    /// the constants are untouched.
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

    /// **A GSI this IO APIC owns is routed to a vector inside the IO APIC's own band**, whatever
    /// global interrupt base the MADT gave it.
    ///
    /// [`route_gsi`] is called as `route_gsi(gsi, gsi_vector(gsi), ...)` from [`enable`], and
    /// [`redirection_index`] is the only thing between a GSI the MADT supplied and a raw
    /// redirection-table write. So the question a model checker can settle is whether the *guard*
    /// and the *vector* agree: for every GSI the guard admits, does the flat map land in
    /// `GSI_VECTOR_BASE..MSI_VECTOR_BASE`? The same shape as milestone 255's
    /// `no_stream_can_reach_another_streams_tables` one architecture over: a firmware-supplied
    /// identifier, one bounds check, and a write that cannot be taken back.
    ///
    /// **No precondition on `base`**: the property holds over every `base`, every entry count an
    /// eight-bit version field can report, and every `gsi` in `u32`, because routing is by
    /// redirection index (a GSI-based map once put GSI 255 on a base of 127 at `0x2f`, inside the
    /// local APIC's own band).
    ///
    /// **The second assertion is the one milestone 308 added**, and it is a claim about two
    /// functions rather than one. [`is_device_vector`] has always defined the band by *index*
    /// (`GSI_VECTOR_BASE` up to the entry count) while [`gsi_vector`] assigned by *GSI*, so on a
    /// nonzero base the trap handler declined to count an interrupt this kernel had itself routed
    /// as a device vector. They are now the same arithmetic, and this says so over every input
    /// instead of leaving it to a reader comparing two function bodies.
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

    /// **A GSI owned by an IO APIC based at global interrupt 200 routes inside the device band**,
    /// and the vector the trap handler would see is one it counts as a device vector.
    ///
    /// The numbers are the proposal's plausible-hardware case rather than the prover's: base 200
    /// with the 24 redirection entries every real part has, so GSI 210 is entry 10. Before
    /// milestone 308 that GSI mapped to `0x30.wrapping_add(210)`, which is **2, the NMI**, and the
    /// assertion on `NMI` below is why this test names a vector rather than only a range: a reader
    /// meeting a failure here should see immediately what the old map did with it.
    ///
    /// The two statics are this machine's real IO APIC state, so they are saved and put back. They
    /// are read by the trap handler's `is_device_vector`, and the entry count is restored to the
    /// same value it is set to here, so nothing in flight can see a band that moved.
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
