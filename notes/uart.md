# The UART

Universal Asynchronous Receiver/Transmitter.

Hardware with exactly one job: convert between parallel and serial.

The CPU thinks in bytes: eight bits at once, in parallel. A wire carries one bit at a time.
The UART sits between them. Write a byte to one of its registers and it shifts those eight
bits out onto a single wire, one after another. At the far end another UART catches them one
at a time and reassembles a byte.

That is the whole device.

## The interesting word is "Asynchronous"

There is no clock wire. Sender and receiver never share a clock signal. (This is what
separates UART from SPI and I²C, which both have a dedicated clock line.)

So how does the receiver know when a bit starts?

They agree on the speed in advance (the baud rate) and each keeps its own local clock.
The line idles high. To send a byte the sender pulls it low for exactly one bit-time: the
start bit. That falling edge is the only synchronization that ever happens. The receiver
sees the edge and then samples the line at the agreed interval, eight times. One or two
stop bits (line back high) guarantee a known idle before the next start bit.

```
idle   start │ d0  d1  d2  d3  d4  d5  d6  d7 │ stop   idle
─────┐       ┌───┐       ┌───────┐   ┌───────────────────
     └───────┘   └───────┘       └───┘
     ^
     the only sync in the entire protocol
```

Two consequences:

Baud mismatch produces garbage. The receiver samples at the wrong instants and reads
bits out of the middle of transitions. Those mojibake characters on a serial console are
*always* this.

You pay 10 bits to send 8. Start + 8 data + stop. So 115200 baud is ~11,520 bytes/sec,
not 14,400.

8N1 (8 data bits, No parity, 1 stop bit) is the near-universal configuration, and it's
what our `init()` sets up.

## Our code, line by line

From `kernel/src/drivers/pl011.rs`:

**`DR`, the data register.** `r.DR.set(byte as u32)`. Write a byte here and the UART shifts
it out. That one line is the entire act of printing a character.

**`FR`, the flag register, and its `TXFF` bit.** "Transmit FIFO Full." The UART holds a
small hardware queue (16 entries on a PL011) so the CPU can dump several bytes and walk away
rather than waiting for each to physically shift out.

That matters more than it sounds. At 115200 baud one byte takes ~87 microseconds to
transmit. In CPU terms that is an eternity. Without the FIFO the processor would spend
nearly all its time standing around.

`write_byte` spins until there's room:

```rust
while r.FR.is_set(FR::TXFF) {
    core::hint::spin_loop();
}
r.DR.set(byte as u32);
```

That is a polled driver: the CPU busy-waits. Simplest thing that works, and fine for a
boot console.

> TODO (milestone 5): once we have interrupts, the UART can tell us when it has room instead
> of us asking.

`IBRD` / `FBRD`, the baud divisors. The UART is fed a clock and you divide it down to get
the bit rate: integer part and fractional part.

```
divisor = 48_000_000 / (16 * 115_200) = 26.0416...
IBRD = 26,  FBRD = round(0.0416... * 64) = 3
```

QEMU ignores both of these completely. There is no wire and no timing being simulated;
QEMU takes the byte and puts it on your terminal instantly. We set them anyway because a real
PL011 needs them, and the Raspberry Pi will.

## Why the UART is every kernel's first device

Because it needs nothing.

No memory allocation. No interrupts. No DMA. No driver framework. No MMU. Write to one
address, a character appears. It works before there is a heap, before there is a page table,
before there is a scheduler.

Which means it is the one thing that still works when everything else is broken. That is
precisely the situation you are in when you need it most. A kernel panic on an otherwise
wedged machine still reaches the serial console, because the serial console is the last thing
standing.

Every kernel on earth learns to do this first, for that reason.

## The console server

`components/src/console.rs` is the userspace program that drives the UART for its clients. A client
writes text into a shared page and sends the length; the server copies that many bytes to the UART
and acknowledges with the count it printed. The count is clamped to the page, because the client
chooses it.

Started in screen mode, it hands every byte to `display_terminal` as well, so the wire and the screen
show one stream. It holds no screen device: the pixels are `display_terminal`'s and the aperture is
`framebuffer_driver`'s.

The screen can be batched. Writes are staged into the out page, and one `OPERATION_WRITE` hands the
batch over when a timer deadline ends the next receive. The deadline arrives through a notification
bound to the server's thread (`receive_bound`) and `Timer::ARM`. A paint per write cost most of the
`x86_64` swish leg's time under TCG, which is what batching is for. With batching on, an ack
promises the wire, and the screen within one window. A spawner that gives no timer or notification
slot still gets a working console, since the first refused `ARM` drops it to one paint per write.
The batcher is off today, behind `SCREEN_BATCHING_ENABLED`, and that constant's doc comment says why.

The system log service sends kernel lines to the same server, sixteen bytes a message and never
acknowledged. The acknowledgement belongs to the client waiting on the reply endpoint. A kernel line
goes out only at the start of a terminal line. Mid-line it waits for the client to end the line, or
for the service's `OPERATION_FLUSH`, which prints it on its own line and redraws the partial line
beneath it. A client write is split after its last newline, so waiting lines go out before a prompt
rather than after it.

## Trivia that bites later

PL011 is ARM's UART design (part of their PrimeCell peripheral family). The classic PC one
is the 16550. Different registers, same idea.

**The Pi has two UARTs**: a real PL011 and a cut-down "mini UART." Which one lands on the GPIO
pins depends on config, and getting this wrong is a classic first-port frustration. See
[target-hardware.md](target-hardware.md).

Voltage levels are a separate layer from the protocol. A Pi's GPIO speaks 3.3V TTL. The
old DB9 port on a 1990s PC used RS-232, which swings ±12V *and inverts the signal*. Wire one
straight to the other and you destroy the Pi. The ~$10 USB-TTL cable is a level converter,
which is why you buy one instead of soldering a DB9.

## The receive side, at EL0

`components/src/input.rs` is the console's receive driver, and it is a driver and nothing more. It
owns the UART's receive side and its interrupt and forwards every byte to the line discipline. Line
assembly, echo and editing live in `line_editor`, where Unix keeps them too: a UART driver feeds
the tty layer and is not the tty layer. Its authority is `WRITE` on the terminal endpoint, the
receive interrupt and the device, so it cannot print, spawn, or read what anyone else typed. On x86
the device is a `PortRange` capability for COM1, which the kernel's TSS I/O bitmap reads; the
driver never invokes it.

A keystroke is one byte and control flow, not bulk data, so the bytes ride in registers, up to eight
in the words of one `OPERATION_BYTES` `CALL`, as the rule of §10 (process model: capability-based, microkernel) asks of control traffic. A paste drains
in 8-byte messages, and the `CALL`'s rendezvous keeps a fast sender from outrunning the discipline.

It is interrupt-driven on all three architectures, and the reason is measured. Until milestone 505
(an x86_64 input driver that never lets the core idle) x86 had a polled twin,
`loop { drain(); yield_now(); }`. A thread that always yields is always runnable, so the run queue
was never empty and the idle loop never ran on that core. QEMU sat at 99 to 100% of a host core at
an idle prompt, 76 CPU-seconds in 78 wall-seconds, measured by milestone 182 (x86_64's own
interactive-boot entry point). The capability-slot gauge the idle loop prints never moved past the
hand-over.

Two orderings are load-bearing:

- It drains the FIFO by polling before it arms the interrupt, because input piped in at boot is
  already waiting there. That narrows the race with the first interrupt and does not close it. A
  line burst-piped into QEMU in the few instructions before that drain runs can lose its leading
  character. A person typing after the prompt never hits it, and every line after the first is
  interrupt-driven and intact. Closing it needs the driver armed before any input arrives.
- In the loop it clears the interrupt at the device first and drains second. QEMU's PL011 raises
  its receive interrupt only when the FIFO goes from empty to one byte, and `ICR` lowers it whatever
  the FIFO holds. Cleared after the drain, a byte that lands between the drain's last empty check
  and the clear has its interrupt wiped while it sits in the FIFO. Every byte behind it lands on a
  non-empty FIFO and raises nothing, so input stops for good. Cleared first, a late byte leaves the
  interrupt raised and the next wait returns at once.

---

*Add to this file as new serial concepts come up.*
