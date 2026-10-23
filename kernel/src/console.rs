//! The kernel console, and `print!` / `println!`.
//!
//! One `IrqSafeMutex` holds the console UART and, where the machine has one, the screen, so a
//! `print!` from an interrupt handler cannot splice into another writer's line. See
//! notes/kernel-console.md.

use core::fmt::Write;

// The early console UART, selected by architecture at compile time. Two concrete drivers, not a
// trait: there are exactly two, they are chosen here and nowhere else, and a trait would be an
// abstraction ahead of a third requirement (AGENTS.md, rules 2/3). aarch64's `virt` has a PL011;
// RISC-V's has an NS16550. Both expose `new`/`init`/`impl Write`, so the console code below names
// neither. See notes/riscv-port.md.
#[cfg(target_arch = "riscv64")]
use crate::drivers::ns16550::Ns16550 as ConsoleUart;
#[cfg(target_arch = "aarch64")]
use crate::drivers::pl011::Pl011 as ConsoleUart;
// x86_64's `q35` (and milestone 87's OptiPlex, through its Dell C4PDJ module) has the SAME NS16550
// the RISC-V board does, at an I/O PORT rather than a memory address. That is a difference in how
// the eight registers are reached and in nothing else, so it is the same driver with a different
// register space rather than a third one; see drivers/ns16550.rs and arch/x86_64/port.rs.
#[cfg(target_arch = "x86_64")]
type ConsoleUart = crate::drivers::ns16550::Ns16550<crate::arch::PortIo>;
use machine_discovery::framebuffer::Framebuffer;
use screen_console::{Cells, PixelSink, ScreenConsole};

use crate::sync::{IrqSafeMutex, rank};

/// The console UART's **physical** address on QEMU's `virt` machine.
#[cfg(target_arch = "aarch64")]
const UART_PHYS: u64 = 0x0900_0000; // PL011
/// RISC-V's is per machine, so it lives under `arch/` (`arch::machine`), not here.
#[cfg(target_arch = "riscv64")]
const UART_PHYS: u64 = crate::arch::machine::CONSOLE_UART_PHYS; // NS16550
/// **A port number, not a physical address**, which is why it does not go through `phys_to_virt`
/// below: x86's I/O space has no page tables in front of it and nothing to translate. COM1 has been
/// at 0x3f8 since the PC/AT and is there on QEMU's `q35`.
#[cfg(target_arch = "x86_64")]
const UART_PORT: usize = crate::arch::mmu::COM1_PORT;

/// The console UART's node name in the device tree, pinned beside `UART_PHYS` and carrying its
/// address in the unit suffix. Same hardcode-with-a-witness stance as the address itself: on riscv,
/// both QEMU `virt` and the JH7110 spell UART0 exactly this way, and the fixture test
/// (`crates/machine_discovery/tests/riscv64_jh7110.rs`) is the witness; on aarch64 it is QEMU
/// `virt`'s PL011 node, witnessed by `crates/device_tree_blob/tests/qemu_aarch64_virt.rs`. Two
/// readers: this file's `configure_from_dtb` (riscv, the register shape) and `memory::init` (both,
/// the interrupt line), which is why it is `pub(crate)` rather than local to either.
#[cfg(target_arch = "aarch64")]
pub(crate) const UART_NODE: &[u8] = b"pl011@9000000";
#[cfg(target_arch = "riscv64")]
pub(crate) const UART_NODE: &[u8] = crate::arch::machine::CONSOLE_UART_NODE;
/// x86 has no device tree, so there is no node to name. The empty slice keeps the constant's shape
/// across the three architectures for the portable readers (`memory::init`); nothing on x86 looks
/// the console up by name, because ACPI does not describe a legacy COM port that way.
#[cfg(target_arch = "x86_64")]
pub(crate) const UART_NODE: &[u8] = b"";

/// The console UART's address, as the kernel sees it: a virtual address in the direct map at
/// `pa | KERNEL_VA_BASE`, which aarch64's boot.s maps before any Rust runs.
///
/// Hardcoded on purpose, not a TODO: the console must come up before the device tree parser it
/// exists to debug, so a new board needs a new constant here (notes/kernel-console.md).
#[cfg(not(target_arch = "x86_64"))]
const UART_BASE: usize = crate::arch::mmu::phys_to_virt(UART_PHYS) as usize;
/// `x86_64`'s console base is a port number and needs no translation; see [`UART_PORT`].
#[cfg(target_arch = "x86_64")]
const UART_BASE: usize = UART_PORT;

/// The screen half of the console, when the machine has one: a framebuffer this kernel was told
/// about by whoever booted it, plus the cursor walking across it. Held inside [`KernelConsole`],
/// so the UART lock serialises the screen too and the panic path has one lock to break.
struct Screen {
    /// The cursor and the geometry. Holds no pixels; see the `screen_console` crate.
    console: ScreenConsole,
    /// Who paints it now. See [`Painter`].
    painter: Painter,
    /// The framebuffer's address in the kernel's direct map, and its length in bytes. A raw
    /// address, turned into a slice for one write at a time, so no mutable borrow of device memory
    /// lives in a `static`.
    pixels: usize,
    len: usize,
}

/// Who paints the screen. Exactly one writer at a time: [`KernelConsole::write_str`] paints only
/// while this says [`Painter::Kernel`], and [`yield_screen`] is the one way out of it, taken once.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Painter {
    /// `print!` tees into it: the boot tour, milestone 243.
    Kernel,
    /// A userspace terminal has it (`framebuffer_driver`, spawned by
    /// `kernel::user::boot_screen_terminal`). The kernel's own lines go to the UART alone, until a
    /// panic takes it back ([`reclaim_screen_for_panic`]).
    Terminal,
}

/// The kernel console: a UART and, where the machine has one, a screen. Both say the same thing,
/// since the person at the bench and the gate reading the wire want the same line.
struct KernelConsole {
    uart: ConsoleUart,
    screen: Option<Screen>,
    /// What the screen says, one byte a cell, which a scroll redraws from so nothing reads the
    /// aperture back (see `screen_console::PixelSink`). Outside [`Screen`] because it is 36,000
    /// bytes: here it is laid down at compile time rather than built on the 64 KiB boot stack
    /// (`notes/stack.md`), at that cost on every architecture.
    cells: Cells,
    /// The line being assembled for the kernel's ring (milestone 342 (the kernel and the
    /// `console` server drive one UART from two address spaces)): see [`crate::kernel_log`].
    line: crate::kernel_log::Line,
}

/// Write `s` to the UART and, while the kernel paints it, the screen.
fn write_wire(
    uart: &mut ConsoleUart,
    screen: &mut Option<Screen>,
    cells: &mut Cells,
    s: &str,
) -> core::fmt::Result {
    uart.write_str(s)?;
    if let Some(screen) = screen.as_mut()
        && screen.painter == Painter::Kernel
    {
        // SAFETY: `pixels` is the direct-map address of a framebuffer whose physical range was
        // validated by `machine_discovery::framebuffer::Framebuffer::span` before it was
        // recorded, and which `arch::mmu::map_everything` maps for the life of the kernel.
        // `len` is that same span. Nothing else writes to it while the painter is the kernel:
        // the one userspace process that ever paints it is spawned only after
        // [`yield_screen`] has moved the painter off this arm.
        let bytes =
            unsafe { core::slice::from_raw_parts_mut(screen.pixels as *mut u8, screen.len) };
        screen.console.write(cells, &mut PixelSink::new(bytes), s);
        // The aperture is write-combining on `x86_64`, so a line's last stores can sit in a
        // combining buffer. The fence drains them before the panic path halts.
        crate::arch::direct_memory_access_write_barrier();
    }
    Ok(())
}

impl core::fmt::Write for KernelConsole {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        write_wire(&mut self.uart, &mut self.screen, &mut self.cells, s)
    }
}

/// The console. Locked because the UART is written one byte at a time, and an interrupt handler
/// printing inside another writer's `write_str` would splice into it mid-word.
///
/// SAFETY: `UART_BASE` is the documented UART address on QEMU `virt`, and nothing else in the kernel
/// touches it.
static CONSOLE: IrqSafeMutex<KernelConsole> = IrqSafeMutex::new(
    rank::CONSOLE,
    KernelConsole {
        // SAFETY: `UART_BASE` is the documented console address on every machine this kernel is
        // built for (see its own doc comment), and nothing else in the kernel touches it.
        uart: unsafe { ConsoleUart::new(UART_BASE) },
        screen: None,
        cells: Cells::new(),
        line: crate::kernel_log::Line::new(),
    },
);

pub fn init() {
    CONSOLE.lock().uart.init();
}

/// Print this machine's console for the machine description: the compile-time UART address and
/// interrupt line, then the screen or its absence. A board whose tree disagrees with the address
/// shows it here.
// The machine description and the boot self-test are the only callers, and neither is built for
// a test or bench boot.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub(crate) fn print_summary() {
    let irq = crate::memory::uart_irq();
    // Copied out and released before printing, because printing takes the lock again.
    let screen = CONSOLE.lock().screen.as_ref().map(|s| (s.pixels, s.len));

    #[cfg(target_arch = "x86_64")]
    crate::print!("  console         : 16550 at i/o port {UART_BASE:#06x}");
    #[cfg(not(target_arch = "x86_64"))]
    crate::print!("  console         : {CONSOLE_KIND} at {UART_BASE:#018x}");
    match irq {
        Some(line) => crate::print!(", interrupt line {line}"),
        // Not a failure: the x86 console is polled, and a tree may name no UART interrupt.
        None => crate::print!(", no interrupt line recorded"),
    }
    crate::println!();
    match screen {
        Some((pixels, len)) => crate::println!(
            "                  : and a screen, {len} bytes of framebuffer at {pixels:#018x}",
        ),
        None => crate::println!("                  : no screen; this console is the UART alone"),
    }
}

/// What the console driver is called, for the line above. x86 spells its own inline because the
/// address is a port rather than a pointer and the sentence is shaped differently.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
#[cfg(target_arch = "aarch64")]
const CONSOLE_KIND: &str = "PL011";
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
#[cfg(target_arch = "riscv64")]
const CONSOLE_KIND: &str = "NS16550";

/// Start printing to a screen as well as to the UART, and clear it.
///
/// `found` is what the previous boot stage measured and wrote into the boot handoff; `virt` is
/// where that framebuffer is readable (the direct map). Returns the grid in character cells, or
/// `None` when the geometry cannot hold one. The clear removes whatever firmware left there.
///
/// # Safety
///
/// `virt` must name `found.span()` bytes of mapped, writable framebuffer memory for the life of
/// the kernel, a previous stage's assertion that nothing here can check.
// Dead on the other two architectures until a framebuffer handoff gives them a caller.
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub(crate) unsafe fn attach_screen(found: Framebuffer, virt: u64) -> Option<(u32, u32)> {
    let mut console = ScreenConsole::new(found)?;
    let len = console.span();
    let mut guard = CONSOLE.lock();
    // SAFETY: this function's own contract, forwarded.
    let pixels = unsafe { core::slice::from_raw_parts_mut(virt as *mut u8, len) };
    console.clear(&mut guard.cells, &mut PixelSink::new(pixels));
    let size = console.size();
    guard.screen = Some(Screen {
        console,
        painter: Painter::Kernel,
        pixels: virt as usize,
        len,
    });
    Some(size)
}

/// Which screen this console has, without taking it, for a caller that must decide whether to
/// take it ([`yield_screen`] takes it too). `None` when there is no screen, or when a userspace
/// terminal already has it.
pub(crate) fn peek_screen() -> Option<Framebuffer> {
    let guard = CONSOLE.lock();
    let screen = guard.screen.as_ref()?;
    if screen.painter != Painter::Kernel {
        return None;
    }
    Some(screen.console.screen())
}

/// Stop painting the screen and say which screen it was, for the one caller that hands it to a
/// userspace terminal. Clears it under the console lock first, then writes kernel lines to the
/// UART alone.
///
/// `None` when there is no screen or it was already yielded, so a second painter gets nothing.
/// The caller must spawn nothing that paints the aperture until this has returned `Some`.
pub(crate) fn yield_screen() -> Option<Framebuffer> {
    // Outside the lock below: see [`hold_screen_for_host`].
    if HOLD_AT_HANDOVER.load(core::sync::atomic::Ordering::Relaxed) {
        hold_screen_for_host();
    }
    let mut guard = CONSOLE.lock();
    let kernel = &mut *guard;
    let screen = kernel.screen.as_mut()?;
    if screen.painter != Painter::Kernel {
        return None;
    }
    // SAFETY: as `write_str`'s: the validated span, mapped for the life of the kernel, and the
    // painter is still the kernel, so nothing else writes it.
    let bytes = unsafe { core::slice::from_raw_parts_mut(screen.pixels as *mut u8, screen.len) };
    screen
        .console
        .clear(&mut kernel.cells, &mut PixelSink::new(bytes));
    screen.painter = Painter::Terminal;
    Some(screen.console.screen())
}

/// Whether [`yield_screen`] stops and asks before it clears the screen. Set only by
/// [`hold_screen_at_handover`], when the boot command line carried
/// `machine_discovery::framebuffer::SCREEN_HOLD`.
static HOLD_AT_HANDOVER: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Ask [`yield_screen`] to hold the screen for a host before it clears it, as §199 (the screen
/// check asks instead of sampling) rules. A debugging affordance: the one caller is the
/// boot-command-line reader, and the token it looks for is written by a gate. Called from the boot
/// tour while the kernel is single-threaded. Only `x86_64` arms it; see notes/kernel-console.md,
/// "The screen hold".
///
/// Name: provisional (`AGENTS.md`: calef names public items).
// Dead on the other two architectures: nothing arms it there.
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub fn hold_screen_at_handover() {
    HOLD_AT_HANDOVER.store(true, core::sync::atomic::Ordering::Relaxed);
}

/// Say on the serial line that the screen still holds the tour, and wait for a byte back.
///
/// Bounded twice, and either bound ends the wait: `HOLD_TICKS` (ten seconds of ticks) and
/// `HOLD_POLLS` (register reads, for a machine whose timer is not ticking). Runs outside the
/// console lock, an `IrqSafeMutex`, which it takes per poll: holding it for seconds would hold
/// interrupts off, and the `println!` here would deadlock on it. See notes/kernel-console.md, "The
/// screen hold".
fn hold_screen_for_host() {
    /// Ten seconds, in scheduler ticks at whatever `TICK_HZ` the build has.
    const HOLD_TICKS: u64 = 10 * crate::arch::timer::TICK_HZ;
    /// The flat backstop, paid down only while the tick counter has not yet moved: about a second
    /// of spinning under TCG on the dev Mac.
    const HOLD_POLLS: u32 = 1_000_000;

    // Drained before the announcement, so the byte this waits for was sent in answer to it.
    CONSOLE.lock().uart.discard_rx();
    crate::println!(
        "{}send any byte on this line to release it",
        boot_ladder::SCREEN_HELD
    );

    let start = crate::arch::timer::ticks();
    let mut polls = HOLD_POLLS;
    let mut clock_alive = false;
    while polls > 0 && crate::arch::timer::ticks().wrapping_sub(start) < HOLD_TICKS {
        if CONSOLE.lock().uart.is_byte_waiting() {
            break;
        }
        // Park rather than spin: a spinning guest starves QEMU's main loop, and the monitor's
        // `screendump` comes back half-written. Only once the tick counter has moved, since a
        // dead timer would make `wait_for_interrupt` an unbounded wait.
        if !clock_alive && crate::arch::timer::ticks() != start {
            clock_alive = true;
        }
        if clock_alive {
            crate::arch::wait_for_interrupt();
        } else {
            polls -= 1;
            core::hint::spin_loop();
        }
    }
    // Taken, or the line editor on this wire would echo it at the prompt.
    CONSOLE.lock().uart.discard_rx();
}

/// Take the screen back to say why the kernel is dying. Panic path only; call after
/// `force_unlock`, before printing.
///
/// # BUGS
///
/// - Another core's terminal flush can still paint over the panic text, since a panic halts only
///   its own core. The UART copy is unaffected (notes/kernel-console.md).
pub(crate) fn reclaim_screen_for_panic() {
    let mut guard = CONSOLE.lock();
    let kernel = &mut *guard;
    let Some(screen) = kernel.screen.as_mut() else {
        return;
    };
    if screen.painter == Painter::Kernel {
        return;
    }
    // SAFETY: as `write_str`'s. The userspace painter may still be writing; see the doc above for
    // why that is accepted on this path and no other.
    let bytes = unsafe { core::slice::from_raw_parts_mut(screen.pixels as *mut u8, screen.len) };
    screen
        .console
        .clear(&mut kernel.cells, &mut PixelSink::new(bytes));
    screen.painter = Painter::Kernel;
}

/// Re-shape the console UART from the device tree (RISC-V): the serial node's `reg-shift`,
/// `reg-io-width`, `clock-frequency` and `compatible`, handed to the driver in one piece. The
/// address stays the compile-time constant. An absent property keeps its default, and a missing
/// node or unreadable tree changes nothing. An unstated clock yields divisor 0, leaving U-Boot's
/// divisor and line controls.
///
/// Must run immediately after [`init`], before the first `println!`: on the JH7110 a byte read of
/// LSR at the unshifted offset lands in the IER word and the transmit poll spins forever. See
/// notes/kernel-console.md and notes/visionfive2.md.
#[cfg(target_arch = "riscv64")]
pub(crate) fn configure_from_dtb() {
    use crate::drivers::ns16550::Shape;

    // Nothing has parsed the tree yet on this boot; the magic check inside `device_tree` is what
    // makes a garbage pointer survivable, and on failure the console simply keeps its defaults.
    let Ok(dt) = crate::device_tree() else {
        return;
    };

    let u32_prop = |name: &[u8]| -> Option<u32> {
        match dt.node_prop(UART_NODE, name) {
            Ok(Some(bytes)) if bytes.len() >= 4 => {
                Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
            }
            _ => None,
        }
    };

    let default = Shape::QEMU_VIRT;
    let shape = Shape {
        reg_shift: u32_prop(b"reg-shift").map_or(default.reg_shift, |v| v as u8),
        reg_io_width: u32_prop(b"reg-io-width").map_or(default.reg_io_width, |v| v as u8),
        // A stated clock is the only licence to reprogram the divisor; see the field's doc.
        divisor: u32_prop(b"clock-frequency").map_or(0, Shape::divisor_for),
        dw_busy_quirk: matches!(
            dt.node_prop(UART_NODE, b"compatible"),
            Ok(Some(compat)) if compat.split(|&b| b == 0).any(|s| s == b"snps,dw-apb-uart")
        ),
    };

    CONSOLE.lock().uart.configure(shape);
}

/// The register shape the userspace console and input drivers must use: they
/// map this same UART and read no device tree, so `boot_progenitor` passes this as the
/// progenitor's first start word and the progenitor hands it to both. See `abi::uart`.
#[cfg(target_arch = "riscv64")]
pub(crate) fn uart_register_shape() -> abi::uart::RegisterShape {
    let shape = CONSOLE.lock().uart.shape();
    abi::uart::RegisterShape::new(shape.reg_shift, shape.reg_io_width)
}

/// The PL011 and COM1 have no stride; the zero word says so. See the riscv64 arm.
#[cfg(not(target_arch = "riscv64"))]
pub(crate) fn uart_register_shape() -> abi::uart::RegisterShape {
    abi::uart::RegisterShape::BYTES
}

/// Has anyone typed at the console? True while a byte sits unread in the console UART's receive
/// buffer; reads nothing, so the answer stays true until [`discard_rx`] takes it.
///
/// The stop button of a self-rebooting soak (`soak::watch`), and of the watchdog soak: a serial
/// cable and one LSR read every five seconds, on every architecture.
///
/// Name: ratified 2026-09-24 (calef, #1255 review). Refused `rx_waiting` and `is_rx_waiting` (`rx`
/// is a decoder for "receive").
#[cfg(any(feature = "reboot_soak_test", soak_watchdog))]
pub fn is_byte_waiting() -> bool {
    CONSOLE.lock().uart.is_byte_waiting()
}

/// Throw away whatever is already in the console UART's receive buffer, so that [`is_byte_waiting`]
/// answers about what arrives from now on. Called once, when a rebooting soak arms itself; see
/// `Ns16550::discard_rx` for why U-Boot's leftovers are the thing being cleared.
///
/// Name: provisional, flagged 2026-09-25 by the lane that re-derived the x86 port falsifications
/// (design/naming/functions-that-answer-yes-or-no-worklist.md, "`rx` and `tx`"). calef asked what `rx` stands
/// for in his #1255 review; recommended `discard_waiting_bytes`, because it drops the bytes
/// `is_byte_waiting` would have reported.
#[cfg(any(feature = "reboot_soak_test", soak_watchdog))]
pub fn discard_rx() {
    CONSOLE.lock().uart.discard_rx();
}

/// Raise the console UART's own interrupt line, for the RISC-V interrupt-delivery tests. Test
/// builds only.
///
/// RISC-V has no SGI, and its IPI arrives as a software interrupt that skips the PLIC path. The
/// 16550's transmit-empty enable interrupts at once on a polled console, with no other consumer
/// to disturb. See notes/interrupts.md, "Testing it on RISC-V, which has no SGI".
#[cfg(all(any(test, feature = "system_tests"), target_arch = "riscv64"))]
pub(crate) fn raise_uart_interrupt() {
    CONSOLE.lock().uart.enable_tx_interrupt();
}

/// Quiet the line [`raise_uart_interrupt`] raised. Test builds only.
#[cfg(all(any(test, feature = "system_tests"), target_arch = "riscv64"))]
pub(crate) fn quiet_uart_interrupt() {
    CONSOLE.lock().uart.disable_interrupts();
}

/// Read one line typed at the console into `out`, echoing it. Returns how many bytes it holds, or
/// `None` when no line arrives within `patience` (measured against the counter), so an unattended
/// boot reaches the prompt.
///
/// `\r` and `\n` end the line, backspace and delete rub one byte out, and bytes past `out` are
/// dropped. The one caller is `user::install_service`. It must not be called once the progenitor
/// has handed the UART to the input driver: two readers of one FIFO lose bytes.
///
/// Name: provisional (milestone 198's rung 2a): calef names public items.
#[cfg(target_arch = "x86_64")]
pub fn read_line(out: &mut [u8], patience: core::time::Duration) -> Option<usize> {
    let hz = crate::arch::timer::frequency_checked()?;
    let deadline = crate::arch::timer::now() + hz * patience.as_secs();
    let mut n = 0usize;
    loop {
        // The lock is taken per byte rather than held across the wait, because holding it would
        // deadlock the first thing that tried to print a fault while a person was thinking.
        let byte = CONSOLE.lock().uart.read_byte();
        let Some(byte) = byte else {
            if crate::arch::timer::now() >= deadline {
                return None;
            }
            core::hint::spin_loop();
            continue;
        };
        match byte {
            b'\r' | b'\n' => {
                crate::println!();
                return Some(n);
            }
            0x08 | 0x7f if n > 0 => {
                n -= 1;
                crate::print!("\u{8} \u{8}");
            }
            b if (b.is_ascii_graphic() || b == b' ') && n < out.len() => {
                out[n] = b;
                n += 1;
                crate::print!("{}", b as char);
            }
            _ => {}
        }
    }
}

/// Break the console lock open. **Panic and fault paths only.**
///
/// # Safety
///
/// If we fault in the middle of a `println!`, the fault handler's own attempt to print
/// would take this lock again and hang, and we would lose the only message that mattered.
/// So the panic path breaks the lock first. Output may be spliced. That is a fine price
/// for getting the message out at all.
///
/// See sync.rs, and DECISIONS §9.
pub(crate) unsafe fn force_unlock() {
    // SAFETY: this function's own `# Safety` contract is exactly the one this call needs; it forwards, it does not weaken.
    unsafe { CONSOLE.force_unlock() }
}

/// The panic's escape from the ring, as §175 (where the kernel's own output goes) rules: "a panic
/// writes the UART directly". Every later line goes straight to the UART, and the lines a drainer
/// had not yet
/// printed go first, so the panic message is never the only thing a reader sees of what led to it.
/// Call after [`force_unlock`].
pub fn enter_panic() {
    let mut guard = CONSOLE.lock();
    let con = &mut *guard;
    let (line, mut wire) = con.split();
    crate::kernel_log::enter_panic(line, |s| {
        let _ = CountedWrites(&mut wire).write_str(s);
    });
}

/// A reset's escape from the ring, [`enter_panic`]'s move: a line left for the drainer is one
/// nobody reads once the machine stops. What the drainer had not printed goes out first, then
/// every later line is direct. Takes the lock the ordinary way; the caller is a syscall.
///
/// # BUGS
///
/// - One way. A firmware that refuses the reset leaves the kernel printing direct for the rest of
///   the boot, beside the console server's own output.
pub(crate) fn enter_reset() {
    let mut guard = CONSOLE.lock();
    let con = &mut *guard;
    let (line, mut wire) = con.split();
    crate::kernel_log::enter_panic(line, |s| {
        let _ = CountedWrites(&mut wire).write_str(s);
    });
}

/// Wait for every printed byte to leave the machine. Every arch reset and the board exit call it
/// before the instruction that may never return.
///
/// [`kernel_log::flush`](crate::kernel_log::flush) moves what a drainer has not printed from the
/// ring to the wire, then the driver's `drain_transmitter` waits for the FIFO to empty: `write_byte`
/// returns when the FIFO has room, not when bytes are sent.
///
/// Name: provisional (milestone 592, 2026-10-10): calef names public items.
pub fn drain() {
    let mut guard = CONSOLE.lock();
    let con = &mut *guard;
    let (line, mut wire) = con.split();
    crate::kernel_log::flush(line, |s| {
        let _ = CountedWrites(&mut wire).write_str(s);
    });
    wire.uart.drain_transmitter();
}

/// The ring's unread lines, written by the kernel because its drainer stopped:
/// `kernel_log::stall_tick`'s half under the console lock.
pub(crate) fn catch_up_stalled_ring() {
    let mut guard = CONSOLE.lock();
    let (_line, mut wire) = guard.split();
    crate::kernel_log::catch_up_stalled(|s| {
        let _ = CountedWrites(&mut wire).write_str(s);
    });
}

/// Bytes handed to the console transmitter since boot, counted after each `write_str` completes.
/// `write_byte`'s THRE poll is unbounded, so a wedged transmitter shows as a hanging print, never
/// as a count with no wire bytes behind it.
static TX_BYTES: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// The [`TX_BYTES`] counter. Diagnostic; racy reads are fine.
///
/// No reader; the counter is still kept true on every print, for the reason `sched::canary` is.
///
/// Name: provisional, flagged 2026-09-25 by the lane that re-derived the x86 port falsifications
/// (design/naming/functions-that-answer-yes-or-no-worklist.md, "`rx` and `tx`"). calef asked what `rx` stands
/// for in his #1255 review; recommended `bytes_written`, because it counts what the kernel wrote to
/// the console, and its static `TX_BYTES` becomes `BYTES_WRITTEN` with it.
#[allow(dead_code)]
pub fn tx_bytes() -> u64 {
    TX_BYTES.load(core::sync::atomic::Ordering::Relaxed)
}

/// The UART and the kernel's screen, without the line being assembled: what a routed byte is
/// written to.
struct Wire<'a> {
    uart: &'a mut ConsoleUart,
    screen: &'a mut Option<Screen>,
    cells: &'a mut Cells,
}

impl KernelConsole {
    fn split(&mut self) -> (&mut crate::kernel_log::Line, Wire<'_>) {
        (
            &mut self.line,
            Wire {
                uart: &mut self.uart,
                screen: &mut self.screen,
                cells: &mut self.cells,
            },
        )
    }
}

impl core::fmt::Write for Wire<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        write_wire(self.uart, self.screen, self.cells, s)
    }
}

/// Route each fragment through the kernel's ring: [`crate::kernel_log::route`]
/// decides whether it reaches the UART now, later, or by way of the log service.
struct Routed<'a>(&'a mut KernelConsole);

impl core::fmt::Write for Routed<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let (line, mut wire) = self.0.split();
        crate::kernel_log::route(line, s, |piece| {
            let _ = CountedWrites(&mut wire).write_str(piece);
        });
        Ok(())
    }
}

/// The counting shim between `write_fmt` and the driver: forwards each fragment, then counts it.
struct CountedWrites<'a, 'b>(&'a mut Wire<'b>);

impl core::fmt::Write for CountedWrites<'_, '_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.0.write_str(s)?;
        TX_BYTES.fetch_add(s.len() as u64, core::sync::atomic::Ordering::Relaxed);
        // A test that announces a skip is seen here. See testing::note_printed.
        #[cfg(any(test, feature = "system_tests"))]
        crate::testing::note_printed(s);
        Ok(())
    }
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    // Output is forward progress: it keeps the test hang-watchdog's heartbeat alive so a slow but
    // live test is not mistaken for a deadlock (test builds only; see testing::note_progress).
    #[cfg(any(test, feature = "system_tests"))]
    crate::testing::note_progress();
    // Writing to a UART cannot fail in any way we can act on, so drop the Result.
    let _ = Routed(&mut CONSOLE.lock()).write_fmt(args);
    // The lock is released: a line held for the drainer can be signalled now, if this core holds
    // nothing else (`kernel_log`'s module doc says why it cannot be done under the lock).
    crate::kernel_log::signal_if_safe();
}

/// Write formatted text to the kernel console, `core::fmt` syntax, no newline.
///
/// Takes the console lock for the duration of one call, so a message cannot be interleaved with
/// another core's. It cannot fail: a UART write has no error a kernel could act on, so the `Result`
/// is dropped in `console::_print` rather than propagated to every call site.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::_print(format_args!($($arg)*)));
}

/// [`print!`] with a trailing newline. The no-argument form writes just the newline.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[cfg(test)]
mod tests {
    //! Tests for the console.

    /// The panic path must be able to print even if the console lock is held.
    ///
    /// Otherwise a fault taken in the middle of a `println!` deadlocks in the fault
    /// handler, and we lose the one message that mattered.
    #[test_case]
    fn console_lock_can_be_busted() {
        // SAFETY: this is exactly the panic path's move, done deliberately.
        unsafe { crate::console::force_unlock() };

        // If force_unlock left the lock in a bad state, this hangs and the test times out
        // rather than failing, which is its own kind of signal.
        crate::println!("    (console still works after force_unlock)");
    }
}
