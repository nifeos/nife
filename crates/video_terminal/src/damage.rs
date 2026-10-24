//! What changed since the last take: the rectangle of cells, and the rows that moved.
//!
//! Name: provisional (the lane of milestone 874 (the terminal engine moves its palette and its
//! tests out), 2026-10-12 UTC), named for `Damage`, the report; calef names things.

// Only the doc links below name the crate root's items, so the import exists only for rustdoc.
#[cfg(doc)]
use super::*;

/// **The rectangle of cells that changed**, half-open in both axes.
///
/// A bounding box rather than a region list, the same trade `compositor::Rect` makes for the
/// compositor and for the same reason: two small changes far apart cost the box that contains both,
/// which is a few extra glyph blits here and the wrong call at desktop resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellRect {
    /// Leftmost changed column.
    pub col: u32,
    /// Topmost changed row.
    pub row: u32,
    /// Width in cells.
    pub cols: u32,
    /// Height in cells.
    pub rows: u32,
}

impl CellRect {
    /// The empty rectangle: no cells, at the origin. What a report carries on its `rect` side
    /// when everything that changed was movement ([`Damage`]'s own doc holds the invariant).
    pub const EMPTY: CellRect = CellRect {
        col: 0,
        row: 0,
        cols: 0,
        rows: 0,
    };

    pub(super) const fn cell(col: u32, row: u32) -> CellRect {
        CellRect {
            col,
            row,
            cols: 1,
            rows: 1,
        }
    }

    /// The bounding box of two rectangles. Public because a client that must carry damage forward
    /// across a frame the compositor has not acknowledged yet does the same accumulation
    /// (`components/src/display_terminal.rs`), and two spellings of a bounding box is one too many.
    pub const fn union(self, o: CellRect) -> CellRect {
        let col = if self.col < o.col { self.col } else { o.col };
        let row = if self.row < o.row { self.row } else { o.row };
        let (sr, or) = (self.col + self.cols, o.col + o.cols);
        let right = if sr > or { sr } else { or };
        let (sb, ob) = (self.row + self.rows, o.row + o.rows);
        let bottom = if sb > ob { sb } else { ob };
        CellRect {
            col,
            row,
            cols: right - col,
            rows: bottom - row,
        }
    }

    /// [`CellRect::union`] with a box that may not exist.
    pub(super) const fn union_with(self, o: Option<CellRect>) -> CellRect {
        match o {
            Some(o) => self.union(o),
            None => self,
        }
    }

    /// This rectangle in **pixels**, as `(x, y, w, h)`. What a `FLUSH` or a compositor damage
    /// rectangle wants.
    pub const fn to_pixels(self) -> (u32, u32, u32, u32) {
        (
            self.col * crate::CELL_W,
            self.row * crate::CELL_H,
            self.cols * crate::CELL_W,
            self.rows * crate::CELL_H,
        )
    }
}

/// **What changed since the last [`Vt::take_damage`]: cells that changed, and rows that moved.**
///
/// A scroll is not a change to any cell; it is a change to *where the cells are*, so the report
/// is two fields with an invariant a painter can hang a fast path on:
///
/// > Move your picture up `scrolled` cell rows, then render `rect`; the result is the grid.
///
/// A consumer that cannot move pixels uses [`Damage::repaint_rect`]. Why, measured, is in
/// notes/video-terminal.md, "Damage: what changed, and what moved".
///
/// Name: provisional (the paint-path lane, 2026-09-30). `Damage` rather than `CellDamage`
/// because it is the one report this engine makes and the scroll half is not measured in cells
/// anyone can point at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Damage {
    /// How many cell rows the live grid scrolled up since the last take, always `< rows` (a
    /// scroll that would report `rows` has replaced every row and is reported as a full
    /// [`Damage::rect`] with `scrolled == 0` instead, so the two fields never double-count).
    pub scrolled: u32,
    /// The bounding box of cells whose **content** changed (not whose position moved), in the
    /// grid *after* any scroll: the box to re-render once the rows have been moved. The blanked
    /// bottom row a scroll opens is content change and is in here; the cursor's departure from
    /// the row it was last reported drawn on is too.
    pub rect: CellRect,
}

impl Damage {
    /// A report in two halves, for callers building one by hand (the tests do; the engine builds
    /// its own through [`Vt::take_damage`]).
    pub const fn new(rect: CellRect, scrolled: u32) -> Damage {
        Damage { scrolled, rect }
    }

    /// The box a consumer that **re-renders** must paint: `rect` when nothing scrolled, the
    /// whole grid when anything did, because a scroll changes every displayed row. This is the
    /// old whole-box meaning of [`Vt::damage`], kept for callers with no pixels to move.
    pub const fn repaint_rect(self, cols: u32, rows: u32) -> CellRect {
        if self.scrolled > 0 {
            CellRect {
                col: 0,
                row: 0,
                cols,
                rows,
            }
        } else {
            self.rect
        }
    }
}

#[cfg(test)]
mod tests;
