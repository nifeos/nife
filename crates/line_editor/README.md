# line_editor

[![check](https://github.com/nifeos/line_editor/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/nifeos/line_editor/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/nifeos/line_editor/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/nifeos/line_editor/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/nifeos/line_editor#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://github.com/nifeos/line_editor/blob/main/Cargo.toml)

An allocation-free line editor for a serial terminal that does no input or output of its own. Bytes
go in one at a time, and echo comes out through a `Sink`.

The editor edits at a cursor, moves by control characters and ANSI (American National Standards
Institute) escape sequences, kills and yanks, and keeps eight lines of history. It ends a line on a
carriage return (CR), a line feed (LF) or a carriage return line feed (CRLF), and reports `^C`,
`^D`, `^Z` and Tab as an `Event`. It holds a few KiB of fixed buffers and never allocates.

## The contract

### Key bindings

| Input | Effect |
|---|---|
| printable ASCII (0x20 to 0x7e) | insert at the cursor |
| `^B`, `^F`, Left, Right | move one byte |
| `^A`, `^E`, Home, End | move to the start or the end |
| `^K` | kill from the cursor to the end into the kill buffer |
| `^U` | kill from the start to the cursor |
| `^W` | kill the word before the cursor |
| `^Y` | insert the kill buffer at the cursor |
| Backspace, Delete | delete before, or under, the cursor |
| Up, Down | step through history, the line in progress stashed |
| `^L` | clear the screen and repaint the prompt and the line |
| CR, LF, CRLF | end the line |
| `^D` | end of input on an empty line, delete under the cursor otherwise |
| `^C`, `^Z` | discard the line, and report an interrupt or a suspend |
| Tab | report an event; the caller decides what it means |

### Limits

| Constant | Value | What it bounds |
|---|---|---|
| `MAXIMUM_LINE_BYTES` | 256 | bytes in a line; the next printable byte is refused with a bell |
| `MAXIMUM_PROMPT_BYTES` | 64 | bytes of prompt repainted; the editor cuts a longer prompt short |
| `HISTORY_LINES` | 8 | history lines held; a new line overwrites the oldest |
| `MAXIMUM_RAW_QUEUE_BYTES` | 64 | bytes a `RawQueue` holds; the queue drops a burst past it |

## EXAMPLES

```rust
use line_editor::{Event, LineEditor, Sink};

struct Echo(Vec<u8>);
impl Sink for Echo {
    fn put(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }
}

let mut out = Echo(Vec::new());
let mut editor = LineEditor::new();
editor.start_line(b"> ", &mut out);
for byte in b"hello" {
    editor.feed(*byte, &mut out);
}
assert_eq!(editor.feed(b'\r', &mut out), Event::Line);
assert_eq!(editor.line(), b"hello");
```

## What is proven

Two Kani harnesses (`src/line_disc/verification.rs`), each with a falsification patch that turns it
red: a control sequence leaves the cursor inside the line, and a range delete removes exactly its
range. The rest is host tests.

## Handoff

`LineEditor::save` and `restore` write and read the editor's state as a blob. The `handoff` module
documents the layout and a test pins it. A change to the layout bumps `handoff::LAYOUT`.

## Build, test and prove

You need a Rust toolchain at or above the minimum version at the foot of this page (`rustup` is the
usual way to get one) and a git checkout. From the crate root:

- `cargo build` and `cargo test` run on the host and need nothing else. The tests are unit tests
  and doctests.
- The crate also builds for bare metal. Add the three targets with `rustup target add
  riscv64imac-unknown-none-elf aarch64-unknown-none-softfloat x86_64-unknown-none`. Then run
  `cargo build --lib --target <target>` for each.
- The proofs need Kani, which `rustup` does not install. Run `cargo install --locked
  kani-verifier --version 0.68.0`, then `cargo kani setup`. The setup step downloads a model
  checker, so it needs network access. After that, `cargo kani` runs both harnesses. A passing
  run prints a long list of checks and one summary line per harness; only the final
  `VERIFICATION:- SUCCESSFUL` and the line `Complete - 2 successfully verified harnesses` matter.
- Each harness has a falsification record at `falsifications/<module>.<harness>.patch`. Applying
  it with `git apply -p1` from the crate root makes that harness fail at its own assertion, which
  shows the proof checks something. Revert each one with `git checkout -- src` before the next.

## BUGS

- Input is ASCII only. The editor drops a byte at or above 0x80 without echo.
- The redraw math assumes one row, so a line that wraps past the margin redraws wrongly.
- History is eight lines, fixed, and the editor skips only a repeat of the newest line. The kill
  buffer holds one entry.
- The editor drops a Meta-prefixed key (Escape, then a key other than `[` or `O`) with its
  introducer.
- The editor cuts a prompt past the limit short.

## Minimum Rust version

Rust 1.88.

## License

MIT OR Apache-2.0, at your option: see `LICENSE-MIT` and `LICENSE-APACHE`.
