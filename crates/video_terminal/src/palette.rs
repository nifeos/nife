//! The sixteen ANSI colors, the compile-time gate that keeps them a test instrument, and the
//! default foreground and background.
//!
//! Name: provisional (the lane of milestone 874 (the terminal engine moves its palette and its
//! tests out), 2026-10-12 UTC), named for `PALETTE`, as that milestone's block proposed; calef
//! names things.

// Only the doc links below name the crate root's items, so the import exists only for rustdoc.
#[cfg(doc)]
use super::*;

/// **The sixteen ANSI colours**, normal then bright, as `0x00RRGGBB` words: Solarized Dark from
/// Schoonover's table (§104 (the rich-text font is DejaVu Sans Mono, and the palette is
/// Solarized)), but entry 14 is `0x93a1a0`, one unit off, so it passes the gate
/// ([`PaletteFaults`]). See notes/video-terminal/palette-keymap-and-script.md.
///
/// # BUGS
///
/// - The defaults are ANSI 7 on ANSI 0, `base2` on `base02`, not Solarized's `base0` on `base03`.
/// - The gate guards the sixteen, not every colour a cell can hold; the scanout comparison does.
/// - Whether one changed number still leaves this "Solarized" is calef's question under §104.
pub const PALETTE: [u32; 16] = [
    0x0007_3642, // 0 black: base02
    0x00dc_322f, // 1 red
    0x0085_9900, // 2 green
    0x00b5_8900, // 3 yellow
    0x0026_8bd2, // 4 blue
    0x00d3_3682, // 5 magenta
    0x002a_a198, // 6 cyan
    0x00ee_e8d5, // 7 white: base2
    0x0000_2b36, // 8 bright black: base03
    0x00cb_4b16, // 9 bright red: orange
    0x0058_6e75, // 10 bright green: base01
    0x0065_7b83, // 11 bright yellow: base00
    0x0083_9496, // 12 bright blue: base0
    0x006c_71c4, // 13 bright magenta: violet
    0x0093_a1a0, // 14 bright cyan: base1, nudged from 0x93a1a1 (see above)
    0x00fd_f6e3, // 15 bright white: base3
];

// The gate: a palette that stops being a test instrument does not compile. See [`PaletteFaults`].
const _: () = assert!(
    matches!(
        palette_faults(&PALETTE),
        PaletteFaults {
            repeated_channel: 0,
            permuted_pairs: 0,
            shared_saturation: 0
        }
    ),
    "PALETTE fails milestone 141's palette check: see PaletteFaults"
);

/// **What makes a palette a test instrument**, one field per property milestone 141 (a palette
/// worth looking at, and a gate that lets it be one) names, each closing one way a corrupted pixel
/// could land on a different legal colour. They are this tree's reconstruction, not a
/// specification; a fourth failure mode nobody has named would pass them. See
/// notes/video-terminal/palette-keymap-and-script.md.
///
/// Name: provisional (milestone 141's lane, 2026-09-26). Private to the crate: nothing outside
/// needs it, and the gate is the `const` assertion beside [`PALETTE`], not a caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PaletteFaults {
    /// Entries whose three channels are not all distinct, as a bit mask by index: a channel-order
    /// bug in the blit leaves such an entry untouched.
    repeated_channel: u16,
    /// Pairs of entries related by a channel permutation, the identity included, so a duplicated
    /// entry counts: swapping channels turns one legal colour into the other.
    permuted_pairs: u8,
    /// Channels (bit 0 red, 1 green, 2 blue) at `0xff` in more than one entry: a saturating write
    /// or a dropped shift tends to pin a channel at `0xff`.
    shared_saturation: u8,
}

impl PaletteFaults {
    const NONE: PaletteFaults = PaletteFaults {
        repeated_channel: 0,
        permuted_pairs: 0,
        shared_saturation: 0,
    };
}

/// The three channels of a `0x00RRGGBB` word, sorted, so two colours that are channel
/// permutations of each other compare equal. A three-element sorting network, because a `const fn`
/// cannot call `sort`.
const fn sorted_channels(c: u32) -> [u8; 3] {
    let (mut a, mut b, mut d) = ((c >> 16) as u8, (c >> 8) as u8, c as u8);
    if a > b {
        (a, b) = (b, a);
    }
    if b > d {
        (b, d) = (d, b);
    }
    if a > b {
        (a, b) = (b, a);
    }
    [a, b, d]
}

/// **Check a palette against [`PaletteFaults`]'s three properties.** A `const fn`, so the gate on
/// [`PALETTE`] is a compile error rather than a test someone has to run; the host tests exercise
/// it against a palette known to fail, which is what makes a clean result mean something.
///
/// Name: provisional (milestone 141's lane, 2026-09-26).
const fn palette_faults(p: &[u32; 16]) -> PaletteFaults {
    let mut f = PaletteFaults::NONE;
    let mut i = 0;
    while i < 16 {
        let [a, b, c] = sorted_channels(p[i]);
        if a == b || b == c {
            f.repeated_channel |= 1 << i;
        }
        let mut j = i + 1;
        while j < 16 {
            let (x, y) = (sorted_channels(p[i]), sorted_channels(p[j]));
            if x[0] == y[0] && x[1] == y[1] && x[2] == y[2] {
                f.permuted_pairs += 1;
            }
            j += 1;
        }
        i += 1;
    }
    let mut channel = 0;
    while channel < 3 {
        let (mut saturated, mut k) = (0, 0);
        while k < 16 {
            if (p[k] >> (8 * (2 - channel))) & 0xff == 0xff {
                saturated += 1;
            }
            k += 1;
        }
        if saturated > 1 {
            f.shared_saturation |= 1 << channel;
        }
        channel += 1;
    }
    f
}

/// The foreground a reset terminal writes with.
pub const DEFAULT_FG: u8 = 7;
/// The background a reset terminal clears to. Black, so an unwritten cell is the darkest thing on
/// the screen and a blank terminal is a defined picture rather than whatever the frames held.
pub const DEFAULT_BG: u8 = 0;

#[cfg(test)]
mod tests;
