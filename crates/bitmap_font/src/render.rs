//! **Reading a glyph**: its rows for a character, and whether a pixel in its cell is ink or the
//! color the cell shows there.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// The rows of the glyph for `ch`, top row first, bit 0 the leftmost pixel.
///
/// `'\u{00}'..='\u{7f}'` come from the drawn table in [`glyphs`], whose control codes are blank
/// because a terminal never puts one in a cell (the VT engine consumes them). Every other `char`,
/// all of Unicode past basic latin, draws [`MISSING`], so a decoded multi-byte UTF-8 sequence takes
/// one cell as one box rather than a run of wrong pictures. A real repertoire needs a real font
/// (notes/glyphs/bitmap-font.md).
///
/// # Examples
///
/// Out-of-cell coordinates are not ink, and a character with no glyph draws a visible box:
///
/// ```
/// use bitmap_font::{is_ink, glyph, MISSING};
///
/// assert!(!is_ink('F', 7, 0)); // past the cell
/// assert!(!is_ink('F', 0, 99));
///
/// // Everything past basic latin (0x80 up, and every non-Latin `char` since the UTF-8
/// // increment) draws the missing-glyph box. A reader sees that the text is wrong instead of seeing
/// // a gap.
/// assert_eq!(glyph('\u{e9}'), &MISSING);
/// assert_ne!(glyph('F'), &MISSING);
/// ```
pub fn glyph(ch: char) -> &'static [u8; 8] {
    let cp = ch as u32;
    if cp >= 0x80 {
        return &MISSING;
    }
    match glyphs::BASIC.get(cp as usize) {
        Some(g) => g,
        None => &MISSING,
    }
}

/// Is the pixel at `(x, y)` **inside this cell** part of the glyph's ink?
///
/// Out-of-cell coordinates are not ink rather than a panic: the callers are pixel loops, and a
/// bounds check they can rely on is cheaper than one each.
///
/// # Examples
///
/// The expected picture can be printed. `F` is asymmetric, so a mirrored font shows; the first
/// and last columns are blank in every glyph, the Kaypro's one-pixel gutter:
///
/// ```
/// use bitmap_font::{GLYPH_H, GLYPH_W, is_ink};
///
/// let art: Vec<String> = (0..GLYPH_H)
///     .map(|y| (0..GLYPH_W).map(|x| if is_ink('F', x, y) { '#' } else { '.' }).collect())
///     .collect();
///
/// assert_eq!(
///     art,
///     [
///         ".#####.", // the top bar runs the full five ink columns
///         ".#.....", // the stem is on the LEFT
///         ".#.....",
///         ".####..", // the middle bar is one column short, which is what makes it an F
///         ".#.....",
///         ".#.....",
///         ".#.....",
///         ".......", // row 7 is the descender row, and F has none
///     ],
/// );
/// ```
pub fn is_ink(ch: char, x: u32, y: u32) -> bool {
    if x >= GLYPH_W || y >= GLYPH_H {
        return false;
    }
    glyph(ch)[y as usize] >> x & 1 != 0
}

/// The color a cell shows at `(x, y)`: `fg` where the glyph has ink, `bg` where it does not. The
/// whole of glyph rendering, pure on purpose: the terminal paints with it, the kernel test predicts
/// with it and the host-side scanout check grades QEMU's display with it, so none of the three can
/// be wrong in a way the others agree with.
///
/// # Examples
///
/// ```
/// use bitmap_font::{GLYPH_H, GLYPH_W, cell_pixel};
///
/// const WHITE: u32 = 0x00ff_ffff;
/// const BLACK: u32 = 0x0000_0000;
///
/// // The cell at column 3, row 0 of a terminal grid, holding a space.
/// let (col, row) = (3u32, 0u32);
/// for y in 0..GLYPH_H {
///     for x in 0..GLYPH_W {
///         assert_eq!(cell_pixel(' ', x, y, WHITE, BLACK), BLACK);
///     }
/// }
///
/// // And the grid arithmetic a fixed-pitch font buys: a pixel's cell is a division.
/// let (px, py) = (col * GLYPH_W + 1, row * GLYPH_H + 1); // column 1 is the F's stem
/// assert_eq!((px / GLYPH_W, py / GLYPH_H), (col, row));
/// assert_eq!(cell_pixel('F', px % GLYPH_W, py % GLYPH_H, WHITE, BLACK), WHITE);
/// ```
pub fn cell_pixel(ch: char, x: u32, y: u32, fg: u32, bg: u32) -> u32 {
    if is_ink(ch, x, y) { fg } else { bg }
}
