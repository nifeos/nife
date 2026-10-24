//! A userspace device driver: the interrupt-driven console input driver (milestone 20).
//!
//! **A device driver is an unprivileged userspace process.** Its entire authority is an `Irq`
//! capability for the UART's interrupt (slot 0), a report endpoint for what it reads (slot 1), and
//! a device-typed mapping of the NS16550's registers at [`UART_VA`]. Its loop is the seL4
//! `IRQHandler` protocol: `WAIT` on the `Irq` until the interrupt arrives as a message; read the
//! byte through its own mapping, so the kernel is never in the data path; `SEND` it; `ACK`, which
//! re-arms the source the kernel masked when it fired (`arch::irq::enable`), without which the
//! level-triggered UART would re-fire forever. It names no architecture: `user_mode_runtime`
//! supplies the syscall ABI, and the register layout is the driver's own knowledge.

//! Name: ratified 2026-09-08 (calef, milestone 264). Shipped as `driver`, which was the fourth
//! driver in `components/src/` and the only unqualified one, beside `block_driver`, `gpu_driver` and
//! `keyboard_driver`. A reader scanning the directory met three programs that say what they drive
//! and one that does not.
//!
//! **`serial` over `uart_driver`**, on the acronym rule calef set on 2026-09-05: an acronym is
//! spelled out unless its expansion teaches nothing, and *universal asynchronous
//! receiver-transmitter* teaches a reader almost nothing useful about a program that reads console
//! input. `serial` is the plain word for the same wire and needs no expansion at all.
//!
//! **`console_driver` refused**: `console` already names a program here, and a driver named for
//! another program invites a reader to think it drives that program rather than a device.
//!
//! Two facts carried here from the provisional block this ratification replaced, because the
//! block itself is gone and they are the evidence rather than the conclusion. **The precedent
//! that settled the form** is `keyboard_driver`, which was `kbd` until 2026-08-27 and was spelled
//! out. And **`serial` is the right qualifier because there is more than one chip**: this drives
//! the PL011 on aarch64 and the NS16550 on riscv64, so naming it for either would be wrong on the
//! other, where `serial` is the plain word for the wire both speak.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{interrupt_acknowledge, interrupt_wait, send};

/// The `Irq` capability for the UART interrupt (slot 0), and the report endpoint (slot 1).
const IRQ: u64 = 0;
const REPORT: u64 = 1;

/// Where a starter maps the NS16550's registers, device-typed, in this driver's address space.
/// Nothing in the kernel starts this program; see the BUGS of milestone 890 (riscv64 and
/// `x86_64` join the shared boot path).
const UART_VA: u64 = address_space_map::pair_page(0x0070_0000);
/// NS16550 register offsets: the Receive Buffer (byte 0) and the Line Status Register (byte 5).
const RBR: usize = 0;
const LSR: usize = 5;
/// Line Status: Data Ready (a received byte is waiting).
const LSR_DR: u8 = 0b0000_0001;

#[unsafe(no_mangle)]
pub extern "C" fn _start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    loop {
        // 1. Block until the kernel delivers the UART interrupt as a message.
        interrupt_wait(IRQ);

        // 2. Read the byte from the device, quieting the receive line.
        let byte = read_uart();

        // 3. Report it. The kernel receives this; it never saw the byte itself.
        send(REPORT, byte as u64, 0, 0);

        // 4. Re-arm the interrupt now that the device is serviced.
        interrupt_acknowledge(IRQ);
    }
}

/// Read one received byte from the UART, spinning briefly until Data Ready is set. The interrupt
/// means a byte arrived, so this returns almost immediately.
fn read_uart() -> u8 {
    // SAFETY: the kernel mapped the NS16550 device-typed at UART_VA; these are its registers.
    unsafe {
        let base = UART_VA as *const u8;
        while core::ptr::read_volatile(base.add(LSR)) & LSR_DR == 0 {
            core::hint::spin_loop();
        }
        core::ptr::read_volatile(base.add(RBR))
    }
}

user_mode_runtime::panic_handler!();
