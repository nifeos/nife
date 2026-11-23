//! **Devices**: interrupt wait and acknowledge, the reboot object, and x86 port I/O.
//!
//! The crate root re-exports every public item, so each keeps its crate-root path.
//!
//! Name: ratified 2026-11-20 (UTC); first minted, lane maintainer/files-over-1500 (#2017), 2026-10-12 (UTC), for the module and
//! the file. Named for the calls it holds, one module per seam of the old crate root; the pull
//! request records no other name considered.

use crate::system_call::invoke;

/// `WAIT` on the `Interrupt` capability in `interrupt_slot`: block until the interrupt fires. The
/// kernel masks
/// it when it fires and hands it to us as a message; nothing device-specific happens in the kernel.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2), with [`interrupt_acknowledge`] and `moho_abi::interrupt`. His words: "Ratify those
/// three." Refused `irq_wait` (round 7's provisional name; a newcomer says interrupt, per §154 (the
/// acronym test is whether the phrase is spoken)).
pub fn interrupt_wait(interrupt_slot: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates the Interrupt capability before it blocks the
    // caller.
    unsafe { invoke(interrupt_slot, moho_abi::interrupt::WAIT, 0, 0, 0) }
}

/// `ACKNOWLEDGE` the `Interrupt` capability in `interrupt_slot`: re-enable the interrupt at the generic interrupt controller (GIC) once
/// the device has
/// been quieted. Until this is called, the interrupt stays masked and cannot storm.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2), with [`interrupt_wait`]. His words: "Ratify those three." Refused `irq_ack` (round
/// 7's provisional name), `irq_acknowledge` (the alternative round 9 recommended).
pub fn interrupt_acknowledge(interrupt_slot: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates the Interrupt capability before it re-enables
    // anything.
    unsafe { invoke(interrupt_slot, moho_abi::interrupt::ACKNOWLEDGE, 0, 0, 0) }
}

/// **Invoke the reboot object in `slot`**: `moho_abi::reboot::REBOOT`. Restarting the machine is a
/// kernel object the first process hands out, so only a holder can ask.
/// Does not return when the reset works. When it does return, the answer is negative: an
/// `moho_abi::Error`: one of the four `Reset…` errors (`moho_abi::Error::ResetDenied` and its kin) when the firmware
/// refused, `NoSuchSlot` when nothing is held there.
///
/// Name: ratified 2026-11-20 (UTC); first minted, milestone 805's lane, 2026-10-06 (UTC).
pub fn reboot(slot: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates the capability before it touches the firmware.
    unsafe { invoke(slot, moho_abi::reboot::REBOOT, 0, 0, 0) }
}

/// **Write one byte to an x86 I/O port.** Not a system call: `out` is an instruction,
/// and a ring-3 program may execute it exactly when it holds a `PortRange` capability naming `port`,
/// which the kernel enforces through the task state segment (TSS) I/O permission bitmap: a device
/// with no page is a range of ports, and the capability names the range. A port it does not hold raises a general
/// protection fault, which its supervisor sees as a fault message; no page table catches a wrong
/// guess. Here rather than in each driver because an instruction belongs with the code that knows
/// the instruction set architecture (ISA).
///
/// Name: ratified 2026-11-20 (calef, #3853), replacing `outb`: the spelled-out name says what the
/// call does, and `outb` stays as a `doc(alias)`.
#[cfg(target_arch = "x86_64")]
#[doc(alias = "outb")]
pub fn port_write_byte(port: u16, value: u8) {
    // SAFETY: `out` has no memory effect and no flag effect; `nostack` holds. The caller promises it
    // holds the port capability, so the CPU permits the instruction; without it this faults, which
    // is the enforcement working rather than undefined behaviour.
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nostack, preserves_flags));
    }
}

/// **Read one byte from an x86 I/O port.** The `in` twin of [`port_write_byte`]; the same
/// capability rule and the same fault on a port the caller does not hold. A read is not automatically
/// harmless: several legacy devices (the 16550's receive register at `COM1`'s base) have read side
/// effects, which is exactly why a driver, not the kernel, decides when to issue one.
///
/// Name: ratified 2026-11-20 (calef, #3853), replacing `inb`: the spelled-out name says what the
/// call does, and `inb` stays as a `doc(alias)`.
#[cfg(target_arch = "x86_64")]
#[doc(alias = "inb")]
pub fn port_read_byte(port: u16) -> u8 {
    let value: u8;
    // SAFETY: as `port_write_byte`; the caller holds the port capability or this faults.
    unsafe {
        core::arch::asm!("in al, dx", out("al") value, in("dx") port, options(nostack, preserves_flags));
    }
    value
}
