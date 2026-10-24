//! Boot, console, clock and exit for the riscv64 `virt` machine under `-bios none`.
//!
//! With no firmware QEMU starts every hart at the image's entry in M-mode with its hart id in
//! `a0`, so there is nothing to start: [`start_secondaries`] is empty. M-mode runs on physical
//! addresses, which RISC-V's main-memory PMA makes coherent, cacheable memory.
//!
//! Name: provisional, minted 2026-10-24 (UTC) by maintainer/measure-arm64-host-reordering.

use core::arch::{asm, global_asm};

global_asm!(
    r#"
    .section .text.boot, "ax"
    .global _start
_start:
    la      t0, __stack_top
    li      t1, 0x10000
    mul     t1, t1, a0
    sub     sp, t0, t1
    call    litmus_main
1:  wfi
    j       1b
"#
);

pub(crate) fn start_secondaries(_cores: usize) {}

pub(crate) fn putc(c: u8) {
    // SAFETY: the NS16550 transmit register of QEMU's virt machine.
    unsafe { core::ptr::write_volatile(0x1000_0000 as *mut u8, c) }
}

/// The `time` CSR, in ticks of [`ticks_per_second`].
pub(crate) fn ticks() -> u64 {
    let t: u64;
    // SAFETY: reads the time CSR, which QEMU's virt backs with the ACLINT's mtime.
    unsafe { asm!("rdtime {}", out(reg) t, options(nostack, nomem)) }
    t
}

pub(crate) fn ticks_per_second() -> u64 {
    10_000_000
}

/// Ends QEMU through the SiFive test device at 0x10_0000.
pub(crate) fn exit(code: u32) -> ! {
    let value = if code == 0 {
        0x5555
    } else {
        (code << 16) | 0x3333
    };
    // SAFETY: the test finisher's register on QEMU's virt machine.
    unsafe { core::ptr::write_volatile(0x10_0000 as *mut u32, value) }
    loop {
        // SAFETY: parks the hart if the write did not end QEMU.
        unsafe { asm!("wfi", options(nostack, nomem)) }
    }
}
