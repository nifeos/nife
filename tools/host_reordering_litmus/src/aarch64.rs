//! Boot, console, clock and exit for the aarch64 `virt` machine, entered at EL1 by QEMU's `-kernel`.
//!
//! Every core turns the MMU on with one shared identity map before it runs Rust, so the litmus
//! accesses are to Normal, inner-shareable, write-back memory, the type the kernel's own data
//! has. The low gigabyte (the PL011) is Device-nGnRnE.
//!
//! Name: provisional, minted 2026-10-24 (UTC) by maintainer/measure-arm64-host-reordering.

use core::arch::{asm, global_asm};

/// One level-1 table: 1 GiB blocks, entry 0 Device (attr 0), entry 1 Normal WB inner-shareable
/// (attr 1). Both have AF set and EL1 read-write.
#[repr(C, align(4096))]
struct Table([u64; 512]);

#[unsafe(no_mangle)]
#[unsafe(link_section = ".data.pagetable")]
static LITMUS_L1: Table = {
    let mut t = [0u64; 512];
    t[0] = 0x0000_0000 | 0x401;
    t[1] = 0x4000_0000 | 0x705;
    Table(t)
};

global_asm!(
    r#"
    .section .text.boot, "ax"
    .global _start
_start:
    mrs     x0, mpidr_el1
    and     x0, x0, #0xff
    .global litmus_secondary
litmus_secondary:
    // x0 holds this core's index (Aff0 on the boot core, the PSCI context id on the others).
    mov     x1, #0xff00
    msr     mair_el1, x1
    ldr     x1, =0x200803519
    msr     tcr_el1, x1
    adrp    x1, LITMUS_L1
    add     x1, x1, :lo12:LITMUS_L1
    msr     ttbr0_el1, x1
    isb
    tlbi    vmalle1
    dsb     nsh
    isb
    mrs     x1, sctlr_el1
    mov     x2, #0x1005
    orr     x1, x1, x2
    msr     sctlr_el1, x1
    isb
    ldr     x1, =__stack_top
    mov     x2, #0x10000
    mul     x3, x0, x2
    sub     sp, x1, x3
    bl      litmus_main
1:  wfi
    b       1b
"#
);

unsafe extern "C" {
    fn litmus_secondary();
}

/// Starts cores `1..cores` through PSCI CPU_ON, which QEMU's `virt` serves over HVC when it boots
/// a kernel at EL1 without firmware.
pub(crate) fn start_secondaries(cores: usize) {
    for id in 1..cores {
        let mut ret: i64 = 0xC400_0003;
        // SAFETY: PSCI CPU_ON with a valid entry point and the core index as its context id; the
        // call returns to this core.
        unsafe {
            asm!(
                "hvc #0",
                inout("x0") ret,
                in("x1") id as u64,
                in("x2") litmus_secondary as *const () as u64,
                in("x3") id as u64,
                options(nostack),
            );
        }
        if ret != 0 {
            crate::console::line(format_args!("PSCI CPU_ON for core {id} returned {ret}"));
            exit(2);
        }
    }
}

pub(crate) fn putc(c: u8) {
    // SAFETY: the PL011 data register of QEMU's virt machine, identity-mapped as Device memory.
    unsafe { core::ptr::write_volatile(0x0900_0000 as *mut u32, c as u32) }
}

/// The virtual counter, in ticks of [`ticks_per_second`].
pub(crate) fn ticks() -> u64 {
    let t: u64;
    // SAFETY: reads CNTVCT_EL0, which EL1 may always read.
    unsafe { asm!("isb", "mrs {}, cntvct_el0", out(reg) t, options(nostack, nomem)) }
    t
}

pub(crate) fn ticks_per_second() -> u64 {
    let f: u64;
    // SAFETY: reads CNTFRQ_EL0.
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) f, options(nostack, nomem)) }
    f
}

/// Ends QEMU through semihosting SYS_EXIT (`-semihosting` must be on the command line).
pub(crate) fn exit(code: u32) -> ! {
    let block: [u64; 2] = [0x20026, code as u64];
    // SAFETY: semihosting SYS_EXIT with a parameter block that lives across the call.
    unsafe {
        asm!("hlt #0xf000", in("x0") 0x18u64, in("x1") &block as *const _ as u64, options(nostack));
    }
    loop {
        // SAFETY: parks the core if semihosting is off.
        unsafe { asm!("wfi", options(nostack, nomem)) }
    }
}
