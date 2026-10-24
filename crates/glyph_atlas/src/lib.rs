//! **An anti-aliased monospace font, and the arithmetic that turns a character into pixels**:
//! DejaVu Sans Mono (§104 (the rich-text font is DejaVu Sans Mono, and the palette is Solarized))
//! at a 14x26 cell, its four faces (§166 (the rasteriser dependency, and whether the glyph atlas
//! ships one face or four)) as coverage tables, and a pure function from `(character, x, y, fg,
//! bg)` to a color.
//!
//! The tables under `coverage/` are `tools/glyph_atlas_generator`'s output, one byte of coverage a
//! pixel, and that generator's test fails unless they are what it makes from `vendor/dejavu`, byte
//! for byte. See notes/glyphs/glyph-atlas.md.
//!
//! # Examples
//!
//! The baseline sits [`ASCENT`] rows down, so an `L`'s foot is the row just above it, and its stem
//! is on the left:
//!
//! ```
//! use glyph_atlas::{ASCENT, CELL_W, REGULAR};
//!
//! let ink = |x: u32, y: u32| REGULAR.coverage('L', x, y) > 128;
//! assert!((0..CELL_W).filter(|&x| ink(x, ASCENT - 1)).count() >= 6, "the foot runs along the baseline");
//! assert!(!ink(CELL_W / 2 + 3, ASCENT - 8), "and above it the right half is empty");
//! assert!(ink(4, ASCENT - 8), "while the stem is on the left");
//! assert!((0..CELL_W).all(|x| !ink(x, ASCENT)), "nothing of an L is below the baseline");
//! ```

//! # BUGS
//!
//! - The repertoire is printable ASCII. Everything else draws the font's own missing-glyph box, as
//!   `bitmap_font` does. A bigger repertoire is a bigger table, up to the point (all of Unicode)
//!   where it is a different design.
//! - A glyph is clipped to its cell. The oblique faces lean up to 1.2 pixels past either side
//!   (`X`, `W`), which a terminal that overdraws its neighbors would show and this one cuts off.
//!
//! Name: provisional, for the crate and every item in it. calef names things.

#![no_std]

mod blend;
mod face;
mod geometry;
mod linear;

pub use blend::blend;
#[cfg(test)]
pub(crate) use blend::to_srgb;
pub use face::{BOLD, BOLD_OBLIQUE, Face, OBLIQUE, REGULAR};
pub use geometry::{
    ASCENT, CELL_H, CELL_W, FACE_BYTES, FIRST, GLYPHS, LAST, PIXELS_PER_EM, STRIKETHROUGH_ROW,
    UNDERLINE_ROW,
};
pub use linear::SRGB_TO_LINEAR;

#[cfg(test)]
mod tests;
