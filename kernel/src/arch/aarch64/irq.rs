//! The interrupt controller, aarch64: the GIC.
//!
//! This is the arch side of the portable `arch::irq` surface. Portable code (the `Irq` capability's
//! ACK, and from here the interrupt setup) names `arch::irq`, never `drivers::gic` directly, so the
//! driver name stays inside `arch/` where rule #1 (DECISIONS §4) puts it. It is a thin adapter: the
//! GIC does the work, this only gives it an architecture-neutral name. The RISC-V twin is the PLIC.
//!
//! # Two GICs, and this is where the choice is made (milestone 227)
//!
//! A GICv2 is one memory-mapped driver (`drivers/gic.rs`). A GICv3 is two halves: the distributor
//! and redistributors are memory (`drivers/gicv3.rs`), and the CPU interface is system registers
//! (`gic_cpu_interface.rs`, beside this file, because `msr`/`mrs` are architecture code). This file
//! is the one place that holds both versions, so every caller in the kernel (the exception
//! handler, the timer, the scheduler's SGIs, the tests) names `arch::irq` and never a driver.
//!
//! [`init`] decides the version once, on the boot core, from the device tree's `compatible`
//! (`machine_discovery::gic::discover`), then **asks the hardware whether the tree is right**
//! (`Gic::confirm`) before a single configuration write. A mismatch is a panic that names both
//! sides. That is the gate milestone 227's block asked for: the failure milestone 222 measured,
//! a kernel that boots, prints `interrupts ON` and takes nothing, is now a boot that stops and
//! says why.
//!
//! The version is one relaxed byte read per call. It is written before any interrupt is unmasked
//! and never again, so a relaxed load is enough and costs the IRQ path a load and a branch.

use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

use machine_discovery::gic::Gic;

use super::gic_cpu_interface;
use crate::drivers::{gic, gicv3};

/// Which GIC this machine has: 0 before [`init`], then 2 or 3.
static VERSION: AtomicU8 = AtomicU8::new(0);

/// The GICv3 driver keeps one redistributor per slot, and the slot is the logical core id, so the
/// kernel's core count must fit its table. Rung one of the ladder: a mismatch is a build failure.
const _: () = assert!(crate::cpu::MAX_CPUS <= gicv3::SLOTS);

/// A version-3 machine? One byte, read on every dispatch below.
#[inline(always)]
fn is_v3() -> bool {
    VERSION.load(Ordering::Relaxed) == 3
}

/// **The interrupt returned by [`acknowledge`] when there is nothing to take.** Do nothing with
/// it, and in particular do not call [`end_of_interrupt`].
pub(crate) const SPURIOUS: u32 = gic::SPURIOUS;

/// A core's `MPIDR_EL1` affinity, from the roster `smp.rs` read out of the device tree. The logical
/// id IS the Aff0 value on every machine this kernel seats (`smp::read_cpu_list` seats by hardware
/// id and refuses Aff1-bearing cores), so the fallback is exact rather than a guess, and it covers
/// the boot core's first calls, which run before the roster exists.
fn affinity_of(cpu: usize) -> u64 {
    crate::smp::hwid(cpu).unwrap_or(cpu as u64)
}

/// Per-INTID target core, or [`UNASSIGNED`] until the first `enable` chooses one. A device line's
/// target must be **stable** across re-enables: the IRQ handler masks the line when it fires, and
/// the driver's ACK re-enables it (this file's `enable`), so if we re-rolled the target on every
/// ACK the line would hop cores on every completion. Assign once, then reuse.
const UNASSIGNED: u8 = 0xff;
static IRQ_TARGET: [AtomicU8; 256] = [const { AtomicU8::new(UNASSIGNED) }; 256];

/// Round-robin cursor for spreading SPI lines over the online cores. Consecutive device lines get
/// consecutive cores regardless of their INTID values, which `intid % ncpus` would not guarantee
/// (three virtio devices at INTIDs 48, 52, 56 would all land on core 0 under mod-4).
static NEXT_IRQ_CPU: AtomicUsize = AtomicUsize::new(0);

/// The core an SPI should target: its stable assignment if it has one, otherwise the next core in
/// the round-robin, recorded so every later re-enable of this line reuses it. Bounded to the cores
/// that are actually online (each has its GIC CPU interface up), so we never target a core that
/// would never take the interrupt.
fn target_cpu(intid: u32) -> usize {
    let slot = &IRQ_TARGET[intid as usize];
    let existing = slot.load(Ordering::Relaxed);
    if existing != UNASSIGNED {
        return existing as usize;
    }
    // The k-th ONLINE core, not index k (first-silicon sweep, 2026-08-14): `cursor % count` as an
    // index routes device lines to parked cores (where they sit pending forever) and never to the
    // online cores past the count, when the online set is not contiguous from zero.
    let chosen = crate::smp::nth_online(NEXT_IRQ_CPU.fetch_add(1, Ordering::Relaxed));
    // First writer wins, so a racing second enable of the same line agrees on the target.
    match slot.compare_exchange(
        UNASSIGNED,
        chosen as u8,
        Ordering::Relaxed,
        Ordering::Relaxed,
    ) {
        Ok(_) => chosen,
        Err(other) => other as usize,
    }
}

/// Re-enable (unmask) an interrupt source at the controller.
///
/// Called by the `Irq` capability's ACK, after a userspace driver has serviced its device: the IRQ
/// handler masked the source when it fired (so a level-triggered device would not re-fire in a storm
/// before the driver quieted it), and this brings it back. seL4's `IRQHandler` protocol, the aarch64
/// half.
///
/// SPI lines are spread across the online cores (see [`target_cpu`]) so a device's completion
/// interrupt, and the driver wake `handle_irq` turns it into, lands on the chosen core instead of
/// funnelling every device onto core 0. PPIs and SGIs are per-core; the target is ignored for them.
///
/// **An SGI or a PPI (INTID below 32) is enabled on the calling core** and takes no target: they are
/// per-core by definition, so no round-robin slot is spent on them. This is the call the timer
/// makes on each core, and the one a test makes to arm an SGI.
pub fn enable(intid: u32) {
    let target = if intid < 32 { 0 } else { target_cpu(intid) };
    if is_v3() {
        gicv3::enable(
            crate::cpu::id(),
            intid,
            affinity_of(target) & 0xff_00ff_ffff,
        );
    } else {
        gic::enable(intid, target);
    }
}

/// Mask an interrupt source at the controller: the IRQ handler's first act for a line routed to a
/// userspace driver, undone by [`enable`] when the driver ACKs. A PPI is masked on the calling core.
pub(crate) fn disable(intid: u32) {
    if is_v3() {
        gicv3::disable(crate::cpu::id(), intid);
    } else {
        gic::disable(intid);
    }
}

/// **Take the interrupt.** Acknowledging has a side effect, so exactly once per IRQ. Returns
/// [`SPURIOUS`] when there is nothing to take; on a GICv3 all four special INTIDs (1020-1023) are
/// folded into it, because none of them is an interrupt this kernel can complete.
#[inline(always)]
pub(crate) fn acknowledge() -> u32 {
    if is_v3() {
        let intid = gic_cpu_interface::acknowledge();
        if intid >= gicv3::SPECIAL_FIRST {
            SPURIOUS
        } else {
            intid
        }
    } else {
        gic::acknowledge()
    }
}

/// "Finished with this one." Until this is written the controller delivers nothing of equal or
/// lower priority to this core.
#[inline(always)]
pub(crate) fn end_of_interrupt(intid: u32) {
    if is_v3() {
        gic_cpu_interface::end_of_interrupt(intid);
    } else {
        gic::end_of_interrupt(intid);
    }
}

/// Raise software-generated interrupt `intid` (0-15) on `target_cpu`. The reschedule IPI is one
/// ([`send_reschedule`]); the tests raise others to prove an interrupt becomes a message.
pub fn send_sgi(intid: u32, target_cpu: usize) {
    if is_v3() {
        gic_cpu_interface::send_sgi(intid, affinity_of(target_cpu));
    } else {
        gic::send_sgi(intid, target_cpu);
    }
}

/// **Reserve a vector a PCI function may deliver an MSI-X message on**, the arch contract's
/// milestone-215 name. `None` here, and the `None` is a statement about this board rather than a
/// hole: a PCI function on QEMU's `virt` board raises its interrupt as **INTx**, through the
/// swizzle (`pci::intx_irq`) onto GIC input `mmu::PCI_IRQ_BASE` (SPIs 35..38), which the machine's own device tree
/// states and `crates/pci`'s fixture test holds the formula against. `kernel/src/pci.rs` asks this
/// first and falls back to that; the fallback is what runs here.
///
/// **It is not "MSI is unsupported on aarch64"**, which would be false: a `GICv3` ITS is exactly
/// this mechanism under another name, and a board with one would implement it here. It is that
/// nothing on this board needs it, and a routing path with no consumer is machinery to maintain
/// with no way to know it works. See design/roadmap/0215-x86-64-pci-interrupt-routing.md for why
/// `x86_64` answers differently.
pub(crate) fn alloc_msi_vector() -> Option<(u32, pci::MsiTarget)> {
    None
}

/// **How many interrupts a device line delivered at bring-up**, which on this port is none: no
/// device here can be made to interrupt without input or an attached disk, so bring-up proves no
/// device line through the GIC, and `self_test`'s `deviceirq` check reports itself absent. `x86_64`
/// proves the PIT's. A device line on this port is proved where one exists: the user-level input
/// driver at the hand-over, and the `blockdev` check's driver when a disk is attached.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)] // self_test reads it
pub(crate) fn device_line_interrupts() -> Option<u64> {
    None
}

/// Bring the interrupt controller up on the boot core. Called once, from the shared
/// `interrupts_init`.
///
/// Three steps, and the middle one is milestone 227's gate. The device tree says which GIC this is
/// (`memory::gic_regions`, read by binding). The hardware is then asked whether that is true
/// (`Gic::confirm`, on the distributor's own revision for a GICv3 and the CPU interface's for a
/// GICv2), and **a disagreement panics, naming both sides**, before any configuration write lands
/// anywhere. Only then is the matching driver brought up; a GICv3's CPU interface then proves
/// itself a second way, by `ICC_SRE_EL1.SRE` sticking (`gic_cpu_interface::init_this_cpu`).
///
/// # Panics
/// With no GIC in the tree, or with one the hardware contradicts.
pub fn init() {
    let found = crate::memory::gic_regions().expect("no interrupt controller in the DTB");
    let gicd = crate::arch::mmu::phys_to_virt(found.distributor().start);
    let second = found.second_region();
    let second_virt = crate::arch::mmu::phys_to_virt(second.start);

    // The addresses came from the device tree, and `mmu::init` mapped both blocks as DEVICE memory
    // at their full device-tree length. Mapping them as normal memory would let the CPU cache and
    // reorder writes to an interrupt controller, which is exactly as bad as it sounds.
    let identification = match found {
        // SAFETY: a mapped block at least as large as a GICv2 CPU interface, per the tree.
        Gic::V2 { .. } => unsafe { gic::identification(second_virt) },
        // SAFETY: a mapped 64 KiB distributor, per the tree.
        Gic::V3 { .. } => unsafe { gicv3::identification(gicd) },
    };
    if let Err(mismatch) = found.confirm(identification) {
        panic!(
            "the device tree names a GICv{} and the hardware reports revision {} (identification \
             register {identification:#x}). Driving it anyway is how a kernel boots and takes no \
             interrupts (milestone 222); stopping instead",
            mismatch.claimed, mismatch.field,
        );
    }

    match found {
        Gic::V2 { .. } => {
            VERSION.store(2, Ordering::Relaxed);
            // SAFETY: the tree named a GICv2 here, the hardware confirmed it, and both blocks are
            // mapped device memory that nothing else drives.
            unsafe { gic::init(gicd, second_virt) };
        }
        Gic::V3 { .. } => {
            VERSION.store(3, Ordering::Relaxed);
            // SAFETY: the tree named a GICv3 here, its distributor confirmed it, and the distributor
            // and the whole redistributor region are mapped device memory that nothing else drives.
            unsafe { gicv3::init(gicd, second_virt, second.size) };
            init_this_cpu();
        }
    }
}

/// Per-core interrupt-controller setup, run by each core as it comes online. On a GICv2 the CPU
/// interface is banked MMIO; on a GICv3 the core finds and wakes its own redistributor, then turns on
/// its system-register CPU interface. Either way every core enables its own.
pub(crate) fn init_this_cpu() {
    if is_v3() {
        gicv3::init_this_cpu(
            crate::cpu::id(),
            gic_cpu_interface::redistributor_affinity(),
        );
        gic_cpu_interface::init_this_cpu();
    } else {
        gic::init_this_cpu();
    }
}

/// Send a reschedule inter-processor interrupt to `target_cpu`. On aarch64 this is a software-
/// generated interrupt (SGI) on the reschedule vector; the target core's handler drains its inbox
/// and reschedules. See sched.rs and DECISIONS §11 (SMP).
pub(crate) fn send_reschedule(target_cpu: usize) {
    send_sgi(crate::sched::RESCHED_SGI, target_cpu);
}

/// **This machine's interrupt controller, for the machine description** (milestone 268).
///
/// One of the eight questions the description answers on every architecture, in this
/// architecture's own vocabulary. The version is the one the device tree's binding named and
/// `init` confirmed against the hardware; until milestone 227 this line printed the literal
/// "GICv2" whatever the machine was, and on a GICv3 it printed the redistributor's address under
/// the heading "cpu interface" (milestone 317 recorded it). A GICv3 has no CPU interface address to
/// print: it is system registers.
// The machine description and the boot self-test are the only callers, and both are
// `#[cfg(not(any(test, feature = "bench")))]`: a test boot exits through semihosting and a bench
// boot diverges into `bench::run`, so neither reads a bring-up transcript. Same treatment
// `memory::print_summary` already carries, and for the same reason.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub(crate) fn print_summary() {
    match crate::memory::gic_regions() {
        Some(Gic::V2 {
            distributor,
            cpu_interface,
        }) => crate::println!(
            "  interrupts      : GICv2, distributor {:#018x}, cpu interface {:#018x}",
            distributor.start,
            cpu_interface.start,
        ),
        Some(Gic::V3 {
            distributor,
            redistributors,
        }) => crate::println!(
            "  interrupts      : GICv3, distributor {:#018x}, redistributors {:#018x}, cpu \
             interface in system registers",
            distributor.start,
            redistributors.start,
        ),
        None => {
            crate::println!("  interrupts      : none (this machine's device tree names no GIC)");
        }
    }
}

#[cfg(test)]
mod tests {
    //! Milestone 227's two claims about the interrupt controller, on whichever GIC the runner gave
    //! this boot. The suite runs them on a GICv2 by default and on a GICv3 with `NIFE_GIC=3` or
    //! under HVF, so the same assertions hold both drivers to the same contract.

    use core::sync::atomic::Ordering;

    use crate::arch::timer;
    use crate::cpu::MAX_CPUS;

    /// **Every online core takes its own timer interrupts.**
    ///
    /// The failure milestone 222 measured is the reason this exists: a GICv3 driven as a GICv2
    /// booted, printed `interrupts ON`, brought four cores up and then took **zero** interrupts,
    /// with nothing faulting. `timer::tests::the_timer_is_ticking` watches only the core the test
    /// runs on; a GICv3 redistributor is per core, and a core that found the wrong frame, or none,
    /// would lose its ticks while its neighbours kept theirs. So every online core is watched.
    ///
    /// The bound is a liveness bound, not a rate: five seconds for a 100 Hz tick to move at all.
    /// A host that stalls the emulator for five whole seconds could fail it, and would say so in
    /// the host-load line the harness prints; the shape it exists to catch is "zero, forever".
    #[test_case]
    fn every_online_cpu_takes_its_own_timer_ticks() {
        let before: [u64; MAX_CPUS] = core::array::from_fn(timer::ticks_on);
        let deadline = timer::now() + timer::frequency() * 5;
        loop {
            let silent = crate::smp::online_cpus().find(|&c| timer::ticks_on(c) <= before[c]);
            let Some(cpu) = silent else {
                return;
            };
            assert!(
                timer::now() < deadline,
                "core {cpu} took no timer interrupt in 5 s on a GICv{}: its CPU interface or \
                 (on a GICv3) its redistributor is not delivering",
                super::VERSION.load(Ordering::Relaxed),
            );
            core::hint::spin_loop();
        }
    }

    /// **The driver this boot runs is the one the device tree names**, so a runner that asked for
    /// a GICv3 is not quietly measured on a GICv2 (or the reverse). `init` refuses a tree the
    /// hardware contradicts; this holds the dispatch byte to the tree, which nothing else checks.
    #[test_case]
    fn the_driven_gic_is_the_one_the_tree_names() {
        let tree = crate::memory::gic_regions().expect("an aarch64 boot with no GIC in its tree");
        assert_eq!(super::VERSION.load(Ordering::Relaxed), tree.version());
    }
}
