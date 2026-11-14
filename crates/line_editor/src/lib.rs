//! The line discipline engine: a sans-IO editor for a serial terminal.
//!
//! [`LineDisc`] takes one byte at a time and emits echo bytes and completed lines; it does no IO,
//! IPC, UART or endpoint work, so the `line_editor` component only wires it up. It edits at a
//! cursor, moves by control characters and ANSI sequences, kills and yanks, keeps [`HIST`] lines of
//! history, ends a line on CR, LF or CRLF, and reports ^C, ^D, ^Z and Tab as an [`Event`]. Output
//! newline translation is the free function [`expand_output`], and [`handoff`] carries the editor's
//! state across a replacement. See notes/line-discipline.md for the bindings and what it leaves
//! out, and notes/terminal-contract.md for the contract its echo implements.
//!
//! Name: ratified 2026-07-30 (calef) for the word and 2026-08-01 for the spelling, replacing
//! `linedisc` and then `lineedit`. Refused `linedisc` (a Unix term of art from the model this OS
//! rejects), `line_discipline` (overclaims: that covers echo, canonical mode, signals and flow
//! control) and `line_edit` (a verb phrase where its sibling `video_terminal` is a noun).

//! # BUGS
//!
//! - The redraw math assumes one row, so a line that wraps past the margin redraws wrongly;
//!   [`LINE_MAXIMUM`] keeps that rare (notes/line-discipline.md).

#![no_std]

// The crate root is an index (§282 (when a file is split, the parent becomes an index)): the
// engine, the raw queue and the output side are private modules re-exported here by name. The
// public module keeps its docs on its `pub mod` line below.

mod line_disc;
mod output;
mod raw_queue;

pub use line_disc::{Event, HIST, LINE_MAXIMUM, LineDisc, PROMPT_MAXIMUM};
pub use output::{Sink, expand_output};
pub use raw_queue::{RAW_QUEUE_MAXIMUM, RawQueue};

/// The line editor's state as a handoff blob (milestone 23 (a capability-routed component OS
/// with live replacement); §209 (state handoff is an opaque blob over a granted frame, and it is
/// optional)).
///
/// It carries what a replacement `line_editor` needs to carry on: the half-typed line and its
/// cursor, the kill buffer, the history ring and the prompt. It leaves out state that lasts one
/// keystroke: a half-received escape sequence, history browsing (the browsed line is the edit
/// line, which is carried) and the last completed line (already delivered or queued). The layout
/// is this crate's, versioned by [`handoff::LAYOUT`]; a supervisor never reads it.
///
/// Name: provisional (the lane for milestone 23, 2026-09-27).
pub mod handoff;

#[cfg(test)]
mod test_support;
