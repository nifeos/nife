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

use crate::sync::{IrqSafeMutex, rank};

// Register offsets and bits from the GIC architecture specification, version 2 (Arm IHI 0048B),
// chapter 4, under each register's own heading, and checked against Linux's
// `include/linux/irqchip/arm-gic.h` (v6.16).

// The distributor, one per machine.

/// `GICD_CTLR`: distributor control. Bit 0 enables forwarding to the CPU interfaces.
const GICD_CTLR: usize = 0x000;
const GICD_CTLR_ENABLE: u32 = 1 << 0;
/// `GICD_ISENABLERn`: set-enable, **one bit per interrupt**, 32 per register, write-one-to-set.
const GICD_ISENABLER: usize = 0x100;
/// `GICD_ICENABLERn`: clear-enable, the same layout, write-one-to-clear.
const GICD_ICENABLER: usize = 0x180;
/// `GICD_IPRIORITYRn`: priority, **one byte per interrupt**, byte-accessible.
const GICD_IPRIORITYR: usize = 0x400;
/// `GICD_ITARGETSRn`: the target cores of each SPI, one byte per interrupt, bit N for core N.
const GICD_ITARGETSR: usize = 0x800;
/// `GICD_SGIR`: raises a software-generated interrupt. Bits 3:0 the SGI, bits 23:16 the target
/// list, bit N for core N.
const GICD_SGIR: usize = 0xf00;
const GICD_SGIR_TARGET_SHIFT: u32 = 16;

// The CPU interface, banked: every core sees its own at the same address.

/// `GICC_CTLR`: CPU interface control. Bit 0 enables signaling to the core.
const GICC_CTLR: usize = 0x000;
const GICC_CTLR_ENABLE: u32 = 1 << 0;
/// `GICC_PMR`: priority mask. An interrupt is delivered only if its priority is **strictly less
/// than** this. Backwards from what you'd guess: `0xff` lets everything through.
const GICC_PMR: usize = 0x004;
/// `GICC_IAR`: reading it ACKNOWLEDGES an interrupt and says which one, in bits 9:0. **1023 means
/// spurious**: the GIC changed its mind between raising the line and us getting here (another
/// core took it, or it was masked). Read it, do nothing, do NOT signal end-of-interrupt.
const GICC_IAR: usize = 0x00c;
const GICC_IAR_INTID: u32 = 0x3ff;
/// `GICC_EOIR`: writing the same INTID back says "I'm done". Until you do, the GIC will not
/// deliver another interrupt of equal or lower priority.
const GICC_EOIR: usize = 0x010;
/// `GICC_IIDR`: identification. `ArchitectureVersion` (bits 19:16) is 2 on a GIC-400 and on QEMU's
/// model; a GICv3 redistributor frame answers zero here. Read once at boot.
const GICC_IIDR: usize = 0x0fc;

/// The lowest priority (highest value) that still gets through. `0xff` = everything.
const PRIORITY_MASK: u32 = 0xff;

/// The priority we give the timer. Anything below PMR is delivered; 0 is the most urgent.
const TIMER_PRIORITY: u8 = 0x80;

/// The GIC reported a spurious interrupt: do nothing, and do NOT signal EOI.
pub(crate) const SPURIOUS: u32 = 1023;

/// The two register blocks' virtual addresses, as `init`'s caller named them.
pub(crate) struct Gic {
    gicd: u64,
    gicc: u64,
}

static GIC: IrqSafeMutex<Option<Gic>> = IrqSafeMutex::new(rank::IRQ_CONTROLLER, None);

impl Gic {
    /// Write the distributor's 32-bit register at `offset`.
    fn distributor_write32(&self, offset: usize, value: u32) {
        // SAFETY: `init`'s caller promised a mapped GICv2 distributor (4 KiB) at `gicd`, and every
        // offset passed here is a register above, 4-byte aligned and inside the block.
        unsafe { core::ptr::write_volatile((self.gicd as usize + offset) as *mut u32, value) }
    }

    /// Write the distributor's byte register at `offset`.
    fn distributor_write8(&self, offset: usize, value: u8) {
        // SAFETY: as `distributor_write32`; `GICD_IPRIORITYRn` and `GICD_ITARGETSRn` are
        // byte-accessible, and their offsets stay below 0xc00.
        unsafe { core::ptr::write_volatile((self.gicd as usize + offset) as *mut u8, value) }
    }

    /// Read this core's CPU interface's 32-bit register at `offset`.
    fn cpu_read32(&self, offset: usize) -> u32 {
        // SAFETY: `init`'s caller promised a mapped GICv2 CPU interface at `gicc`, and every
        // offset passed here is a register above, 4-byte aligned and inside the block.
        unsafe { core::ptr::read_volatile((self.gicc as usize + offset) as *const u32) }
    }

    /// Write this core's CPU interface's 32-bit register at `offset`.
    fn cpu_write32(&self, offset: usize, value: u32) {
        // SAFETY: as `cpu_read32`.
        unsafe { core::ptr::write_volatile((self.gicc as usize + offset) as *mut u32, value) }
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
    unsafe { core::ptr::read_volatile((gicc as usize + GICC_IIDR) as *const u32) }
}

/// Bring the GIC up.
///
/// `gicd` and `gicc` are **virtual** addresses: the caller has mapped both as device memory.
///
/// # Safety
/// The two addresses must name a real GICv2, mapped and exclusively ours.
pub unsafe fn init(gicd: u64, gicc: u64) {
    let gic = Gic { gicd, gicc };

    // Distributor: on. This is machine-wide, so **only the boot core does it** (this function).
    gic.distributor_write32(GICD_CTLR, GICD_CTLR_ENABLE);

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
    gic.cpu_write32(GICC_PMR, PRIORITY_MASK);
    gic.cpu_write32(GICC_CTLR, GICC_CTLR_ENABLE);
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
    gic.distributor_write32(GICD_ISENABLER + 4 * reg, 1 << bit);

    gic.distributor_write8(GICD_IPRIORITYR + intid as usize, TIMER_PRIORITY);

    // ITARGETSR (bit N = core N) means something only for SPIs; PPIs and SGIs are per-core.
    if intid >= 32 {
        gic.distributor_write8(GICD_ITARGETSR + intid as usize, 1 << target_cpu);
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
    gic.distributor_write32(GICD_ICENABLER + 4 * reg, 1 << bit);
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

    gic.distributor_write32(GICD_SGIR, (1 << (GICD_SGIR_TARGET_SHIFT + target_cpu as u32)) | intid);
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
    gic.cpu_read32(GICC_IAR) & GICC_IAR_INTID
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
    gic.cpu_write32(GICC_EOIR, intid);
}
