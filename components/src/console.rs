//! The console server: a whole program in one job.
//!
//! It owns a device mapping of the PL011 UART and one request/reply channel. A client writes text
//! into a page it shares with the server, SENDs the length on the request endpoint, and the server
//! copies that many bytes to the UART and ACKs on the reply endpoint. The kernel never touches the
//! bytes, and a bad length faults the *server* (a read out of its own mapping), not the kernel.
//!
//! Its whole authority is the request endpoint (slot 0, RECEIVE), the reply endpoint (slot 1,
//! SEND), the UART registers and the shared page mapped read-only, plus the screen below.
//!

//! # A screen beside the wire (milestone 198 (a package manager)'s rung 1b)
//!
//! Started with `arg0` = [`MODE_SCREEN`], it also holds a **terminal on a screen**: slot 2 is
//! `display_terminal`'s served endpoint (`WRITE`) and [`SCREEN_OUT_VA`] maps the page that terminal
//! reads an `OPERATION_WRITE`'s bytes from. Every byte it puts on the UART it hands that terminal
//! too, so **the same stream reaches both surfaces**, as the kernel's own console does
//! (`kernel/src/console.rs`, "both, not either"). It holds no device for the screen: the pixels are
//! `display_terminal`'s and the aperture is `framebuffer_driver`'s.
//!
//! **The screen is batched, the wire is not.** Bytes reach the UART the moment they arrive, but
//! the screen paints at most once per [`SCREEN_BATCH_NANOS`] window: writes are staged into the out
//! page and one `OPERATION_WRITE` hands the batch over when the timer's deadline ends the next
//! receive, through a notification bound to this thread (`receive_bound`) and `Timer::ARM`. A paint
//! per write cost most of the `x86_64` swish leg's time under TCG. **An ack promises the wire, and
//! the screen within one window.** A spawner that passes [`MODE_SCREEN`] without the timer and
//! notification slots still gets a working console: the first refused `ARM` drops it back to one
//! paint per write.
//!

//! # Kernel lines (milestone 342 (the kernel and the `console` server drive one UART from two
//! address spaces))
//!
//! The system log service sends the kernel's lines here, sixteen bytes a message, opcode
//! `system_log_contract::console::OPERATION_KERNEL_LINE`, never acknowledged. They are written
//! only at the start of a terminal line: mid-line they wait for this console's own client to end
//! the line, or for the service's `OPERATION_FLUSH`, which puts them on a line of their own and
//! redraws the partial line beneath them.
//!
//! # BUGS
//!
//! - **A redraw replays the partial line's bytes, not the line editor's state**; a partial line
//!   longer than 512 bytes is not redrawn at all.
//! - **The batcher assumes the bind, and cannot probe it.** A spawner that granted both slots and
//!   did not bind the notification to this thread would leave batches flushing only when the page
//!   filled. Today's only `MODE_SCREEN` spawner (`system_initializer`) treats a refused bind as a
//!   refused boot.
//! - **A screen terminal that stopped answering still stalls the console**, one window at a time:
//!   the paint at the deadline is a synchronous `CALL`. The bytes reach the UART first.
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
use user_mode_runtime::{
    Received, call, cntfrq, now, receive, receive_bound, send, timer_arm, timer_cancel,
};

/// The PL011's register block, migrated onto `tock_registers` (milestone 139 round 5): every
/// offset checked at compile time instead of asserted by a hand-written comment, matching
/// `kernel/src/drivers/pl011.rs`'s own idiom for the identical hardware. Only the two registers
/// `uart_put` needs (DR, FR): the kernel's own driver configures the device at boot, so this
/// program only ever transmits. The RISC-V half below stays on plain volatile access, for the
/// reason `kernel/src/drivers/ns16550.rs`'s own module doc gives: the NS16550's register *stride*
/// is a runtime value (QEMU spaces its emulated registers one byte apart; the JH7110's real
/// hardware spaces them four bytes apart, `reg-shift = <2>`), which `register_structs!`'s
/// compile-time-fixed layout cannot express. The PL011 has no such knob, so it is the one half of
/// this file the macro actually fits.
#[cfg(target_arch = "aarch64")]
mod pl011 {
    use tock_registers::registers::{ReadOnly, WriteOnly};
    use tock_registers::{register_bitfields, register_structs};

    register_bitfields! {
        u32,
        /// Flag register.
        pub FR [
            /// Transmit FIFO full. Writing to DR while this is set would drop the byte.
            TXFF OFFSET(5) NUMBITS(1) [],
        ],
    }

    register_structs! {
        /// The PL011's memory-mapped register block, the same layout
        /// `kernel/src/drivers/pl011.rs` verifies at compile time for the identical hardware.
        #[allow(non_snake_case)]
        pub RegisterBlock {
            (0x00 => pub DR: WriteOnly<u32>),
            (0x04 => _reserved0),
            (0x18 => pub FR: ReadOnly<u32, FR::Register>),
            (0x1c => @END),
        }
    }
}

/// The request endpoint (slot 0): the server receives a byte count on it.
const REQUEST: u64 = 0;
/// The reply endpoint (slot 1): the server SENDs the acked count back on it.
const REPLY: u64 = 1;

/// The page the client writes text into, mapped read-only in the server's space. Must match what
/// the client (the progenitor, or the shell) maps and what the progenitor hands the server (`CON_SHARED_VA`).
const SHARED_VA: u64 = address_space_map::pair_page(0x0060_0000);
/// How much of it there is. One frame, which is what `console_service` maps, and the bound every
/// byte count from a client is clamped to.
const PAGE: u64 = 4096;
/// **`arg0` asking for the screen as well as the UART** (the shell on the firmware screen). `0`, the
/// only value any other boot passes, is the UART alone. Must match `crates/system_initializer`'s
/// `CONSOLE_MODE_SCREEN`: a spawn-argument convention between a parent and the one program it
/// spawns, the same kind `line_editor`'s modes are.
const MODE_SCREEN: u64 = 1;
/// `display_terminal`'s served endpoint (slot 2, `WRITE`), in [`MODE_SCREEN`] only. Ahead of the
/// batching pair and, on `x86_64`, the port range, which this process holds but never names by
/// slot.
const SCREEN: u64 = 2;
/// The notification the batching deadline ends the receive on (slot 3, `READ | WRITE`, in
/// [`MODE_SCREEN`] only), bound to this thread by the progenitor before it started us (milestone
/// 151's `BIND`: a running thread cannot bind itself). `WRITE` because `Timer::ARM` signals this
/// same object and takes the right `SIGNAL` needs.
const NOTIFIED: u64 = 3;
/// The timer that ends each batching window (slot 4, `WRITE`, in [`MODE_SCREEN`] only; milestone
/// 106).
const TIMER: u64 = 4;
/// How long the screen may lag the wire: the batching window. Two scheduler ticks (the timer's
/// resolution is the 10 ms tick, `abi::timer`), which is one to two frames at 60 Hz: a screen that
/// lands within the window reads as instant to the person it is for, while every write a line
/// generates (echo, output, prompt) folds into one paint instead of one paint per write.
const SCREEN_BATCHING_ENABLED: bool = false;
const SCREEN_BATCH_NANOS: u32 = 20_000_000;
///
/// **Off until the deadline path is root-caused, bisected `2026-09-30`.** The first bisect of the `x86_64`
/// leg's wild-transfer kill (a user thread at an address one byte into an instruction, after a
/// long multi-line answer) cleared with this flag false and the scroll and aperture fixes alone;
/// with it true the kill reproduced within one boot. The suspects are the timer's deadline
/// delivery and the notification that ends the bounded receive, not the staging math, which the
/// green run exercised in full. Ship the 3.4x; the batcher's remaining win waits on the why.
/// Where the page `display_terminal` reads an `OPERATION_WRITE`'s bytes from is mapped, in [`MODE_SCREEN`]
/// only. Must match `crates/system_initializer`'s `CON_SCREEN_OUT_VA`.
const SCREEN_OUT_VA: u64 = address_space_map::pair_page(0x0068_0000);

/// The server's device mapping of the UART registers. Must match the progenitor's `CON_UART_VA`.
// Unused on x86_64: there is no page for it to name (`user::UART_PHYS` is zero, DECISIONS §121),
// so the arm below traps instead of reading. Kept unconditional rather than cfg'd out because the
// address is the wiring's fact, agreed with the progenitor, and hiding it on one architecture would make the
// two sides of that agreement look like two different constants.
#[cfg_attr(target_arch = "x86_64", allow(dead_code))]
const UART_VA: u64 = address_space_map::pair_page(0x0070_0000);

#[unsafe(no_mangle)]
pub extern "C" fn _start(mode: u64, uart_shape: u64, _x2: u64) -> ! {
    #[cfg(target_arch = "riscv64")]
    UART_SHAPE.store(uart_shape, core::sync::atomic::Ordering::Relaxed);
    #[cfg(not(target_arch = "riscv64"))]
    let _ = uart_shape;
    let screen = mode == MODE_SCREEN;
    // The screen's batching state; `batching` turns itself off the first time the timer refuses
    // (`ARM` is the whole probe: a spawner that gave no timer slot answers once, negatively, and
    // every later write takes the synchronous path the console had before 2026-09-30).
    let mut batch = ScreenBatch {
        pending: 0,
        armed: false,
        batching: screen && SCREEN_BATCHING_ENABLED,
    };
    loop {
        // Block until a client hands us a length, or, when a batch is waiting on the screen,
        // until the window's deadline ends the wait instead (milestone 151 (notification objects)'s
        // bound receive; on a thread with nothing bound it is an ordinary receive, which is the fallback's path).
        let (len, w1, w2) = match if screen {
            receive_bound(REQUEST)
        } else {
            let (len, w1, w2) = receive(REQUEST);
            Received::Message(len, w1, w2)
        } {
            Received::Notification(_) => {
                paint_screen(&mut batch);
                continue;
            }
            Received::Message(len, w1, w2) => (len, w1, w2),
        };

        // **A kernel line from the log service** (milestone 342 (the kernel and the `console`
        // server drive one UART from two address spaces)): sixteen bytes in the message, never
        // acknowledged, because the acknowledgement on `REPLY` belongs to the client waiting for
        // it. `Inserter` writes the line at the start of a terminal line and holds it otherwise,
        // which is what keeps it out of the middle of an echo.
        match system_log_contract::console::operation(len) {
            system_log_contract::console::OPERATION_KERNEL_LINE => {
                let mut bytes = [0u8; 16];
                bytes[..8].copy_from_slice(&w1.to_le_bytes());
                bytes[8..].copy_from_slice(&w2.to_le_bytes());
                let n = ((len & 0xff) as usize).min(16);
                inserter().kernel_chunk(&bytes[..n], |b| emit(b, screen, &mut batch));
                continue;
            }
            system_log_contract::console::OPERATION_FLUSH => {
                inserter().flush(|b| emit(b, screen, &mut batch));
                continue;
            }
            _ => {}
        }

        // **Clamp to the page, because the length is the CLIENT's** (milestone 43,
        // notes/shared-page-audit.md finding 3). This used to be unbounded, with a comment calling
        // an over-long count "a driver bug" and shrugging it off as a crashed process. The count
        // does not come from the driver; it arrives in the request word from whoever holds WRITE
        // on the request endpoint, so `u64::MAX` was a one-message kill of a server every other
        // client of this console shares. `line_editor` clamps at all four of its length sites; the
        // asymmetry was the finding.
        let len = len.min(PAGE);

        // Copy that many bytes from the shared page to the UART, one at a time, exactly as the
        // kernel's PL011 driver used to. The difference is only where this code runs.
        let shared = SHARED_VA as *const u8;
        // SAFETY: the shared page is mapped read-only in our address space and `len` is clamped
        // to it above; the client is blocked on our acknowledgement, so it is not rewriting it.
        let written = unsafe { core::slice::from_raw_parts(shared, len as usize) };

        // **Two halves, split after the last newline** (milestone 342): the kernel lines waiting
        // for a line end go out at that newline, before whatever follows it on the next line (a
        // prompt, usually), rather than after the prompt with a redraw.
        let cut = written
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |i| i + 1);
        for part in [&written[..cut], &written[cut..]] {
            // Copy to the UART one byte at a time, exactly as the kernel's PL011 driver used to.
            // The difference is only where this code runs.
            for &byte in part {
                uart_put(byte);
            }
            // Then the screen, the same bytes, so the two surfaces never disagree about what was
            // said: staged now, painted when the window closes (`take_screen` paints at once if
            // the batcher is off or the page fills).
            if screen && !part.is_empty() {
                take_screen(part.as_ptr(), part.len() as u64, &mut batch);
            }
            // Track the terminal's partial line, and write the kernel lines that were waiting if
            // this part ended one.
            inserter().wrote(part, |b| emit(b, screen, &mut batch));
        }

        // Acknowledge with the count actually printed, not the count asked for: a client that
        // asked for more than a page learns that fewer bytes went out rather than being told its
        // whole request was honoured. The ack means the wire has them; the screen has them within
        // one window (the module doc records the change from the old both-surfaces meaning).
        send(REPLY, len, 0, 0);
    }
}

/// The kernel-line state, in `.bss`: three buffers that are too large for this program's stack.
static mut INSERTER: system_log_contract::console::Inserter =
    system_log_contract::console::Inserter::new();

/// The one reference to [`INSERTER`].
fn inserter() -> &'static mut system_log_contract::console::Inserter {
    // SAFETY: this program has one thread, and each caller uses the reference for one call and
    // drops it before the next.
    unsafe { &mut *core::ptr::addr_of_mut!(INSERTER) }
}

/// Write bytes that are not a client's (a kernel line, or a redrawn partial line) to the UART and,
/// in [`MODE_SCREEN`], the screen.
fn emit(bytes: &[u8], screen: bool, batch: &mut ScreenBatch) {
    for &b in bytes {
        uart_put(b);
    }
    if screen && !bytes.is_empty() {
        let mut at = 0;
        while at < bytes.len() {
            let n = (bytes.len() - at).min(PAGE as usize);
            take_screen(bytes[at..].as_ptr(), n as u64, batch);
            at += n;
        }
    }
}

/// **The screen's batching state.** `pending` is what is staged in the out page awaiting its
/// `OPERATION_WRITE`; `armed` is whether a deadline is outstanding; `batching` is whether the timer answers
/// at all.
struct ScreenBatch {
    pending: u64,
    armed: bool,
    batching: bool,
}

/// **Stage `len` bytes for the screen**, painting first if the batch cannot hold them.
///
/// The copy lands in the out page at `pending`, the same page `show` used to fill, so the terminal
/// contract is untouched: one `OPERATION_WRITE` naming a length, bytes from the page's start. Staging is
/// copies through cacheable RAM, which is why the reply no longer has to wait for a paint.
fn take_screen(shared: *const u8, len: u64, b: &mut ScreenBatch) {
    if b.pending + len > PAGE {
        // The batch cannot hold this write on top of what it has; hand the terminal what is
        // staged, then stage this. A single write can be a whole page, so this is also the path
        // that keeps `pending` from ever wrapping past the frame.
        paint_screen(b);
    }
    let out = SCREEN_OUT_VA as *mut u8;
    for i in 0..len {
        // SAFETY: both pages are one frame each, mapped at spawn (the shared page read-only, the
        // terminal's page read/write), `len` is clamped to that frame by the caller, and
        // `pending + len <= PAGE` is what the flush above established.
        unsafe {
            core::ptr::write_volatile(
                out.add(b.pending as usize + i as usize),
                core::ptr::read_volatile(shared.add(i as usize)),
            );
        }
    }
    b.pending += len;
    if !b.batching || b.pending >= PAGE {
        // No batching (the flag, or a refused `ARM` turned it off mid-run): one paint per
        // write, the behavior this console had before any of this. Staging without this was
        // kilobyte-cadence screen updates, found as stale rows in the firmware-screen test.
        paint_screen(b);
        return;
    }
    if b.batching && !b.armed {
        // Arm the window. The deadline is in counter ticks (the counter every `now()` caller
        // reads); `counter_ticks_for` rounds up so the window is never short, and the saturating
        // add keeps a counter near `u64::MAX` from wrapping into a deadline already long past.
        let deadline = now().saturating_add(abi::timer::counter_ticks_for(
            0,
            SCREEN_BATCH_NANOS,
            cntfrq(),
        ));
        if timer_arm(TIMER, deadline, NOTIFIED, 1) >= 0 {
            b.armed = true;
        } else {
            // No timer, or no right to it: one paint per write, the behavior this console had
            // before batching. Paint now rather than arming nothing.
            b.batching = false;
            paint_screen(b);
        }
    }
}

/// **Hand the staged batch to the terminal**, or do nothing when nothing is staged.
///
/// One `OPERATION_WRITE` `CALL`, which returns once the terminal has drawn the batch and its driver has
/// put it on the screen. A terminal that refuses or answers short is not an error this process can
/// act on: the UART already had the bytes, and the UART is the surface every gate reads. So the
/// answer is ignored, the way `print!` ignores a UART write's.
fn paint_screen(b: &mut ScreenBatch) {
    if b.armed {
        // The deadline's paint is happening early (the page filled, or a spawner-less timer turned
        // the batcher off). Disarm; if it already fired the word is set, and the next receive
        // returns it to find nothing staged, which is a no-op here.
        let _ = timer_cancel(TIMER);
        b.armed = false;
    }
    let pending = b.pending;
    if pending == 0 {
        return;
    }
    b.pending = 0;
    // The bytes must be visible to the terminal before the request that names them.
    //
    // PAIR: no acquire fence, and none is needed. `display_terminal` is blocked in `receive_cap` and
    // the `call` below is what wakes it, so the kernel's IPC lock (released here, acquired on its
    // side) is the pair. Redundant, kept, for the reason `kernel::user::term_print` keeps the same
    // fence for the same contract. See notes/memory-ordering.md.
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    let _ = call(SCREEN, proto::req(proto::OPERATION_WRITE, pending), 0);
}

/// Transmit one byte, spinning while the transmit path is busy. The register layout is the one
/// arch-specific thing a UART driver is *for*: aarch64's `virt` has a PL011 (32-bit registers, the
/// transmit-FIFO-full flag in the Flag Register), RISC-V's has an NS16550 (byte registers, the
/// transmit-holding-empty flag in the Line Status Register). The kernel configured the device at
/// boot; we only transmit.
#[cfg(target_arch = "aarch64")]
fn uart_put(byte: u8) {
    use pl011::{FR, RegisterBlock};
    use tock_registers::interfaces::{Readable, Writeable};

    // SAFETY: UART_VA is our device mapping of the PL011, handed to us at spawn, for the whole
    // lifetime of this process. This is the same invariant the hand-written read_volatile/
    // write_volatile calls used to assert by comment; register_structs! now checks every offset
    // above at compile time instead (kernel/src/drivers/pl011.rs's own comment: "an off-by-four
    // here is a build error rather than a mystery at runtime", true here for the identical reason).
    let regs = unsafe { &*(UART_VA as *const RegisterBlock) };
    while regs.FR.is_set(FR::TXFF) {
        core::hint::spin_loop();
    }
    regs.DR.set(byte as u32);
}

/// The UART's register shape, the second start word (`abi::uart`), stored before the first byte.
#[cfg(target_arch = "riscv64")]
static UART_SHAPE: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// How many polls of LSR go by before the transmit wait starts reading the clock. A 16-byte FIFO
/// at 115200 baud drains in under 1.5 ms, so a healthy UART almost never gets here, and on the
/// JH7110 reading the clock from user mode costs a trap into the firmware.
#[cfg(target_arch = "riscv64")]
const POLLS_BEFORE_CLOCK: u32 = 1024;

/// The NS16550 twin (RISC-V): Transmit Holding Register Empty in the LSR, at the stride and access
/// width the kernel read from the device tree (`abi::uart`).
///
/// **The wait is bounded at one second, and running out kills this process on purpose.** A UART
/// that never empties its holding register is almost always a wrong register shape, and a console
/// that spins on it prints nothing and blocks every program that prints. Killed, it leaves a line
/// on the wire: the kernel's ring stalls without its drainer, the kernel writes the UART itself
/// again (`kernel/src/kernel_log.rs`), and its fault report names a load fault whose `stval` is the
/// LSR offset this console polled, `0x5` for byte registers and `0x14` for the JH7110's.
#[cfg(target_arch = "riscv64")]
fn uart_put(byte: u8) {
    use abi::uart::RegisterShape;
    const THR: u64 = 0; // transmit holding register
    const LSR: u64 = 5; // line status register
    const LSR_THRE: u8 = 1 << 5; // transmit holding register empty
    let shape = RegisterShape::from_word(UART_SHAPE.load(core::sync::atomic::Ordering::Relaxed));
    let mut polls = 0u32;
    let mut deadline = None;
    while uart_read(shape, LSR) & LSR_THRE == 0 {
        polls = polls.saturating_add(1);
        if polls >= POLLS_BEFORE_CLOCK {
            let t = now();
            match deadline {
                None => deadline = Some(t.saturating_add(cntfrq())),
                Some(d) if t > d => transmit_never_emptied(shape.offset(LSR)),
                Some(_) => {}
            }
        }
        core::hint::spin_loop();
    }
    uart_write(shape, THR, byte);
}

/// Read register `index` of the NS16550 at `UART_VA`, at the shape's stride and width.
#[cfg(target_arch = "riscv64")]
fn uart_read(shape: abi::uart::RegisterShape, index: u64) -> u8 {
    let addr = UART_VA + shape.offset(index);
    // SAFETY: UART_VA is our device mapping of the NS16550, handed to us at spawn, and the largest
    // offset `RegisterShape` allows stays inside that one page. The 32-bit arm is taken only when
    // the stride keeps every register 4-byte aligned.
    unsafe {
        if shape.is_word_access() {
            core::ptr::read_volatile(addr as *const u32) as u8
        } else {
            core::ptr::read_volatile(addr as *const u8)
        }
    }
}

/// Write register `index` of the NS16550 at `UART_VA`; see [`uart_read`].
#[cfg(target_arch = "riscv64")]
fn uart_write(shape: abi::uart::RegisterShape, index: u64, value: u8) {
    let addr = UART_VA + shape.offset(index);
    // SAFETY: as `uart_read`.
    unsafe {
        if shape.is_word_access() {
            core::ptr::write_volatile(addr as *mut u32, value as u32);
        } else {
            core::ptr::write_volatile(addr as *mut u8, value);
        }
    }
}

/// Die with the LSR offset as the faulting address, so the kernel's kill report names it. Page
/// zero is never mapped in any process, so the load faults.
#[cfg(target_arch = "riscv64")]
fn transmit_never_emptied(lsr_offset: u64) -> ! {
    // SAFETY: the address is in page zero, which no process maps, so the load faults and the
    // kernel kills this thread before any value is produced.
    unsafe {
        core::ptr::read_volatile(lsr_offset as *const u8);
    }
    user_mode_runtime::trap()
}

/// **The x86 twin: COM1 by port I/O, not memory** (milestone 299, DECISIONS §121 reversed
/// 2026-09-15). The other two arms differ in a register layout; this one differs in kind. COM1's
/// 16550 lives at I/O ports `0x3F8..=0x3FF`, reached only by `in`/`out`, which ring 3 may execute
/// only for a port it holds a capability to. This process holds the `(0x3F8, 8)` port range the
/// progenitor delegated it, so the kernel's TSS I/O bitmap permits exactly these eight ports and no
/// others; an `out` to any other port would fault.
///
/// The kernel configured the device at boot (baud divisor, 8N1), so this only transmits: spin until
/// the Transmit Holding Register is empty, then write the byte. Same shape as the NS16550 arm above,
/// with `outb`/`inb` in place of the volatile MMIO because the registers are ports.
#[cfg(target_arch = "x86_64")]
fn uart_put(byte: u8) {
    use user_mode_runtime::{inb, outb};
    const THR: u16 = 0x3F8; // transmit holding register (COM1 base)
    const LSR: u16 = 0x3FD; // line status register (base + 5)
    const LSR_THRE: u8 = 1 << 5; // transmit holding register empty
    while inb(LSR) & LSR_THRE == 0 {
        core::hint::spin_loop();
    }
    outb(THR, byte);
}

user_mode_runtime::panic_handler!();
