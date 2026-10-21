//! The interrupt controller, RISC-V: the PLIC.
//!
//! The arch side of the portable `arch::irq` surface, the twin of the aarch64 GIC adapter. Portable
//! code names `arch::irq`, and here it resolves to `drivers::plic`, keeping the driver name inside
//! `arch/` (rule #1, DECISIONS §4). Thin by design: the PLIC does the work.

use core::sync::atomic::{AtomicU16, AtomicUsize, Ordering};

/// Each hart's S-mode PLIC context, read from the device tree's `interrupts-extended` by
/// [`init_contexts`], indexed by hart id. [`CTX_UNKNOWN`] where the tree did not say, in which
/// case [`s_context_of`] falls back to the `2*hart + 1` formula.
///
/// A table, not a formula, because the formula is QEMU `virt`'s layout and not a law: the JH7110's
/// disabled S7 (hart 0) contributes only an M context, so there hart h's S context is `2h`, one
/// off from the formula on every hart, and a kernel using the formula programs its neighbour's
/// context: enables, claims and completes all land one context over, and every external interrupt
/// sits pending forever. See `machine_discovery::plic` and notes/visionfive2.md.
static S_CTX: [AtomicUsize; machine_discovery::plic::MAX_CONTEXT_HARTS] =
    [const { AtomicUsize::new(CTX_UNKNOWN) }; machine_discovery::plic::MAX_CONTEXT_HARTS];
const CTX_UNKNOWN: usize = usize::MAX;

/// **Record the PLIC context layout out of the device tree.** Called once on the boot hart, before
/// anything touches a PLIC context (the PLIC itself is brought up later, by [`init`]). A tree
/// that does not state the layout (no PLIC node, no `interrupts-extended`) leaves the table empty
/// and every lookup falls back to the QEMU formula, which is the behavior this kernel always had.
///
/// Prints only when the machine differs from the formula, because that is the fact worth a boot
/// line: on QEMU it is silence, on the JH7110 it is the reason interrupts work.
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

/// This hart's S-mode PLIC context. On RISC-V the logical cpu id equals the hart id (see
/// `send_reschedule`), so the current hart's context is derived from [`S_CTX`], never stored
/// per-CPU. The external-interrupt handler claims and completes against this, so a secondary hart
/// uses its own context, not the boot hart's.
pub(crate) fn this_s_context() -> usize {
    s_context_of(crate::cpu::id())
}

/// Per-source target PLIC context, or [`CTX_UNASSIGNED`] until the first `enable` chooses one. A
/// device source's target must be **stable** across re-enables: the handler masks the source when it
/// fires, and the driver's ACK re-enables it (this file's `enable`); re-rolling the target each time
/// would leave the mask and the re-enable on different contexts. Assign once, then reuse. Indexed by
/// source number; QEMU `virt` uses small source ids (the UART is 10, virtio-mmio 1..8).
const CTX_UNASSIGNED: u16 = u16::MAX;
const MAX_SOURCES: usize = 128;
static SOURCE_CTX: [AtomicU16; MAX_SOURCES] =
    [const { AtomicU16::new(CTX_UNASSIGNED) }; MAX_SOURCES];

/// Round-robin cursor for spreading device sources over the online harts' contexts, the PLIC twin of
/// the GIC's `ITARGETSR` distribution. Consecutive sources get consecutive harts regardless of their
/// source numbers.
static NEXT_IRQ_HART: AtomicUsize = AtomicUsize::new(0);

/// The PLIC context a source should target: its stable assignment if it has one, otherwise the next
/// online hart's S-context in the round-robin, opened (its threshold lowered) so that hart can take
/// the interrupt, and recorded so every later re-enable reuses it. Bounded to the online harts, each
/// of which has `SEIE` set in [`init_this_cpu`], so we never route to a hart that would drop it.
fn target_context(source: u32) -> usize {
    let slot = &SOURCE_CTX[source as usize % MAX_SOURCES];
    let existing = slot.load(Ordering::Relaxed);
    if existing != CTX_UNASSIGNED {
        return existing as usize;
    }
    // The k-th ONLINE hart, not index k (first-silicon sweep, 2026-08-14): on the VisionFive 2 the
    // online set is {1,2,3}, and `cursor % count` as an index would route a source to parked hart
    // 0's PLIC context, where it sits pending forever with nothing claiming it (the same wedge as
    // the boot_s_context bug below), and would never route one to online hart 3.
    let hart = crate::smp::nth_online(NEXT_IRQ_HART.fetch_add(1, Ordering::Relaxed));
    let ctx = s_context_of(hart);
    // Open the target hart's context before the first source lands on it. The boot hart runs the
    // enables (test wiring, driver spawn), and the threshold register is global MMIO, so it can open
    // any hart's context. Idempotent, so a race just opens it twice.
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

/// The context holding each source's claim, or [`CTX_UNASSIGNED`] when none is outstanding. Set by
/// the external-interrupt handler ([`hold_claim`]) and taken by the driver's ACK ([`enable`]),
/// which must complete against the context that claimed: a source may be enabled on more than one
/// (the hand-over pins the console's to the boot hart, and an ACK with no claim targets
/// [`target_context`]), and only a context that enables it may complete it.
static CLAIM_CTX: [AtomicU16; MAX_SOURCES] = [const { AtomicU16::new(CTX_UNASSIGNED) }; MAX_SOURCES];

/// Record that `context` claimed `source` and that its driver's ACK owes the completion.
pub(crate) fn hold_claim(source: u32, context: usize) {
    CLAIM_CTX[source as usize % MAX_SOURCES].store(context as u16, Ordering::Release);
}

/// **An `Irq` capability's ACK, and the arming at setup.**
///
/// With a claim outstanding, the ACK is the completion, written to the context that claimed: the
/// PLIC held the source from the claim until now, and completing it lets the gateway forward the
/// next interrupt, at once if the driver left the line asserted. Without one (setup, or an ACK with
/// nothing claimed), it enables the source at its stable target hart (see [`target_context`]), so
/// device interrupts spread across harts instead of all landing on the boot hart.
pub fn enable(intid: u32) {
    match CLAIM_CTX[intid as usize % MAX_SOURCES].swap(CTX_UNASSIGNED, Ordering::Acquire) {
        CTX_UNASSIGNED => crate::drivers::plic::enable(intid, target_context(intid)),
        claimed => crate::drivers::plic::complete(intid, claimed as usize),
    }
}

/// **Reserve a vector a PCI function may deliver an MSI-X message on**, the arch contract's
/// milestone-215 name. `None` here, and the `None` is a statement about this board rather than a
/// hole: a PCI function on QEMU's `virt` board raises its interrupt as **INTx**, through the
/// swizzle (`pci::intx_irq`) onto PLIC input `mmu::PCI_IRQ_BASE` (sources 32..35), which the machine's own device tree
/// states and `crates/pci`'s fixture test holds the formula against. `kernel/src/pci.rs` asks this
/// first and falls back to that; the fallback is what runs here.
///
/// **It is not "MSI is unsupported on RISC-V"**, which would be false: the RISC-V AIA's IMSIC is the same mechanism under another name, and a board with one would implement it here. It is that
/// nothing on this board needs it, and a routing path with no consumer is machinery to maintain
/// with no way to know it works. See design/roadmap/0215-x86-64-pci-interrupt-routing.md for why
/// `x86_64` answers differently.
pub(crate) fn alloc_msi_vector() -> Option<(u32, pci::MsiTarget)> {
    None
}

/// The PLIC context that targets the **boot hart's** S-mode. This must be derived, not hardcoded
/// to 1: OpenSBI elects the boot hart by lottery, and a kernel that programs hart 0's context
/// while running (and setting `sie.SEIE`) on hart 3 leaves every external interrupt pending at the
/// PLIC forever, with all harts parked in `wfi`. Found by the parity-C disk test hanging on some
/// runs and passing on others, exactly the lottery's coin flip. The hart-to-context mapping itself
/// comes from [`S_CTX`], for the reason recorded there.
pub(crate) fn boot_s_context() -> usize {
    s_context_of(super::boot_hartid())
}

/// **How many interrupts a device line delivered at bring-up**, which on this port is none: no
/// device here can be made to interrupt without input or an attached disk, so bring-up proves no
/// device line through the PLIC, and `self_test`'s `deviceirq` check reports itself absent. `x86_64`
/// proves the PIT's. A device line on this port is proved where one exists: the user-level input
/// driver at the hand-over, and the `blockdev` check's driver when a disk is attached.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)] // self_test reads it
pub(crate) fn device_line_interrupts() -> Option<u64> {
    None
}

/// The boot hart's half of the shared `interrupts_init`: bring the PLIC up, then unmask this hart.
/// This is the step aarch64 fills with its GIC, and portable code names `arch::irq`, never a
/// controller.
///
/// The PLIC's base comes from `memory::plic_region`, which `memory::init` filled from the device
/// tree, and the context opened is the boot hart's S context, which [`init_contexts`] read from the
/// same tree. Opening that context's threshold here, before the other harts start, delivers nothing
/// on its own: every source stays disabled until [`enable`] routes it. What it buys is that the base
/// is stored before `sie.SEIE` is set, so an external interrupt can never be claimed through a zero
/// base. A machine whose tree names no PLIC gets the unmasking alone, as before.
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

/// Per-core interrupt setup, run on each hart as it becomes a scheduler participant (the primary through
/// [`init`], each secondary in `secondary_main`). Unmasks this hart's software interrupts
/// (`sie.SSIE`), the reschedule-IPI source, so a thread another hart hands it wakes it promptly, and
/// its supervisor external interrupts (`sie.SEIE`), so IRQ affinity ([`enable`]) can route a device
/// source to this hart's PLIC context and have it taken here. The context's threshold is opened, and
/// sources routed, later and by the boot hart (`target_context`); this only unmasks the CSR.
pub(crate) fn init_this_cpu() {
    super::exceptions::enable_software_interrupts();
    // Unmask this hart's supervisor external interrupts (`sie.SEIE`) too, so IRQ affinity can route a
    // device source to any online hart and have it actually take the interrupt. Setting SEIE with no
    // source enabled for this hart's context delivers nothing, so this is safe before any source is
    // routed; a secondary's context is opened, and sources routed to it, later and by the boot hart.
    super::exceptions::enable_external();
}

/// Send a reschedule IPI to `target_cpu`. On RISC-V the logical cpu id equals the hart id, and the
/// IPI goes through the SBI, which sets the target hart's `sip.SSIP` so it takes a supervisor
/// software interrupt (`scause` = 1) and drains its inbox. Unlike the PLIC's external interrupts this
/// is a software interrupt, the firmware's mechanism rather than this controller's, but the
/// `arch::irq` seam is the same one aarch64 fills with a GIC SGI.
pub(crate) fn send_reschedule(target_cpu: usize) {
    crate::arch::sbi_send_ipi(target_cpu);
}

/// **This machine's interrupt controller, for the machine description** (milestone 268).
///
/// One of the eight questions the description answers on every architecture, in this
/// architecture's own vocabulary. The context number is printed beside the address because it is
/// the number that is *not* architectural: `2 * hart + 1` is QEMU's layout, and the JH7110's
/// disabled S7 shifts every context down one, so a board whose PLIC address is right and whose
/// context is wrong enables sources nobody is listening on (notes/visionfive2.md).
// The machine description is the only caller, and it is
// `#[cfg(not(any(test, feature = "bench")))]`: a test boot exits through semihosting and a bench
// boot diverges into `bench::run`, so neither reads a bring-up transcript. Same treatment
// `memory::print_summary` already carries, and for the same reason.
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

    /// **The context table was filled from the machine's own tree, and it agrees with itself.**
    /// A fresh parse of the live DTB must agree with what `init_contexts` recorded for every
    /// online hart, which proves the boot-path read end to end. It must also agree with the
    /// tree's own answer, not a formula: the QEMU `virt` machine every merge boots follows
    /// `2*hart + 1`, but the VisionFive 2's real U-Boot control DTB does not (bench, 2026-08-21:
    /// hart h's S context there is `2h`, because the disabled S7 contributes only a machine
    /// context). Both machines are witnessed on the host
    /// (`crates/machine_discovery/tests/riscv64_plic_contexts.rs`), which is where the formula-specific
    /// assertion belongs; this test only checks that the live table matches a fresh parse of the
    /// live tree, on whichever machine is actually running it.
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

        // The online set, so the loop's claim ("every online hart") is the loop's shape.
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

/// **Milestone 890's probe**: is the console's PLIC gateway stuck after one interrupt? Every 5 s on
/// any hart, print the UART's registers and the PLIC's view of the console source, then complete
/// that source on each context that enables it. The handler completes after it has disabled the
/// source, and the PLIC specification ignores a completion for a source the target does not
/// enable, so if that is the bug each completion here lets exactly one more interrupt through.
#[cfg(feature = "plic_completion_probe")]
pub(crate) fn completion_probe_tick() {
    use core::sync::atomic::AtomicU64;
    // Any hart may print: whichever tick first passes the deadline claims the next one.
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let now = super::timer::now();
    let next = NEXT.load(Ordering::Relaxed);
    if now < next || crate::sync::current_rank() != crate::sync::rank::NONE {
        return;
    }
    let every = super::timer::frequency().wrapping_mul(5);
    if NEXT
        .compare_exchange(next, now.wrapping_add(every), Ordering::Relaxed, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    let source = crate::memory::uart_irq().unwrap_or(10);
    let [ier, iir, lsr, usr] = crate::console::probe_uart();
    let routed = super::exceptions::ROUTED_IRQS.load(Ordering::Relaxed);
    let spurious = super::exceptions::SPURIOUS_IRQS.load(Ordering::Relaxed);
    let target = SOURCE_CTX[source as usize % MAX_SOURCES].load(Ordering::Relaxed);
    let mut pending = false;
    let mut enabled = [false; 5];
    for (hart, e) in enabled.iter_mut().enumerate() {
        let (p, en) = crate::drivers::plic::probe_bits(source, s_context_of(hart));
        pending |= p;
        *e = en;
    }
    let priority = crate::drivers::plic::probe_read(source as usize * 4);
    crate::println!(
        "plic-probe: source={source} routed={routed} spurious={spurious} pending={pending} \
         enabled(by hart)={enabled:?} target_ctx={target} boot_ctx={} prio={priority} \
         ier={ier:#x} iir={iir:#x} lsr={lsr:#x} usr={usr:#x}",
        boot_s_context()
    );
    for (hart, &en) in enabled.iter().enumerate() {
        if en {
            crate::drivers::plic::complete(source, s_context_of(hart));
        }
    }
}
