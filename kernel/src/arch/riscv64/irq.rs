//! The interrupt controller, RISC-V: the PLIC.
//!
//! The arch side of the portable `arch::irq` surface, the twin of aarch64's GIC adapter, resolving
//! to `drivers::plic` so the driver name stays inside `arch/` (DECISIONS §4 (kernel shape:
//! monolithic, deferred)). See notes/riscv-port/traps.md.

use core::sync::atomic::{AtomicU16, AtomicUsize, Ordering};

/// Each hart's S-mode PLIC context, read from the device tree's `interrupts-extended` by
/// [`init_contexts`], indexed by hart id. [`CTX_UNKNOWN`] where the tree did not say, in which
/// case [`s_context_of`] falls back to the `2*hart + 1` formula.
///
/// A table, because the formula is QEMU's layout: on the JH7110 hart h's S context is `2h`. See
/// notes/riscv-port/traps.md.
static S_CTX: [AtomicUsize; machine_discovery::plic::MAX_CONTEXT_HARTS] =
    [const { AtomicUsize::new(CTX_UNKNOWN) }; machine_discovery::plic::MAX_CONTEXT_HARTS];
const CTX_UNKNOWN: usize = usize::MAX;

/// Record the PLIC context layout from the device tree at physical `dtb_ptr`, once, on the boot
/// hart, before anything touches a context.
///
/// A tree that does not state it leaves the formula in force. Prints only when the layout differs
/// from the formula.
pub(crate) fn init_contexts(dtb_ptr: usize) {
    // SAFETY: the pointer firmware handed us, already parsed by memory::init on this boot, named
    // through the direct map.
    let Ok(dt) = (unsafe {
        device_tree_blob::DeviceTreeBlob::from_ptr(
            super::mmu::phys_to_virt(dtb_ptr as u64) as *const u8
        )
    }) else {
        return;
    };
    let Ok(ctx) = machine_discovery::plic::PlicContexts::from_device_tree(&dt) else {
        return;
    };
    let mut differs = false;
    for (hart, slot) in S_CTX.iter().enumerate() {
        if let Some(c) = ctx.s_context(hart) {
            slot.store(c, Ordering::Relaxed);
            differs |= c != 2 * hart + 1;
        }
    }
    if differs {
        crate::println!(
            "  plic: S contexts from the device tree differ from the 2h+1 formula (expected on \
             the JH7110: the disabled S7 shifts every context down one)"
        );
    }
}

/// Hart `hart`'s S-mode context: the device tree's answer where it gave one, the QEMU `virt`
/// formula where it did not (a hart past the table, or a tree that never said).
fn s_context_of(hart: usize) -> usize {
    match S_CTX.get(hart).map(|s| s.load(Ordering::Relaxed)) {
        Some(c) if c != CTX_UNKNOWN => c,
        _ => 2 * hart + 1,
    }
}

/// This hart's S-mode PLIC context, which the external-interrupt handler claims against. The
/// logical cpu id is the hart id, so it is derived from [`S_CTX`].
pub(crate) fn this_s_context() -> usize {
    s_context_of(crate::cpu::id())
}

/// Each source's target PLIC context, or [`CTX_UNASSIGNED`] until the first `enable` chooses one.
/// Assigned once and stable, so a claim and its completion land on one context. Indexed by source
/// number; QEMU `virt`'s are small (the UART is 10, virtio-mmio 1..8).
const CTX_UNASSIGNED: u16 = u16::MAX;
const MAX_SOURCES: usize = 128;
static SOURCE_CTX: [AtomicU16; MAX_SOURCES] =
    [const { AtomicU16::new(CTX_UNASSIGNED) }; MAX_SOURCES];

/// Round-robin cursor spreading device sources over the online harts, the twin of the GIC's
/// `ITARGETSR` distribution.
static NEXT_IRQ_HART: AtomicUsize = AtomicUsize::new(0);

/// The PLIC context `source` targets: its stable assignment, or else the next online hart's S
/// context, opened and recorded. Only online harts, which set `SEIE`, are chosen.
fn target_context(source: u32) -> usize {
    let slot = &SOURCE_CTX[source as usize % MAX_SOURCES];
    let existing = slot.load(Ordering::Relaxed);
    if existing != CTX_UNASSIGNED {
        return existing as usize;
    }
    // The k-th online hart, not hart k: the VisionFive 2's online set is {1,2,3}.
    let hart = crate::smp::nth_online(NEXT_IRQ_HART.fetch_add(1, Ordering::Relaxed));
    let ctx = s_context_of(hart);
    // Open it before the first source lands. Threshold registers are global MMIO and opening is
    // idempotent, so any hart may do it and a race is harmless.
    crate::drivers::plic::open_context(ctx);
    // First writer wins, so a racing second enable of the same source agrees on the context.
    match slot.compare_exchange(
        CTX_UNASSIGNED,
        ctx as u16,
        Ordering::Relaxed,
        Ordering::Relaxed,
    ) {
        Ok(_) => ctx,
        Err(other) => other as usize,
    }
}

/// The context holding each source's outstanding claim, or [`CTX_UNASSIGNED`]. Set by
/// [`hold_claim`] and taken by [`enable`], which must complete against the context that claimed.
static CLAIM_CTX: [AtomicU16; MAX_SOURCES] =
    [const { AtomicU16::new(CTX_UNASSIGNED) }; MAX_SOURCES];

/// Record that `context` claimed `source` and that its driver's ACK owes the completion.
pub(crate) fn hold_claim(source: u32, context: usize) {
    CLAIM_CTX[source as usize % MAX_SOURCES].store(context as u16, Ordering::Release);
}

/// An `Irq` capability's ACK for `intid`, and its arming at setup.
///
/// With a claim outstanding, completes it on the claiming context, so the gateway forwards the next
/// interrupt. Otherwise enables the source at its stable target hart (see [`target_context`]).
pub fn enable(intid: u32) {
    match CLAIM_CTX[intid as usize % MAX_SOURCES].swap(CTX_UNASSIGNED, Ordering::Acquire) {
        CTX_UNASSIGNED => crate::drivers::plic::enable(intid, target_context(intid)),
        claimed => crate::drivers::plic::complete(intid, claimed as usize),
    }
}

/// Reserve a vector a PCI function may deliver an MSI-X message on: `None` on this architecture.
///
/// PCI functions here raise INTx onto PLIC inputs from `mmu::PCI_IRQ_BASE`, which
/// `kernel/src/pci.rs` falls back to. A board with the AIA's IMSIC would implement this. See
/// notes/riscv-port/traps.md.
pub(crate) fn alloc_msi_vector() -> Option<(u32, pci::MsiTarget)> {
    None
}

/// The PLIC context of the boot hart's S-mode, derived because OpenSBI elects the boot hart by
/// lottery; a fixed context leaves every interrupt pending when another hart wins.
pub(crate) fn boot_s_context() -> usize {
    s_context_of(super::boot_hartid())
}

/// How many interrupts a device line delivered at bring-up: `None`, because no device here
/// interrupts without input or a disk, so `self_test`'s `deviceirq` check reports itself absent.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)] // self_test reads it
pub(crate) fn device_line_interrupts() -> Option<u64> {
    None
}

/// The boot hart's half of the shared `interrupts_init`: bring the PLIC up on the boot hart's S
/// context, then unmask this hart.
///
/// The PLIC base is stored before `sie.SEIE` is set, so no claim goes through a zero base. Every
/// source stays disabled until [`enable`] routes it.
pub fn init() {
    if let Some((plic_phys, _)) = crate::memory::plic_region() {
        // SAFETY: `mmu::init` mapped the PLIC device-typed in the direct map, and this is its
        // virtual address there. The context is the boot hart's S context, derived rather than
        // hardcoded because OpenSBI elects the boot hart by lottery (see `boot_s_context`).
        unsafe {
            crate::drivers::plic::init(
                super::mmu::phys_to_virt(plic_phys) as usize,
                boot_s_context(),
            );
        }
    }
    init_this_cpu();
}

/// Unmask this hart's software interrupts (the reschedule IPI) and external interrupts, as it
/// joins the scheduler. The boot hart opens its context and routes sources later.
pub(crate) fn init_this_cpu() {
    super::exceptions::enable_software_interrupts();
    // Safe before any source is routed: SEIE with nothing enabled delivers nothing.
    super::exceptions::enable_external();
}

/// Send a reschedule IPI to `target_cpu`, whose hart id it is, through SBI, which pends the
/// target's `sip.SSIP`.
pub(crate) fn send_reschedule(target_cpu: usize) {
    crate::arch::sbi_send_ipi(target_cpu);
}

/// Print this machine's interrupt controller for the machine description, with this hart's S
/// context, the number that is not architectural.
// The machine description is the only caller, and test and bench boots do not print it.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub(crate) fn print_summary() {
    match crate::memory::plic_region() {
        Some((plic, _)) => crate::println!(
            "  interrupts      : PLIC at {plic:#018x}, this hart's S-mode context {}",
            this_s_context(),
        ),
        None => {
            crate::println!("  interrupts      : none (this machine's device tree names no PLIC)");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The live context table matches a fresh parse of the live tree for every online hart.
    ///
    /// The per-machine layouts are host-tested in
    /// `crates/machine_discovery/tests/riscv64_plic_contexts.rs`.
    #[test_case]
    fn the_context_table_is_the_machines_own_and_matches_the_formula_here() {
        let ptr = crate::DTB.load(core::sync::atomic::Ordering::Relaxed);
        // SAFETY: the pointer firmware handed us, already parsed several times on this boot.
        let dt = unsafe {
            device_tree_blob::DeviceTreeBlob::from_ptr(
                crate::arch::mmu::phys_to_virt(ptr as u64) as *const u8
            )
        }
        .expect("device tree is unreadable");
        let ctx = machine_discovery::plic::PlicContexts::from_device_tree(&dt)
            .expect("the PLIC wiring parses");

        for hart in crate::smp::online_cpus() {
            let parsed = ctx
                .s_context(hart)
                .expect("an online hart has an S context in the tree");
            assert_eq!(
                parsed,
                s_context_of(hart),
                "hart {hart}: the live table does not hold the tree's answer",
            );
        }
    }
}
