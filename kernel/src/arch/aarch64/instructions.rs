//! **One function per instruction**, kept out of the logic so a proof can replace it; the twin of
//! `arch/riscv64/instructions.rs`, which has the reasoning.
//!
//! *Name provisional (a lane minted it 2026-09-25, matching the riscv64 file; names are calef's).*
//!
//! Names ratified 2026-10-26 (UTC), milestone 707 (aarch64 registers spelled the way riscv64 and
//! x86_64 spell them): the accessors, `wfi`, `dsb_sy`, `isb` and `arch::current_el`. Each accessor
//! opens with its register's full name and carries `#[doc(alias)]` (design/naming.md).
//!
//! Here is every `asm!` this tree wrote around logic, and one accessor for each system register
//! that `timer`, `mmu`, `exceptions`, `isa`, `percpu` and `thread_pointer` read or write whole.
//! Fixed sequences with no Rust logic inside stay at their sites, and so do `pmu.rs`'s own `PMU*`
//! reads. A model of a function here is an
//! unchecked claim about the silicon (notes/kernel-proofs/stubbing-an-instruction.md). Every
//! function is `#[inline(always)]` and carries its call site's `options` unchanged.
//!
//! A system-register accessor is one `mrs` or `msr` with `options(nomem, nostack)`, so a write whose
//! order against memory matters (`TTBRn_EL1`, `VBAR_EL1`) sits between [`dsb_sy`] or [`isb`] at its
//! call site; those carry no `nomem`. A write that changes reachable memory or the trap target is
//! an `unsafe fn`. Field constants live with their users and cite Arm DDI 0487 by register.

use core::arch::asm;

// ---- CPACR_EL1: who may execute FP, SIMD, SVE and SME ----

/// **Architectural Feature Access Control Register** (`CPACR_EL1`, Arm DDI 0487).
/// `mrs cpacr_el1`.
#[inline(always)]
#[doc(alias = "floating-point access control")]
pub(super) fn read_cpacr() -> u64 {
    let cpacr: u64;
    // SAFETY: reads one control register into a local.
    unsafe { asm!("mrs {}, cpacr_el1", out(reg) cpacr, options(nomem, nostack, preserves_flags)) };
    cpacr
}

/// **Architectural Feature Access Control Register** (`CPACR_EL1`, Arm DDI 0487).
/// `msr cpacr_el1` then `isb`. One fixed pair rather than two functions because `CPACR_EL1` is
/// context-synchronizing: without the `isb`, an FP instruction already in the pipeline may resolve
/// against the old value, so there is no correct use of the write without it. A model must make the
/// new value visible to the next [`read_cpacr`].
#[inline(always)]
#[doc(alias = "floating-point access control")]
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

/// **Interrupt Controller System Register Enable Register (EL1)** (`ICC_SRE_EL1`, Arm DDI 0487).
/// `mrs icc_sre_el1`.
///
/// # Safety
/// The register exists only on a GICv3 system; on anything else the access is UNDEFINED and traps.
#[inline(always)]
#[doc(alias = "GICv3 system register enable")]
pub(super) unsafe fn read_icc_sre() -> u64 {
    let sre: u64;
    // SAFETY: the caller's contract.
    unsafe { asm!("mrs {}, icc_sre_el1", out(reg) sre, options(nostack, preserves_flags)) };
    sre
}

/// **Interrupt Controller System Register Enable Register (EL1)** (`ICC_SRE_EL1`, Arm DDI 0487).
/// `msr icc_sre_el1` then `isb`, so a following [`read_icc_sre`] sees the write. A model must allow
/// `SRE` (bit 0) not to stick: an EL2 above this kernel can hold it clear.
///
/// # Safety
/// As [`read_icc_sre`].
#[inline(always)]
#[doc(alias = "GICv3 system register enable")]
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

/// **Multiprocessor Affinity Register** (`MPIDR_EL1`, Arm DDI 0487).
/// `mrs mpidr_el1`: this core's affinity. Readable at EL1 on every aarch64 core.
#[inline(always)]
#[doc(alias = "core affinity")]
pub(super) fn read_mpidr() -> u64 {
    let mpidr: u64;
    // SAFETY: reads an ID register. No memory, no flags.
    unsafe { asm!("mrs {}, mpidr_el1", out(reg) mpidr, options(nomem, nostack, preserves_flags)) };
    mpidr
}

// ---- interrupt masking ----

/// **Interrupt Mask Bits** (`DAIF`, Arm DDI 0487).
/// `mrs daif`.
#[inline(always)]
#[doc(alias = "interrupt mask")]
pub(super) fn read_daif() -> u64 {
    let daif: u64;
    // SAFETY: reads a system register. No side effects.
    unsafe { asm!("mrs {}, daif", out(reg) daif, options(nomem, nostack)) };
    daif
}

/// **Interrupt Mask Bits** (`DAIF`, Arm DDI 0487).
/// `msr daifset, #2`: mask interrupts. This writes `DAIFSet`, the immediate form, not `DAIF`.
#[inline(always)]
#[doc(alias = "irq")]
#[doc(alias = "interrupt mask")]
pub(super) fn mask_interrupts() {
    // SAFETY: masking interrupts is always sound.
    unsafe { asm!("msr daifset, #2", options(nomem, nostack)) };
}

/// **Interrupt Mask Bits** (`DAIF`, Arm DDI 0487).
/// `msr daifclr, #2`: unmask interrupts. This writes `DAIFClr`, the immediate form, not `DAIF`.
#[inline(always)]
#[doc(alias = "irq")]
#[doc(alias = "interrupt mask")]
pub(super) fn unmask_interrupts() {
    // SAFETY: unmasking interrupts is sound; whether it is wise is the caller's question.
    unsafe { asm!("msr daifclr, #2", options(nomem, nostack)) };
}

// ---- the stack and the PMU's EL0 enables ----

/// **Stack Pointer** (`SP`, Arm DDI 0487).
/// `mov {}, sp`.
#[inline(always)]
#[doc(alias = "stack pointer")]
pub(super) fn read_sp() -> u64 {
    let sp: u64;
    // SAFETY: reads a register. No side effects.
    unsafe { asm!("mov {}, sp", out(reg) sp, options(nomem, nostack, preserves_flags)) };
    sp
}

/// **Stack Pointer Select** (`SPSel`, Arm DDI 0487).
/// `mrs spsel`.
#[cfg(feature = "system_tests")]
#[inline(always)]
#[doc(alias = "stack pointer select")]
pub(super) fn read_spsel() -> u64 {
    let spsel: u64;
    // SAFETY: reading SPSel has no side effects.
    unsafe { asm!("mrs {}, spsel", out(reg) spsel, options(nostack, nomem)) };
    spsel
}

/// **Performance Monitors User Enable Register** (`PMUSERENR_EL0`, Arm DDI 0487).
/// `msr pmuserenr_el0`.
///
/// # Safety
/// The register exists only with `FEAT_PMUv3`; the caller must have established that on this core.
/// Every field is an EL0 enable, so no value written can widen what EL1 may do.
#[inline(always)]
#[doc(alias = "performance counters at EL0")]
pub(super) unsafe fn write_pmuserenr(value: u64) {
    // SAFETY: the caller's contract. Touches no memory and clobbers no flags.
    unsafe {
        asm!("msr pmuserenr_el0, {}", in(reg) value, options(nomem, nostack, preserves_flags));
    };
}

// ---- the generic timer (`timer.rs`, `pmu.rs`) ----

/// **Counter-timer Frequency Register** (`CNTFRQ_EL0`, Arm DDI 0487).
/// `mrs cntfrq_el0`: the counter's rate in Hz, as firmware wrote it.
#[inline(always)]
#[doc(alias = "counter frequency")]
pub(super) fn read_cntfrq() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) value, options(nomem, nostack)) };
    value
}

/// **Counter-timer Virtual Count Register** (`CNTVCT_EL0`, Arm DDI 0487).
/// `mrs cntvct_el0`: the virtual count. Not self-synchronizing: two reads stay in order, and a
/// read may be taken early relative to the instructions around it, which every caller here tolerates.
#[inline(always)]
#[doc(alias = "virtual counter")]
pub(super) fn read_cntvct() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, cntvct_el0", out(reg) value, options(nomem, nostack)) };
    value
}

/// **Counter-timer Kernel Control Register** (`CNTKCTL_EL1`, Arm DDI 0487).
/// `mrs cntkctl_el1`.
#[inline(always)]
#[doc(alias = "counter access control")]
pub(super) fn read_cntkctl() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, cntkctl_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// **Counter-timer Kernel Control Register** (`CNTKCTL_EL1`, Arm DDI 0487).
/// `msr cntkctl_el1`: what EL0 may reach of the counters and timers.
#[inline(always)]
#[doc(alias = "counter access control")]
pub(super) fn write_cntkctl(value: u64) {
    // SAFETY: a control register naming no memory. Its fields open counters and timers to EL0, which can widen what EL0 may read and never what EL1 may do.
    unsafe { asm!("msr cntkctl_el1, {}", in(reg) value, options(nomem, nostack)) };
}

/// **Counter-timer Virtual Timer `CompareValue` Register** (`CNTV_CVAL_EL0`, Arm DDI 0487).
/// `mrs cntv_cval_el0`: this core's armed deadline.
#[inline(always)]
#[doc(alias = "timer deadline")]
pub(super) fn read_cntv_cval() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, cntv_cval_el0", out(reg) value, options(nomem, nostack)) };
    value
}

/// **Counter-timer Virtual Timer `CompareValue` Register** (`CNTV_CVAL_EL0`, Arm DDI 0487).
/// `msr cntv_cval_el0`: arm this core's deadline.
#[inline(always)]
#[doc(alias = "timer deadline")]
pub(super) fn write_cntv_cval(value: u64) {
    // SAFETY: the virtual timer's deadline. The widest thing a wrong value does is raise the tick early or late.
    unsafe { asm!("msr cntv_cval_el0, {}", in(reg) value, options(nomem, nostack)) };
}

/// **Counter-timer Virtual Timer Control Register** (`CNTV_CTL_EL0`, Arm DDI 0487).
/// `mrs cntv_ctl_el0`.
#[inline(always)]
#[doc(alias = "timer control")]
pub(super) fn read_cntv_ctl() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, cntv_ctl_el0", out(reg) value, options(nomem, nostack)) };
    value
}

/// **Counter-timer Virtual Timer Control Register** (`CNTV_CTL_EL0`, Arm DDI 0487).
/// `msr cntv_ctl_el0`.
#[inline(always)]
#[doc(alias = "timer control")]
pub(super) fn write_cntv_ctl(value: u64) {
    // SAFETY: enables or masks this core's virtual timer. No memory, and nothing outside the tick.
    unsafe { asm!("msr cntv_ctl_el0, {}", in(reg) value, options(nomem, nostack)) };
}

// ---- translation (`mmu.rs`) ----

/// **System Control Register (EL1)** (`SCTLR_EL1`, Arm DDI 0487).
/// `mrs sctlr_el1`.
#[inline(always)]
#[doc(alias = "system control")]
pub(super) fn read_sctlr() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, sctlr_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// **Translation Control Register (EL1)** (`TCR_EL1`, Arm DDI 0487).
/// `mrs tcr_el1`.
#[cfg(test)]
#[inline(always)]
#[doc(alias = "translation control")]
pub(super) fn read_tcr() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, tcr_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// **Translation Control Register (EL1)** (`TCR_EL1`, Arm DDI 0487).
/// `msr tcr_el1`.
///
/// # Safety
/// The value must describe the tables in `TTBR0_EL1` and `TTBR1_EL1` (granule, sizes,
/// cacheability) and the next fetch after the write takes effect walks with it. See `mmu::install`,
/// which carries the full contract.
#[inline(always)]
#[doc(alias = "translation control")]
pub(super) unsafe fn write_tcr(value: u64) {
    // SAFETY: the caller's contract.
    unsafe { asm!("msr tcr_el1, {}", in(reg) value, options(nomem, nostack)) };
}

/// **Memory Attribute Indirection Register (EL1)** (`MAIR_EL1`, Arm DDI 0487).
/// `msr mair_el1`.
///
/// # Safety
/// Every attribute index a live page-table entry names must keep a meaning compatible with
/// how that memory is used: a device page re-read as cacheable is the failure.
#[inline(always)]
#[doc(alias = "memory attributes")]
pub(super) unsafe fn write_mair(value: u64) {
    // SAFETY: the caller's contract.
    unsafe { asm!("msr mair_el1, {}", in(reg) value, options(nomem, nostack)) };
}

/// **Translation Table Base Register 0 (EL1)** (`TTBR0_EL1`, Arm DDI 0487).
/// `mrs ttbr0_el1`.
#[inline(always)]
#[doc(alias = "user page-table root")]
pub(super) fn read_ttbr0() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, ttbr0_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// **Translation Table Base Register 0 (EL1)** (`TTBR0_EL1`, Arm DDI 0487).
/// `msr ttbr0_el1`.
///
/// # Safety
/// The root in `value` must be a page-aligned L0 table whose descriptors are all valid or all
/// zero, and its ASID must be the one that owns those mappings. See `mmu::set_ttbr0`.
#[inline(always)]
#[doc(alias = "user page-table root")]
pub(super) unsafe fn write_ttbr0(value: u64) {
    // SAFETY: the caller's contract.
    unsafe { asm!("msr ttbr0_el1, {}", in(reg) value, options(nomem, nostack)) };
}

/// **Translation Table Base Register 1 (EL1)** (`TTBR1_EL1`, Arm DDI 0487).
/// `mrs ttbr1_el1`.
#[inline(always)]
#[doc(alias = "kernel page-table root")]
pub(super) fn read_ttbr1() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, ttbr1_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// **Translation Table Base Register 1 (EL1)** (`TTBR1_EL1`, Arm DDI 0487).
/// `msr ttbr1_el1`.
///
/// # Safety
/// The root in `value` must map the code executing the write, its stack and the console,
/// because the next fetch walks it. See `mmu::install`.
#[inline(always)]
#[doc(alias = "kernel page-table root")]
pub(super) unsafe fn write_ttbr1(value: u64) {
    // SAFETY: the caller's contract.
    unsafe { asm!("msr ttbr1_el1, {}", in(reg) value, options(nomem, nostack)) };
}

// ---- exceptions (`exceptions.rs`) ----

/// **Exception Syndrome Register (EL1)** (`ESR_EL1`, Arm DDI 0487).
/// `mrs esr_el1`: the syndrome of the last synchronous exception taken to EL1.
#[inline(always)]
#[doc(alias = "exception syndrome")]
pub(super) fn read_esr() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, esr_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// **Fault Address Register (EL1)** (`FAR_EL1`, Arm DDI 0487).
/// `mrs far_el1`: the faulting address, meaningful only for the classes that set it.
#[inline(always)]
#[doc(alias = "fault address")]
pub(super) fn read_far() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, far_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// **Vector Base Address Register (EL1)** (`VBAR_EL1`, Arm DDI 0487).
/// `mrs vbar_el1`.
#[cfg(test)]
#[inline(always)]
#[doc(alias = "exception vector base")]
pub(super) fn read_vbar() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, vbar_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// **Vector Base Address Register (EL1)** (`VBAR_EL1`, Arm DDI 0487).
/// `msr vbar_el1`.
///
/// # Safety
/// `value` must be the 2 KiB-aligned address of a vector table whose every entry is valid
/// code: the next exception of any kind jumps into it.
#[inline(always)]
#[doc(alias = "exception vector base")]
pub(super) unsafe fn write_vbar(value: u64) {
    // SAFETY: the caller's contract.
    unsafe { asm!("msr vbar_el1, {}", in(reg) value, options(nomem, nostack)) };
}

// ---- identification (`isa.rs`, `pmu.rs`) ----

/// **Main ID Register** (`MIDR_EL1`, Arm DDI 0487).
/// `mrs midr_el1`: the part.
#[inline(always)]
#[doc(alias = "processor part number")]
pub(super) fn read_midr() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, midr_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// **AArch64 Debug Feature Register 0** (`ID_AA64DFR0_EL1`, Arm DDI 0487).
/// `mrs id_aa64dfr0_el1`: the debug and PMU features.
#[inline(always)]
#[doc(alias = "debug and performance monitor features")]
pub(super) fn read_id_aa64dfr0() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, id_aa64dfr0_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// **AArch64 Instruction Set Attribute Register 0** (`ID_AA64ISAR0_EL1`, Arm DDI 0487).
/// `mrs id_aa64isar0_el1`: instruction-set features.
#[inline(always)]
#[doc(alias = "instruction set features")]
pub(super) fn read_id_aa64isar0() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, id_aa64isar0_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// **AArch64 Memory Model Feature Register 0** (`ID_AA64MMFR0_EL1`, Arm DDI 0487).
/// `mrs id_aa64mmfr0_el1`: memory-model features.
#[inline(always)]
#[doc(alias = "memory model features")]
pub(super) fn read_id_aa64mmfr0() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, id_aa64mmfr0_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// **AArch64 Memory Model Feature Register 2** (`ID_AA64MMFR2_EL1`, Arm DDI 0487).
/// `mrs id_aa64mmfr2_el1`: more memory-model features.
#[inline(always)]
#[doc(alias = "memory model features")]
pub(super) fn read_id_aa64mmfr2() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, id_aa64mmfr2_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// **Current Exception Level** (`CurrentEL`, Arm DDI 0487).
/// `mrs CurrentEL`: the exception level this code runs at, in bits 3:2.
#[inline(always)]
#[doc(alias = "exception level")]
pub(super) fn read_current_el() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, CurrentEL", out(reg) value, options(nomem, nostack)) };
    value
}

// ---- the per-CPU pointer and the thread pointer (`percpu.rs`, `thread_pointer.rs`) ----

/// **EL1 Software Thread ID Register** (`TPIDR_EL1`, Arm DDI 0487).
/// `mrs tpidr_el1`.
#[inline(always)]
#[doc(alias = "per-core pointer")]
pub(super) fn read_tpidr_el1() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, tpidr_el1", out(reg) value, options(nomem, nostack)) };
    value
}

/// **EL1 Software Thread ID Register** (`TPIDR_EL1`, Arm DDI 0487).
/// `msr tpidr_el1`.
///
/// # Safety
/// `TPIDR_EL1` is the kernel's per-CPU pointer on this ISA and `cpu::id()` trusts it, so it
/// must name this core's `PerCpu`.
#[inline(always)]
#[doc(alias = "per-core pointer")]
pub(super) unsafe fn write_tpidr_el1(value: u64) {
    // SAFETY: the caller's contract.
    unsafe { asm!("msr tpidr_el1, {}", in(reg) value, options(nomem, nostack)) };
}

/// **EL0 Read/Write Software Thread ID Register** (`TPIDR_EL0`, Arm DDI 0487).
/// `mrs tpidr_el0`: the running thread's own thread pointer.
#[inline(always)]
#[doc(alias = "thread pointer")]
pub(super) fn read_tpidr_el0() -> u64 {
    let value: u64;
    // SAFETY: reads one system register into a local. No memory, no side effects.
    unsafe { asm!("mrs {}, tpidr_el0", out(reg) value, options(nomem, nostack)) };
    value
}

/// **EL0 Read/Write Software Thread ID Register** (`TPIDR_EL0`, Arm DDI 0487).
/// `msr tpidr_el0`.
#[inline(always)]
#[doc(alias = "thread pointer")]
pub(super) fn write_tpidr_el0(value: u64) {
    // SAFETY: EL0's own thread pointer, which EL0 may write itself, so no value written here gives anybody more than it already had.
    unsafe { asm!("msr tpidr_el0, {}", in(reg) value, options(nomem, nostack)) };
}

// ---- waiting and barriers ----

/// **Wait For Interrupt** (`WFI`, Arm DDI 0487).
/// `wfi`: wait for an interrupt.
#[inline(always)]
#[doc(alias = "wait for interrupt")]
pub(super) fn wfi() {
    // SAFETY: waits; no memory, no registers.
    unsafe { asm!("wfi", options(nomem, nostack)) };
}

/// **Data Synchronization Barrier, full system** (`DSB SY`, Arm DDI 0487).
/// `dsb sy`: every earlier memory access completes, for every observer, before any later one. No
/// `nomem`, so it is a compiler barrier too, which the callers that order a system-register write
/// against memory rely on.
#[inline(always)]
#[doc(alias = "memory barrier")]
pub(super) fn dsb_sy() {
    // SAFETY: a barrier has no effect a program can misuse.
    unsafe { asm!("dsb sy", options(nostack)) };
}

/// **Instruction Synchronization Barrier** (`ISB`, Arm DDI 0487).
/// `isb`: what follows is fetched after every earlier context-changing write has taken effect. No
/// `nomem`, for the reason in [`dsb_sy`].
#[inline(always)]
#[doc(alias = "instruction barrier")]
pub(super) fn isb() {
    // SAFETY: as [`dsb_sy`].
    unsafe { asm!("isb", options(nostack)) };
}
