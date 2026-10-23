# The kernel console

`kernel/src/console.rs` is where `print!` and `println!` end up. This note carries the design its
doc comments link to. The console server, the shell and the gates that read the wire are in
notes/board-console.md; output on a machine with no serial port is in notes/serial-less-output.md;
the ring between the two address spaces is `kernel/src/kernel_log.rs`.

## One lock, two surfaces

The console is a UART and, where the machine has one, a screen. Both say the same thing. The person
at the bench and the gate reading the wire look for the same line, and a console that chose one
surface would be a console somebody has to configure.

One `IrqSafeMutex` at `rank::CONSOLE` holds both. The UART is written one byte at a time, so an
interrupt handler printing in the middle of another writer's `write_str` would splice into it
mid-word. The screen lives inside the same struct rather than under its own lock. A second mutex
would need a rank below `rank::CONSOLE`, would be taken on every `print!` for nothing, and would
give the panic path two locks to break instead of one.

The cell buffer a scroll redraws from is 36,000 bytes, and it sits in the `static` beside the screen
rather than inside it. Inside the screen struct it would be built on the 64 KiB boot stack and
moved, the shape of the 16 KiB-array incident in notes/stack.md. As a field of the static it is laid
down at compile time, at the same cost in image on every architecture.

## The address is compiled in

Everywhere else the kernel asks the machine what it is (notes/device-tree.md). The console cannot.
The device tree parser is the code most likely to have a bug, and `println!` is how it gets
debugged, so the console comes up before the tree is parsed and cannot depend on it. A new board
needs a different constant, and that is the right shape: a per-board early-console address that
gets the kernel far enough to read the tree that says everything else.

aarch64's `virt` has a PL011 and RISC-V's has an NS16550. `x86_64` has the same NS16550 at I/O port
0x3f8 (COM1) instead of a memory address, so it is the same driver over a port register space, and
the port goes through no `phys_to_virt`. The machine description prints the compile-time address.
A board whose real UART is elsewhere prints nothing at all, and a board whose tree disagrees with
the constant shows the disagreement on that line.

The shape comes from the tree, though the address does not. On RISC-V, `configure_from_dtb` reads the
serial node's `reg-shift`, `reg-io-width`, `clock-frequency` and `compatible`. The JH7110's UART0 sits
at QEMU's address with different silicon behind it: a DesignWare 8250 with registers four bytes
apart, 32-bit accesses, a 24 MHz clock and the DW busy quirk. It runs immediately after `init` and
before the first `println!`. On the board a byte read of LSR at the unshifted offset 5 lands in the
IER word, and the transmit poll spins forever. Between the two calls the console is correct for
QEMU and wrong for the board, and nothing prints. Mainline JH7110 trees give the clock as a phandle
rather than a frequency, which yields divisor 0. The kernel then leaves the divisor and line
controls as U-Boot programmed them, on a board whose firmware just printed a prompt at 115200.

## Who paints the screen

One aperture has exactly one writer at a time. Two painters on one framebuffer interleave the way
two UART writers do, except that on a screen the splice is pixels and neither message can be read.
So the handover is a state, `Painter`, rather than a convention. The kernel paints while it is
`Kernel`. `yield_screen` clears the screen under the console lock, so no kernel line lands between
the clear and the handover, and moves it to `Terminal` once; a second caller gets nothing. After
that the kernel's lines go to the UART alone.

`attach_screen` clears the screen when it arms. Whatever firmware left there (a vendor logo, a boot
menu, the loader's own lines) is not the kernel's, and a boot tour written over a splash cannot be
read. The cost is that a machine that clears its screen and then says nothing has failed between
the two, which is itself a reading.

A panic takes the screen back (`reclaim_screen_for_panic`). On a machine with no serial port the
screen is the only place a panic can be read. A panic halts only its own core, so a flush already
running on another core, or one a live shell asks for afterwards, can paint over the panic text. That
is a garbled panic where there would otherwise be none, and the UART copy is unaffected. Stopping the
other cores is the panic path's job, and it does not do it yet.

## The screen hold

`cargo xtask uefi-boot` checks that a machine with no serial port shows its boot on its screen by
photographing the framebuffer through QEMU's monitor. Each line of the tour is on the framebuffer
only until the next page scrolls it away and then `yield_screen` clears it. Both close in guest time,
so no host-side deadline widens them. Sampling a transient state is rung four of the ladder in
`AGENTS.md`; being told when to look is rung one. §199 (the screen check asks instead of sampling)
rules the handshake.

When the boot command line carries `machine_discovery::framebuffer::SCREEN_HOLD`,
`hold_screen_at_handover` sets a flag and `yield_screen` first calls `hold_screen_for_host`. That
drains the receive buffer, announces `boot_ladder::SCREEN_HELD` on the serial line, waits for a
byte, and discards the answer so the line editor does not echo it at the prompt. An ordinary boot
pays one relaxed load, once.

The wait is bounded twice, because a knob that can wedge a machine is a worse defect than the one
it fixes. `HOLD_TICKS` is ten seconds of scheduler ticks. The host's round trip (a monitor command,
a PPM write, a read and a glyph decode) measures about 50 ms idle. Ten seconds is two orders of
magnitude of headroom, and short enough that a knob set by mistake is a pause rather than a power
cycle. `HOLD_POLLS` counts register reads for a machine whose timer is not ticking, since ticks come
from the timer interrupt. It is about a second of spinning under TCG and never fires where the
timer works.

The loop parks the core between polls once the tick counter has moved. A guest spinning flat out
starves QEMU's main loop, the thread that serves the monitor, and every `screendump` taken during
the hold came back half-written. Before the counter moves it spins and pays `HOLD_POLLS` down,
because `wait_for_interrupt` on a machine whose timer is dead would never return.

The wait runs outside the console lock. That lock is an `IrqSafeMutex`, so holding it for seconds
would hold interrupts off for seconds, and the announcement's own `println!` would deadlock on it.
Nothing can paint in between, because the kernel is the only painter until `yield_screen` says
otherwise.

Only `x86_64` arms the hold, in `arch::x86_64::machine::attach_screen`, because only its boot chain
hands over a command line. The mechanism is arch-neutral, and both console UARTs carry the receive
half it needs. aarch64 and riscv64 have no window to close yet: their only screen is
`crate::screen`'s `ramfb`, whose pixels are the kernel's own `.bss`, and `user::boot_screen_terminal`
refuses to hand kernel memory to a process (`screen::is_kernel_memory`). So `yield_screen` is never
reached there and the tour stays on screen. A firmware aperture outside the kernel image would open
the window; arming the hold there needs a reader for `/chosen/bootargs`, which the kernel does not
parse (`kernel/build.rs`).

## The kernel's ring

`kernel/src/kernel_log.rs` turns every line the kernel prints into one record in a ring of frames,
whose layout is `system_log_contract::kernel_ring`, shared with the log service. §175 (where the
kernel's own output goes once userspace owns the console) rules where each line goes, and the
drain's shape is calef's ruling F of 2026-10-03 (UTC), in notes/kernel-ring-drain.md. What changes
is whether the kernel also writes the line to the UART itself:

- With no drainer yet, which is all of boot, it does, byte by byte, and flags the record `DIRECT`
  so the service does not print it again.
- With a drainer that keeps up, it does not. The line waits in the ring, the kernel signals the
  service, and the service forwards it whole, on a line of its own.
- With a drainer that has stopped (more than half the ring unread, or the oldest unread line older
  than `STALL_NANOS`), it does, after every unread line before it, each counted in the ring's
  fallback count and flagged `DIRECT`. A torn line while the service is down is cheaper than a line
  nobody sees, calef's addition to §242 (a system log).
- On a panic it always does, as the next section says.

Signalling is deferred. The console lock is the leaf of the lock order, and `println!` runs under
arbitrary locks, `IPC_TABLES` among them. A held line sets a flag, and `signal_if_safe` signals once
this core holds no lock: at the end of `_print` or, failing that, at the next timer tick. A drained
line is up to one tick late, and never early. The stall test runs on every kernel line and once
every `STALL_POLL_TICKS` on core 0.

## Escapes from the ring

Kernel lines normally go through the ring in `kernel_log`, which a drainer in the console server
prints. A panic writes the UART directly (§175 (where the kernel's own output goes)), and so does a
reset: either stops the machine, so a line left in the ring is a line nobody reads. Both print what
the drainer had not, then write every later line direct. A reset firmware refuses leaves the kernel
printing direct for the rest of the boot; handing the ring back would need the drainer's cursor
moved past what was caught up.

`drain` waits for every printed byte to leave the machine, and every arch reset and the board exit
call it before the instruction that may not return. It has two halves. `kernel_log::flush` moves
what the drainer has not printed onto the wire, and the driver's `drain_transmitter` waits out the
FIFO. `write_byte` returns when the FIFO has room, not when the bytes are sent. On radon,
`arch::reboot` printed its line and called SBI SRST microseconds later, and the PMIC dropped power
mid-line.
