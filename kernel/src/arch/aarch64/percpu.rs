//! **Which exception level firmware entered at, this core's identity and its per-CPU block**,
//! read through `TPIDR_EL1`.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

unsafe extern "C" {
    /// The exception level core 0 was entered at, written by `boot.s` before anything else runs.
    /// See [`entry_el`].
    static boot_entry_el: u64;
}

/// **Which exception level did firmware enter this kernel at?** 1 or 2.
///
/// The kernel always runs at EL1; `boot.s` drops from EL2 when entered there. The PSCI conduit
/// follows the entry level, and the boot banner prints it first. See notes/aarch64-port.md, "PSCI
/// and the entry exception level".
///
/// The name is provisional (AGENTS.md: names are an architect's).
///
/// The callers are the banner and `isa`'s conduit test, so only the `bench` boot has none.
#[cfg_attr(feature = "bench", allow(dead_code))]
pub fn entry_el() -> u64 {
    // SAFETY: an 8-byte aligned word in .bss, written once by `_boot_el1` on core 0 before any
    // Rust runs and never again. Volatile because nothing in the Rust program writes it, and a
    // compiler that noticed that would be entitled to fold the read to the zero .bss began at.
    unsafe { core::ptr::read_volatile(&raw const boot_entry_el) }
}

/// Point `TPIDR_EL1` at this core's per-CPU block.
///
/// `TPIDR_EL1` is reserved for software, and holds the per-core pointer as it does in Linux. The
/// portable side is `kernel/src/cpu.rs`.
pub(crate) fn set_percpu(ptr: usize) {
    TPIDR_EL1.set(ptr as u64);
}

/// The logical id of the core the kernel boots on. On aarch64's `virt` the boot core is MPIDR
/// affinity 0, so it is always 0; the SMP bring-up starts every *other* core. (RISC-V's boot hart is
/// not guaranteed to be 0, so its `arch::boot_cpu_id` reads the actual hart id.)
pub(crate) fn boot_cpu_id() -> usize {
    0
}

/// Read this core's per-CPU pointer back. One instruction.
pub fn percpu() -> usize {
    TPIDR_EL1.get() as usize
}

/// **This core's `PerCpu::current`, read with IRQs masked**, so no interrupt can fall between
/// finding the block and reading the field (notes/mmu.md, BUGS).
///
/// [`percpu()`] then a load is two steps, and a thread migrated between them reads another core's
/// block. aarch64 has no load relative to a system register, so this masks IRQs across the pair,
/// in one `asm!` block because the icount gate measures an opt-level 0 kernel. Returns
/// `PerCpu::current`; restores `DAIF` whole. See notes/aarch64-port.md, "The per-CPU block".
///
/// Name: provisional.
#[inline(always)]
pub fn read_current_thread() -> u64 {
    let v: u64;
    // SAFETY: `TPIDR_EL1` holds this core's `PerCpu` (`set_percpu`, written once per core at boot),
    // and `CURRENT_OFF` is the offset of an aligned `AtomicU64` in it, so the `ldr` is a relaxed
    // atomic load. IRQs are masked from before the `mrs` to after the `ldr`, so both happen on one
    // core, and the saved `DAIF` is restored unchanged. Nothing in memory is written.
    unsafe {
        core::arch::asm!(
            "mrs {daif}, daif",
            "msr daifset, #2",
            "mrs {base}, tpidr_el1",
            "ldr {v}, [{base}, #{off}]",
            "msr daif, {daif}",
            daif = out(reg) _,
            base = out(reg) _,
            v = out(reg) v,
            off = const CURRENT_OFF,
            options(nostack, readonly, preserves_flags),
        );
    }
    v
}

/// The offset of `PerCpu::current`, which [`read_current_thread`] loads from `TPIDR_EL1`.
const CURRENT_OFF: usize = core::mem::offset_of!(crate::cpu::PerCpu, current);
const _: () = assert!(
    CURRENT_OFF.is_multiple_of(8) && CURRENT_OFF < 32768,
    "PerCpu::current must sit within `ldr`'s scaled unsigned 12-bit offset from TPIDR_EL1"
);

/// **Test-only: does the per-CPU pointer name the hart we run on?** Always `true` here: the trap
/// frame never saves `TPIDR_EL1`. The RISC-V twin checks, because its `tp` can ride a stale trap
/// frame across a migration (DECISIONS §28).
#[cfg(any(test, feature = "system_tests"))]
pub fn percpu_matches_hart() -> bool {
    true
}
