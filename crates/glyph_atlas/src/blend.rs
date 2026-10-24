//! Coverage to color, in linear light.
//!
//! Name: provisional.

use crate::SRGB_TO_LINEAR;

/// **`fg` over `bg` at `coverage` of 255, in linear light**: each `0x00RRGGBB` channel is taken
/// to linear light through [`SRGB_TO_LINEAR`], mixed there, and brought back to the sRGB code
/// whose linear value is nearest. 0 is exactly `bg` and 255 exactly `fg`.
///
/// Integer only, and the way back searches the same table, so no second table can disagree with
/// the first. Why linear light is in notes/glyphs/glyph-atlas.md.
///
/// ```
/// use glyph_atlas::blend;
///
/// assert_eq!(blend(0x00ff_ffff, 0x0000_0000, 0), 0x0000_0000);
/// assert_eq!(blend(0x00ff_ffff, 0x0000_0000, 255), 0x00ff_ffff);
/// assert_eq!(blend(0x00ff_ffff, 0x0000_0000, 128), 0x00bc_bcbc);
/// assert_eq!(blend(0x00ff_0000, 0x0000_00ff, 128), 0x00bc_00bb);
/// ```
///
/// Name: provisional.
pub const fn blend(fg: u32, bg: u32, coverage: u8) -> u32 {
    // Most of a screen is background or solid ink, and the kernel test runs this for every pixel
    // of the scanout; the two ends are also the two values the arithmetic below must return exactly.
    match coverage {
        0 => return bg,
        u8::MAX => return fg,
        _ => {}
    }
    let a = coverage as u32;
    channel(fg, bg, a, 16) | channel(fg, bg, a, 8) | channel(fg, bg, a, 0)
}

/// One channel of [`blend`], at bit `shift`: to linear light, mixed, and back.
const fn channel(fg: u32, bg: u32, a: u32, shift: u32) -> u32 {
    let f = SRGB_TO_LINEAR[((fg >> shift) & 0xff) as usize] as u32;
    let b = SRGB_TO_LINEAR[((bg >> shift) & 0xff) as usize] as u32;
    (to_srgb((f * a + b * (255 - a) + 127) / 255) as u32) << shift
}

/// The sRGB code whose linear value in [`SRGB_TO_LINEAR`] is nearest `linear`, the lower on a tie.
/// A binary search in eight fixed steps rather than a loop, so it ends by construction: the table
/// rises strictly, which `the_linear_table_rises_and_round_trips` holds it to.
pub(crate) const fn to_srgb(linear: u32) -> u8 {
    // The last code whose linear value is at most `linear`; code 0's is 0, so there always is one.
    let mut at = 0;
    at = step(at, 128, linear);
    at = step(at, 64, linear);
    at = step(at, 32, linear);
    at = step(at, 16, linear);
    at = step(at, 8, linear);
    at = step(at, 4, linear);
    at = step(at, 2, linear);
    at = step(at, 1, linear);
    if at < 255 && (SRGB_TO_LINEAR[at + 1] as u32) - linear < linear - SRGB_TO_LINEAR[at] as u32 {
        return (at + 1) as u8;
    }
    at as u8
}

/// One step of [`to_srgb`]'s search: `at + by` if that code's linear value is at most `linear`.
const fn step(at: usize, by: usize, linear: u32) -> usize {
    if SRGB_TO_LINEAR[at + by] as u32 <= linear {
        at + by
    } else {
        at
    }
}
