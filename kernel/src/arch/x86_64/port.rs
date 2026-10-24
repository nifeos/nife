//! **Port I/O**: x86's second, 16-bit address space, reached only by `in` and `out`, with no page
//! tables in front of it. The legacy hardware lives there: the 8259 PICs, the PIT, the CMOS clock,
//! the 16550 COM ports, and QEMU's `isa-debug-exit`.
//!
//! A port has no page, so a port capability cannot be a mapping. Ring 3's access is gated by the
//! TSS I/O permission bitmap, one bit per port and one bitmap per core (`segments`, §121 (a device
//! capability when the device has no page: x86 port I/O)). These functions are instructions, so
//! they live under `arch/`. See notes/x86-port/cpu-state.md.

use core::arch::asm;

/// Write one byte to an I/O port.
///
/// # Safety
/// A port write is a device command. The caller must know what is at `port` and that writing `val`
/// to it is a thing the system wants to happen; there is no page table to catch a wrong guess.
pub(crate) unsafe fn out8(port: u16, val: u8) {
    // SAFETY: the caller promises the port is one it means to drive. `out` has no memory effect,
    // and no flag effect, but it is emphatically not `nomem`-in-spirit: the compiler must not
    // reorder it against the volatile accesses around it, which `options(nostack)` alone preserves
    // because inline asm without `nomem`/`readonly` is already treated as touching memory.
    unsafe { asm!("out dx, al", in("dx") port, in("al") val, options(nostack, preserves_flags)) };
}

/// Read one byte from an I/O port.
///
/// # Safety
/// As [`out8`]: the caller must know what is at `port`. A read is not automatically harmless, since
/// several legacy devices (the 8259's ISR, the 16550's receive register) have read side effects.
pub(crate) unsafe fn in8(port: u16) -> u8 {
    let val: u8;
    // SAFETY: as `out8`.
    unsafe {
        asm!("in al, dx", out("al") val, in("dx") port, options(nostack, preserves_flags));
    };
    val
}

/// Write one 16-bit word to an I/O port. The Intel TCO watchdog's registers are 16 bits wide
/// (`arch::x86_64::tco`), and a byte pair would not be the same access.
///
/// # Safety
/// As [`out8`].
#[cfg_attr(not(soak_watchdog), allow(dead_code))]
pub(crate) unsafe fn out16(port: u16, val: u16) {
    // SAFETY: as `out8`.
    unsafe { asm!("out dx, ax", in("dx") port, in("ax") val, options(nostack, preserves_flags)) };
}

/// Read one 16-bit word from an I/O port.
///
/// # Safety
/// As [`out8`].
#[cfg_attr(not(soak_watchdog), allow(dead_code))]
pub(crate) unsafe fn in16(port: u16) -> u16 {
    let val: u16;
    // SAFETY: as `out8`.
    unsafe {
        asm!("in ax, dx", out("ax") val, in("dx") port, options(nostack, preserves_flags));
    };
    val
}

/// Write one 32-bit word to an I/O port. Used by PCI configuration access through the legacy
/// 0xcf8/0xcfc pair, which is 32-bit only (`arch::x86_64::machine::enable_pcie_ecam`).
///
/// # Safety
/// As [`out8`].
pub(crate) unsafe fn out32(port: u16, val: u32) {
    // SAFETY: as `out8`.
    unsafe { asm!("out dx, eax", in("dx") port, in("eax") val, options(nostack, preserves_flags)) };
}

/// Read one 32-bit word from an I/O port.
///
/// # Safety
/// As [`out8`].
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) unsafe fn in32(port: u16) -> u32 {
    let val: u32;
    // SAFETY: as `out8`.
    unsafe {
        asm!("in eax, dx", out("eax") val, in("dx") port, options(nostack, preserves_flags));
    };
    val
}

/// **The 16550's registers reached as I/O ports**, which is where every x86 machine puts COM1.
///
/// Only the instruction differs from `drivers::ns16550::Mmio`; the register indices, stride,
/// divisor arithmetic and transmit poll are the 16550's and shared. The 32-bit accessors exist because the
/// trait has them; a port-space 16550 has `reg_shift = 0`, so `reg_io_width == 4` is never taken.
/// They are implemented rather than left to panic, since a panic in a console accessor has nowhere
/// to print itself.
pub struct PortIo;

// SAFETY: `in`/`out` reach exactly the port and width named, and the CPU does not cache, reorder or
// elide them; that is stronger than what the trait asks for.
unsafe impl crate::drivers::ns16550::RegisterSpace for PortIo {
    unsafe fn read8(addr: usize) -> u8 {
        // SAFETY: the caller promises `addr` names a real device register; in this space that means
        // a port number, which is 16 bits.
        unsafe { in8(addr as u16) }
    }
    unsafe fn write8(addr: usize, val: u8) {
        // SAFETY: as `read8`.
        unsafe { out8(addr as u16, val) }
    }
    unsafe fn read32(addr: usize) -> u32 {
        // SAFETY: as `read8`.
        unsafe { in32(addr as u16) }
    }
    unsafe fn write32(addr: usize, val: u32) {
        // SAFETY: as `read8`.
        unsafe { out32(addr as u16, val) }
    }
}
