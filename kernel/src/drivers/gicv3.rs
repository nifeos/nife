//! GICv3: the memory-mapped half, the distributor and one redistributor per core.
//!
//! The CPU interface (`ICC_*` system registers) is `arch/aarch64/gic_cpu_interface.rs`, because
//! `msr`/`mrs` are architecture code; `arch::irq` holds both halves and picks the GIC version. Each
//! core finds its redistributor by walking the frames for its own affinity, every interrupt is
//! Group 1, and writes wait on `RWP` where Linux waits. See notes/interrupts/controller-drivers.md.
//!

//! # BUGS
//!
//! - No ITS, so no LPIs or message-signalled interrupts (milestone 317 wants them).
//! - No extended SPI or PPI ranges; [`enable`] refuses an INTID past 1019.
//! - No redistributor power-down; nothing in this kernel powers a core off.
//! - The frame stride is inferred from `VLPIS`. A board that pads its frames and states a
//!   `redistributor-stride` would be misread, and the walk would fail loudly.
//!
//! Name: provisional (`gicv3` for the module), and a lane's to propose: the version suffix is the
//! vocabulary Arm's own documents and Linux's `irq-gic-v3.c` use for exactly this split.

use core::sync::atomic::{AtomicU64, Ordering};

use tock_registers::interfaces::{Readable, Writeable};
use tock_registers::register_structs;
use tock_registers::registers::{ReadOnly, ReadWrite, WriteOnly};

register_structs! {
    /// The distributor. One per machine. Offsets from Arm IHI 0069, table 12-24.
    #[allow(non_snake_case)]
    pub Distributor {
        (0x0000 => CTLR: ReadWrite<u32>),
        (0x0004 => TYPER: ReadOnly<u32>),
        (0x0008 => _reserved0),
        /// Group, one BIT per interrupt. Ones mean Group 1, the only group this kernel takes.
        (0x0080 => IGROUPR: [ReadWrite<u32>; 32]),
        (0x0100 => ISENABLER: [WriteOnly<u32>; 32]),
        (0x0180 => ICENABLER: [WriteOnly<u32>; 32]),
        (0x0200 => _reserved1),
        (0x0380 => ICACTIVER: [WriteOnly<u32>; 32]),
        /// Priority, one BYTE per interrupt, as on GICv2.
        (0x0400 => IPRIORITYR: [ReadWrite<u8>; 1024]),
        (0x0800 => _reserved2),
        /// **Routing, one 64-bit AFFINITY per SPI** (INTIDs 32-1019; entries 0-31 are reserved).
        /// Replaces GICv2's `ITARGETSR` once `CTLR.ARE` is set. Layout is `MPIDR_EL1`'s: Aff0 in
        /// bits 7:0, Aff1 15:8, Aff2 23:16, Aff3 39:32, and bit 31 (`IRM`) clear for "this core".
        (0x6000 => IROUTER: [ReadWrite<u64>; 1024]),
        (0x8000 => _reserved3),
        /// Peripheral ID 2. `ArchRev` in bits 7:4 is 3 on a GICv3 and 4 on a GICv4.
        (0xffe8 => PIDR2: ReadOnly<u32>),
        (0xffec => _reserved4),
        (0x10000 => @END),
    }
}

register_structs! {
    /// A redistributor's first frame, `RD_base`: control, identity and power.
    #[allow(non_snake_case)]
    pub RedistributorControl {
        (0x0000 => CTLR: ReadWrite<u32>),
        (0x0004 => IIDR: ReadOnly<u32>),
        /// Bits 63:32: this frame's core, as Aff3.Aff2.Aff1.Aff0, one byte each. Bit 4: last frame
        /// in the region. Bit 1: virtual LPIs, which doubles the frame stride.
        (0x0008 => TYPER: ReadOnly<u64>),
        (0x0010 => _reserved0),
        /// Bit 1 `ProcessorSleep`, bit 2 `ChildrenAsleep`. Out of reset a redistributor is asleep
        /// and delivers nothing to its core, so waking it is the first thing a core does.
        (0x0014 => WAKER: ReadWrite<u32>),
        (0x0018 => _reserved1),
        (0xffe8 => PIDR2: ReadOnly<u32>),
        (0xffec => _reserved2),
        (0x10000 => @END),
    }
}

register_structs! {
    /// A redistributor's second frame, `SGI_base`: the GICv2 distributor's banked registers for
    /// INTIDs 0-31, moved out to live beside the core they belong to.
    #[allow(non_snake_case)]
    pub RedistributorPrivate {
        (0x0000 => _reserved0),
        (0x0080 => IGROUPR0: ReadWrite<u32>),
        (0x0084 => _reserved1),
        (0x0100 => ISENABLER0: WriteOnly<u32>),
        (0x0104 => _reserved2),
        (0x0180 => ICENABLER0: WriteOnly<u32>),
        (0x0184 => _reserved3),
        (0x0380 => ICACTIVER0: WriteOnly<u32>),
        (0x0384 => _reserved4),
        (0x0400 => IPRIORITYR: [ReadWrite<u8>; 32]),
        (0x0420 => _reserved5),
        (0x10000 => @END),
    }
}

const GICD_CTLR_ENABLE_G1: u32 = 1 << 0; // EnableGrp1 (non-secure view) or EnableGrp0 (DS=1)
const GICD_CTLR_ENABLE_G1A: u32 = 1 << 1; // EnableGrp1A (non-secure view) or EnableGrp1 (DS=1)
const GICD_CTLR_ARE_NS: u32 = 1 << 4; // affinity routing, non-secure
const GICD_CTLR_RWP: u32 = 1 << 31;
const GICR_CTLR_RWP: u32 = 1 << 3;
const GICR_WAKER_PROCESSOR_SLEEP: u32 = 1 << 1;
const GICR_WAKER_CHILDREN_ASLEEP: u32 = 1 << 2;
const GICR_TYPER_VLPIS: u64 = 1 << 1;
const GICR_TYPER_LAST: u64 = 1 << 4;
const FRAME: u64 = 0x1_0000;

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

fn gicd() -> &'static Distributor {
    let base = DISTRIBUTOR.load(Ordering::Relaxed);
    assert!(base != 0, "gicv3 used before gicv3::init");
    // SAFETY: `init`'s caller promised a mapped GICv3 distributor at this address, and it is never
    // unmapped.
    unsafe { &*(base as *const Distributor) }
}

fn rd(slot: usize) -> (&'static RedistributorControl, &'static RedistributorPrivate) {
    let base = REDISTRIBUTOR[slot].load(Ordering::Relaxed);
    assert!(
        base != 0,
        "gicv3: slot {slot} has no redistributor; init_this_cpu has not run on that core"
    );
    // SAFETY: `init_this_cpu` stored a frame it found inside the region `init`'s caller promised is
    // mapped, and whose own GICR_TYPER named this core.
    unsafe {
        (
            &*(base as *const RedistributorControl),
            &*((base + FRAME) as *const RedistributorPrivate),
        )
    }
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
    unsafe { (*(gicd as *const Distributor)).PIDR2.get() }
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
    d.CTLR.set(0);
    wait_for_rwp(|| d.CTLR.get(), GICD_CTLR_RWP, "distributor disable");

    // ITLinesNumber: the distributor implements 32 * (N + 1) INTIDs, capped at 1020.
    let lines = (32 * ((d.TYPER.get() & 0x1f) + 1)).min(1020) as usize;
    for reg in 1..lines / 32 {
        d.IGROUPR[reg].set(!0);
        d.ICACTIVER[reg].set(!0);
        d.ICENABLER[reg].set(!0);
    }
    for priority in &d.IPRIORITYR[32..lines] {
        priority.set(DEFAULT_PRIORITY);
    }

    d.CTLR
        .set(GICD_CTLR_ARE_NS | GICD_CTLR_ENABLE_G1A | GICD_CTLR_ENABLE_G1);
    wait_for_rwp(|| d.CTLR.get(), GICD_CTLR_RWP, "distributor enable");
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
        let frame = unsafe { &*(at as *const RedistributorControl) };
        let arch_rev = (frame.PIDR2.get() >> 4) & 0xf;
        if !(3..=4).contains(&arch_rev) {
            break None; // not a redistributor: Linux's "we're in trouble" check
        }
        let typer = frame.TYPER.get();
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
    ctl.WAKER.set(ctl.WAKER.get() & !GICR_WAKER_PROCESSOR_SLEEP);
    wait_for_rwp(
        || ctl.WAKER.get(),
        GICR_WAKER_CHILDREN_ASLEEP,
        "redistributor wake",
    );

    // The private INTIDs: all Group 1, all inactive, PPIs disabled, SGIs enabled and never
    // disabled, as GICv2 forces them (`hw/intc/arm_gic.c`, the GIC-400); the Irq capability's SGI
    // tests mask and unmask on different cores.
    private.IGROUPR0.set(!0);
    private.ICACTIVER0.set(!0);
    private.ICENABLER0.set(0xffff_0000);
    for (intid, priority) in private.IPRIORITYR.iter().enumerate() {
        priority.set(if intid < 16 {
            ENABLED_PRIORITY
        } else {
            DEFAULT_PRIORITY
        });
    }
    private.ISENABLER0.set(0x0000_ffff);
    wait_for_rwp(|| ctl.CTLR.get(), GICR_CTLR_RWP, "redistributor setup");
}

/// Enable `intid`. An SPI (32-1019) is routed to `route`, an `IROUTER` value (the target core's
/// `MPIDR_EL1` affinity fields, `IRM` clear), at the distributor. A PPI (16-31) is enabled in
/// `slot`'s redistributor, which must be the calling core's, as GICv2's banked write was. An SGI
/// is already enabled and stays so.
pub fn enable(slot: usize, intid: u32, route: u64) {
    assert!(intid < SPECIAL_FIRST, "gicv3: INTID {intid} is not a line");
    let bit = 1 << (intid % 32);
    if intid < 16 {
        return;
    }
    if intid < 32 {
        let (_, private) = rd(slot);
        private.IPRIORITYR[intid as usize].set(ENABLED_PRIORITY);
        private.ISENABLER0.set(bit);
        return;
    }
    let d = gicd();
    d.IPRIORITYR[intid as usize].set(ENABLED_PRIORITY);
    d.IROUTER[intid as usize].set(route);
    d.ISENABLER[(intid / 32) as usize].set(bit);
}

/// Disable `intid`, and wait until the disable has taken effect (`RWP`), so a level-triggered line
/// masked by the IRQ handler cannot fire again after the handler's EOI. PPIs are disabled in
/// `slot`'s redistributor, which must be the calling core's.
///
/// An SGI is never disabled, which is the GICv2 behaviour the tree depends on (see
/// [`init_this_cpu`]). An SGI is an edge a core raises on purpose; it cannot storm, so the reason
/// the handler masks a line does not apply to it.
pub(crate) fn disable(slot: usize, intid: u32) {
    let bit = 1 << (intid % 32);
    if intid < 16 {
        return;
    }
    if intid < 32 {
        let (ctl, private) = rd(slot);
        private.ICENABLER0.set(bit);
        wait_for_rwp(|| ctl.CTLR.get(), GICR_CTLR_RWP, "redistributor disable");
        return;
    }
    let d = gicd();
    d.ICENABLER[(intid / 32) as usize].set(bit);
    wait_for_rwp(
        || d.CTLR.get(),
        GICD_CTLR_RWP,
        "distributor disable of a line",
    );
}
