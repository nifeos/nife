//! **Reading a screen the way [`super::watch`] reads a wire.**
//!
//! On a machine with no serial port nife's console is a framebuffer (`screen_console`). This turns
//! a screendump of it back into the text written into it, so [`super::progress`] applies
//! unchanged. [`bitmap_font`] is a constant table of 7x8 glyphs, so a cell matches a glyph bit for
//! bit or matches nothing: an exact lookup, not optical character recognition. `cargo xtask
//! uefi-boot` reads the tour off a QEMU screendump with [`read`], and the graphical `swish-check`
//! leg reads the display terminal with [`decode_exact`]. A photograph of a screen is a different
//! problem; `notes/serial-less-output.md` records it. See notes/board-console/modules.md.

//! # BUGS
//!
//! - **Trailing blanks are trimmed and leading ones are not**: a framebuffer pads every row with
//!   background, and leading blanks are the boot tour's indentation.
//! - **A cell that matches no glyph reads as `?`**, not as an error (a logo, half a character).
//! - **Characters sharing a glyph decode as the first in alphabet order**, and nothing checks
//!   `bitmap_font` for duplicate glyphs.
//! - **A scheme whose ink is not the brighter colour decodes to a grid of `?`**: ink and paper
//!   split at their per-channel midpoint ([`ink_thresholds`]).

/// What `screen_console` paints ink with, as one 24-bit RGB triplet: its own
/// [`screen_console::ScreenConsole::FOREGROUND`], converted, so the reader takes its color from
/// the crate that paints.
pub(crate) const INK: [u8; 3] = rgb(screen_console::ScreenConsole::FOREGROUND);

/// What it paints background with: [`screen_console::ScreenConsole::BACKGROUND`].
pub(crate) const PAPER: [u8; 3] = rgb(screen_console::ScreenConsole::BACKGROUND);

/// The three channels of a `0x00RRGGBB` word, in the order a PPM stores them.
const fn rgb(word: u32) -> [u8; 3] {
    [(word >> 16) as u8, (word >> 8) as u8, word as u8]
}

/// Where each channel stops being `paper` and starts being `ink`: their midpoint. A midpoint
/// rather than an exact match, so that a screendump that went through a color conversion still
/// reads. Computed from the scheme rather than fixed at `0x60`, because the scheme is data.
fn ink_thresholds(ink: [u8; 3], paper: [u8; 3]) -> [u8; 3] {
    let mid = |ink: u8, paper: u8| ((u16::from(ink) + u16::from(paper)) / 2) as u8;
    [
        mid(ink[0], paper[0]),
        mid(ink[1], paper[1]),
        mid(ink[2], paper[2]),
    ]
}

/// Every printable ASCII byte, in ASCII order: the alphabet [`read`] decodes with, so any
/// character the console can draw comes back, and `' '` wins any tie with another blank glyph.
const PRINTABLE: [u8; 0x7f - 0x20] = {
    let mut alphabet = [0u8; 0x7f - 0x20];
    let mut byte = 0x20u8;
    while byte < 0x7f {
        alphabet[(byte - 0x20) as usize] = byte;
        byte += 1;
    }
    alphabet
};

/// What went wrong reading a dump. Small on purpose: there are only two ways a PPM can be
/// unreadable that are worth telling apart.
#[derive(Debug, PartialEq, Eq)]
pub enum ReadError {
    /// The bytes are not a binary (`P6`) portable pixmap, or its header is malformed.
    NotAPixmap,
    /// The header describes more pixels than the file holds.
    Truncated {
        /// What the header promised, in bytes of pixel data.
        wanted: usize,
        /// What was there.
        got: usize,
    },
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAPixmap => write!(f, "not a binary P6 portable pixmap"),
            Self::Truncated { wanted, got } => {
                write!(
                    f,
                    "the pixmap promised {wanted} bytes of pixels and has {got}"
                )
            }
        }
    }
}

impl std::error::Error for ReadError {}

/// **Turn a screendump back into the text that was drawn into it.**
///
/// One line per row of character cells, newline-terminated, trailing blanks trimmed. Rows that are
/// entirely blank are kept, because a blank line in a boot tour is a blank line.
///
/// # Errors
///
/// [`ReadError`] when the bytes are not a readable binary PPM.
///
/// # Examples
///
/// ```
/// use serial_console::screen;
///
/// // A two-by-one grid of 7x8 cells, in the PPM QEMU's `screendump` writes.
/// let mut ppm = b"P6\n14 8\n255\n".to_vec();
/// for y in 0..8 {
///     for x in 0..14 {
///         let ink = bitmap_font::is_ink(if x < 7 { 'h' } else { 'i' }, x % 7, y);
///         ppm.extend_from_slice(if ink { &[0xc8, 0xc8, 0xc8] } else { &[0, 0, 0] });
///     }
/// }
///
/// assert_eq!(screen::read(&ppm).unwrap(), "hi\n");
/// ```
pub fn read(ppm: &[u8]) -> Result<String, ReadError> {
    let (width, height, pixels) = parse_pixmap(ppm)?;
    // The console draws a wide screen at an integer scale (`ScreenConsole::scale_for`, 2026-10-04),
    // and asking the painter's own rule is what keeps this reader from disagreeing with it.
    let scale =
        screen_console::ScreenConsole::scale_for(u32::try_from(width).unwrap_or(u32::MAX)) as usize;
    let mut out = String::new();
    for row in decode_rows(
        pixels,
        width,
        height,
        scale,
        &PRINTABLE,
        ink_thresholds(INK, PAPER),
    ) {
        out.push_str(row.trim_end());
        out.push('\n');
    }
    Ok(out)
}

/// **A screendump read back**: the picture's own geometry, and its rows of text.
///
/// The geometry rides along because a caller that asserts it wants it from the same parse that
/// produced the rows, and this module owns the only parse. The graphical `swish-check` leg is
/// that caller: a scanout of any size but the surface's is a `SET_SCANOUT` bug, not a near miss.
///
/// Name: provisional, minted 2026-10-16 by lane/377 (one screendump decoder, not two).
#[derive(Debug, PartialEq, Eq)]
pub struct Screendump {
    /// Width of the pixmap, in pixels.
    pub width: u32,
    /// Height of the pixmap, in pixels.
    pub height: u32,
    /// One string per row of character cells, `?` where nothing in the alphabet matched. Trailing
    /// blanks are left as they were drawn; trimming them is [`read`]'s presentation, not the
    /// decoder's.
    pub rows: Vec<String>,
}

/// **Turn a screendump drawn in `ink` on `paper` back into its rows of text**, decoding each
/// cell against `alphabet` at `scale`, and reading `?` where nothing in the alphabet matches.
///
/// [`read`] is this with the console's scheme, the painter's own scale and every printable
/// character; anti-aliased cells need [`decode_exact`]. `scale` is how many screen pixels wide one
/// font pixel was painted (`screen_console::ScreenConsole::scale_for` for [`read`]), and zero
/// panics on the divide.
///
/// # Errors
///
/// [`ReadError`] when the bytes are not a readable binary PPM.
///
/// # Examples
///
/// ```
/// use serial_console::screen;
///
/// let (ink, paper) = ([0xee, 0xe8, 0xd5], [0x07, 0x36, 0x42]);
/// let mut ppm = b"P6\n14 8\n255\n".to_vec();
/// for y in 0..8u32 {
///     for x in 0..14u32 {
///         let word = if bitmap_font::is_ink(if x < 7 { 'h' } else { 'i' }, x % 7, y) {
///             0x00ee_e8d5
///         } else {
///             0x0007_3642
///         };
///         ppm.extend_from_slice(&[(word >> 16) as u8, (word >> 8) as u8, word as u8]);
///     }
/// }
///
/// assert_eq!(screen::decode(&ppm, ink, paper, b"hi", 1).unwrap().rows, ["hi"]);
/// ```
///
/// Name: provisional, minted 2026-10-16 by lane/377 (one screendump decoder, not two).
pub fn decode(
    ppm: &[u8],
    ink: [u8; 3],
    paper: [u8; 3],
    alphabet: &[u8],
    scale: u32,
) -> Result<Screendump, ReadError> {
    let (width, height, pixels) = parse_pixmap(ppm)?;
    Ok(Screendump {
        width: u32::try_from(width).unwrap_or(u32::MAX),
        height: u32::try_from(height).unwrap_or(u32::MAX),
        rows: decode_rows(
            pixels,
            width,
            height,
            scale as usize,
            alphabet,
            ink_thresholds(ink, paper),
        ),
    })
}

/// The grid's rows, one string of cells each: the whole decode, shared by [`read`] and
/// [`decode`].
fn decode_rows(
    pixels: &[u8],
    width: usize,
    height: usize,
    scale: usize,
    alphabet: &[u8],
    thresholds: [u8; 3],
) -> Vec<String> {
    let cols = width / (bitmap_font::GLYPH_W as usize * scale);
    let rows = height / (bitmap_font::GLYPH_H as usize * scale);
    (0..rows)
        .map(|row| {
            (0..cols)
                .map(|col| cell(pixels, width, scale, col, row, alphabet, thresholds))
                .collect()
        })
        .collect()
}

/// **Turn a screendump of `cell_w` by `cell_h` cells back into rows of text by exact match**: a
/// cell reads as the first byte of `alphabet` whose `draw(ch, x, y)`, a `0x00RRGGBB` word for
/// pixel `(x, y)` of the cell, equals every pixel the dump holds there, and as `?` otherwise.
///
/// For glyphs that are not monochrome, such as `glyph_atlas`'s anti-aliased cells; the caller
/// passes the function the guest painted with. `scale` is [`decode`]'s, and the top-left screen
/// pixel of each block is the one compared.
///
/// # Errors
///
/// [`ReadError`] when the bytes are not a readable binary PPM.
///
/// # Examples
///
/// ```
/// use serial_console::screen;
///
/// // Two 2x1 cells: `a` is left-half ink, `b` right-half, on black.
/// let draw = |ch: char, x: u32, _y: u32| match (ch, x) {
///     ('a', 0) | ('b', 1) => 0x00ff_ffff,
///     _ => 0,
/// };
/// let mut ppm = b"P6\n4 1\n255\n".to_vec();
/// for x in 0..4u32 {
///     let word = draw(if x < 2 { 'b' } else { 'a' }, x % 2, 0);
///     ppm.extend_from_slice(&[(word >> 16) as u8, (word >> 8) as u8, word as u8]);
/// }
/// assert_eq!(screen::decode_exact(&ppm, 2, 1, 1, b"ab", draw).unwrap().rows, ["ba"]);
/// ```
///
/// Name: provisional.
pub fn decode_exact(
    ppm: &[u8],
    cell_w: u32,
    cell_h: u32,
    scale: u32,
    alphabet: &[u8],
    draw: impl Fn(char, u32, u32) -> u32,
) -> Result<Screendump, ReadError> {
    let (width, height, pixels) = parse_pixmap(ppm)?;
    let (cw, ch, k) = (cell_w as usize, cell_h as usize, scale as usize);
    let at = |col: usize, row: usize, x: usize, y: usize| {
        let i = (((row * ch + y) * width + col * cw + x) * k) * 3;
        (pixels[i] as u32) << 16 | (pixels[i + 1] as u32) << 8 | pixels[i + 2] as u32
    };
    let read = |col: usize, row: usize| {
        alphabet
            .iter()
            .map(|&b| b as char)
            .find(|&c| {
                (0..ch).all(|y| (0..cw).all(|x| at(col, row, x, y) == draw(c, x as u32, y as u32)))
            })
            .unwrap_or('?')
    };
    Ok(Screendump {
        width: u32::try_from(width).unwrap_or(u32::MAX),
        height: u32::try_from(height).unwrap_or(u32::MAX),
        rows: (0..height / (ch * k))
            .map(|row| (0..width / (cw * k)).map(|col| read(col, row)).collect())
            .collect(),
    })
}

/// The character in one cell, or `?` when the cell matches nothing in `alphabet`.
fn cell(
    pixels: &[u8],
    width: usize,
    scale: usize,
    col: usize,
    row: usize,
    alphabet: &[u8],
    thresholds: [u8; 3],
) -> char {
    let left = col * bitmap_font::GLYPH_W as usize * scale;
    let top = row * bitmap_font::GLYPH_H as usize * scale;
    let mut seen = [[false; bitmap_font::GLYPH_W as usize]; bitmap_font::GLYPH_H as usize];
    for (y, line) in seen.iter_mut().enumerate() {
        for (x, ink) in line.iter_mut().enumerate() {
            // One sample per scale-by-scale block: its top-left screen pixel.
            let at = ((top + y * scale) * width + left + x * scale) * 3;
            *ink = pixels[at..at + 3]
                .iter()
                .zip(thresholds)
                .all(|(channel, threshold)| *channel >= threshold);
        }
    }
    // First match in `alphabet` order wins. `read` passes the alphabet ASCII-sorted, so `' '`
    // still wins any tie with another blank glyph.
    for &byte in alphabet {
        let ch = byte as char;
        let matches = seen.iter().enumerate().all(|(y, line)| {
            line.iter()
                .enumerate()
                .all(|(x, ink)| *ink == bitmap_font::is_ink(ch, x as u32, y as u32))
        });
        if matches {
            return ch;
        }
    }
    '?'
}

/// Header and pixel data of a binary portable pixmap: `(width, height, pixels)`.
///
/// PPM's header is whitespace-separated tokens with `#` comments, and exactly **one** whitespace
/// byte after the maximum value belongs to the header rather than to the pixels. Getting that one
/// byte wrong shifts the whole image by a third of a pixel, which looks like a font bug.
fn parse_pixmap(ppm: &[u8]) -> Result<(usize, usize, &[u8]), ReadError> {
    let token = |at: &mut usize| -> Option<usize> {
        loop {
            while ppm.get(*at).is_some_and(|b| b.is_ascii_whitespace()) {
                *at += 1;
            }
            if ppm.get(*at) == Some(&b'#') {
                while ppm.get(*at).is_some_and(|b| *b != b'\n') {
                    *at += 1;
                }
                continue;
            }
            break;
        }
        let start = *at;
        while ppm
            .get(*at)
            .is_some_and(|b| !b.is_ascii_whitespace() && *b != b'#')
        {
            *at += 1;
        }
        (start != *at).then_some(start)?;
        std::str::from_utf8(&ppm[start..*at]).ok()?.parse().ok()
    };

    if ppm.get(..2) != Some(b"P6") {
        return Err(ReadError::NotAPixmap);
    }
    // Past the magic; the three header numbers follow, whitespace-separated.
    let mut at = 2;
    let width = token(&mut at).ok_or(ReadError::NotAPixmap)?;
    let height = token(&mut at).ok_or(ReadError::NotAPixmap)?;
    let max = token(&mut at).ok_or(ReadError::NotAPixmap)?;
    if max != 255 || width == 0 || height == 0 {
        return Err(ReadError::NotAPixmap);
    }
    // Exactly one whitespace byte separates the header from the pixels.
    at += 1;
    // The header is the dump's own claim, so its product is checked: a wrapped `wanted` would let
    // a short buffer pass and the decoders index past it.
    let wanted = width
        .checked_mul(height)
        .and_then(|n| n.checked_mul(3))
        .ok_or(ReadError::NotAPixmap)?;
    let pixels = ppm.get(at..).unwrap_or(&[]);
    if pixels.len() < wanted {
        return Err(ReadError::Truncated {
            wanted,
            got: pixels.len(),
        });
    }
    Ok((width, height, &pixels[..wanted]))
}

#[cfg(test)]
mod tests {
    use machine_discovery::framebuffer::{Framebuffer, PixelOrder};
    use screen_console::{Cells, PixelSink, ScreenConsole};

    use super::{INK, PAPER, ReadError, decode, decode_exact, read};

    /// **`decode_exact` reads a grid of cells at a scale, by every channel**: two rows of two
    /// 3x2 cells drawn at scale two, each cell's own character a different color in each channel,
    /// so a cell read from the wrong row, the wrong column, the wrong place in a block or with a
    /// channel lost matches nothing and reads `?`.
    #[test]
    fn decode_exact_reads_each_cell_at_its_scale() {
        let draw = |ch: char, x: u32, y: u32| -> u32 {
            let k = ch as u32 - b'a' as u32 + 1;
            ((k * 0x10 + x) << 16) | ((k * 0x20 + y) << 8) | (k * 0x30)
        };
        let screen = [['a', 'b'], ['c', 'd']];
        let (w, h) = (2 * 3 * 2, 2 * 2 * 2);
        let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
        for sy in 0..h {
            for sx in 0..w {
                let (x, y) = (sx / 2, sy / 2);
                let word = draw(screen[(y / 2) as usize][(x / 3) as usize], x % 3, y % 2);
                ppm.extend_from_slice(&[(word >> 16) as u8, (word >> 8) as u8, word as u8]);
            }
        }
        let read = decode_exact(&ppm, 3, 2, 2, b"abcd", draw).expect("a readable dump");
        assert_eq!(read.rows, ["ab", "cd"]);
        assert_eq!((read.width, read.height), (w, h));
    }

    /// Paint `text` with the **kernel's own console** and hand back the screendump QEMU would have
    /// produced of it.
    ///
    /// This is what makes the test below worth having rather than a test of this file against
    /// itself: the writer is the crate the kernel links, so a change to either side that the other
    /// does not follow fails here, on the host, in milliseconds. It is the same rule the
    /// `uefi_loader` handoff tests are held to.
    fn painted(cols: u32, rows: u32, text: &str) -> Vec<u8> {
        painted_px(
            cols * bitmap_font::GLYPH_W,
            rows * bitmap_font::GLYPH_H,
            text,
        )
    }

    /// [`painted`] at a pixel size, for screens wide enough that the console scales its font.
    fn painted_px(width: u32, height: u32, text: &str) -> Vec<u8> {
        let screen = Framebuffer {
            base: 0,
            width,
            height,
            // A padded stride, because a decoder that ignores the difference between the two would
            // pass with the picture sheared and this test is the only thing that would notice.
            stride: width * 4 + 16,
            order: PixelOrder::Bgrx,
        };
        let mut pixels = vec![0u8; screen.span().expect("a valid geometry")];
        let mut console = ScreenConsole::new(screen).expect("a screen of whole cells");
        let mut cells = Box::new(Cells::new());
        console.clear(&mut cells, &mut PixelSink::new(&mut pixels));
        console.write(&mut cells, &mut PixelSink::new(&mut pixels), text);

        // QEMU's screendump is 24-bit RGB with no padding, so this is the conversion the emulator
        // does: drop the stride, drop the unused byte, and reorder to R, G, B.
        let mut ppm = format!("P6\n{} {}\n255\n", screen.width, screen.height).into_bytes();
        for y in 0..screen.height {
            for x in 0..screen.width {
                let at = (y * screen.stride + x * 4) as usize;
                let bgrx = &pixels[at..at + 4];
                ppm.extend_from_slice(&[bgrx[2], bgrx[1], bgrx[0]]);
            }
        }
        ppm
    }

    /// **The whole claim of this module, end to end**: what the kernel's console draws is what this
    /// reads back, exactly, with no screen and no emulator involved.
    #[test]
    fn what_the_kernel_console_draws_is_what_this_reads_back() {
        let tour = "nife on x86_64 (long mode, ring 0, 4-level paging)\n\
                      cpu 0 booted: high-half kernel, .bss, and the 16550 console are up.\n\
                    \n\
                    nife x86_64: boot complete, halting.\n";
        let decoded = read(&painted(80, 6, tour)).expect("a readable dump");
        for line in tour.lines() {
            assert!(
                decoded.contains(line),
                "{line:?} was drawn and did not come back\n--- decoded ---\n{decoded}"
            );
        }
    }

    /// xenon's 1920x1080 monitor, where the console draws every font pixel as a 2x2 block
    /// (`ScreenConsole::scale_for`, 2026-10-04). The reader must decode at the painter's scale, and
    /// a line longer than the 137 columns must come back wrapped, not cut.
    #[test]
    fn a_scaled_screen_reads_back_at_the_painters_scale() {
        assert_eq!(ScreenConsole::scale_for(1920), 2);
        let long = "disk-throughput: ".repeat(9);
        let text = format!("diag vt-d frcd0 sid 01:00.0 reason 0x01 read addr 0x9e1000\n{long}\n");
        let decoded = read(&painted_px(1920, 64, &text)).expect("a readable dump");
        let lines: Vec<&str> = decoded.lines().collect();
        assert_eq!(
            lines[0],
            "diag vt-d frcd0 sid 01:00.0 reason 0x01 read addr 0x9e1000"
        );
        assert_eq!(
            lines[1].len(),
            137,
            "the long line fills one row of 137 columns"
        );
        assert_eq!(format!("{}{}", lines[1], lines[2]), long.trim_end());
    }

    /// The point of decoding at all: [`crate::progress`] judges a screen exactly as it judges a
    /// wire, with nothing between them that knows which it is.
    #[test]
    fn a_decoded_screen_is_judged_by_the_same_recogniser_as_a_serial_log() {
        let banner = "nife on x86_64 (long mode, ring 0, 4-level paging)";
        let decoded = read(&painted(80, 4, &format!("{banner}\n"))).expect("a readable dump");

        let mut from_the_screen = crate::progress::BootProgress::default();
        for line in decoded.lines() {
            from_the_screen.observe_line(line);
        }
        let mut from_a_wire = crate::progress::BootProgress::default();
        from_a_wire.observe_line(banner);

        assert_eq!(
            from_the_screen.reached(),
            from_a_wire.reached(),
            "the same text reaches the same stage however it was carried"
        );
        assert_ne!(
            from_a_wire.reached(),
            crate::progress::Stage::Cold,
            "and the stage it reached is not the do-nothing one, or this proves nothing"
        );
    }

    /// Text that wraps and text that scrolls both come back, because the console's own wrapping is
    /// what put it there and this reads the result rather than replaying the input.
    #[test]
    fn a_wrapped_line_reads_back_as_the_two_lines_it_became() {
        let decoded = read(&painted(4, 2, "abcdefgh")).expect("a readable dump");
        assert_eq!(decoded, "abcd\nefgh\n");
    }

    /// A blank screen is blank lines, not an error and not `?`s.
    #[test]
    fn a_cleared_screen_decodes_to_nothing() {
        let decoded = read(&painted(8, 3, "")).expect("a readable dump");
        assert_eq!(decoded, "\n\n\n");
    }

    /// A cell holding something that is not one of our glyphs is one `?`, and the rest of the
    /// screen still decodes. See this module's BUGS.
    #[test]
    fn a_cell_that_is_not_a_glyph_reads_as_a_question_mark() {
        let mut ppm = painted(2, 1, "ab");
        let header = ppm.len() - 14 * 8 * 3;
        // Scribble a diagonal through the first cell that no 7x8 glyph could be.
        for y in 0..8 {
            let at = header + (y * 14 + y.min(6)) * 3;
            ppm[at..at + 3].copy_from_slice(&[0xff, 0xff, 0xff]);
        }
        assert_eq!(read(&ppm).expect("still readable"), "?b\n");
    }

    /// The two ways a dump can be unusable are told apart, because "you pointed me at a PNG" and
    /// "the emulator was still writing" want different reactions from whoever reads the failure.
    #[test]
    fn an_unreadable_dump_says_which_kind_it_is() {
        assert_eq!(read(b"\x89PNG\r\n").unwrap_err(), ReadError::NotAPixmap);
        assert_eq!(read(b"P6\n2 2\n254\n").unwrap_err(), ReadError::NotAPixmap);
        assert!(matches!(
            read(b"P6\n8 8\n255\n\x00\x00\x00").unwrap_err(),
            ReadError::Truncated {
                wanted: 192,
                got: 3
            }
        ));
    }

    /// A comment in the header is legal PPM and nothing in this tree writes one, so it is the sort
    /// of thing a parser gets wrong the first time somebody else's tool produces a dump.
    #[test]
    fn a_header_comment_is_skipped() {
        let mut ppm = b"P6\n# written by something else\n7 8\n255\n".to_vec();
        ppm.extend(std::iter::repeat_n(PAPER, 7 * 8).flatten());
        assert_eq!(read(&ppm).expect("readable"), "\n");
    }

    /// [`ReadError`]'s `Display`, which nothing else in the suite reads: the other error test
    /// checks the *value* with `PartialEq`, never the words a person at a bench would see.
    #[test]
    fn a_read_error_displays_what_went_wrong() {
        assert_eq!(
            ReadError::NotAPixmap.to_string(),
            "not a binary P6 portable pixmap"
        );
        assert_eq!(
            ReadError::Truncated { wanted: 10, got: 3 }.to_string(),
            "the pixmap promised 10 bytes of pixels and has 3"
        );
    }

    /// **Width and height of zero are each rejected alone**, not only together. The header check
    /// is three conditions joined by `||`; a test that only ever sets more than one of them at
    /// once cannot tell that operator apart from `&&`, since either grouping still fires when
    /// every condition is true. `an_unreadable_dump_says_which_kind_it_is` covers the third
    /// (`max != 255`) alone already; this covers the other two.
    #[test]
    fn width_or_height_of_zero_is_rejected_even_when_the_other_is_fine() {
        assert_eq!(read(b"P6\n0 5\n255\n").unwrap_err(), ReadError::NotAPixmap);
        assert_eq!(read(b"P6\n5 0\n255\n").unwrap_err(), ReadError::NotAPixmap);
    }

    /// **A header whose size overflows is refused, not wrapped.** In a release build an unchecked
    /// `width * height * 3` wraps to a small number, the length check passes on a short buffer,
    /// and `decode_exact` then indexes past it.
    #[test]
    fn a_header_whose_size_overflows_is_refused() {
        let huge = format!("P6\n{} 3\n255\n", usize::MAX / 2);
        assert_eq!(read(huge.as_bytes()).unwrap_err(), ReadError::NotAPixmap);
    }

    /// **The display terminal's default scheme decodes through the same `decode`** (milestone
    /// 377). The colors are read from `video_terminal` itself, because the scheme is that
    /// crate's choice and a copy of two RGB words here is exactly the drift this decoder exists
    /// to refuse; the alphabet and the 1:1 scale are the graphical swish-check leg's, whose live
    /// screen this reads.
    #[test]
    fn the_display_terminals_default_scheme_decodes_like_the_consoles() {
        let (fg, bg) = video_terminal::Attr::DEFAULT.colours();
        let ink = [(fg >> 16) as u8, (fg >> 8) as u8, fg as u8];
        let paper = [(bg >> 16) as u8, (bg >> 8) as u8, bg as u8];

        let mut ppm = b"P6\n14 8\n255\n".to_vec();
        for y in 0..8u32 {
            for x in 0..14u32 {
                let word = if bitmap_font::is_ink(if x < 7 { 'h' } else { 'i' }, x % 7, y) {
                    fg
                } else {
                    bg
                };
                ppm.extend_from_slice(&[(word >> 16) as u8, (word >> 8) as u8, word as u8]);
            }
        }

        let screen = decode(&ppm, ink, paper, b"hi", 1).expect("a readable dump");
        assert_eq!(screen.rows, ["hi"]);
        assert_eq!(
            (screen.width, screen.height),
            (14, 8),
            "the geometry rides along for the caller that asserts it"
        );
    }

    /// **INK and PAPER are the console's own colors, split into PPM channel order**, asserted
    /// against the words the console paints with. [`read`] classifies every pixel
    /// channel-by-channel against thresholds derived from these two constants and nothing else,
    /// and no paint-and-read test can see a wrong split: with the console's near-black paper, a
    /// channel of INK shifted to zero (`>>` become `<<`) moves the matching threshold to zero
    /// too, and every pixel the console paints classifies exactly as before. The split itself
    /// is the contract, so that is what is asserted.
    #[test]
    fn ink_and_paper_are_the_console_colors_in_ppm_channel_order() {
        let (fg, bg) = (ScreenConsole::FOREGROUND, ScreenConsole::BACKGROUND);
        assert_eq!(INK, [(fg >> 16) as u8, (fg >> 8) as u8, fg as u8]);
        assert_eq!(PAPER, [(bg >> 16) as u8, (bg >> 8) as u8, bg as u8]);
    }

    /// **Ink and paper split at their midpoint, and both sides of the arithmetic are
    /// load-bearing.** The console's own scheme cannot show that: any threshold strictly
    /// between its two colors classifies the same pixels. So [`decode`] runs here on a scheme
    /// chosen to straddle the arithmetic, `0xc0` ink on `0x40` paper: the true midpoint is
    /// `0x80`, where `(ink - paper) / 2` would read `0x40` and paper pixels would pass, and
    /// `(ink + paper) * 2` would wrap past the byte to `0x00`, where every pixel does. Either
    /// way a painted cell goes all-ink and the character comes back `?`, so the exact text
    /// below is what fails.
    #[test]
    fn the_split_between_ink_and_paper_is_their_midpoint() {
        let (ink, paper) = ([0xc0, 0xc0, 0xc0], [0x40, 0x40, 0x40]);
        let mut ppm = b"P6\n14 8\n255\n".to_vec();
        for y in 0..8u32 {
            for x in 0..14u32 {
                let word = if bitmap_font::is_ink(if x < 7 { 'h' } else { 'i' }, x % 7, y) {
                    0x00c0_c0c0
                } else {
                    0x0040_4040
                };
                ppm.extend_from_slice(&[(word >> 16) as u8, (word >> 8) as u8, word as u8]);
            }
        }
        assert_eq!(
            decode(&ppm, ink, paper, b"hi", 1)
                .expect("a readable dump")
                .rows,
            ["hi"]
        );
    }

    /// **The alphabet constrains the decode**: a cell whose glyph is outside it reads `?` while
    /// its neighbor still comes back, which is what makes the swish-check leg's failures name
    /// the character it could not read rather than lie about the row.
    #[test]
    fn a_glyph_outside_the_alphabet_reads_as_a_question_mark() {
        let screen = decode(&painted(2, 1, "hi"), INK, PAPER, b"i", 1).expect("a readable dump");
        assert_eq!(screen.rows, ["?i"]);
    }

    /// **[`decode`]'s rows are the whole grid, trailing blanks included.** The caller that
    /// asserts geometry also searches the raw rows, and trimming is [`read`]'s presentation
    /// choice rather than the decoder's. Note the alphabet has to carry the space for a blank
    /// cell to come back as one.
    #[test]
    fn a_decoded_row_keeps_its_trailing_blanks() {
        let screen = decode(&painted(2, 1, "h "), INK, PAPER, b"h ", 1).expect("a readable dump");
        assert_eq!(screen.rows, ["h "]);
    }
}
