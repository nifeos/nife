//! **One function per instruction**, kept out of the logic so a proof can replace it; the twin of
//! `arch/riscv64/instructions.rs`, which has the reasoning.
//!
//! *Name provisional (a lane minted it 2026-09-25, matching the riscv64 file; names are calef's).*
//!
//! Here is every `asm!` this tree wrote around logic, and one accessor for each system register
//! that `timer`, `mmu`, `exceptions`, `isa`, `percpu` and `thread_pointer` read or write whole.
//! Fixed sequences with no Rust logic inside stay at their sites, and so do `pmu.rs`'s own `PMU*`
//! reads. A model of a function here is an
//! unchecked claim about the silicon (notes/kernel-proofs/stubbing-an-instruction.md). Every
//! function is `#[inline(always)]` and carries its call site's `options` unchanged.
//!
//! # The system-register accessors
//!
//! One `mrs` or `msr` each, with `options(nomem, nostack)`. `nomem` lets the compiler move
//! ordinary loads and stores across a write, so a write whose order against
//! memory matters (the translation registers, `VBAR_EL1`) is bracketed at its call site by
//! [`dsb_sy`] or [`isb`], which carry no `nomem` and so order the compiler as well as the core. A
//! write that can change which memory is reachable, or what a trap runs, is an `unsafe fn` here,
//! the way riscv64's `write_satp` and `write_stvec` are. Field layouts live with their users
//! (`mmu.rs` for `SCTLR_EL1` and `TCR_EL1`), each citing the Arm Architecture Reference Manual
//! (Arm DDI 0487) by the register's own heading.

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

// ---- the generic timer (`timer.rs`, `pmu.rs`) ----

/// `mrs cntfrq_el0`: the counter's rate in Hz, as firmware wrote it.
#[inline(always)]
pub(super) fn read_cntfrq() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) value, options(nomem, nostack)) };
    value
}

/// `mrs cntvct_el0`: the virtual count. Not self-synchronizing: two reads stay in order, and a
/// read may be taken early relative to the instructions around it, which every caller here tolerates.
#[inline(always)]
pub(super) fn read_cntvct() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, cntvct_el0", out(reg) value, options(nomem, nostack)) };
    value
}

/// `mrs cntkctl_el1`.
#[inline(always)]
pub(super) fn read_cntkctl() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, cntkctl_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// `msr cntkctl_el1`: what EL0 may reach of the counters and timers.
#[inline(always)]
pub(super) fn write_cntkctl(value: u64) {
    // SAFETY: a control register naming no memory. Its fields open counters and timers to EL0, which can widen what EL0 may read and never what EL1 may do.
    unsafe { asm!("msr cntkctl_el1, {}", in(reg) value, options(nomem, nostack)) };
}

/// `mrs cntv_cval_el0`: this core's armed deadline.
#[inline(always)]
pub(super) fn read_cntv_cval() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, cntv_cval_el0", out(reg) value, options(nomem, nostack)) };
    value
}

/// `msr cntv_cval_el0`: arm this core's deadline.
#[inline(always)]
pub(super) fn write_cntv_cval(value: u64) {
    // SAFETY: the virtual timer's deadline. The widest thing a wrong value does is raise the tick early or late.
    unsafe { asm!("msr cntv_cval_el0, {}", in(reg) value, options(nomem, nostack)) };
}

/// `mrs cntv_ctl_el0`.
#[inline(always)]
pub(super) fn read_cntv_ctl() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, cntv_ctl_el0", out(reg) value, options(nomem, nostack)) };
    value
}

/// `msr cntv_ctl_el0`.
#[inline(always)]
pub(super) fn write_cntv_ctl(value: u64) {
    // SAFETY: enables or masks this core's virtual timer. No memory, and nothing outside the tick.
    unsafe { asm!("msr cntv_ctl_el0, {}", in(reg) value, options(nomem, nostack)) };
}

// ---- translation (`mmu.rs`) ----

/// `mrs sctlr_el1`.
#[inline(always)]
pub(super) fn read_sctlr() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, sctlr_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// `mrs tcr_el1`.
#[cfg(test)]
#[inline(always)]
pub(super) fn read_tcr() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, tcr_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// `msr tcr_el1`.
///
/// # Safety
/// The value must describe the tables in `TTBR0_EL1` and `TTBR1_EL1` (granule, sizes,
/// cacheability) and the next fetch after the write takes effect walks with it. See `mmu::install`,
/// which carries the full contract.
#[inline(always)]
pub(super) unsafe fn write_tcr(value: u64) {
    // SAFETY: the caller's contract.
    unsafe { asm!("msr tcr_el1, {}", in(reg) value, options(nomem, nostack)) };
}

/// `msr mair_el1`.
///
/// # Safety
/// Every attribute index a live page-table entry names must keep a meaning compatible with
/// how that memory is used: a device page re-read as cacheable is the failure.
#[inline(always)]
pub(super) unsafe fn write_mair(value: u64) {
    // SAFETY: the caller's contract.
    unsafe { asm!("msr mair_el1, {}", in(reg) value, options(nomem, nostack)) };
}

/// `mrs ttbr0_el1`.
#[inline(always)]
pub(super) fn read_ttbr0() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, ttbr0_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// `msr ttbr0_el1`.
///
/// # Safety
/// The root in `value` must be a page-aligned L0 table whose descriptors are all valid or all
/// zero, and its ASID must be the one that owns those mappings. See `mmu::set_ttbr0`.
#[inline(always)]
pub(super) unsafe fn write_ttbr0(value: u64) {
    // SAFETY: the caller's contract.
    unsafe { asm!("msr ttbr0_el1, {}", in(reg) value, options(nomem, nostack)) };
}

/// `mrs ttbr1_el1`.
#[inline(always)]
pub(super) fn read_ttbr1() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, ttbr1_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// `msr ttbr1_el1`.
///
/// # Safety
/// The root in `value` must map the code executing the write, its stack and the console,
/// because the next fetch walks it. See `mmu::install`.
#[inline(always)]
pub(super) unsafe fn write_ttbr1(value: u64) {
    // SAFETY: the caller's contract.
    unsafe { asm!("msr ttbr1_el1, {}", in(reg) value, options(nomem, nostack)) };
}

// ---- exceptions (`exceptions.rs`) ----

/// `mrs esr_el1`: the syndrome of the last synchronous exception taken to EL1.
#[inline(always)]
pub(super) fn read_esr() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, esr_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// `mrs far_el1`: the faulting address, meaningful only for the classes that set it.
#[inline(always)]
pub(super) fn read_far() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, far_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// `mrs vbar_el1`.
#[cfg(test)]
#[inline(always)]
pub(super) fn read_vbar() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, vbar_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// `msr vbar_el1`.
///
/// # Safety
/// `value` must be the 2 KiB-aligned address of a vector table whose every entry is valid
/// code: the next exception of any kind jumps into it.
#[inline(always)]
pub(super) unsafe fn write_vbar(value: u64) {
    // SAFETY: the caller's contract.
    unsafe { asm!("msr vbar_el1, {}", in(reg) value, options(nomem, nostack)) };
}

// ---- identification (`isa.rs`, `pmu.rs`) ----

/// `mrs midr_el1`: the part.
#[inline(always)]
pub(super) fn read_midr() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, midr_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// `mrs id_aa64dfr0_el1`: the debug and PMU features.
#[inline(always)]
pub(super) fn read_id_aa64dfr0() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, id_aa64dfr0_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// `mrs id_aa64isar0_el1`: instruction-set features.
#[inline(always)]
pub(super) fn read_id_aa64isar0() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, id_aa64isar0_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// `mrs id_aa64mmfr0_el1`: memory-model features.
#[inline(always)]
pub(super) fn read_id_aa64mmfr0() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, id_aa64mmfr0_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// `mrs id_aa64mmfr2_el1`: more memory-model features.
#[inline(always)]
pub(super) fn read_id_aa64mmfr2() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, id_aa64mmfr2_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// `mrs CurrentEL`: the exception level this code runs at, in bits 3:2.
#[inline(always)]
pub(super) fn read_current_el() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, CurrentEL", out(reg) value, options(nomem, nostack)) };
    value
}

// ---- the per-CPU pointer and the thread pointer (`percpu.rs`, `thread_pointer.rs`) ----

/// `mrs tpidr_el1`.
#[inline(always)]
pub(super) fn read_tpidr_el1() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, tpidr_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// `msr tpidr_el1`.
///
/// # Safety
/// `TPIDR_EL1` is the kernel's per-CPU pointer on this ISA and `cpu::id()` trusts it, so it
/// must name this core's `PerCpu`.
#[inline(always)]
pub(super) unsafe fn write_tpidr_el1(value: u64) {
    // SAFETY: the caller's contract.
    unsafe { asm!("msr tpidr_el1, {}", in(reg) value, options(nomem, nostack)) };
}

/// `mrs tpidr_el0`: the running thread's own thread pointer.
#[inline(always)]
pub(super) fn read_tpidr_el0() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, tpidr_el0", out(reg) value, options(nomem, nostack)) };
    value
}

/// `msr tpidr_el0`.
#[inline(always)]
pub(super) fn write_tpidr_el0(value: u64) {
    // SAFETY: EL0's own thread pointer, which EL0 may write itself, so no value written here gives anybody more than it already had.
    unsafe { asm!("msr tpidr_el0, {}", in(reg) value, options(nomem, nostack)) };
}

// ---- waiting and barriers ----

/// `wfi`: wait for an interrupt.
#[inline(always)]
pub(super) fn wfi() {
    // SAFETY: waits; no memory, no registers.
    unsafe { asm!("wfi", options(nomem, nostack)) };
}

/// `dsb sy`: every earlier memory access completes, for every observer, before any later one. No
/// `nomem`, so it is a compiler barrier too, which the callers that order a system-register write
/// against memory rely on.
#[inline(always)]
pub(super) fn dsb_sy() {
    // SAFETY: a barrier has no effect a program can misuse.
    unsafe { asm!("dsb sy", options(nostack)) };
}

/// `isb`: what follows is fetched after every earlier context-changing write has taken effect. No
/// `nomem`, for the reason in [`dsb_sy`].
#[inline(always)]
pub(super) fn isb() {
    // SAFETY: as [`dsb_sy`].
    unsafe { asm!("isb", options(nostack)) };
}
