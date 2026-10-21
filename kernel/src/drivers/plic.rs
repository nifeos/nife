//! PLIC driver: the RISC-V Platform-Level Interrupt Controller (milestone 20).
//!
//! The RISC-V analog of the GIC. Where an aarch64 device raises an SPI into the GIC and the CPU
//! takes an IRQ exception, a RISC-V device raises a wire into the **PLIC**, which routes it to a
//! *context* (a hart at a privilege level) as a supervisor external interrupt (`scause` = 9). The
//! PLIC's job is priority arbitration and the claim/complete handshake. Between a claim and its
//! completion the source's gateway forwards nothing more, which is the only mask the interrupt
//! path uses; the per-context enable bits and threshold decide where a source may go at all.
//!
//! **A completion counts only on a context that enables the source.** The specification says a
//! completion for a source the target does not enable is silently ignored, and the JH7110's PLIC
//! does ignore it, leaving the gateway waiting for a completion that has already been written:
//! that source never interrupts again. QEMU's PLIC (`hw/intc/sifive_plic.c`) completes whatever
//! it is given, so only silicon shows the difference. That is why the interrupt path never
//! disables a source.
//!
//! The register model, all 32-bit MMIO at fixed offsets from the base (which comes from the device
//! tree, like the GIC's):
//!
//! ```text
//!   base + 0x0000 + 4*source          per-source priority (0 = never interrupt)
//!   base + 0x1000 + ...               per-source pending bits (read-only; we do not use them)
//!   base + 0x2000 + 0x80*context      per-context enable bits, one bit per source
//!   base + 0x20_0000 + 0x1000*context per-context priority threshold (interrupt if prio > threshold)
//!   base + 0x20_0004 + 0x1000*context per-context claim (read) / complete (write)
//! ```
//!
//! A *context* is a (hart, privilege) pair, and **the numbering is the board's**, read out of the
//! device tree's `interrupts-extended` by `arch::irq::init_contexts` (`machine_discovery::plic`). On QEMU's `virt`
//! S-mode is `2*hart + 1` (`2*hart` is M-mode, which OpenSBI owns); on the JH7110 the disabled S7
//! contributes only an M context and S-mode is `2*hart`. This driver is always handed a context
//! and never derives one, so the mapping lives in `arch::irq`, not here.
//!
//! Same rule as every driver here (DECISIONS §4): **it reaches into no kernel globals.** [`init`]
//! is handed the base address and the context; everything else works from the base it stored. (It
//! owns a lock, as `drivers/gic.rs` does, which is not a kernel global: it is this driver's own
//! state, and the rank constant is the only thing it borrows from above.)
//!

//! # Exactly one register in this map needs serializing, and it is not the hot one
//!
//! Audited register by register (finding 3 of notes/arch-audit.md):
//!
//! | Register | Shape | Serialized? |
//! |---|---|---|
//! | per-source priority | one 32-bit word **per source**, whole-word write of a constant | no: unshared |
//! | per-context enable bits | one word shared by **32 sources**, read-modify-write | **yes** |
//! | per-context threshold | one word per context, whole-word write of `0`, idempotent | no: not an RMW |
//! | per-context claim/complete | one word per context, read = claim, write = complete | no: not an RMW |
//!
//! Only the enable word is a read-modify-write over state shared between callers, so only it takes
//! the lock. The claim/complete handshake stays lock-free on purpose: it is the external-interrupt
//! hot path. A claim is one read by the context's own hart, and a completion is one write of a
//! source number, from whichever hart runs the driver's ACK; the PLIC orders the two itself, and
//! neither reads back what the other wrote. It also does not share a word with the enable bits
//! (different 0x2000 and `0x20_0000` blocks), so serializing one does not drag in the other.

use core::sync::atomic::{AtomicUsize, Ordering};

use crate::sync::{IrqSafeMutex, rank};

/// The PLIC's MMIO base (a kernel virtual address in the direct map), stored by [`init`].
static PLIC_BASE: AtomicUsize = AtomicUsize::new(0);

const PRIORITY_BASE: usize = 0x0000;
const ENABLE_BASE: usize = 0x2000;
const ENABLE_STRIDE: usize = 0x80; // per context
const THRESHOLD_BASE: usize = 0x20_0000;
const CONTEXT_STRIDE: usize = 0x1000; // per context, for threshold and claim/complete
const CLAIM_OFFSET: usize = 0x0004; // claim/complete sits one word past the threshold

fn base() -> usize {
    PLIC_BASE.load(Ordering::Relaxed)
}

/// Read a 32-bit PLIC register at `off` from the base.
fn read(off: usize) -> u32 {
    // SAFETY: `off` names a PLIC register within the block `init` mapped and promised.
    unsafe { core::ptr::read_volatile((base() + off) as *const u32) }
}

/// Write a 32-bit PLIC register at `off` from the base.
fn write(off: usize, val: u32) {
    // SAFETY: as `read`.
    unsafe { core::ptr::write_volatile((base() + off) as *mut u32, val) }
}

/// A raw PLIC register read, for milestone 890's probe.
#[cfg(feature = "plic_completion_probe")]
pub(crate) fn probe_read(off: usize) -> u32 {
    read(off)
}

/// `source`'s pending bit and its enable bit in `context`, for milestone 890's probe.
#[cfg(feature = "plic_completion_probe")]
pub(crate) fn probe_bits(source: u32, context: usize) -> (bool, bool) {
    if base() == 0 {
        return (false, false);
    }
    let bit = 1u32 << (source % 32);
    let word = (source as usize / 32) * 4;
    (
        read(0x1000 + word) & bit != 0,
        read(ENABLE_BASE + context * ENABLE_STRIDE + word) & bit != 0,
    )
}

/// Bring the PLIC up for one context: record the base and context, and set the threshold to 0 so
/// every source with a nonzero priority can interrupt. Sources are still individually disabled
/// until [`enable`]; this only opens the gate.
///
/// # Safety
/// `base` must be the PLIC's MMIO base as a mapped, device-typed kernel virtual address, and
/// `context` must be this hart's supervisor context number.
pub unsafe fn init(base: usize, context: usize) {
    // The one check [`claim_register`] leans on, made once here instead of on every interrupt.
    assert!(
        base.checked_add(CLAIM_REACH).is_some(),
        "plic: base {base:#x} puts claim registers past the top of the address space"
    );
    PLIC_BASE.store(base, Ordering::Relaxed);
    // Threshold 0: an interrupt is taken when its priority is strictly greater, so priority >= 1
    // gets through. (Threshold at max would mask everything.) Opens the boot context; a secondary
    // hart's context is opened lazily by `arch::irq` the first time a source is routed to it.
    open_context(context);
}

/// Open `context`'s threshold so any source with a nonzero priority can interrupt it. [`init`] does
/// this for the boot context; IRQ affinity (`arch::irq`) calls this for a secondary hart's context
/// the first time it routes a device source there, so that hart can actually take the interrupt. A
/// context whose threshold is never opened takes nothing, which is the safe default. Idempotent.
///
/// **Deliberately not serialized**, and it was checked rather than assumed. The threshold is one
/// whole 32-bit register per context and this writes the constant `0` into it: there is no read, so
/// there is nothing to lose. Two harts racing here both store `0` and the register ends up `0`,
/// which is the only value we ever write. The affinity work made this reachable from the boot hart
/// for *another* hart's context, which is what put it on the audit list; it is a plain store, so it
/// stays a plain store.
pub(crate) fn open_context(context: usize) {
    write(THRESHOLD_BASE + context * CONTEXT_STRIDE, 0);
}

/// **Serializes the per-context enable-bit words**, the one read-modify-write in this driver.
///
/// # Why a lock at all
///
/// The enable bits are one 32-bit register per **32 sources** of a context, so setting one source's
/// bit means read the word, set a bit, write it back. Two of those interleaved lose an update, and
/// a lost enable leaves a device masked forever and its driver blocked on an interrupt that never
/// comes. It is reachable, because [`enable`] runs from kernel-thread context on any hart (the
/// spawn paths, an `Irq` ACK with no claim outstanding), and affinity puts several of QEMU `virt`'s
/// sources on each context. See notes/arch-audit.md finding 3.
///
/// aarch64 needs no equivalent because the GIC's `ISENABLER`/`ICENABLER` are write-1-to-set and
/// write-1-to-clear: one store touches one line and cannot disturb its neighbours, so the
/// architecture hands you the atomicity. The PLIC has plain read/write enable bits and does not.
///
/// # Why an `IrqSafeMutex` and not a plain one
///
/// No interrupt handler takes it: the handler claims and the ACK completes, and neither touches an
/// enable bit. It stays IRQ-safe because the critical section is two MMIO accesses, so masking
/// costs nothing, and a handler that did take a plain spinlock here would be the §9 deadlock: a
/// thread on hart H holding it, an external interrupt on H, and a handler spinning forever on code
/// that cannot run until it returns.
///
/// # The rank
///
/// [`rank::IRQ_CONTROLLER`], the same rank the GIC's lock takes, because it is the same role on the
/// other ISA (see sync.rs). It is a **leaf**: nothing is acquired while it is held (the body is two
/// MMIO accesses and no calls), and it is itself always taken holding nothing: every [`enable`]
/// caller has already released `IPC_TABLES` by the time it calls (`bind_irq` and
/// `create_rendezvous` take and drop it first). So the strictly-decreasing rule has slack rather
/// than being met exactly, which is the comfortable place to be.
///
/// It guards `()` rather than the base address on purpose. The state being protected is a device
/// register, not Rust data, and wrapping [`PLIC_BASE`] would put this lock on the claim/complete hot
/// path, which is precisely what must not happen.
static ENABLE_BITS: IrqSafeMutex<()> = IrqSafeMutex::new(rank::IRQ_CONTROLLER, ());

/// Set or clear `source`'s bit in `context`'s enable word, under [`ENABLE_BITS`]. Clearing is the
/// tests' alone (the module doc says why nothing else clears one); it lives here so the
/// read-modify-write exists in exactly one place.
fn set_enable_bit(source: u32, context: usize, on: bool) {
    let word = ENABLE_BASE + context * ENABLE_STRIDE + (source as usize / 32) * 4;
    let bit = 1u32 << (source % 32);

    let _guard = ENABLE_BITS.lock();
    let was = read(word);
    write(word, if on { was | bit } else { was & !bit });
}

/// Enable `source` for `context` and give it a nonzero priority, so the PLIC will deliver it there.
///
/// `context` is a hart's S-mode context (the device tree's numbering; see the module docs); the
/// affinity policy in `arch::irq` chooses which one, so a device line lands on a chosen hart
/// rather than always the boot hart. The driver is told the context; it does not read the hartid
/// (rule #2, DECISIONS §4).
///
/// **The enable bit first and the priority second, and the order is load-bearing.** This runs
/// when a source is armed, and a device that spoke before it was armed has a pending bit waiting
/// here. By the
/// specification the PLIC delivers that the moment the bit is enabled. QEMU's PLIC (checked in
/// `hw/intc/sifive_plic.c` at 11.1.1) does not: a write to an enable word changes the word and
/// re-evaluates nothing, while a priority write, a threshold write, a completion and a rising
/// source line all do. With the priority written first, the re-evaluation happened while the bit
/// was still clear, the enable then changed nothing, and the interrupt stayed pending, enabled and
/// undelivered until some other device's line happened to rise. That stalled the USB keyboard
/// mid-line about one boot in thirty on riscv64 while the ACK was an enable (notes/usb.md); the
/// ACK is now a [`complete`], which QEMU does re-evaluate on. Written second, the priority write is
/// the re-evaluation. It costs nothing on a PLIC that follows the specification, where the order
/// does not matter.
pub fn enable(source: u32, context: usize) {
    set_enable_bit(source, context, true);
    // Priority 1 (the lowest that still interrupts; we do not prioritize among sources yet). Outside
    // the lock deliberately: this is a whole-word write to a register private to `source`, so it
    // shares nothing, and keeping it out keeps the critical section to the two accesses that need it
    // (§9 (locking: IrqSafeMutex, plus a discipline): keep them short, interrupts are off for the
    // whole of it). After the enable bit, for the reason above.
    write(PRIORITY_BASE + source as usize * 4, 1);
}

/// **Claim the highest-priority pending interrupt** for `context`, which masks it: the PLIC will
/// not deliver this source again until [`complete`] is called for it. Returns 0 if nothing is pending
/// (0 is not a valid source; source numbering starts at 1). Reading the claim register is the
/// acknowledge, so call it exactly once per interrupt. `context` must be the **claiming hart's own**
/// context, so a secondary hart claims from its context, not the boot hart's.
pub fn claim(context: usize) -> u32 {
    // SAFETY: `claim_register` names `context`'s claim register, inside the block `init` mapped.
    unsafe { core::ptr::read_volatile(claim_register(context)) }
}

/// **Complete** a claimed interrupt: tell the PLIC we are done with `source`, so it may deliver that
/// source again. The counterpart to [`claim`]; between the two the source is masked at the PLIC.
/// `context` must be the context that claimed it, and `source` must still be enabled there, or the
/// PLIC ignores the completion (the module doc). Any hart may write it.
pub fn complete(source: u32, context: usize) {
    // SAFETY: as `claim`.
    unsafe { core::ptr::write_volatile(claim_register(context), source) }
}

/// One past the last byte any context's claim register can occupy, as an offset from the base:
/// every context this kernel holds is at most `u16::MAX` (`machine_discovery::plic` stores the
/// device tree's as `u16`, and the `2h + 1` fallback is bounded by `MAX_CPUS`).
const CLAIM_REACH: usize = THRESHOLD_BASE + u16::MAX as usize * CONTEXT_STRIDE + CLAIM_OFFSET + 4;

/// `context`'s claim/complete register.
///
/// **Wrapping, because no wrap can happen**, and this is on every external interrupt and inlined
/// into the trap body every syscall crosses (release builds check overflow,
/// notes/overflow-checks.md). The offset is under `CLAIM_REACH` for any context at most
/// `u16::MAX`, which is every context this kernel holds, and `init` refused a base that `CLAIM_REACH`
/// would carry past the top of the address space. What the per-interrupt overflow checks used to
/// refuse was only a context near `2^52`; a context past the PLIC's own 15,872 was never refused by
/// them either, and is not now.
#[inline(always)]
fn claim_register(context: usize) -> *mut u32 {
    let off = THRESHOLD_BASE
        .wrapping_add(context.wrapping_mul(CONTEXT_STRIDE))
        .wrapping_add(CLAIM_OFFSET);
    base().wrapping_add(off) as *mut u32
}

#[cfg(test)]
mod tests {
    //! **What can and cannot be tested here, stated up front.**
    //!
    //! The bug this file's lock closes is a lost update between two MMIO accesses. It cannot be
    //! reproduced on demand: the window is two instructions wide, enables happen a handful of times
    //! per boot, and widening the window to catch it would mean shipping test-only code inside the
    //! critical section and then testing the instrumented version rather than the real one. A loop of
    //! two harts hammering the enable bits would pass with the lock and pass without it, so it would
    //! prove nothing. There is no test here that would have caught the bug, and pretending otherwise
    //! would be worse than saying so.
    //!
    //! What *is* tested is the half that a test can actually pin: the read-modify-write preserves the
    //! neighbours that share its word. That is precisely the invariant a lost update violates, so a
    //! regression in the RMW itself (an inverted flag, a wrong bit or word index, a `write` that
    //! forgets to `read` first) fails here deterministically.
    //!
    //! The rank is proved by the suite it runs inside rather than by an assertion: every riscv
    //! virtio test spawns a driver, which calls `enable` from thread context with interrupts on. If
    //! the rank were misplaced, §9's enforcement is an `assert!` in `IrqSafeMutex::lock` and the run
    //! would panic with LOCK ORDER VIOLATION.

    use super::*;

    /// Clear a source's enable bit. The tests' alone: the interrupt path never clears one.
    fn disable(source: u32, context: usize) {
        set_enable_bit(source, context, false);
    }

    /// **The read-modify-write does not clobber the sources that share its word.**
    ///
    /// One enable register carries 32 sources, so setting one bit means read, modify, write, and
    /// getting that wrong takes its 31 neighbours with it. Two sources are enabled, then one is
    /// disabled, and the other must survive: that is the lost-update *shape*, checked deterministically
    /// on one hart.
    ///
    /// Sources 80 and 81 are used because they live in enable word 2 (sources 64..95), which nothing
    /// on QEMU `virt` occupies: virtio-mmio (1..8) and the UART (10) are in word 0, PCIe INTx (32..35)
    /// in word 1. So the test cannot perturb a live interrupt even transiently, and nothing raises
    /// these lines, so enabling them delivers nothing. The word and the priorities are put back
    /// afterwards regardless.
    #[test_case]
    fn setting_one_enable_bit_leaves_its_neighbours_alone() {
        const A: u32 = 80;
        const B: u32 = 81;

        assert!(
            base() != 0,
            "the PLIC base is unset: plic::init has not run, so this test would write to a null \
             device"
        );

        let ctx = crate::arch::irq::this_s_context();
        let word = ENABLE_BASE + ctx * ENABLE_STRIDE + (A as usize / 32) * 4;
        assert_eq!(
            A as usize / 32,
            B as usize / 32,
            "the two probe sources must share one enable word or this proves nothing"
        );
        let saved = read(word);

        let bit_a = 1u32 << (A % 32);
        let bit_b = 1u32 << (B % 32);

        enable(A, ctx);
        assert_ne!(read(word) & bit_a, 0, "enable did not set the source's bit");

        enable(B, ctx);
        assert_ne!(
            read(word) & bit_b,
            0,
            "enable did not set the neighbour's bit"
        );
        assert_ne!(
            read(word) & bit_a,
            0,
            "enabling the neighbour cleared the first source: the read-modify-write dropped it"
        );

        disable(A, ctx);
        assert_eq!(read(word) & bit_a, 0, "disable did not clear the bit");
        assert_ne!(
            read(word) & bit_b,
            0,
            "disabling one source cleared its neighbour: this is the lost update the lock exists \
             to prevent, arrived at from inside a single hart"
        );

        disable(B, ctx);
        assert_eq!(
            read(word) & bit_b,
            0,
            "disable did not clear the second bit"
        );

        // Leave no trace: the word exactly as found, and the priorities `enable` raised back to 0
        // (never interrupt), which is what an untouched source reads.
        write(PRIORITY_BASE + A as usize * 4, 0);
        write(PRIORITY_BASE + B as usize * 4, 0);
        write(word, saved);
        assert_eq!(
            read(word),
            saved,
            "the test did not restore the enable word"
        );
    }

    /// **The lock is IRQ-safe at the shape that would deadlock.**
    ///
    /// `enable` runs in thread context with interrupts on, and the lock must be safe to take with
    /// them masked too. A plain spinlock taken by a handler would let a thread take the word on hart
    /// H, take an external interrupt on H, and spin in the handler forever on a lock only the
    /// interrupted code can release (DECISIONS §9). The observable consequence of getting it right
    /// is that the interrupt state around the call is *restored*, not merely enabled, in both
    /// directions.
    ///
    /// This does not prove the mutual exclusion. It proves the thing that would turn the fix into a
    /// hang, which is the more likely way to get this wrong later.
    #[test_case]
    fn the_enable_path_restores_the_interrupt_state_it_found() {
        use crate::arch::interrupts;

        const PROBE: u32 = 80;
        let ctx = crate::arch::irq::this_s_context();
        let word = ENABLE_BASE + ctx * ENABLE_STRIDE + (PROBE as usize / 32) * 4;
        let saved = read(word);

        // Thread context: interrupts on, and they must still be on afterwards.
        interrupts::enable();
        enable(PROBE, ctx);
        assert!(
            interrupts::is_enabled(),
            "the enable path left interrupts masked: the guard did not restore them"
        );

        // Handler context: interrupts already masked, and they must STAY masked. Blindly enabling
        // here is the bug `restore` exists to avoid, and it would unmask inside a handler.
        let outer = interrupts::disable();
        disable(PROBE, ctx);
        assert!(
            !interrupts::is_enabled(),
            "the disable path unmasked interrupts inside an IRQ-disabled context, which is what it \
             would do to an interrupt handler"
        );
        interrupts::restore(outer);

        write(PRIORITY_BASE + PROBE as usize * 4, 0);
        write(word, saved);
    }
}
