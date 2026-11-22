//! The line discipline engine: an editor for a serial terminal that does no input or output of its
//! own.
//!
//! [`LineEditor`] takes one byte at a time and emits echo bytes and completed lines; it does no
//! input or output (IO), inter-process communication (IPC), universal asynchronous
//! receiver-transmitter (UART) or endpoint work, so the program that runs it only wires it
//! up. It edits at a cursor, moves by control characters and ANSI (American National Standards
//! Institute) sequences, kills and yanks, keeps [`HISTORY_LINES`] lines of history, ends a line on
//! a carriage return (CR), a line feed (LF) or both (CRLF), and reports ^C, ^D, ^Z and Tab as an
//! [`Event`]. Output newline translation is the free function [`expand_output`], and [`handoff`]
//! carries the editor's state across a replacement of the program that runs it.
//!
//! Name: ratified 2026-07-30 (calef) for the word and 2026-08-01 for the spelling, replacing
//! `linedisc` and then `lineedit`. Refused `linedisc` (a Unix term of art from the model this OS
//! rejects), `line_discipline` (overclaims: that covers echo, canonical mode, signals and flow
//! control) and `line_edit` (a verb phrase where its sibling `video_terminal` is a noun).

//! # Key bindings
//!
//! | Input | Effect |
//! |---|---|
//! | printable ASCII (0x20 to 0x7e) | insert at the cursor |
//! | `^B`, `^F`, Left, Right | move one byte |
//! | `^A`, `^E`, Home, End | move to the start or the end |
//! | `^K` | kill from the cursor to the end into the kill buffer |
//! | `^U` | kill from the start to the cursor |
//! | `^W` | kill the word before the cursor (blanks, then non-blanks) |
//! | `^Y` | insert the kill buffer at the cursor |
//! | Backspace (0x08, 0x7f), Delete | delete before, or under, the cursor |
//! | Up, Down | step through history, the line in progress stashed |
//! | `^L` | clear the screen and repaint the prompt and the line |
//! | CR, LF, CRLF | end the line ([`Event::Line`]) |
//! | `^D` | [`Event::Eof`] on an empty line, delete under the cursor otherwise |
//! | `^C` | discard the line ([`Event::Interrupt`]) |
//! | `^Z` | discard the line ([`Event::Suspend`]) |
//! | Tab | [`Event::Tab`], nothing echoed; the caller decides what it means |
//!
//! The arrow, Home, End and Delete keys arrive as `ESC [ ...` or `ESC O ...` sequences. An
//! unrecognized final byte drops the whole sequence, and an X10 mouse report (`ESC [ M` and three
//! payload bytes) is dropped with its payload. A second Escape is the same introducer.

//! # Limits
//!
//! | Constant | Value | What it bounds |
//! |---|---|---|
//! | [`MAXIMUM_LINE_BYTES`] | 256 | bytes in a line; the next printable byte is refused with a bell |
//! | [`MAXIMUM_PROMPT_BYTES`] | 64 | bytes of prompt repainted; a longer prompt is truncated |
//! | [`HISTORY_LINES`] | 8 | history lines held; the oldest is overwritten |
//! | [`MAXIMUM_RAW_QUEUE_BYTES`] | 64 | bytes a [`RawQueue`] holds; a burst past it is dropped |
//!
//! # Example
//!
//! The engine does no IO. Bytes go in one at a time, and echo comes out through a [`Sink`].
//!
//! ```
//! use line_editor::{Event, LineEditor, Sink};
//!
//! struct Echo(Vec<u8>);
//! impl Sink for Echo {
//!     fn put(&mut self, bytes: &[u8]) {
//!         self.0.extend_from_slice(bytes);
//!     }
//! }
//!
//! let mut out = Echo(Vec::new());
//! let mut editor = LineEditor::new();
//! editor.start_line(b"> ", &mut out);
//! for byte in b"hello" {
//!     editor.feed(*byte, &mut out);
//! }
//! assert_eq!(editor.feed(b'\r', &mut out), Event::Line);
//! assert_eq!(editor.line(), b"hello");
//! ```

//! # BUGS
//!
//! - Input is ASCII only. A byte at or above 0x80 is dropped without echo, so UTF-8 text typed
//!   into the line vanishes silently.
//! - The redraw math assumes one row, so a line that wraps past the margin redraws wrongly;
//!   [`MAXIMUM_LINE_BYTES`] keeps that rare. The editor tracks no terminal size, because asking for it
//!   would block on a terminal that never answers.
//! - History is [`HISTORY_LINES`] lines, fixed, and a line equal to the newest entry is not recorded again;
//!   an equal line further back is. The kill buffer holds one entry.
//! - A Meta-prefixed key (`ESC` then a key other than `[` or `O`) drops the introducer and the key.
//! - A prompt past [`MAXIMUM_PROMPT_BYTES`] is truncated, as the limit says.

#![no_std]
#![forbid(unsafe_code)]

// The crate root is an index: the engine, the raw queue and the output side are private modules re-exported here by name. The
// public module keeps its docs on its `pub mod` line below.

mod line_disc;
mod output;
mod raw_queue;

pub use line_disc::{Event, HISTORY_LINES, LineEditor, MAXIMUM_LINE_BYTES, MAXIMUM_PROMPT_BYTES};
pub use output::{Sink, expand_output};
pub use raw_queue::{MAXIMUM_RAW_QUEUE_BYTES, RawQueue};

/// The line editor's state as a handoff blob, which a running program passes to the program that
/// replaces it.
///
/// It carries what a replacement `line_editor` needs to carry on: the half-typed line and its
/// cursor, the kill buffer, the history ring and the prompt. It leaves out state that lasts one
/// keystroke: a half-received escape sequence, history browsing (the browsed line is the edit
/// line, which is carried) and the last completed line (already delivered or queued). The layout
/// is this crate's, versioned by [`handoff::LAYOUT`]; a supervisor never reads it.
///
/// # Layout
///
/// Every word is a little-endian `u32`, and a field is a length word followed by that many bytes.
/// The blob is, in order: [`handoff::MAGIC`], [`handoff::LAYOUT`], the line (a field), the cursor
/// (a word), the kill buffer (a field), the prompt (a field), the number of history entries held
/// and the next history slot (two words), then the eight history slots in slot order, each a field.
/// A [`RawQueue`] saves as one field of its unread bytes, oldest first. Any change to this order,
/// to a field's meaning or to a width bumps [`handoff::LAYOUT`]; a reader refuses every layout but
/// its own and reports which one it met.
///
/// With every buffer full the editor's part is 2688 bytes and a raw queue's part is 68, so both
/// fit one 4 KiB page. A refused blob ([`handoff::Refusal`]) may leave the editor half written,
/// so the caller discards it.
///
/// Name: provisional (the lane for milestone 23, 2026-09-27).
pub mod handoff;

#[cfg(test)]
mod test_support;
