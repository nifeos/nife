//! Console **input**, at EL0: the raw receive driver (milestone 19f.4, raw since milestone 28).
//!
//! The receive half of a terminal, reduced to what a driver actually is: own the UART's receive
//! side and its interrupt, and forward every byte that arrives. It assembles nothing, echoes
//! nothing, and interprets nothing; the editing, echo, and line assembly that used to live here
//! moved to the line discipline component (`line_editor`), where Unix keeps them too (a UART driver
//! feeds the tty layer; it is not the tty layer). What remains is the irreducible driver loop:
//! WAIT on the interrupt, clear it at the device, drain the FIFO, hand the bytes on, ACK.
//!
//! Bytes travel packed in the words of an `OPERATION_BYTES` CALL (up to 8 per message, the terminal
//! contract's driver half; see notes/terminal-contract.md). A keystroke is one byte and control
//! flow, not bulk data, so the words-in-registers path fits §10's rule; a paste drains in
//! 8-byte messages, and the CALL's rendezvous is the flow control that keeps a fast sender from
//! outrunning the discipline.
//!
//! Its whole authority: WRITE on the terminal endpoint (slot 0), the RX interrupt capability (slot
//! 1), and the device the machine gives it. On aarch64/riscv64 that is the UART registers mapped
//! device-typed; **on x86 (milestone 299 (the x86 port-range capability: the serial console
//! becomes a userspace driver)) it is a `PortRange` capability for COM1's ports** (slot 2), which
//! the TSS I/O bitmap reads rather than this program invoking it. It cannot print, spawn, or read
//! what anyone else typed. No role selector; the syscall runtime comes from
//! `user_mode_runtime`.
//!
//! The arch-specific part is the UART register layout, in the `uart` module below (aarch64 PL011,
//! RISC-V NS16550 by memory, x86 16550 by port I/O). The loop is one `_start` on all three: x86's
//! polled twin went with milestone 505 (an x86_64 input driver that never lets the core idle).
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
// Migrated onto `tock_registers` (milestone 139 round 5): every register offset checked at
// compile time instead of asserted by a hand-written comment, matching
// `kernel/src/drivers/pl011.rs`'s own idiom for the identical hardware. The RISC-V module below
// stays on plain volatile access, for the reason `kernel/src/drivers/ns16550.rs`'s own module doc
// gives: the NS16550's register *stride* is a runtime value (QEMU spaces its emulated registers
// one byte apart; the JH7110's real hardware spaces them four bytes apart, `reg-shift = <2>`),
// which `register_structs!`'s compile-time-fixed layout cannot express. The PL011 has no such
// knob, so it is the one half of this file the macro actually fits.
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

/// Forward wire bytes forever. No arguments: a standalone binary.
///
/// **Interrupt-driven on all three architectures**: WAIT on the receive interrupt, clear it at
/// the device, drain the FIFO, hand the bytes on, ACK. Until milestone 505 (an x86_64 input driver
/// that never lets the core idle) x86 had a polled twin, `loop { drain(); yield_now(); }`. A thread
/// that always yields is always runnable, so the run queue was never empty and the idle loop never
/// ran on that core: QEMU sat at 99 to 100% of a host core at an idle prompt (76 CPU-seconds in 78
/// wall-seconds, measured by milestone 182 (x86_64's own interactive-boot entry point)), and the
/// capability-slot gauge the idle loop prints never moved past the hand-over.
#[unsafe(no_mangle)]
pub extern "C" fn _start(uart_shape: u64, _x1: u64, _x2: u64) -> ! {
    #[cfg(target_arch = "riscv64")]
    uart::set_shape(uart_shape);
    #[cfg(not(target_arch = "riscv64"))]
    let _ = uart_shape;
    // Drain anything already in the FIFO by POLLING, before arming the interrupt: input piped in at
    // boot is sitting in the FIFO already, and the first interrupt after arming can race with it.
    // This narrows the window but does NOT close it. A line burst-piped into QEMU during the few
    // instructions between the driver starting and this drain running can still lose its leading
    // character. A real user typing after the prompt never hits it, and every line after the first is
    // interrupt-driven and intact. Fully closing it needs the driver armed before any input arrives.
    drain();
    uart::arm_rx_interrupt();

    loop {
        interrupt_wait(IRQ);
        // **Clear, then drain, never the other way round.** QEMU's PL011 raises its receive
        // interrupt only when the FIFO goes from empty to one byte (`read_trigger` is 1), and
        // `ICR` lowers it whatever the FIFO holds. Cleared after the drain, a byte that lands
        // between the drain's last empty check and the clear has its interrupt wiped while it
        // sits in the FIFO, and every byte behind it lands on a non-empty FIFO and raises
        // nothing: input stops for good. A line longer than the 16-byte FIFO arrives in two
        // pieces, so every such line opens that window (`script/swish-check --input-race`).
        // Cleared first, a byte that arrives after the drain's last check leaves the interrupt
        // raised, and the next wait returns at once.
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
