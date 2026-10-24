//! **The VT state engine: the grid a display terminal keeps.** Bytes in, a character grid out, plus
//! the rectangle that changed. Sans-IO like `line_editor`: no endpoint, no syscall, no framebuffer;
//! `components/src/display_terminal.rs` feeds it and paints what it says, and the kernel-side test
//! and `cargo xtask` run it to predict and grade the same picture.
//!
//! It implements the sequences the line discipline emits (notes/terminal-contract.md), plus cursor
//! motion, erase, SGR and RIS; anything else is **swallowed whole** rather than printed as garbage.
//! There is no alternate screen, scrolling region, reporting sequence or reflow. The sequences,
//! limits and design are in notes/video-terminal.md; the font is in notes/glyphs.md.
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
