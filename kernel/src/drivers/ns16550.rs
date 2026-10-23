//! NS16550 UART driver: the RISC-V console, and `x86_64`'s COM1.
//!
//! The register block's shape is the board's ([`Shape`]): QEMU `virt` wires byte registers at
//! consecutive addresses, the JH7110's `DW_apb_uart` four bytes apart with 32-bit accesses. How the
//! registers are reached is a type parameter ([`RegisterSpace`]): memory by default, I/O ports on
//! x86 (`arch/x86_64/port.rs`). It reaches into no globals (§4). See notes/uart.md, "The NS16550
//! driver", and notes/visionfive2.md.

// Register indices from the UART base (multiply by the stride, `1 << reg_shift`, for the byte
// offset: index 5 is byte offset 5 on QEMU `virt` and byte offset 0x14 on the JH7110).
const THR: usize = 0; // Transmit Holding (write) / Receive Buffer (read); divisor low when DLAB=1.
const IER: usize = 1; // Interrupt Enable; divisor high when DLAB=1.
const FCR: usize = 2; // FIFO Control (write).
const LCR: usize = 3; // Line Control.
const LSR: usize = 5; // Line Status.

// Line Control bits.
const LCR_8N1: u8 = 0b0000_0011; // 8 data bits, no parity, one stop bit.
const LCR_DLAB: u8 = 0b1000_0000; // Divisor Latch Access Bit.

// FIFO Control bits: enable, and clear both FIFOs.
const FCR_ENABLE_CLEAR: u8 = 0b0000_0111;

// Line Status bit: Transmit Holding Register Empty (room for another byte).
const LSR_THRE: u8 = 0b0010_0000;
// Line Status bit: Transmitter Empty (holding register AND shift register drained). The DW busy
// quirk's precondition: a DW_apb_uart ignores an LCR write while it is busy, so LCR is only
// touched once the transmitter is completely idle.
const LSR_TEMT: u8 = 0b0100_0000;
// Line Status bit: Data Ready. Cleared only by reading the byte out of RBR, never by time or a
// write, so it is a mailbox a slow poll cannot miss (the soak's stop button, the screen hold).
const LSR_DR: u8 = 0b0000_0001;
// Interrupt Enable bit: Enable Transmitter Holding Register Empty Interrupt. Asserts as soon as it is
// set if LSR.THRE is already set. See `enable_tx_interrupt`. Test builds only, its one caller's.
#[cfg(any(test, feature = "system_tests"))]
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the system tests call it; a unit-test boot on some ISAs does not
const IER_ETBEI: u8 = 0b0000_0010;

/// The console's line rate, which every machine here runs at (QEMU has no real wire and does not
/// care; the VisionFive 2's serial header is documented at 115200 8N1).
const BAUD: u32 = 115_200;

/// How this particular 16550 is wired: the facts the device tree states about the register
/// block, plus what to program the divisor to. One struct so the console can swap all of it in a
/// single call, and `const`-constructible so the pre-device-tree default costs nothing.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) struct Shape {
    /// `reg-shift`: log2 of the distance in bytes between consecutive registers. 0 on QEMU `virt`,
    /// 2 on the JH7110.
    pub reg_shift: u8,
    /// `reg-io-width`: the access size in bytes, 1 or 4. Honored only when the shifted offset is
    /// 4-byte aligned (`reg_shift >= 2`), because an unaligned 32-bit volatile access would trap;
    /// any other combination falls back to byte access, which is also what a width this driver
    /// does not know (2, 8) gets.
    pub reg_io_width: u8,
    /// What to program the baud divisor to, or 0 to leave the divisor and line controls as
    /// firmware programmed them, which is right when the tree states no `clock-frequency`: a wrong
    /// guess is 1.5 Mbaud garbage (divisor 1 at 24 MHz).
    pub divisor: u16,
    /// The `DesignWare` busy quirk (`snps,dw-apb-uart`): the part ignores an LCR write while the
    /// transmitter is busy and latches a "busy" interrupt. When set, `init` drains the transmitter
    /// (LSR.TEMT, bounded) before touching LCR.
    pub dw_busy_quirk: bool,
}

impl Shape {
    /// QEMU `virt`'s wiring, and this driver's entire behavior before the VisionFive 2 prep:
    /// consecutive byte registers, and divisor 1 (115200 from the standard 1.8432 MHz UART clock),
    /// which QEMU ignores. The default until `console::configure_from_dtb` reads the real answer.
    pub(crate) const QEMU_VIRT: Shape = Shape {
        reg_shift: 0,
        reg_io_width: 1,
        divisor: 1,
        dw_busy_quirk: false,
    };

    /// The divisor for [`BAUD`] from a stated input clock, rounded to nearest: a 16550 divides the
    /// clock by 16 x divisor. 24 MHz gives 13 (actual rate 115385, 0.16% high, well inside
    /// tolerance); QEMU `virt`'s stated 3.6864 MHz gives exactly 2.
    pub(crate) const fn divisor_for(clock_hz: u32) -> u16 {
        ((clock_hz + 8 * BAUD) / (16 * BAUD)) as u16
    }
}

// The two clocks this kernel expects to meet, proved at compile time so the arithmetic cannot
// drift: the JH7110's 24 MHz and QEMU virt's 3.6864 MHz (notes/visionfive2.md).
const _: () = {
    assert!(Shape::divisor_for(24_000_000) == 13);
    assert!(Shape::divisor_for(3_686_400) == 2);
};

/// How this 16550's eight registers are reached: memory ([`Mmio`]), or the x86 I/O port space
/// (`arch/x86_64/port.rs`, since `in` and `out` are instructions). A trait, fixed per architecture
/// at compile time, so no register access pays a branch.
///
/// # Safety
/// An implementation performs raw accesses at addresses the caller supplies. Implementing it is a
/// promise that the four functions read and write exactly the width and location named, with no
/// caching, reordering or elision, which is what a device register requires and what an ordinary
/// Rust load does not promise.
pub(crate) unsafe trait RegisterSpace {
    /// Read one byte at `addr`.
    ///
    /// # Safety
    /// `addr` must name a real register of a real device.
    unsafe fn read8(addr: usize) -> u8;
    /// Write one byte at `addr`.
    ///
    /// # Safety
    /// As [`read8`](Self::read8), and the value must be one the device is meant to receive.
    unsafe fn write8(addr: usize, val: u8);
    /// Read one 32-bit word at `addr`, for the parts whose registers are four bytes wide.
    ///
    /// # Safety
    /// As [`read8`](Self::read8), plus `addr` must be 4-byte aligned.
    unsafe fn read32(addr: usize) -> u32;
    /// Write one 32-bit word at `addr`.
    ///
    /// # Safety
    /// As [`read32`](Self::read32).
    unsafe fn write32(addr: usize, val: u32);
}

/// Registers reached as memory, which is what every 16550 outside x86 looks like. The default, so
/// a bare `Ns16550` means what it has always meant.
pub(crate) struct Mmio;

// SAFETY: every access below is a `read_volatile`/`write_volatile` of the exact width named at the
// exact address given, which is the contract.
unsafe impl RegisterSpace for Mmio {
    unsafe fn read8(addr: usize) -> u8 {
        // SAFETY: the caller promises `addr` names a mapped device register.
        unsafe { core::ptr::read_volatile(addr as *const u8) }
    }
    unsafe fn write8(addr: usize, val: u8) {
        // SAFETY: as `read8`.
        unsafe { core::ptr::write_volatile(addr as *mut u8, val) }
    }
    unsafe fn read32(addr: usize) -> u32 {
        // SAFETY: as `read8`, and the caller promises 4-byte alignment.
        unsafe { core::ptr::read_volatile(addr as *const u32) }
    }
    unsafe fn write32(addr: usize, val: u32) {
        // SAFETY: as `read32`.
        unsafe { core::ptr::write_volatile(addr as *mut u32, val) }
    }
}

/// A handle to one NS16550: a base address, the block's [`Shape`], and how to reach it.
pub(crate) struct Ns16550<S: RegisterSpace = Mmio> {
    base: usize,
    shape: Shape,
    space: core::marker::PhantomData<S>,
}

// SAFETY: the base names a device, not memory Rust manages. Concurrent use is excluded by the
// console's lock, not by this type, exactly as for `Pl011`.
unsafe impl<S: RegisterSpace> Send for Ns16550<S> {}

impl<S: RegisterSpace> Ns16550<S> {
    /// # Safety
    /// `base` must be the address of a real NS16550 register block, in whichever space `S` names: a
    /// mapped physical address for [`Mmio`], an I/O port number for the x86 port space. The shape
    /// starts as [`Shape::QEMU_VIRT`]; [`configure`](Self::configure) replaces it once the device
    /// tree has been read.
    pub const unsafe fn new(base: usize) -> Self {
        Self {
            base,
            shape: Shape::QEMU_VIRT,
            space: core::marker::PhantomData,
        }
    }

    /// Adopt the shape the device tree stated and re-run [`init`](Self::init) with it. Called by
    /// `console::configure_from_dtb` under the console lock, before the first `println!`, so no
    /// output is ever produced with a stale stride.
    #[cfg_attr(target_arch = "x86_64", allow(dead_code))] // the device-tree bring-up that calls it is riscv's
    pub fn configure(&mut self, shape: Shape) {
        self.shape = shape;
        self.init();
    }

    /// The shape in force: [`Shape::QEMU_VIRT`] until [`configure`](Self::configure) replaces it.
    ///
    /// Name: provisional, minted 2026-10-20 (UTC) by milestone/890-radon-console-shape.
    #[cfg_attr(not(target_arch = "riscv64"), allow(dead_code))] // only riscv64 hands it on
    pub fn shape(&self) -> Shape {
        self.shape
    }

    /// The address of register `reg` under the current shape. Going through `usize` rather than
    /// pointer casts is the same move `drivers/plic.rs` makes: the 32-bit arm's alignment is a
    /// checked runtime fact (`off & 3 == 0`), not a static property a pointer cast could promise.
    fn reg_addr(&self, reg: usize) -> usize {
        self.base + (reg << self.shape.reg_shift)
    }

    fn read(&self, reg: usize) -> u8 {
        let addr = self.reg_addr(reg);
        // SAFETY: `reg` is one of the register indices above; the shifted offset stays within the
        // block promised by `new` (the largest, LSR at shift 2, is 0x14 of a 0x10000 block). The
        // 32-bit arm requires 4-byte alignment, checked, because an unaligned volatile u32 traps.
        unsafe {
            if self.shape.reg_io_width == 4 && addr & 3 == 0 {
                S::read32(addr) as u8
            } else {
                S::read8(addr)
            }
        }
    }

    fn write(&self, reg: usize, val: u8) {
        let addr = self.reg_addr(reg);
        // SAFETY: as `read`.
        unsafe {
            if self.shape.reg_io_width == 4 && addr & 3 == 0 {
                S::write32(addr, val as u32);
            } else {
                S::write8(addr, val);
            }
        }
    }

    /// Configure the UART: FIFOs on, interrupts off (this is a polling console), and, when the
    /// divisor is known, 8N1 at [`BAUD`].
    ///
    /// A shape with `divisor == 0` touches neither LCR nor the divisor latch (see
    /// [`Shape::divisor`]). On the DW part the transmitter is drained, bounded, before LCR is
    /// written, since the part ignores LCR writes while busy.
    pub fn init(&self) {
        self.write(IER, 0x00); // interrupts off: the console polls LSR

        if self.shape.divisor != 0 {
            if self.shape.dw_busy_quirk {
                // Bounded: it trips only on silicon not answering, and then firmware's settings
                // stay rather than the console hanging.
                let mut spins = 1_000_000u32;
                while self.read(LSR) & LSR_TEMT == 0 && spins > 0 {
                    core::hint::spin_loop();
                    spins -= 1;
                }
            }
            // Program the baud divisor behind DLAB, then drop back to 8N1 (which clears DLAB).
            self.write(LCR, LCR_DLAB);
            self.write(THR, (self.shape.divisor & 0xff) as u8); // divisor low
            self.write(IER, (self.shape.divisor >> 8) as u8); // divisor high
            self.write(LCR, LCR_8N1);
        }

        self.write(FCR, FCR_ENABLE_CLEAR);
    }

    /// Write one byte, spinning until the transmit holding register has room.
    pub(crate) fn write_byte(&self, byte: u8) {
        while self.read(LSR) & LSR_THRE == 0 {
            core::hint::spin_loop();
        }
        self.write(THR, byte);
    }

    /// Wait until every byte written has left the wire: polls `LSR_TEMT` (FIFO and shift register
    /// idle), where [`write_byte`](Self::write_byte) only waits for room. Bounded like `init`'s
    /// drain; the caller is about to reset the machine, where proceeding beats hanging.
    ///
    /// Name: provisional for milestone 592 (radon's cold reboot dies in OpenSBI's PMIC write),
    /// 2026-10-10: calef names public items.
    pub fn drain_transmitter(&self) {
        let mut spins = 1_000_000u32;
        while self.read(LSR) & LSR_TEMT == 0 && spins > 0 {
            core::hint::spin_loop();
            spins -= 1;
        }
    }

    /// Is a byte waiting to be read? Reads LSR and consumes nothing, so the answer stays true
    /// until [`discard_rx`](Self::discard_rx) takes the byte. Any byte counts; needs no interrupt.
    ///
    /// Name: ratified 2026-09-24 (calef, #1255 review), provisional from milestone 249 (the boot
    /// lottery is sampled by a person walking to the board) until then.
    /// Refused `rx_waiting` and `is_rx_waiting` (`rx` is a decoder for "receive").
    pub fn is_byte_waiting(&self) -> bool {
        self.read(LSR) & LSR_DR != 0
    }

    /// Take the byte, if one is waiting. The one caller, `kernel::console::read_line`, runs before
    /// userspace exists; once the progenitor has handed the UART to the input driver this must not
    /// be called, because two readers of one FIFO lose bytes.
    ///
    /// Name: provisional (milestone 198's rung 2a): calef names public items.
    ///
    /// Dead on riscv64, which has no install offer; allowed rather than `cfg`-ed, so it still
    /// compiles there.
    #[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
    pub fn read_byte(&self) -> Option<u8> {
        (self.read(LSR) & LSR_DR != 0).then(|| self.read(THR)) // THR on write is RBR on read.
    }

    /// Throw away everything in the receive buffer, so that what arrives after this call is what
    /// [`is_byte_waiting`](Self::is_byte_waiting) reports (U-Boot's leftovers, for a soak arming).
    /// Bounded at 64 reads, four times the FIFO depth, so a busy wire cannot hold it forever.
    ///
    /// Name: provisional since milestone 249 (the boot lottery is sampled by a person walking to
    /// the board), flagged again 2026-09-25 by the lane that re-derived the x86 port falsifications
    /// (design/naming/functions-that-answer-yes-or-no-worklist.md, "`rx` and `tx`"). calef asked what `rx`
    /// stands for in his #1255 review; recommended `discard_waiting_bytes`, because it drops the
    /// bytes `is_byte_waiting` would have reported.
    pub fn discard_rx(&self) {
        let mut bound = 64u32;
        while self.read(LSR) & LSR_DR != 0 && bound > 0 {
            let _ = self.read(THR); // THR on write is RBR on read; this is the byte.
            bound -= 1;
        }
    }

    /// Raise this UART's interrupt line: set `IER.ETBEI` with `LSR.THRE` set, which asserts at
    /// once on a polling console. For the RISC-V interrupt-delivery tests, which have no SGI
    /// (notes/interrupts.md, "Testing it on RISC-V, which has no SGI"). Test builds only.
    ///
    /// Waits, bounded, for `LSR.THRE` first: on real silicon setting `ETBEI` while THRE is 0 only
    /// arms the interrupt for the next 0-to-1 edge, which a polling console may never make. QEMU
    /// has no transmit latency and never shows it. See notes/uart.md, "The NS16550 driver".
    ///
    /// Name: provisional, flagged 2026-09-25 by the lane that re-derived the x86 port
    /// falsifications (design/naming/functions-that-answer-yes-or-no-worklist.md, "`rx` and `tx`"). calef asked
    /// what `rx` stands for in his #1255 review; recommended `enable_transmit_interrupt`.
    #[cfg(any(test, feature = "system_tests"))]
    pub fn enable_tx_interrupt(&self) {
        let mut spins = 1_000_000u32;
        while self.read(LSR) & LSR_THRE == 0 && spins > 0 {
            core::hint::spin_loop();
            spins -= 1;
        }
        self.write(IER, IER_ETBEI);
    }

    /// Mask every interrupt source in this UART, quieting a line raised by
    /// [`enable_tx_interrupt`]. The console is a polling console, so all-off is its resting state
    /// (`init` writes the same value). Test builds only, for the same reason as its partner above.
    #[cfg(any(test, feature = "system_tests"))]
    pub(crate) fn disable_interrupts(&self) {
        self.write(IER, 0x00);
    }
}

impl<S: RegisterSpace> core::fmt::Write for Ns16550<S> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            // Terminals want CRLF; Rust gives us LF.
            if byte == b'\n' {
                self.write_byte(b'\r');
            }
            self.write_byte(byte);
        }
        Ok(())
    }
}
