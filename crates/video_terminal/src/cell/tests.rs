use super::*;
use crate::test_support::attr;

// ---- survivor triage of the 2026-10-03 census ---------------------------------------------

/// Every rendition flag reads back as set, and as unset on a plain cell, through its own
/// predicate. Setting one twice leaves it set: it is an OR, not a toggle.
#[test]
fn each_flag_reads_back_through_its_own_predicate() {
    let plain = Attr::DEFAULT;
    for set in [
        plain.with(BOLD, true),
        plain.with(DIM, true),
        plain.with(REVERSE, true),
        plain.with(INVISIBLE, true),
    ] {
        assert_eq!(
            [
                set.is_bold(),
                set.is_dim(),
                set.reverse(),
                set.is_invisible()
            ]
            .iter()
            .filter(|&&f| f)
            .count(),
            1,
            "exactly the flag that was set"
        );
    }
    assert!(!plain.is_bold() && !plain.is_dim() && !plain.reverse() && !plain.is_invisible());
    assert!(plain.with(BOLD, true).with(BOLD, true).is_bold());
}

/// **Bold is a face and leaves the color as it was set**, the eight normal colors included: the
/// terminal draws `glyph_atlas::BOLD` instead of brightening, so a bold `7` is still `7`.
#[test]
fn bold_does_not_change_the_colour() {
    let bg = Colour::Indexed(0);
    for i in [1u8, 7, 8, 196] {
        let bold = Attr::new(Colour::Indexed(i), bg).with(BOLD, true);
        assert_eq!(bold.colours().0, Colour::Indexed(i).resolve());
    }
}

/// Dim is the midpoint of foreground and background, channel by channel, so two unequal
/// colours land on the average of each channel and not on a mix of channels.
#[test]
fn dim_averages_each_channel() {
    assert_eq!(midpoint(0x11_22_33, 0x33_44_55), 0x22_33_44);
    assert_eq!(midpoint(0xff_00_80, 0x00_ff_80), 0x7f_7f_80);
}

/// **A rendition names two palette entries, and reverse swaps which is ink.** Every other test
/// compares one `Attr` against another, which cannot see a packer that lost a field or a
/// `colours` that returns a constant: both sides move together. These are the numbers.
#[test]
fn a_rendition_resolves_to_the_palette_entries_it_names() {
    assert_eq!(Attr::DEFAULT.fg(), Colour::Indexed(DEFAULT_FG));
    assert_eq!(Attr::DEFAULT.bg(), Colour::Indexed(DEFAULT_BG));
    assert!(!Attr::DEFAULT.reverse());
    assert_eq!(Attr::DEFAULT.colours(), (PALETTE[7], PALETTE[0]));
    // A background that is not 7, so a mask that widened to "always 7" is visible.
    assert_eq!(attr(3, 4).colours(), (PALETTE[3], PALETTE[4]));
    assert_eq!(
        attr(3, 4).with(REVERSE, true).colours(),
        (PALETTE[4], PALETTE[3]),
        "reverse swaps ink and paper, it does not pick different colours"
    );
}

/// **The three ways to name a colour resolve to the numbers every emulator agrees on.** The
/// 256-colour table is arithmetic, so it is checked at its seams: the last of the sixteen, the
/// cube's first, a mid-cube entry whose three channels all differ (so a swapped axis fails),
/// the cube's last, and both ends of the grey ramp. Values are xterm's, written out.
#[test]
fn indexed_and_24_bit_colours_resolve_to_xterms_numbers() {
    assert_eq!(Colour::Indexed(15).resolve(), PALETTE[15]);
    assert_eq!(Colour::Indexed(16).resolve(), 0x00_0000);
    // 16 + 36*1 + 6*2 + 3 = 67: red step 1, green step 2, blue step 3.
    assert_eq!(Colour::Indexed(67).resolve(), 0x5f_87af);
    assert_eq!(Colour::Indexed(231).resolve(), 0xff_ffff);
    assert_eq!(Colour::Indexed(232).resolve(), 0x08_0808);
    assert_eq!(Colour::Indexed(255).resolve(), 0xee_eeee);
    assert_eq!(Colour::Rgb(0xb5, 0x89, 0x00).resolve(), 0xb5_8900);
}
