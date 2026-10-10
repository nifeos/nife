//! NS16550 UART driver (RISC-V console).
//!
//! The other ancient, beautifully dumb serial port. Where aarch64's `virt` machine has a PL011,
//! RISC-V's has an NS16550 (a 16550-compatible 8250) at `0x1000_0000`. Same idea, different
//! register block: eight registers, and the transmit-ready flag lives in the Line Status Register
//! instead of a Flag Register.
//!
//! **The register block's shape is the board's, not the architecture's**, and since the
//! VisionFive 2 prep (notes/visionfive2.md) this driver carries it as data ([`Shape`]) instead of
//! assuming QEMU's. QEMU `virt` wires byte-wide registers at consecutive addresses; the JH7110's
//! UART0 is a Synopsys `DesignWare` `DW_apb_uart`, an 8250 whose registers sit **four bytes apart**
//! (`reg-shift = <2>`), want **32-bit accesses** (`reg-io-width = <4>`), and run from a **24 MHz**
//! clock, so the old byte read of LSR at offset 5 landed in the middle of the IER word and the
//! transmit poll would spin on garbage forever. The shape comes from the device tree
//! (`console::configure_from_dtb`); until that runs, [`Shape::QEMU_VIRT`] keeps the behavior this
//! driver always had, byte for byte.
//!
//! This one uses plain volatile access rather than `tock_registers` register blocks: the register
//! *indices* are what the 16550 defines, and the stride between them is a runtime value no static
//! layout macro can express. Named offsets and bit masks are clearer for a device this small.
//!
//! **The register block need not be in memory at all** (milestone 161). The same 16550 that QEMU's
//! RISC-V `virt` puts at physical `0x1000_0000` is, on every x86 machine including the `OptiPlex`
//! milestone 87 tracks, at **I/O port** `0x3f8`: a separate address space reached only by the `in`
//! and `out` instructions, with no page tables in front of it. That is a difference in how the
//! eight registers are *reached* and in nothing else, so it is a type parameter
//! ([`RegisterSpace`]) rather than a second driver. The parameter defaults to [`Mmio`], so every
//! existing use of `Ns16550` means exactly what it always did.
//!
//! Splitting the access out this way is also what keeps rule #1: `in`/`out` are instructions, so
//! the port-space implementation lives under `arch/x86_64/`, not here.
//!
//! Same rule as every driver here (DECISIONS §4): **it reaches into no globals.** It is
//! constructed with a base address and a shape and knows nothing else. It is the sibling of
//! `pl011.rs`, selected by the console at compile time. See notes/riscv-port.md.

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
// Line Status bit: Data Ready. Set while at least one byte sits unread in the receive buffer or
// FIFO, and cleared only by *reading* that byte out of RBR. Nothing else clears it: it is not a
// write-one-to-clear latch and it does not time out. That is what makes it usable as a mailbox
// rather than as an event, which is the whole of milestone 249's escape: the kernel polls it once
// every five seconds and a keypress that arrived at any moment in between is still there to be
// found. Only the rebooting soak reads it; the console is otherwise transmit-only (see
// `enable_rx_interrupt`, whose whole point is that the kernel arms the line and reads nothing).
//
// Milestone 445 (the screen check stops sampling and starts asking) gave it a second reader with
// the same shape: the screen-hold handshake polls it to
// learn that a host has finished photographing the framebuffer. That is why it is no longer behind
// `reboot_soak_test`, and why the two methods below are not either.
const LSR_DR: u8 = 0b0000_0001;
// Interrupt Enable bit: Enable Received Data Available Interrupt (fires while the RX FIFO is nonempty).
// x86_64 never arms it: the callers below are the riscv console paths (`console::rx_enable`), and
// the x86 console adopts the default shape and stays polled.
#[cfg_attr(target_arch = "x86_64", allow(dead_code))]
const IER_ERBFI: u8 = 0b0000_0001;
// Interrupt Enable bit: Enable Transmitter Holding Register Empty Interrupt. Asserts as soon as it is
// set if LSR.THRE is already set, which on a polling console it always is. See `enable_tx_interrupt`.
// Test builds only, because that is where its only caller is; the crate-wide removal in
// milestone 41 (dead code: triage the suppressions) of the
// riscv `allow(dead_code)` means an unused constant here is a build error, which is the point.
#[cfg(any(test, feature = "system_tests"))]
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the system tests call it; a unit-test boot on some ISAs does not
const IER_ETBEI: u8 = 0b0000_0010;

/// The console's line rate, which every machine here runs at (QEMU has no real wire and does not
/// care; the VisionFive 2's serial header is documented at 115200 8N1).
const BAUD: u32 = 115_200;

/// **How this particular 16550 is wired**: the facts the device tree states about the register
/// block, plus what to program the divisor to. One struct so the console can swap all of it in a
/// single call, and `const`-constructible so the pre-device-tree default costs nothing.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Shape {
    /// `reg-shift`: log2 of the distance in bytes between consecutive registers. 0 on QEMU `virt`,
    /// 2 on the JH7110.
    pub reg_shift: u8,
    /// `reg-io-width`: the access size in bytes, 1 or 4. Honored only when the shifted offset is
    /// 4-byte aligned (`reg_shift >= 2`), because an unaligned 32-bit volatile access would trap;
    /// any other combination falls back to byte access, which is also what a width this driver
    /// does not know (2, 8) gets.
    pub reg_io_width: u8,
    /// What to program the baud divisor to, or **0 to leave the divisor and line controls alone**
    /// and trust whatever firmware programmed. Zero is the honest choice when the tree states no
    /// `clock-frequency`: a wrong guess is 1.5 Mbaud garbage at the far terminal (divisor 1 at
    /// 24 MHz), while U-Boot has already set 115200 8N1 on any board that showed a prompt.
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
    pub const QEMU_VIRT: Shape = Shape {
        reg_shift: 0,
        reg_io_width: 1,
        divisor: 1,
        dw_busy_quirk: false,
    };

    /// The divisor for [`BAUD`] from a stated input clock, rounded to nearest: a 16550 divides the
    /// clock by 16 x divisor. 24 MHz gives 13 (actual rate 115385, 0.16% high, well inside
    /// tolerance); QEMU `virt`'s stated 3.6864 MHz gives exactly 2.
    pub const fn divisor_for(clock_hz: u32) -> u16 {
        ((clock_hz + 8 * BAUD) / (16 * BAUD)) as u16
    }
}

// The two clocks this kernel expects to meet, proved at compile time so the arithmetic cannot
// drift: the JH7110's 24 MHz and QEMU virt's 3.6864 MHz (notes/visionfive2.md).
const _: () = {
    assert!(Shape::divisor_for(24_000_000) == 13);
    assert!(Shape::divisor_for(3_686_400) == 2);
};

/// **How this 16550's eight registers are reached**: memory, or the x86 I/O port space.
///
/// A trait rather than a runtime flag because the choice is fixed per architecture at compile time
/// and there is no call site that could want either, so a branch on every register access would buy
/// nothing. Two implementations exist: [`Mmio`] below, and the port-space one in
/// `arch/x86_64/port.rs`, which lives there because `in` and `out` are instructions (rule #1).
///
/// # Safety
/// An implementation performs raw accesses at addresses the caller supplies. Implementing it is a
/// promise that the four functions read and write exactly the width and location named, with no
/// caching, reordering or elision, which is what a device register requires and what an ordinary
/// Rust load does not promise.
pub unsafe trait RegisterSpace {
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
pub struct Mmio;

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
pub struct Ns16550<S: RegisterSpace = Mmio> {
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
    /// A shape with `divisor == 0` deliberately touches neither LCR nor the divisor latch: it
    /// means the input clock is unstated, and reprogramming against a guessed clock is how a
    /// working firmware console turns to garbage (the Shape field's doc has the arithmetic). On
    /// the DW part the busy quirk is honored first: LCR writes are ignored while the transmitter
    /// is busy, so it is drained, bounded, before being touched.
    pub fn init(&self) {
        self.write(IER, 0x00); // interrupts off: the console polls LSR

        if self.shape.divisor != 0 {
            if self.shape.dw_busy_quirk {
                // Drain the transmitter so the LCR writes below take. Bounded: at 115200 the
                // FIFO and shift register drain in low milliseconds, so a bound this size only
                // ever trips on silicon that is not answering, and then skipping the reprogram
                // (firmware's settings stay) beats hanging a console nobody can see yet.
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
    pub fn write_byte(&self, byte: u8) {
        while self.read(LSR) & LSR_THRE == 0 {
            core::hint::spin_loop();
        }
        self.write(THR, byte);
    }

    /// **Wait until every byte written has left the wire.** Polls `LSR_TEMT`, the transmitter-empty
    /// bit that says the FIFO *and* the shift register are idle, where [`write_byte`](Self::write_byte)
    /// only waits for room, so returning from a print says nothing about the bytes having been sent.
    ///
    /// This is the fix for a defect the 2026-10-09 radon bench caught: `arch::reboot` printed its
    /// line and called SBI SRST in the next microseconds, and the reset dropped power with the
    /// UART mid-line, so neither the `soak-test-reboot:` detail nor `rebooting now` ever reached
    /// the console log. The bound is `init`'s bound for `init`'s reason: at 115200 the drain takes
    /// low milliseconds, a bound this size only trips on silicon that is not answering, and the
    /// caller is about to reset the machine, where proceeding beats hanging.
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

    /// **Is a byte waiting to be read?** Reads LSR and consumes nothing, so the answer stays true
    /// until [`discard_rx`](Self::discard_rx) takes the byte.
    ///
    /// This is milestone 249's escape from a self-rebooting soak, and the two properties that make
    /// it the right mechanism are both in `LSR_DR`'s comment above: it is sticky, so a poll every
    /// five seconds cannot miss a keypress, and it needs no interrupt, no PLIC route and no
    /// userspace driver, so it works in a soak boot where the console has no reader at all.
    ///
    /// It cannot tell *which* byte arrived and deliberately does not try. Any byte stops the loop,
    /// which means nothing has to agree on a magic character: a person mashing a key in `screen`
    /// and a script writing one byte to the port are the same event.
    ///
    /// Name: ratified 2026-09-24 (calef, #1255 review), provisional from milestone 249 (the boot
    /// lottery is sampled by a person walking to the board) until then.
    /// Refused `rx_waiting` and `is_rx_waiting` (`rx` is a decoder for "receive").
    pub fn is_byte_waiting(&self) -> bool {
        self.read(LSR) & LSR_DR != 0
    }

    /// **Take the byte, if one is waiting.** [`is_byte_waiting`](Self::is_byte_waiting) with the
    /// read that consumes it, which is the pair milestone 41 deleted when the input path left the
    /// kernel and milestone 198 (a package manager, and the trivial install that makes a second
    /// customer possible)'s rung 2a needed back.
    ///
    /// **There is exactly one caller and it runs before userspace exists**: an installer's
    /// confirmation, asked on the console the kernel is still the only holder of
    /// (`kernel::console::read_line`). Once the progenitor has handed the UART to the input driver
    /// this must not be called, because two readers of one FIFO lose bytes between them and neither
    /// can tell.
    ///
    /// Name: provisional (milestone 198's rung 2a): calef names public items.
    ///
    /// Dead on riscv64, which shares this driver and has no install offer: `install_service` is
    /// `x86_64` only for the reasons stated at its declaration. Allowed rather than `cfg`-ed, so
    /// the method still compiles in every configuration; a receive path that only type-checks where
    /// it is called is one that rots everywhere else.
    #[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
    pub fn read_byte(&self) -> Option<u8> {
        (self.read(LSR) & LSR_DR != 0).then(|| self.read(THR)) // THR on write is RBR on read.
    }

    /// **Throw away everything currently in the receive buffer**, so that what arrives after this
    /// call is what [`is_byte_waiting`](Self::is_byte_waiting) reports.
    ///
    /// Called once, when a rebooting soak arms itself. Without it the first check would fire on
    /// U-Boot's leftovers rather than on a person: this board's firmware prints an autoboot
    /// countdown that anything typed at it interrupts, so a console that has been sitting in front
    /// of a human is a plausible source of a stray byte, and a stop that fires on boot 1 of 50 is a
    /// silent way to get no distribution at all. It fails safe either way, which is why it is a
    /// convenience rather than a correctness fix.
    ///
    /// Bounded, because an unbounded drain on a wire that is being written to continuously would
    /// never return. Sixteen is the 16550's FIFO depth; four times that is slack for a part with a
    /// deeper one and is still a fixed number of register reads.
    ///
    /// Name: provisional since milestone 249 (the boot lottery is sampled by a person walking to
    /// the board), flagged again 2026-09-25 by the lane that re-derived the x86 port falsifications
    /// (design/naming/boolean-predicates-worklist.md, "`rx` and `tx`"). calef asked what `rx`
    /// stands for in his #1255 review; recommended `discard_waiting_bytes`, because it drops the
    /// bytes `is_byte_waiting` would have reported.
    pub fn discard_rx(&self) {
        let mut bound = 64u32;
        while self.read(LSR) & LSR_DR != 0 && bound > 0 {
            let _ = self.read(THR); // THR on write is RBR on read; this is the byte.
            bound -= 1;
        }
    }

    /// Turn on the receive-data-available interrupt. After this, the UART raises its interrupt line
    /// (into the PLIC) whenever a byte sits unread in the RX buffer. It is **level-triggered**: the
    /// line stays asserted until the byte is read, so *something* must read the byte to quiet it
    /// before completing the interrupt, or it re-fires immediately. That something is the userspace
    /// input driver, which holds this device's registers as a capability; the kernel arms the
    /// interrupt and reads nothing. The console still polls for transmit.
    ///
    /// Name: provisional, flagged 2026-09-25 by the lane that re-derived the x86 port
    /// falsifications (design/naming/boolean-predicates-worklist.md, "`rx` and `tx`"). calef asked
    /// what `rx` stands for in his #1255 review; recommended `enable_receive_interrupt`.
    #[cfg_attr(target_arch = "x86_64", allow(dead_code))] // riscv's `console::rx_enable` is the caller; the x86 console stays polled
    pub fn enable_rx_interrupt(&self) {
        self.write(IER, IER_ERBFI);
    }

    /// Turn on the transmit-holding-register-empty interrupt, and turn it off again.
    ///
    /// A 16550 asserts its line the moment `IER.ETBEI` is set while `LSR.THRE` is already set, and
    /// on a console that has just finished printing THRE is *always* set. So this pair is a way to
    /// raise and lower this device's interrupt line with two register writes, no transfer, no
    /// external stimulus and nothing to read back. That is what `kernel::sched`'s RISC-V
    /// interrupt-delivery tests use in place of aarch64's SGI, which RISC-V has no equivalent of
    /// (notes/interrupts.md, "Testing it on RISC-V, which has no SGI").
    ///
    /// It is 16550 architecture, not a QEMU behaviour, so it should carry to any 16550-compatible
    /// part (the VisionFive 2's UART is a `DesignWare` 8250). Nothing in this kernel drives transmit
    /// by interrupt (the console polls `LSR`), so an asserted THRE line has no other consumer.
    ///
    /// Test builds only: a production caller would be a transmit-interrupt console, which this is
    /// deliberately not.
    ///
    /// **Waits for `LSR.THRE` first** (bench, 2026-08-21, VisionFive 2). A real 16550's
    /// THRE-interrupt is edge-triggered inside the chip: setting `ETBEI` while THRE is *already*
    /// 1 asserts at once, exactly as this function's doc above assumed, but setting it while THRE
    /// is still 0 only arms the interrupt for the *next* 0->1 transition, which may not come for a
    /// polling console with nothing queued to send. QEMU's model has no transmission latency, so
    /// THRE reads back 1 the instant the previous byte's write instruction retires and the race
    /// window this closes never opens there; real serial hardware still shifting out the boot
    /// banner (or, on this board, this very driver's own diagnostic prints) can have THRE at 0 for
    /// real time. Bounded the same way `init`'s busy-quirk drain is: a spin this size only ever
    /// trips on silicon not answering at all.
    ///
    /// Name: provisional, flagged 2026-09-25 by the lane that re-derived the x86 port
    /// falsifications (design/naming/boolean-predicates-worklist.md, "`rx` and `tx`"). calef asked
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
    pub fn disable_interrupts(&self) {
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
