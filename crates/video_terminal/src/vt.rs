//! The engine: the parser's state and `Vt`, the grid it maintains.
//!
//! Name: provisional (the lane of milestone 874 (the terminal engine moves its palette and its
//! tests out), 2026-10-12 UTC), named for `Vt`, the central type, as §282 (when a file is split,
//! the parent becomes an index) asks; calef names things.

use super::cell::{
    BOLD, DIM, INVISIBLE, ITALIC, REVERSE, STRIKETHROUGH, STRIKETHROUGH_ROW, UNDERLINE_CURLY,
    UNDERLINE_DASHED, UNDERLINE_DOTTED, UNDERLINE_DOUBLE, UNDERLINE_NONE, UNDERLINE_ROW,
    UNDERLINE_SINGLE,
};
use super::*;
use crate::handoff::{
    CELL_WORDS, CELL_WORDS_1, FLAG_CURSOR_VISIBLE, FLAG_IGNORE, FLAG_WRAP_PENDING, HEADER_WORDS,
    HEADER_WORDS_1, LAYOUT, MAGIC, Refusal, cell_words, color_word, put_word, underline_word,
    word_at, words_attr, words_attr_1, words_cell, words_cell_1,
};

/// The parser's state. Deliberately few: most of a full VT's dozen states distinguish sequences
/// this engine swallows anyway. See notes/video-terminal.md, "Parsing".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Printing.
    Ground,
    /// `ESC` has been seen.
    Esc,
    /// Inside `ESC [`, accumulating parameters.
    Csi,
    /// Inside a **string** sequence (`ESC ]`, the operating-system command, and its relatives),
    /// whose arbitrary text a parser without this state would print, a window title included.
    Str,
    /// Inside a string sequence, having just seen `ESC`: a `\` ends the string (ST), anything else
    /// is part of it.
    StrEsc,
}

/// How many numeric parameters a CSI sequence may carry: a foreground and a background in 24-bit
/// color are ten, with room for a reset and a rendition or two. A seventeenth marks the sequence
/// ignored rather than growing the array.
pub(crate) const MAX_PARAMS: usize = 16;

/// **A terminal's live grid, plus its off-screen history.**
///
/// Construct with [`Vt::new`], push bytes with [`Vt::feed`], read pixels with [`Vt::pixel`], ask
/// what changed with [`Vt::take_damage`], and scroll with [`Vt::scroll_up`]/[`Vt::scroll_down`].
///
/// **A `Vt` is several hundred KiB and must never be a runtime-constructed local or return
/// value.** Build it in a `static` with constant geometry and retarget it with [`Vt::reset_to`];
/// `script/stack-frame-check` catches a violation. See notes/video-terminal.md, "The grid's
/// capacity".
pub struct Vt {
    cells: [Cell; MAX_CELLS],
    /// Off-screen history: a ring of whole rows, oldest overwritten first. See
    /// [`Vt::scroll_up`]/[`Vt::scroll_down`] for the read side and [`Vt::line_feed`] for the write
    /// side (a row scrolled off the live grid's top is pushed here before being discarded).
    scrollback: [Cell; SCROLLBACK_CELLS],
    /// The ring index (in rows) the **next** pushed row will occupy. Rows already stored occupy the
    /// `sb_len` slots immediately before this one, wrapping.
    sb_tail: u32,
    /// How many scrollback rows are populated, capped at [`SCROLLBACK_ROWS`].
    sb_len: u32,
    /// How many rows scrolled back from the live view, `0..=sb_len`. `0` is the ordinary live view;
    /// nonzero means [`Vt::cell`] (and therefore [`Vt::pixel`]/[`Vt::row_bytes`]) reads from
    /// [`scrollback`](Self::scrollback) for the topmost `view_offset` display rows.
    view_offset: u32,
    cols: u32,
    rows: u32,
    col: u32,
    row: u32,
    attr: Attr,
    /// The right margin has been reached and the *next* printable wraps. See [`Vt::feed`].
    wrap_pending: bool,
    cursor_visible: bool,
    state: State,
    params: [u16; MAX_PARAMS],
    /// Bit `k` set when `params[k]` followed a `:` rather than a `;`: a sub-parameter of the one
    /// before it, as in `4:3` (a curly underline) or `58:2::255:0:0`. ECMA-48's colon form.
    subs: u16,
    nparams: usize,
    /// A parameter list this engine will not act on (private `?` sequences, an intermediate byte, or
    /// more parameters than fit). The sequence is still swallowed whole; only its effect is dropped.
    ignore: bool,
    /// How many UTF-8 continuation bytes are still expected before [`utf8_code`](Self::utf8_code) is
    /// a complete code point. `0` means the next byte starts a fresh character (or is plain ASCII).
    utf8_need: u8,
    /// The code point accumulated so far, valid only while [`utf8_need`](Self::utf8_need) is
    /// nonzero.
    utf8_code: u32,
    dirty: Option<CellRect>,
    /// How many cell rows the live grid scrolled up since the last [`Vt::take_damage`]. The scroll
    /// half of [`Damage`]; see there for why a scroll is reported as movement rather than as a
    /// whole-grid rectangle.
    scrolled: u32,
    /// Where the cursor was drawn in the picture the last [`Vt::take_damage`] reported, in
    /// **current** coordinates: each scroll moves it up a row with the grid, so the row a scrolled
    /// block landed on is re-rendered. `None` when the last reported picture drew no cursor.
    drawn: Option<(u32, u32)>,
}

impl Vt {
    /// A blank terminal of `cols` by `rows` cells, clamped to [`MAX_COLS`] and [`MAX_ROWS`].
    ///
    /// Clamped rather than refused, because the alternative in a `no_std` component is a panic it
    /// cannot report. **`const`, so a terminal can live in `.bss`**; the clamps are spelled out
    /// because `Ord::clamp` is not `const`. A runtime geometry constructs once, typically at
    /// `(1, 1)`, and calls [`Vt::reset_to`] (see the struct doc).
    pub const fn new(cols: u32, rows: u32) -> Vt {
        let cols = Self::clamp_cols(cols);
        let rows = Self::clamp_rows(rows, cols);
        Vt {
            cells: [Cell::blank(Attr::DEFAULT); MAX_CELLS],
            scrollback: [Cell::blank(Attr::DEFAULT); SCROLLBACK_CELLS],
            sb_tail: 0,
            sb_len: 0,
            view_offset: 0,
            cols,
            rows,
            col: 0,
            row: 0,
            attr: Attr::DEFAULT,
            wrap_pending: false,
            cursor_visible: true,
            state: State::Ground,
            params: [0; MAX_PARAMS],
            subs: 0,
            nparams: 0,
            ignore: false,
            utf8_need: 0,
            utf8_code: 0,
            // Nothing has painted the surface yet, so the first present covers the whole grid.
            dirty: Some(CellRect {
                col: 0,
                row: 0,
                cols,
                rows,
            }),
            scrolled: 0,
            drawn: None,
        }
    }

    const fn clamp_cols(cols: u32) -> u32 {
        if cols == 0 {
            1
        } else if cols > MAX_COLS as u32 {
            MAX_COLS as u32
        } else {
            cols
        }
    }

    /// `rows` clamped to one and to [`MAX_ROWS`], and to what fits in [`MAX_CELLS`] at `cols`.
    const fn clamp_rows(rows: u32, cols: u32) -> u32 {
        let fit = (MAX_CELLS / cols as usize) as u32;
        let most = if fit < MAX_ROWS as u32 {
            fit
        } else {
            MAX_ROWS as u32
        };
        if rows == 0 {
            1
        } else if rows > most {
            most
        } else {
            rows
        }
    }

    /// **How many scrolled-off rows this terminal keeps**: [`SCROLLBACK_ROWS`], or fewer when a
    /// row is wider than [`SCROLLBACK_CELLS`] can hold that many of.
    #[must_use]
    pub(crate) const fn scrollback_depth(&self) -> u32 {
        let fit = (SCROLLBACK_CELLS / self.cols as usize) as u32;
        if fit < SCROLLBACK_ROWS as u32 {
            fit
        } else {
            SCROLLBACK_ROWS as u32
        }
    }

    /// **Re-target this terminal to `cols` by `rows`, in place, clearing the grid and the
    /// scrollback.** What [`Vt::new`] does, field by field, so a caller with a runtime geometry
    /// never needs a `Vt`-sized value (see the struct doc).
    pub fn reset_to(&mut self, cols: u32, rows: u32) {
        let cols = Self::clamp_cols(cols);
        let rows = Self::clamp_rows(rows, cols);
        self.cells = [Cell::blank(Attr::DEFAULT); MAX_CELLS];
        self.scrollback = [Cell::blank(Attr::DEFAULT); SCROLLBACK_CELLS];
        self.sb_tail = 0;
        self.sb_len = 0;
        self.view_offset = 0;
        self.cols = cols;
        self.rows = rows;
        self.col = 0;
        self.row = 0;
        self.attr = Attr::DEFAULT;
        self.wrap_pending = false;
        self.cursor_visible = true;
        self.state = State::Ground;
        self.params = [0; MAX_PARAMS];
        self.subs = 0;
        self.nparams = 0;
        self.ignore = false;
        self.utf8_need = 0;
        self.utf8_code = 0;
        self.scrolled = 0;
        self.drawn = None;
        self.damage_all();
    }

    /// The grid's width in cells.
    pub const fn cols(&self) -> u32 {
        self.cols
    }

    /// The grid's height in cells.
    pub const fn rows(&self) -> u32 {
        self.rows
    }

    /// The grid's width in pixels.
    pub const fn width(&self) -> u32 {
        self.cols * crate::CELL_W
    }

    /// The grid's height in pixels.
    pub const fn height(&self) -> u32 {
        self.rows * crate::CELL_H
    }

    /// Where the cursor is, as `(col, row)`.
    pub const fn cursor(&self) -> (u32, u32) {
        (self.col, self.row)
    }

    /// Show or hide the block cursor. A hidden cursor is what a terminal that is only *printing*
    /// wants, and it is what makes a picture independent of where the last write left off.
    pub fn set_cursor_visible(&mut self, visible: bool) {
        if self.cursor_visible != visible {
            self.cursor_visible = visible;
            self.damage_cell(self.col, self.row);
        }
    }

    /// **The cell at `(col, row)` of what is currently displayed** (the live grid, or scrollback if
    /// [`Vt::view_offset`] is nonzero). Out of range gives a default blank rather than a panic,
    /// because the callers are pixel loops.
    ///
    /// `row` is a **display** row: `0` is the top of whatever is shown. Every reader (`pixel`,
    /// `row_bytes`) goes through here, so the scrollback view is uniform.
    pub fn cell(&self, col: u32, row: u32) -> Cell {
        if col >= self.cols || row >= self.rows {
            return Cell::default();
        }
        if row < self.view_offset {
            // Display row 0 is the oldest row shown (`age = view_offset - 1`); the row just above
            // the live grid is `age = 0`.
            let age = self.view_offset - 1 - row;
            return self.scrollback_cell(col, age);
        }
        let live_row = row - self.view_offset;
        self.cells[(live_row * self.cols + col) as usize]
    }

    /// The scrollback cell `age` rows above the live grid's top (`age = 0` is the most recently
    /// scrolled-off row), at column `col`. `age >= sb_len` (asked for history that was never kept,
    /// or never existed) gives a default blank, the same total-function discipline [`Vt::cell`]
    /// uses for a coordinate outside the grid.
    fn scrollback_cell(&self, col: u32, age: u32) -> Cell {
        if age >= self.sb_len {
            return Cell::default();
        }
        // `sb_tail` is the slot the next push uses, so age 0 is one slot behind it.
        let capacity = self.scrollback_depth();
        let ring_row = (self.sb_tail + capacity - 1 - age) % capacity;
        self.scrollback[(ring_row * self.cols + col) as usize]
    }

    /// **The pixel at `(x, y)` of the terminal's surface.** The whole of rendering, as a pure
    /// function of the grid and the scroll position.
    ///
    /// The cursor is a **block**, drawn by swapping the cell's two colors rather than as an
    /// overlay, so a test that predicts the screen predicts the cursor too. It is not drawn while
    /// scrolled back. The `col < self.cols` guard is redundant and kept for the reader; see
    /// notes/video-terminal.md, "Rendering" and "Mutants the engine cannot kill".
    pub fn pixel(&self, x: u32, y: u32) -> u32 {
        let (col, row) = (x / crate::CELL_W, y / crate::CELL_H);
        let cell = self.cell(col, row);
        let mut attr = cell.attr;
        if self.view_offset == 0
            && self.cursor_visible
            && col == self.col
            && row == self.row
            && col < self.cols
        {
            attr = attr.with(REVERSE, !attr.reverse());
        }
        let (fg, bg) = attr.colours();
        let (gx, gy) = (x % crate::CELL_W, y % crate::CELL_H);
        // Line renditions ink the whole cell width, gutters included, and patterned underlines
        // read the screen's `x`, so both run unbroken across cells.
        if underline_ink(attr.underline(), x, gy) {
            return attr.underline_color(fg);
        }
        if attr.is_struck_through() && gy == STRIKETHROUGH_ROW {
            return fg;
        }
        let face = match (attr.is_bold(), attr.is_italic()) {
            (false, false) => &glyph_atlas::REGULAR,
            (true, false) => &glyph_atlas::BOLD,
            (false, true) => &glyph_atlas::OBLIQUE,
            (true, true) => &glyph_atlas::BOLD_OBLIQUE,
        };
        glyph_atlas::blend(fg, bg, face.coverage(cell.ch, gx, gy))
    }

    /// What has changed since the last [`Vt::take_damage`]: the scroll half as well as the box.
    /// `Some` whenever anything changed, **including a scroll that changed no cell**: a painter
    /// polling this has to learn its picture moved, or the surface drifts a row behind the grid
    /// forever.
    pub const fn damage(&self) -> Option<Damage> {
        match (self.dirty, self.scrolled) {
            (None, 0) => None,
            (Some(rect), scrolled) => Some(Damage { scrolled, rect }),
            // A scroll that changed no cell still changed the picture: an empty `rect` and the
            // movement on the other half of the report.
            (None, scrolled) => Some(Damage {
                scrolled,
                rect: CellRect::EMPTY,
            }),
        }
    }

    /// **What changed since the last call, clearing the record**, as a [`Damage`]. Taking it also
    /// records where the cursor was drawn ([`Vt::drawn`]).
    ///
    /// # Examples
    ///
    /// Bytes in, a grid out, and the rectangle that changed, taken once:
    ///
    /// ```
    /// use video_terminal::Vt;
    ///
    /// let mut vt = Vt::new(8, 4);
    /// vt.feed(b"hello\r\nworld");
    ///
    /// let mut row = [0u8; 8];
    /// let n = vt.row_bytes(0, &mut row);
    /// assert_eq!(&row[..n], b"hello   "); // the rest of the row is blanks, not garbage
    /// vt.row_bytes(1, &mut row);
    /// assert_eq!(&row[..5], b"world");
    /// assert_eq!(vt.cursor(), (5, 1));
    ///
    /// // Damage is in cells and is taken, not read: the caller repaints exactly what changed and the
    /// // record clears. That is the whole reason the engine reports a rectangle instead of "redraw".
    /// let dirty = vt.take_damage().expect("something changed");
    /// assert!(dirty.rect.rows >= 2, "two rows were written to");
    /// assert_eq!(dirty.scrolled, 0, "nothing scrolled");
    /// assert_eq!(vt.take_damage(), None); // taken once
    /// ```
    pub fn take_damage(&mut self) -> Option<Damage> {
        // Dirty the cursor cells only on a real difference, so an engine at rest reports `None`.
        let drawn_now = if self.cursor_visible {
            Some((self.col, self.row))
        } else {
            None
        };
        if drawn_now != self.drawn {
            if let Some((c, r)) = self.drawn {
                self.damage_cell(c, r);
            }
            if let Some((c, r)) = drawn_now {
                self.damage_cell(c, r);
            }
            self.drawn = drawn_now;
        }
        let scrolled = core::mem::take(&mut self.scrolled);
        // `put`'s comparison models a move of one row at a time; a painter moves `scrolled` rows
        // at once, so past one the exposed bottom rows must be rendered whole.
        if scrolled > 1 {
            self.dirty = Some(
                CellRect {
                    col: 0,
                    row: self.rows - scrolled,
                    cols: self.cols,
                    rows: scrolled,
                }
                .union_with(self.dirty),
            );
        }
        let rect = match self.dirty.take() {
            Some(rect) => rect,
            // A scroll with nothing to render is still a report: the painter's pixels moved.
            None if scrolled > 0 => CellRect::EMPTY,
            None => return None,
        };
        Some(Damage { scrolled, rect })
    }

    /// Copy row `row`'s characters into `out` as bytes, returning how many were written. For tests
    /// and for a caller that wants the text; a non-ASCII `char` becomes `?`.
    ///
    /// # Examples
    ///
    /// `CSI 2J` clears the screen and `CSI H` homes the cursor:
    ///
    /// ```
    /// use video_terminal::Vt;
    ///
    /// let mut vt = Vt::new(8, 4);
    /// vt.feed(b"one\r\ntwo\r\n");
    ///
    /// // CSI H homes the cursor; CSI 2J clears the screen.
    /// vt.feed(b"\x1b[2J\x1b[H");
    /// assert_eq!(vt.cursor(), (0, 0));
    /// let mut row = [0u8; 8];
    /// let n = vt.row_bytes(0, &mut row);
    /// assert_eq!(&row[..n], b"        ");
    /// ```
    pub fn row_bytes(&self, row: u32, out: &mut [u8]) -> usize {
        let n = (self.cols as usize).min(out.len());
        for (i, o) in out.iter_mut().enumerate().take(n) {
            let ch = self.cell(i as u32, row).ch;
            *o = if ch.is_ascii() { ch as u8 } else { b'?' };
        }
        n
    }

    /// How many rows scrolled back from the live view. `0` is the ordinary live view.
    pub const fn view_offset(&self) -> u32 {
        self.view_offset
    }

    /// How many rows of history are available to scroll into.
    pub const fn scrollback_len(&self) -> u32 {
        self.sb_len
    }

    /// **Scroll `n` rows further into history**, clamped at [`Vt::scrollback_len`]. Marks the whole
    /// grid dirty, because scrolling changes every displayed row at once (the same reason the
    /// ordinary scroll on `LF` at the bottom row does).
    pub fn scroll_up(&mut self, n: u32) {
        let new_offset = self.view_offset.saturating_add(n).min(self.sb_len);
        if new_offset != self.view_offset {
            self.view_offset = new_offset;
            self.damage_all();
        }
    }

    /// **Scroll `n` rows back toward the live view**, clamped at `0`. See [`Vt::scroll_up`].
    pub fn scroll_down(&mut self, n: u32) {
        let new_offset = self.view_offset.saturating_sub(n);
        if new_offset != self.view_offset {
            self.view_offset = new_offset;
            self.damage_all();
        }
    }

    /// **Push bytes through the parser.** New output snaps a scrolled-back view to live.
    ///
    /// Writing into the last column leaves the cursor there with a wrap pending, which the next
    /// printable byte does first; cursor motion, `CR` and `LF` cancel it. See
    /// notes/video-terminal.md, "Printing and the cursor".
    ///
    /// # Examples
    ///
    /// A line that exactly fills the width does not wrap until the next printable byte:
    ///
    /// ```
    /// use video_terminal::Vt;
    ///
    /// let mut vt = Vt::new(4, 4);
    /// vt.feed(b"abcd"); // exactly fills row 0
    ///
    /// // The cursor stays on the last cell it wrote, with a wrap pending. It has NOT moved to row 1.
    /// assert_eq!(vt.cursor(), (3, 0));
    ///
    /// // A CR arriving now finds the cursor on row 0, which is where a line discipline expects it. If
    /// // the wrap had already happened it would be a row too low.
    /// vt.feed(b"\r");
    /// assert_eq!(vt.cursor(), (0, 0));
    ///
    /// // And when the next printable byte does arrive without an intervening CR, it wraps first.
    /// let mut vt = Vt::new(4, 4);
    /// vt.feed(b"abcde");
    /// assert_eq!(vt.cursor(), (1, 1));
    /// let mut row = [0u8; 4];
    /// vt.row_bytes(1, &mut row);
    /// assert_eq!(row[0], b'e');
    /// ```
    pub fn feed(&mut self, bytes: &[u8]) {
        if self.view_offset != 0 {
            self.view_offset = 0;
            self.damage_all();
        }
        let before = (self.col, self.row);
        for &b in bytes {
            self.byte(b);
        }
        if (self.col, self.row) != before && self.cursor_visible {
            // Once per `feed`, not per byte, so a long print dirties no trail of cells.
            self.damage_cell(before.0, before.1);
            self.damage_cell(self.col, self.row);
        }
    }

    fn byte(&mut self, b: u8) {
        match self.state {
            State::Ground => self.ground(b),
            State::Esc => self.esc(b),
            State::Csi => self.csi(b),
            State::Str => match b {
                0x07 => self.state = State::Ground, // BEL, the xterm terminator
                0x1b => self.state = State::StrEsc,
                _ => {}
            },
            State::StrEsc => {
                // `ESC \` (ST) ends the string; an ESC followed by anything else is part of it.
                self.state = if b == b'\\' {
                    State::Ground
                } else {
                    State::Str
                };
            }
        }
    }

    /// The Unicode replacement character, drawn for an invalid or incomplete UTF-8 sequence, as
    /// `String::from_utf8_lossy` does.
    const REPLACEMENT: char = '\u{fffd}';

    fn ground(&mut self, b: u8) {
        // UTF-8 first: every control code and introducer is below 0x80, so a high-bit byte can
        // only belong to a multi-byte character. See notes/video-terminal.md, "UTF-8".
        if self.utf8_need > 0 {
            if b & 0xc0 == 0x80 {
                // A continuation byte. The operands of `|` share no bit, so `^` would agree.
                self.utf8_code = (self.utf8_code << 6) | (b & 0x3f) as u32;
                self.utf8_need -= 1;
                if self.utf8_need == 0 {
                    let ch = char::from_u32(self.utf8_code).unwrap_or(Self::REPLACEMENT);
                    self.print(ch);
                }
                return;
            }
            // Truncated: draw the replacement and reprocess `b` rather than eating it.
            self.utf8_need = 0;
            self.print(Self::REPLACEMENT);
        }
        match b {
            0x1b => {
                self.state = State::Esc;
            }
            b'\r' => {
                self.col = 0;
                self.wrap_pending = false;
            }
            b'\n' => {
                self.wrap_pending = false;
                self.line_feed();
            }
            0x08 => {
                // No wrap back to the previous row: a line discipline only backs up within a line.
                self.wrap_pending = false;
                self.col = self.col.saturating_sub(1);
            }
            b'\t' => {
                self.wrap_pending = false;
                // Every eight columns, stopping at the last one rather than wrapping.
                self.col = ((self.col / 8 + 1) * 8).min(self.cols - 1);
            }
            0x07 => {} // BEL: there is no bell on a framebuffer, and a visual bell is policy
            0x00..=0x1f | 0x7f => {} // every other control code: consumed, never drawn
            0x20..=0x7e => self.print(b as char), // plain printable ASCII, the common case
            // RFC 3629's lead bytes; 0x80..=0xc1 and 0xf5..=0xff can never start a sequence.
            0xc2..=0xdf => {
                self.utf8_need = 1;
                self.utf8_code = (b & 0x1f) as u32;
            }
            0xe0..=0xef => {
                self.utf8_need = 2;
                self.utf8_code = (b & 0x0f) as u32;
            }
            0xf0..=0xf4 => {
                self.utf8_need = 3;
                self.utf8_code = (b & 0x07) as u32;
            }
            0x80..=0xc1 | 0xf5..=0xff => self.print(Self::REPLACEMENT),
        }
    }

    fn print(&mut self, ch: char) {
        if self.wrap_pending {
            self.wrap_pending = false;
            self.col = 0;
            self.line_feed();
        }
        let (col, row) = (self.col, self.row);
        self.put(
            col,
            row,
            Cell {
                ch,
                attr: self.attr,
            },
        );
        if self.col + 1 >= self.cols {
            self.wrap_pending = true;
        } else {
            self.col += 1;
        }
    }

    /// Down one row, scrolling the grid up if that would fall off the bottom.
    fn line_feed(&mut self) {
        if self.row + 1 < self.rows {
            self.row += 1;
            return;
        }
        // The top row goes to scrollback before the shift overwrites it. `put` dirties the
        // blanked bottom row; `scroll_damage` reports the movement.
        let cols = self.cols as usize;
        self.push_scrollback_row(0);
        let used = cols * self.rows as usize;
        self.cells.copy_within(cols..used, 0);
        // The box of what changed moves up with its cells, before the bottom row is added.
        self.dirty = self.dirty.and_then(|d| match d.row {
            0 if d.rows <= 1 => None,
            0 => Some(CellRect {
                rows: d.rows - 1,
                ..d
            }),
            _ => Some(CellRect {
                row: d.row - 1,
                ..d
            }),
        });
        for c in 0..self.cols {
            self.put(c, self.rows - 1, Cell::blank(self.attr));
        }
        self.scroll_damage();
    }

    /// Copy live row `row` into the scrollback ring's next slot, as the newest entry.
    ///
    /// Called only from [`Vt::line_feed`] with `row` `0`, after [`Vt::feed`] has reset
    /// `view_offset`, so it never adjusts the view. See notes/video-terminal.md, "Mutants the
    /// engine cannot kill".
    fn push_scrollback_row(&mut self, row: u32) {
        let cols = self.cols;
        let capacity = self.scrollback_depth();
        let ring_row = self.sb_tail;
        for c in 0..cols {
            self.scrollback[(ring_row * cols + c) as usize] = self.cells[(row * cols + c) as usize];
        }
        self.sb_tail = (self.sb_tail + 1) % capacity;
        if self.sb_len < capacity {
            self.sb_len += 1;
        }
    }

    fn esc(&mut self, b: u8) {
        self.state = State::Ground;
        match b {
            b'[' => {
                self.state = State::Csi;
                self.params = [0; MAX_PARAMS];
                self.subs = 0;
                self.nparams = 0;
                self.ignore = false;
            }
            b'c' => self.reset(),
            // OSC, DCS, SOS/PM/APC: arbitrary text, consumed rather than printed.
            b']' | b'P' | b'X' | b'^' | b'_' => self.state = State::Str,
            // An unknown two-byte escape consumes its final byte too, or it prints as a letter.
            _ => {}
        }
    }

    fn csi(&mut self, b: u8) {
        match b {
            b'0'..=b'9' => {
                if self.nparams == 0 {
                    self.nparams = 1;
                }
                if self.nparams <= MAX_PARAMS {
                    let p = &mut self.params[self.nparams - 1];
                    *p = p.saturating_mul(10).saturating_add((b - b'0') as u16);
                }
            }
            // `:` opens a sub-parameter. A separator before any digit opens an empty first
            // parameter too (`CSI ;5H`, `CSI :2m`).
            b';' | b':' => {
                if self.nparams == 0 {
                    self.nparams = 1;
                }
                self.nparams += 1;
                if self.nparams > MAX_PARAMS {
                    self.ignore = true;
                    self.nparams = MAX_PARAMS;
                } else if b == b':' {
                    self.subs |= 1 << (self.nparams - 1);
                }
            }
            // Private-use introducers and intermediate bytes: a parameter list that means something
            // else. The catch-all does the same; the arm names the family.
            0x20..=0x2f | 0x3c..=0x3f => self.ignore = true,
            0x40..=0x7e => {
                let ignore = self.ignore;
                self.state = State::Ground;
                if !ignore {
                    self.csi_final(b);
                }
            }
            _ => {
                // A control code inside a sequence is dropped with it: fail toward drawing nothing.
                self.ignore = true;
            }
        }
    }

    /// The first parameter, defaulting to `d` when absent or zero (the ANSI convention: `CSI 0 A`
    /// and `CSI A` both mean one row).
    fn param(&self, i: usize, d: u32) -> u32 {
        match self.params.get(i) {
            Some(&0) | None => d,
            Some(&p) => p as u32,
        }
    }

    fn csi_final(&mut self, b: u8) {
        match b {
            b'A' => {
                self.wrap_pending = false;
                self.row = self.row.saturating_sub(self.param(0, 1));
            }
            b'B' => {
                self.wrap_pending = false;
                self.row = (self.row + self.param(0, 1)).min(self.rows - 1);
            }
            b'C' => {
                self.wrap_pending = false;
                self.col = (self.col + self.param(0, 1)).min(self.cols - 1);
            }
            b'D' => {
                self.wrap_pending = false;
                self.col = self.col.saturating_sub(self.param(0, 1));
            }
            // CUP: parameters are one-based on the wire and zero-based here.
            b'H' | b'f' => {
                self.wrap_pending = false;
                self.row = (self.param(0, 1) - 1).min(self.rows - 1);
                self.col = (self.param(1, 1) - 1).min(self.cols - 1);
            }
            b'J' => self.erase_display(self.param(0, 0)),
            b'K' => self.erase_line(self.param(0, 0)),
            b'm' => self.sgr(),
            // Every other final byte, device reports included: consumed, and nothing happens.
            _ => {}
        }
    }

    /// `CSI n J`: 0 erases from the cursor to the end of the screen, 1 to its start, 2 all of it.
    /// `CSI 2 J` does **not** move the cursor. `to > from` always holds, since `cols` and `rows`
    /// are at least one and the cursor is inside the grid: mode 0's `end - here >= cols - col >= 1`,
    /// mode 1's `here + 1 >= 1`, the default's `rows * cols >= 1`. So `>` relaxed to `>=` survives.
    fn erase_display(&mut self, mode: u32) {
        let here = self.row * self.cols + self.col;
        let end = self.rows * self.cols;
        let (from, to) = match mode {
            0 => (here, end),
            1 => (0, here + 1),
            _ => (0, end),
        };
        for i in from..to.min(end) {
            self.cells[i as usize] = Cell::blank(self.attr);
        }
        if to > from {
            self.damage_all();
        }
    }

    /// `CSI n K`: 0 erases from the cursor to the end of the line, 1 to its start, 2 the whole line.
    fn erase_line(&mut self, mode: u32) {
        let (from, to) = match mode {
            0 => (self.col, self.cols),
            1 => (0, self.col + 1),
            _ => (0, self.cols),
        };
        for c in from..to.min(self.cols) {
            let row = self.row;
            self.put(c, row, Cell::blank(self.attr));
        }
    }

    /// `CSI ... m`: the rendition. An empty parameter list is `CSI 0 m`, a reset, which is the one
    /// place the "absent means zero" default differs from the cursor sequences' "absent means one".
    ///
    /// The extended colors (`38`, `48`, `58`) consume the parameters they name, and a malformed one
    /// ends the sequence's effect there, as xterm does. The colon form (`4:3`, `38:2::r:g:b`) is
    /// read too, and unused sub-parameters are skipped. See notes/video-terminal.md, "SGR".
    fn sgr(&mut self) {
        let n = self.nparams.max(1);
        let mut i = 0;
        // Every pass consumes at least one parameter, so the bound ends the walk by construction.
        for _ in 0..MAX_PARAMS {
            if i >= n {
                break;
            }
            let p = self.params[i];
            i += 1;
            // The sub-parameters that follow `p`, at `i..i + subs`.
            let mut subs = 0;
            for k in i..n {
                if self.subs & 1 << k == 0 {
                    break;
                }
                subs += 1;
            }
            let a = self.attr;
            self.attr = match p {
                0 => Attr::DEFAULT,
                1 => a.with(BOLD, true),
                2 => a.with(DIM, true),
                3 => a.with(ITALIC, true),
                4 if subs == 0 => a.with_underline(UNDERLINE_SINGLE),
                4 => match self.params[i] {
                    s @ 0..=5 => a.with_underline(s as u8),
                    _ => a,
                },
                21 => a.with_underline(UNDERLINE_DOUBLE),
                7 => a.with(REVERSE, true),
                8 => a.with(INVISIBLE, true),
                9 => a.with(STRIKETHROUGH, true),
                22 => a.with(BOLD, false).with(DIM, false),
                23 => a.with(ITALIC, false),
                24 => a.with_underline(UNDERLINE_NONE),
                27 => a.with(REVERSE, false),
                28 => a.with(INVISIBLE, false),
                29 => a.with(STRIKETHROUGH, false),
                30..=37 => Attr {
                    fg: Colour::Indexed(p as u8 - 30),
                    ..a
                },
                39 => Attr {
                    fg: Colour::Indexed(DEFAULT_FG),
                    ..a
                },
                40..=47 => Attr {
                    bg: Colour::Indexed(p as u8 - 40),
                    ..a
                },
                49 => Attr {
                    bg: Colour::Indexed(DEFAULT_BG),
                    ..a
                },
                59 => Attr { ul: None, ..a },
                90..=97 => Attr {
                    fg: Colour::Indexed(p as u8 - 90 + 8),
                    ..a
                },
                100..=107 => Attr {
                    bg: Colour::Indexed(p as u8 - 100 + 8),
                    ..a
                },
                38 | 48 | 58 => {
                    let found = if subs > 0 {
                        self.colon_color(i, subs)
                    } else {
                        self.extended_colour(i, n)
                    };
                    let Some((color, used)) = found else {
                        return;
                    };
                    i += used;
                    match p {
                        38 => Attr { fg: color, ..a },
                        48 => Attr { bg: color, ..a },
                        _ => Attr {
                            ul: Some(color),
                            ..a
                        },
                    }
                }
                // Blink, overline and fonts are dropped: no clock, and the text still draws.
                _ => a,
            };
            // Whatever sub-parameters `p` had and did not consume are its own, not attributes.
            if !matches!(p, 38 | 48 | 58) {
                i += subs;
            }
        }
    }

    /// The color named by the parameters from `i` onward, after a `38`, `48` or `58`, and how
    /// many parameters it used. `None` for a malformed one; see [`Vt::sgr`].
    fn extended_colour(&self, i: usize, n: usize) -> Option<(Colour, usize)> {
        let component = |k: usize| -> Option<u8> {
            if i + k < n {
                u8::try_from(self.params[i + k]).ok()
            } else {
                None
            }
        };
        match self.params.get(i).filter(|_| i < n)? {
            5 => Some((Colour::Indexed(component(1)?), 2)),
            2 => Some((Colour::Rgb(component(1)?, component(2)?, component(3)?), 4)),
            _ => None,
        }
    }

    /// The colon form of [`Vt::extended_colour`]: the `subs` sub-parameters at `i`, all of which
    /// it uses. `2:r:g:b`, or `2:cs:r:g:b` with the color-space slot (usually empty) ignored, or
    /// `5:n`.
    fn colon_color(&self, i: usize, subs: usize) -> Option<(Colour, usize)> {
        let at = |k: usize| u8::try_from(self.params[i + k]).ok();
        let color = match (self.params[i], subs) {
            (5, 2) => Colour::Indexed(at(1)?),
            (2, 4) => Colour::Rgb(at(1)?, at(2)?, at(3)?),
            (2, 5) => Colour::Rgb(at(2)?, at(3)?, at(4)?),
            _ => return None,
        };
        Some((color, subs))
    }

    /// `ESC c`: back to power-on. Clears the grid in the *default* rendition rather than the current
    /// one, which is the difference between a reset and an erase.
    fn reset(&mut self) {
        self.attr = Attr::DEFAULT;
        self.cells = [Cell::blank(Attr::DEFAULT); MAX_CELLS];
        self.col = 0;
        self.row = 0;
        self.wrap_pending = false;
        self.damage_all();
    }

    fn put(&mut self, col: u32, row: u32, cell: Cell) {
        if col >= self.cols || row >= self.rows {
            return;
        }
        let at = (row * self.cols + col) as usize;
        if self.cells[at] != cell {
            self.cells[at] = cell;
            self.damage_cell(col, row);
        }
    }

    fn damage_cell(&mut self, col: u32, row: u32) {
        if col >= self.cols || row >= self.rows {
            return;
        }
        let r = CellRect::cell(col, row);
        self.dirty = Some(match self.dirty {
            Some(d) => d.union(r),
            None => r,
        });
    }

    fn damage_all(&mut self) {
        self.dirty = Some(CellRect {
            col: 0,
            row: 0,
            cols: self.cols,
            rows: self.rows,
        });
        // A full box subsumes any scroll, which keeps `Damage::scrolled < rows` at every take.
        self.scrolled = 0;
    }

    /// **A scroll's half of the damage report**: the grid moved up a row, which is neither a
    /// changed cell nor a rectangle. See [`Damage`] for the invariant the painter hangs on this.
    fn scroll_damage(&mut self) {
        // The drawn block moved up with its pixels; re-render the row it landed on.
        if let Some((c, r)) = self.drawn {
            self.drawn = if r > 0 { Some((c, r - 1)) } else { None };
            if r > 0 {
                self.damage_cell(c, r - 1);
            }
        }
        self.scrolled += 1;
        if self.scrolled >= self.rows {
            // `rows` scrolls replaced everything, so report content, not movement.
            self.damage_all();
        }
    }
}

const fn state_word(s: State) -> u32 {
    match s {
        State::Ground => 0,
        State::Esc => 1,
        State::Csi => 2,
        State::Str => 3,
        State::StrEsc => 4,
    }
}

const fn word_state(w: u32) -> Option<State> {
    match w {
        0 => Some(State::Ground),
        1 => Some(State::Esc),
        2 => Some(State::Csi),
        3 => Some(State::Str),
        4 => Some(State::StrEsc),
        _ => None,
    }
}

/// **Whether an underline in `style` inks the pixel at screen column `x`, cell row `gy`.** Every
/// style starts on the font's underline row. Double adds the row two below it, curly is a wave
/// eight pixels long and three rows deep, dotted is two on and two off, and dashed five on and
/// three off.
const fn underline_ink(style: u8, x: u32, gy: u32) -> bool {
    match style {
        UNDERLINE_SINGLE => gy == UNDERLINE_ROW,
        UNDERLINE_DOUBLE => gy == UNDERLINE_ROW || gy == UNDERLINE_ROW + 2,
        UNDERLINE_CURLY => {
            let t = x % 8;
            let height = if t < 4 { t } else { 8 - t };
            gy == UNDERLINE_ROW + height / 2
        }
        UNDERLINE_DOTTED => gy == UNDERLINE_ROW && x / 2 % 2 == 0,
        UNDERLINE_DASHED => gy == UNDERLINE_ROW && x % 8 < 5,
        _ => false,
    }
}

/// **The state handoff** (`crate::handoff` has the layout and why it is one).
impl Vt {
    /// How many bytes [`Vt::write_handoff`] writes for this terminal as it stands.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23 (a capability-routed
    /// component OS with live replacement).
    pub fn handoff_len(&self) -> usize {
        let cells = (self.cols * self.rows + self.sb_len * self.cols) as usize;
        4 * (HEADER_WORDS + CELL_WORDS * cells)
    }

    /// **Write this terminal's whole state into `out`**, and return how many bytes that took, or
    /// `None` when `out` is shorter than [`Vt::handoff_len`]. A blob that does not fit is not
    /// truncated, because a truncated blob that read back would be state silently lost.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
    pub fn write_handoff(&self, out: &mut [u8]) -> Option<usize> {
        let len = self.handoff_len();
        if out.len() < len {
            return None;
        }
        let mut flags = 0;
        for (on, bit) in [
            (self.wrap_pending, FLAG_WRAP_PENDING),
            (self.cursor_visible, FLAG_CURSOR_VISIBLE),
            (self.ignore, FLAG_IGNORE),
        ] {
            if on {
                flags |= bit;
            }
        }
        let header = [
            MAGIC,
            LAYOUT,
            self.cols,
            self.rows,
            self.col,
            self.row,
            self.sb_len,
            self.view_offset,
            flags,
            state_word(self.state),
            self.nparams as u32,
            u32::from(self.utf8_need),
            self.utf8_code,
            0,
            color_word(self.attr.fg),
            color_word(self.attr.bg),
            self.attr.flags as u32,
            underline_word(self.attr.ul),
            self.attr.underline as u32,
            u32::from(self.subs),
        ];
        for (i, w) in header.iter().enumerate() {
            put_word(out, i, *w);
        }
        for (i, p) in self.params.iter().enumerate() {
            put_word(out, header.len() + i, u32::from(*p));
        }
        let mut at = HEADER_WORDS;
        let live = (self.cols * self.rows) as usize;
        for c in &self.cells[..live] {
            for w in cell_words(*c) {
                put_word(out, at, w);
                at += 1;
            }
        }
        // Oldest first, so the reader rebuilds the ring from slot zero whatever `sb_tail` was.
        for age in (0..self.sb_len).rev() {
            for col in 0..self.cols {
                for w in cell_words(self.scrollback_cell(col, age)) {
                    put_word(out, at, w);
                    at += 1;
                }
            }
        }
        Some(len)
    }

    /// **Become the terminal a blob describes.** The blob's grid must be this terminal's own
    /// shape, the one it negotiated with its screen before absorbing. The whole blob is checked
    /// first, so on any [`Refusal`] this terminal is as it was.
    ///
    /// Everything is marked as damage: the caller repaints its whole surface from what it
    /// absorbed.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
    pub fn absorb_handoff(&mut self, blob: &[u8]) -> Result<(), Refusal> {
        let w = |i| word_at(blob, i).ok_or(Refusal::Malformed);
        if w(0).ok() != Some(MAGIC) {
            return Err(Refusal::NoBlob);
        }
        // Layout 1 (milestone 23's) is still read: a replacement reads its predecessor's blob.
        let layout = w(1)?;
        let (header_words, cell_words_n, params_at) = match layout {
            1 => (HEADER_WORDS_1, CELL_WORDS_1, 17),
            LAYOUT => (HEADER_WORDS, CELL_WORDS, 20),
            other => return Err(Refusal::Layout(other)),
        };
        if (w(2)?, w(3)?) != (self.cols, self.rows) {
            return Err(Refusal::Geometry);
        }
        let (col, row, sb_len, view_offset) = (w(4)?, w(5)?, w(6)?, w(7)?);
        let flags = w(8)?;
        let state = word_state(w(9)?).ok_or(Refusal::Malformed)?;
        let nparams = w(10)? as usize;
        let utf8_need = w(11)?;
        let utf8_code = w(12)?;
        let (attr, subs) = if layout == 1 {
            (words_attr_1(w(14)?, w(15)?, w(16)?), Some(0))
        } else {
            (
                words_attr(w(14)?, w(15)?, w(16)?, w(17)?, w(18)?),
                u16::try_from(w(19)?).ok(),
            )
        };
        let (attr, subs) = (
            attr.ok_or(Refusal::Malformed)?,
            subs.ok_or(Refusal::Malformed)?,
        );
        let mut params = [0u16; MAX_PARAMS];
        for (i, p) in params.iter_mut().enumerate() {
            *p = u16::try_from(w(params_at + i)?).map_err(|_| Refusal::Malformed)?;
        }
        if col >= self.cols
            || row >= self.rows
            || sb_len > self.scrollback_depth()
            || view_offset > sb_len
            || flags & !(FLAG_WRAP_PENDING | FLAG_CURSOR_VISIBLE | FLAG_IGNORE) != 0
            || nparams > MAX_PARAMS
            || utf8_need > 3
        {
            return Err(Refusal::Malformed);
        }
        let live = (self.cols * self.rows) as usize;
        let history = (sb_len * self.cols) as usize;
        let cell = |k: usize| -> Result<Cell, Refusal> {
            let at = header_words + cell_words_n * k;
            let found = if layout == 1 {
                words_cell_1([w(at)?, w(at + 1)?, w(at + 2)?])
            } else {
                words_cell([w(at)?, w(at + 1)?, w(at + 2)?, w(at + 3)?])
            };
            found.ok_or(Refusal::Malformed)
        };
        // Every cell is checked before any is written, so a refusal leaves the terminal whole.
        for k in 0..live + history {
            cell(k)?;
        }
        for (k, c) in self.cells[..live].iter_mut().enumerate() {
            *c = cell(k)?;
        }
        for (k, c) in self.scrollback[..history].iter_mut().enumerate() {
            *c = cell(live + k)?;
        }
        self.sb_len = sb_len;
        self.sb_tail = sb_len % self.scrollback_depth();
        self.view_offset = view_offset;
        self.col = col;
        self.row = row;
        self.attr = attr;
        self.wrap_pending = flags & FLAG_WRAP_PENDING != 0;
        self.cursor_visible = flags & FLAG_CURSOR_VISIBLE != 0;
        self.ignore = flags & FLAG_IGNORE != 0;
        self.state = state;
        self.params = params;
        self.subs = subs;
        self.nparams = nparams;
        self.utf8_need = utf8_need as u8;
        self.utf8_code = utf8_code;
        self.scrolled = 0;
        self.drawn = None;
        self.damage_all();
        Ok(())
    }
}

#[cfg(test)]
mod tests;
