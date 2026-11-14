# line_editor

An allocation-free line editor for a serial terminal that does no input or output of its own. Bytes
go in one at a time, and echo comes out through a `Sink`.

The editor edits at a cursor, moves by control characters and ANSI (American National Standards
Institute) escape sequences, kills and yanks, keeps eight lines of history, ends a line on a
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
| Tab | reported as an event; the caller decides what it means |

### Limits

| Constant | Value | What it bounds |
|---|---|---|
| `LINE_MAXIMUM` | 256 | bytes in a line; the next printable byte is refused with a bell |
| `PROMPT_MAXIMUM` | 64 | bytes of prompt repainted; a longer prompt is truncated |
| `HIST` | 8 | history lines held; the oldest is overwritten |
| `RAW_QUEUE_MAXIMUM` | 64 | bytes a `RawQueue` holds; a burst past it is dropped |

## EXAMPLES

```rust
use line_editor::{Event, LineDisc, Sink};

struct Echo(Vec<u8>);
impl Sink for Echo {
    fn put(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }
}

let mut out = Echo(Vec::new());
let mut editor = LineDisc::new();
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

`LineDisc::save` and `restore` write and read the editor's state as a blob whose layout is
documented on the `handoff` module and pinned by a test. A change to the layout bumps
`handoff::LAYOUT`.

## Building and testing

```sh
cargo test
```

## BUGS

- Input is ASCII only. A byte at or above 0x80 is dropped without echo.
- The redraw math assumes one row, so a line that wraps past the margin redraws wrongly.
- History is eight lines, fixed, and only a repeat of the newest line is skipped. The kill buffer
  holds one entry.
- A Meta-prefixed key (Escape, then a key other than `[` or `O`) is dropped with its introducer.
- A prompt past the limit is truncated.

## Minimum Rust version

The workspace's pinned toolchain; no older version is tested.

## License

Licensed under either of MIT or Apache-2.0, at your option.
