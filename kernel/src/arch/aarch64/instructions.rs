//! **One function per instruction**: the aarch64 `asm!` that logic calls, kept out of the logic so a
//! proof can replace it. The twin of `arch/riscv64/instructions.rs`, which has the reasoning.
//!
//! *Name provisional (a lane minted it 2026-09-25, matching the riscv64 file; names are calef's).*
//!
//! Here is every `asm!` this tree wrote around logic. Registers reached through `aarch64-cpu`, and
//! fixed sequences with no Rust logic inside (TLB and cache maintenance, `at` probes, the PSCI call,
//! the GIC and PMU sequences), stay at their sites. A model of a function here is an unchecked claim about the silicon; each function says what a
//! model must honour (notes/kernel-proofs/stubbing-an-instruction.md). Every function is
//! `#[inline(always)]` and carries its call site's `options` unchanged.

use core::arch::asm;

// ---- CPACR_EL1: who may execute FP, SIMD, SVE and SME ----

/// `mrs cpacr_el1`.
#[inline(always)]
pub(super) fn read_cpacr() -> u64 {
    let cpacr: u64;
    // SAFETY: reads one control register into a local.
    unsafe { asm!("mrs {}, cpacr_el1", out(reg) cpacr, options(nomem, nostack, preserves_flags)) };
    cpacr
}

/// `msr cpacr_el1` then `isb`. One fixed pair rather than two functions because `CPACR_EL1` is
/// context-synchronizing: without the `isb`, an FP instruction already in the pipeline may resolve
/// against the old value, so there is no correct use of the write without it. A model must make the
/// new value visible to the next [`read_cpacr`].
#[inline(always)]
pub(super) fn write_cpacr_synchronized(cpacr: u64) {
    // SAFETY: a control register naming no memory. The widest thing a wrong value can do is let a
    // thread execute an FP instruction, or trap one it should not have; it cannot hand anybody a
    // page.
    unsafe {
        asm!(
            "msr cpacr_el1, {}",
            "isb",
            in(reg) cpacr,
            options(nomem, nostack, preserves_flags),
        );
    };
}

// ---- the GICv3 system-register interface ----

/// `mrs icc_sre_el1`.
///
/// # Safety
/// The register exists only on a GICv3 system; on anything else the access is UNDEFINED and traps.
#[inline(always)]
pub(super) unsafe fn read_icc_sre() -> u64 {
    let sre: u64;
    // SAFETY: the caller's contract.
    unsafe { asm!("mrs {}, icc_sre_el1", out(reg) sre, options(nostack, preserves_flags)) };
    sre
}

/// `msr icc_sre_el1` then `isb`, so a following [`read_icc_sre`] sees the write. A model must allow
/// `SRE` (bit 0) not to stick: an EL2 above this kernel can hold it clear.
///
/// # Safety
/// As [`read_icc_sre`].
#[inline(always)]
pub(super) unsafe fn write_icc_sre_synchronized(sre: u64) {
    // SAFETY: the caller's contract. Setting SRE only changes how the ICC registers are reached.
    unsafe {
        asm!(
            "msr icc_sre_el1, {}",
            "isb",
            in(reg) sre,
            options(nostack, preserves_flags),
        );
    };
}

/// `mrs mpidr_el1`: this core's affinity. Readable at EL1 on every aarch64 core.
#[inline(always)]
pub(super) fn read_mpidr() -> u64 {
    let mpidr: u64;
    // SAFETY: reads an ID register. No memory, no flags.
    unsafe { asm!("mrs {}, mpidr_el1", out(reg) mpidr, options(nomem, nostack, preserves_flags)) };
    mpidr
}

// ---- interrupt masking ----

/// `mrs daif`.
#[inline(always)]
pub(super) fn read_daif() -> u64 {
    let daif: u64;
    // SAFETY: reads a system register. No side effects.
    unsafe { asm!("mrs {}, daif", out(reg) daif, options(nomem, nostack)) };
    daif
}

/// `msr daifset, #2`: mask IRQs.
#[inline(always)]
pub(super) fn mask_irqs() {
    // SAFETY: masking IRQs is always sound.
    unsafe { asm!("msr daifset, #2", options(nomem, nostack)) };
}

/// `msr daifclr, #2`: unmask IRQs.
#[inline(always)]
pub(super) fn unmask_irqs() {
    // SAFETY: unmasking IRQs is sound; whether it is wise is the caller's question.
    unsafe { asm!("msr daifclr, #2", options(nomem, nostack)) };
}

// ---- the stack and the PMU's EL0 enables ----

/// `mov {}, sp`.
#[inline(always)]
pub(super) fn read_sp() -> u64 {
    let sp: u64;
    // SAFETY: reads a register. No side effects.
    unsafe { asm!("mov {}, sp", out(reg) sp, options(nomem, nostack, preserves_flags)) };
    sp
}

/// `mrs spsel`.
#[cfg(feature = "system_tests")]
#[inline(always)]
pub(super) fn read_spsel() -> u64 {
    let spsel: u64;
    // SAFETY: reading SPSel has no side effects.
    unsafe { asm!("mrs {}, spsel", out(reg) spsel, options(nostack, nomem)) };
    spsel
}

/// `msr pmuserenr_el0`.
///
/// # Safety
/// The register exists only with `FEAT_PMUv3`; the caller must have established that on this core.
/// Every field is an EL0 enable, so no value written can widen what EL1 may do.
#[inline(always)]
pub(super) unsafe fn write_pmuserenr(value: u64) {
    // SAFETY: the caller's contract. Touches no memory and clobbers no flags.
    unsafe {
        asm!("msr pmuserenr_el0, {}", in(reg) value, options(nomem, nostack, preserves_flags));
    };
}
