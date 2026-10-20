//! **The VT state engine: the grid a display terminal keeps.** Bytes in, a character grid out, plus
//! the rectangle that changed. Sans-IO like `line_editor`: no endpoint, no syscall, no framebuffer;
//! `components/src/display_terminal.rs` feeds it and paints what it says. Because it is a value
//! with no IO, the expected picture is computable by anyone holding the same bytes: the terminal
//! component runs it to draw, the kernel-side test to predict the framebuffer, and `cargo xtask` on
//! the host to grade what QEMU scans out. Three witnesses, one definition.
//!
//! It implements the sequences the line discipline emits (notes/terminal-contract.md), checked by a
//! test that feeds the real `line_editor`'s echo to this parser, plus the screen verbs any program
//! expects:
//!
//! - Printable bytes, with **deferred wrap** at the right margin (see [`Vt::feed`]) and UTF-8: a
//!   multi-byte sequence takes one cell, drawn as `glyph_atlas`'s missing-glyph box past
//!   printable ASCII.
//! - `CR`, `LF` (scrolling into [`SCROLLBACK_ROWS`] of history), `BS`, `TAB`, `BEL` (ignored).
//! - `CSI A/B/C/D` cursor motion, `CSI H` / `CSI f` absolute positioning.
//! - `CSI J` erase in display, `CSI K` erase in line, both with all three modes.
//! - `CSI m` (SGR): reset; bold and italic (drawn in their own faces), dim, reverse, concealed and
//!   crossed-out, each with its off switch; underline in five styles (`4`, `21`, `4:0` to `4:5`)
//!   and its own color (`58`, `59`); the eight colors and their bright forms; and the 256-color
//!   and 24-bit forms, in both the `;` and the `:` spelling, for foreground and background. Blink
//!   is parsed and dropped: there is no clock.
//! - `ESC c` (RIS), a full reset.
//!
//! Anything else is **swallowed whole**, introducer and all, rather than printed as garbage.
//!
//! Not here: an alternate screen, origin mode, scrolling regions, tab stops other than every eight
//! columns, mouse, and any reporting sequence, since this engine never writes to its input and so
//! stays a value. No reflow: `MAX_COLS` and `MAX_ROWS` are fixed. The limits are in notes/glyphs.md.
//!
//! Name: ratified 2026-08-01 (calef), replacing `vt`. Refused `vt` (reads as *vector table* in a
//! kernel), `virtual_terminal` (not what VT stood for, and Linux's virtual consoles have it) and
//! `screen_grid` (the work is interpreting the protocol). Named for DEC's Video Terminals, and
//! `display_terminal` for the role.

#![no_std]

pub mod component;
pub mod handoff;
pub mod keymap;
pub mod script;

/// What the display terminal reports to whoever spawned it.
///
/// **Status, not contract**: no client can ask for any of this, and it is here rather than in the
/// terminal contract (`line_editor::proto`) because it is about the *component*, not about the terminal
/// a program talks to. The engine above is sans-IO and this module is three constants; nothing in
/// the engine reads them.
pub mod status;

mod cell;
mod damage;
mod geometry;
mod palette;
mod vt;

#[cfg(test)]
mod test_support;

pub use cell::{Attr, Cell, Colour};
pub use damage::{CellRect, Damage};
pub use geometry::{
    CELL_H, CELL_W, MAX_CELLS, MAX_COLS, MAX_ROWS, SCROLLBACK_CELLS, SCROLLBACK_ROWS,
};
pub use palette::{DEFAULT_BG, DEFAULT_FG, PALETTE};
pub use vt::Vt;
