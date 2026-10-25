//! Console input, at EL0: the raw UART receive driver (step 4 of milestone 19f (run a real
//! workload), raw since milestone 28 (a solid terminal: the line discipline as a component)).
//!
//! It owns the UART's receive side and its interrupt and forwards every byte that arrives,
//! interpreting nothing. Bytes travel up to 8 per `OPERATION_BYTES` `CALL`
//! (notes/terminal-contract.md), whose rendezvous is the flow control. Its whole authority is
//! `WRITE` on the terminal endpoint ([`TERM`]), the receive interrupt ([`IRQ`]) and the device: the
//! UART registers mapped device-typed on aarch64 and riscv64, and on x86 a `PortRange` capability for COM1 (slot 2, milestone 299 (the x86 port-range
//! capability: the serial console becomes a userspace driver)). See notes/uart.md.
//!
//! Name: ratified 2026-07-30 (calef, DECISIONS §39), among the names recorded there as always
//! right.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use line_editor::proto;
use user_mode_runtime::{call, interrupt_acknowledge, interrupt_wait};

// Unused on x86_64: there is no page for it to name (`user::UART_PHYS` is zero, DECISIONS §121),
// so the arm below traps instead of reading. Kept unconditional rather than cfg'd out because the
// address is the wiring's fact, agreed with the progenitor, and hiding it on one architecture would make the
// two sides of that agreement look like two different constants.
#[cfg_attr(target_arch = "x86_64", allow(dead_code))]
const UART_VA: u64 = address_space_map::pair_page(0x0000_0000_00a0_0000);

const TERM: u64 = 0; // CALL: forward raw wire bytes to the line discipline
const IRQ: u64 = 1; // WAIT / ACK the receive interrupt

/// The UART, the one arch-specific part of an input driver. aarch64's `virt` has a PL011 (32-bit
/// registers, RX-FIFO-empty and TX-FIFO-full flags in the Flag Register, a maskable RX interrupt in
/// IMSC, an explicit interrupt-clear register ICR). RISC-V's has an NS16550 (byte registers,
/// data-ready and transmit-holding-empty in the Line Status Register, RX interrupt enabled in IER,
/// and the RX interrupt is cleared by *reading* the received byte, so there is nothing to clear).
// The PL011 half uses `tock_registers`, as `kernel/src/drivers/pl011.rs` does, so every offset is
// checked at compile time. The RISC-V half stays on volatile access because the NS16550's register
// stride is a runtime value (QEMU spaces registers one byte apart, the JH7110 four, `reg-shift =
// <2>`), which `register_structs!`'s fixed layout cannot express.
#[cfg(target_arch = "aarch64")]
mod uart {
    use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};
    use tock_registers::registers::{ReadOnly, ReadWrite, WriteOnly};
    use tock_registers::{register_bitfields, register_structs};

    use super::UART_VA;

    register_bitfields! {
        u32,
        /// Flag register.
        FR [
            /// Receive FIFO empty.
            RXFE OFFSET(4) NUMBITS(1) [],
        ],
        /// Interrupt mask set/clear register.
        IMSC [
            /// Receive interrupt mask.
            RXIM OFFSET(4) NUMBITS(1) [],
        ],
    }

    register_structs! {
        /// The PL011's memory-mapped register block, the same layout
        /// `kernel/src/drivers/pl011.rs` verifies at compile time for the identical hardware,
        /// extended with IMSC and ICR (the receive-interrupt registers the kernel's own driver
        /// does not touch, but this driver does).
        #[allow(non_snake_case)]
        RegisterBlock {
            (0x00 => DR: ReadOnly<u32>),
            (0x04 => _reserved0),
            (0x18 => FR: ReadOnly<u32, FR::Register>),
            (0x1c => _reserved1),
            (0x38 => IMSC: ReadWrite<u32, IMSC::Register>),
            (0x3c => _reserved2),
            (0x44 => ICR: WriteOnly<u32>),
            (0x48 => @END),
        }
    }

    fn regs() -> &'static RegisterBlock {
        // SAFETY: UART_VA is our device mapping of the PL011, handed to us at spawn, for the
        // whole lifetime of this process. This is the same invariant the hand-written rd/wr calls
        // used to assert by comment; register_structs! now checks every offset above at compile
        // time instead.
        unsafe { &*(UART_VA as *const RegisterBlock) }
    }

    /// Name: ratified 2026-09-24 (calef, #1255 review). Refused `rx_pending` and `is_rx_pending`
    /// (`rx` is a decoder for "receive").
    pub fn is_byte_waiting() -> bool {
        !regs().FR.is_set(FR::RXFE)
    }
    /// Name: provisional, flagged 2026-09-25 by the lane that re-derived the x86 port
    /// falsifications (design/naming/functions-that-answer-yes-or-no-worklist.md, "`rx` and `tx`"). calef asked
    /// what `rx` stands for in his #1255 review; recommended `read_byte`, because it reads the byte
    /// `is_byte_waiting` reports.
    pub fn rx_get() -> u8 {
        regs().DR.get() as u8
    }
    /// Name: provisional, flagged 2026-09-25 by the lane that re-derived the x86 port
    /// falsifications (design/naming/functions-that-answer-yes-or-no-worklist.md, "`rx` and `tx`"). calef asked
    /// what `rx` stands for in his #1255 review; recommended `arm_receive_interrupt`, the word the
    /// NS16550 and PL011 manuals spell out.
    pub fn arm_rx_interrupt() {
        regs().IMSC.modify(IMSC::RXIM::SET);
    }
    pub(crate) fn clear_interrupt() {
        regs().ICR.set(0x7ff);
    }
}

#[cfg(target_arch = "riscv64")]
mod uart {
    use core::sync::atomic::{AtomicU64, Ordering};

    use abi::uart::RegisterShape;

    use super::UART_VA;
    // Register indices; the byte offset is the index at the kernel's stride (`abi::uart`).
    const RBR: u64 = 0; // receive buffer (read)
    const IER: u64 = 1; // interrupt enable
    const LSR: u64 = 5; // line status
    const IER_ERBFI: u8 = 1 << 0; // enable received-data-available interrupt
    const LSR_DR: u8 = 1 << 0; // data ready

    /// The first start word, set before the first register access.
    static SHAPE: AtomicU64 = AtomicU64::new(0);

    /// Adopt the register shape the progenitor passed (`abi::uart`), before the first access.
    ///
    /// Name: provisional, minted 2026-10-20 (UTC) by milestone/890-radon-console-shape.
    pub fn set_shape(word: u64) {
        SHAPE.store(word, Ordering::Relaxed);
    }

    fn shape() -> RegisterShape {
        RegisterShape::from_word(SHAPE.load(Ordering::Relaxed))
    }

    fn rd(index: u64) -> u8 {
        let shape = shape();
        let addr = UART_VA + shape.offset(index);
        // SAFETY: UART_VA is our device mapping of the NS16550, and the largest offset
        // `RegisterShape` allows stays inside that page. The 32-bit arm is taken only when the
        // stride keeps every register 4-byte aligned.
        unsafe {
            if shape.is_word_access() {
                core::ptr::read_volatile(addr as *const u32) as u8
            } else {
                core::ptr::read_volatile(addr as *const u8)
            }
        }
    }
    fn wr(index: u64, v: u8) {
        let shape = shape();
        let addr = UART_VA + shape.offset(index);
        // SAFETY: as above.
        unsafe {
            if shape.is_word_access() {
                core::ptr::write_volatile(addr as *mut u32, v as u32);
            } else {
                core::ptr::write_volatile(addr as *mut u8, v);
            }
        }
    }

    /// Name: ratified 2026-09-24 (calef, #1255 review). Refused `rx_pending` and `is_rx_pending`
    /// (`rx` is a decoder for "receive").
    pub fn is_byte_waiting() -> bool {
        rd(LSR) & LSR_DR != 0
    }
    /// Name: provisional, flagged 2026-09-25 by the lane that re-derived the x86 port
    /// falsifications (design/naming/functions-that-answer-yes-or-no-worklist.md, "`rx` and `tx`"). calef asked
    /// what `rx` stands for in his #1255 review; recommended `read_byte`, because it reads the byte
    /// `is_byte_waiting` reports.
    pub fn rx_get() -> u8 {
        rd(RBR) // reading clears the receive interrupt; that is why clear_interrupt is a no-op
    }
    /// Name: provisional, flagged 2026-09-25 by the lane that re-derived the x86 port
    /// falsifications (design/naming/functions-that-answer-yes-or-no-worklist.md, "`rx` and `tx`"). calef asked
    /// what `rx` stands for in his #1255 review; recommended `arm_receive_interrupt`, the word the
    /// NS16550 and PL011 manuals spell out.
    pub fn arm_rx_interrupt() {
        wr(IER, IER_ERBFI);
    }
    pub(crate) fn clear_interrupt() {
        // The NS16550 clears the receive interrupt when the byte is read (rx_get, in drain).
    }
}

/// **The x86 twin: COM1's receive side by port I/O** (milestone 299, DECISIONS §121 reversed
/// 2026-09-15). The two arms above differ in a register layout; this one differs in kind. COM1's
/// 16550 lives at I/O ports `0x3F8..=0x3FF`, reached only by `in`/`out`, which ring 3 may execute
/// only for a port it holds a capability to. This driver holds the `(0x3F8, 8)` port range the
/// progenitor delegated it, so the kernel's TSS I/O bitmap permits these eight ports and no others.
///
/// **Interrupt-driven since milestone 505 (an x86_64 input driver that never lets the core
/// idle).** COM1's receive line is legacy IRQ 4, which the kernel routes through the IO APIC to the
/// `Irq` capability in slot 1, so this arm needs only what the riscv64 NS16550 arm has: a receive
/// enable to set and nothing to clear, since reading RBR clears the condition.
#[cfg(target_arch = "x86_64")]
mod uart {
    use user_mode_runtime::{inb, outb};
    const RBR: u16 = 0x3F8; // receive buffer (COM1 base)
    const IER: u16 = 0x3F9; // interrupt enable (base + 1)
    const MCR: u16 = 0x3FC; // modem control (base + 4)
    const LSR: u16 = 0x3FD; // line status register (base + 5)
    const IER_ERBFI: u8 = 1 << 0; // enable received-data-available interrupt
    const MCR_OUT2: u8 = 1 << 3; // gates the UART's interrupt output onto the ISA line on a PC
    const LSR_DR: u8 = 1 << 0; // data ready

    /// Name: ratified 2026-09-24 (calef, #1255 review). Refused `rx_pending` and `is_rx_pending`
    /// (`rx` is a decoder for "receive").
    pub fn is_byte_waiting() -> bool {
        inb(LSR) & LSR_DR != 0
    }
    /// Name: provisional, flagged 2026-09-25 by the lane that re-derived the x86 port
    /// falsifications (design/naming/functions-that-answer-yes-or-no-worklist.md, "`rx` and `tx`"). calef asked
    /// what `rx` stands for in his #1255 review; recommended `read_byte`, because it reads the byte
    /// `is_byte_waiting` reports.
    pub fn rx_get() -> u8 {
        inb(RBR) // reading clears the receive condition, as on the NS16550
    }
    /// Name: provisional, flagged 2026-09-25 by the lane that re-derived the x86 port
    /// falsifications (design/naming/functions-that-answer-yes-or-no-worklist.md, "`rx` and `tx`"). calef asked
    /// what `rx` stands for in his #1255 review; recommended `arm_receive_interrupt`, the word the
    /// NS16550 and PL011 manuals spell out.
    ///
    /// **`OUT2` as well as the receive enable**, which is the one PC-specific fact here: on a PC
    /// the 16550's interrupt output reaches the ISA line only through a gate the modem control
    /// register's `OUT2` bit drives, so a UART with receive interrupts enabled and `OUT2` clear
    /// raises nothing. QEMU's 16550 does not model the gate; a real COM1 (xenon's) does. Set by
    /// read-modify-write, so the console's `DTR`/`RTS` survive.
    pub fn arm_rx_interrupt() {
        outb(MCR, inb(MCR) | MCR_OUT2);
        outb(IER, IER_ERBFI);
    }
    pub(crate) fn clear_interrupt() {
        // The 16550 clears the receive interrupt when the byte is read (rx_get, in drain).
    }
}

user_mode_runtime::program_entry!(start);

/// Forward wire bytes forever, interrupt-driven on all three architectures: wait on the receive
/// interrupt, clear it at the device, drain the FIFO, hand the bytes on, acknowledge. `uart_shape`
/// is riscv64's register shape (`abi::uart`); the other arguments are unused. A polled loop would
/// never let the core idle; the measurement is in notes/uart.md.
fn start(uart_shape: u64, _x1: u64, _x2: u64) -> ! {
    #[cfg(target_arch = "riscv64")]
    uart::set_shape(uart_shape);
    #[cfg(not(target_arch = "riscv64"))]
    let _ = uart_shape;
    // Drain by polling before arming: input piped in at boot is already in the FIFO. This narrows
    // the race with the first interrupt and does not close it; a line burst-piped into QEMU before
    // this drain runs can lose its first character (notes/uart.md).
    drain();
    uart::arm_rx_interrupt();

    loop {
        interrupt_wait(IRQ);
        // Clear, then drain, never the other way round. QEMU's PL011 raises its receive interrupt
        // only when the FIFO goes from empty to one byte (`read_trigger` is 1), and `ICR` lowers it
        // whatever the FIFO holds, so a clear after the drain can wipe the interrupt of a byte that
        // landed in between, and input stops for good. A line longer than the 16-byte FIFO opens
        // that window (`script/swish-check --input-race`).
        uart::clear_interrupt(); // PL011: ICR; 16550: nothing, reading RBR clears it
        drain();
        hold_after_drain();
        interrupt_acknowledge(IRQ); // re-enable the line at the controller
    }
}

/// **The probe's hold** (`input_race_probe`, for `script/swish-check --input-race`; name
/// provisional): spin 200 ms after each drain, standing in for a host that deschedules this vCPU
/// there. The harness presses Enter 50 ms after the line, so Enter always arrives inside the
/// hold, the window a lost interrupt needs. Off in every shipped image.
#[cfg(feature = "input_race_probe")]
fn hold_after_drain() {
    const HOLD_NANOS: u64 = 200_000_000;
    let start = user_mode_runtime::monotonic_nanos();
    while user_mode_runtime::monotonic_nanos().wrapping_sub(start) < HOLD_NANOS {
        core::hint::spin_loop();
    }
}

#[cfg(not(feature = "input_race_probe"))]
fn hold_after_drain() {}

/// Read everything in the FIFO and forward it, up to 8 bytes per `OPERATION_BYTES` message, packed
/// little-endian in the second word. The CALL blocks until the discipline has taken the bytes;
/// the FIFO fills while we wait, and we drain what accumulated on return.
fn drain() {
    loop {
        let mut word: u64 = 0;
        let mut n: u64 = 0;
        while n < 8 && uart::is_byte_waiting() {
            word |= (uart::rx_get() as u64) << (8 * n);
            n += 1;
        }
        if n == 0 {
            return;
        }
        call(TERM, proto::req(proto::OPERATION_BYTES, n), word);
    }
}

user_mode_runtime::panic_handler!();
