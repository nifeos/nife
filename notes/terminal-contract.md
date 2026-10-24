# The terminal contract

Milestone 28. This is the interface a terminal presents, written down so that the programs on
either side of it can be built independently and swapped without either one knowing. Milestones
29 (the display terminal) and 31 (the capability shell) implement *against* this contract rather
than against `line_editor`, the particular component that satisfies it today.

A terminal sits between two driver endpoints and an application:

```text
  input driver ──OPERATION_BYTES──►┌──────────┐──text──► console server ──► UART
                            │ terminal │
       application ◄─lines──└──────────┘◄─OPERATION_WRITE / OPERATION_READLINE── application
```

Nobody in that picture can name anyone else. The input driver holds "a rendezvous I send wire
bytes to." The application holds "a rendezvous that prints text and reads lines." The console
server holds "a rendezvous requests arrive on." Rendezvous-only naming
([ipc-naming.md](ipc-naming.md)) is the whole point: rewire the rendezvouses and no client can tell
the terminal changed, which is milestone 23 (a capability-routed component OS with live replacement)'s hot-swap claim in component form. See
[line-discipline.md](line-discipline.md) for the component that implements this today and why it
was built rather than ported.

## The two halves of the contract

A contract has a wire half and an IPC half, and they are independent.

- The wire half is what the terminal echoes to the screen and what escape sequences it
  understands from the keyboard. A client never sees this. It is the agreement between the
  terminal and the *human* at the far end of the serial line, and it is documented in
  [line-discipline.md](line-discipline.md) with the engine that produces it.
- The IPC half is the protocol on the endpoints: the opcodes, the flags, the shared pages.
  This is what a client and the drivers must speak, and it is the substance of this note. The
  framing constants live in `line_editor::proto` so the server, its clients, and the kernel-side
  tests share one definition.

The protocol is a userspace protocol, not kernel ABI. The kernel routes these words the way
it routes any IPC (§10, §12); it never reads an opcode. Adding an opcode is a change to this note
and to `line_editor::proto`, not a change to the syscall surface.

## The IPC protocol

Every request is an endpoint `CALL` (DECISIONS §12): the client sends two words and blocks until
the terminal replies through the one-shot Reply capability the kernel mints. The first word packs
an opcode in bits 63:56 and a length or count in the low 32; bits 55:32 are reserved and zero.
`proto::req(op, len)` builds it; `proto::op` and `proto::len` take it apart.

Bulk data never rides in the words. It travels in pages the client shares with the terminal, one
outbound and one inbound, exactly the §10 split: control by message, data by shared memory. A
client maps an output page (it writes, the terminal reads) and an input page (the
terminal writes, it reads).

| Opcode | Direction | First word | Second word | Reply `r0` | Reply `r1` |
|---|---|---|---|---|---|
| `OPERATION_WRITE` | app → terminal | `req(OPERATION_WRITE, len)` | 0 | bytes consumed | 0 |
| `OPERATION_READLINE` | app → terminal | `req(OPERATION_READLINE, plen)` | 0 | line length | flags |
| `OPERATION_BYTES` | driver → terminal | `req(OPERATION_BYTES, n)` | n bytes, packed LE | 0 | 0 |
| `OPERATION_INTRCOUNT` | app → terminal | `req(OPERATION_INTRCOUNT, 0)` | 0 | `^C` count so far | 0 |
| `OPERATION_PRINT` | adapter → terminal | `req(OPERATION_PRINT, len)` | len bytes, packed LE | bytes consumed | 0 |
| `OPERATION_RAWMODE` | app → terminal | `req(OPERATION_RAWMODE, 0\|1)` | 0 | 0 | 0 |
| `OPERATION_READRAW` | app → terminal | `req(OPERATION_READRAW, 0)` | 0 | byte count (1..=8) | bytes, packed LE |
| `OPERATION_QUIESCE` | supervisor → terminal | `req(OPERATION_QUIESCE, 0)` | 0 | `QUIESCED` | 0 |

`OPERATION_QUIESCE` (milestone 23, a capability-routed component OS with live replacement; calef's ruling
of 2026-09-26) stops a terminal so a replacement can take over. It rides the terminal endpoint, so
the endpoint's FIFO does the draining. A parked `OPERATION_READLINE` or `OPERATION_READRAW` is answered
`FLAG_RETRY` first, because the reply capability naming that reader cannot leave the terminal. The
terminal then waits on a control endpoint for `CTL_RESUME` or `CTL_QUIT`. A terminal started with
no control endpoint refuses it with `BAD_REQUEST`. The boot terminal is not that case:
`terminal_supervisor`, which `system_initializer` builds on every boot, starts it with a control
endpoint. The terminal does not check who sends `OPERATION_QUIESCE`, so any unbadged holder (the
input driver, the shell, `login`, `system_initializer`, `terminal_supervisor`) can quiesce the boot
terminal until reboot.

- `OPERATION_WRITE`: print `len` bytes from the client's output page. The terminal performs
  output-side newline translation (`\n` becomes `\r\n`) and passes everything else, ANSI
  included, untouched: the wire belongs to the application while it is printing. The reply comes
  when the bytes are on the console's side; the output page is the client's to reuse again.

- `OPERATION_READLINE`: read one line. The low bits carry a prompt length; the prompt bytes sit at
  the start of the output page and the terminal paints them, followed by any type-ahead the user
  already entered. The reply comes when a completed line is ready: `r0` is its length (the bytes
  are in the client's input page) and `r1` carries the flags below. At most one read may be
  outstanding per terminal. A second `OPERATION_READLINE` while one is parked is a protocol violation
  and is refused with `BAD_REQUEST`; the contract is one line reader per terminal, which is what
  a session is.

- `OPERATION_BYTES`: the driver half. One to eight raw wire bytes, packed little-endian in the
  second word, replied immediately. A keystroke is one byte and control flow, not bulk, so the
  words-in-registers path fits; a paste drains eight bytes per message, and the `CALL`
  rendezvous is the flow control that keeps a fast sender from outrunning the discipline. The
  driver does no editing, echo, or line assembly; it forwards bytes and nothing else, the way a
  UART driver feeds the Unix tty layer without being the tty layer.

- `OPERATION_PRINT` (DECISIONS §67 (a program's second stream is a declaration, not a number)): print one to eight bytes carried in the request's own words.
  Same job as `OPERATION_WRITE` and same manners (both go through `expand_output`), and it exists because
  of a limit `OPERATION_WRITE` has that is easy to miss: it reads from the client's output page, and
  there is exactly one of those. The progenitor maps a single frame into the terminal read-only and into the
  shell read/write, so a second page-based client would need a second frame and a page index in
  every request. That is `filesystem_contract`'s one-page-two-clients problem (DECISIONS §55 (the file behind a `>` is the shell itself, because one page cannot serve two clients)) arriving in a
  second contract.

  Register-only sidesteps it: `components/src/terminal_sink_caretaker.rs` turns the sink contract into terminal
  output with no page at all, which is what let the terminal become a destination a program's
  output slot can hold. Eight bytes rather than sixteen is this contract's request shape, not a
  choice: a served request arrives through `receive_cap` with the reply capability and two data words,
  which is why `OPERATION_BYTES` carries eight too.

- `OPERATION_RAWMODE` / `OPERATION_READRAW` (milestone 169 (the smallest real text editor)): the raw-keystroke primitive `kilo` needs and the
  line discipline, by design, does not give a program (DECISIONS §21 says a program "never sees a
  keystroke, an escape sequence, or an echo"). `OPERATION_RAWMODE` switches the terminal between the line
  discipline and raw mode (`len` 1 to enter, 0 to leave), replied immediately. While raw mode is on,
  `OPERATION_BYTES` bypasses [`LineDisc`](../crates/line_editor/src/line_disc.rs) entirely: no echo, no editing,
  no line assembly, and a control byte like `^C` is delivered literally rather than intercepted.
  `OPERATION_READRAW` reads the result: one to eight raw bytes, packed little-endian. That is the
  same register-only shape `OPERATION_BYTES` and `OPERATION_PRINT` already use, so this needs no
  page either. At most
  one `OPERATION_READRAW` may be outstanding, the same rule `OPERATION_READLINE` already has. The two input
  models refuse each other with `BAD_REQUEST`: `OPERATION_READLINE` while raw mode is on, `OPERATION_READRAW`
  while it is off. Switching mode in either direction abandons whatever line was in progress in the
  mode being left (the line discipline's edit buffer, or raw mode's queued-but-unread bytes). It
  fails a parked read of the mode being left rather than hang it forever. History and the kill
  buffer are untouched. Proved in `system_tests/src/user/raw_mode_tests.rs` against a real `line_editor`
  process. It checks echo suppression (both ways, so the check cannot be vacuous) and literal
  delivery of bytes the line discipline would otherwise interpret. Then the two refusals, and a read
  parked before data
  arrives still being answered once it does.

  Only `line_editor` serves it. `display_terminal` does not serve `OPERATION_READLINE` either (see
  "For milestones 29 and 31" below); raw mode is a line-discipline opcode exactly like
  `OPERATION_READLINE` is, and belongs nowhere else. A client behind the display terminal that wants raw
  keystrokes composes `line_editor` in front of it exactly as one wanting edited lines already does,
  and that composition needs no change to either component. `line_editor`'s `OPERATION_WRITE` output
  is already backend-agnostic (it prints through whatever `Con` sink is wired underneath). Raw mode
  never touches `Con` at all except its overflow bell, so the primitive is identical behind
  either backend by construction rather than by having been wired twice.

- `OPERATION_INTRCOUNT` (DECISIONS §24 (interrupting the foreground process)): reply immediately with the running count of `^C` the terminal
  has seen since boot. This is the shell's `^C` sensor for the case a parked read cannot cover: when
  a foreground job is running, the shell is not in `OPERATION_READLINE`, so there is no read to fail with
  `FLAG_INTERRUPTED`. The shell polls this count while watching the job and escalates from its
  advance (DECISIONS §24). A poll, not a delivered signal, because there is no non-blocking receive
  to block the shell on both the job and `^C` at once; a busy-poll with `yield` is the honest
  interim. The count is monotonic, so the shell learns of a `^C` by the count changing and never
  misses one; it tracks a session watermark of what it has consumed.

### Read flags

- `FLAG_EOF` (`1<<0`): end of input (`^D` on an empty line). The line length is 0.
- `FLAG_INTERRUPTED` (`1<<1`): the read was interrupted (`^C`). The line length is 0. This is the
  contract's `^C` hook for a job blocked reading (the shell at its prompt). A job that is
  running is reached through `OPERATION_INTRCOUNT` and the two-tier routing instead (DECISIONS §24,
  built; design/interrupt-routing.md is the original proposal). One `^C` at the terminal does both:
  it fails any parked read and it bumps the count.

- `FLAG_RETRY` (`1<<2`): ask again. The terminal is being replaced and handed this read back;
  nothing typed was lost. Re-issue the same request unchanged, and the terminal that answers it
  resumes the line without repainting it. An `OPERATION_READLINE` reply carries it as `r0 = 0`,
  `r1 = FLAG_RETRY`. So does an `OPERATION_READRAW` reply, whose `r1` is otherwise data, which is safe
  because a raw read never returns zero bytes. `proto::is_retry` reads both.

A client that speaks the contract handles all three flags; `swish` and `rmle`, the two readers in
the tree, re-issue on `FLAG_RETRY`. The shell's response is the model: on
`FLAG_INTERRUPTED` it discards and reprompts, on `FLAG_EOF` it notes there is nowhere to exit to
and reprompts. Neither flag carries a signal or a process identity; the terminal reports a fact
about the read, and what to do with it is the client's business.

### `BAD_REQUEST`

`proto::BAD_REQUEST` (`u64::MAX`) is the reply `r0` to a request whose opcode the terminal does
not implement, and to a second concurrent read. A sentinel rather than silence, so a confused
client fails fast instead of hanging on a reply that will never come.

### Badged copies

A terminal endpoint capability with a non-zero badge is served by the badge's value, and every
request outside that badge's set is refused with `BAD_REQUEST`. An unbadged capability is served
everything except `OPERATION_READ_LINE_BYTES`, and that is every holder the boot wires: the input
driver, the shell and `login`. The badge is
the kernel's word on every receive (§230 (badged endpoint capabilities)), and a badged copy cannot
be re-badged, so a holder cannot widen it back.

- `proto::RAW_ONLY_BADGE` (milestone 709 (a graphical terminal session on the no-keyboard arm
  holds only the raw half of the boot discipline)) reads keystrokes and nothing else: `OPERATION_RAWMODE` and
  `OPERATION_READRAW`. Without `OPERATION_BYTES` its holder cannot type a line somebody else reads,
  and without `OPERATION_READLINE` it cannot take one. It cannot print or quiesce. The spawn service
  mints it for a `graphical_terminal` session with no keyboard, whose keystrokes come from the boot's
  own discipline.
- `proto::STDIN_BADGE` (milestone 892 (a std program at the prompt holds the network and a name
  resolver)) is a program's standard input, served
  `OPERATION_READ_LINE_BYTES` only.

## What a terminal owes a program, and what it does not

Owes:

- Line discipline on input, by default. The program calls `OPERATION_READLINE` and receives a finished
  line. All editing (cursor motion, backspace, kill and yank, history) happened on the far side of
  the endpoint; the program never sees a keystroke, an escape sequence, or an echo. Unless it
  asked not to: `OPERATION_RAWMODE` (milestone 169) opts a program into exactly that, one keystroke at a
  time through `OPERATION_READRAW`, for the class of program (a screen editor) that needs to.
- Newline translation on output. A program writes Unix `\n` and the terminal puts a carriage
  return on the serial wire. A program that wants raw control of the wire gets it: everything
  that is not a bare `\n` passes through, so ANSI from the application reaches the screen intact.
- Type-ahead. Bytes typed before (or during) a read are buffered and delivered in order, up to
  a bounded queue; past the bound the newest line is dropped with a bell, as a real tty's flooded
  input queue does.

Does not owe:

- Terminal size tracking. The redraw math assumes a line fits one row. A line longer than the
  terminal is wide will redraw incorrectly past the margin. `line_editor::LINE_MAX` keeps this rare;
  a full fix needs size negotiation the serial contract does not carry. Honest limit, recorded.
- Tab completion. Completion needs the command namespace, which is the application's
  knowledge, not the terminal's. Tab is ignored here. The shell completes by editing its own line
  over raw mode (DECISIONS §227 (how Tab reaches the shell), [shell-line-editing.md](shell-line-editing.md)).
- More than one concurrent reader. One session, one line reader (above).

## A known race, carried forward from milestone 10

The first byte of input piped in a single burst at boot can be lost once. The input driver arms its
receive interrupt a few instructions after the FIFO already holds the piped text, and the leading
byte can fall in that window. An interactive user typing after the prompt never hits it,
and every line after the first is intact. Fully closing it needs the driver armed before any
input arrives. Noted, not papered over; see [shell.md](shell.md).

## For milestones 29 and 31

- 29 (display terminal): built, 2026-07-30, and the prediction held. `components/src/display_terminal.rs`
  implements the *same IPC half* against a framebuffer and a VT engine instead of a serial line and
  this line discipline. `OPERATION_WRITE` prints from the client's output page, and
  `OPERATION_BYTES` carries keystrokes in from a driver (or from the compositor, forwarding to the
  focused client). The framing constants are these ones, unchanged. The wire half differs, as this
  note said it would: a
  grid, not a row.

  Two things this note could not have predicted, both recorded in [glyphs.md](glyphs.md):

  - The display terminal does not serve `OPERATION_READLINE`. It renders a stream and echoes
    keystrokes; it is not a line discipline. A client that wants edited lines composes `line_editor` in
    front of it and prints the discipline's echo through `OPERATION_WRITE`, which needs no new protocol at
    all, because `line_editor`'s echo is exactly a byte stream the VT engine parses. That is not a
    hope: `crates/video_terminal` proves it on the host by running both components against each other.
  - The one-endpoint consequence. A terminal has two classes of sender (an application printing,
    an input source typing) and a process here has one wait point (DECISIONS §33), so both arrive on
    one endpoint and are told apart by opcode, exactly as `line_editor` does. The security consequence is
    stated rather than hidden: an application holding that endpoint could send `OPERATION_BYTES` and forge a
    keystroke into its own terminal. It gains nothing by it, and the boundary that matters (one
    client's input not reaching another's) is the compositor's and is a capability there.
- 31 (capability shell) is a client of this contract. It reads lines through `OPERATION_READLINE`
  and prints through `OPERATION_WRITE`, and it adds the command semantics (completion, grant
  expressions) that the terminal deliberately does not carry.
