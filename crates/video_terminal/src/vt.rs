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

/// The parser's state. Deliberately few: a VT's full state machine has a dozen states, and most of
/// the extra ones exist to distinguish sequences this engine swallows anyway.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Printing.
    Ground,
    /// `ESC` has been seen.
    Esc,
    /// Inside `ESC [`, accumulating parameters.
    Csi,
    /// Inside a **string** sequence (`ESC ]`, the operating-system command, and its relatives).
    ///
    /// This state earns its place: a string sequence carries arbitrary text, so a parser without it
    /// prints the window title onto the screen. That is not hypothetical, it is what this engine did
    /// before the test caught it, and it is the reason the test feeds a title-setting sequence.
    Str,
    /// Inside a string sequence, having just seen `ESC`: a `\` ends the string (ST), anything else
    /// is part of it.
    StrEsc,
}

/// How many numeric parameters a CSI sequence may carry. **Sixteen since milestone 142's truecolour
/// pass**, up from four: one 24-bit color is five parameters (`38;2;r;g;b`), a foreground and a
/// background together are ten, and a highlighter commonly adds a reset and a rendition or two in
/// the same sequence. A seventeenth is swallowed rather than growing the array, because a parameter
/// list that long belongs to a sequence this engine does not implement.
pub(crate) const MAX_PARAMS: usize = 16;

/// **A terminal's live grid, plus its off-screen history.**
///
/// Construct with [`Vt::new`], push bytes with [`Vt::feed`], read pixels with [`Vt::pixel`], and ask
/// what changed with [`Vt::take_damage`]. Scroll into history with [`Vt::scroll_up`]/
/// [`Vt::scroll_down`] (milestone 142 increment 2).
///
/// **Large enough now that a `Vt` must never be a runtime-constructed local or return value**,
/// which is worth stating as a rule next to the type rather than only in the notes. At [`MAX_COLS`]
/// x [`MAX_ROWS`] plus [`SCROLLBACK_CELLS`] this is several hundred KiB, comfortably past a kernel
/// thread stack (24 KiB) and past a user process's own stack page (4 KiB). `Vt::new` stays `const
/// fn` for exactly this reason: called with compile-time-constant `cols`/`rows` (as every
/// `static mut ... = Vt::new(...)` in this tree does), the value is built by the compiler and placed
/// directly in `.bss`, never on a stack. A caller with a **runtime** geometry (`display_terminal`
/// negotiating with its driver, a kernel test reading dimensions off a reply) must construct once,
/// at a fixed or upper-bound size, and then call [`Vt::reset_to`] to retarget it in place; it must
/// never write `let vt = Vt::new(runtime_cols, runtime_rows);` or `*existing = Vt::new(...);`; both
/// are calls to a `const fn` outside a const context, which run as ordinary functions and would
/// require an ordinary, Vt-sized return value to exist somewhere at runtime. `script/stack-frame-
/// check` is the gate that would catch a violation, and this doc comment is why a reader should not
/// need it to.
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
    /// **current** coordinates: each scroll moves it up a row with the grid. This is what makes
    /// the cursor honest under the scroll fast path, because the cursor is the one thing on the
    /// screen whose pixels do not move with its cell: a scroll carries the *rendered* block up a
    /// row while the cursor itself stays on the bottom row, so the row the block landed on has to
    /// be re-rendered, and this field is how the engine knows which row that is. `None` when the
    /// last reported picture drew no cursor (hidden, or never taken).
    drawn: Option<(u32, u32)>,
}

impl Vt {
    /// A blank terminal of `cols` by `rows` cells, clamped to [`MAX_COLS`] and [`MAX_ROWS`].
    ///
    /// Clamped rather than refused because the alternative in a `no_std` component is a panic in a
    /// process that has no way to report one, and a terminal that is smaller than asked for is
    /// visibly wrong. The component that wires it asserts the real geometry at compile time.
    ///
    /// **`const`, so a terminal can live in `.bss`.** See this struct's own doc for why that
    /// matters more now than it used to: a `Vt` is hundreds of KiB, and `const` is what lets a
    /// compile-time-constant geometry cost nothing at runtime. The clamps are spelled out rather
    /// than using `Ord::clamp`, which is not `const`.
    ///
    /// A **runtime** geometry must not call this directly and bind the result (see the struct doc);
    /// construct once with any geometry (typically `(1, 1)`, as `components/src/display_terminal.rs`'s
    /// `static` does) and call [`Vt::reset_to`] instead.
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
            // A fresh terminal is entirely damage: nothing has painted its surface yet, so the
            // first present must cover the whole grid rather than the empty rectangle "nothing
            // changed" would give.
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
    /// scrollback.** What [`Vt::new`] does, applied to an existing `Vt` by mutating it field by
    /// field, so a caller with a **runtime** geometry never needs a `Vt`-sized return value or
    /// local (see this struct's own doc for why that is now a real hazard rather than a style
    /// preference). Typical use: a `static mut` constructed once at `(1, 1)`, retargeted here the
    /// moment the real geometry is known (`components/src/display_terminal.rs`'s own bring-up, a kernel
    /// test that read a window's size off a control page).
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
    /// `row` is a **display** row: `0` is always the top of whatever is currently shown, whether
    /// that is the live grid (view offset `0`) or a scrolled-back history page. This is the one
    /// function every reader (`pixel`, `row_bytes`) goes through, so the scrollback view is uniform
    /// rather than a special case each caller has to know about.
    pub fn cell(&self, col: u32, row: u32) -> Cell {
        if col >= self.cols || row >= self.rows {
            return Cell::default();
        }
        if row < self.view_offset {
            // The topmost `view_offset` display rows come from history: display row 0 is the
            // oldest of the rows being shown from scrollback (`age = view_offset - 1`), and each
            // row closer to the live grid is one age newer, down to `age = 0` immediately above it.
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
        // `sb_tail` is the ring slot the *next* push will use, so the most recent row (age 0) is
        // one slot behind it, and each older age is one slot further behind, wrapping.
        let capacity = self.scrollback_depth();
        let ring_row = (self.sb_tail + capacity - 1 - age) % capacity;
        self.scrollback[(ring_row * self.cols + col) as usize]
    }

    /// **The pixel at `(x, y)` of the terminal's surface.** The whole of rendering, as a pure
    /// function of the grid (and, since milestone 142 increment 2, of the scroll position).
    ///
    /// The cursor is a **block**, drawn by swapping the cell's own two colors. Drawing it here
    /// rather than as a separate overlay is what keeps the picture a function of the state: a test
    /// that predicts the screen predicts the cursor too, and a cursor left in the wrong place is a
    /// failure rather than a cosmetic difference nobody notices.
    ///
    /// **The cursor does not draw while scrolled back.** It names a position in the live grid, which
    /// is not what is on screen when `view_offset` is nonzero; drawing it there would put the block
    /// on whatever history cell happens to share its `(col, row)`, which is not the cursor's
    /// position and would be actively misleading.
    ///
    /// **`col < self.cols` is checked after `col == self.col`, and by then it is redundant.**
    /// `self.col` is never `>= self.cols` (the same cursor-in-bounds invariant `Vt::erase_display`'s
    /// doc comment names), so `col == self.col` already implies `col < self.cols`; relaxing the
    /// comparison to `<=` cannot change which branch is taken. It stays written out because a reader
    /// should not have to chase that invariant to see the guard is safe.
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
        // The line renditions are ink across the **whole** cell width, gutters included, so an
        // underlined or struck-out word is one unbroken line rather than a dash per letter (unlike
        // the underscore glyph, whose ink stops at the gutters; see notes/glyphs.md). The
        // patterned underlines read the screen's `x`, not the cell's, so a wave or a run of dots
        // carries on across cells rather than restarting in each.
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

    /// **What changed since the last call, clearing the record.**
    ///
    /// The returned [`Damage`] carries the invariant a painter needs (move `scrolled` rows up,
    /// render `rect`); its own doc says why a scroll is movement rather than a whole-grid box.
    /// Taking the report also fixes the cursor into it: from this moment the engine knows where
    /// the cursor was last drawn ([`Vt::drawn`]'s own doc says why that has to survive scrolls),
    /// and the next report will name the row the old block has to be lifted from.
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
        // The cursor is part of the picture, so the report has to cover it moving, appearing or
        // vanishing since the last take. `feed` already dirties the endpoints of every move it
        // makes; this is the net that catches what a scroll carries sideways (a drawn cursor one
        // row up, say), and only dirtying on a real difference is what keeps an engine at rest
        // reporting `None`: a `BEL` must not become a one-cell flush.
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
        // **The rows a move of more than one row exposes are rendered whole.** A painter moves its
        // picture up `scrolled` rows at once, so the bottom `scrolled` rows keep the pixels the old
        // picture had there. The cell model behind `put`'s comparison is a move of one row at a
        // time, which leaves a copy of the bottom row behind, and the two agree only when
        // `scrolled` is one. Past that, a row wider than the rows that replaced it would leave its
        // tail on the screen.
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
            // A scroll with nothing to render (a bare LF over an already-blank bottom row, cursor
            // hidden) is still a report: the painter's pixels moved.
            None if scrolled > 0 => CellRect::EMPTY,
            None => return None,
        };
        Some(Damage { scrolled, rect })
    }

    /// Copy row `row`'s characters into `out` as bytes, returning how many were written. For tests
    /// and for a caller that wants the text rather than the pixels; there is no `String` in
    /// `no_std`. ASCII characters pass through unchanged; anything else (a non-ASCII `char`, since
    /// milestone 142's UTF-8 increment) becomes `?`, the conventional lossy placeholder, since the
    /// return type has no room for anything wider than a byte.
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

    /// **Push bytes through the parser.**
    ///
    /// # Deferred wrap, which is the one subtlety in printing
    ///
    /// Writing into the last column does **not** move the cursor to the next row. It leaves the
    /// cursor on that last cell and arms a pending wrap, and the *next* printable byte does the
    /// wrap first. Every real terminal does this, and the reason is visible in a line discipline's
    /// echo: a line that exactly fills the width would otherwise scroll the screen before anything
    /// asked it to, and a `CR` arriving right after the last character would find the cursor a row
    /// too low. Any cursor motion, `CR`, or `LF` cancels the pending wrap.
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
        // New output snaps the view back to live, the same convention every real terminal follows:
        // typing (or a program printing) while scrolled into history is disorienting otherwise, and
        // it is what makes `view_offset` a read-only concern for a caller that never scrolls. A
        // caller mid-history who then feeds nothing (a scroll key alone, which goes through
        // `scroll_up`/`scroll_down` and never reaches this function) is unaffected.
        if self.view_offset != 0 {
            self.view_offset = 0;
            self.damage_all();
        }
        let before = (self.col, self.row);
        for &b in bytes {
            self.byte(b);
        }
        if (self.col, self.row) != before && self.cursor_visible {
            // The cursor is painted, so moving it dirties both the cell it left and the one it
            // arrived at. Doing it once per `feed` rather than per byte keeps a long print from
            // dirtying a trail of cells it also overwrote.
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
                // `ESC \` is the standard string terminator; an ESC followed by anything else was
                // part of the string after all.
                self.state = if b == b'\\' {
                    State::Ground
                } else {
                    State::Str
                };
            }
        }
    }

    /// The Unicode replacement character, drawn for an invalid or incomplete UTF-8 sequence. The
    /// conventional choice (the same one `String::from_utf8_lossy` makes), so this engine's failure
    /// picture is the one a reader has likely seen from every other tool that decodes UTF-8.
    const REPLACEMENT: char = '\u{fffd}';

    fn ground(&mut self, b: u8) {
        // **UTF-8 decoding, ahead of the control-code match.** Every control code and every escape
        // introducer this engine understands is plain ASCII (`< 0x80`), so a byte with the high bit
        // set can only be part of a multi-byte character; checking `utf8_need` first is what lets
        // the two decoders (this one, and the ANSI/CSI state machine below) coexist without either
        // having to know about the other's bytes. State persists across `feed` calls, so a sequence
        // split at a buffer boundary still decodes correctly.
        if self.utf8_need > 0 {
            if b & 0xc0 == 0x80 {
                // A well-formed continuation byte.
                //
                // **The `|` cannot be distinguished from `^` here.** `self.utf8_code << 6` always has
                // its low six bits zero (a left shift by six fills them with zero), and `b & 0x3f` is
                // exactly six bits, so the two operands never share a set bit; OR and XOR agree on
                // every input for that reason alone, not because of anything this loop's callers do.
                self.utf8_code = (self.utf8_code << 6) | (b & 0x3f) as u32;
                self.utf8_need -= 1;
                if self.utf8_need == 0 {
                    let ch = char::from_u32(self.utf8_code).unwrap_or(Self::REPLACEMENT);
                    self.print(ch);
                }
                return;
            }
            // Not a continuation byte: the sequence was truncated. Draw the replacement for what
            // was collected so far and reprocess `b` as a fresh byte, rather than eating it, so a
            // truncated sequence loses exactly the bytes it claimed and nothing after them.
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
                // Backspace does not wrap back to the previous row. A line discipline never asks it
                // to (it only backs up within the line it echoed), and a terminal that did would
                // have to remember whether the previous row ended in a wrap.
                self.wrap_pending = false;
                self.col = self.col.saturating_sub(1);
            }
            b'\t' => {
                self.wrap_pending = false;
                // Every eight columns, and never past the last one: a tab at the right margin stops
                // there rather than wrapping, which is what the fixed-tab-stop terminals do.
                self.col = ((self.col / 8 + 1) * 8).min(self.cols - 1);
            }
            0x07 => {} // BEL: there is no bell on a framebuffer, and a visual bell is policy
            0x00..=0x1f | 0x7f => {} // every other control code: consumed, never drawn
            0x20..=0x7e => self.print(b as char), // plain printable ASCII, the common case
            // A UTF-8 lead byte: 0xc2..0xdf is two bytes total, 0xe0..0xef three, 0xf0..0xf4 four
            // (RFC 3629's range, past which no code point is assigned). 0x80..0xc1 and 0xf5..0xff
            // can never start a sequence (0x80..0xbf are continuation-only; 0xc0/0xc1 could only
            // encode a code point already representable in one byte, which RFC 3629 forbids as an
            // overlong form), so those draw the replacement immediately rather than waiting for
            // bytes that would never complete a valid character.
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
        // Scroll: the row about to fall off the top goes to scrollback first (milestone 142
        // increment 2), before the shift below overwrites it. The blanked bottom row is content
        // change and is dirtied by `put` (which compares, so blanking an already-blank row is
        // free); the movement itself is reported by `scroll_damage`, because a painter that can
        // move pixels should not re-render a screen it already holds.
        let cols = self.cols as usize;
        self.push_scrollback_row(0);
        let used = cols * self.rows as usize;
        self.cells.copy_within(cols..used, 0);
        // What changed since the last take moved up with the cells it changed, so its box moves
        // too, before the bottom row's own changes are added at the bottom.
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
    /// Only ever called from [`Vt::line_feed`], itself only reachable through [`Vt::feed`]'s byte
    /// loop, and [`Vt::feed`] resets `view_offset` to `0` before that loop runs (see its own doc).
    /// So a push never happens while the caller is looking at history, and this does not need to
    /// (and does not) adjust `view_offset` to compensate for the shift a push would otherwise cause.
    ///
    /// **`row` is always `0`.** This is a private method with one call site, which passes the
    /// literal `0` (see [`Vt::line_feed`]): the row about to scroll off the top is always the grid's
    /// first row, because scrolling is what makes room at the bottom, not a copy from elsewhere. So
    /// the source index's `row * cols` is always `0 * cols`, and a mutant reading it as `row / cols`
    /// computes the same `0` on the only input this function is ever given.
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
            // The string introducers: OSC (`]`), DCS (`P`), SOS/PM/APC (`X`, `^`, `_`). Their
            // payload is arbitrary text, so it has to be *consumed*, not returned to Ground.
            b']' | b'P' | b'X' | b'^' | b'_' => self.state = State::Str,
            // Anything else: the introducer and this byte are both consumed. A two-byte escape this
            // engine does not speak must not leave its final byte to be printed as a letter, which is
            // the classic "stray letter on the screen" bug.
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
            // A `:` separates a sub-parameter from the parameter it belongs to. The first
            // parameter may be empty either way (`CSI ;5H`, `CSI :2m`), so a separator before any
            // digit opens the first parameter as well as the next.
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
            // A private-use introducer (`?`, `<`, `=`, `>`) or an intermediate byte: this engine
            // implements none of those sequences, so it swallows the whole thing rather than acting
            // on a parameter list that means something else.
            //
            // **Deleting this arm changes nothing**: `0x20..=0x2f` and `0x3c..=0x3f` are not matched
            // by `b'0'..=b'9'`, `b';'`, or `0x40..=0x7e`, so removing the arm routes those bytes to
            // the catch-all below, which sets `self.ignore = true` too. Kept written out rather than
            // folded into `_` because a reader needs to see the two families of swallowed byte (the
            // sequences this engine chose not to implement, versus a stray control code) named
            // separately, even though the code they run is identical.
            0x20..=0x2f | 0x3c..=0x3f => self.ignore = true,
            0x40..=0x7e => {
                let ignore = self.ignore;
                self.state = State::Ground;
                if !ignore {
                    self.csi_final(b);
                }
            }
            _ => {
                // A control code inside a sequence: real terminals execute it. Nothing that reaches
                // this engine does that, so it is dropped along with the sequence, which fails in the
                // direction of drawing nothing rather than drawing garbage.
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
            // CUP. Parameters are **one-based** on the wire and zero-based here, which is the
            // off-by-one every terminal implementation has had at least once.
            b'H' | b'f' => {
                self.wrap_pending = false;
                self.row = (self.param(0, 1) - 1).min(self.rows - 1);
                self.col = (self.param(1, 1) - 1).min(self.cols - 1);
            }
            b'J' => self.erase_display(self.param(0, 0)),
            b'K' => self.erase_line(self.param(0, 0)),
            b'm' => self.sgr(),
            // Every other final byte: the sequence is consumed and nothing happens. That includes
            // the device-report sequences, deliberately: this engine has no way to answer one and a
            // half-answered query is worse than an unanswered one.
            _ => {}
        }
    }

    /// `CSI n J`: 0 erases from the cursor to the end of the screen, 1 to its start, 2 all of it.
    /// Note that `CSI 2 J` does **not** move the cursor; that is why a line discipline's ^L sends
    /// `CSI 2J` followed by `CSI H`.
    ///
    /// **`to > from` always holds here**, given two invariants this type maintains everywhere else:
    /// `cols >= 1` and `rows >= 1` ([`Vt::clamp_cols`]/[`Vt::clamp_rows`], the only places `cols`/
    /// `rows` are ever set), and `col < cols`, `row < rows` (every cursor-moving path clamps with
    /// `.min(self.cols - 1)`/`.min(self.rows - 1)`, so the cursor is never parked on or past the
    /// margin). Mode 0's `end - here = (rows - row) * cols - col >= cols - col >= 1`; mode 1's
    /// `here + 1 >= 1`; the default's `end = rows * cols >= 1`. So the guard below can never see
    /// `to == from`, which is why a mutant relaxing `>` to `>=` survives: there is no reachable
    /// input on which the two disagree.
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
    /// The extended colors (`38`, `48` and `58`, followed by `5;n` or `2;r;g;b`) consume the
    /// parameters they name, so the loop walks an index rather than iterating. A malformed one (a
    /// missing or out-of-range component, or a color space other than 2 or 5) ends the sequence's
    /// effect there, which is xterm's behavior: the parameters after it cannot be told apart from
    /// the broken color's own.
    ///
    /// **The colon form** (ECMA-48's sub-parameters) is read too: `4:3` is a curly underline, and
    /// `38:2::r:g:b` (with its empty color-space slot) or `38:2:r:g:b` a 24-bit color. A
    /// parameter's sub-parameters belong to it alone, so ones it does not use are skipped rather
    /// than read as the next attribute.
    fn sgr(&mut self) {
        let n = self.nparams.max(1);
        let mut i = 0;
        // Every pass consumes at least one parameter, so `MAX_PARAMS` passes always reach the end;
        // a bound rather than `while i < n` so the walk ends by construction.
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
                // Everything else (blink, overline, fonts) is dropped. Blink needs a clock this
                // engine deliberately does not read; drawing the *text* in the wrong style is
                // better than not drawing it.
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
        // A full box already says "repaint everything", so any scroll the same window accumulated
        // is subsumed: reporting movement as well would only invite a painter to move rows it is
        // about to overwrite. This is what keeps `Damage::scrolled < rows` true at every take.
        self.scrolled = 0;
    }

    /// **A scroll's half of the damage report**: the grid moved up a row, which is neither a
    /// changed cell nor a rectangle. See [`Damage`] for the invariant the painter hangs on this.
    fn scroll_damage(&mut self) {
        // The cursor the last report drew has just moved up a row with the pixels under it, while
        // the cursor itself stayed on the bottom row: the row the rendered block landed on must be
        // re-rendered, or a stale block rides up the screen one scroll at a time. This is the one
        // bookkeeping a scroll needs beyond the counter, and it is why `drawn` is tracked at all.
        if let Some((c, r)) = self.drawn {
            self.drawn = if r > 0 { Some((c, r - 1)) } else { None };
            if r > 0 {
                self.damage_cell(c, r - 1);
            }
        }
        self.scrolled += 1;
        if self.scrolled >= self.rows {
            // Everything on screen was replaced between takes (`rows` scrolls move the top row
            // clean off), so the move conveys nothing: report the whole grid as content instead.
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
