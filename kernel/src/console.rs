//! The kernel console, and `print!` / `println!`.
//!
//! There is deliberately no global mutable state here. A `Pl011` handle is just a
//! pointer, so we mint a fresh one per call rather than keeping a
//! `static mut CONSOLE`. The real state lives in the hardware, not in our memory.

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

/// The console UART's address, as the kernel sees it.
///
/// **Hardcoded on purpose, and it should stay that way.** Not a TODO.
///
/// Everywhere else we insist the machine tell us what it is rather than guessing
/// (notes/device-tree.md). The console is the one place we can't, and the reason is a
/// chicken-and-egg: the device tree parser is the code most likely to have a bug, and
/// `println!` is how you would debug it. So the console has to come up *before* the
/// device tree is parsed, which means the console cannot depend on it.
///
/// A new board needs a different constant here, and that is the correct shape: a per-board
/// early-console address, chosen at compile time, that gets us far enough to read the tree that
/// tells us everything else.
///
/// **This is a virtual address.** It lives in the kernel's direct map at `pa | KERNEL_VA_BASE`; on
/// aarch64 boot.s maps it before any Rust runs and `mmu::init` preserves it. On RISC-V the kernel
/// currently runs bare (identity map), so `phys_to_virt` is the identity until the Sv39 step.
#[cfg(not(target_arch = "x86_64"))]
const UART_BASE: usize = crate::arch::mmu::phys_to_virt(UART_PHYS) as usize;
/// `x86_64`'s console base is a port number and needs no translation; see [`UART_PORT`].
#[cfg(target_arch = "x86_64")]
const UART_BASE: usize = UART_PORT;

/// **The screen half of the console** (milestone 243), when the machine has one.
///
/// A framebuffer this kernel was told about by whoever booted it, plus the cursor walking across
/// it. Held here, inside [`KernelConsole`], rather than under a lock of its own, and that is the
/// whole reason this struct exists: **the UART lock is what serialises the screen too.** A second
/// mutex would need a rank below `rank::CONSOLE` and would be taken on every `print!` for no
/// benefit, and `force_unlock` (the panic path) would then have two locks to break instead of one.
struct Screen {
    /// The cursor and the geometry. Holds no pixels; see the `screen_console` crate.
    console: ScreenConsole,
    /// Who paints it now. See [`Painter`].
    painter: Painter,
    /// The framebuffer's address **in the kernel's direct map**, and its length in bytes.
    ///
    /// A raw address rather than a `&'static mut [u8]`, because a slice would be a live mutable
    /// borrow of device memory sitting in a `static` for the life of the machine. It is turned into
    /// a slice for the duration of one write and no longer.
    pixels: usize,
    len: usize,
}

/// **Who paints the screen** (the shell on the firmware screen, milestone 198's rung 1b).
///
/// One aperture, and at any moment exactly one process that writes it. Two painters on one
/// framebuffer interleave the way milestone 230 found two UART writers did, except that on a
/// screen the splice is pixels and nobody can read either message. So the handover is a state
/// here rather than a convention somewhere else: [`KernelConsole::write_str`] paints only while
/// this says [`Painter::Kernel`], and [`yield_screen`] is the one way out of it, which it takes
/// once.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Painter {
    /// `print!` tees into it: the boot tour, milestone 243.
    Kernel,
    /// A userspace terminal has it (`framebuffer_driver`, spawned by
    /// `kernel::user::boot_screen_terminal`). The kernel's own lines go to the UART alone, until a
    /// panic takes it back ([`reclaim_screen_for_panic`]).
    Terminal,
}

/// The kernel console: a UART, and since milestone 243 (a machine with no serial port) optionally a
/// screen.
///
/// **Both, not either.** A machine with a serial port and a monitor should say the same thing on
/// both, because the person at the bench and the gate reading the wire are looking for the same
/// line, and a console that chose one of them would be a console somebody has to configure.
struct KernelConsole {
    uart: ConsoleUart,
    screen: Option<Screen>,
    /// What the screen says, one byte a cell, which is what a scroll redraws from so that nothing
    /// ever reads the aperture back (see `screen_console::PixelSink`). **Here rather than inside
    /// [`Screen`]** because it is 36,000 bytes: as a field of this `static` it is laid down at
    /// compile time, where inside `Screen` it would be built on the 64 KiB boot stack by
    /// [`attach_screen`] and moved, which is the shape of the 16 KiB-array incident in
    /// `notes/stack.md`. It costs the same 36,000 bytes of image on every architecture, whether
    /// or not the machine has a screen.
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
        // The aperture is write-combining on `x86_64` (`arch::x86_64::mmu`), so the last few
        // stores of this line can sit in a combining buffer rather than on the screen. A fence
        // drains them, which matters exactly once: the panic path halts after its last line with
        // interrupts off, and nothing else would ever push those pixels out.
        crate::arch::direct_memory_access_write_barrier();
    }
    Ok(())
}

impl core::fmt::Write for KernelConsole {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        write_wire(&mut self.uart, &mut self.screen, &mut self.cells, s)
    }
}

/// The console.
///
/// It used to be lock-free: we minted a fresh handle per `print!`, since the handle is just a
/// pointer and the real state lives in the hardware. That was fine with no interrupts. It stops
/// being fine the moment an interrupt handler can print in the middle of somebody else's
/// `write_str`, because the UART is written **one byte at a time** and the two writers would splice
/// into each other mid-word.
///
/// SAFETY: `UART_BASE` is the documented UART address on QEMU `virt`, and nothing else in the kernel
/// touches it.
static CONSOLE: IrqSafeMutex<KernelConsole> = IrqSafeMutex::new(
    rank::CONSOLE,
    KernelConsole {
        // SAFETY: `UART_BASE` is the documented console address on every machine this kernel is
        // built for (see its own doc comment), and nothing else in the kernel touches it. This is
        // the static's initializer; it moved inside a struct in milestone 243 and the reason it is
        // `unsafe` did not change.
        uart: unsafe { ConsoleUart::new(UART_BASE) },
        screen: None,
        cells: Cells::new(),
        line: crate::kernel_log::Line::new(),
    },
);

pub fn init() {
    CONSOLE.lock().uart.init();
}

/// **This machine's console, for the machine description** (milestone 268).
///
/// One of the eight questions the description answers on every architecture, and the one that
/// answers itself: a reader who can see this line is reading it through the device it names. That
/// is not a joke at the line's expense, it is what makes it worth printing. The address here is the
/// **compile-time** constant this kernel came up on (see [`UART_BASE`]'s own doc for why the
/// console cannot ask the machine where it is), so a board whose real UART is somewhere else
/// prints nothing at all, and a board whose UART is here but whose *tree* disagrees is a board
/// where this line and the interrupt line below it are the diagnosis.
///
/// **The screen half is printed too, or its absence is** (milestone 243). A machine with a
/// framebuffer says the same thing on both surfaces, and `xenon` is why: at first light there was
/// no serial console this project could read, so the description *was* the transcript, photographed
/// off a monitor.
// The machine description and the boot self-test are the only callers, and both are
// `#[cfg(not(any(test, feature = "bench")))]`: a test boot exits through semihosting and a bench
// boot diverges into `bench::run`, so neither reads a bring-up transcript. Same treatment
// `memory::print_summary` already carries, and for the same reason.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub fn print_summary() {
    let irq = crate::memory::uart_irq();
    // The screen is read through the same lock the UART is, which is the point of `KernelConsole`:
    // one lock serialises both surfaces. Copied out and released before printing, because printing
    // takes that lock again.
    let screen = CONSOLE.lock().screen.as_ref().map(|s| (s.pixels, s.len));

    #[cfg(target_arch = "x86_64")]
    crate::print!("  console         : 16550 at i/o port {UART_BASE:#06x}");
    #[cfg(not(target_arch = "x86_64"))]
    crate::print!("  console         : {CONSOLE_KIND} at {UART_BASE:#018x}");
    match irq {
        Some(line) => crate::print!(", interrupt line {line}"),
        // Not a failure: the x86 console is polled and nothing routes its line yet, and a device
        // tree that names no `interrupts` property for the UART is a tree this kernel still boots
        // on. Saying which is the diagnosis a blank would not be.
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

/// **Start printing to a screen as well as to the UART** (milestone 243).
///
/// `found` is what the boot stage before this kernel measured and wrote into the boot handoff;
/// `virt` is where that physical framebuffer is readable, which on every caller so far is the
/// direct map. Returns the grid it came up with, in character cells, or `None` when the geometry
/// cannot hold one.
///
/// **It clears the screen**, which is the one visible side effect and is deliberate: whatever the
/// firmware left there (a vendor logo, a boot menu, the loader's own four lines) is not this
/// kernel's, and a boot tour written over a splash is a boot tour nobody can read. The cost is that
/// the loader's lines are gone by the time the kernel's first line appears, so a machine that
/// clears its screen and then says nothing has failed *between* the two, which is itself a reading.
///
/// # Safety
///
/// `virt` must name `found.span()` bytes of mapped, writable memory that is a framebuffer and not
/// anything else, for the life of the kernel. Nothing here can check that: a physical address out
/// of a boot handoff is an assertion by a previous stage, on the same footing as the memory map.
// Dead on the other two architectures until milestone 157's U-Boot framebuffer handoff gives them a
// caller. The console half is arch-neutral deliberately; what they are missing is the discovery.
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub unsafe fn attach_screen(found: Framebuffer, virt: u64) -> Option<(u32, u32)> {
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

/// **Which screen this console has, without taking it** (milestone 243).
///
/// [`yield_screen`] answers the same question and takes the screen in the same breath, which is
/// right for its one caller and wrong for a caller that has to decide *whether* to take it: a
/// refusal after the yield leaves a cleared screen nobody is painting. So the question is separable
/// and this is the separation.
///
/// `None` when there is no screen, or when a userspace terminal already has it.
pub fn peek_screen() -> Option<Framebuffer> {
    let guard = CONSOLE.lock();
    let screen = guard.screen.as_ref()?;
    if screen.painter != Painter::Kernel {
        return None;
    }
    Some(screen.console.screen())
}

/// **Stop painting the screen and say which screen it was** (the shell on the firmware screen,
/// milestone 198's rung 1b), for the one caller that hands it to a userspace terminal.
///
/// The screen is cleared first, under the same lock every `print!` takes, so the driver that paints
/// next starts from black rather than from the boot tour's last page, and so no kernel line can land
/// between the clear and the handover. After this returns the kernel writes its own lines to the
/// UART alone.
///
/// `None` when there is no screen, and **also when it has already been yielded**: a second caller
/// gets nothing, which is what makes two userspace painters unrepresentable rather than merely
/// unlikely. The caller must spawn nothing that paints the aperture until this has returned
/// `Some`.
pub fn yield_screen() -> Option<Framebuffer> {
    // **The handshake, when a host asked for one** (milestone 445). Off unless the boot command
    // line carried `machine_discovery::framebuffer::SCREEN_HOLD`, and then this is one relaxed load
    // on a path taken once per boot. See [`hold_screen_for_host`] for why the wait is out here
    // rather than inside the lock below.
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

/// **Whether [`yield_screen`] stops and asks before it clears the screen** (milestone 445).
///
/// False on every machine anybody boots for its own sake, and there is no way to set it but
/// [`hold_screen_at_handover`], which one caller reaches only when the boot command line carried
/// `machine_discovery::framebuffer::SCREEN_HOLD`. An ordinary boot pays one relaxed load, once.
static HOLD_AT_HANDOVER: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// **Ask [`yield_screen`] to hold the screen for a host before it clears it**
/// (milestone 445 (the screen check stops sampling and starts asking)), ruled in
/// §199 (the screen check asks instead of sampling).
///
/// Name: provisional (`AGENTS.md`: calef names public items).
///
/// **A debugging affordance, and the doc comment says so where a reader meets it.** Nothing in a
/// boot anybody performs calls this: the one caller is the boot-command-line reader, and the token
/// it looks for is written by a gate. Called before the handover, from the boot tour, while the
/// kernel is still single-threaded.
///
/// # Scope: one architecture arms this, and the other two have nothing to arm it for
///
/// §19 (architectural parity is a tenet; the targets are aarch64, riscv64, and `x86_64`) makes parity
/// a gate, so the gap is stated rather than left to be discovered. The
/// *mechanism* is arch-neutral: [`yield_screen`] consults the flag on all three, the wait is the
/// same code, and both console UARTs grew the receive half it needs. What is x86-only is the
/// **arming**, in `arch::x86_64::machine::attach_screen`, because that is the only architecture
/// whose boot chain hands a command line over at all.
///
/// **And the other two do not need it yet, which is the part worth checking rather than assuming.**
/// The race this exists to remove is the window between the tour being painted and [`yield_screen`]
/// clearing it. On aarch64 and riscv64 the only screen is `crate::screen`'s `ramfb`, whose pixels
/// are this kernel's own `.bss`, and `user::boot_screen_terminal` refuses to hand that to a
/// userspace driver (`screen::is_kernel_memory`) rather than grant a process a window onto kernel
/// statics. So [`yield_screen`] is never reached there, nothing ever clears the tour, and
/// `cargo xtask screen-boot` photographs a screen that will still be showing the same thing an hour
/// later. There is no window to close.
///
/// That changes when milestone 157 (real display output on the board) gives those two a firmware
/// aperture outside the kernel image: the refusal stops firing, the handover starts happening, and
/// the window appears. What is needed then is a reader for `/chosen/bootargs`, which this kernel
/// does not parse today (`kernel/build.rs` says so), and one call to this function beside it.
// Dead on the other two architectures for exactly the reason above, and the attribute is the same
// one `arch::x86_64::machine::attach_screen`'s aarch64/riscv64 twin carries in `console.rs`.
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub fn hold_screen_at_handover() {
    HOLD_AT_HANDOVER.store(true, core::sync::atomic::Ordering::Relaxed);
}

/// **Say on the serial line that the screen still holds the tour, and wait for a byte back**
/// (milestone 445).
///
/// The problem this replaces is a race nobody could win from outside. `cargo xtask uefi-boot`
/// asserts milestone 243's claim (a machine with no serial port shows its boot on its screen) by
/// photographing the framebuffer through QEMU's monitor, and every line of the tour is on that
/// framebuffer only until something replaces it: the next page of the tour scrolling up, and then
/// [`yield_screen`]'s clear. Both of those close in **guest** time, so no host-side deadline widens
/// them: on 2026-09-20 a loaded `script/test` run caught zero rows where the same leg run a minute
/// later caught 56. Sampling a transient state and hoping is rung four of `AGENTS.md`'s ladder;
/// being told when to look is rung one, because the state is no longer transient.
///
/// **The wait is bounded, and that is not optional.** A knob that can wedge a machine forever is a
/// worse defect than the one it fixes, so there are two bounds and either one ends the wait:
///
/// - `HOLD_TICKS`, ten seconds of scheduler ticks, which is the bound that means something. The
///   host's round trip is a monitor command, an asynchronous PPM write, a read and a glyph decode,
///   measured at about 50 ms on an idle machine; ten seconds is two orders of magnitude of headroom
///   for the loaded machine that broke the old gate, and short enough that a knob set by mistake on
///   a bench is a pause somebody waits out rather than a machine somebody power-cycles.
/// - `HOLD_POLLS`, a flat count of register reads, which is the backstop for a machine whose
///   timer is not ticking at all. Ticks come from the timer interrupt, and a clock that has stopped
///   would otherwise turn the first bound into no bound. Two bounds rather than one is the price of
///   not trusting a clock inside the mechanism that exists so nothing hangs.
///
/// **Outside the console lock, on purpose.** [`yield_screen`]'s lock is an `IrqSafeMutex`: holding
/// it for seconds would hold interrupts off for seconds, and the `println!` below would deadlock
/// against it. So the announcement and the wait happen first, and the clear follows the moment the
/// host answers, with nothing able to paint in between (the kernel is the only painter until
/// [`yield_screen`] says otherwise, and no other kernel line is printed here).
fn hold_screen_for_host() {
    /// Ten seconds, in scheduler ticks. `TICK_HZ` is 100 on all three architectures.
    const HOLD_TICKS: u64 = 10 * crate::arch::timer::TICK_HZ;
    /// The flat backstop, paid down only while the tick counter has not yet moved. Large enough
    /// that it is never the bound that fires on a machine whose timer works, and finite so that a
    /// machine whose timer does not still boots. Roughly a second of spinning on the dev Mac under
    /// TCG, measured at about five million iterations per ten seconds of held boot.
    const HOLD_POLLS: u32 = 1_000_000;

    // Whatever was already on the wire is not an answer to a question nobody had asked yet. One
    // drain, before the announcement, so the byte this waits for is one somebody sent on purpose.
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
        // **Park the core between polls rather than spinning, once the clock has proved itself.**
        //
        // This is measured rather than tidy. A guest spinning flat out starves the emulator's own
        // main loop, which is the thread that serves the QEMU monitor: with `spin_loop` here, every
        // `screendump` taken during the ten-second hold came back a half-written file that would not
        // decode, and the gate failed for a reason that had nothing to do with the kernel. Parking
        // gives the host the core back and the dumps decode first time.
        //
        // **Only after the tick counter has moved**, which is the whole point of `clock_alive`:
        // `wait_for_interrupt` sleeps until an interrupt arrives, so entering it on a machine whose
        // timer is dead would be the unbounded wait this function exists not to be. Until the clock
        // has demonstrated itself, this spins and pays `HOLD_POLLS` down, which is the bound that
        // covers exactly that machine.
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
    // The answering byte is taken rather than left, or the line editor that comes up on this same
    // wire a moment later would find a keystroke nobody typed at it and echo it at the prompt.
    CONSOLE.lock().uart.discard_rx();
}

/// **Take the screen back to say why the kernel is dying.** Panic path only.
///
/// On a machine with no serial port the screen is the only place a panic can be read, and a panic
/// after the shell came up would otherwise be written to a UART that is not there. So the dying
/// kernel clears the screen and paints again, and the cost is recorded rather than avoided: the
/// panic halts only the core that panicked, so a flush already in progress on another core, or one
/// a still-running shell asks for afterwards, can paint the terminal's corner of the screen over
/// the top of the panic text. That is a garbled panic where there would otherwise be none at all,
/// and the UART copy is unaffected. Stopping the other cores is the panic path's job and it does
/// not do it yet.
///
/// Call after `force_unlock`, before printing.
pub fn reclaim_screen_for_panic() {
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

/// **Re-shape the console UART from the device tree** (RISC-V; the VisionFive 2 prep,
/// notes/visionfive2.md). The JH7110's UART0 sits at QEMU's address with different silicon behind
/// it: a `DesignWare` 8250 with registers four bytes apart, 32-bit accesses, a 24 MHz clock, and the
/// DW busy quirk. Those four facts come from the serial node's `reg-shift`, `reg-io-width`,
/// `clock-frequency` and `compatible`, read here and handed to the driver in one piece.
///
/// **Called immediately after [`init`], before the first `println!`**, so no output is ever
/// produced with a stale stride; on the board, a byte read of LSR at unshifted offset 5 lands in
/// the IER word and the transmit poll spins forever, which is why the ordering matters. Between
/// `init` and this call the console is misconfigured *for the board* and correct for QEMU; nothing
/// prints in that window, and a panic inside it is one of the fault paths that were always dark
/// before the UART worked at all.
///
/// The **address** stays the compile-time constant above, deliberately (see `UART_BASE`): parsing
/// the tree for the address would make the console depend on the parser it exists to debug. This
/// function reads only the *shape*, fails toward the defaults on any absent property, and a tree
/// without the node (or an unreadable tree) changes nothing, so QEMU `virt` behaves as it always
/// has. An unstated `clock-frequency` (mainline JH7110 trees express the clock as a phandle)
/// yields divisor 0: leave the divisor and line controls exactly as U-Boot programmed them, which
/// is the correct move on a board whose firmware just printed a prompt at 115200.
#[cfg(target_arch = "riscv64")]
pub fn configure_from_dtb() {
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

/// Turn on the console UART's receive interrupt (RISC-V, milestone 20). After this the NS16550 raises
/// its line into the PLIC whenever a keystroke is waiting.
///
/// The kernel arms the device and then stays out of the way: the *byte* is read by the userspace
/// input driver, which `riscv_shell_boot` hands the NS16550's registers as a `DeviceFrame`
/// capability. There used to be a kernel-side `rx_read` here to drain it, from milestone 20 when
/// the input path was still in the kernel; milestone 41 deleted it, along with `Ns16550::read_byte`
/// and `LSR_DR`, after removing the crate-wide riscv `allow(dead_code)` showed they had no caller
/// in *any* configuration, `--features shell` included.
///
/// riscv-only: the aarch64 console stays polling, and its `ConsoleUart` (a PL011) has no such method.
///
/// Name: provisional, flagged 2026-09-25 by the lane that re-derived the x86 port falsifications
/// (design/naming/boolean-predicates-worklist.md, "`rx` and `tx`"). calef asked what `rx` stands
/// for in his #1255 review; recommended `enable_receive_interrupt`, because it enables an
/// interrupt, which `rx_enable` does not say, and matches the driver method it calls.
#[cfg(target_arch = "riscv64")]
pub fn rx_enable() {
    CONSOLE.lock().uart.enable_rx_interrupt();
}

/// **Has anyone typed at the console?** (Milestone 249.) True while a byte sits unread in the
/// console UART's receive buffer; reads nothing, so the answer stays true until [`discard_rx`]
/// takes it.
///
/// This exists for exactly one caller, `soak::watch`, and it is the escape from a self-rebooting
/// soak. The board is running a workload with no console reader, so there is no shell to type a
/// command at and no input driver holding the device; what there is, on every bench run, is a
/// serial cable and a terminal. Polling LSR turns that cable into a stop button that costs one
/// register read every five seconds and needs nothing else to be wired up.
///
/// Every architecture, since the rebooting soak reached aarch64 and `x86_64` (milestone 249 (the
/// boot lottery is sampled by a person walking to the board)'s parity half): the console is a PL011
/// on the first and a port-I/O 16550 on the second, and both drivers already carried the same sticky
/// data-ready read for milestone 445's screen hold.
///
/// Name: ratified 2026-09-24 (calef, #1255 review). Refused `rx_waiting` and `is_rx_waiting` (`rx`
/// is a decoder for "receive").
///
/// The watchdog soak (milestone 593 (a wedged kernel resets itself), number provisional) reads the same byte to halt its watchdog.
#[cfg(any(feature = "reboot_soak_test", feature = "watchdog_soak_test"))]
pub fn is_byte_waiting() -> bool {
    CONSOLE.lock().uart.is_byte_waiting()
}

/// Throw away whatever is already in the console UART's receive buffer, so that [`is_byte_waiting`]
/// answers about what arrives from now on. Called once, when a rebooting soak arms itself; see
/// `Ns16550::discard_rx` for why U-Boot's leftovers are the thing being cleared.
///
/// Name: provisional, flagged 2026-09-25 by the lane that re-derived the x86 port falsifications
/// (design/naming/boolean-predicates-worklist.md, "`rx` and `tx`"). calef asked what `rx` stands
/// for in his #1255 review; recommended `discard_waiting_bytes`, because it drops the bytes
/// `is_byte_waiting` would have reported.
#[cfg(any(feature = "reboot_soak_test", feature = "watchdog_soak_test"))]
pub fn discard_rx() {
    CONSOLE.lock().uart.discard_rx();
}

/// **Raise and lower the console UART's own interrupt line, for the RISC-V interrupt-delivery
/// tests.** Test builds only.
///
/// aarch64 raises a test interrupt with a GIC SGI, which needs no device at all. RISC-V has no SGI:
/// the only software-raised interrupt it has is the SBI's IPI, which arrives as a *software*
/// interrupt (`scause` = 1) down a different arm of the trap dispatcher than a device's, so it would
/// not exercise the PLIC claim/route/complete path at all. What it has instead is a device whose
/// line software can assert without any transfer or external stimulus: setting the 16550's
/// transmit-empty interrupt enable makes it interrupt at once, because the transmitter of a polling
/// console is always empty. Two register writes up, one down.
///
/// The console is the right device for it precisely because the kernel does not drive it by
/// interrupt: transmit is polled (`write_byte` spins on `LSR`), so an asserted transmit line has no
/// other consumer to disturb, and lowering it restores exactly the state `init` left.
///
/// See `kernel::sched::tests` and notes/interrupts.md.
#[cfg(all(any(test, feature = "system_tests"), target_arch = "riscv64"))]
pub fn raise_uart_interrupt() {
    CONSOLE.lock().uart.enable_tx_interrupt();
}

/// Quiet the line [`raise_uart_interrupt`] raised. Test builds only.
#[cfg(all(any(test, feature = "system_tests"), target_arch = "riscv64"))]
pub fn quiet_uart_interrupt() {
    CONSOLE.lock().uart.disable_interrupts();
}

/// **Read one line typed at the console**, echoing it, and answer how many bytes it holds.
///
/// Milestone 198 (a package manager, and the trivial install that makes a second customer
/// possible)'s rung 2a, and its one caller is `user::install_service`: an installer has to ask
/// before it wipes a disk, and at the point in the boot where that question belongs the kernel is
/// still the only holder of the console. After the progenitor hands the UART to the input driver
/// this must not be called, because two readers of one FIFO lose bytes between them.
///
/// **It is bounded in time, and that is what makes it safe to put on the boot path.** A stick
/// booted on a machine with nobody watching must reach the prompt rather than wait forever for an
/// answer that is not coming, so a line that has not arrived within `patience` returns `None` and
/// the caller carries on. The bound is a duration measured against the counter, not a spin count.
///
/// `\r` and `\n` both end the line; backspace and delete rub one byte out; anything that does not
/// fit in `out` is dropped rather than wrapping. Nothing here is a line editor and nothing should
/// grow into one: `crates/line_editor` is that, at EL0, where it belongs.
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
pub unsafe fn force_unlock() {
    // SAFETY: this function's own `# Safety` contract is exactly the one this call needs; it forwards, it does not weaken.
    unsafe { CONSOLE.force_unlock() }
}

/// **The panic's escape from the ring** (milestone 342, §175 (where the kernel's own output
/// goes)'s "a panic writes the UART
/// directly"). Every later line goes straight to the UART, and the lines a drainer had not yet
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

/// **A reset's escape from the ring** (milestone 805 (`reboot` at the prompt)): [`enter_panic`]'s
/// move for the same reason. A reset stops the machine as surely as a panic does, so a line left in
/// the ring for the drainer is a line nobody reads, and the reset's own lines are the only record
/// of which route was tried. Everything the drainer had not printed goes out first, then every
/// later line is direct. Unlike a panic it takes the lock the ordinary way: the caller is a
/// syscall, which holds nothing.
///
/// # BUGS
///
/// - **One way.** A firmware that refuses leaves the kernel printing direct for the rest of the
///   boot, beside the console server's own output, as the detached control does. Handing the ring
///   back would need the drainer's cursor moved past what was caught up here, and a machine whose
///   firmware refused a reset has a bigger problem than a spliced line.
pub fn enter_reset() {
    let mut guard = CONSOLE.lock();
    let con = &mut *guard;
    let (line, mut wire) = con.split();
    crate::kernel_log::enter_panic(line, |s| {
        let _ = CountedWrites(&mut wire).write_str(s);
    });
}

/// **Wait for every printed byte to leave the machine**, for milestone 592 (radon's cold reboot
/// dies in OpenSBI's PMIC write), found 2026-10-10.
///
/// Two halves, because kernel output has two places to be caught: [`kernel_log::flush`] moves what
/// an attached drainer has not printed from the ring to the wire, and the driver's
/// `drain_transmitter` then waits out the 115200-bit gap between the FIFO and the wire. `write_byte`
/// returns when the FIFO has *room*, not when bytes are sent, so code that prints and then kills the
/// machine loses the tail of its own last lines without both.
///
/// The 2026-10-09 radon bench caught it live: `arch::reboot` printed its line, called SBI SRST in
/// the next microseconds, and the PMIC dropped power mid-line, so the reset's own transcript never
/// reached the console log. Every arch reset and the board exit drain through this before the
/// instruction that may never return.
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

/// **Bytes handed to the console transmitter since boot** (first-silicon diagnostics, 2026-08-15).
///
/// Counted after each `write_str` completes, so a count here means the driver's bounded world has
/// already accepted the bytes: `write_byte`'s THRE poll is unbounded, so a wedged transmitter
/// shows as a *hanging* print, never as a count with no wire bytes behind it. The point is the
/// contrapositive at a bench: VisionFive 2 boots 7 through 9 ended with kernel state proving the
/// tour's printing steps ran while the serial record showed none of their lines, and nothing
/// could say whether the bytes were ever emitted. The riscv diag line prints this counter, so
/// boot 11's dumps carry "how much output has the kernel pushed" beside "what the wire showed".
static TX_BYTES: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// The [`TX_BYTES`] counter. Diagnostic; racy reads are fine.
///
/// **No reader since milestone 295.** The riscv diag line printed it, and that line lived inside
/// `kernel::user::riscv_initrd_demo`'s hang watcher, which went with the program it loaded. The
/// counter is still incremented on every print, so it is still true and still free to read; what is
/// gone is the thing that read it. Kept for the same reason `sched::canary` is, written there.
///
/// Name: provisional, flagged 2026-09-25 by the lane that re-derived the x86 port falsifications
/// (design/naming/boolean-predicates-worklist.md, "`rx` and `tx`"). calef asked what `rx` stands
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

/// **Route each fragment through the kernel's ring** (milestone 342): [`crate::kernel_log::route`]
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
        // What a test says about itself is evidence the harness can act on: a test that announces
        // a skip and then returns is counted as a pass, and this is where that becomes visible.
        // Test builds only. See testing::note_printed.
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
