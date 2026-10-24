//! **Model-specific registers**: reading and writing one, and the `gs` base MSR the per-CPU
//! pointer lives in.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// `IA32_GS_BASE`: the base of the `gs` segment, which holds the per-CPU pointer (aarch64's
/// `TPIDR_EL1`, RISC-V's `tp`).
///
/// `IA32_KERNEL_GS_BASE` (0xC0000102) holds the other value and `swapgs` exchanges the two, so a
/// trap from ring 3 recovers the per-CPU pointer without trusting anything the user set (RISC-V's
/// `sscratch` problem). **In ring 0, `IA32_GS_BASE` names the per-CPU block and
/// `IA32_KERNEL_GS_BASE` holds the user's value; in ring 3 they are the other way round.** trap.s
/// swaps them, guarded on the interrupted CPL, and its header says why.
pub(super) const IA32_GS_BASE: u32 = 0xC000_0101;

/// Read a model-specific register. `rdmsr` returns the value split across `edx:eax`, with the
/// register number in `ecx`.
///
/// Name: provisional (milestone 161 (the `x86_64` kernel port)): calef names public functions
/// (AGENTS.md, milestone 160 (review the public function names across the kernel's dependency
/// crates)), and this one was minted by a lane.
///
/// # Safety
/// `msr` must be a register this CPU implements. Reading one it does not is a general protection
/// fault, and `CPUID` is the only way to know for the optional ones.
pub unsafe fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    // SAFETY: the caller's contract. A read has no architectural side effects.
    unsafe {
        asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") low,
            out("edx") high,
            options(nomem, nostack, preserves_flags),
        );
    }
    ((high as u64) << 32) | low as u64
}

/// Write a model-specific register. The counterpart of [`read_msr`], with the same split.
///
/// Name: provisional (milestone 161).
///
/// # Safety
/// `msr` must be one this CPU implements, and `value` must be legal for it. An MSR write is one of
/// the few instructions that can change what mode the machine is in (`IA32_EFER` alone controls
/// long mode, `NXE` and `syscall`), so this is exactly as dangerous as what the caller names.
pub unsafe fn write_msr(msr: u32, value: u64) {
    // SAFETY: the caller's contract.
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") value as u32,
            in("edx") (value >> 32) as u32,
            options(nomem, nostack, preserves_flags),
        );
    }
}
