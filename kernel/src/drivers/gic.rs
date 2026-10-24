//! GICv2: the ARM Generic Interrupt Controller, a machine-wide distributor (GICD) and a banked
//! per-core CPU interface (GICC).
//!
//! Lower priority values are more urgent, and `GICC_PMR` delivers only a priority strictly less
//! than itself. See notes/interrupts.md, "The GIC: the multiplexer in front of the CPU", and
//! notes/interrupts/controller-drivers.md.
//!

//! GICv2 only: a GICv3 is `gicv3.rs` plus `arch/aarch64/gic_cpu_interface.rs`, and
//! `arch/aarch64/irq.rs` picks one from the device tree and confirms it against the hardware.
//!
//! # BUGS
//!
//! - [`send_sgi`] issues no barrier before the `SGIR` write, so the Device store is not ordered
//!   after the Normal stores the target core will read (the GICv3 path has the `dsb`). TCG does
//!   not model the reordering; argon, a GIC-400, would. The fix is one `dsb ishst` in
//!   `arch::irq::send_sgi` ahead of the GICv2 call.

use tock_registers::interfaces::{Readable, Writeable};
use tock_registers::registers::{ReadOnly, ReadWrite, WriteOnly};
use tock_registers::{register_bitfields, register_structs};

use crate::sync::{IrqSafeMutex, rank};

register_bitfields! {
    u32,

    /// Distributor control.
    GICD_CTLR [
        ENABLE OFFSET(0) NUMBITS(1) [],
    ],

    /// CPU interface control.
    GICC_CTLR [
        ENABLE OFFSET(0) NUMBITS(1) [],
    ],

    /// Interrupt acknowledge: reading this *takes* the interrupt.
    GICC_IAR [
        /// The interrupt we are now servicing. **1023 means spurious**: the GIC changed its
        /// mind between raising the line and us getting here (another core took it, or it was
        /// masked). Read it, do nothing, do NOT signal end-of-interrupt.
        INTID OFFSET(0) NUMBITS(10) [],
    ],
}

register_structs! {
    /// The distributor. One per machine.
    #[allow(non_snake_case)]
    pub Distributor {
        (0x000 => CTLR: ReadWrite<u32, GICD_CTLR::Register>),
        (0x004 => TYPER: ReadOnly<u32>),
        (0x008 => _reserved0),
        /// Set-enable. **One BIT per interrupt**, 32 per register.
        (0x100 => ISENABLER: [ReadWrite<u32>; 32]),
        (0x180 => ICENABLER: [ReadWrite<u32>; 32]),
        (0x200 => _reserved1),
        /// Priority. One byte per interrupt, a different stride from ISENABLER's bitmap.
        (0x400 => IPRIORITYR: [ReadWrite<u8>; 1024]),
        (0x800 => ITARGETSR: [ReadWrite<u8>; 1024]),
        (0xc00 => _reserved2),
        /// Software Generated Interrupt Register: raises one of the 16 SGIs on a chosen set of
        /// cores.
        (0xf00 => SGIR: WriteOnly<u32>),
        (0xf04 => _reserved3),
        (0x1000 => @END),
    }
}

register_structs! {
    /// The per-core CPU interface. Banked: every core sees its own at the same address.
    #[allow(non_snake_case)]
    pub CpuInterface {
        (0x000 => CTLR: ReadWrite<u32, GICC_CTLR::Register>),
        /// Priority mask. An interrupt is delivered only if its priority is **strictly less
        /// than** this. Backwards from what you'd guess: `0xff` lets everything through.
        (0x004 => PMR: ReadWrite<u32>),
        (0x008 => _reserved0),
        /// Reading this ACKNOWLEDGES an interrupt and tells you which one.
        (0x00c => IAR: ReadOnly<u32, GICC_IAR::Register>),
        /// Writing the same INTID back says "I'm done". Until you do, the GIC will not deliver
        /// another interrupt of equal or lower priority.
        (0x010 => EOIR: WriteOnly<u32>),
        (0x014 => _reserved1),
        /// Identification. `ArchitectureVersion` (bits 19:16) is 2 on a GIC-400 and on QEMU's
        /// model; a GICv3 redistributor frame answers zero here. Read once at boot.
        (0x0fc => IIDR: ReadOnly<u32>),
        (0x100 => @END),
    }
}

/// The lowest priority (highest value) that still gets through. `0xff` = everything.
const PRIORITY_MASK: u32 = 0xff;

/// The priority we give the timer. Anything below PMR is delivered; 0 is the most urgent.
const TIMER_PRIORITY: u8 = 0x80;

/// The GIC reported a spurious interrupt: do nothing, and do NOT signal EOI.
pub(crate) const SPURIOUS: u32 = 1023;

pub(crate) struct Gic {
    gicd: *mut Distributor,
    gicc: *mut CpuInterface,
}

// SAFETY: MMIO pointers, not Rust-managed memory. The lock provides exclusion.
unsafe impl Send for Gic {}

static GIC: IrqSafeMutex<Option<Gic>> = IrqSafeMutex::new(rank::IRQ_CONTROLLER, None);

impl Gic {
    fn gicd(&self) -> &Distributor {
        // SAFETY: the address came from the device tree and was mapped as device memory.
        unsafe { &*self.gicd }
    }

    fn gicc(&self) -> &CpuInterface {
        // SAFETY: as above.
        unsafe { &*self.gicc }
    }
}

/// `GICC_IIDR` at `gicc`, read before anything else is trusted: `arch::irq` hands it to
/// `machine_discovery::gic::Gic::confirm`, so a tree naming the wrong block fails at boot instead
/// of booting and taking nothing.
///
/// # Safety
/// `gicc` must be a mapped device-memory address with at least 256 bytes behind it.
pub(crate) unsafe fn identification(gicc: u64) -> u32 {
    // SAFETY: per this function's contract.
    unsafe { (*(gicc as *const CpuInterface)).IIDR.get() }
}

/// Bring the GIC up.
///
/// `gicd` and `gicc` are **virtual** addresses: the caller has mapped both as device memory.
///
/// # Safety
/// The two addresses must name a real GICv2, mapped and exclusively ours.
pub unsafe fn init(gicd: u64, gicc: u64) {
    let gic = Gic {
        gicd: gicd as *mut Distributor,
        gicc: gicc as *mut CpuInterface,
    };

    // Distributor: on. This is machine-wide, so **only the boot core does it** (this function).
    gic.gicd().CTLR.write(GICD_CTLR::ENABLE::SET);

    *GIC.lock() = Some(gic);

    // The boot core's own CPU interface. Every other core brings up its own via `init_this_cpu`.
    init_this_cpu();
}

/// Bring up **this core's** GIC CPU interface. The distributor ([`init`]) is machine-wide and done
/// once; the CPU interface is banked per core (same MMIO address, different hardware behind it), so
/// each core must enable its own. Called by the boot core inside [`init`] and by every secondary as
/// it comes online (DECISIONS §11).
pub(crate) fn init_this_cpu() {
    let guard = GIC.lock();
    let gic = guard.as_ref().expect("gic::init_this_cpu before gic::init");

    // ORDER: the mask before the enable, or the interface goes live with the firmware's PMR,
    // often 0 ("deliver nothing") on a cold boot.
    gic.gicc().PMR.set(PRIORITY_MASK);
    gic.gicc().CTLR.write(GICC_CTLR::ENABLE::SET);
}

/// Enable one interrupt source, give it a priority, and target it at `target_cpu`.
///
/// `target_cpu` is the core the distributor should deliver this SPI to (bit N of `ITARGETSR`). The
/// policy that chooses it (spreading device lines across cores so they do not all funnel through
/// core 0) lives one level up in `arch::irq::enable`; this driver only writes the register it is
/// told to, keeping the choice out of the GIC (DECISIONS §4: a driver reaches into no kernel
/// global, including the online-core count). PPIs and SGIs are per-core, so `target_cpu` is ignored
/// for them.
pub fn enable(intid: u32, target_cpu: usize) {
    let guard = GIC.lock();
    let gic = guard.as_ref().expect("gic::enable before gic::init");

    let reg = (intid / 32) as usize;
    let bit = intid % 32;
    gic.gicd().ISENABLER[reg].set(1 << bit);

    gic.gicd().IPRIORITYR[intid as usize].set(TIMER_PRIORITY);

    // ITARGETSR (bit N = core N) means something only for SPIs; PPIs and SGIs are per-core.
    if intid >= 32 {
        gic.gicd().ITARGETSR[intid as usize].set(1 << target_cpu);
    }
}

/// Disable one interrupt source, at the distributor.
///
/// How a userspace driver's interrupt is masked the moment it fires, so a level-triggered line
/// cannot storm before the driver services it; its `Irq` capability's `ACK` re-enables it. See
/// notes/interrupts.md.
pub(crate) fn disable(intid: u32) {
    let guard = GIC.lock();
    let gic = guard.as_ref().expect("gic::disable before gic::init");

    // Write-1-to-clear; zero bits leave their neighbours alone.
    let reg = (intid / 32) as usize;
    let bit = intid % 32;
    gic.gicd().ICENABLER[reg].set(1 << bit);
}

/// Raise a Software Generated Interrupt (INTID 0-15) on core 0.
///
/// A controllable interrupt with no hardware behind it. The whole "an interrupt becomes a
/// message" path can be tested with this: raise the SGI, watch a blocked thread wake.
pub(crate) fn send_sgi(intid: u32, target_cpu: usize) {
    debug_assert!(intid < 16, "SGIs are INTID 0-15");
    debug_assert!(target_cpu < 8, "GICv2 SGIR targets at most 8 cores");
    let guard = GIC.lock();
    let gic = guard.as_ref().expect("gic::send_sgi before gic::init");

    // SGIR: bits[3:0] the SGI id, bits[23:16] the target CPU list (bit N = core N).
    gic.gicd().SGIR.set((1 << (16 + target_cpu)) | intid);
}

/// Take the interrupt. **Reading `IAR` is what acknowledges it**, so this has a side effect
/// and must be called exactly once per IRQ.
///
/// Returns [`SPURIOUS`] (1023) if the GIC changed its mind. Do nothing with that, and in
/// particular do **not** call [`end_of_interrupt`]: signalling completion for an interrupt you
/// never took corrupts the GIC's priority stack.
pub(crate) fn acknowledge() -> u32 {
    let guard = GIC.lock();
    let gic = guard.as_ref().expect("gic::acknowledge before gic::init");
    gic.gicc().IAR.read(GICC_IAR::INTID)
}

/// "I'm finished with this one."
///
/// Until this is written, the GIC will not deliver another interrupt of **equal or lower**
/// priority. Forget it and the timer fires exactly once and then never again, which looks
/// nothing like "you forgot to write a register".
pub(crate) fn end_of_interrupt(intid: u32) {
    let guard = GIC.lock();
    let gic = guard
        .as_ref()
        .expect("gic::end_of_interrupt before gic::init");
    gic.gicc().EOIR.set(intid);
}
