//! Runs the `jh7110_entropy` crate's board checks on a VisionFive 2 and prints the results.
//!
//! U-Boot's `booti` enters it in S-mode with `a0` the hart id and `a1` the device tree, exactly as
//! it enters the kernel, so the board's network-first boot script serves it as `nife-vf2.img` with
//! no typing at the prompt. It ungates the TRNG's clocks and releases its reset with
//! `jh7110_clock_and_reset::ENTROPY_BRING_UP`, then runs each check in [`checks`] in order and
//! prints one line per result on UART0. Every line is `board-check: <record> *<crc32>`, so the
//! checker (`results.py`) can drop a line the serial link damaged rather than misread it. When the
//! run ends it parks the hart; the board needs a power cycle before the next run.
//!
//! See notes/jh7110-entropy-board-check.md for the runbook, the record format and what each check
//! decides.
//!
//! Name: provisional.

#![no_std]
#![no_main]

mod checks;
mod console;
mod device;
#[cfg(feature = "model")]
mod model;

use core::arch::{asm, global_asm};

global_asm!(
    r#"
    .section .text.boot, "ax"
    .global _start
_start:
    # The RISC-V Linux Image header, the same 64 bytes kernel/src/arch/riscv64/boot.s emits: U-Boot's
    # booti checks the magic at 0x38 and relocates the file to RAM base plus text_offset.
    j       2f
    .word   0
    .balign 8
    .dword  0x40200000
    .dword  __image_size
    .dword  0
    .word   2
    .word   0
    .dword  0
    .ascii  "RISCV\0\0\0"
    .ascii  "RSC\x05"
    .word   0
2:
    # Only one hart runs the checks. A second hart that arrives (a firmware that starts every hart
    # at the payload) loses this race and parks, so it cannot touch the block concurrently.
    lla     t0, hart_lottery
    li      t1, 1
    amoadd.w t1, t1, (t0)
    bnez    t1, 4f
    csrci   sstatus, 2
    lla     sp, __stack_top
    lla     t0, __bss_start
    lla     t1, __bss_end
    # hart_lottery lives in .data, not .bss, so clearing .bss cannot reset the race.
3:
    bgeu    t0, t1, 5f
    sd      zero, 0(t0)
    addi    t0, t0, 8
    j       3b
5:
    lla     t0, trap_entry
    csrw    stvec, t0
    call    board_check_main
4:
    wfi
    j       4b

    .balign 4
trap_entry:
    csrr    a0, scause
    csrr    a1, sepc
    csrr    a2, stval
    call    board_check_trap
6:
    wfi
    j       6b

    .section .data
    .balign 4
hart_lottery:
    .word   0
"#
);

/// The first Rust code to run: `hart` and `device_tree` are `a0` and `a1` as `booti` passed them.
#[unsafe(no_mangle)]
extern "C" fn board_check_main(hart: u64, device_tree: u64) -> ! {
    checks::run(hart, device_tree);
    park()
}

/// Reports an exception (`scause`, `sepc`, `stval`) and parks, since nothing after a fault in a
/// test of device registers can be trusted.
#[unsafe(no_mangle)]
extern "C" fn board_check_trap(cause: u64, pc: u64, value: u64) -> ! {
    console::record(format_args!(
        "fault scause={cause:#x} sepc={pc:#x} stval={value:#x}"
    ));
    console::record(format_args!("end complete=0"));
    park()
}

/// Waits for an interrupt forever, with interrupts disabled, so the hart stops here. A model build
/// runs only under QEMU `virt`, so it first ends QEMU through the SiFive test device at `0x10_0000`.
fn park() -> ! {
    #[cfg(feature = "model")]
    // SAFETY: the test finisher's register on QEMU `virt`, the one machine a model build runs on.
    unsafe {
        core::ptr::write_volatile(0x10_0000 as *mut u32, 0x5555);
    }
    loop {
        // SAFETY: `wfi` only stalls this hart; the run is over and nothing else needs it.
        unsafe { asm!("wfi", options(nomem, nostack)) }
    }
}

/// The `time` CSR, in ticks of the device tree's `timebase-frequency`.
pub(crate) fn ticks() -> u64 {
    let t: u64;
    // SAFETY: reads the `time` CSR, which OpenSBI provides to S-mode on both machines.
    unsafe { asm!("rdtime {}", out(reg) t, options(nomem, nostack, preserves_flags)) };
    t
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    console::record(format_args!("fault panic={}", info.message()));
    console::record(format_args!("end complete=0"));
    park()
}
