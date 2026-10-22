# board-console's design: the read, the port, no dependency, and the tests

An appendix to [notes/board-console.md](../board-console.md), moved there on 2026-10-11 (UTC)
under §212 (a prose budget). It holds the decisions that are not obvious from the code, why there is
no serial dependency, and how it is tested with no board.

## Two things in the design that are not obvious

A partial line is weaker evidence than a complete one. The recognizer is offered the incomplete tail
of the stream as well as the finished lines. U-Boot's `StarFive #` prompt has no newline after it,
and a tool that waited for one would sit there while the board sat waiting for it. But a tail is
ambiguous in two ways that a byte-at-a-time test found and reasoning did not. `U-Boot ` reads as
U-Boot proper right up until the next three bytes turn out to be `SPL`. So a board dying in SPL was
being reported two stages further along than it got. And a marker carrying a payload captures a
truncated one: the tail `nife on ` recorded the banner as the empty string, and `[PANIC] ` recorded
a panic with no message, both latched before the rest arrived. So a tail may ratchet a stage,
because a substring match is monotone and more bytes cannot unmake it. It may not settle a word
boundary and it may not capture text.

The read happens on its own thread, so the deadline holds whatever the reader does. The port is
configured with `min 0 time 1`, which makes a `read` return after a tenth of a second with whatever
arrived, including nothing. When that works, a single-threaded loop would be fine. There are
several ordinary ways for it not to work: an `stty` that failed and was only warned about, a driver
that ignores the setting, a file or a pipe standing in for a port. So the read runs on a worker and
the loop waits on a channel with a timeout. The worker may still be parked in `read` when the tool
decides to stop, and that is fine. Deciding to stop means returning, the process exits, and the
descriptor goes with it. This is `CLAUDE.md`'s *Never leave QEMU running* rule wearing different
clothes. An emulator that never exits and a board that never speaks are the same bug seen from the
tool's side.

Open the device first, then run `stty` on it, then check that the speed took. Opening a macOS `cu`
device resets its termios towards the driver's default. So a configuration made before the read
descriptor exists is undone by the open that follows it. Holding the descriptor is what keeps the
setting alive, because it reverts when the last user closes. Measured on the rig: the CH343 sits at
9600 when nobody holds it and reads 115200 while this tool does.

This is not a theory, and somebody already paid for it. calef's first capture of the board on
2026-09-01 was pure garbage, because it configured the port with `stty` and then ran `cat`, and
`cat`'s open put the device back to the default rate. It looks exactly like a wiring fault, which is
the triage ladder's second row, and it cost a power cycle to diagnose. So the tool now reads the
speed back after configuring and says loudly when it is not 115200. An invisible wrong baud sends
the next person chasing hardware, and a stated one takes a second to fix.

The residual hazard, stated rather than hidden. The configuration is a separate process rather than
a `tcsetattr` on our own descriptor. So any *other* process that opens the device mid-session can
put it back, and nothing here would notice. That is the honest cost of having no dependency, and it
is the case that would justify taking one.

`O_NONBLOCK` is not used, and that is a decision rather than an oversight. Its job would be to stop
a silent board hanging the tool in `read`, and the worker thread above already guarantees that. The
thread's guarantee is the stronger one: it holds even when the `stty` fails outright, where a
non-blocking read on its own would not. A non-blocking read returns immediately, but only a deadline
decides when to stop asking.

And `cu.`, never `tty.` On macOS the `tty.*` name is the dial-in side and blocks in `open` until
carrier detect asserts, which a three-wire console cable never does. That hangs *before* any
deadline of ours has started. The tool warns if it is handed one.

## Why there is no dependency

Configuring a UART is one `tcsetattr`. The two ordinary ways to reach it from Rust are the
`serialport` crate, or `libc` and the struct by hand. Both are new dependencies in the shipping
graph, and §46 (thin primitives or whole subsystems; we write everything in between) makes taking
one a decision rather than a convenience. A lane does not take that decision. A serial configuration
call is squarely the "in between" that section refuses: not a thin architectural primitive, and
nothing like a whole subsystem. `stty(1)` makes the same call, is in every base system, and costs a
process spawn per session.

Milestone 324 (the bench console cannot speak to any of the three boards)'s writing mode did not
change this, which is worth saying because it looks like it
should have. The port is already opened read-write, and one byte goes out through an ordinary
`write`, with no flow control to negotiate and no baud to change. If this ever needs to write *with*
flow control, or at a non-standard baud, the trade changes and the dependency is worth proposing.

## Testing it with no board

The board was powered off for all of this and its power strip was unreachable, so nothing below
involved a booting machine.

- The recognizer runs against four fixtures under `crates/serial_console/tests/fixtures/`, fed one
  byte at a time. That is the worst case a real UART delivers, and the case that catches a
  recognizer depending on chunk boundaries. Two of them are in `captured/` and are raw bytes off the
  wire on 2026-09-01, control characters and all. Two are in `synthetic/` and are cases nobody has
  yet seen at a bench. The directory split is the provenance, deliberately, because a claim in a
  README is a weaker record than a path.
- The deadline runs against sources that block forever, which is what a powered-off board looks
  like through a port whose read timeout did not take.
- A real descriptor: a FIFO standing in for a port covers `stty` failing (a warning, not fatal) and
  a source that speaks and then stops (caught as silence, exit 2).
- The port layer, which is the part that looks untestable and mostly is not. Several pieces are
  pure functions with the IO lifted off them. They are the argument list `stty` is given, the complaint it
  produces when it fails, the speed read back afterwards, the dial-in warning, and the choice
  between zero, one and several adapters. So a host test asserts on the exact words a person meets
  at a bench. `open` itself is exercised against a temporary file holding the real capture, so the
  path from `port::open` to a recognized boot runs in a host test.
- The real adapter, with the board off, covers everything except the board. Discovery finds
  `/dev/cu.usbmodem*`, the `stty` moves it to 115200 and it reverts on exit, and the deadline
  returns with zero bytes and exit 3.
- The writing mode (milestone 324) is tested against a `Vec<u8>` standing in for the port. So a
  host test asserts the exact bytes a board would have received and the log entries beside them.
  Eleven tests in `stop.rs` cover it. They test the decision: nothing is written until the board
  announces an armed reboot loop, one byte is sent once, and it is the n-th draw and not the one
  before it. They test the invariant: the log names the byte in hex, and names it *before* the
  write, so a failed write reads correctly. And they test the confirmation, a port that refuses the
  write, and the agreement between this mode's draw count and `--tally`'s. Two more in `watch.rs`
  run the same thing through the real loop. What none of them prove is the wire: no byte has
  reached a board, and `stop.rs`'s `BUGS` says so where a reader meets it.

The residue no host test reaches is one claim: that `tcsetattr` actually took. Nothing on a host is
a tty, so a test can prove the right arguments were sent and cannot prove the device listened. That
was checked by hand against the CH343 (9600 before, 115200 while held, reverting on exit). It is
also what `confirm_speed` gates at runtime, which is the better answer anyway: the check runs on
every real session rather than only when somebody runs the tests.
