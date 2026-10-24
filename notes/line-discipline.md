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

## What the engine does and leaves out

The engine's bindings, as `crates/line_editor/src/lib.rs` implements them:

- Insertion and deletion at a cursor. Typing at the end echoes only the byte; an edit mid-line
  redraws the tail.
- Cursor movement as control characters (`^B` `^F` `^A` `^E`) and as ANSI sequences: CSI D, C, H
  and F, their SS3 forms, and the vt220 `~` forms.
- Kill and yank: `^K` kills to the end, `^U` to the start, `^W` a word back, and `^Y` inserts the
  kill buffer.
- History: the last `HIST` non-empty lines on up and down, with the line in progress stashed.
- CR or LF ends a line, and an LF straight after a CR is swallowed. Interactive input (CR) and
  piped input (LF, CRLF) both work without the caller knowing which it has.
- `^C` discards the line. `^D` is end of input on an empty line and a delete otherwise, Unix's
  double duty. `^L` repaints.

Three things it leaves out on purpose:

- It tracks no terminal size. The redraw math assumes one row, so a wrapped long line redraws
  wrongly past the margin. Probing the terminal would block the discipline on a terminal that may
  never answer, such as a piped boot script, and `LINE_MAX` keeps the wrap rare.
- It does no tab completion, because that needs the command namespace. Tab comes back as
  `Event::Tab` and the terminal ignores it. The shell runs this engine itself in raw mode and
  answers Tab with `LineDisc::pending` and `LineDisc::insert_text` (§227 (how Tab reaches the
  shell)).
- It keeps no output-side state. Newline translation is the free function `expand_output`,
  because output shares no state with input editing (notes/thread-control-block.md has the tty
  layer).

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

## The component's slots

`line_editor::component` declares what a supervisor routes to a `line_editor` it can replace.
Both declarations put the terminal in slot 0 and the output sink in slot 1. The console's reply
endpoint takes slot 2 when there is one, then come the control endpoint and the supervisor's
notification endpoint. `line_editor` learns the control slot from its second start argument and
finds the notification endpoint in the slot after it, so one binary serves both sink shapes.

`terminal_supervisor` holds every object a `line_editor` is built from, with `GRANT`, so it can
build a second one. It holds the terminal endpoint with `WRITE` too, so it can send
`OPERATION_QUIESCE`. Its start arguments are the sink's mode (`MODE_CONSOLE` = 0 or `MODE_DISPLAY`
= 1), the two image lengths `supervisor::image_lengths` packs, and the slot of a swap-request
endpoint. That slot is 0 on every boot today: the trigger is milestone 198 (a package manager,
and the trivial install)'s installer, ruled by §308 (the installer asks the terminal to swap) and
not built.

The machine's swap reserve, `supervisor::SWAP_RESERVE_PAGES`, is one per machine and apart from
every job pool (calef's ruling "D", 2026-10-19 UTC). It is sized to the largest swappable
instance plus its handoff, which today is `display_terminal`'s: that budget holds one instance and
so cannot hold its replacement. `terminal_supervisor` holds the size to
`video_terminal::component::SWAP_PAGES` at compile time. Whoever drives a swap splits the reserve
whole and lends it with the request; the supervisor gives it back before it answers, and the
lender's `DESTROY` revokes what was made from it. A second swap while one holds the reserve finds
nothing to split and is refused, so the reserve itself serializes swaps. The holder is the swap
trigger, milestone 694 (the installer asks the terminal to swap); until that is built, the
kernel's test `system_tests::user::terminal_display_swap_tests` holds it.

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
