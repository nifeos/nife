//! **One function per instruction**: the `x86_64` `asm!` that logic calls, kept out of the logic so
//! a proof can replace it. The twin of `arch/riscv64/instructions.rs`, which has the reasoning.
//!
//! *Name provisional (a lane minted it 2026-09-25, matching the riscv64 file; names are calef's).*
//!
//! Here: the control-register accesses, `clts`, `invlpg`, `hlt`, `sfence` and the `rsp` read
//! (`port.rs`, `msr.rs` and `interrupts.rs` hold theirs). `lidt`, the GDT reload, `int3`, `rdseed`
//! and `fp::touch` stay `asm!`. A model replacing a function is a claim about the silicon, so each
//! says what a model must honor (notes/kernel-proofs/stubbing-an-instruction.md). Every function
//! is `#[inline(always)]` and carries its call site's `options` unchanged.

use core::arch::asm;

// ---- control registers ----

/// `mov {}, cr0`.
#[inline(always)]
pub(super) fn read_cr0() -> u64 {
    let cr0: u64;
    // SAFETY: reads one control register into a local.
    unsafe { asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags)) };
    cr0
}

/// `mov cr0, {}`. A model must make the value visible to the next [`read_cr0`], and must treat
/// [`clts`] as clearing bit 3 (`TS`) of the same register.
///
/// # Safety
/// `CR0` holds `PG` and `WP`. The caller must write back every bit it did not mean to change exactly
/// as it read it.
#[inline(always)]
pub(super) unsafe fn write_cr0(value: u64) {
    // SAFETY: the caller's contract.
    unsafe { asm!("mov cr0, {}", in(reg) value, options(nomem, nostack, preserves_flags)) };
}

/// `clts`: clear `CR0.TS` and nothing else. One byte, and it exists for exactly the lazy-FP scheme.
#[inline(always)]
pub(super) fn clts() {
    // SAFETY: clears one bit of `CR0`. It names no memory; the widest consequence of getting it
    // wrong is a thread executing an FP instruction, or trapping one it should not have.
    unsafe { asm!("clts", options(nomem, nostack, preserves_flags)) };
}

/// `mov {}, cr2`: the linear address of the last page fault.
#[inline(always)]
pub(super) fn read_cr2() -> u64 {
    let cr2: u64;
    // SAFETY: reads a control register. No side effects.
    unsafe { asm!("mov {}, cr2", out(reg) cr2, options(nomem, nostack, preserves_flags)) };
    cr2
}

/// `mov {}, cr3`: the page-table root, flag and PCID bits included.
#[inline(always)]
pub(super) fn read_cr3() -> u64 {
    let cr3: u64;
    // SAFETY: reads a control register. No side effects.
    unsafe { asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags)) };
    cr3
}

/// `mov cr3, {}`: install a page-table root and discard every non-global TLB entry. **No `nomem`**:
/// every access after it translates through the new root.
///
/// # Safety
/// `root` must be a complete kernel map covering the executing code and stack; see `mmu::install`.
#[inline(always)]
pub(super) unsafe fn write_cr3(root: u64) {
    // SAFETY: the caller's contract.
    unsafe { asm!("mov cr3, {}", in(reg) root, options(nostack, preserves_flags)) };
}

/// `mov {}, cr4`.
#[inline(always)]
pub(super) fn read_cr4() -> u64 {
    let cr4: u64;
    // SAFETY: reads one control register into a local.
    unsafe { asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags)) };
    cr4
}

/// `mov cr4, {}`. A model must make the value visible to the next [`read_cr4`].
///
/// # Safety
/// `CR4` holds `PAE` and the other paging controls. The caller must write back every bit it did not
/// mean to change exactly as it read it, and must set only bits CPUID advertises, or the write
/// raises `#GP`.
#[inline(always)]
pub(super) unsafe fn write_cr4(value: u64) {
    // SAFETY: the caller's contract.
    unsafe { asm!("mov cr4, {}", in(reg) value, options(nomem, nostack, preserves_flags)) };
}

// ---- the TLB, waiting, ordering, the stack ----

/// `invlpg [va]`: this CPU's translation for one page. Local only; `mmu::shoot_down_others` is the
/// remote half.
#[inline(always)]
pub(super) fn invlpg(va: u64) {
    // SAFETY: TLB maintenance is always sound, at any address.
    unsafe { asm!("invlpg [{}]", in(reg) va, options(nostack, preserves_flags)) };
}

/// `hlt`: stop until the next interrupt.
#[inline(always)]
pub(super) fn hlt() {
    // SAFETY: halting until the next interrupt only affects when the next instruction runs.
    unsafe { asm!("hlt", options(nomem, nostack)) };
}

/// `sfence`: order every earlier store before any later one. No `nomem`: ordering memory is its
/// purpose.
#[inline(always)]
pub(super) fn sfence() {
    // SAFETY: a fence has no memory effect of its own; it only constrains ordering.
    unsafe { asm!("sfence", options(nostack, preserves_flags)) };
}

/// `mov {}, rsp`.
#[inline(always)]
pub(super) fn read_rsp() -> u64 {
    let rsp: u64;
    // SAFETY: reads a register. No side effects.
    unsafe { asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack, preserves_flags)) };
    rsp
}
