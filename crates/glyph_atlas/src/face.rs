//! The four faces and the lookup into one.
//!
//! Name: provisional.

use crate::{CELL_H, CELL_W, FIRST, GLYPHS, LAST, blend};

/// One face: [`GLYPHS`] cells of coverage, top row first, 0 for background and 255 for ink.
///
/// **A slice, not `[u8; FACE_BYTES]`, on purpose, and that is a deliberate step down a rung.** The
/// array type would make a table of the wrong size a compile error, but the generator reads its
/// geometry from this crate, so a change to [`CELL_W`] would then stop the one tool that can make
/// the new tables from building. The size is held instead by the generator's regenerate gate and by
/// this crate's `the_tables_are_the_size_the_geometry_says`.
///
/// Name: provisional.
pub struct Face(pub(crate) &'static [u8]);

/// The regular face, the one the terminal draws with.
///
/// Name: provisional.
pub static REGULAR: Face = Face(include_bytes!("../coverage/regular.coverage"));
/// The bold face, for SGR 1.
///
/// Name: provisional.
pub static BOLD: Face = Face(include_bytes!("../coverage/bold.coverage"));
/// The italic face, DejaVu's Oblique (§166 (the rasteriser dependency, and whether the glyph
/// atlas ships one face or four)), for SGR 3.
///
/// Name: provisional.
pub static OBLIQUE: Face = Face(include_bytes!("../coverage/oblique.coverage"));
/// The bold italic face, for SGR 1 and 3 together.
///
/// Name: provisional.
pub static BOLD_OBLIQUE: Face = Face(include_bytes!("../coverage/bold_oblique.coverage"));

impl Face {
    /// **How much of the pixel at `(x, y)` in `ch`'s cell is ink**, 0 to 255. Out-of-cell
    /// coordinates are 0 rather than a panic: the callers are pixel loops.
    ///
    /// Name: provisional.
    pub fn coverage(&self, ch: char, x: u32, y: u32) -> u8 {
        if x >= CELL_W || y >= CELL_H {
            return 0;
        }
        let cp = ch as u32;
        let glyph = if (FIRST..=LAST).contains(&cp) {
            (cp - FIRST) as usize
        } else {
            GLYPHS - 1
        };
        self.0[glyph * (CELL_W * CELL_H) as usize + (y * CELL_W + x) as usize]
    }

    /// **The color `ch`'s cell shows at `(x, y)`**: `fg` over `bg` by the glyph's coverage there.
    /// The terminal paints with this, the kernel test predicts with it and the host grades with it.
    ///
    /// Name: provisional.
    pub fn cell_pixel(&self, ch: char, x: u32, y: u32, fg: u32, bg: u32) -> u32 {
        blend(fg, bg, self.coverage(ch, x, y))
    }
}
