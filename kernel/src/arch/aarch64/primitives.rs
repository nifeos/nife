//! **The small primitives the portable kernel calls**: halting, waiting for an interrupt, the
//! stack pointer and `SPSel`, the device write barrier and instruction-cache sync.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// Park this core forever.
///
/// `wfi`, not `wfe`: QEMU halts the vCPU on `wfi` and spins on `wfe`, which burns a host core (see
/// notes/qemu.md). Takes a [`crate::arch::HaltReason`], whose constructors are the list of who may
/// stop a core for good; a thread whose work is done leaves with `sched::exit` instead.
// A test image on this architecture never halts: its panics and its suite's end both exit through
// semihosting.
#[cfg_attr(any(test, feature = "system_tests"), allow(dead_code))]
pub fn halt(_: crate::arch::HaltReason) -> ! {
    loop {
        aarch64_cpu::asm::wfi();
    }
}

/// Wait for one interrupt, then return. **The idle thread's whole body.**
///
/// When every other thread is blocked (all waiting on I/O, say), the scheduler runs the idle
/// thread, which parks the CPU here until *something* interrupts: the timer, or the device a
/// blocked driver is waiting on. The handler may wake a thread; when `wfi` returns, the idle
/// thread yields and the scheduler picks up whatever became runnable.
///
/// `wfi`, not `wfe`, for the reason in [`halt`].
pub fn wait_for_interrupt() {
    aarch64_cpu::asm::wfi();
}

/// This core's current stack pointer. Reading `sp` is arch-specific (rule 1), so the stack-overflow
/// canary check (stack.rs) goes through here rather than embedding an `asm!` in portable code.
pub fn current_sp() -> u64 {
    instructions::read_sp()
}

/// `SPSel`, the register that says which stack pointer the name `sp` currently means at EL1:
/// bit 0 set is `SP_EL1`, clear is `SP_EL0`. Reading a system register is arch-specific (rule 1),
/// so the trap-frame sanity test (`el1_runs_on_sp_el1`, `user/el0_boundary_tests.rs`) goes through here rather
/// than embedding an `asm!` in portable code, the same shape as [`current_sp`]. Test-only because
/// nothing but that test asks, and aarch64-only on purpose: RISC-V does not bank the stack pointer
/// by privilege level, so there is no analogous register to read (notes/riscv-parity-scope.md).
#[cfg(feature = "system_tests")]
pub fn spsel() -> u64 {
    instructions::read_spsel()
}

/// Order all prior normal-memory writes before the next device (MMIO) write.
///
/// A device reads a descriptor ring by DMA, as a separate observer, so the ring stores must be
/// globally visible before the MMIO notify lands. `dsb sy` guarantees it. On QEMU DMA is coherent
/// and this is nearly free; on hardware it is load-bearing.
pub fn direct_memory_access_write_barrier() {
    aarch64_cpu::asm::barrier::dsb(aarch64_cpu::asm::barrier::SY);
}

/// Make the instruction fetcher aware of code we just wrote as data, over `[va, va+len)`.
///
/// The D-cache and the I-cache are not coherent on aarch64, by architecture. `dc cvau` cleans the
/// data cache to the point of unification, `ic ivau` invalidates the instruction cache, and the
/// barriers order the two. Without it the CPU executes whatever was in the page before. RISC-V's
/// twin is one `fence.i`; see notes/riscv-port.md, leak #3.
pub fn sync_icache(va: u64, len: usize) {
    const LINE: u64 = 64; // conservative: the real size is in CTR_EL0

    let mut p = va & !(LINE - 1);
    let end = va + len as u64;

    // SAFETY: cache maintenance on a mapped, readable range is always sound.
    unsafe {
        while p < end {
            core::arch::asm!("dc cvau, {p}", p = in(reg) p, options(nostack));
            p += LINE;
        }
        core::arch::asm!("dsb ish", options(nostack));

        let mut p = va & !(LINE - 1);
        while p < end {
            core::arch::asm!("ic ivau, {p}", p = in(reg) p, options(nostack));
            p += LINE;
        }
        core::arch::asm!("dsb ish", "isb", options(nostack));
    }
}
