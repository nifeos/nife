use super::*;
use crate::test_support::attr;
extern crate std;
use std::string::String;
use std::vec::Vec;

/// A terminal's rows as strings, for assertions that read like the screen.
fn rows(vt: &Vt) -> Vec<String> {
    (0..vt.rows())
        .map(|r| {
            let mut buf = [0u8; MAX_COLS];
            let n = vt.row_bytes(r, &mut buf);
            String::from_utf8_lossy(&buf[..n]).into_owned()
        })
        .collect()
}

// ---- survivor triage of the 2026-10-03 census ---------------------------------------------

/// `ESC [ 2 m` is dim, and nothing else in that sequence is.
#[test]
fn sgr_2_is_dim() {
    let mut vt = Vt::new(10, 3);
    vt.feed(b"\x1b[2mA\x1b[0mB");
    assert!(vt.cell(0, 0).attr.is_dim());
    assert!(!vt.cell(1, 0).attr.is_dim());
    vt.feed(b"\x1b[1m\x1b[1mC");
    assert!(vt.cell(2, 0).attr.is_bold());
}

/// An underline is drawn on the font's underline row, which is a pixel a blank cell otherwise
/// leaves in the background color.
#[test]
fn an_underline_is_drawn_on_the_underline_row() {
    let mut vt = Vt::new(10, 3);
    vt.feed(b"\x1b[4m ");
    let (fg, bg) = vt.cell(0, 0).attr.colours();
    assert_eq!(vt.pixel(0, UNDERLINE_ROW), fg);
    assert_eq!(vt.pixel(0, UNDERLINE_ROW - 1), bg);
    assert_eq!(vt.pixel(0, UNDERLINE_ROW + 1), bg);
    assert_eq!(vt.pixel(0, 0), bg);
}

/// A scroll lifts the old cursor block with the row it sat on: the drawn position moves up with
/// the pixels, so the report's box reaches one row above the cursor.
#[test]
fn a_scroll_reports_the_row_the_old_cursor_block_moved_to() {
    let mut vt = Vt::new(10, 3);
    vt.feed(b"\n\nabc");
    vt.take_damage();
    vt.feed(b"\n");
    let d = vt.take_damage().expect("a scroll is a report");
    assert_eq!(d.scrolled, 1);
    assert_eq!((d.rect.row, d.rect.rows), (1, 2), "{:?}", d.rect);
}

/// The foreground a cell actually paints with, bold and dim applied: what a person sees, which
/// is the thing an SGR test should be about now that bold is a flag rather than a rewritten
/// index.
fn ink(t: &Vt, col: u32, row: u32) -> u32 {
    t.cell(col, row).attr.colours().0
}

fn vt(cols: u32, rows: u32) -> Vt {
    let mut vt = Vt::new(cols, rows);
    vt.take_damage();
    vt
}

/// The grid's size in cells and in pixels, as numbers rather than as the expression that
/// computes them. 6 by 3 because 6*14, 6+14 and 6/14 are three different answers.
#[test]
fn the_grid_reports_its_size_in_cells_and_in_pixels() {
    let t = Vt::new(6, 3);
    assert_eq!((t.cols(), t.rows()), (6, 3));
    assert_eq!(t.width(), 84);
    assert_eq!(t.height(), 78);
}

/// **The pixel is a pure function of the grid**, so it can be pinned exactly. `L` at cell
/// (1, 1) in green, which starts at pixel (14, 26) because the cell is 14 by 26: its stem is the
/// cell's fourth column, the column left of it is a partial pixel, its top half holds nothing
/// right of the stem, and its foot reaches the twelfth column on the row above the baseline.
/// Those pixels separate every way the cell-versus-glyph coordinate split can go wrong (a
/// mixed-up divide and remainder agree at cell (0, 0), which is where every other pixel
/// assertion sits), the two axes use different divisors so a single constant cannot satisfy
/// both, and the partial pixel is the blend rather than either color.
#[test]
fn a_pixel_names_its_cell_and_its_place_inside_the_glyph() {
    let mut t = vt(4, 3);
    t.set_cursor_visible(false);
    t.feed(b"\x1b[2;2H\x1b[32mL");
    let (green, black) = (PALETTE[2], PALETTE[0]);
    assert_eq!(t.pixel(15, 36), black, "the cell's second column is margin");
    assert_eq!(t.pixel(17, 36), green, "the stem is its fourth");
    assert_eq!(
        t.pixel(16, 36),
        glyph_atlas::blend(green, black, 80),
        "and the stem's left edge is a partial pixel, blended"
    );
    assert_eq!(
        t.pixel(22, 36),
        black,
        "the L's upper half is stem and nothing else"
    );
    assert_eq!(
        t.pixel(25, 26 + glyph_atlas::ASCENT - 1),
        green,
        "its foot reaches the twelfth column on the row above the baseline"
    );
}

/// A column past the last one is off the grid, not the next row's first cell. The rows of the
/// grid are contiguous, so an out-of-range column that is not rejected reads a real cell.
#[test]
fn a_column_past_the_margin_is_not_the_next_rows_first_cell() {
    let mut t = vt(4, 2);
    t.feed(b"\x1b[2;1Hab");
    assert_eq!(t.cell(4, 0), Cell::default());
}

/// **Tabs stop every eight columns and never past the last one.** Nothing else feeds a tab, and
/// a line discipline does not emit one, but a program's output does.
#[test]
fn a_tab_advances_to_the_next_stop_and_stops_at_the_margin() {
    let mut t = vt(20, 2);
    t.feed(b"\t");
    assert_eq!(t.cursor(), (8, 0));
    t.feed(b"\t");
    assert_eq!(t.cursor(), (16, 0), "the stop is measured from where it is");
    t.feed(b"\t");
    assert_eq!(
        t.cursor(),
        (19, 0),
        "a tab at the right margin stops there rather than wrapping"
    );
}

/// Sixteen parameters is the limit and the limit is **legal**: a sixteen-parameter `CSI m`
/// acts, and only a seventeenth is dropped. Every other test here uses fewer, so the limit
/// itself was never judged from the inside.
#[test]
fn the_sixteenth_parameter_is_legal_and_the_seventeenth_is_not() {
    let mut t = vt(8, 1);
    // Fourteen harmless parameters, then bold and green: the last two are the ones that count.
    t.feed(b"\x1b[0;0;0;0;0;0;0;0;0;0;0;0;0;0;1;32ma");
    let want = attr(2, DEFAULT_BG).with(BOLD, true);
    assert_eq!(t.cell(0, 0).attr, want);
    t.feed(b"\x1b[0;0;0;0;0;0;0;0;0;0;0;0;0;0;0;0;31mb");
    assert_eq!(
        t.cell(1, 0).attr,
        want,
        "a sequence with too many parameters is swallowed, not half-applied"
    );
}

/// `CSI n J` from a cursor that is **not on the first row**, in the mode that erases backwards.
/// Both directions read `row * cols + col`, and at row 0 that arithmetic cannot be wrong.
#[test]
fn erase_in_display_starts_from_where_the_cursor_actually_is() {
    let mut t = vt(4, 3);
    t.feed(b"abcd\r\nefgh\r\nijkl\x1b[2;3H\x1b[J");
    assert_eq!(rows(&t), ["abcd", "ef  ", "    "]);

    let mut t = vt(4, 3);
    t.set_cursor_visible(false);
    t.feed(b"abcd\r\nefgh\r\nijkl");
    t.take_damage();
    t.feed(b"\x1b[2;3H\x1b[1J");
    assert_eq!(
        rows(&t),
        ["    ", "   h", "ijkl"],
        "mode 1 erases through the cursor's own cell and no further"
    );
    assert!(t.damage().is_some(), "erasing the screen is damage");
}

/// The rest of SGR: the bright foregrounds as their own sequences, and the three switches that
/// turn something *off*. A terminal that only ever set attributes would pass every other test
/// here and leave a line reversed forever.
#[test]
fn sgr_has_bright_colours_and_switches_that_turn_things_off() {
    let mut t = vt(8, 1);
    t.feed(b"\x1b[92ma");
    assert_eq!(t.cell(0, 0).attr, attr(10, DEFAULT_BG));
    t.feed(b"\x1b[1mb");
    assert_eq!(
        ink(&t, 1, 0),
        PALETTE[10],
        "bold on an already-bright colour is idempotent, not a toggle"
    );
    t.feed(b"\x1b[22mc");
    assert_eq!(
        ink(&t, 2, 0),
        PALETTE[10],
        "SGR 22 undoes bold, not a colour that was asked for bright"
    );
    t.feed(b"\x1b[7;41md\x1b[27me\x1b[39mf\x1b[49mg");
    assert_eq!(t.cell(3, 0).attr, attr(10, 1).with(REVERSE, true));
    assert_eq!(t.cell(4, 0).attr, attr(10, 1), "SGR 27");
    assert_eq!(t.cell(5, 0).attr, attr(DEFAULT_FG, 1), "SGR 39");
    assert_eq!(t.cell(6, 0).attr, Attr::DEFAULT, "SGR 49");
}

/// Text lands in the grid, `CR` returns to column 0, and `LF` goes down without returning.
/// The `\r\n` pair is what `line_editor::expand_output` produces from a Unix `\n`, so a terminal
/// that treated `LF` as `CRLF` would look right on that stream and wrong on every other.
#[test]
fn printing_moves_the_cursor_the_way_a_terminal_does() {
    let mut t = vt(8, 3);
    t.feed(b"hi");
    assert_eq!(t.cursor(), (2, 0));
    t.feed(b"\n");
    assert_eq!(t.cursor(), (2, 1), "LF alone must not return the carriage");
    t.feed(b"\r");
    assert_eq!(t.cursor(), (0, 1));
    t.feed(b"there");
    assert_eq!(rows(&t), ["hi      ", "there   ", "        "]);
    // Backspace steps back within the row and stops at the margin.
    t.feed(b"\x08\x08");
    assert_eq!(t.cursor(), (3, 1));
    t.feed(b"\r\x08");
    assert_eq!(
        t.cursor(),
        (0, 1),
        "backspace must not wrap to the row above"
    );
}

/// **Deferred wrap.** Filling the last column leaves the cursor on it; the next printable wraps.
/// A terminal that wrapped eagerly would scroll a full-width line before anything asked it to.
#[test]
fn the_right_margin_wraps_late_not_early() {
    let mut t = vt(4, 3);
    t.feed(b"abcd");
    assert_eq!(t.cursor(), (3, 0), "the cursor stays on the last column");
    assert_eq!(rows(&t)[1], "    ", "nothing has moved to the next row yet");
    t.feed(b"e");
    assert_eq!(t.cursor(), (1, 1));
    assert_eq!(rows(&t), ["abcd", "e   ", "    "]);

    // And a CR arriving right after a full line finds the cursor on the same row.
    let mut t = vt(4, 3);
    t.feed(b"abcd\rX");
    assert_eq!(rows(&t)[0], "Xbcd");
    assert_eq!(rows(&t)[1], "    ");
}

/// A line feed on the bottom row scrolls, and scrolling exposes a blank row rather than the
/// row that used to be there.
#[test]
fn the_bottom_row_scrolls() {
    let mut t = vt(4, 3);
    t.feed(b"one\r\ntwo\r\nsix\r\n");
    assert_eq!(rows(&t), ["two ", "six ", "    "]);
    assert_eq!(t.cursor(), (0, 2), "the cursor stays on the bottom row");
    t.feed(b"new\r\nend");
    assert_eq!(rows(&t), ["six ", "new ", "end "]);
    // Printing plus the cursor between them already dirty the whole grid here, and the scroll
    // is movement rather than change: the honest re-render box is still everything, but the
    // report says how many rows moved rather than that every cell changed. Two scrolls
    // happened (one before `six`'s line feed, one inside `new`'s).
    assert_eq!(
        t.damage().map(|d| d.repaint_rect(4, 3)),
        Some(CellRect {
            col: 0,
            row: 0,
            cols: 4,
            rows: 3
        }),
    );
    assert_eq!(t.damage().expect("the feeds above scrolled").scrolled, 2);

    // The same claim with nothing else in the frame. Above, the printing and the cursor
    // between them already dirty the whole grid, so a scroll that reported no damage at all
    // would still add up to the right rectangle. A bare LF on the bottom row moves every row
    // and writes no new cell, which is the only shape that can tell a scroll from a change
    // apart.
    let mut t = vt(4, 3);
    t.set_cursor_visible(false);
    t.feed(b"ab\r\ncd\r\nef");
    t.take_damage();
    t.feed(b"\r\n");
    assert_eq!(rows(&t), ["cd  ", "ef  ", "    "]);
    // One row of movement. The only cells to re-render are the ones the bottom row's old
    // content (`ef`) vacated when the scroll blanked it.
    assert_eq!(
        t.take_damage().map(|d| (d.rect, d.scrolled)),
        Some((
            CellRect {
                col: 0,
                row: 2,
                cols: 2,
                rows: 1
            },
            1
        )),
    );

    // And again, over a bottom row that is blank this time: pure movement, nothing to render.
    t.feed(b"\r\n");
    assert_eq!(rows(&t), ["ef  ", "    ", "    "]);
    assert_eq!(t.take_damage(), Some(Damage::new(CellRect::EMPTY, 1)));

    // The old whole-grid report is what a consumer that cannot move pixels must still paint,
    // and `repaint_rect` is the one-call spelling of it.
    assert_eq!(
        Damage::new(CellRect::EMPTY, 1).repaint_rect(4, 3),
        CellRect {
            col: 0,
            row: 0,
            cols: 4,
            rows: 3
        },
        "a scroll moves every row, so the repaint box is the whole grid",
    );
}

/// A scroll's report carries movement, and a painter that moves pixels wants the two halves
/// separately: enough scrolls between takes replace the whole screen and are reported as
/// content again, and a scroll never re-renders a single unchanged row.
#[test]
fn scrolls_report_movement_and_cap_at_the_screen() {
    let mut t = vt(4, 3);
    t.set_cursor_visible(false);
    t.feed(b"ab\r\ncd\r\nef");
    t.take_damage();

    // One scroll: `ef` moved from row 2 to row 1 without changing, and the only cells to
    // render are the ones its old position vacated when the bottom row blanked.
    t.feed(b"\r\n");
    let d = t.take_damage().expect("the grid scrolled");
    assert_eq!(d.scrolled, 1);
    assert_eq!(
        d.rect,
        CellRect {
            col: 0,
            row: 2,
            cols: 2,
            rows: 1
        },
        "just the vacated bottom row"
    );

    // A second scroll over the now-blank bottom row: movement only, nothing to render.
    t.feed(b"\r\n");
    let d = t.take_damage().expect("the grid scrolled again");
    assert_eq!(d.scrolled, 1);
    assert_eq!(d.rect, CellRect::EMPTY, "the bottom row opened blank");

    // Three scrolls between takes (one first, then two more) is every row of a three-row grid
    // replaced, so the report gives up the movement and says content.
    t.feed(b"\r\n");
    t.feed(b"\r\n\r\n");
    let d = t.take_damage().expect("the grid scrolled");
    assert_eq!(d.scrolled, 0, "a full replacement is content, not movement");
    assert_eq!(
        d.rect,
        CellRect {
            col: 0,
            row: 0,
            cols: 4,
            rows: 3
        }
    );
}

/// The cursor is drawn onto its cell rather than being part of it, so a scroll carries the
/// **rendered** block up a row while the cursor itself stays on the bottom row. The report has
/// to name the row the block landed on, or a painter that moves pixels leaves a trail of
/// stale blocks climbing the screen.
#[test]
fn a_scroll_reports_the_row_the_drawn_cursor_lands_on() {
    let mut t = vt(4, 4);
    t.feed(b"ab\r\ncd\r\nef\r\ngh"); // the cursor is on gh, row 3
    t.take_damage(); // the picture is taken with the block on (0, 3)

    t.feed(b"\r\n"); // scroll: the block's pixels move to row 2, the cursor stays at row 3
    assert_eq!(rows(&t), ["cd  ", "ef  ", "gh  ", "    "]);
    let d = t.take_damage().expect("the scroll is damage");
    assert_eq!(d.scrolled, 1);
    let (x, y, w, h) = d.rect.to_pixels();
    assert!(
        y <= 2 * crate::CELL_H && y + h >= 3 * crate::CELL_H,
        "the rect must cover the row the drawn block landed on (row 2), not just the bottom: \
         {w}x{h} at ({x},{y})"
    );
}

/// Cursor sequences, with the parameter defaults ANSI specifies: an absent or zero parameter is
/// one, and `CUP`'s parameters are one-based on the wire.
#[test]
fn the_cursor_sequences_use_ansi_defaults() {
    let mut t = vt(8, 4);
    t.feed(b"\x1b[3;5H");
    assert_eq!(t.cursor(), (4, 2), "CSI H is row;col and one-based");
    t.feed(b"\x1b[A");
    assert_eq!(t.cursor(), (4, 1), "an absent parameter means one");
    t.feed(b"\x1b[0B");
    assert_eq!(t.cursor(), (4, 2), "a zero parameter means one too");
    t.feed(b"\x1b[2D");
    assert_eq!(t.cursor(), (2, 2));
    t.feed(b"\x1b[99C");
    assert_eq!(t.cursor(), (7, 2), "motion clamps to the grid");
    t.feed(b"\x1b[99A");
    assert_eq!(t.cursor(), (7, 0));
    t.feed(b"\x1b[H");
    assert_eq!(t.cursor(), (0, 0), "CSI H with no parameters is home");
    // The far edges. A clamp that is one too generous parks the cursor off the grid, where
    // every subsequent write is silently discarded by `put`.
    t.feed(b"\x1b[99B");
    assert_eq!(t.cursor(), (0, 3), "downward motion clamps to the last row");
    t.feed(b"\x1b[99;99H");
    assert_eq!(t.cursor(), (7, 3), "CUP clamps on both axes");
}

/// Erasing, in all three modes of both verbs, and the property that makes `CSI K` useful to a
/// line discipline: it erases **to the end of the line** and leaves the cursor alone.
#[test]
fn erasing_clears_what_it_says_and_no_more() {
    let mut t = vt(6, 2);
    t.feed(b"abcdef\r\nghijkl");
    t.feed(b"\x1b[2;3H\x1b[K");
    assert_eq!(rows(&t), ["abcdef", "gh    "]);
    assert_eq!(t.cursor(), (2, 1), "erase in line must not move the cursor");
    t.feed(b"\x1b[1;4H\x1b[1K");
    assert_eq!(
        rows(&t),
        ["    ef", "gh    "],
        "mode 1 erases through the cursor"
    );
    t.feed(b"\x1b[2K");
    assert_eq!(rows(&t), ["      ", "gh    "]);

    let mut t = vt(6, 2);
    t.feed(b"abcdef\r\nghijkl\x1b[1;4H\x1b[J");
    assert_eq!(
        rows(&t),
        ["abc   ", "      "],
        "erase to the end of the screen"
    );
    let mut t = vt(6, 2);
    t.feed(b"abcdef\r\nghijkl\x1b[2J");
    assert_eq!(rows(&t), ["      ", "      "]);
    assert_eq!(t.cursor(), (5, 1), "CSI 2J does not home the cursor");
}

/// **Erasing writes spaces in the current rendition**, so clearing on a coloured background
/// leaves the background. A terminal that erased to black would leave a gap in a coloured line,
/// and that is exactly the redraw a line discipline does on every backspace.
#[test]
fn erasing_keeps_the_current_background() {
    let mut t = vt(4, 1);
    t.feed(b"\x1b[44mxy\x1b[K");
    for c in 0..4 {
        assert_eq!(
            t.cell(c, 0).attr.bg(),
            Colour::Indexed(4),
            "column {c} lost its background"
        );
    }
    assert_eq!(t.cell(3, 0).ch, ' ');
}

/// SGR: the colours, the reverse flag, and the two rules a terminal actually needs. Bold is a
/// face rather than a brighter foreground, and setting a color afterwards keeps it bold.
#[test]
fn sgr_sets_colour_bold_and_reverse() {
    let mut t = vt(8, 1);
    t.feed(b"\x1b[31ma");
    assert_eq!(t.cell(0, 0).attr, attr(1, DEFAULT_BG));
    t.feed(b"\x1b[1mb");
    assert_eq!(
        ink(&t, 1, 0),
        PALETTE[1],
        "bold must not brighten the color"
    );
    assert!(t.cell(1, 0).attr.is_bold());
    t.feed(b"\x1b[32mc");
    assert_eq!(ink(&t, 2, 0), PALETTE[2]);
    assert!(
        t.cell(2, 0).attr.is_bold(),
        "a new colour keeps the bold bit"
    );
    t.feed(b"\x1b[22md");
    assert_eq!(ink(&t, 3, 0), PALETTE[2]);
    t.feed(b"\x1b[7;44me");
    assert_eq!(t.cell(4, 0).attr, attr(2, 4).with(REVERSE, true));
    t.feed(b"\x1b[0mf");
    assert_eq!(t.cell(5, 0).attr, Attr::DEFAULT, "SGR 0 resets everything");
    t.feed(b"\x1b[mg");
    assert_eq!(t.cell(6, 0).attr, Attr::DEFAULT, "an empty SGR is a reset");
}

/// **Truecolour and the 256-colour form, as a program sends them**, including both in one
/// sequence (ten parameters, past the old limit of four) and a rendition after them, which is
/// the case that proves the extended forms consume exactly the parameters they name.
#[test]
fn sgr_takes_24_bit_and_256_colour_foregrounds_and_backgrounds() {
    let mut t = vt(8, 1);
    t.feed(b"\x1b[38;2;181;137;0ma");
    assert_eq!(t.cell(0, 0).attr.fg(), Colour::Rgb(181, 137, 0));
    t.feed(b"\x1b[48;5;67mb");
    assert_eq!(t.cell(1, 0).attr.bg(), Colour::Indexed(67));
    assert_eq!(
        t.cell(1, 0).attr.fg(),
        Colour::Rgb(181, 137, 0),
        "the foreground stays"
    );
    t.feed(b"\x1b[0;38;5;196;48;2;0;43;54;4mc");
    let c = t.cell(2, 0).attr;
    assert_eq!(
        (c.fg(), c.bg()),
        (Colour::Indexed(196), Colour::Rgb(0, 43, 54))
    );
    assert!(
        c.underline() == UNDERLINE_SINGLE,
        "the rendition after two extended colours was lost"
    );
    // Bold does not change the color, here a 256-color one.
    t.feed(b"\x1b[1md");
    assert_eq!(ink(&t, 3, 0), Colour::Indexed(196).resolve());
    // The bright backgrounds, which the one-byte rendition had no room for.
    t.feed(b"\x1b[0;104me");
    assert_eq!(t.cell(4, 0).attr.bg(), Colour::Indexed(12));
}

/// **A malformed extended colour ends the sequence's effect** rather than reading its missing
/// components out of whatever follows: a `38;2` with one component, an out-of-range component,
/// and an unknown colour space. What came *before* it in the same sequence still applies.
#[test]
fn a_malformed_extended_colour_is_not_half_applied() {
    let mut t = vt(8, 1);
    t.feed(b"\x1b[31;38;2;10ma");
    assert_eq!(t.cell(0, 0).attr, attr(1, DEFAULT_BG), "missing components");
    t.feed(b"\x1b[0;38;2;10;300;10;4mb");
    assert_eq!(t.cell(1, 0).attr, Attr::DEFAULT, "a component past 255");
    t.feed(b"\x1b[38;3;1;2;3;4mc");
    assert_eq!(
        t.cell(2, 0).attr,
        Attr::DEFAULT,
        "colour space 3 is not one"
    );
    t.feed(b"\x1b[38;5md");
    assert_eq!(
        t.cell(3, 0).attr,
        Attr::DEFAULT,
        "an index that never arrived"
    );
}

/// **Underline and strikethrough are ink across the whole cell, on their own rows**, and each
/// has an off switch. Read from the pixels, because a flag that is set and never drawn is the
/// failure a person would see and an attribute comparison would not. A space is used so every
/// ink pixel in the cell is the line's.
#[test]
fn underline_and_strikethrough_draw_a_full_width_line_on_their_rows() {
    let mut t = vt(4, 1);
    t.set_cursor_visible(false);
    t.feed(b"\x1b[4m \x1b[9m \x1b[24m \x1b[29m ");
    let (fg, bg) = Attr::DEFAULT.colours();
    let row_of = |col: u32, gy: u32| -> std::vec::Vec<u32> {
        (0..crate::CELL_W)
            .map(|gx| t.pixel(col * crate::CELL_W + gx, gy))
            .collect()
    };
    let full = std::vec![fg; crate::CELL_W as usize];
    let none = std::vec![bg; crate::CELL_W as usize];
    for gy in 0..crate::CELL_H {
        let under = gy == UNDERLINE_ROW;
        let strike = gy == STRIKETHROUGH_ROW;
        assert_eq!(
            &row_of(0, gy),
            if under { &full } else { &none },
            "underline, row {gy}"
        );
        assert_eq!(
            &row_of(1, gy),
            if under || strike { &full } else { &none },
            "both, row {gy}"
        );
        assert_eq!(
            &row_of(2, gy),
            if strike { &full } else { &none },
            "SGR 24, row {gy}"
        );
        assert_eq!(row_of(3, gy), none, "SGR 29, row {gy}");
    }
    assert_ne!(UNDERLINE_ROW, STRIKETHROUGH_ROW);
}

/// **Dim, invisible and the cursor, in the order [`Attr::colours`] documents.** Dim is the
/// channel midpoint toward the background; invisible paints the text in the background; and the
/// block cursor on a concealed cell still shows, which is what applying invisible last buys.
#[test]
fn dim_and_invisible_resolve_in_the_documented_order() {
    let red_on_blue = Attr::new(Colour::Rgb(200, 0, 0), Colour::Rgb(0, 0, 100));
    assert_eq!(
        red_on_blue.with(DIM, true).colours(),
        (0x64_0032, 0x00_0064)
    );
    assert_eq!(
        red_on_blue.with(INVISIBLE, true).colours(),
        (0x00_0064, 0x00_0064)
    );
    assert_eq!(
        red_on_blue
            .with(INVISIBLE, true)
            .with(REVERSE, true)
            .colours(),
        (0xc8_0000, 0xc8_0000),
        "reversed and concealed is a solid block of the old foreground"
    );

    let mut t = vt(4, 1);
    t.feed(b"\x1b[8mA\x1b[28mB\x1b[8m");
    let (fg, bg) = Attr::DEFAULT.colours();
    let ink_in = |col: u32| {
        (0..crate::CELL_W * crate::CELL_H).any(|i| {
            let (gx, gy) = (i % crate::CELL_W, i / crate::CELL_W);
            t.pixel(col * crate::CELL_W + gx, gy) == fg
        })
    };
    assert!(!ink_in(0), "concealed text drew ink");
    assert!(ink_in(1), "SGR 28 reveals");
    // The cursor sits on column 2, which is blank; conceal it and it is still a block.
    t.feed(b"\x1b[D\x1b[C");
    assert_eq!(t.cursor(), (2, 0));
    t.feed(b" \x1b[D");
    assert_eq!(
        t.pixel(2 * crate::CELL_W, 0),
        fg,
        "the cursor vanished on a concealed cell"
    );
    assert_ne!(fg, bg);
    assert!(t.cell(2, 0).attr.is_invisible());
}

/// **A sequence this engine does not implement is swallowed whole.** The failure mode this
/// prevents is the one everybody has seen: an unimplemented escape whose final byte lands on the
/// screen as a stray letter, corrupting a line that was otherwise fine.
#[test]
fn an_unknown_sequence_leaves_nothing_on_the_screen() {
    let mut t = vt(8, 1);
    t.feed(b"a\x1b[?25lb\x1b[38;5;120mc\x1b]0;title\x07d\x1bZe");
    assert_eq!(
        rows(&t)[0],
        "abcde   ",
        "an unimplemented sequence left bytes on the screen",
    );
    // A sequence split across two feeds is still one sequence: the parser is a state machine
    // across calls, which is what a byte-at-a-time IPC path needs.
    let mut t = vt(8, 1);
    t.feed(b"x\x1b");
    t.feed(b"[2");
    t.feed(b"Cy");
    assert_eq!(rows(&t)[0], "x  y    ");

    // A string sequence ends at ST (`ESC \`) as well as at BEL, and an ESC inside it that is
    // not ST is part of the string. Only the BEL terminator is exercised above, so the whole
    // ESC-in-a-string state was reachable but never distinguished from Ground.
    let mut t = vt(8, 1);
    t.feed(b"a\x1b]0;title\x1b\\b");
    assert_eq!(rows(&t)[0], "ab      ");
    let mut t = vt(8, 1);
    t.feed(b"a\x1b]x\x1bZy\x1b\\b");
    assert_eq!(
        rows(&t)[0],
        "ab      ",
        "an ESC that is not ST stays in the string"
    );

    // Control codes with no meaning here are consumed. Drawn instead, they are the font's
    // blanks or its missing-glyph box, in the middle of a line that was otherwise fine.
    let mut t = vt(8, 1);
    t.feed(b"a\x01\x02\x1f\x7fb");
    assert_eq!(rows(&t)[0], "ab      ");
}

/// The cursor is part of the picture, so moving it is damage; and a hidden cursor is not.
#[test]
fn the_cursor_is_painted_and_therefore_damages() {
    let mut t = vt(4, 2);
    let (fg, bg) = Attr::DEFAULT.colours();
    // Under the cursor, a blank cell shows the *background* colour as its ink field.
    assert_eq!(t.pixel(0, 0), fg, "the block cursor should invert its cell");
    assert_eq!(t.pixel(crate::CELL_W, 0), bg, "and only its own cell");

    t.take_damage();
    t.feed(b"\x1b[1;3H");
    let d = t.damage().expect("moving the cursor must be damage").rect;
    assert!(
        d.col == 0 && d.cols >= 3,
        "both cells must be in the damage"
    );

    t.set_cursor_visible(false);
    assert_eq!(t.pixel(2 * crate::CELL_W, 0), bg, "hidden means not drawn");
}

/// Damage is the bounding box of what changed, and writing a cell the value it already holds is
/// not a change. Without that, a terminal that repainted the same line would flush the screen
/// every frame and the damage rectangle would be decoration.
#[test]
fn damage_is_a_bounding_box_of_real_changes() {
    let mut t = vt(8, 4);
    t.set_cursor_visible(false);
    t.take_damage();
    assert_eq!(t.damage(), None, "a terminal at rest reports no damage");

    t.feed(b"\x1b[2;3Hab");
    assert_eq!(
        t.take_damage().map(|d| d.rect),
        Some(CellRect {
            col: 2,
            row: 1,
            cols: 2,
            rows: 1
        }),
    );
    assert_eq!(t.damage(), None, "taking the damage clears it");

    t.feed(b"\x1b[2;3Hab");
    assert_eq!(t.damage(), None, "rewriting identical cells is not damage");

    t.feed(b"\x1b[1;1HZ\x1b[4;8HY");
    let d = t.take_damage().unwrap().rect;
    assert_eq!(
        (d.col, d.row, d.cols, d.rows),
        (0, 0, 8, 4),
        "two far-apart changes cost the box that contains both",
    );
    assert_eq!(d.to_pixels(), (0, 0, 112, 104));
}

/// `ESC c` is a reset and not an erase: it clears in the *default* rendition and homes the
/// cursor, where `CSI 2J` does neither.
#[test]
fn ris_resets_the_rendition_too() {
    let mut t = vt(4, 2);
    t.feed(b"\x1b[41;33mab\x1bc");
    assert_eq!(t.cursor(), (0, 0));
    assert_eq!(t.cell(0, 0).attr, Attr::DEFAULT);
    assert_eq!(rows(&t), ["    ", "    "]);
}

/// **The engine parses what the line discipline emits**, checked against the real component
/// rather than against a list of sequences somebody wrote down.
///
/// This is the interoperability claim milestone 28's contract makes and milestone 29 relies on:
/// the display terminal's VT engine is fed the same echo stream the serial console gets, so a
/// sequence `line_editor` emits and this engine does not understand would be a hole between two
/// components that are otherwise separately correct. Feeding the actual editor closes it, and it
/// keeps closing it if `line_editor` changes its redraw strategy.
#[test]
fn it_understands_the_line_disciplines_echo() {
    struct Echo(Vec<u8>);
    impl line_editor::Sink for Echo {
        fn put(&mut self, bytes: &[u8]) {
            self.0.extend_from_slice(bytes);
        }
    }

    // Type "hello", back up two (^B), insert "XY", delete forward (^D), kill to end (^K), then
    // Enter. That is the editing that produces CSI D, CSI C, CSI K and a mid-line redraw, which
    // is the whole set a display terminal has to understand to show an edited line correctly.
    let mut ld = line_editor::LineDisc::new();
    let mut echo = Echo(Vec::new());
    let mut event = line_editor::Event::None;
    for &b in b"hello\x02\x02XY\x04\x0b\r" {
        event = ld.feed(b, &mut echo);
    }
    assert_eq!(event, line_editor::Event::Line);
    assert!(
        echo.0.windows(3).any(|w| w == b"\x1b[K"),
        "the discipline stopped emitting CSI K: this test is no longer interoperability",
    );
    assert!(
        echo.0.windows(3).any(|w| w == b"\x1b[D"),
        "the discipline stopped emitting cursor motion: likewise",
    );

    let mut t = vt(16, 3);
    t.set_cursor_visible(false);
    t.feed(&echo.0);
    // Whatever redraw strategy the discipline chose, the screen must show the line it completed.
    let line = String::from_utf8_lossy(ld.line()).into_owned();
    assert_eq!(line, "helXY");
    assert_eq!(
        rows(&t)[0].trim_end(),
        line,
        "the grid disagrees with the line the discipline assembled",
    );

    // And the ^L repaint (CSI 2J, CSI H, then the prompt and the line) drives this engine
    // correctly: the screen is cleared and the line is back at the top.
    let mut echo = Echo(Vec::new());
    for &b in b"redraw\x0c" {
        ld.feed(b, &mut echo);
    }
    assert!(echo.0.windows(4).any(|w| w == b"\x1b[2J"));
    t.feed(&echo.0);
    assert_eq!(
        rows(&t)[0].trim_end(),
        "redraw",
        "the ^L repaint did not land at the top of a cleared screen",
    );
    assert_eq!(rows(&t)[1].trim_end(), "", "the old screen survived a ^L");
}

/// Each compositor window shows its own banner and its own typed text, so a test that mixed
/// two windows up, or drew one twice, cannot pass. `script::window` is what the guest builds;
/// covering it here means a change to the shared script fails on the host in milliseconds
/// rather than in QEMU minutes later.
#[test]
fn each_window_shows_its_own_banner_and_its_own_typing() {
    let mut a = vt(script::COLS, script::ROWS);
    let mut b = vt(script::COLS, script::ROWS);
    script::window(&mut a, 0);
    script::window(&mut b, 1);
    let (ra, rb) = (rows(&a), rows(&b));
    assert_eq!(ra[0].trim_end(), "term0");
    assert_eq!(rb[0].trim_end(), "term1");
    assert_ne!(
        ra, rb,
        "two windows rendering identically would pass a mixed-up test"
    );
    assert!(
        ra.iter().any(|r| r.contains('A')) && rb.iter().any(|r| r.contains('B')),
        "each window must show what was typed at it, not at its neighbour",
    );
}

/// The script the milestone-29 tests drive is a fair one: it uses more than one row, more than
/// one rendition, and leaves a picture that a blank screen, a shifted screen, or a screen
/// missing its last write cannot match. Asserted here so the QEMU test is not the first thing to
/// find out otherwise.
#[test]
fn the_demo_script_produces_a_picture_worth_checking() {
    // Drive `script::full_screen` rather than rebuilding it here. The guest test renders
    // exactly that, so reconstructing the setup by hand let the two witnesses diverge: this
    // test fed GREETING and stopped, while the screen being graded in QEMU also had TYPED on
    // it. `script.rs` exists so every party agrees, which only works if every party calls it.
    let mut t = vt(script::COLS, script::ROWS);
    script::full_screen(&mut t);
    let seen = rows(&t);
    assert_eq!(seen[0].trim_end(), "nife");
    assert!(
        seen.iter().filter(|r| !r.trim().is_empty()).count() >= 3,
        "the script fills one row: a stride bug would be invisible",
    );
    let attrs: Vec<Attr> = (0..t.rows())
        .flat_map(|r| (0..t.cols()).map(move |c| (c, r)))
        .map(|(c, r)| t.cell(c, r).attr)
        .collect();
    assert!(
        attrs.iter().any(|a| *a != Attr::DEFAULT),
        "the script never changes rendition: the colour path is untested on the machine",
    );
    // The truecolour pass's own two claims reach the machine only if the script carries them:
    // a 24-bit colour, and a line rendition, each on a cell with ink.
    assert!(
        attrs.iter().any(|a| matches!(a.fg(), Colour::Rgb(..))),
        "the script has no 24-bit colour: truecolour is untested on the machine",
    );
    assert!(
        attrs.iter().any(|a| a.underline() != UNDERLINE_NONE),
        "the script has no underline: the line renditions are untested on the machine",
    );
    assert!(
        attrs.iter().any(|a| a.is_bold() && a.is_italic()),
        "the script has no bold italic: the faces are untested on the machine",
    );
    assert!(
        attrs
            .iter()
            .any(|a| a.underline() == UNDERLINE_CURLY && a.ul.is_some()),
        "the script has no styled, colored underline: increment five is untested on the machine",
    );
    assert!(
        (0..t.rows()).any(|r| (0..t.cols()).any(|c| t.cell(c, r).ch != ' ')),
        "the script drew nothing",
    );
}

/// **A wide grid keeps a shallower history, and the ring still wraps correctly at its width**
/// (2026-10-04): at 182 columns [`SCROLLBACK_CELLS`] holds 131 rows, and 200 lines
/// through a two-row grid must keep exactly the newest 131, oldest last.
#[test]
fn a_wide_grid_keeps_the_history_its_cell_budget_holds() {
    let mut t = vt(182, 2);
    assert_eq!(t.scrollback_depth(), 131);
    for i in 0..200u32 {
        t.feed(std::format!("L{i:03}\r\n").as_bytes());
    }
    assert_eq!(t.scrollback_len(), 131);
    t.scroll_up(u32::MAX);
    assert_eq!(t.view_offset(), 131);
    let row0: std::string::String = (0..4).map(|c| t.cell(c, 0).ch).collect();
    assert_eq!(
        row0, "L068",
        "the oldest kept line is the 131st from the newest push"
    );
    assert_eq!(
        vt(4, 2).scrollback_depth(),
        SCROLLBACK_ROWS as u32,
        "narrow grids keep 300"
    );
}

/// **The size clamp is exact at the boundary.** `MAX_COLS`/`MAX_ROWS` themselves are legal, one
/// past either is clamped down, and zero clamps up to one rather than producing an empty grid no
/// cursor could ever occupy. Milestone 326 (turn a mutation score upward)'s mutation triage.
#[test]
fn geometry_clamps_exactly_at_the_boundary_not_one_off_it() {
    // The widest grid gets the rows MAX_CELLS leaves it; the tallest needs a narrow one.
    let wide_rows = (MAX_CELLS / MAX_COLS) as u32;
    let tall_cols = (MAX_CELLS / MAX_ROWS) as u32;
    for (cols, rows) in [(MAX_COLS as u32, wide_rows), (tall_cols, MAX_ROWS as u32)] {
        let t = Vt::new(cols, rows);
        assert_eq!(
            (t.cols(), t.rows()),
            (cols, rows),
            "the boundary value itself must not be clamped"
        );
        let t = Vt::new(cols, rows + 1);
        assert_eq!(
            (t.cols(), t.rows()),
            (cols, rows),
            "one row past the boundary must be clamped"
        );
    }
    let t = Vt::new(MAX_COLS as u32 + 1, 1);
    assert_eq!(
        t.cols(),
        MAX_COLS as u32,
        "one column past the boundary must be clamped"
    );
    // The two screens this was sized for fit whole: OVMF's 1280x800 and xenon's 1920x1080 at the
    // 14x26 cell.
    for (cols, rows) in [(91, 30), (137, 41)] {
        assert_eq!(
            (Vt::new(cols, rows).cols(), Vt::new(cols, rows).rows()),
            (cols, rows)
        );
    }

    let t = Vt::new(0, 0);
    assert_eq!(
        (t.cols(), t.rows()),
        (1, 1),
        "a zero-sized grid has nowhere for the cursor to be"
    );
}

/// **`reset_to` clears the grid, the scrollback and the geometry, in place.** Nothing before this
/// test called it at all: every other test either builds a `Vt` at its final size with `Vt::new`
/// or never resizes, so the whole function was dead as far as the suite could tell. Milestone
/// 326's mutation triage.
#[test]
fn reset_to_clears_everything_and_retargets_the_geometry() {
    let mut t = Vt::new(4, 2);
    t.feed(b"one\r\ntwo\r\nthree\r\n"); // scrolls at least once, building real scrollback
    assert!(
        t.scrollback_len() > 0,
        "test setup: something must have scrolled into history"
    );
    t.take_damage();

    t.reset_to(3, 5);
    assert_eq!(
        (t.cols(), t.rows()),
        (3, 5),
        "the geometry actually changed"
    );
    assert_eq!(t.cursor(), (0, 0), "the cursor went home");
    for r in 0..t.rows() {
        let mut buf = [0u8; 3];
        let n = t.row_bytes(r, &mut buf);
        assert_eq!(&buf[..n], b"   ", "row {r} carried old content forward");
    }
    assert_eq!(
        t.take_damage().map(|d| (d.rect, d.scrolled)),
        Some((
            CellRect {
                col: 0,
                row: 0,
                cols: 3,
                rows: 5
            },
            0
        )),
        "a retarget is damage across the whole new grid",
    );
    assert_eq!(
        t.scrollback_len(),
        0,
        "reset_to must clear the old grid's history, not just the live view"
    );
}

/// **Multi-byte UTF-8 decodes to the right code point**, exercising the shift-and-accumulate
/// arithmetic and the lead-byte masks for all three lengths this engine understands, plus the two
/// ways a sequence can go wrong: truncation (a non-continuation byte arrives early) and a byte
/// that can never start a sequence. Nothing before this test fed a non-ASCII byte at all.
/// Milestone 326's mutation triage.
#[test]
fn utf8_decodes_every_sequence_length_and_recovers_from_a_bad_one() {
    // An accented Latin letter: two bytes, one continuation.
    let mut t = vt(8, 1);
    t.feed("é".as_bytes());
    assert_eq!(t.cell(0, 0).ch, 'é');

    // A CJK character: three bytes, two continuations.
    let mut t = vt(8, 1);
    t.feed("日".as_bytes());
    assert_eq!(t.cell(0, 0).ch, '日');

    // An astral character outside the BMP: four bytes, three continuations.
    let mut t = vt(8, 1);
    t.feed("🎉".as_bytes());
    assert_eq!(t.cell(0, 0).ch, '🎉');

    // A truncated sequence: the lead byte of a two-byte character, then a byte that is not a
    // continuation byte. The truncated character draws the replacement, and the byte that
    // interrupted it is reprocessed as its own character rather than eaten.
    let mut t = vt(8, 1);
    t.feed(b"\xc3Z");
    assert_eq!(t.cell(0, 0).ch, '\u{fffd}', "truncated sequence");
    assert_eq!(
        t.cell(1, 0).ch,
        'Z',
        "the interrupting byte must still be typed"
    );

    // A byte that can never start a sequence (a bare continuation byte): the replacement,
    // immediately, with nothing held waiting for continuation bytes that were never coming.
    let mut t = vt(8, 1);
    t.feed(b"\x80A");
    assert_eq!(t.cell(0, 0).ch, '\u{fffd}', "a lone continuation byte");
    assert_eq!(t.cell(1, 0).ch, 'A');
}

/// **Scrollback survives the ring's wrap, and `Vt::cell` reads it back in the right order.**
/// Feeding 350 lines through a two-row grid scrolls 349 times, comfortably past
/// [`SCROLLBACK_ROWS`] (300): the ring overwrites its oldest 49 pushes, and this is the test that
/// the retained 300 are the right ones, at the right ages, at both edges of the ring rather than
/// only near where it happens to start. It doubles as the test for [`SCROLLBACK_CELLS`] itself:
/// that constant sizes the `scrollback` array, nothing else in this suite pushes past
/// `SCROLLBACK_CELLS / cols` rows, and a wrong constant here is an index past a too-small array,
/// which panics rather than silently misdrawing a pixel. Milestone 326's mutation triage: before
/// this test, nothing in the suite called `scroll_up`, `scroll_down`, `view_offset`, or
/// `scrollback_len`, and nothing scrolled far enough to read scrollback at all.
#[test]
fn scrollback_survives_the_rings_wrap_and_reads_back_in_order() {
    const LINES: u32 = 350;
    let mut t = vt(4, 2);
    for i in 0..LINES {
        t.feed(std::format!("L{i:03}\r\n").as_bytes());
    }

    // 349 scrolls happened (every line after the first fills the grid and pushes one row), and
    // the ring caps at SCROLLBACK_ROWS.
    assert_eq!(t.scrollback_len(), SCROLLBACK_ROWS as u32);

    // The live grid holds the last line printed; nothing was fed after it, so the bottom row is
    // still blank.
    assert_eq!(rows(&t), ["L349", "    "]);

    // Scrolling clamps at the ring's true depth, not at whatever was asked for.
    t.scroll_up(u32::MAX);
    assert_eq!(t.view_offset(), SCROLLBACK_ROWS as u32);

    // Six checkpoints spanning the ring, including both edges: age 0 (the row pushed right
    // before the wrap stopped, where `sb_tail` last wrote) and age SCROLLBACK_ROWS - 1 (the
    // oldest survivor, on the far side of the ring from where it is currently writing). A wrong
    // `+`/`*`/`%` in the ring's index arithmetic is very unlikely to agree with the right one at
    // all six. Both display rows are checked, not just the top one: at row 0, `Vt::cell`'s
    // `age = view_offset - 1 - row` has `row == 0`, so a `-row` mutated to `+row` is invisible
    // there and needs row 1 (and, at `offset == 1`, row 1 comes from the *live* half of
    // `Vt::cell` instead, which is what exercises `live_row = row - view_offset`).
    for offset in [1u32, 2, 50, 150, 299, 300] {
        t.scroll_down(u32::MAX);
        t.scroll_up(offset);
        assert_eq!(
            t.view_offset(),
            offset,
            "view_offset did not reach what was asked for"
        );
        let seen = rows(&t);
        assert_eq!(
            seen[0],
            std::format!("L{:03}", LINES - 1 - offset),
            "view_offset {offset}: wrong row surfaced from the ring at row 0"
        );
        assert_eq!(
            seen[1],
            std::format!("L{:03}", LINES - offset),
            "view_offset {offset}: wrong row surfaced from the ring at row 1"
        );
    }

    // Scrolling back down reaches live ground exactly at zero, not before or after it.
    t.scroll_down(u32::MAX);
    assert_eq!(t.view_offset(), 0);
    assert_eq!(rows(&t), ["L349", "    "]);

    // Typing snaps the view back to live even from deep in history.
    t.scroll_up(200);
    assert_ne!(t.view_offset(), 0);
    t.feed(b"!");
    assert_eq!(
        t.view_offset(),
        0,
        "new output must snap the view back to live"
    );
}

/// **`put` and `damage_cell` refuse a coordinate where only one axis is out of range.** Nothing
/// in this crate ever calls either with such a coordinate (every caller clamps first: `print`
/// always writes at the cursor, which is always in range, and `erase_line` bounds its column with
/// `.min(self.cols)`), so this reaches them directly, the way this module already can
/// (`use super::*` at the top of `mod tests`, which is the same crate as the private methods
/// under test). Milestone 326's mutation triage.
#[test]
fn put_and_damage_cell_reject_a_partly_out_of_range_coordinate() {
    let mut t = vt(4, 3);
    let mark = Cell {
        ch: 'X',
        attr: Attr::DEFAULT,
    };

    // Column out of range, row in range.
    t.put(t.cols(), 1, mark);
    assert_eq!(
        t.cells,
        [Cell::default(); MAX_CELLS],
        "a partly out-of-range put must be a no-op"
    );
    assert_eq!(t.damage(), None);

    // Row out of range, column in range.
    t.put(1, t.rows(), mark);
    assert_eq!(t.cells, [Cell::default(); MAX_CELLS]);
    assert_eq!(t.damage(), None);

    t.damage_cell(t.cols(), 1);
    assert_eq!(
        t.damage(),
        None,
        "a partly out-of-range damage_cell must be a no-op"
    );
    t.damage_cell(1, t.rows());
    assert_eq!(t.damage(), None);
}

/// **A painter that moves pixels and renders only the reported box ends on the same picture
/// as one that renders every pixel.** This is the invariant `Damage` states, checked as a
/// model of `display_terminal`'s present (move `scrolled` glyph rows up across the whole
/// surface, then render `rect`), fed the firmware-screen test's own script one write at a
/// time, on a surface taller than the grid the way a real scanout leaves a strip below it.
#[test]
fn moving_pixels_and_rendering_the_damage_reproduces_the_whole_picture() {
    // One present per write (the console unbatched), and several writes per present (a
    // batch window, or a terminal that drained more than one request before presenting).
    moving_pixels_with_k::<1>();
    moving_pixels_with_k::<2>();
    moving_pixels_with_k::<3>();
    moving_pixels_with_k::<7>();
}

fn moving_pixels_with_k<const K: usize>() {
    use crate::{CELL_H as GLYPH_H, CELL_W as GLYPH_W};
    let (cols, rows) = (3, 5);
    let (sw, sh) = (cols * GLYPH_W + 2, rows * GLYPH_H + 3);
    let mut t = Vt::new(cols, rows);
    let mut surface = std::vec![0u32; (sw * sh) as usize];
    let render = |t: &Vt, s: &mut [u32], x0: u32, y0: u32, w: u32, h: u32| {
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                s[(y * sw + x) as usize] = t.pixel(x, y);
            }
        }
    };
    let mut first = true;
    let mut present = |t: &mut Vt, s: &mut [u32]| {
        let Some(d) = t.take_damage() else { return };
        if first {
            first = false;
            render(t, s, 0, 0, sw, sh);
            return;
        }
        let px = d.scrolled * GLYPH_H;
        if px > 0 {
            for y in 0..sh - px {
                for x in 0..sw {
                    s[(y * sw + x) as usize] = s[((y + px) * sw + x) as usize];
                }
            }
        }
        let (x, y, w, h) = d.rect.to_pixels();
        render(t, s, x, y, w, h);
    };
    present(&mut t, &mut surface);
    let mut writes: std::vec::Vec<std::vec::Vec<u8>> = std::vec![
        b"\x1b[41m \x1b[44m \x1b[0m\r\n".to_vec(),
        script::GREETING.to_vec(),
    ];
    script::write_scroller(|bytes| writes.push(bytes.to_vec()));
    for (n, w) in writes.iter().enumerate() {
        t.feed(w);
        if n % K != K - 1 && n != writes.len() - 1 {
            continue;
        }
        present(&mut t, &mut surface);
        for y in 0..rows * GLYPH_H {
            for x in 0..sw {
                assert_eq!(
                    surface[(y * sw + x) as usize],
                    t.pixel(x, y),
                    "after write {n} ({:?}) the moved picture is wrong at ({x},{y})",
                    core::str::from_utf8(w).unwrap_or("?"),
                );
            }
        }
    }
}

/// **A painter that keeps [`Damage`]'s promise ends with the engine's picture**: move the picture up
/// `scrolled` rows, render `rect`, and every pixel is [`Vt::pixel`]'s. Checked after each feed of
/// writes that scroll several rows between takes, which is what one `OPERATION_WRITE` of several
/// lines is. Two shapes are the ones that can break it: a row wider than the rows that replace it
/// in one move (its tail must not stay on the screen), and a change made above the bottom row and
/// then scrolled in the same take (its box must go where the change went).
#[test]
fn a_painter_that_moves_and_renders_what_damage_says_ends_with_the_engines_picture() {
    for cursor in [true, false] {
        let mut t = vt(20, 6);
        t.set_cursor_visible(cursor);
        let (w, h) = (t.width() as usize, t.height() as usize);
        let mut screen: Vec<u32> = (0..w * h)
            .map(|i| t.pixel((i % w) as u32, (i / w) as u32))
            .collect();
        for feed in [
            &b"\r\n\r\n\r\n\r\n\r\n\x1b[1;4mwider than the rest\x1b[0m\r\n"[..],
            b"one\r\ntwo\r\nthree\r\nfour\r\n",
            b"\x1b[2;3Hmid\x1b[6;1H\r\n\r\n\r\n",
            b"x\r\ny\r\nzzzzzzzzzzzzzzzzzzz\r\n\r\n",
            b"a\r\nb\r\nc\r\nd\r\ne\r\nf\r\ng\r\n",
            // Two full rows, then two feeds with nothing written: the move exposes the bottom row,
            // whose old pixels are the second full row.
            b"AAAAAAAAAAAAAAAAAAA\r\nBBBBBBBBBBBBBBBBBBB",
            b"\r\n\r\n",
        ] {
            t.feed(feed);
            if let Some(d) = t.take_damage() {
                let px = (d.scrolled * crate::CELL_H) as usize;
                screen.copy_within(px * w.., 0);
                let (x0, y0, dw, dh) = d.rect.to_pixels();
                for y in y0..y0 + dh {
                    for x in x0..x0 + dw {
                        screen[y as usize * w + x as usize] = t.pixel(x, y);
                    }
                }
            }
            for (i, &p) in screen.iter().enumerate() {
                let (x, y) = ((i % w) as u32, (i / w) as u32);
                assert_eq!(
                    p,
                    t.pixel(x, y),
                    "cursor {cursor}, after {:?}: pixel ({x}, {y}), cell ({}, {})",
                    std::string::String::from_utf8_lossy(feed),
                    x / crate::CELL_W,
                    y / crate::CELL_H,
                );
            }
        }
    }
}

/// **A change's box moves up with the change, and a box at the top shrinks by the row that left.**
/// Two marks on the top two rows, then one scroll over a blank bottom row: what is left to render
/// is the one row the second mark moved to, not two.
#[test]
fn a_scroll_carries_the_box_of_what_changed_up_with_it() {
    let mut t = vt(4, 3);
    t.set_cursor_visible(false);
    t.feed(b"ab\r\ncd\r\n");
    t.take_damage();
    t.feed(b"\x1b[1;1HX\x1b[2;1HY\x1b[3;1H\n");
    let d = t.take_damage().expect("a scroll is a report");
    assert_eq!(d.scrolled, 1);
    assert_eq!(
        d.rect,
        CellRect {
            col: 0,
            row: 0,
            cols: 1,
            rows: 1
        },
        "the X scrolled off; only the Y, now on row 0, is left to render"
    );
}

/// **SGR's colon forms and the renditions they carry**: `4:n` picks an underline style and `21`
/// is double; `58` colors the underline in either form and `59` returns it to the foreground;
/// `38:2::r:g:b` (with the empty color-space slot) and `38:5:n` are the colon spellings of the
/// extended colors; and a parameter's sub-parameters that it does not use are skipped rather than
/// read as the next attribute (`1:5` is bold, not bold then blink).
#[test]
fn sgr_reads_the_colon_forms_italic_and_underline_styles_and_color() {
    let mut t = vt(12, 1);
    t.feed(b"\x1b[3;4:3ma");
    let a = t.cell(0, 0).attr;
    assert!(a.is_italic());
    assert_eq!(a.underline(), UNDERLINE_CURLY);
    t.feed(b"\x1b[23;21mb");
    let b = t.cell(1, 0).attr;
    assert!(!b.is_italic());
    assert_eq!(b.underline(), UNDERLINE_DOUBLE);
    t.feed(b"\x1b[4:0;58;5;9mc");
    let c = t.cell(2, 0).attr;
    assert_eq!(c.underline(), UNDERLINE_NONE);
    assert_eq!(c.ul, Some(Colour::Indexed(9)));
    t.feed(b"\x1b[58:2::1:2:3;4:5md");
    assert_eq!(t.cell(3, 0).attr.ul, Some(Colour::Rgb(1, 2, 3)));
    assert_eq!(t.cell(3, 0).attr.underline(), UNDERLINE_DASHED);
    t.feed(b"\x1b[59;38:2:4:5:6;48:5:17;4:4me");
    let e = t.cell(4, 0).attr;
    assert_eq!(e.ul, None);
    assert_eq!(
        (e.fg(), e.bg()),
        (Colour::Rgb(4, 5, 6), Colour::Indexed(17))
    );
    assert_eq!(e.underline(), UNDERLINE_DOTTED);
    t.feed(b"\x1b[0;1:5;31mf");
    let f = t.cell(5, 0).attr;
    assert_eq!(f, attr(1, DEFAULT_BG).with(BOLD, true));
}

/// **An empty first parameter is the first parameter's default**, not the second's value read
/// into the first slot: `CSI ;5H` is row one, column five. The colon work made `;` and `:` share
/// one arm, and the arm now opens the first parameter before moving to the next.
#[test]
fn an_empty_first_parameter_is_its_default() {
    let mut t = vt(8, 8);
    t.feed(b"\x1b[;5H");
    assert_eq!(t.cursor(), (4, 0));
}

/// **Bold and italic draw their faces**, and the color stays as set: every pixel of a bold, an
/// italic and a bold-italic `R` is that face's own blend, and the four faces differ, so a cell
/// drawn in the wrong face shows.
#[test]
fn bold_and_italic_draw_the_atlas_faces() {
    let mut t = vt(4, 1);
    t.set_cursor_visible(false);
    t.feed(b"R\x1b[1mR\x1b[22;3mR\x1b[1mR");
    let (fg, bg) = Attr::DEFAULT.colours();
    let faces = [
        &glyph_atlas::REGULAR,
        &glyph_atlas::BOLD,
        &glyph_atlas::OBLIQUE,
        &glyph_atlas::BOLD_OBLIQUE,
    ];
    let picture = |col: u32| -> Vec<u32> {
        (0..crate::CELL_H)
            .flat_map(|gy| (0..crate::CELL_W).map(move |gx| (gx, gy)))
            .map(|(gx, gy)| t.pixel(col * crate::CELL_W + gx, gy))
            .collect()
    };
    for (col, face) in faces.iter().enumerate() {
        let want: Vec<u32> = (0..crate::CELL_H)
            .flat_map(|gy| (0..crate::CELL_W).map(move |gx| (gx, gy)))
            .map(|(gx, gy)| face.cell_pixel('R', gx, gy, fg, bg))
            .collect();
        assert_eq!(picture(col as u32), want, "column {col}");
    }
    for a in 0..4 {
        for b in a + 1..4 {
            assert_ne!(
                picture(a),
                picture(b),
                "columns {a} and {b} draw the same face"
            );
        }
    }
}

/// **Each underline style inks its own pixels, in the underline's color**: single and double on
/// their rows, curly as a wave, dotted and dashed as runs that carry across the cell boundary,
/// and SGR 58's color where it is set. Read from a blank cell, so every inked pixel is the line's.
#[test]
fn each_underline_style_draws_its_own_line_in_its_own_colour() {
    let ink_rows = |seq: &[u8]| -> Vec<(u32, u32)> {
        let mut t = vt(2, 1);
        t.set_cursor_visible(false);
        t.feed(seq);
        let (_, bg) = Attr::DEFAULT.colours();
        (0..crate::CELL_H)
            .flat_map(|y| (0..2 * crate::CELL_W).map(move |x| (x, y)))
            .filter(|&(x, y)| t.pixel(x, y) != bg)
            .collect()
    };
    let u = UNDERLINE_ROW;
    let single = ink_rows(b"\x1b[4m  ");
    assert!(single.iter().all(|&(_, y)| y == u) && single.len() == 28);
    let double = ink_rows(b"\x1b[21m  ");
    assert_eq!(double.len(), 56);
    assert!(double.iter().all(|&(_, y)| y == u || y == u + 2));
    // Curly: one pixel a column, a wave eight pixels long, its depth rising and falling by rows.
    let mut curly = ink_rows(b"\x1b[4:3m  ");
    curly.sort();
    let depths: Vec<u32> = curly.iter().map(|&(_, y)| y - u).collect();
    let wave = [0, 0, 1, 1, 2, 1, 1, 0];
    assert_eq!(depths, (0..28).map(|x| wave[x % 8]).collect::<Vec<u32>>());
    // Dotted: two on, two off, counted from the screen's left edge.
    let dotted: Vec<u32> = ink_rows(b"\x1b[4:4m  ").iter().map(|&(x, _)| x).collect();
    assert_eq!(
        dotted,
        (0..28).filter(|x| x / 2 % 2 == 0).collect::<Vec<u32>>()
    );
    // Dashed: five on, three off.
    let dashed: Vec<u32> = ink_rows(b"\x1b[4:5m  ").iter().map(|&(x, _)| x).collect();
    assert_eq!(dashed.len(), 19);
    assert_eq!(dashed, (0..28).filter(|x| x % 8 < 5).collect::<Vec<u32>>());
    // The color.
    let mut t = vt(1, 1);
    t.set_cursor_visible(false);
    t.feed(b"\x1b[4;58;2;250;0;0m ");
    assert_eq!(t.pixel(0, u), 0x00fa_0000);
    t.feed(b"\r\x1b[59m ");
    assert_eq!(t.pixel(0, u), Attr::DEFAULT.colours().0);
    // Concealed text hides its underline too, its own color included.
    t.feed(b"\r\x1b[8;58;2;250;0;0m ");
    assert_eq!(t.pixel(0, u), Attr::DEFAULT.colours().1);
}
