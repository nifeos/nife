//! **A `Vt` as a §209 (state handoff is an opaque blob over a granted frame, and it is optional)
//! blob**: everything a replacement `display_terminal` needs to carry on where
//! its incumbent stopped (milestone 23 (a capability-routed component OS with live replacement);
//! calef's ruling of 2026-10-19 (UTC), "everything").
//!
//! The live grid, the scrollback in order, the cursor, the rendition and the parser's state, so a
//! sequence cut in half by a swap (`ESC [ 3` before it, `1 m` after) still means red. What is not
//! carried is the damage record: a replacement repaints its whole surface from the absorbed state,
//! which is what makes the screen after a swap a witness of the state rather than of the pixels
//! the incumbent left behind.
//!
//! The layout is fixed little-endian words, never a memory image of `Vt`, so a build with a
//! different field order or a different `Cell` padding reads it the same way. A cell is four words:
//! the character (a `char` needs 21 bits) with the rendition flags at bit 21 and the underline
//! style at bit 28, then the foreground, the background and the underline color. A color word's
//! top byte is its kind (`0` indexed, `1` 24-bit, `2` none, for an underline that follows the
//! foreground) and the low three bytes its value. Only the cells in use are written: `cols * rows` of the live grid,
//! then the scrollback's rows oldest first, so a blob is as long as the screen has history.
//!
//! # BUGS
//!
//! **A blob for a grid of another shape is refused, not reflowed** ([`Refusal::Geometry`]). The
//! replacement negotiates its geometry with the screen before it absorbs, so a swap that also
//! changes the screen's size, or a build that changes `MAX_CELLS` or the scrollback budget so the
//! old shape no longer fits, rolls back and the incumbent serves on. Reading an earlier layout is
//! kept across builds; reading another geometry is not attempted.
//!
//! Name: provisional (the lane for milestone 23, 2026-10-19 UTC), for this module and every item
//! in it. calef names things.

use crate::cell::{Attr, Cell, Colour};
use crate::geometry::{MAX_CELLS, SCROLLBACK_CELLS};
use crate::vt::MAX_PARAMS;

/// "VTRM", the blob's first word.
///
/// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
pub const MAGIC: u32 = 0x4d52_5456;
/// The layout this build writes. **A build reads every earlier one too** (calef, 2026-10-19 UTC:
/// a replacement reads its predecessor's handoff layout). So a change to the layout is a new
/// number, the reader for the old number stays, and the new layout's blob is added to the fixtures
/// in `handoff/tests.rs`, whose tests fail on a layout that changed without a bump and on a bump
/// without a fixture.
///
/// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
pub const LAYOUT: u32 = 2;

/// Words before the first cell: magic, layout, the twelve fields of the cursor and the
/// scrollback ring, the rendition's five, the parser's sub-parameter mask and its [`MAX_PARAMS`]
/// parameters.
pub(crate) const HEADER_WORDS: usize = 2 + 12 + 5 + 1 + MAX_PARAMS;
pub(crate) const CELL_WORDS: usize = 4;
/// Layout 1's header: the rendition had three words (no underline color or style) and the parser
/// no sub-parameter mask.
pub(crate) const HEADER_WORDS_1: usize = 2 + 12 + 3 + MAX_PARAMS;
/// Layout 1's cell: no underline color word, and underline a flag rather than a style.
pub(crate) const CELL_WORDS_1: usize = 3;
/// Layout 1's underline flag, which a layout-1 reader turns into a single underline.
pub(crate) const LAYOUT1_UNDERLINE: u32 = 1 << 2;

/// **The longest blob a `Vt` writes**: a full live grid and a full scrollback ring. 537,744 bytes,
/// so [`HANDOFF_PAGES`] is 132, the figure `component::SWAP_PAGES` and the swap reserve calef
/// sized are built on; [`MAX_CELLS`] and [`SCROLLBACK_CELLS`] are chosen to keep it there.
///
/// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
pub const HANDOFF_BYTES: usize = 4 * (HEADER_WORDS + CELL_WORDS * (MAX_CELLS + SCROLLBACK_CELLS));
/// [`HANDOFF_BYTES`] in page frames: what a supervisor retypes for a `display_terminal`'s handoff.
///
/// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
pub const HANDOFF_PAGES: u64 = HANDOFF_BYTES.div_ceil(address_space_map::PAGE as usize) as u64;

/// Why a blob was not absorbed. The `Vt` is untouched by every one of them: the whole blob is
/// checked before a cell is written.
///
/// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The first word is not [`MAGIC`]: no blob was written here.
    NoBlob,
    /// A blob of a layout this build does not read.
    Layout(u32),
    /// The blob is for a grid of another shape than the one this terminal negotiated. Its cells
    /// would land on the wrong rows, so it is refused rather than reflowed.
    Geometry,
    /// A field out of range, a cell that is not a character, or a blob cut short.
    Malformed,
}

pub(crate) const FLAG_WRAP_PENDING: u32 = 1 << 0;
pub(crate) const FLAG_CURSOR_VISIBLE: u32 = 1 << 1;
pub(crate) const FLAG_IGNORE: u32 = 1 << 2;
/// Every rendition flag `Attr` defines; a cell carrying another bit was not written by a `Vt`.
pub(crate) const KNOWN_ATTR_FLAGS: u32 = 0x7b;
/// Layout 1's flags, its underline bit among them.
pub(crate) const KNOWN_ATTR_FLAGS_1: u32 = 0x3f;
/// The most underline styles `Attr` defines: `0` none through `5` dashed.
pub(crate) const MAX_UNDERLINE_STYLE: u32 = 5;

pub(crate) const fn color_word(c: Colour) -> u32 {
    match c {
        Colour::Indexed(i) => i as u32,
        Colour::Rgb(r, g, b) => 1 << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32,
    }
}

pub(crate) const fn word_color(w: u32) -> Option<Colour> {
    match w >> 24 {
        0 if w <= 0xff => Some(Colour::Indexed(w as u8)),
        1 => Some(Colour::Rgb((w >> 16) as u8, (w >> 8) as u8, w as u8)),
        _ => None,
    }
}

/// An underline color's word: [`color_word`]'s, or kind `2` for none.
pub(crate) const fn underline_word(c: Option<Colour>) -> u32 {
    match c {
        Some(c) => color_word(c),
        None => 2 << 24,
    }
}

/// [`underline_word`] read back: `Some(None)` for kind `2`, `None` for a word no build writes.
pub(crate) const fn word_underline(w: u32) -> Option<Option<Colour>> {
    if w == 2 << 24 {
        return Some(None);
    }
    match word_color(w) {
        Some(c) => Some(Some(c)),
        None => None,
    }
}

/// The rendition from its words, checked: `None` for a flag or a style no `Attr` has.
pub(crate) fn words_attr(fg: u32, bg: u32, flags: u32, ul: u32, style: u32) -> Option<Attr> {
    if flags & !KNOWN_ATTR_FLAGS != 0 || style > MAX_UNDERLINE_STYLE {
        return None;
    }
    Some(Attr {
        fg: word_color(fg)?,
        bg: word_color(bg)?,
        ul: word_underline(ul)?,
        flags: flags as u8,
        underline: style as u8,
    })
}

/// Layout 1's rendition, in this build's terms: its underline flag becomes a single underline,
/// and it had no underline color.
pub(crate) fn words_attr_1(fg: u32, bg: u32, flags: u32) -> Option<Attr> {
    if flags & !KNOWN_ATTR_FLAGS_1 != 0 {
        return None;
    }
    let style = if flags & LAYOUT1_UNDERLINE != 0 { 1 } else { 0 };
    words_attr(fg, bg, flags & !LAYOUT1_UNDERLINE, 2 << 24, style)
}

pub(crate) fn cell_words(c: Cell) -> [u32; CELL_WORDS] {
    [
        c.ch as u32 | (c.attr.flags as u32) << 21 | (c.attr.underline as u32) << 28,
        color_word(c.attr.fg),
        color_word(c.attr.bg),
        underline_word(c.attr.ul),
    ]
}

pub(crate) fn words_cell(w: [u32; CELL_WORDS]) -> Option<Cell> {
    Some(Cell {
        ch: char::from_u32(w[0] & 0x1f_ffff)?,
        attr: words_attr(w[1], w[2], w[0] >> 21 & 0x7f, w[3], w[0] >> 28)?,
    })
}

/// A layout-1 cell: the flags in the top byte, and three words.
pub(crate) fn words_cell_1(w: [u32; CELL_WORDS_1]) -> Option<Cell> {
    Some(Cell {
        ch: char::from_u32(w[0] & 0x00ff_ffff)?,
        attr: words_attr_1(w[1], w[2], w[0] >> 24)?,
    })
}

pub(crate) fn word_at(blob: &[u8], i: usize) -> Option<u32> {
    let b = blob.get(4 * i..4 * i + 4)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

pub(crate) fn put_word(out: &mut [u8], i: usize, w: u32) {
    out[4 * i..4 * i + 4].copy_from_slice(&w.to_le_bytes());
}

#[cfg(test)]
mod tests;

// Written out for `component`'s reason: 36 header words and four a cell over 33,600 cells.
const _: () = assert!(HANDOFF_BYTES == 537_744);
const _: () = assert!(HANDOFF_PAGES == 132);
