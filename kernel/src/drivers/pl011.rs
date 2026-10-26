//! PL011 UART driver, aarch64's console.
//!
//! Constructed with a base address, it reaches into no globals (§4 (kernel shape: monolithic,
//! deferred, with two cheap rules)); whoever owns it decides where it lives. See notes/uart.md and
//! notes/portability.md.

// Register offsets from the base and the bits this driver uses, from the PrimeCell UART (PL011)
// Technical Reference Manual (Arm DDI 0183), chapter 3, "Programmers Model", under each
// register's own heading, and checked against Linux's `include/linux/amba/serial.h` (v6.16). Every
// register is 32 bits wide and reached with a 32-bit volatile access.

/// `UARTDR`: data. A write queues a byte; a read takes one.
const DR: usize = 0x00;
/// `UARTFR`: flags. Read-only.
const FR: usize = 0x18;
/// `UARTIBRD`: the integer part of the baud divisor.
const IBRD: usize = 0x24;
/// `UARTFBRD`: the fractional part of the baud divisor, in units of 1/64.
const FBRD: usize = 0x28;
/// `UARTLCR_H`: line control.
const LCR_H: usize = 0x2c;
/// `UARTCR`: control.
const CR: usize = 0x30;
/// `UARTICR`: interrupt clear, write-one-to-clear.
const ICR: usize = 0x44;

/// `UARTFR.BUSY`, bit 3: the UART is transmitting, from the FIFO or the shift register.
const FR_BUSY: u32 = 1 << 3;
/// `UARTFR.RXFE`, bit 4: the receive FIFO is empty. Clear while at least one byte is waiting in
/// `DR`, and set again only when `DR` has been read: a level, not a latch, which is what lets a poll
/// every few milliseconds serve as a mailbox. See `is_byte_waiting`.
const FR_RXFE: u32 = 1 << 4;
/// `UARTFR.TXFF`, bit 5: the transmit FIFO is full. A write to `DR` now would drop the byte.
const FR_TXFF: u32 = 1 << 5;

/// `UARTLCR_H.FEN`, bit 4: the FIFOs are on.
const LCR_H_FEN: u32 = 1 << 4;
/// `UARTLCR_H.WLEN`, bits 6:5, set to 0b11: eight data bits.
const LCR_H_WLEN_8: u32 = 0b11 << 5;

/// `UARTCR.UARTEN`, bit 0: the UART is on.
const CR_UARTEN: u32 = 1 << 0;
/// `UARTCR.TXE`, bit 8: transmit is on.
const CR_TXE: u32 = 1 << 8;
/// `UARTCR.RXE`, bit 9: receive is on.
const CR_RXE: u32 = 1 << 9;

/// Every interrupt `UARTICR` can clear, bits 10:0.
const ICR_ALL: u32 = 0x7ff;

/// A handle to one PL011.
///
/// This is just a pointer. Constructing it is free, which is why the console can
/// mint one per `print!` instead of holding global mutable state.
pub(crate) struct Pl011 {
    base: *mut u8,
}

// SAFETY: the pointer names MMIO, not memory Rust manages. Moving the handle between
// contexts is harmless; *concurrent use* is what needs excluding, and that is the lock's
// job (see console.rs), not this type's.
unsafe impl Send for Pl011 {}

impl Pl011 {
    /// # Safety
    ///
    /// `base` must be the address of a real, mapped PL011 register block.
    pub const unsafe fn new(base: usize) -> Self {
        Self {
            base: base as *mut u8,
        }
    }

    /// Read the 32-bit register at `offset`.
    fn read(&self, offset: usize) -> u32 {
        // SAFETY: `new`'s contract makes `base` a mapped PL011, and `offset` is one of the register
        // offsets above, all inside its 4 KiB block and 4-byte aligned.
        unsafe { core::ptr::read_volatile(self.base.wrapping_add(offset).cast::<u32>()) }
    }

    /// Write the 32-bit register at `offset`.
    fn write(&self, offset: usize, value: u32) {
        // SAFETY: as `read`.
        unsafe { core::ptr::write_volatile(self.base.wrapping_add(offset).cast::<u32>(), value) }
    }

    /// Configure the UART: 8 data bits, no parity, one stop bit, FIFOs on.
    pub fn init(&self) {
        // Turn the UART off while we reconfigure it.
        self.write(CR, 0);

        // Clear every pending interrupt.
        self.write(ICR, ICR_ALL);

        // Baud rate divisors. QEMU ignores these completely (there is no real wire
        // and no real clock), but a real PL011 needs them and the Raspberry Pi will.
        // For 115200 baud from a 48 MHz clock:
        //     divisor = 48_000_000 / (16 * 115_200) = 26.0416...
        //     IBRD = 26,  FBRD = round(0.0416... * 64) = 3
        self.write(IBRD, 26);
        self.write(FBRD, 3);

        self.write(LCR_H, LCR_H_WLEN_8 | LCR_H_FEN);

        self.write(CR, CR_UARTEN | CR_TXE | CR_RXE);
    }

    /// **Is a byte waiting to be read?** Reads FR and consumes nothing, so the answer stays true
    /// until [`discard_rx`](Self::discard_rx) takes the byte.
    ///
    /// The PL011 half of `console::hold_screen_for_host` (milestone 445 (the screen check stops sampling and starts asking)), and the twin of
    /// `Ns16550::is_byte_waiting`, whose comment on the equivalent NS16550 bit has the reasoning.
    /// aarch64's console is otherwise transmit-only: the *byte* on a booted machine belongs to the
    /// userspace input driver, which holds this device as a capability, and this path runs before
    /// any of that exists.
    ///
    /// Name: ratified 2026-09-24 (calef, #1255 review), provisional from milestone 445 until then.
    /// Refused `rx_waiting` and `is_rx_waiting` (`rx` is a decoder for "receive").
    pub fn is_byte_waiting(&self) -> bool {
        self.read(FR) & FR_RXFE == 0
    }

    /// **Throw away whatever is already in the receive FIFO**, so that
    /// [`is_byte_waiting`](Self::is_byte_waiting) answers about what arrives from now on.
    ///
    /// Bounded, because an unbounded drain on a wire somebody is typing into would never return.
    /// Sixteen is the PL011's FIFO depth; four times that is slack for a part with a deeper one and
    /// is still a fixed number of register reads. Same shape and same number as
    /// `Ns16550::discard_rx`, deliberately.
    ///
    /// Name: provisional since milestone 445, flagged again 2026-09-25 by the lane that re-derived
    /// the x86 port falsifications (design/naming/functions-that-answer-yes-or-no-worklist.md, "`rx` and `tx`").
    /// calef asked what `rx` stands for in his #1255 review; recommended `discard_waiting_bytes`,
    /// because it drops the bytes `is_byte_waiting` would have reported.
    pub fn discard_rx(&self) {
        let mut bound = 64u32;
        while self.read(FR) & FR_RXFE == 0 && bound > 0 {
            let _ = self.read(DR);
            bound -= 1;
        }
    }

    /// Write one byte, spinning until the transmit FIFO has room for it.
    pub(crate) fn write_byte(&self, byte: u8) {
        while self.read(FR) & FR_TXFF != 0 {
            core::hint::spin_loop();
        }
        self.write(DR, byte as u32);
    }

    /// Wait until every byte written has left the wire: polls `UARTFR.BUSY` (FIFO or shift register
    /// still transmitting), where [`write_byte`](Self::write_byte) only waits for room. The twin of
    /// `Ns16550::drain_transmitter`, bounded the same way, for `arch::reboot`'s PSCI call
    /// (notes/kernel-console.md, "Escapes from the ring").
    ///
    /// Name: provisional for milestone 592 (radon's cold reboot dies in OpenSBI's PMIC write),
    /// 2026-10-10: calef names public items.
    pub fn drain_transmitter(&self) {
        let mut spins = 1_000_000u32;
        while self.read(FR) & FR_BUSY != 0 && spins > 0 {
            core::hint::spin_loop();
            spins -= 1;
        }
    }
}

/// This is what earns us `println!("{:#x}", addr)` on bare metal.
///
/// The entire formatting engine (`{:?}`, `{:x}`, width, padding) lives in
/// `core::fmt`, which we still have without `std`. All `std` was ever contributing
/// was *somewhere for the bytes to go*. We supply that in five lines.
/// See notes/no-std.md.
impl core::fmt::Write for Pl011 {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            // Terminals want CRLF. Rust gives us LF.
            if byte == b'\n' {
                self.write_byte(b'\r');
            }
            self.write_byte(byte);
        }
        Ok(())
    }
}
