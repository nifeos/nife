//! The screen console's host tests, moved out of `lib.rs` unchanged on 2026-10-12 (UTC) by lane
//! maintainer/near-ceiling-splits, so the crate root stays under §266 (a Rust source file stays
//! under 2,000 lines).

use std::boxed::Box;
use std::vec;
use std::vec::Vec;

use machine_discovery::framebuffer::{Framebuffer, PixelOrder};

use super::{Cells, PixelSink, ScreenConsole};

/// [`ScreenConsole::write`] with a fresh grid, for the tests that write once into a cleared
/// screen. A grid that is blank while the screen is not would draw a different picture on a
/// scroll, so every test that scrolls clears first through [`clear`], which keeps its own grid.
fn write(console: &mut ScreenConsole, pixels: &mut [u8], text: &str) {
    GRID.with_borrow_mut(|cells| console.write(cells, &mut PixelSink::new(pixels), text));
}

/// [`ScreenConsole::clear`], blanking the grid [`write`] uses.
fn clear(console: &mut ScreenConsole, pixels: &mut [u8]) {
    GRID.with_borrow_mut(|cells| console.clear(cells, &mut PixelSink::new(pixels)));
}

std::thread_local! {
    /// One grid per test thread, so the helpers above read like the old one-argument calls.
    static GRID: core::cell::RefCell<Box<Cells>> = core::cell::RefCell::new(Box::new(Cells::new()));
}

/// A screen with a **padded stride**, which is the geometry that catches the classic
/// framebuffer bug: a painter that multiplies by the width instead of the stride paints a
/// picture that shears progressively down the screen, and it looks perfectly correct on the
/// first row.
fn screen(cols: u32, rows: u32, pad: u32, order: PixelOrder) -> (Framebuffer, [u8; 4096 * 16]) {
    let width = cols * bitmap_font::GLYPH_W;
    let height = rows * bitmap_font::GLYPH_H;
    let found = Framebuffer {
        base: 0,
        width,
        height,
        stride: width * 4 + pad,
        order,
    };
    assert!(found.span().expect("a valid geometry") <= 4096 * 16);
    (found, [0u8; 4096 * 16])
}

/// Read one pixel back as a `u32`, the way a screendump would.
fn pixel(found: &Framebuffer, pixels: &[u8], x: u32, y: u32) -> u32 {
    let at = (y * found.stride + x * 4) as usize;
    u32::from_le_bytes(pixels[at..at + 4].try_into().expect("four bytes"))
}

/// The whole crate is a pure function from text to pixels, so the expected picture is
/// computable and can be *printed*. This is the same property `bitmap_font` claims for itself
/// and it is the reason a console nobody can see is nonetheless provable.
#[test]
fn a_letter_is_drawn_the_right_way_up_and_the_right_way_round() {
    let (found, mut pixels) = screen(4, 2, 12, PixelOrder::Bgrx);
    let mut console = ScreenConsole::new(found).expect("four by two cells");
    write(&mut console, &mut pixels, "F");

    let art: [[char; 7]; 8] = core::array::from_fn(|y| {
        core::array::from_fn(|x| {
            if pixel(&found, &pixels, x as u32, y as u32) == ScreenConsole::FOREGROUND {
                '#'
            } else {
                '.'
            }
        })
    });
    assert_eq!(
        art,
        [
            ['.', '#', '#', '#', '#', '#', '.'], // the top bar
            ['.', '#', '.', '.', '.', '.', '.'], // the stem is on the LEFT, which is what a
            ['.', '#', '.', '.', '.', '.', '.'], // mirrored font gets wrong
            ['.', '#', '#', '#', '#', '.', '.'], // one column short: that is what makes it an F
            ['.', '#', '.', '.', '.', '.', '.'],
            ['.', '#', '.', '.', '.', '.', '.'],
            ['.', '#', '.', '.', '.', '.', '.'],
            ['.', '.', '.', '.', '.', '.', '.'], // row 7 is the descender row, and F has none
        ]
    );
}

/// A 1920-wide screen (xenon's) draws at two: 137 columns, and every font pixel a 2x2 block.
/// Narrow screens, which every other test here uses, stay at one.
#[test]
fn a_wide_screen_draws_each_font_pixel_as_a_block() {
    assert_eq!(ScreenConsole::scale_for(1280), 1);
    assert_eq!(ScreenConsole::scale_for(1679), 1);
    assert_eq!(ScreenConsole::scale_for(1680), 2);
    assert_eq!(ScreenConsole::scale_for(1920), 2);
    assert_eq!(ScreenConsole::scale_for(2520), 3);
    let found = Framebuffer {
        base: 0,
        width: 1920,
        height: 32,
        stride: 1920 * 4,
        order: PixelOrder::Bgrx,
    };
    let mut pixels = [0u8; 1920 * 4 * 32];
    assert_eq!(found.span(), Some(pixels.len()));
    let mut console = ScreenConsole::new(found).expect("a wide screen");
    assert_eq!(console.size(), (137, 2));
    write(&mut console, &mut pixels, &"F".repeat(138));
    for (col, row) in [(0, 0), (0, 1)] {
        for y in 0..16 {
            for x in 0..14 {
                let want = bitmap_font::cell_pixel(
                    'F',
                    x / 2,
                    y / 2,
                    ScreenConsole::FOREGROUND,
                    ScreenConsole::BACKGROUND,
                );
                assert_eq!(
                    pixel(&found, &pixels, col * 14 + x, row * 16 + y),
                    want,
                    "cell ({col},{row}) pixel ({x},{y}); the 138th F must wrap to row 1"
                );
            }
        }
    }
}

/// The stride is padding, and a painter that ignores it writes the second row seven pixels to
/// the left of where it belongs. The pad bytes must be untouched.
#[test]
fn the_padding_between_rows_is_never_written() {
    const PAD: u32 = 12;
    let (found, mut pixels) = screen(4, 2, PAD, PixelOrder::Bgrx);
    let mut console = ScreenConsole::new(found).expect("four by two cells");
    clear(&mut console, &mut pixels);
    // Poison the padding, then paint over the whole grid and check it survived.
    let visible = (found.width * 4) as usize;
    for y in 0..found.height as usize {
        let at = y * found.stride as usize + visible;
        pixels[at..at + PAD as usize].fill(0xa5);
    }
    write(&mut console, &mut pixels, "MMMM\nMMMM");
    for y in 0..found.height as usize {
        let at = y * found.stride as usize + visible;
        assert_eq!(
            &pixels[at..at + PAD as usize],
            &[0xa5; PAD as usize],
            "row {y}'s padding was painted over"
        );
    }
}

/// Running past the last column wraps rather than painting outside the screen, and running past
/// the last row scrolls. Both are checked by where a known glyph ends up.
#[test]
fn text_wraps_at_the_right_edge_and_scrolls_at_the_bottom() {
    let (found, mut pixels) = screen(2, 2, 0, PixelOrder::Bgrx);
    let mut console = ScreenConsole::new(found).expect("two by two cells");
    assert_eq!(console.size(), (2, 2));
    clear(&mut console, &mut pixels);

    // Five characters into a 2x2 grid: "ab" wraps to "cd", then "e" scrolls and lands at (0,1).
    write(&mut console, &mut pixels, "abcde");
    let cell = |pixels: &[u8], col: u32, row: u32, ch: char| {
        (0..bitmap_font::GLYPH_H).all(|y| {
            (0..bitmap_font::GLYPH_W).all(|x| {
                let want = bitmap_font::cell_pixel(
                    ch,
                    x,
                    y,
                    ScreenConsole::FOREGROUND,
                    ScreenConsole::BACKGROUND,
                );
                pixel(
                    &found,
                    pixels,
                    col * bitmap_font::GLYPH_W + x,
                    row * bitmap_font::GLYPH_H + y,
                ) == want
            })
        })
    };
    assert!(cell(&pixels, 0, 0, 'c'), "the second line scrolled up");
    assert!(cell(&pixels, 1, 0, 'd'));
    assert!(cell(&pixels, 0, 1, 'e'), "the fifth character opened a row");
    assert!(cell(&pixels, 1, 1, ' '), "the rest of the new row is blank");
}

/// `ScreenConsole::span` had no caller at all: every test either uses `Framebuffer::span`
/// directly (`screen`'s own assertion) or `Aperture::span` (a different method entirely).
#[test]
fn console_span_is_the_underlying_screens_span() {
    let (found, _pixels) = screen(4, 2, 12, PixelOrder::Bgrx);
    let console = ScreenConsole::new(found).expect("four by two cells");
    assert_eq!(console.span(), found.span().unwrap());
}

/// Every other test's `clear()` call is immediately followed by writing text that covers the
/// whole grid, so nothing ever looked at the screen right after a clear and before a write,
/// which is the one moment a stubbed-out `clear` and the real one look different.
#[test]
fn clear_paints_the_whole_surface_in_the_background_colour() {
    let (found, mut pixels) = screen(2, 2, 0, PixelOrder::Bgrx);
    pixels.fill(0xa5); // poison: a no-op clear would leave this untouched
    let mut console = ScreenConsole::new(found).expect("two by two cells");
    clear(&mut console, &mut pixels);
    for y in 0..found.height {
        for x in 0..found.width {
            assert_eq!(
                pixel(&found, &pixels, x, y),
                ScreenConsole::BACKGROUND,
                "({x},{y})"
            );
        }
    }
}

/// A carriage return resets the column without drawing anything, which is the one control
/// character `text_wraps_at_the_right_edge_and_scrolls_at_the_bottom` never sends.
#[test]
fn a_carriage_return_resets_the_column_without_drawing_a_glyph() {
    let (found, mut pixels) = screen(3, 1, 0, PixelOrder::Bgrx);
    let mut console = ScreenConsole::new(found).expect("three by one cells");
    clear(&mut console, &mut pixels);
    write(&mut console, &mut pixels, "a\rb");
    let cell = |pixels: &[u8], col: u32, ch: char| {
        (0..bitmap_font::GLYPH_H).all(|y| {
            (0..bitmap_font::GLYPH_W).all(|x| {
                let want = bitmap_font::cell_pixel(
                    ch,
                    x,
                    y,
                    ScreenConsole::FOREGROUND,
                    ScreenConsole::BACKGROUND,
                );
                pixel(&found, pixels, col * bitmap_font::GLYPH_W + x, y) == want
            })
        })
    };
    assert!(cell(&pixels, 0, 'b'), "'b' overwrote 'a' at column 0");
    assert!(
        cell(&pixels, 1, ' '),
        "the carriage return must not have advanced the column or drawn anything"
    );
}

/// A one-row console has nowhere to scroll to: `scroll`'s `band >= live` guard exists for
/// exactly this case, and no existing test has a console with only one row (every `screen()`
/// fixture that reaches `scroll` uses at least two).
#[test]
fn scrolling_a_one_row_console_leaves_it_alone_rather_than_blanking_it() {
    let (found, mut pixels) = screen(3, 1, 0, PixelOrder::Bgrx);
    let mut console = ScreenConsole::new(found).expect("three by one cells");
    clear(&mut console, &mut pixels);
    write(&mut console, &mut pixels, "abcd"); // the fourth character wraps and tries to scroll
    let cell = |pixels: &[u8], col: u32, ch: char| {
        (0..bitmap_font::GLYPH_H).all(|y| {
            (0..bitmap_font::GLYPH_W).all(|x| {
                let want = bitmap_font::cell_pixel(
                    ch,
                    x,
                    y,
                    ScreenConsole::FOREGROUND,
                    ScreenConsole::BACKGROUND,
                );
                pixel(&found, pixels, col * bitmap_font::GLYPH_W + x, y) == want
            })
        })
    };
    assert!(
        cell(&pixels, 0, 'd'),
        "'d' wrapped back to column 0, overwriting 'a'"
    );
    assert!(
        cell(&pixels, 1, 'b'),
        "a one-row console has nowhere to scroll; 'b' must survive"
    );
    assert!(cell(&pixels, 2, 'c'), "same for 'c'");
}

/// **The old painter, kept as the oracle**: the console as it was before 2026-10-04, which
/// scrolled by copying the screen up one row of cells with `copy_within`. That copy read the
/// framebuffer back, which is what made xenon's screen sweep visibly on every new line, and it
/// is the picture the grid-driven scroll must reproduce exactly. Written out here in full
/// rather than sharing `draw` with the code under test, so a bug in the shared half could not
/// make the two agree.
fn copy_up_reference(found: Framebuffer, pixels: &mut [u8], text: &str) {
    let k = ScreenConsole::scale_for(found.width) as usize;
    let (gw, gh) = (bitmap_font::GLYPH_W as usize, bitmap_font::GLYPH_H as usize);
    let stride = found.stride as usize;
    let cols = found.width as usize / (gw * k);
    let rows = found.height as usize / (gh * k);
    let (mut col, mut row) = (0usize, 0usize);
    let ink = |c: u32| found.order.store(c).to_le_bytes();
    for chunk in pixels.as_chunks_mut::<4>().0 {
        chunk.copy_from_slice(&ink(ScreenConsole::BACKGROUND));
    }
    let newline = |row: &mut usize, pixels: &mut [u8]| {
        if *row + 1 < rows {
            *row += 1;
            return;
        }
        let band = stride * gh * k;
        let live = stride * rows * gh * k;
        if band >= live {
            return;
        }
        pixels.copy_within(band..live, 0);
        for chunk in pixels[live - band..live].as_chunks_mut::<4>().0 {
            chunk.copy_from_slice(&ink(ScreenConsole::BACKGROUND));
        }
    };
    for byte in text.bytes() {
        match byte {
            b'\n' => {
                col = 0;
                newline(&mut row, pixels);
                continue;
            }
            b'\r' => {
                col = 0;
                continue;
            }
            _ => {}
        }
        if col >= cols {
            col = 0;
            newline(&mut row, pixels);
        }
        let glyph = if (0x20..0x7f).contains(&byte) {
            byte as char
        } else {
            ' '
        };
        for y in 0..gh * k {
            for x in 0..gw * k {
                let at = (row * gh * k + y) * stride + (col * gw * k + x) * 4;
                let c = bitmap_font::cell_pixel(
                    glyph,
                    (x / k) as u32,
                    (y / k) as u32,
                    ScreenConsole::FOREGROUND,
                    ScreenConsole::BACKGROUND,
                );
                pixels[at..at + 4].copy_from_slice(&ink(c));
            }
        }
        col += 1;
    }
}

/// Text that exercises every path a scroll can take: lines shorter and longer than a row (so
/// wrapping scrolls as well as newlines), blank lines, carriage returns over existing text, and
/// bytes with no glyph. A fixed linear congruential generator, so a failure is reproducible.
fn awkward_text(lines: usize, cols: usize) -> std::string::String {
    let mut seed = 0x2026_1004u32;
    let mut next = || {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        (seed >> 16) as usize
    };
    let mut text = std::string::String::new();
    for _ in 0..lines {
        for _ in 0..next() % (cols * 2 + 1) {
            text.push(match next() % 40 {
                0 => '\r',
                1 => '\u{7}',
                2..=9 => ' ',
                n => (b'!' + (n as u8 * 3) % 94) as char,
            });
        }
        text.push('\n');
    }
    text
}

/// **The whole correctness claim of the grid**: for every geometry that matters (a padded
/// stride, a screen that is not a whole number of cells, both byte orders, and a screen wide
/// enough to draw at scale two), text that scrolls many times leaves exactly the bytes the old
/// copy-up left, compared over the entire buffer including the padding and the unpainted
/// edges, after every one of several writes.
#[test]
fn the_grid_scroll_paints_exactly_what_the_copy_up_painted() {
    let geometries = [
        (5 * 7 + 3, 3 * 8 + 5, 12, PixelOrder::Bgrx),
        (4 * 7, 2 * 8, 0, PixelOrder::Rgbx),
        (1680 + 5, 16 * 4 + 3, 20, PixelOrder::Bgrx),
    ];
    for (width, height, pad, order) in geometries {
        let found = Framebuffer {
            base: 0,
            width,
            height,
            stride: width * 4 + pad,
            order,
        };
        let mut console = ScreenConsole::new(found).expect("a whole cell");
        let (cols, rows) = console.size();
        let text = awkward_text(rows as usize * 6, cols as usize);
        let mut cells = Box::new(Cells::new());
        let mut grid = vec![0xa5u8; found.span().expect("valid")];
        console.clear(&mut cells, &mut PixelSink::new(&mut grid));
        // The oracle paints from the start each time, so a prefix comparison checks the
        // picture after every chunk rather than only at the end.
        let mut written = 0;
        for chunk in text.as_bytes().chunks(97) {
            let chunk = core::str::from_utf8(chunk).expect("ascii");
            console.write(&mut cells, &mut PixelSink::new(&mut grid), chunk);
            written += chunk.len();
            let mut oracle = vec![0x5au8; grid.len()];
            copy_up_reference(found, &mut oracle, &text[..written]);
            assert!(
                grid == oracle,
                "{width}x{height} pad {pad} {order:?}: pictures differ after {written} bytes"
            );
        }
    }
}

/// **What a scroll costs on xenon, counted rather than estimated**: a real boot transcript
/// (the 2026-09-17 tour, `bench/xenon-2026-09-17/`) through a 1920x1080 console at scale two,
/// with every store counted. The old copy-up read the live rows minus one band and wrote all
/// of them on every scroll; this reads nothing, because it cannot, and writes the cells whose
/// glyph changed. Run with `--nocapture` to see the numbers the assertion bounds.
#[test]
fn a_scroll_writes_only_the_cells_that_changed() {
    let found = Framebuffer {
        base: 0,
        width: 1920,
        height: 1080,
        stride: 1920 * 4,
        order: PixelOrder::Bgrx,
    };
    let tour = include_str!("../../../bench/xenon-2026-09-17/tour-display-225100.log");
    let mut console = ScreenConsole::new(found).expect("xenon's screen");
    let (cols, rows) = console.size();
    assert_eq!((cols, rows), (137, 67));
    let mut cells = Box::new(Cells::new());
    let mut pixels = vec![0u8; found.span().expect("valid")];
    let mut sink = PixelSink::new(&mut pixels);
    console.clear(&mut cells, &mut sink);

    // Fill the screen first, so every line after this is a scroll.
    let lines: Vec<&str> = tour.lines().collect();
    let (fill, rest) = lines.split_at(rows as usize);
    for line in fill {
        console.write(&mut cells, &mut sink, line);
        console.write(&mut cells, &mut sink, "\n");
    }
    let mut scrolls = 0u64;
    let mut stored = 0u64;
    for line in rest {
        console.write(&mut cells, &mut sink, line);
        let before = sink.stores;
        console.write(&mut cells, &mut sink, "\n");
        stored += (sink.stores - before) as u64 * 4;
        scrolls += 1;
    }
    let band = (found.stride * bitmap_font::GLYPH_H * 2) as u64;
    let live = band * rows as u64;
    let (old_read, old_written) = ((live - band) * scrolls, live * scrolls);
    std::println!(
        "{scrolls} scrolls on {cols}x{rows}: copy-up read {old_read} and wrote {old_written} \
         bytes; the grid read 0 and wrote {stored} ({:.1}% of the old writes, {} per scroll)",
        stored as f64 * 100.0 / old_written as f64,
        stored / scrolls,
    );
    assert!(
        scrolls > 50,
        "the transcript must scroll enough to mean something"
    );
    assert!(
        stored * 2 < old_written,
        "a transcript of short lines must cost less than half the copy-up's writes"
    );
}

/// The worst case, so the bound above is not mistaken for a promise: a screen where every cell
/// differs from the one below it rewrites every cell on a scroll, which is the same number of
/// bytes the copy-up wrote. It still reads none.
#[test]
fn the_worst_scroll_writes_what_the_copy_up_wrote_and_no_more() {
    let (found, mut pixels) = screen(3, 2, 0, PixelOrder::Bgrx);
    let mut console = ScreenConsole::new(found).expect("three by two cells");
    let mut cells = Box::new(Cells::new());
    let mut sink = PixelSink::new(&mut pixels);
    console.clear(&mut cells, &mut sink);
    console.write(&mut cells, &mut sink, "abc\ndef");
    let before = sink.stores;
    console.write(&mut cells, &mut sink, "\n");
    let cell = (bitmap_font::GLYPH_W * bitmap_font::GLYPH_H) as usize;
    assert_eq!(
        sink.stores - before,
        6 * cell,
        "every one of six cells changed"
    );
}

/// The blank glyph must be all paper, or a grid of spaces and a cleared screen would be two
/// different pictures and skipping a space-onto-space cell would leave ink behind.
#[test]
fn a_space_is_drawn_entirely_in_paper() {
    for y in 0..bitmap_font::GLYPH_H {
        for x in 0..bitmap_font::GLYPH_W {
            assert_eq!(
                bitmap_font::cell_pixel(
                    ' ',
                    x,
                    y,
                    ScreenConsole::FOREGROUND,
                    ScreenConsole::BACKGROUND
                ),
                ScreenConsole::BACKGROUND,
                "({x},{y})"
            );
        }
    }
}

/// A screen taller than [`Cells::CAPACITY`] has room for is drawn on the rows that fit, not
/// refused and not written past the grid. 1600 pixels at scale one is 228 columns, and
/// 1600 rows of pixels would be 200 rows of cells.
#[test]
fn a_screen_taller_than_the_grid_uses_the_rows_that_fit() {
    let found = Framebuffer {
        base: 0,
        width: 1600,
        height: 1600,
        stride: 1600 * 4,
        order: PixelOrder::Bgrx,
    };
    let console = ScreenConsole::new(found).expect("a big screen");
    let (cols, rows) = console.size();
    assert_eq!(cols, 228);
    assert_eq!(rows as usize, Cells::CAPACITY / 228);
    assert!(rows < 200);
}

/// The column bound [`Cells::CAPACITY`] is built on: no width gets `2 * MIN_COLUMNS` columns
/// or more, at any scale. Checked over every width up to five scales' worth.
#[test]
fn no_screen_is_wider_than_the_grid_allows() {
    for width in 1..7 * super::MIN_COLUMNS * 6 {
        let cols = width / (bitmap_font::GLYPH_W * ScreenConsole::scale_for(width));
        assert!(
            cols < 2 * super::MIN_COLUMNS,
            "{width} wide gets {cols} columns"
        );
    }
}

/// The byte order is the one thing that cannot be seen by looking at the screen in a test, and
/// is the one thing that makes the picture wrong on half the machines that could run this.
#[test]
fn the_two_pixel_orders_store_different_bytes_for_the_same_ink() {
    let (bgrx, mut a) = screen(1, 1, 0, PixelOrder::Bgrx);
    let (rgbx, mut b) = screen(1, 1, 0, PixelOrder::Rgbx);
    write(
        &mut ScreenConsole::new(bgrx).expect("one cell"),
        &mut a,
        "#",
    );
    write(
        &mut ScreenConsole::new(rgbx).expect("one cell"),
        &mut b,
        "#",
    );
    // FOREGROUND is grey, so its red and blue bytes are equal and the two orders agree. That is
    // exactly why the check below uses a colour whose channels differ.
    assert_eq!(
        PixelOrder::Bgrx.store(0x0011_2233),
        0x0011_2233,
        "bgrx stores a colour as written"
    );
    assert_eq!(
        PixelOrder::Rgbx.store(0x0011_2233),
        0x0033_2211,
        "rgbx exchanges red and blue"
    );
    assert_eq!(a, b, "grey ink is the same bytes in both orders");
}

/// A screen too small for one character cell has no console, rather than a console that writes
/// outside it.
#[test]
fn a_screen_smaller_than_one_cell_has_no_console() {
    for (width, height) in [(6, 8), (7, 7), (0, 0)] {
        let found = Framebuffer {
            base: 0,
            width,
            height,
            stride: width * 4,
            order: PixelOrder::Bgrx,
        };
        assert!(ScreenConsole::new(found).is_none(), "{width}x{height}");
    }
}

/// **The aperture copies through the stride, not the width**, and leaves the padding alone.
/// A copy that multiplied by the width would put row 1 twelve bytes early, which is the shear
/// `the_padding_between_rows_is_never_written` guards against in the text painter.
#[test]
fn an_aperture_copies_through_the_stride_and_never_touches_the_padding() {
    use super::Aperture;
    const PAD: u32 = 12;
    let (found, mut pixels) = screen(2, 1, PAD, PixelOrder::Bgrx);
    pixels.fill(0xa5);
    let aperture = Aperture::new(&found, bitmap_font::GLYPH_W, 1000, 1000).expect("a real screen");
    assert_eq!(
        aperture.size(),
        (found.width, found.height),
        "clipped to the screen"
    );
    let (w, h) = aperture.size();
    // Every surface pixel a function of its coordinate, so a misplaced pixel is a wrong value.
    let surface = |x: u32, y: u32| (y << 8) | x;
    assert!(aperture.copy(0, 0, w, h, surface, |at, word| {
        pixels[at..at + 4].copy_from_slice(&word.to_le_bytes());
    }));
    for y in 0..h {
        for x in 0..w {
            assert_eq!(pixel(&found, &pixels, x, y), surface(x, y), "({x},{y})");
        }
        let pad = (y * found.stride + w * 4) as usize;
        assert_eq!(
            &pixels[pad..pad + PAD as usize],
            &[0xa5; PAD as usize],
            "row {y}"
        );
    }
    assert_eq!(aperture.span(), ((h - 1) * found.stride + w * 4) as usize);
}

/// Clipping takes the smaller of the two sizes on each axis independently, and a rectangle
/// that leaves the clipped part, or is empty, or overflows `u32`, is refused with nothing
/// written.
#[test]
fn an_aperture_is_the_smaller_of_screen_and_surface_and_refuses_what_leaves_it() {
    use super::Aperture;
    let found = Framebuffer {
        base: 0,
        width: 1280,
        height: 800,
        stride: 1280 * 4,
        order: PixelOrder::Bgrx,
    };
    let aperture = Aperture::new(&found, bitmap_font::GLYPH_W, 924, 344).expect("a real screen");
    assert_eq!(
        aperture.size(),
        (924, 344),
        "the surface is smaller: it wins"
    );
    let narrow = Framebuffer {
        width: 800,
        stride: 800 * 4,
        ..found
    };
    assert_eq!(
        Aperture::new(&narrow, bitmap_font::GLYPH_W, 924, 344)
            .expect("a real screen")
            .size(),
        (800, 344),
        "a narrow screen clips the surface's width and not its height"
    );
    for (x, y, w, h) in [
        (0, 0, 925, 1),
        (0, 0, 1, 345),
        (923, 0, 2, 1),
        (0, 0, 0, 1),
        (0, 0, 1, 0),
        (u32::MAX, 0, 2, 1),
    ] {
        assert!(
            !aperture.copy(
                x,
                y,
                w,
                h,
                |_, _| 0,
                |_, _| panic!("wrote for ({x},{y},{w},{h})")
            ),
            "({x},{y},{w},{h}) should have been refused"
        );
    }
    assert!(
        Aperture::new(&found, bitmap_font::GLYPH_W, 0, 344).is_none(),
        "an empty surface"
    );
}

/// The words that carry an aperture across a spawn come back as the same aperture, both byte
/// orders survive, and words describing an impossible geometry are refused rather than
/// believed.
#[test]
fn an_aperture_survives_the_trip_through_two_words() {
    use super::Aperture;
    for order in [PixelOrder::Bgrx, PixelOrder::Rgbx] {
        let found = Framebuffer {
            base: 0,
            width: 1920,
            height: 1080,
            stride: 7680,
            order,
        };
        let aperture =
            Aperture::new(&found, bitmap_font::GLYPH_W, 924, 344).expect("a real screen");
        let (size, layout) = aperture.to_words();
        assert_eq!(Aperture::from_words(size, layout), Some(aperture));
    }
    // A stride narrower than the row, and an order this tree has no name for. The scale is set,
    // so the stride is what refuses: at scale zero the first word would be refused for that.
    assert_eq!(Aperture::from_words(10 | 10 << 32, 39), None);
    assert_eq!(Aperture::from_words(10 | 10 << 32, 39 | 1 << 40), None);
    assert!(Aperture::from_words(10 | 10 << 32, 40 | 1 << 40).is_some());
    assert_eq!(Aperture::from_words(10 | 10 << 32, 79 | 2 << 40), None);
    assert!(Aperture::from_words(10 | 10 << 32, 80 | 2 << 40).is_some());
    assert_eq!(Aperture::from_words(10 | 10 << 32, 40 | 2 << 32), None);
    assert_eq!(Aperture::from_words(0, 40), None);
}

/// `copy_wide` lands the same bytes as `copy` for every base misalignment, on a stride with
/// padding that is not itself 8-aligned. This is the whole correctness claim of the wide path
/// (a pair is two word stores in one, on a little-endian machine), checked byte for byte
/// including the padding, which neither path may touch.
#[test]
fn the_wide_copy_lands_the_same_bytes_for_every_rectangle_and_base_alignment() {
    use super::Aperture;
    // 10 pixels wide (40 bytes), rows of 44: padding of 4, and a stride that is 4 off 8, so
    // some rows' qwords start at head words and stop at tails.
    let found = Framebuffer {
        base: 0,
        width: 10,
        height: 3,
        stride: 44,
        order: PixelOrder::Bgrx,
    };
    let aperture = Aperture::new(&found, bitmap_font::GLYPH_W, 10, 3).expect("a real screen");
    let (w, h) = aperture.size();
    // The surface refuses a read outside it, as `MappedWindow` does in the driver: a column
    // counted from the wrong origin is a panic here, not a quietly wrong pixel.
    let surface = |x: u32, y: u32| {
        assert!(x < w && y < h, "read ({x},{y}) outside the {w}x{h} surface");
        (y << 12) | (x << 8) | 0x0f
    };
    const SPAN: usize = 44 * 2 + 40;
    // **Every rectangle, not only the whole one.** A run that starts at `x > 0` is what a
    // cell's damage flushes; until 2026-10-02 every case here started at column 0, which is
    // the one origin where counting `x` twice (the run's offset already holds it) is
    // harmless. The driver read past the surface on the x86_64 swish leg instead.
    for (x, y) in (0..w).flat_map(|x| (0..h).map(move |y| (x, y))) {
        for (rw, rh) in (1..=w - x).flat_map(|rw| (1..=h - y).map(move |rh| (rw, rh))) {
            let mut narrow = [0xa5u8; SPAN];
            assert!(aperture.copy(x, y, rw, rh, surface, |at, word| {
                narrow[at..at + 4].copy_from_slice(&word.to_le_bytes());
            }));
            for misalign in 0..8u32 {
                // RefCell because the two writer closures must share one buffer; a plain
                // `&mut` split across them is what the borrow checker rightly refuses.
                let wide = core::cell::RefCell::new([0xa5u8; SPAN]);
                assert!(aperture.copy_wide(
                    x,
                    y,
                    rw,
                    rh,
                    misalign,
                    surface,
                    |at, pair| {
                        wide.borrow_mut()[at..at + 8].copy_from_slice(&pair.to_le_bytes());
                    },
                    |at, word| {
                        wide.borrow_mut()[at..at + 4].copy_from_slice(&word.to_le_bytes());
                    }
                ));
                assert_eq!(
                    narrow,
                    wide.into_inner(),
                    "({x},{y}) {rw}x{rh}, misalign {misalign}"
                );
            }
        }
    }
}

/// **Every qword store lands on an 8-aligned address of the aperture**, for every base
/// misalignment, which is the reason the wide path exists at all: a pair store to an address
/// the hardware cannot do in one transaction is a fault on some targets and a split write on
/// the rest. The byte-for-byte test above cannot see it, because a misplaced pair of pixels
/// lands the same bytes. A stride of 44 gives rows that start 4 off 8, and `misalign` of 2 or
/// 6 makes the base 2 off a word, so the row has no aligned pair and must take word stores only.
#[test]
fn every_qword_store_is_eight_aligned_in_the_apertures_own_address_space() {
    use super::Aperture;
    let found = Framebuffer {
        base: 0,
        width: 10,
        height: 3,
        stride: 44,
        order: PixelOrder::Bgrx,
    };
    let aperture = Aperture::new(&found, bitmap_font::GLYPH_W, 10, 3).expect("a real screen");
    let (w, h) = aperture.size();
    for (x, y) in (0..w).flat_map(|x| (0..h).map(move |y| (x, y))) {
        for (rw, rh) in (1..=w - x).flat_map(|rw| (1..=h - y).map(move |rh| (rw, rh))) {
            for misalign in 0..8u32 {
                let mut qwords = std::vec::Vec::new();
                assert!(aperture.copy_wide(
                    x,
                    y,
                    rw,
                    rh,
                    misalign,
                    |_, _| 0,
                    |at, _| qwords.push(at),
                    |_, _| {},
                ));
                for at in qwords {
                    assert_eq!(
                        (misalign as usize + at) % 8,
                        0,
                        "({x},{y}) {rw}x{rh}, misalign {misalign}: qword at {at}"
                    );
                }
            }
        }
    }
}

/// **What the wide path permits, as a store count.** The scanout's own geometry (1120x624,
/// stride 4480, an even width on an 8-aligned stride) is the best case and the one the swish
/// leg pays: one qword store per pixel pair, no head or tail words, 349,440 stores for a
/// full-surface flush against 698,880 one-word stores. A run that starts misaligned earns at
/// most one head word and one tail word, never a narrower pair.
#[test]
fn the_wide_copy_uses_one_qword_per_pair_and_never_more_than_two_edge_words() {
    use graphics_contract as gfx;

    use super::Aperture;
    let found = Framebuffer {
        base: 0,
        width: gfx::WIDTH,
        height: gfx::HEIGHT,
        stride: gfx::STRIDE,
        order: PixelOrder::Bgrx,
    };
    let aperture =
        Aperture::new(&found, bitmap_font::GLYPH_W, gfx::WIDTH, gfx::HEIGHT).expect("the scanout");
    let (w, h) = aperture.size();
    let mut qwords = 0u32;
    let mut words = 0u32;
    assert!(aperture.copy_wide(
        0,
        0,
        w,
        h,
        0,
        |_, _| 0,
        |_, _| qwords += 1,
        |_, _| words += 1,
    ));
    assert_eq!(words, 0, "an even width on an aligned stride has no edges");
    assert_eq!(
        qwords,
        (gfx::WIDTH / 2) * gfx::HEIGHT,
        "one qword per pixel pair, whole surface"
    );

    // Five pixels on a 4-off stride, base misaligned by 4: one head word, two pairs, no tail.
    let odd = Framebuffer {
        base: 0,
        width: 5,
        height: 1,
        stride: 24,
        order: PixelOrder::Bgrx,
    };
    let aperture = Aperture::new(&odd, bitmap_font::GLYPH_W, 5, 1).expect("a real screen");
    let mut qwords = 0u32;
    let mut words = 0u32;
    assert!(aperture.copy_wide(
        0,
        0,
        5,
        1,
        4,
        |_, _| 0,
        |_, _| qwords += 1,
        |_, _| words += 1,
    ));
    assert_eq!((qwords, words), (2, 1), "head word, then whole pairs");
}

/// The two pixel orders survive the pairing: a qword's low half is the earlier pixel in the
/// screen's byte order, which is the half a big-endian packing or a swapped pair would get
/// wrong and a grey test pattern cannot see.
#[test]
fn a_staged_pair_keeps_the_earlier_pixel_low_in_the_screens_byte_order() {
    use super::Aperture;
    let found = Framebuffer {
        base: 0,
        width: 2,
        height: 1,
        stride: 8,
        order: PixelOrder::Rgbx,
    };
    let aperture = Aperture::new(&found, bitmap_font::GLYPH_W, 2, 1).expect("a real screen");
    let mut seen = [0u8; 8];
    assert!(aperture.copy_wide(
        0,
        0,
        2,
        1,
        0,
        |x, _| 0x0011_0000 | x,
        |at, pair| {
            seen.copy_from_slice(&pair.to_le_bytes());
            assert_eq!(at, 0, "the pair sits at the run's start");
        },
        |_, _| panic!("two aligned pixels need no word store"),
    ));
    // Rgbx exchanges red and blue on the way in: pixel 0 (red set) stores as 0x0000_0011 and
    // pixel 1 (red and blue set) as 0x0001_0011, little-endian bytes below. The low half of
    // the qword is the earlier pixel.
    assert_eq!(
        seen,
        [0x11, 0x00, 0x00, 0x00, 0x11, 0x00, 0x01, 0x00],
        "low half is the earlier pixel, byte order applied per word"
    );
}

/// Both `|`s in `to_words` survive being mutated to `^`, and this is why: each word packs a
/// `u32` in the low half and a value shifted left by 32 in the high half, and a left shift by
/// 32 zeroes exactly the low 32 bits it would otherwise collide with. Checked at the widest
/// each half can be; if the extremes do not collide, no narrower value can either.
#[test]
fn to_words_two_halves_share_no_bit_so_or_and_xor_agree() {
    assert_eq!(u32::MAX as u64 & (u32::MAX as u64) << 32, 0);
}

/// **xenon's screen, covered edge to edge at scale two.** A surface of 7-pixel cells on this
/// 1920x1080 monitor is 960x540 at scale two, and one full flush must write every byte of every
/// visible pixel, each surface pixel as a 2x2 block. The shell's terminal, at the atlas's 14-pixel
/// cell, gets the same 137 columns at scale one: the rule gives both consoles the same width.
#[test]
fn a_1920x1080_screen_is_covered_entirely_at_scale_two() {
    use super::Aperture;
    let found = Framebuffer {
        base: 0,
        width: 1920,
        height: 1080,
        stride: 1920 * 4,
        order: PixelOrder::Bgrx,
    };
    // What the kernel asks for (`display_service::start_screen_terminal`): as much surface as
    // one terminal grid can cover, and the grid must fit a `Vt`.
    let terminal = Aperture::new(
        &found,
        video_terminal::CELL_W,
        (video_terminal::MAX_COLS as u32) * video_terminal::CELL_W,
        u32::MAX,
    )
    .expect("xenon's screen");
    assert_eq!((terminal.size(), terminal.scale()), ((1920, 1080), 1));
    let grid = (1920 / video_terminal::CELL_W, 1080 / video_terminal::CELL_H);
    assert_eq!(grid, (137, 41));
    assert!((grid.0 * grid.1) as usize <= video_terminal::MAX_CELLS);
    assert_eq!(
        grid.0,
        ScreenConsole::new(found).expect("xenon's screen").size().0,
        "the shell's columns are the boot console's"
    );

    let aperture = Aperture::new(&found, bitmap_font::GLYPH_W, 2000, 2000).expect("xenon's screen");
    let (w, h) = aperture.size();
    assert_eq!((aperture.size(), aperture.scale()), ((960, 540), 2));
    assert_eq!(
        aperture.span(),
        found.span().expect("valid"),
        "the whole screen"
    );

    let surface = |x: u32, y: u32| {
        assert!(x < w && y < h, "read ({x},{y}) outside the surface");
        (y << 12) ^ x ^ 0x0080_0000
    };
    let mut screen = vec![0u8; found.span().expect("valid")];
    let mut written = vec![false; screen.len() / 4];
    let cell = core::cell::RefCell::new((&mut screen, &mut written));
    assert!(aperture.copy_wide(
        0,
        0,
        w,
        h,
        0,
        surface,
        |at, pair| {
            let (s, seen) = &mut *cell.borrow_mut();
            s[at..at + 8].copy_from_slice(&pair.to_le_bytes());
            seen[at / 4] = true;
            seen[at / 4 + 1] = true;
        },
        |at, word| {
            let (s, seen) = &mut *cell.borrow_mut();
            s[at..at + 4].copy_from_slice(&word.to_le_bytes());
            seen[at / 4] = true;
        },
    ));
    assert!(written.iter().all(|&w| w), "every screen pixel is written");
    for y in 0..1080 {
        for x in 0..1920 {
            assert_eq!(
                pixel(&found, &screen, x, y),
                surface(x / 2, y / 2),
                "({x},{y})"
            );
        }
    }
}

/// The scaled wide copy and the scaled word copy agree byte for byte, on a scale-two screen
/// with a padded stride that is 4 off 8, for every base misalignment and a spread of
/// rectangles that start and end on odd surface pixels: the paths a damage flush takes.
#[test]
fn the_scaled_wide_copy_lands_the_same_bytes_as_the_scaled_word_copy() {
    use super::Aperture;
    let found = Framebuffer {
        base: 0,
        width: 1683,
        height: 9,
        stride: 1683 * 4 + 4,
        order: PixelOrder::Rgbx,
    };
    let aperture = Aperture::new(&found, bitmap_font::GLYPH_W, 2000, 2000).expect("a real screen");
    assert_eq!((aperture.size(), aperture.scale()), ((841, 4), 2));
    let surface = |x: u32, y: u32| (y << 16) | x;
    let span = found.span().expect("valid");
    for (x, y, rw, rh) in [(0, 0, 841, 4), (1, 1, 3, 2), (5, 0, 836, 1), (840, 3, 1, 1)] {
        let mut narrow = vec![0xa5u8; span];
        assert!(aperture.copy(x, y, rw, rh, surface, |at, word| {
            narrow[at..at + 4].copy_from_slice(&word.to_le_bytes());
        }));
        for misalign in [0, 4] {
            let wide = core::cell::RefCell::new(vec![0xa5u8; span]);
            assert!(aperture.copy_wide(
                x,
                y,
                rw,
                rh,
                misalign,
                surface,
                |at, pair| wide.borrow_mut()[at..at + 8].copy_from_slice(&pair.to_le_bytes()),
                |at, word| wide.borrow_mut()[at..at + 4].copy_from_slice(&word.to_le_bytes()),
            ));
            assert!(
                narrow == wide.into_inner(),
                "({x},{y}) {rw}x{rh} misalign {misalign}"
            );
        }
    }
}

/// An rgbx screen gets its red and blue exchanged on the way in, which is the byte-order half
/// of the copy and the half a grey test pattern cannot see.
#[test]
fn an_aperture_stores_in_the_screens_byte_order() {
    use super::Aperture;
    let (found, mut pixels) = screen(1, 1, 0, PixelOrder::Rgbx);
    let aperture = Aperture::new(&found, bitmap_font::GLYPH_W, 1, 1).expect("one pixel");
    assert!(aperture.copy(
        0,
        0,
        1,
        1,
        |_, _| 0x0011_2233,
        |at, word| {
            pixels[at..at + 4].copy_from_slice(&word.to_le_bytes());
        }
    ));
    assert_eq!(
        &pixels[0..4],
        &[0x11, 0x22, 0x33, 0x00],
        "bytes R, G, B, unused"
    );
}

/// A framebuffer slice shorter than the geometry claims must truncate the picture, never panic.
/// The console runs on the path where something else has already gone wrong.
#[test]
fn a_short_framebuffer_truncates_rather_than_panicking() {
    let (found, _) = screen(8, 8, 0, PixelOrder::Bgrx);
    let mut console = ScreenConsole::new(found).expect("eight by eight cells");
    let mut pixels = [0u8; 64];
    clear(&mut console, &mut pixels);
    write(
        &mut console,
        &mut pixels,
        "this is far more text than sixty-four bytes can hold\n",
    );
}

/// A pixel sink reports whether it covers anything, which is what a caller checks before it
/// paints into a slice it was handed.
#[test]
fn a_pixel_sink_knows_when_it_covers_nothing() {
    assert!(PixelSink::new(&mut []).is_empty());
    let mut one = [0u8; 4];
    assert!(!PixelSink::new(&mut one).is_empty());
}

/// **The grid holds 150 rows at the widest scale-one screen.** A screen just under two scales'
/// worth of columns wide and 1200 pixels tall is the case [`Cells::CAPACITY`] was sized for, and
/// it gets every row.
#[test]
fn the_grid_holds_150_rows_at_the_widest_scale_one_screen() {
    let cols = 2 * super::MIN_COLUMNS - 1;
    let width = cols * bitmap_font::GLYPH_W;
    assert_eq!(ScreenConsole::scale_for(width), 1);
    let found = Framebuffer {
        base: 0,
        width,
        height: 1200,
        stride: width * 4,
        order: PixelOrder::Bgrx,
    };
    assert_eq!(ScreenConsole::new(found).unwrap().size(), (cols, 150));
}

/// **A console whose rows fill the grid exactly still scrolls.** The bottom row has no row under
/// it in the grid, so the scroll must blank it rather than read the cell past the grid's end. A
/// short pixel slice is enough: the painter truncates rather than panics.
#[test]
fn a_console_that_fills_the_grid_scrolls_without_reading_past_it() {
    let cols = super::MIN_COLUMNS;
    let rows = (Cells::CAPACITY / cols as usize) as u32;
    let width = cols * bitmap_font::GLYPH_W;
    let found = Framebuffer {
        base: 0,
        width,
        height: rows * bitmap_font::GLYPH_H,
        stride: width * 4,
        order: PixelOrder::Bgrx,
    };
    let mut console = ScreenConsole::new(found).unwrap();
    assert_eq!(console.size(), (cols, rows));
    let mut pixels = vec![0u8; 4096];
    let mut cells = Box::new(Cells::new());
    console.clear(&mut cells, &mut PixelSink::new(&mut pixels));
    for _ in 0..rows + 1 {
        console.write(&mut cells, &mut PixelSink::new(&mut pixels), "x\n");
    }
}

/// **The largest scale draws.** At scale 16 a glyph row is exactly the painter's run buffer, and
/// it must be painted rather than skipped as too wide.
#[test]
fn a_glyph_at_scale_sixteen_is_painted() {
    let width = 16 * bitmap_font::GLYPH_W * super::MIN_COLUMNS;
    assert_eq!(ScreenConsole::scale_for(width), 16);
    let found = Framebuffer {
        base: 0,
        width,
        height: 16 * bitmap_font::GLYPH_H,
        stride: width * 4,
        order: PixelOrder::Bgrx,
    };
    let mut console = ScreenConsole::new(found).unwrap();
    // The first glyph row's sixteen screen rows, which is where the top bar of an F is.
    let mut pixels = vec![0u8; 16 * width as usize * 4];
    write(&mut console, &mut pixels, "F");
    // The bar starts one font pixel in: screen pixel (16, 0) onward is ink.
    assert_eq!(pixel(&found, &pixels, 16 + 8, 8), ScreenConsole::FOREGROUND);
}
