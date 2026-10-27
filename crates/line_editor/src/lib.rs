//! The line discipline engine: a sans-IO editor for a serial terminal.
//!
//! [`LineDisc`] takes one byte at a time and emits echo bytes and completed lines; it does no IO,
//! IPC, UART or endpoint work, so the `line_editor` component only wires it up. It edits at a
//! cursor, moves by control characters and ANSI sequences, kills and yanks, keeps [`HIST`] lines of
//! history, ends a line on CR, LF or CRLF, and reports ^C, ^D, ^Z and Tab as an [`Event`]. Output
//! newline translation is the free function [`expand_output`], and `proto` is the IPC framing. See
//! notes/line-discipline.md for the bindings and what it leaves out, and
//! notes/terminal-contract.md for the contract its echo and `proto` implement.
//!
//! Name: ratified 2026-07-30 (calef) for the word and 2026-08-01 for the spelling, replacing
//! `linedisc` and then `lineedit`. Refused `linedisc` (a Unix term of art from the model this OS
//! rejects), `line_discipline` (overclaims: that covers echo, canonical mode, signals and flow
//! control) and `line_edit` (a verb phrase where its sibling `video_terminal` is a noun).

//! # BUGS
//!
//! - The redraw math assumes one row, so a line that wraps past the margin redraws wrongly;
//!   [`LINE_MAX`] keeps that rare (notes/line-discipline.md).

#![no_std]

// The crate root is an index (§282 (when a file is split, the parent becomes an index)): the
// engine, the raw queue and the output side are private modules re-exported here by name. The
// three public modules keep their docs on their `pub mod` lines below.

mod line_disc;
mod output;
mod raw_queue;

pub use line_disc::{Event, HIST, LINE_MAX, LineDisc, PROMPT_MAX};
pub use output::{Sink, expand_output};
pub use raw_queue::{RAW_QUEUE_MAX, RawQueue};

/// The IPC framing of the terminal contract (notes/terminal-contract.md): opcodes, flags, and
/// limits shared by the discipline server, its clients, and the kernel-side tests. This is a
/// **userspace protocol**, not kernel ABI: the kernel routes these words without reading them.
pub mod proto;

/// What a supervisor routes to a `line_editor` it can replace (milestone 23 (a
/// capability-routed component OS with live replacement)): the capability half of the terminal
/// contract, beside the wire half in [`proto`].
///
/// It holds two declarations, one per output sink: the console's two-endpoint protocol, older than
/// §12 (call/reply IPC: a one-shot reply capability), which needs a reply endpoint, and
/// `display_terminal`'s one `CALL`, which does not. Both keep the slot order `line_editor` reads,
/// which the `const` assertions hold them to; notes/line-discipline.md ("The component's slots")
/// gives the order.
///
/// Name: provisional (the lane for milestone 23, 2026-09-27), for the module, both constants,
/// `HANDOFF_VA` and the role and contract strings.
pub mod component;

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
