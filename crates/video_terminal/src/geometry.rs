//! The grid's capacity: the most columns and rows a `Vt` holds, and its scrollback budget.
//!
//! Name: provisional (the lane of milestone 874 (the terminal engine moves its palette and its
//! tests out), 2026-10-12 UTC), named for the "Geometry" banner it was under in `lib.rs`; calef
//! names things.

// Only the doc links below name the crate root's items, so the import exists only for rustdoc.
#[cfg(doc)]
use super::*;

/// A cell's width in pixels: `glyph_atlas`'s, since every pixel this engine draws comes from that
/// atlas. A consumer laying a `Vt` out over a surface divides by this, never by a font's own
/// constant, so the grid and the picture cannot disagree about where a cell ends.
///
/// Name: provisional.
pub const CELL_W: u32 = glyph_atlas::CELL_W;
/// A cell's height in pixels. See [`CELL_W`].
///
/// Name: provisional.
pub const CELL_H: u32 = glyph_atlas::CELL_H;

/// The widest grid this engine can hold, in cells.
///
/// Fixed rather than allocated, because the terminal component has no allocator; the component
/// asserts its own geometry fits at compile time. 240 is every screen's width in cells at
/// `screen_console::ScreenConsole::scale_for_cell`'s scale for [`CELL_W`], which never gives a
/// screen `2 * screen_console::MIN_COLUMNS` (240) columns or more. See notes/video-terminal.md.
pub const MAX_COLS: usize = 240;
/// The tallest grid this engine can hold, in cells. See [`MAX_COLS`]. **Not every pair is
/// reachable**: a grid also fits in [`MAX_CELLS`], so 240 columns get 40 rows and 150 rows need 64
/// columns or fewer.
pub const MAX_ROWS: usize = 150;
/// Cells in the largest possible **live** grid, the on-screen viewport, paid once per terminal
/// instance from that program's own region.
///
/// A budget rather than `MAX_COLS * MAX_ROWS`: the grid is stored at its own width, so any shape up
/// to this many cells fits. Chosen with [`SCROLLBACK_CELLS`] so a `Vt`'s cells stay 164 page frames
/// and `handoff::HANDOFF_PAGES` stays 132, which `component::SWAP_PAGES` is built on. The
/// arithmetic is in notes/video-terminal.md, "The grid's capacity".
pub const MAX_CELLS: usize = 9_600;

/// **Off-screen history**, in whole rows, kept alongside the live grid: a fixed ring, because this
/// crate reaches no allocator.
///
/// 300 is a working depth, not a derived one, and the most rows rather than always the rows: the
/// ring holds [`SCROLLBACK_CELLS`] cells at the grid's own width, so it is 300 deep up to 80
/// columns and shallower past that. See [`Vt::scrollback_depth`] and notes/video-terminal.md.
pub const SCROLLBACK_ROWS: usize = 300;
/// Cells in the scrollback ring: 300 rows of the virtio scanout's 80 columns. See
/// [`SCROLLBACK_ROWS`] and [`MAX_CELLS`] for why it is a cell budget.
pub const SCROLLBACK_CELLS: usize = 24_000;
