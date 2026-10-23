# The VT engine: its handoff blob and its component

An appendix to [the VT engine](../video-terminal.md). It holds the design behind
`video_terminal::handoff` and `video_terminal::component`.

## The handoff blob

A replacement `display_terminal` absorbs its incumbent's whole state as a §209 (state handoff is
an opaque blob over a granted frame, and it is optional) blob. That is milestone 23 (a
capability-routed component OS with live replacement), under calef's ruling of 2026-10-19 (UTC),
"everything". The blob holds the live grid, the scrollback in order, the cursor, the rendition and
the parser's state. So a sequence cut in half by a swap (`ESC [ 3` before it, `1 m` after) still
means red. The damage record is not carried: a replacement repaints its whole surface from the
absorbed state, so the screen after a swap witnesses the state rather than the incumbent's pixels.

The layout is fixed little-endian words, never a memory image of `Vt`, so a build with another
field order or `Cell` padding reads it the same way. The header is the magic, the layout number,
twelve words of cursor and ring, the rendition's five, the parser's sub-parameter mask and its
sixteen parameters. A cell is four words. The first holds the character (a `char` needs 21 bits),
the rendition flags at bit 21 and the underline style at bit 28. Then come the foreground, the
background and the underline color. A color word's top byte is its kind (`0` indexed, `1` 24-bit,
`2` none, for an underline that follows the foreground) and the low three bytes its value. Only
cells in use are written: `cols * rows` of the live grid, then the scrollback's rows oldest first,
so the reader rebuilds the ring from slot zero.

A build reads every earlier layout too (calef, 2026-10-19 UTC: a replacement reads its
predecessor's handoff layout). A change to the layout is a new number, the old number's reader
stays, and the new layout's blob joins the fixtures in `handoff/tests.rs`. Those tests fail on a
layout that changed without a bump and on a bump without a fixture. Layout 1 had a three-word
rendition with no underline color or style, no sub-parameter mask, and underline as flag bit 2.

A blob for a grid of another shape is refused, not reflowed. The replacement negotiates its
geometry with the screen before it absorbs. So a swap that also changes the screen's size, or a
build that changes `MAX_CELLS` or the scrollback budget so the old shape no longer fits, rolls back
and the incumbent serves on.

## The component's slots and the swap

`video_terminal::component` holds the capability slots every builder hands `display_terminal`: the
kernel's test wiring, `system_initializer`'s session and `terminal_supervisor`. They are
positions rather than a `capability_plan` declaration because two of them are not endpoints, a map
budget and two frame runs, which `capability_plan::Direction` has no word for. A declaration for
this program is milestone 597 (a program carries its manifest in an ELF note)'s to write.

A supervisor starts the terminal with a control endpoint in `CONTROL` and `line_editor::proto`'s
start bits in its third argument. A quiesce is `OPERATION_QUIESCE` on the endpoint it serves,
answered `QUIESCED`, after which it reads only `CONTROL`. Unlike `line_editor`'s, its handoff run
is not mapped at build. It comes from the machine's swap reserve
(`line_editor::component::supervisor::SWAP_RESERVE_PAGES`), which exists only while a swap holds
it. So it arrives with `CTL_HANDOFF`, a `SEND_CAP` on `CONTROL`, and is mapped at
`HANDOFF_ADDRESS`. The blob is written into it and answered `NOTE_WRITTEN` on `SUPERVISOR`. A
replacement started with `START_ABSORB` takes the same message before it serves.

The page counts were measured with `MemoryRegion::USAGE` on 2026-10-19 (UTC):

- The map budget covers the page tables for the scanout, the output page and the handoff run. Four
  pages were committed on every architecture under `terminal_display_swap_tests`, so eight is twice
  that. The kernel's wiring and a session hand twenty-four, `kernel::user::display_service`'s
  figure, which predates a measurement.
- An instance is its image, its stack, its page tables and its revocation log. The image is most of
  it: blank cells are not zero bytes, so the grid (164 pages) sits in `.data` and in the file, about
  220 pages in all. The measurement was taken with a 177-page grid: debug images committed 208 pages
  on aarch64, 207 on riscv64 and 212 on `x86_64`. 256 is about a fifth over the largest, a smaller
  margin than `line_editor`'s half, because the grid's pages do not vary.
- A swap takes a whole second instance, its map budget and the handoff run, 396 pages. calef sized
  the reserve to exactly this.
- A `terminal_supervisor` holding this program adds one bay, four endpoints it retypes for the
  child, and four pages of page tables for its scratch window. A supervisor holding both children
  committed 440 pages with eighteen more of map budget than now, on all three architectures, against
  the 432 this and `SWAPPING_BUDGET_PAGES` grant.

The sums are also written out as const assertions, so a changed operator is a build error rather
than a reserve that quietly no longer fits. The mutation gate's survivors on these lines showed
the need.

The status reports are one each, ever: a status `SEND` is a rendezvous, so a component that
narrated every frame would block until its spawner listened.

