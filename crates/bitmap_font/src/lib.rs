//! A 7x8 bitmap font and the arithmetic that turns a character into pixels: a constant table of
//! monochrome glyphs and a pure function from `(character, x, y)` to a color. The expected picture
//! is a function three parties evaluate independently (the terminal that draws with
//! [`cell_pixel`], the kernel test that checks, and the host that reads the scanout back out of
//! QEMU), which makes on-screen text provable. A mirrored font hides in symmetric letters, so
//! [`is_ink`]'s example prints an `F`. Not here: scalable outlines, kerning, proportional widths,
//! anti-aliasing, anything past basic latin ([`glyph`] draws a box for it) and font loading. See
//! notes/glyphs.md and notes/glyphs/bitmap-font.md.
//!
//! Name: ratified 2026-08-23 (calef), renamed from `bitfont` to spell out the contraction.

#![no_std]

pub mod glyphs;

mod render;

pub use render::{cell_pixel, glyph, is_ink};

/// A glyph cell's width in pixels. The font is fixed-pitch, so a terminal grid column is
/// `x / GLYPH_W` and nothing more. Seven is the Kaypro II's cell: its video board shifted out a
/// zero, five bits of character generator and a zero, so five ink columns sit between one-pixel
/// gutters (asserted in the tests). The gutter is the inter-character gap, so there is no tracking
/// to add. 128 / 7 is 18 columns where 128 / 8 was 16; the scanout is not a whole multiple of 7, so
/// the two rightmost pixels of a full-width surface stay background (see
/// `components/src/display_terminal.rs`).
pub const GLYPH_W: u32 = 7;

/// A glyph cell's height in pixels. Eight rows, of which the last carries only the descenders
/// (`g j p q y , ;`) and the underscore, so consecutive text rows do not collide.
pub const GLYPH_H: u32 = 8;

/// **The glyph for a byte this font has no picture for**: a hollow box, the convention every
/// terminal uses for "I cannot draw this".
///
/// It exists so that [`glyph`] is *total*. A font that returned `None`, or blank, for an unmapped
/// byte would make a mojibake bug look like a spacing bug, and the whole point of drawing something
/// visible is that a wrong byte in the grid shows up as a wrong picture rather than as nothing.
pub static MISSING: [u8; 8] = [0x3e, 0x22, 0x22, 0x22, 0x22, 0x22, 0x3e, 0x00];

#[cfg(test)]
mod tests;
