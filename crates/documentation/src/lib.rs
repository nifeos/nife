//! **The documentation crate**: markdown in, styled terminal bytes out, and the index that says
//! what exists. It holds a renderer (`render`, feature `markdown`), the search index's byte layout
//! and its query ([`index`]), and no IO; `mdr/` is the program. See notes/documentation.md.
//!
//! # BUGS
//! - Width counts characters, not columns, so a wide CJK character counts as one.
//! - A table wider than the terminal shrinks its widest column, and a shrunk cell loses its tail.
//! - HTML is shown as text, a link reference definition (`[x]: url`) prints nothing of its own, and
//!   an ordered list is numbered `1.` whatever its source used.
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
