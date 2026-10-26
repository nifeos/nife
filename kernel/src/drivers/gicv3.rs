//! GICv3: the memory-mapped half, the distributor and one redistributor per core.
//!
//! The CPU interface (`ICC_*` system registers) is `arch/aarch64/gic_cpu_interface.rs`, because
//! `msr`/`mrs` are architecture code; `arch::irq` holds both halves and picks the GIC version. Each
//! core finds its redistributor by walking the frames for its own affinity, every interrupt is
//! Group 1, and writes wait on `RWP` where Linux waits. See notes/interrupts/controller-drivers.md.
//!

//! # BUGS
//!
//! - No ITS, so no LPIs or message-signaled interrupts (milestone 317 (the interrupt-remapping
//!   flags, and where MSI confinement actually lives) wants them).
//! - No extended SPI or PPI ranges; [`enable`] refuses an INTID past 1019.
//! - No redistributor power-down; nothing in this kernel powers a core off.
//! - The frame stride is inferred from `VLPIS`. A board that pads its frames and states a
//!   `redistributor-stride` would be misread, and the walk would fail loudly.
//!
//! Name: provisional (`gicv3` for the module), and a lane's to propose: the version suffix is the
//! vocabulary Arm's own documents and Linux's `irq-gic-v3.c` use for exactly this split.

use core::sync::atomic::{AtomicU64, Ordering};

// Register offsets and bits from the GIC architecture specification, versions 3 and 4 (Arm IHI
// 0069), chapter 12, under each register's own heading, and checked against Linux's
// `include/linux/irqchip/arm-gic-v3.h` (v6.16).

// The distributor, one 64 KiB frame per machine.

/// `GICD_CTLR`.
const GICD_CTLR: usize = 0x0000;
const GICD_CTLR_ENABLE_G1: u32 = 1 << 0; // EnableGrp1 (non-secure view) or EnableGrp0 (DS=1)
const GICD_CTLR_ENABLE_G1A: u32 = 1 << 1; // EnableGrp1A (non-secure view) or EnableGrp1 (DS=1)
const GICD_CTLR_ARE_NS: u32 = 1 << 4; // affinity routing, non-secure
const GICD_CTLR_RWP: u32 = 1 << 31; // register write pending
/// `GICD_TYPER`. `ITLinesNumber`, bits 4:0: the distributor implements 32 * (N + 1) INTIDs.
const GICD_TYPER: usize = 0x0004;
const GICD_TYPER_IT_LINES: u32 = 0x1f;
/// `GICD_IGROUPRn`: group, one BIT per interrupt. Ones mean Group 1, the only group this kernel
/// takes.
const GICD_IGROUPR: usize = 0x0080;
/// `GICD_ISENABLERn` and `GICD_ICENABLERn`: set- and clear-enable, one bit per interrupt,
/// write-one-to-act.
const GICD_ISENABLER: usize = 0x0100;
const GICD_ICENABLER: usize = 0x0180;
/// `GICD_ICACTIVERn`: clear-active, one bit per interrupt.
const GICD_ICACTIVER: usize = 0x0380;
/// `GICD_IPRIORITYRn`: priority, one BYTE per interrupt, as on GICv2.
const GICD_IPRIORITYR: usize = 0x0400;
/// `GICD_IROUTERn`: **routing, one 64-bit AFFINITY per SPI** (INTIDs 32-1019; entries 0-31 are
/// reserved), at `0x6000 + 8n`. Replaces GICv2's `ITARGETSR` once `CTLR.ARE` is set. Layout is
/// `MPIDR_EL1`'s: Aff0 in bits 7:0, Aff1 15:8, Aff2 23:16, Aff3 39:32, and bit 31 (`IRM`) clear
/// for "this core".
const GICD_IROUTER: usize = 0x6000;
/// `GICD_PIDR2` and `GICR_PIDR2`: peripheral ID 2. `ArchRev` in bits 7:4 is 3 on a GICv3 and 4 on
/// a GICv4.
const PIDR2: usize = 0xffe8;

// A redistributor's first frame, `RD_base`: control, identity and power.

/// `GICR_CTLR`. Bit 3 is `RWP`.
const GICR_CTLR: usize = 0x0000;
const GICR_CTLR_RWP: u32 = 1 << 3;
/// `GICR_TYPER`, 64 bits. Bits 63:32: this frame's core, as Aff3.Aff2.Aff1.Aff0, one byte each.
/// Bit 4 `Last`: last frame in the region. Bit 1 `VLPIS`: virtual LPIs, which doubles the frame
/// stride.
const GICR_TYPER: usize = 0x0008;
const GICR_TYPER_VLPIS: u64 = 1 << 1;
const GICR_TYPER_LAST: u64 = 1 << 4;
/// `GICR_WAKER`. Bit 1 `ProcessorSleep`, bit 2 `ChildrenAsleep`. Out of reset a redistributor is
/// asleep and delivers nothing to its core, so waking it is the first thing a core does.
const GICR_WAKER: usize = 0x0014;
const GICR_WAKER_PROCESSOR_SLEEP: u32 = 1 << 1;
const GICR_WAKER_CHILDREN_ASLEEP: u32 = 1 << 2;

// A redistributor's second frame, `SGI_base`: the GICv2 distributor's banked registers for INTIDs
// 0-31, at the distributor's offsets, moved out to live beside the core they belong to.

/// `GICR_IGROUPR0`, `GICR_ISENABLER0`, `GICR_ICENABLER0`, `GICR_ICACTIVER0`: one bit per private
/// INTID.
const GICR_IGROUPR0: usize = 0x0080;
const GICR_ISENABLER0: usize = 0x0100;
const GICR_ICENABLER0: usize = 0x0180;
const GICR_ICACTIVER0: usize = 0x0380;
/// `GICR_IPRIORITYRn`: one byte per private INTID, 32 of them.
const GICR_IPRIORITYR: usize = 0x0400;

/// Every frame, the distributor's and each of a redistributor's, is 64 KiB.
const FRAME: u64 = 0x1_0000;

/// A register's width: what [`Frame::read`] and [`Frame::write`] may move in one access.
trait Width: Copy {}
impl Width for u8 {}
impl Width for u32 {}
impl Width for u64 {}

/// **One mapped 64 KiB register frame**: the distributor, or a redistributor's `RD_base` or
/// `SGI_base`. Only [`Frame::new`]'s contract makes one, so a safe access through it is sound for
/// any in-frame, naturally aligned offset, which every access asserts.
#[derive(Clone, Copy)]
struct Frame(u64);

impl Frame {
    /// # Safety
    /// `base` must be the virtual address of a GICv3 register frame mapped as device memory, 64 KiB
    /// of it, and never unmapped.
    unsafe fn new(base: u64) -> Self {
        Self(base)
    }

    /// The register of type `T` at `offset`: in the frame, and aligned to its own width.
    fn register<T: Width>(self, offset: usize) -> usize {
        let size = core::mem::size_of::<T>();
        assert!(
            offset % size == 0 && offset + size <= FRAME as usize,
            "gicv3: no {size}-byte register at {offset:#x}"
        );
        self.0 as usize + offset
    }

    /// Read the register of type `T` at `offset`.
    fn read<T: Width>(self, offset: usize) -> T {
        // SAFETY: `new`'s contract maps the frame, and `register` asserts the access is inside it
        // and aligned.
        unsafe { core::ptr::read_volatile(self.register::<T>(offset) as *const T) }
    }

    /// Write the register of type `T` at `offset`.
    fn write<T: Width>(self, offset: usize, value: T) {
        // SAFETY: as `read`.
        unsafe { core::ptr::write_volatile(self.register::<T>(offset) as *mut T, value) }
    }
}

/// Priority for every line nobody has enabled yet, Linux's `GICD_INT_DEF_PRI`. Irrelevant while a
/// line is disabled; [`enable`] replaces it with [`ENABLED_PRIORITY`].
const DEFAULT_PRIORITY: u8 = 0xa0;

/// The priority an enabled line gets, the same value the GICv2 driver gives the timer. Lower is
/// more urgent, and the CPU interface's mask lets everything below `0xff` through.
const ENABLED_PRIORITY: u8 = 0x80;

/// The first of the four special INTIDs (1020-1023) an acknowledge can return. 1023 is "spurious".
pub(crate) const SPECIAL_FIRST: u32 = 1020;

/// How many redistributors this driver remembers, one per slot the caller names. The aarch64
/// adapter asserts at compile time that `cpu::MAX_CPUS` fits, so the kernel's core count and this
/// table cannot drift apart without a build failure.
pub(crate) const SLOTS: usize = 8;

/// The distributor's virtual address, zero until [`init`]. Written once, on the boot core, before
/// any other core exists; everything after reads it.
static DISTRIBUTOR: AtomicU64 = AtomicU64::new(0);
/// The redistributor region, as the device tree named it (virtual base, length).
static REDISTRIBUTOR_REGION: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];
/// Each slot's own `RD_base`, zero until that core has run [`init_this_cpu`].
static REDISTRIBUTOR: [AtomicU64; SLOTS] = [const { AtomicU64::new(0) }; SLOTS];

fn gicd() -> Frame {
    let base = DISTRIBUTOR.load(Ordering::Relaxed);
    assert!(base != 0, "gicv3 used before gicv3::init");
    // SAFETY: `init`'s caller promised a mapped GICv3 distributor at this address, and it is never
    // unmapped.
    unsafe { Frame::new(base) }
}

/// `slot`'s redistributor: its `RD_base` frame, then its `SGI_base` frame.
fn rd(slot: usize) -> (Frame, Frame) {
    let base = REDISTRIBUTOR[slot].load(Ordering::Relaxed);
    assert!(
        base != 0,
        "gicv3: slot {slot} has no redistributor; init_this_cpu has not run on that core"
    );
    // SAFETY: `init_this_cpu` stored a frame it found inside the region `init`'s caller promised is
    // mapped, and whose own GICR_TYPER named this core.
    unsafe { (Frame::new(base), Frame::new(base + FRAME)) }
}

/// Spin until a register-write-pending bit reads clear. Bounded, and loud past the bound: an `RWP`
/// that never clears is a controller not answering, and a silent hang here would look like a boot
/// that simply stopped.
fn wait_for_rwp(read: impl Fn() -> u32, bit: u32, what: &str) {
    for _ in 0..10_000_000u32 {
        if read() & bit == 0 {
            return;
        }
        core::hint::spin_loop();
    }
    panic!("gicv3: {what} RWP never cleared; the controller is not answering");
}

/// `GICD_PIDR2` at `gicd`, read before anything else is trusted: the adapter hands it to
/// `machine_discovery::gic::Gic::confirm` so that a tree naming the wrong block fails loudly.
///
/// # Safety
/// `gicd` must be a mapped device-memory address with at least 64 KiB behind it.
pub(crate) unsafe fn identification(gicd: u64) -> u32 {
    // SAFETY: per this function's contract.
    unsafe { Frame::new(gicd) }.read(PIDR2)
}

/// Bring the distributor up: every SPI Group 1, disabled, inactive, at the default priority, then
/// affinity routing and both non-secure groups on. The boot core does this once; the private half
/// is per core ([`init_this_cpu`]). Linux's `gic_dist_init` (v6.16) minus the extended SPI range.
/// SPIs are routed when they are enabled, not here.
///
/// # Safety
/// `distributor` must name a mapped GICv3 distributor (64 KiB) and `redistributors` and
/// `redistributors_size` its mapped redistributor region, both device memory, both exclusively ours.
pub unsafe fn init(distributor: u64, redistributors: u64, redistributors_size: u64) {
    DISTRIBUTOR.store(distributor, Ordering::Relaxed);
    REDISTRIBUTOR_REGION[0].store(redistributors, Ordering::Relaxed);
    REDISTRIBUTOR_REGION[1].store(redistributors_size, Ordering::Relaxed);
    let d = gicd();

    // Off, and wait for it to be off, before touching the configuration beneath it.
    d.write(GICD_CTLR, 0u32);
    wait_for_rwp(|| d.read(GICD_CTLR), GICD_CTLR_RWP, "distributor disable");

    // Capped at 1020: INTIDs 1020-1023 are special.
    let lines = (32 * ((d.read::<u32>(GICD_TYPER) & GICD_TYPER_IT_LINES) + 1)).min(1020) as usize;
    for reg in 1..lines / 32 {
        d.write(GICD_IGROUPR + 4 * reg, !0u32);
        d.write(GICD_ICACTIVER + 4 * reg, !0u32);
        d.write(GICD_ICENABLER + 4 * reg, !0u32);
    }
    for intid in 32..lines {
        d.write(GICD_IPRIORITYR + intid, DEFAULT_PRIORITY);
    }

    d.write(
        GICD_CTLR,
        GICD_CTLR_ARE_NS | GICD_CTLR_ENABLE_G1A | GICD_CTLR_ENABLE_G1,
    );
    wait_for_rwp(|| d.read(GICD_CTLR), GICD_CTLR_RWP, "distributor enable");
}

/// Find, wake and configure this core's redistributor, and remember it under `slot`.
///
/// `affinity` is this core's `MPIDR_EL1` affinity packed the way `GICR_TYPER[63:32]` states it:
/// Aff3 in bits 31:24, Aff2 23:16, Aff1 15:8, Aff0 7:0. The adapter reads `MPIDR_EL1` (a system
/// register, so architecture code) and packs it; this driver only compares.
///
/// Panics, naming the affinity, if no frame in the region claims this core, which would otherwise
/// take no timer and no SGI.
pub(crate) fn init_this_cpu(slot: usize, affinity: u32) {
    let base = REDISTRIBUTOR_REGION[0].load(Ordering::Relaxed);
    let size = REDISTRIBUTOR_REGION[1].load(Ordering::Relaxed);
    assert!(base != 0, "gicv3::init_this_cpu before gicv3::init");

    let mut at = base;
    let found = loop {
        if at + 2 * FRAME > base + size {
            break None;
        }
        // SAFETY: `at` is inside the region `init`'s caller promised is mapped (checked above).
        let frame = unsafe { Frame::new(at) };
        let arch_rev = (frame.read::<u32>(PIDR2) >> 4) & 0xf;
        if !(3..=4).contains(&arch_rev) {
            break None; // not a redistributor: Linux's "we're in trouble" check
        }
        let typer: u64 = frame.read(GICR_TYPER);
        if (typer >> 32) as u32 == affinity {
            break Some(at);
        }
        if typer & GICR_TYPER_LAST != 0 {
            break None;
        }
        at += if typer & GICR_TYPER_VLPIS != 0 {
            4 * FRAME
        } else {
            2 * FRAME
        };
    };
    let Some(frame) = found else {
        panic!(
            "gicv3: no redistributor in {base:#x}+{size:#x} claims affinity {affinity:#010x}; this \
             core would take no interrupts"
        );
    };
    REDISTRIBUTOR[slot].store(frame, Ordering::Relaxed);
    let (ctl, private) = rd(slot);

    // Wake it: clear ProcessorSleep, then wait for ChildrenAsleep to clear (Linux
    // `gic_enable_redist`). Until then the redistributor forwards nothing to this core.
    ctl.write(GICR_WAKER, ctl.read::<u32>(GICR_WAKER) & !GICR_WAKER_PROCESSOR_SLEEP);
    wait_for_rwp(
        || ctl.read(GICR_WAKER),
        GICR_WAKER_CHILDREN_ASLEEP,
        "redistributor wake",
    );

    // The private INTIDs: all Group 1, all inactive, PPIs disabled, SGIs enabled and never
    // disabled, as GICv2 forces them (`hw/intc/arm_gic.c`, the GIC-400); the Irq capability's SGI
    // tests mask and unmask on different cores.
    private.write(GICR_IGROUPR0, !0u32);
    private.write(GICR_ICACTIVER0, !0u32);
    private.write(GICR_ICENABLER0, 0xffff_0000u32);
    for intid in 0..32 {
        let priority = if intid < 16 {
            ENABLED_PRIORITY
        } else {
            DEFAULT_PRIORITY
        };
        private.write(GICR_IPRIORITYR + intid, priority);
    }
    private.write(GICR_ISENABLER0, 0x0000_ffffu32);
    wait_for_rwp(|| ctl.read(GICR_CTLR), GICR_CTLR_RWP, "redistributor setup");
}

/// Enable `intid`. An SPI (32-1019) is routed to `route`, an `IROUTER` value (the target core's
/// `MPIDR_EL1` affinity fields, `IRM` clear), at the distributor. A PPI (16-31) is enabled in
/// `slot`'s redistributor, which must be the calling core's, as GICv2's banked write was. An SGI
/// is already enabled and stays so.
pub fn enable(slot: usize, intid: u32, route: u64) {
    assert!(intid < SPECIAL_FIRST, "gicv3: INTID {intid} is not a line");
    let bit: u32 = 1 << (intid % 32);
    if intid < 16 {
        return;
    }
    if intid < 32 {
        let (_, private) = rd(slot);
        private.write(GICR_IPRIORITYR + intid as usize, ENABLED_PRIORITY);
        private.write(GICR_ISENABLER0, bit);
        return;
    }
    let d = gicd();
    d.write(GICD_IPRIORITYR + intid as usize, ENABLED_PRIORITY);
    d.write(GICD_IROUTER + 8 * intid as usize, route);
    d.write(GICD_ISENABLER + 4 * (intid / 32) as usize, bit);
}

/// Disable `intid`, and wait until the disable has taken effect (`RWP`), so a level-triggered line
/// masked by the IRQ handler cannot fire again after the handler's EOI. PPIs are disabled in
/// `slot`'s redistributor, which must be the calling core's.
///
/// An SGI is never disabled, which is the GICv2 behaviour the tree depends on (see
/// [`init_this_cpu`]). An SGI is an edge a core raises on purpose; it cannot storm, so the reason
/// the handler masks a line does not apply to it.
pub(crate) fn disable(slot: usize, intid: u32) {
    let bit: u32 = 1 << (intid % 32);
    if intid < 16 {
        return;
    }
    if intid < 32 {
        let (ctl, private) = rd(slot);
        private.write(GICR_ICENABLER0, bit);
        wait_for_rwp(|| ctl.read(GICR_CTLR), GICR_CTLR_RWP, "redistributor disable");
        return;
    }
    let d = gicd();
    d.write(GICD_ICENABLER + 4 * (intid / 32) as usize, bit);
    wait_for_rwp(
        || d.read(GICD_CTLR),
        GICD_CTLR_RWP,
        "distributor disable of a line",
    );
}
