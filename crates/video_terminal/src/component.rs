//! **What a supervisor builds a `display_terminal` from, and replaces it with** (milestone 23 (a
//! capability-routed component OS with live replacement); calef's rulings of 2026-10-19 (UTC)).
//!
//! The capability slots every builder hands `display_terminal` (the kernel's test wiring,
//! `system_initializer`'s session, and `terminal_supervisor`), so the four agree through one list
//! (AGENTS.md rule 7), and the page budgets a supervisor sets aside for it. Under a supervisor the
//! handoff run arrives with `CTL_HANDOFF` on [`CONTROL`] and is mapped at [`HANDOFF_ADDRESS`]. A
//! quiesce is refused unless it comes through a copy badged `line_editor::proto::SUPERVISOR_BADGE`.
//! The swap and the measured budgets are in notes/video-terminal/handoff-and-component.md.
//!
//! Name: provisional (the lane for milestone 23, 2026-10-19 UTC), for this module and every item
//! in it.

/// Its status reports (`TERM_UP`) go here: the spawner's endpoint, or under a supervisor the
/// supervisor's.
///
/// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
pub const REPORT: u64 = 0;
/// The endpoint it presents through: the display driver's, or a compositor's doorbell.
///
/// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
pub const PRESENT: u64 = 1;
/// The endpoint it serves, the terminal contract's IPC half.
///
/// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
pub const TERM: u64 = 2;
/// The untyped its own mappings' page tables come from, [`MAP_BUDGET_PAGES`] of it.
///
/// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
pub const BUDGET: u64 = 3;
/// The scanout, one frame run.
///
/// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
pub const SURFACE: u64 = 4;
/// The page an application writes the bytes of an `OPERATION_WRITE` into.
///
/// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
pub const OUT: u64 = 5;
/// Under a supervisor: the endpoint it is told `CTL_HANDOFF`, `CTL_RESUME` and `CTL_QUIT` on.
///
/// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
pub const CONTROL: u64 = 6;
/// Under a supervisor: where it says `NOTE_WRITTEN`, `NOTE_ABSORBED` and their refusals.
///
/// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
pub const SUPERVISOR: u64 = 7;

/// The map budget a supervisor hands an instance: the page tables for the scanout, the output
/// page and the handoff run. Eight is twice the four measured; the kernel's wiring and a session
/// hand twenty-four. See notes/video-terminal/handoff-and-component.md.
///
/// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
pub const MAP_BUDGET_PAGES: u64 = 8;

/// Where the handoff run is mapped: its own 2 MiB window, clear of the output page and the
/// compositor's control page below it.
///
/// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
pub const HANDOFF_ADDRESS: u64 = address_space_map::pair_page(0x0000_0000_0b00_0000);

/// **Pages one instance is built out of.** Its image, its stack, its page tables and its
/// revocation log. The image is most of it, because blank cells are not zero bytes and the grid
/// sits in `.data`. About a fifth over the largest debug image measured (212 pages, `x86_64`);
/// see notes/video-terminal/handoff-and-component.md.
///
/// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
pub const INSTANCE_PAGES: u64 = 256;

/// What a supervisor sets aside for one instance: the instance and its map budget.
///
/// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
pub const BAY_PAGES: u64 = INSTANCE_PAGES + MAP_BUDGET_PAGES;

/// **What a swap of one instance takes from the swap reserve**: a whole second instance, its map
/// budget, and the handoff run, 396 pages. calef sized the reserve to exactly this on 2026-10-19
/// (UTC).
///
/// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
pub const SWAP_PAGES: u64 = BAY_PAGES + crate::handoff::HANDOFF_PAGES;

/// What a `terminal_supervisor` that holds this program adds to its budget: one bay, the four
/// endpoints it retypes for the child (two control endpoints, a notification endpoint and a fault
/// endpoint), and four pages for the page tables that map an instance into its scratch window
/// while it is built. The measurement is in notes/video-terminal/handoff-and-component.md.
///
/// Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.
pub const SUPERVISOR_PAGES: u64 = BAY_PAGES + 4 + 4;

// The sizes written out, so a changed operator in a sum is a build error rather than a reserve
// that quietly no longer fits (`line_editor::component`'s precedent; the mutation gate's
// survivors on these lines, 2026-10-19).
const _: () = assert!(BAY_PAGES == 264);
const _: () = assert!(SWAP_PAGES == 396);
const _: () = assert!(SUPERVISOR_PAGES == 272);
