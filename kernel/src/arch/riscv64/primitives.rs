//! The small primitives the portable kernel calls: halting, waiting for an interrupt, the stack
//! pointer, the DMA write barrier and instruction-cache sync.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// Stop this hart forever with `wfi`, never a spin, so a halted QEMU costs no host CPU
/// (notes/skills/qemu-hygiene/SKILL.md).
///
/// The [`crate::arch::HaltReason`]'s constructors list who may stop a core for good; a finished
/// thread leaves with `sched::exit` instead.
// A test image on this architecture never halts: its panics and its suite's end both exit through
// the SiFive test finisher (`semihosting::exit`), as aarch64's do through semihosting.
#[cfg_attr(any(test, feature = "system_tests"), allow(dead_code))]
pub fn halt(_: crate::arch::HaltReason) -> ! {
    loop {
        instructions::wfi();
    }
}

/// Park until the next interrupt (the scheduler's idle primitive).
pub fn wait_for_interrupt() {
    instructions::wfi();
}

/// This core's current stack pointer, for the stack-overflow canary check (stack.rs).
pub fn current_sp() -> u64 {
    instructions::read_sp()
}

/// A DMA write barrier: order all prior stores before any device sees a later one. A full `fence`,
/// like aarch64's `dsb sy`, where `fence ow, ow` would be the narrower choice.
pub fn direct_memory_access_write_barrier() {
    instructions::full_fence();
}

/// Make every online hart's instruction fetch see code just written as data: `fence.i` here, and
/// through SBI RFENCE on the others.
///
/// `fence.i` has no address range, so `va` and `len` are ignored. See
/// notes/riscv-port/hal-leaks.md.
pub fn sync_icache(va: u64, len: usize) {
    let _ = (va, len);
    instructions::fence_i();
    // `fence.i` is local and has no broadcast form, so a thread moved to another hart could fetch
    // stale bytes. TCG's coherent icache hides this; the U74 does not.
    let others = crate::smp::online_harts_mask() & !(1 << crate::cpu::id());
    if others != 0 {
        sbi_remote_fence_i(others);
    }
}
