---
status: BUILT
raised: 2026-08-25
built: 2026-08-27
---
# 169. `kilo`: the smallest real text editor, as the forcing function for raw terminal input

Built, 2026-08-27, as `rmle` rather than a literal `kilo.c` port. The raw-keystroke
primitive (`OP_RAWMODE`/`OP_READRAW`, `crates/line_editor`) is built and proven; the editor itself
is a Rust reimplementation of `kilo`'s spirit and scope (`components/src/rmle.rs`), not a port, because
DECISIONS §31's foreign-language seam as actually built is a one-shot call and cannot support
`kilo`'s own blocking event loop without new seam infrastructure (that infrastructure is
[milestone 181](0181-persistent-foreign-component.md), raised separately). **Named `rmle`, not
`kilo`**, calef, 2026-08-27, specifically to avoid two things being called `kilo` once a real
`kilo.c` port through milestone 181's extended seam exists; see `rmle.rs`'s own module doc for the
full naming note. This milestone's own primary deliverable, the raw-keystroke primitive, is
unaffected by the naming question either way.

A dependency review of Emacs, nano and vim, for a lightweight first text editor, found the same wall
under all three: [DECISIONS §31 (the foreign-language
seam)](../decisions/0031-foreign-language-seam.md) forbids C code from making a syscall or holding a
capability directly, so any C program's port is really "rewrite its syscall layer against nife's
Rust-mediated shim," and no editor sidesteps that by being smaller. What *does* scale with size is
how much of that rewrite there is to do. antirez's `kilo` (<https://github.com/antirez/kilo>) is a
public-domain, single-file, roughly 1,000-line terminal text editor written explicitly to have
almost no dependencies: raw termios mode and hand-written ANSI escape sequences, no ncurses, no
subprocess calls, no dynamic linking, no threads. It is the cheapest real program that exercises
nife's one missing terminal capability without also carrying nano's or vim's much larger
optional-feature surface.

## The one real gap this milestone exists to close

nife's terminal contract, [DECISIONS §21](../decisions/0021-terminal-in-userspace.md), is a **line
discipline**, not a curses substrate: `OP_READLINE` hands a program a finished line, and the
program "never sees a keystroke, an escape sequence, or an echo" (per that decision's own text).
`OP_WRITE` passes a program's own ANSI output straight through to the screen untouched, so the
*output* half of what `kilo` needs already exists. What does not exist is the *input* half: raw,
unbuffered, per-keystroke delivery with echo suppressed, which is what lets `kilo` see arrow keys,
Ctrl-key chords and Escape sequences as they happen rather than after a line is finished.

This milestone's real deliverable is not "get `kilo`'s C source to compile." It is: **design and
build the raw-keystroke input primitive nife's terminal layer does not have today**, prove it with
the smallest program that needs it, and leave that primitive in place for nano (see below) and
anything else that wants a screen editor's shape of input.

## What else `kilo` needs, and why each one is already answered

Checked directly against `kilo.c`'s own structure, not assumed from its reputation:

- No subprocess. `kilo` has no shell-out, no external spell-checker, no `:!cmd` equivalent. Its
  one filesystem interaction beyond open/read/write is a plain save, so it needs nothing from a
  fork/exec primitive nife does not have.
- No dynamic linking. `kilo` links nothing beyond the C standard library it's built against, and
  `crates/elf` already only ever loads a static `ET_EXEC` image; this is not a new constraint for
  it to hit.
- No threads. `kilo` is a single-threaded event loop (read a key, act, redraw); §105's decline
  of `std::thread::spawn` does not affect it.
- **Signals are decorative, not load-bearing.** `kilo` installs a `SIGWINCH` handler for terminal
  resize as a convenience; without it, the editor simply does not notice a resized terminal until
  the next redraw. [DECISIONS §101](../decisions/0101-notification-objects.md)'s notification
  objects are a plausible nife-native substitute if a lane wants the feature, but a first cut can
  ship without it and note the gap.
- File I/O is a straight port. Open the file named on the command line, read it into `kilo`'s
  in-memory row array, write it back on save. This is `files.c`-sized work translated onto whatever
  capability the program is handed for its target file, not a new mechanism.

## What this unblocks

Directly, [DECISIONS §31](../decisions/0031-foreign-language-seam.md)'s foreign-language seam gets its
first real, load-bearing C program beyond the confined `c_seam.c` spike, and the raw-keystroke
primitive this milestone has to build is reusable infrastructure, not a `kilo`-specific hack.

[Milestone 170](0170-nano-editor.md) is the direct follow-on, sequenced to start only
once this milestone's raw-input primitive exists: nano needs the exact same terminal capability at
roughly 25x the code size, plus an optional (skippable) subprocess dependency for spell-check and
external filtering that `kilo` never has to answer. Building `kilo` first is what turns "design a
raw-terminal-input primitive and prove it under a full-size editor at the same time" into two
separable pieces of work.

## What this does not decide

Whether `SIGWINCH`-equivalent resize notification is worth building now or left as a recorded gap;
whether the raw-input primitive should be a new syscall, a new `line_editor` opcode, or a capability
narrowing of the existing terminal object; and whether `kilo`'s optional syntax-highlighting feature
(a few hundred lines, off by default without a language database) is in scope for a first cut. Left
for whoever picks this up, informed by how the raw-input primitive actually gets designed.

## Follow-on

- **Milestone 170.** `nano`, sequenced to start once this milestone's raw-input primitive exists. It
  needs the identical terminal capability at roughly 25x the code size, plus the skippable
  subprocess dependency for spell-check and external filtering that `kilo` never has to answer.
- **Milestone 181.** A real `kilo.c` port. DECISIONS §31's foreign-language seam as built is a
  one-shot call and cannot carry `kilo`'s own blocking event loop, which is why this milestone
  shipped a Rust reimplementation named `rmle` instead, deliberately leaving the name `kilo` free.
- **Recorded.** `components/src/rmle.rs`, in the module doc a reader meets first: no `SIGWINCH`-equivalent
  resize notification, no syntax highlighting and no incremental search, so a resized terminal is
  not noticed until something else forces a redraw.

## Index row

A dependency review of Emacs, nano and vim for a possible nife port found the same wall under all
three (DECISIONS §31's C-syscall seam), and one real, reusable gap behind it: nife's terminal
contract is a line discipline with no raw-keystroke input. Built as `rmle`, a Rust
reimplementation of `kilo`'s spirit and scope rather than a literal port (the seam as built cannot
support an interactive event loop; that gap is milestone 181); the raw-keystroke primitive itself
(`OP_RAWMODE`/`OP_READRAW`) is the milestone's own real deliverable.
