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
/// Fixed rather than allocated, because the terminal component has no allocator and this crate is
/// the same code the kernel and the host run. The terminal component asserts its own geometry fits
/// at compile time, so a screen too wide is a build error rather than a truncated screen.
///
/// 240 is every screen's width in cells: the terminal on a firmware screen fills it at
/// `screen_console::ScreenConsole::scale_for_cell`'s scale for [`CELL_W`], and that rule never gives
/// a screen `2 * screen_console::MIN_COLUMNS` (240) columns or more. QEMU's 1280x800 OVMF screen is
/// 91, xenon's 1920x1080 is 137. The virtio-gpu scanout lays out 80 (`script::COLS`).
pub const MAX_COLS: usize = 240;
/// The tallest grid this engine can hold, in cells. See [`MAX_COLS`]. **Not every pair is
/// reachable**: a grid also fits in [`MAX_CELLS`], so 240 columns get 40 rows and 150 rows need 64
/// columns or fewer.
pub const MAX_ROWS: usize = 150;
/// Cells in the largest possible **live** grid (the on-screen viewport; see [`SCROLLBACK_ROWS`] for
/// the off-screen history alongside it). A real cost, paid once per terminal instance and
/// auto-provisioned from that program's own region (the kernel's `user::load` sizes a process's
/// address-space region from its ELF segments, `.bss` included), not from any shared budget.
///
/// **A budget rather than `MAX_COLS * MAX_ROWS`**: the grid is stored at its own width, so any
/// shape up to this many cells fits. 9,600 holds every screen this tree drives at the 14x26 cell
/// (xenon's 137x41 is 5,617) with room.
///
/// **Chosen with [`SCROLLBACK_CELLS`] to hold two numbers still.** A [`Cell`] is twenty bytes and a
/// handoff blob four words a cell, so the 33,600 cells of both budgets make a `Vt`'s cells 672,000
/// bytes (164 page frames, under the 717,312 of the sixteen-byte cell's larger budgets) and a blob
/// at most 537,744 bytes: `handoff::HANDOFF_PAGES` stays 132, which the swap reserve calef sized
/// is built on (`component::SWAP_PAGES`).
pub const MAX_CELLS: usize = 9_600;

/// **Off-screen history**, in whole rows, kept alongside the live grid (milestone 142 increment 2).
///
/// A fixed ring rather than a `Vec`, for the same reason the live grid is a fixed array: this crate
/// reaches no allocator, so the capacity is a constant three parties (the terminal, the kernel test,
/// the host-side check) already agree on the same way they agree on [`MAX_COLS`]/[`MAX_ROWS`].
///
/// **300, chosen as a working depth rather than derived from anything.** The ring's memory is the
/// [`SCROLLBACK_CELLS`] budget, priced with the live grid's in [`MAX_CELLS`]'s doc. The kernel's
/// test image holds eight `Vt` statics as witnesses (`system_tests/src/user/display_tests.rs` and
/// `compositor_tests.rs`), about 5.4 MB of `.bss` against QEMU's 256 MiB.
///
/// **The most rows, not always the rows** (2026-10-04): the ring holds [`SCROLLBACK_CELLS`] cells
/// at the grid's own width, so it is 300 rows deep up to 80 columns, the virtio scanout's, and
/// shallower past that: 263 at OVMF's 91, 175 at xenon's 137. See [`Vt::scrollback_depth`].
pub const SCROLLBACK_ROWS: usize = 300;
/// Cells in the scrollback ring: 300 rows of the virtio scanout's 80 columns. See
/// [`SCROLLBACK_ROWS`] and [`MAX_CELLS`] for why it is a cell budget.
pub const SCROLLBACK_CELLS: usize = 24_000;
