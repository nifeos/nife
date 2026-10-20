//! **The documentation crate**: markdown in, styled terminal bytes out, and the index that says
//! what exists. It is the pure half of the documentation service: a renderer (`render`, behind the
//! `markdown` feature), the byte layout of a search index ([`index`]), and the query that reads one.
//! No IO, syscall or endpoint is in it; `mdr/` is the program. The crate and program names differ on
//! purpose: the crate is a superset of what the program does.
//!
//! The parser is `pulldown-cmark`, so the input is CommonMark with tables and strikethrough. What a
//! page looks like on a terminal is this crate's: wrapped paragraphs at a two-column margin,
//! capitalised level-one headings, hanging list items, block quotes ruled with `|`, verbatim code
//! blocks, link and image destinations shown beside their text, and aligned tables.
//!
//! # BUGS
//!
//! - Width counts characters, not columns, so a wide CJK character counts as one.
//! - A table wider than the terminal shrinks its widest column, and a shrunk cell loses its tail on
//!   screen. Nothing wraps a cell.
//! - HTML is shown as the text it is, and a link reference definition (`[x]: url`) prints nothing of
//!   its own: its destination appears beside each link that uses it.
//! - An ordered list is numbered `1.` style whatever its source used, since the parser does not
//!   report whether the author wrote `1)`.
//!
//! Name: ratified 2026-09-13 (calef). Refused `manual`, `doc`, `documentation_system`, `markdown`.

#![no_std]

#[cfg(any(feature = "builder", feature = "markdown"))]
extern crate alloc;

pub mod index;
#[cfg(feature = "markdown")]
mod render;

#[cfg(feature = "markdown")]
pub use render::{Attr, Style, render};

/// Somewhere rendered bytes go.
///
/// The same shape as [`line_editor::Sink`](../line_editor/trait.Sink.html), and deliberately not
/// that trait: this crate does not depend on `line_editor`, and a renderer that needed the line discipline crate
/// to describe "bytes leave here" would have taken one for a four-line definition. A caller that
/// holds both can implement this over that in one line.
///
/// **An implementation may be called with an empty slice** and must tolerate it.
pub trait Sink {
    /// Take these bytes. There is no partial acceptance and no error: a sink that cannot take them
    /// drops them, which is the same choice `byte_sink_contract` makes for a destination that has gone.
    fn put(&mut self, bytes: &[u8]);
}
