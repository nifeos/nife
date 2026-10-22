# Parts 1 and 4 as built: the stop mode, 2026-09-19

An appendix of [milestone 324 (the bench console cannot speak to any of the three
boards)](../0324-the-bench-console-cannot-speak-to-a-board.md), moved out of it on 2026-10-12 (UTC)
under §212 (a prose budget). The block keeps what landed, the trigger finding, the exit statuses and
what could not be tested, and is complete without this file. This file keeps the working: how the
invariant is enforced, why the obvious trigger loses, the end-to-end check it makes possible, the
test coverage and the two choices calef may want to overrule.

## What landed

`--stop` and `--stop-after <n>` on `script/board-console`, the rewritten invariant in its header,
and the same invariant in `crates/serial_console/src/lib.rs` and `notes/board-console.md`. The engine
is a new module, `crates/serial_console/src/stop.rs`, whose header carries the argument. The reading
loop was not duplicated; it took one parameter (`watch::watch_with`).

## The invariant is a mechanism

The invariant now reads: *it writes only what a named mode sends, and every byte it sends is
printed into the log.* That is a mechanism rather than a promise. `Escape::observe_line` is the only
thing in the crate that writes to a port. It writes the announcement into the log and flushes it
before the byte goes out, so the two failure orders are the readable one. A log that says a byte is
going and then says the write failed is honest. There is no path that puts a byte on a wire the log
does not name. The byte is rendered as `0x0d` and never written raw, so a capture stays greppable
and a replay of it cannot be fooled. A host test asserts that no line this mode writes reads as a
boot marker.

## The obvious trigger is the wrong one

It is wrong silently, which is why it is worth carrying out of this lane. The natural place to send
the escape is `soak-test: started`, the line that reaches `Stage::Soak`. That would lose.
`kernel/src/soak.rs` prints `START_MARKER`, then four more lines, and *then* calls `arm_reboot`,
whose first statement is `console::discard_rx`. That is about a kilobyte between the two, roughly
ninety milliseconds at 115200 baud, and a host that reads a line and answers it is comfortably
inside that. The byte would be thrown away by the kernel's own drain, on almost every run. The
failure would look exactly like a board whose receive path is miswired.

So the trigger is the arming announcement (`THIS BUILD REBOOTS THE BOARD`), which `arm_reboot`
prints after the drain. Nothing else in a session can cause a byte to go out. That one gate answers
every state the ruling did not cover, in the same way and with the same answer: nothing is sent.
The states are a board powered off, a board at a U-Boot prompt, a board at a `swish` prompt, a
kernel with no soak in it, and a soak with no reboot loop.

It also closes the hazard the ruling is actually about without relying on the byte's value. A
kernel that has printed that banner has been running for the length of a boot tour, and U-Boot's
countdown is long over. The price is one sentence and it is recorded in three places: a session
attaching to a board already mid-draw waits out that draw, up to two minutes, before a banner
arrives.

## The first end-to-end check of the soak escape

A `--stop` is also the first thing in this tree that can verify the soak escape end to end.
`kernel/src/soak.rs`'s own `BUGS` says the escape is a poll of one bit. It says that *"nothing in
this kernel can prove otherwise, because a UART cannot receive a byte it sends"*, and names a
procedure as the substitute: press a key on the first boot and confirm `DISARMED` before walking
away. A host holding the far end is not under that limit. `--stop` sends a byte it did not print
and waits fifteen seconds (three of the kernel's five-second polls) for the board's own `DISARMED`
line. That procedure becomes a command with an exit status, which is rung four moving up to rung
two. notes/soak.md's step 4 says so where a reader meets it.

## Exit statuses

No exit status was added. Confirmed is `0`. Sent-and-unacknowledged and never-sent are both `3`,
the time running out with the requested thing unreached, which is what `3` already meant. A failure
the board announced (`1`) and a hang (`2`) still win. Those are facts about the board, where an
unconfirmed stop is a fact about this tool.

## What is tested, and what is not

No byte has reached a board. This lane had no board attached to it and neither radon nor xenon was
reachable. Nothing here was run on hardware, and nothing in this block should be read as saying
otherwise.

What is tested is the decision and the bytes. Eleven host tests in `stop.rs` drive the state machine
against transcripts with a `Vec<u8>` standing in for the port, so a test asserts the exact bytes a
board would have received. Two more in `watch.rs` run the same thing through the real loop. What
that leaves unproven is that a write to the descriptor reaches the UART and that the kernel's poll
finds it. The first real `--stop` is the experiment and its `DISARMED` line is the result. It is the
same shape as the gap milestone 249 (the boot lottery is sampled by a person walking to the board)
already carries, one milestone on.

## Two choices calef may want to overrule

Both are cheap to change and both are recorded where a reader meets them rather than only here.

- The byte is a carriage return. The kernel takes any byte, so this is a choice inside a fixed
  contract rather than a wire value two programs agree on. Carriage return because the kernel's own
  instruction is *"press any key"* and Enter is what a person presses when told that. The tool runs
  the same experiment the documented manual procedure runs. `NUL` was the other candidate and the
  refusal is recorded beside the constant.
- `--for` defaults to 150 seconds per requested draw plus two minutes when a stop mode is given and
  `--for` is not. That is derived from `kernel/src/soak.rs`'s two-minute draw plus a boot. It is a
  default and not an agreement (getting it wrong costs a re-run, never a wrong answer). That is why
  the number is duplicated rather than hoisted into a shared crate.

## The soak's markers were never hoisted

This is identified work with no home yet, recorded in `stop.rs`'s `BUGS` where the next reader
meets it. The soak's console markers are string literals in three places (the kernel, the
recognizer, and now this module). They agree by a reader having checked rather than by the
compiler. `crates/boot_ladder` exists for exactly that and holds the boot tour's markers as shared
constants. `START_MARKER`, `REBOOT_MARKER` and the arming and disarming lines were never hoisted
into it. That is a change to the kernel and is outside parts 1 and 4.
