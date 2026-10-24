# The line discipline as a userspace component

Milestone 28. The layer Unix builds into the kernel as the tty line discipline is here a process,
`line_editor`, sitting on plain endpoints between the raw input driver and the application, and between
the application and the console server. This note covers the build-vs-reuse call, the component's
shape, and the one argument that makes the design correct: it cannot deadlock. The interface it
presents is written up separately in [terminal-contract.md](terminal-contract.md); the interrupt
question it raises is [../design/interrupt-routing.md](../design/interrupt-routing.md).

## Two pieces: a sans-IO engine and a wiring process

The editing lives in the `line_editor` crate, which does no IO and knows nothing about IPC, UARTs,
or endpoints. Byte in, echo bytes out, completed lines out. That split is DECISIONS §7 (testing: host-testable crates) applied. The
editing rules are host-tested in milliseconds against a small terminal model that interprets the
echo the way a VT does. So the tests assert what the user sees, not which escape
sequences were emitted. Twenty tests cover typing, backspace, mid-line insert and delete, cursor
keys in three encodings (CSI, SS3, vt220 `~`), and kill and yank (`^K` `^U` `^W` `^Y`). They also
cover history with non-destructive browsing and dedup, CR/LF/CRLF endings, `^C`, `^D`'s double duty,
`^L` repaint,
overflow, and output newline expansion. The redraw strategy can change without touching a test,
which is the payoff of modeling the screen instead of the byte stream.

The `line_editor` binary (in `user/`) is the other piece: words in, pages copied, words out. It owns no hardware.
Its whole authority is the terminal endpoint (serve), the console server's request and reply
endpoints (print), and three shared pages (the console's, the client's output, the client's
input). It touches no UART and no interrupt, which is exactly why the component could not exist
until the drivers did: it is the layer *between* drivers, not a driver.

## The build-vs-reuse call: built, against the rule's default

The prior-art rule (DECISIONS §14) says for userspace, outside the trusted computing base,
actively prefer porting. The editing engine was built anyway, and here is the accounting for
why the two live candidates did not fit.

- **`noline`** (a `no_std` readline) has a sans-IO core, which is the right shape. Two things
  disqualify it. Its initialization blocks on a cursor-position report (`ESC[6n`) that the
  terminal may never send: a serial line driven by a piped boot script never answers, so a line
  discipline that must be always-on would hang at startup. And it is a **per-read readline**, a
  function you call when you want a line, not an always-on discipline that echoes type-ahead
  while the application is busy elsewhere. Our component must accept and echo bytes that arrive
  before any read is outstanding, which is a different lifecycle than `noline` offers.
- **`embedded-cli`** is a command framework: it parses commands and dispatches handlers. That is
  the *application's* altitude (the shell, milestone 31), not the terminal's. It does not give us
  the line-editing-and-echo layer we needed and would have pulled command parsing into the wrong
  process.

What we would reuse if the requirement grew: a mature VT engine on the *output* side. It is a real
port candidate for milestone 29 (a display terminal), where libghostty-vt or `vte` would serve,
because a full grid-and-scrollback
state machine is genuinely worth not writing. The input-side line editor, by contrast, is a few
hundred lines of well-understood logic with an unusual lifecycle requirement, and owning it kept
the sans-IO testability and the always-on behavior that the candidates fought.

## Why it cannot deadlock

This is the part worth getting right, because a terminal that can wedge is worse than none. The
risk: a client blocked *printing* while the server is blocked *delivering a line* to it, each
waiting for the other.

The Reply capability (DECISIONS §12) removes it. Every request is a `CALL`, served through
`RECEIVE_CAP`. When an `OPERATION_READLINE` arrives and no line is ready, the server does not block waiting
for input. It **parks the caller's one-shot Reply capability in a slot** and loops back to serve
everyone else; the caller stays blocked (that is `CALL`'s contract) without holding the server
hostage. Bytes keep flowing from the input driver, the discipline assembles a line, and only then
does the server invoke the parked Reply to wake the reader. A client blocked in `OPERATION_WRITE` and a
reader parked in `OPERATION_READLINE` are both just parked callers; the server is never blocked on either
while serving the other.

The kernel makes the slot management safe: `capability_table.insert` hands each incoming Reply capability a
*fresh* free slot, so a parked read's Reply at one slot is never clobbered by the next `CALL`'s
Reply at another. One outstanding read per terminal (the contract) bounds the parked set to one.

The one place `line_editor` does block is its own call to the console server, and that is a separate,
single-client rendezvous (`line_editor` is the console's only client) that cannot interleave with the
terminal endpoint it serves.

## Type-ahead, bounded

Completed lines that arrive with no reader waiting go into a fixed FIFO (four lines). A user
typing ahead of a busy application loses nothing until the queue fills; then the newest line is
dropped with a bell, which is what a real tty's flooded input queue does. When a read finally
arrives and a line is queued, the two are married at once: the line is copied into the client's
input page and the parked Reply is invoked. `^C` clears the queue and fails any parked read with
`FLAG_INTERRUPTED`.

## The component is swappable, and that is the demonstrator point

Because every party names only an endpoint ([ipc-naming.md](ipc-naming.md)), the discipline can
be replaced by rewiring endpoints, with no client able to tell. The input driver holds "an
endpoint I send bytes to"; the shell holds "an endpoint that prints and reads lines." Neither
holds a name for `line_editor`. This is milestone 23's "the console hot-swap is instance one" in
concrete form: a terminal with real behavior is a far better first swap target than a raw echo
loop. `line_editor` faults into a clean kill on any bug (it `brk`s / `ebreak`s in its panic handler),
which is honest rather than reckless precisely because the hot-swap story is what recovers it.

## Where it runs

The interactive system is built by userspace progenitor out of its own budget, on both
architectures. On aarch64 it is built through `hello`'s init role, the `shell` feature. The default
tour hands off to it too, since the kernel-wired `shell_service` cannot host a contract-speaking
shell. `shell` is the only feature on this path; milestone 296 (the `initboot` feature was `shell`)
records why. On RISC-V it is built through the portable `system_initializer` builder
(`boot_progenitor`).
Init creates the terminal endpoint and the shared frames. It grants `line_editor` the serve side and
the drivers and shell the `WRITE` side, and none of the wiring is in the kernel. Proven under QEMU
on
both ISAs: `help`, `echo`, and `run` drive a worker to completion through the full path
(input driver → line discipline → console server, and shell → line discipline → console server).

## The graphical terminal: a session over a discipline

`graphical_terminal` is a prompt on the display, launched from the swish prompt rather than started
at boot (calef's ruling of 2026-09-30, milestone 632 (graphics on demand)). The person at the boot
prompt delegates the display devices by typing its name. The progenitor then builds `gpu_driver`,
`display_terminal`, and one of two keystroke sources, and passes the choice in `x0`:

```text
  arm 0 (a keyboard came):
    keyboard_driver ──OPERATION_BYTES──► line_editor ──OPERATION_WRITE──► display_terminal ──► screen
           graphical_terminal ◄──lines── (slot 2)     (slot 1 == slot 2's server)

  arm 1 (no keyboard; the UART is the keystroke source):
    boot input ──OPERATION_BYTES──► boot line_editor ◄──OPERATION_READRAW── graphical_terminal
           graphical_terminal ──OPERATION_WRITE──► display_terminal ──► screen
```

On arm 0 the session's own `line_editor` does the whole job in cooked mode. It paints the prompt,
echoes each keystroke and hands over the completed line. On arm 1 the program puts the boot
discipline in raw mode, reads register-only, and paints the echo itself as the bytes arrive, which
is the shell's own §227 (how Tab reaches the shell) shape. The discipline's cooked echo is bypassed
in raw mode, so nothing echoes twice.

A session ends on `^C` or on a line reading `quit`. Arm 1 puts the boot discipline back in cooked
mode on the way out. The shell's §227 fallback, its `BAD_REQUEST` path, takes it back to raw at the
next prompt, and that covers a session the kernel killed before it could clean up.

The session's authority is the result endpoint (slot 0), an endpoint that prints to the screen
(slot 1) with the page its server reads, an endpoint that owns the keystrokes (slot 2), and, on arm
0 only, the page the discipline delivers lines into. It holds no device, no interrupt and no DMA.

Its limits, which `components/src/graphical_terminal.rs` lists as BUGS:

- A session runs no program. Its lines are echoed and nothing else.
- Arm 1 has no line editing. The only edits are `^C` and retyping.
- Arm 1 can switch the boot discipline's mode under the shell. Slot 2 on that arm is the boot
  discipline's endpoint badged `line_editor::proto::RAW_ONLY_BADGE`, under milestone 709 (a
  graphical terminal session on the no-keyboard arm holds only the raw half of the boot discipline).
  So a session can neither type nor take a line the boot shell reads, and cannot print to the UART.
  Flipping the mode while the shell is between reads abandons what the shell had half typed and
  fails a parked read with `BAD_REQUEST`, which the shell's §227 recovery absorbs. That is a
  disruption, not authority.

## The keyboard as a second input source

`keyboard_driver` turns virtio-input key events into the bytes a terminal understands, and its
spawner picks one of two wirings in `arg0`:

```text
  MODE_RING (rung two, a compositor scene):
  virtio-input ──virtio (PCIe, behind the IOMMU)──► keyboard_driver ──the input ring──► compositor
   (a keyboard)                                      │                (shared memory)
                                                     └──doorbell COMMIT──► "look at the surfaces"

  MODE_DIRECT (milestone 177, option A: the boot's single-terminal case):
  virtio-input ──virtio──► keyboard_driver ──OPERATION_BYTES, one CALL──► line_editor
```

`MODE_RING` is DECISIONS §33 (the compositor's authority is memory, not messages) seen from the
producing side, and [glyphs.md](glyphs.md) has its argument. `MODE_DIRECT` serves a boot with
exactly one terminal, where there is no second window to misdirect a keystroke to
(design/roadmap/0177-graphical-interactive-boot.md). The driver holds one fixed `CALL` capability to
`line_editor`'s served endpoint, granted at spawn, and sends every keystroke there in the same
`OPERATION_BYTES` framing the UART receive driver uses. A keyboard and a serial line are both one
input source to the discipline, and neither contract grew to carry the other's bytes. That is
narrower authority than the ring, not looser: one capability, fixed by whoever built the driver.

The driver does no key repeat of its own: the device sends repeats, and `video_terminal::keymap`
honors them. It drives no LEDs and switches no layouts. It handles no mouse or tablet, though a
`virtio-tablet-pci` presents the same PCI id (recorded in `crates/pci`), and it makes no
configuration-space query.
