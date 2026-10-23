//! PLIC driver: the RISC-V Platform-Level Interrupt Controller.
//!
//! The PLIC routes a device's wire to a context (a hart at a privilege level) as a supervisor
//! external interrupt. Between a claim and its completion the source's gateway forwards nothing
//! more, the only mask the interrupt path uses. A completion counts only on a context that enables
//! the source, so the interrupt path never disables one. See notes/interrupts/controller-drivers.md.
//!

//! The register model, all 32-bit MMIO at fixed offsets from the base (from the device tree):
//!
//! ```text
//!   base + 0x0000 + 4*source          per-source priority (0 = never interrupt)
//!   base + 0x1000 + ...               per-source pending bits (read-only; we do not use them)
//!   base + 0x2000 + 0x80*context      per-context enable bits, one bit per source
//!   base + 0x20_0000 + 0x1000*context per-context priority threshold (interrupt if prio > threshold)
//!   base + 0x20_0004 + 0x1000*context per-context claim (read) / complete (write)
//! ```
//!

//! A context's number is the board's, from the device tree via `arch::irq`; this driver is handed
//! one and never derives it. It reaches into no kernel globals (§4): [`init`] is handed the base
//! and the context. Only the enable words, a read-modify-write shared by 32 sources, take a lock
//! ([`ENABLE_BITS`]); claim/complete is the lock-free hot path.

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
/// Not serialized: a whole-word store of the constant `0`, with no read, so two racing harts
/// both leave `0`.
pub(crate) fn open_context(context: usize) {
    write(THRESHOLD_BASE + context * CONTEXT_STRIDE, 0);
}

/// Serializes the per-context enable-bit words, the one read-modify-write in this driver: two
/// interleaved updates lose one, and a lost enable masks a device forever.
///
/// Context: an `IrqSafeMutex` at [`rank::IRQ_CONTROLLER`], a leaf taken holding nothing, never by
/// an interrupt handler. It guards `()`, not [`PLIC_BASE`], so claim/complete never takes it. See
/// notes/interrupts/controller-drivers.md and notes/arch-audit.md finding 3.
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
/// `context` is a hart's S-mode context, chosen by the affinity policy in `arch::irq`.
///
/// Context: takes [`ENABLE_BITS`].
///
/// The enable bit first and the priority second: QEMU's PLIC re-evaluates a pending source on a
/// priority write but not on an enable write, so the other order leaves an early interrupt pending
/// and undelivered (notes/usb.md, "The keystroke stall").
pub fn enable(source: u32, context: usize) {
    set_enable_bit(source, context, true);
    // Priority 1, the lowest that interrupts. Outside the lock: a whole-word write to a register
    // private to `source`. After the enable bit, for the reason above.
    write(PRIORITY_BASE + source as usize * 4, 1);
}

/// Claim the highest-priority pending interrupt for `context`, which masks it: the PLIC will
/// not deliver this source again until [`complete`] is called for it. Returns 0 if nothing is pending
/// (0 is not a valid source; source numbering starts at 1). Reading the claim register is the
/// acknowledge, so call it exactly once per interrupt. `context` must be the claiming hart's own
/// context, so a secondary hart claims from its context, not the boot hart's.
pub fn claim(context: usize) -> u32 {
    // SAFETY: `claim_register` names `context`'s claim register, inside the block `init` mapped.
    unsafe { core::ptr::read_volatile(claim_register(context)) }
}

/// Complete a claimed interrupt: tell the PLIC we are done with `source`, so it may deliver that
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
/// Wrapping arithmetic, because no wrap can happen and this is inlined into the trap body every
/// interrupt crosses (release builds check overflow, notes/overflow-checks.md): the offset is under
/// `CLAIM_REACH` for every context this kernel holds, and `init` refused a base that `CLAIM_REACH`
/// would carry past the top of the address space.
#[inline(always)]
fn claim_register(context: usize) -> *mut u32 {
    let off = THRESHOLD_BASE
        .wrapping_add(context.wrapping_mul(CONTEXT_STRIDE))
        .wrapping_add(CLAIM_OFFSET);
    base().wrapping_add(off) as *mut u32
}

#[cfg(test)]
mod tests {
    //! Tests of the enable path. The lost update the lock closes cannot be reproduced on demand,
    //! so no test here would catch it; what is pinned is that the read-modify-write preserves its
    //! neighbours, and that the lock restores the interrupt state. The rank is checked by every
    //! riscv virtio test, whose `enable` would panic with LOCK ORDER VIOLATION if it were wrong.

    use super::*;

    /// Clear a source's enable bit. The tests' alone: the interrupt path never clears one.
    fn disable(source: u32, context: usize) {
        set_enable_bit(source, context, false);
    }

    /// The read-modify-write does not clobber the sources that share its word: two sources are
    /// enabled, one is disabled, and the other must survive.
    ///
    /// Sources 80 and 81 sit in enable word 2, which nothing on QEMU `virt` occupies (virtio-mmio
    /// and the UART are in word 0, PCIe INTx in word 1), and nothing raises them. The word and the
    /// priorities are restored.
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

    /// The lock is IRQ-safe: the interrupt state around the call is restored, not merely enabled,
    /// in both directions (§9). This proves what would turn the lock into a hang, not the mutual
    /// exclusion.
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
