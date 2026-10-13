---
status: NOT-STARTED
raised: 2026-09-26
promoted_from: scrollback-from-the-keyboard
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 668. Scrollback from the keyboard: shift and page up scrolls the display terminal's history

*Title and slug are drafts.*
Raised by milestone 142 (a text display good enough that people use
it instead of a GUI)'s lane. It went to wire the scrollback it built in August to a key and found
the key never reaches the component that holds the history.

The recommended route adds one opcode to the terminal contract, which two
programs agree on. The options, the recommendation and the seven questions are below.

The display terminal keeps 300 rows of history and can show any of them (`Vt::scroll_up` and
`Vt::scroll_down` in `crates/video_terminal`, built 2026-08-26). Nothing a person can press reaches
those two methods. This note is about why that is a routing question and not a small follow-up, and
which route to take.

## What is decided and built

- The key. Shift and page up, shift and page down, the convention of the Linux console, xterm,
  VTE and every emulator a reader has used. The keymap sends xterm's sequences for them since
  2026-09-26: `CSI 5;2~` and `CSI 6;2~` (`video_terminal::keymap`, with a test driving the real line
  discipline).
- The engine. `scroll_up(n)`, `scroll_down(n)`, and new output snapping the view back to live.
  Host-tested.

## Why it is a routing question

The component that owns the history is not the component that sees the keys. In the boot a person
actually uses (journey 1, `user::boot_graphical_terminal`):

```text
keyboard_driver or input ──OPERATION_BYTES──► line_editor ──OPERATION_WRITE (echo)──► display_terminal
```

`display_terminal` only ever receives output. A keystroke has already been consumed, edited and
echoed by the time anything reaches it. `CSI 5;2~` is swallowed by the line discipline as a
sequence it does not implement (which is correct: it must not type `5;2~` into the line).

## The options

| | Route | New wire | New capability | Covers raw mode (kilo) |
|---|---|---|---|---|
| **A** | `line_editor` recognizes the two sequences and sends a scroll request to the terminal on the endpoint it already `CALL`s | one opcode on the terminal contract | none | yes, if checked before raw passthrough |
| B | keystrokes go through the terminal first, which forwards them to the discipline, as xterm forwards to a pty | none | `display_terminal` gains a `CALL` on `line_editor` | yes |
| C | the keyboard driver acts on the key event and sends a scroll request to the terminal directly | one opcode | the driver gains `WRITE` on the terminal's endpoint | yes |
| D | the discipline echoes a private escape sequence the terminal treats as "scroll" | a private sequence in the output stream | none | yes |

A is recommended. The line discipline is the one component that sees every keystroke in every
wiring where a person types at a prompt, and it already holds the terminal's endpoint
(`components/src/line_editor.rs`, `MODE_DISPLAY`, slot 1). The terminal already tells its senders
apart by opcode on that one endpoint. That is a consequence of §33 (the compositor's authority is
memory, not messages), which `display_terminal.rs`'s module note spells out. A scroll request is
one more arm in a match that exists.

Why each other option loses:

- B reorders the whole input pipeline and makes two servers `CALL` each other.
  `display_terminal` already records one deadlock of exactly that shape (its module note, "the
  deadlock that taught us"), and this would add a second, between the discipline echoing and the
  terminal forwarding. It is the most faithful to how a Unix terminal emulator is built and the most
  expensive here.
- C puts the gesture in the right place in principle (Linux does it in the keyboard layer; see
  prior art) but not in this tree. On real boards the keystroke source is usually the UART
  (milestone 192 (a keyboard on real silicon)'s option A, `KeystrokeSource::Serial`), which sees
  bytes rather than key events. So C would need a second implementation in `input` and would still
  leave a serial-driven screen without scrolling. It also spends a capability slot in the driver,
  and `boot_graphical_terminal`'s own comment records that aarch64's progenitor has three left.
- D puts an input gesture into the output stream, where any program can print it and scroll a
  person's view out from under them. Every other option keeps output unable to do that.

## The seven questions, for A

1. Considered and refused: B, C and D above, each with its reason.
2. What the tree does in the analogous case: the discipline already turns a keystroke into a
   non-byte effect. ^C does not reach the application as a byte; it becomes an interrupt the
   application reads with `OPERATION_INTRCOUNT`. A scroll request is the same move toward the terminal
   instead of toward the application.
3. Prior art, from memory and marked as such: the Linux virtual console binds shift and page up
   to a `Scroll_Backward` action in its keymap, handled inside the VT layer rather than passed to
   the tty (the closest analogue to C). xterm, VTE and Alacritty intercept the key in the emulator
   before anything is written to the pty (the analogue to B). Neither was re-read for this note.
4. Is the premise true: yes, checked. `Vt::scroll_up`/`scroll_down` have no callers outside
   `crates/video_terminal` (`grep -rn scroll_up components kernel` is empty), and the line
   discipline's CSI dispatch (`crates/line_editor/src/lib.rs`, `csi_final`) has no arm for `5~` or
   `6~`.
5. Cost of A, estimated rather than measured. It needs one opcode constant in `line_editor::proto`
   and two arms in the discipline's CSI dispatch. It also needs a way to report a scroll request out
   of `feed` (an `Event` variant is the existing shape), one `CALL` in `components/src/line_editor.rs`, and one
   arm in `display_terminal.rs`. Perhaps sixty lines and a host test on each side. No capability,
   no slot.
6. Reversibility: the opcode is the only irreversible part, because two programs agree on it.
   Nothing has acted on it yet. The keymap sequences are xterm's and are not ours to change.
7. **Same cost, same choice?** Yes. A is also the cheapest, but the reason is that it needs no new
   authority and no new deadlock surface, which would hold if it cost more.

## What is blocking, and what is not

- **Blocking:** the opcode, because it is the terminal contract (`notes/terminal-contract.md`) and
  a wire two programs agree on. That is an architect's call under AGENTS.md's *move fast* tenet.
- **Also in the way, not an architect's call:** milestone 23 (component OS live replacement) is
  building the swap of exactly these two programs, `display_terminal` and `line_editor`, as of this
  note. Option A edits both. Sequencing A after that lane lands avoids a conflict in the files it
  is restructuring.
- **Not blocking:** how many rows one press scrolls. Half a screen is the common default; it is a
  constant in the terminal and can change freely.

## The work, once the route is chosen

For the recommended route (the line discipline sends the terminal a scroll request on the endpoint
it already `CALL`s):

- `line_editor::proto` gains the opcode; the discipline's CSI dispatch recognizes `CSI 5;2~` and
  `CSI 6;2~` (the keymap sends both since 2026-09-26) and reports a scroll request out of `feed`,
  including in raw mode, before bytes pass through to the application.
- `components/src/line_editor.rs` makes the `CALL` in `MODE_DISPLAY`, and does nothing in
  `MODE_CONSOLE`, where the host terminal at the far end of the UART scrolls itself.
- `components/src/display_terminal.rs` serves it with `Vt::scroll_up`/`scroll_down` and presents.
- A host test on each side, and a guest leg that proves a scroll reached the screen.

**Sequence it after milestone 23** (component OS live replacement): that lane is building the swap
of exactly these two programs, and this edits both.

## Index row

The display terminal keeps 300 rows of history but no key a person can press reaches it. Proposed: route shift and page up through the terminal contract, which adds one opcode two programs agree on.
