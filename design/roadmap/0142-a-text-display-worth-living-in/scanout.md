# The scanout and the grid: the original arithmetic and how increments one and two were built

This appendix belongs to [milestone 142 (a text display good enough that people use it instead of a
GUI)](../0142-a-text-display-worth-living-in.md). It was moved out of that page on 2026-10-12 (UTC)
under [§212 (a prose budget)](../../decisions/0212-a-prose-budget-for-every-document.md), and the
main page is complete without it. It holds the scanout section as first written at minting and the
build narrative of increments one and two.

## The scanout, as written at minting

128x64 was chosen as a test instrument and says so. `gfx_proto::WIDTH` records that a square
surface hides a stride bug, a transposition and an x/y swap, and that QEMU refuses a scanout
smaller than 16 a side. That reasoning survives growth unchanged, because it constrains the
*ratio* rather than the size. The size it wants now falls out of the measured cell rather than out
of taste.

The recommendation was 1280x720, and it was arithmetic:

| Surface | Frames | Grid at the 2x cell (14x26) | Grid at the 1x cell (7x13) |
|---|---|---|---|
| 128x64 today | 8 | 9 x 2 | 18 x 4 |
| 1024x768 | 768 | 73 x 29 | 146 x 59 |
| 1280x720 | 900 | 91 x 27 | 182 x 55 |

1024x768 is the obvious number and it misses 80 columns at the 2x cell, which is the one
requirement a terminal has. 1280x720 clears it with room and is 16:9, so decidedly not square. It
satisfies both `const` assertions `gfx_proto` carries: it is far above 16 a side, and 1280x720x4 is
3,686,400 bytes, exactly 900 frames with nothing left over.

[§102 (a Frame names a run of pages)](../../decisions/0102-frame-names-a-run.md) is what made this
reachable. At minting it was decided and nobody was building it. A `PageFrame` naming a run turns
900 capabilities and 900 `MAP` calls into one of each. Without it the sixteen-slot capability table
refuses the surface outright. `Object::PageFrame` was then still `PageFrame(u64)` in
`crates/capability_table/src/lib.rs`, one page and no count.

That was worth flagging, because the milestone that motivated §102 no longer needed it. §102 was
raised to unblock milestone 29 (a display terminal)'s font increment at 800x608 for gohufont-14 at
8x14. [§100 (the terminal font)](../../decisions/0100-the-terminal-font.md) was then amended, and
the font that landed is a 7x8 drawing that gives 18x8 on the existing scanout. So the decision was
made and the pressure that produced it went away. This milestone became §102's first consumer, and
increment one is where it got built.

Two costs that are nobody's decision and that a lane will meet:

- The wire is fine. `gfx_proto::rect` packs each of `x`, `y`, `w`, `h` into 14 bits, so the
  ceiling is 16,383 a side. 1280x720 does not come close, and no format change is owed.
- The harness gets 112 times more expensive, and it is polled. `cargo xtask`'s scanout referee
  takes a `screendump` every 100 ms for the whole suite and compares every pixel. At 128x64 that
  PPM is about 24 KiB; at 1280x720 it is about 2.6 MiB, ten times a second, per ISA. The
  comparison is 921,600 pixels against 8,192. This is the real price of a big screen in this tree,
  and it belongs in the increment that grows the surface rather than in a later surprise. Nothing
  forces the referee to poll at 100 ms once a dump costs a hundred times more. (Settled since: the
  referee poll in `xtask/src/main.rs` records the 39x growth, and calef kept the 100 ms cadence.)

## How increments one and two were built

Both landed on `milestone/142-terminal-size`. They are also [journey
1](../../journeys/01-login-to-kilo.md)'s own step 6 (calef, 2026-08-27: "I want a full size usable
terminal"). That step was added once tracing the journey found that step 3, milestone 177 (wire the
graphical terminal stack into the real interactive boot), only wires an 18x8 test-instrument grid
into the real boot. That is not a terminal anyone would sit at. This milestone is independent of
177. It grows the VT engine's own grid, provable under the same test harness milestone 29 uses, and
needed neither the real-boot wiring 177 does nor anything else on that journey.

§102 is built and consumed, with `Object::PageFrame` now carrying the page count. The scanout was
first grown to 1280x720: 900 page frames, one capability instead of 900. Every place that named the
surface at the sixteen-slot table's old ceiling now holds one run capability:

- the display driver's DMA region;
- the painting client's and the display terminal's own surface grant;
- the compositor's screen, and a capture client's read-only mirror of it, which this milestone's own
  text did not anticipate touching.

`gfx_proto::WIDTH`'s non-square argument was re-checked at that size and held by construction:
1280x720x4 was exactly 900 page frames, no remainder.

The grid became a real terminal:

- UTF-8 decoding in the VT engine (`bitmap_font::glyph` takes `char`);
- a scrollback ring (`video_terminal::SCROLLBACK_ROWS`) with a viewport and
  `Vt::scroll_up`/`scroll_down`, which changes the damage model and is why milestone 29 deferred it;
- the arrow-key cluster in the keymap (`CSI A/B/C/D`), the sequence `line_editor` already
  understood on its receiving side.

### The correction, and the retarget to 132x43

One correction to this block's own arithmetic. Increment two's "91x27 at the target cell" computed
columns and rows at a *future* anti-aliased Menlo-derived cell: 14x26 at 2x, the cell increments 3
to 6 would build. At today's 7x8 bitmap font, unchanged by this pass, the honest grid at 1280x720
was 182x90. That is comfortably past the 80x24 floor. On review with calef (2026-08-27) it was also
roughly double any terminal anyone runs, most being 80x24 up to maybe 160x50 on a large monitor.

So on 2026-08-27 the target became 132x43, the classic VT100/VT220 "wide mode" size. The scanout is
924x344, sized directly against the shipping 7x8 cell instead of the future one: 924 = 132 * 7 and
344 = 43 * 8, both exact. This drops `graphics_contract::SURFACE_PAGE_FRAMES` to 311.
`crates/graphics_contract/src/lib.rs`'s `WIDTH` doc comment has the full arithmetic. It includes
the one property lost: 924x344's byte count is no longer an exact multiple of 4096, unlike
1280x720's, and no nearby resolution that still delivers exactly 132x43 recovers it. When the atlas
lands and the cell widens, the grid shrinks with it.

The scrollback engine was built and host-tested, but nothing sent `Vt::scroll_up`/`scroll_down`
from a keystroke. This block first called that a small, separate follow-up. On 2026-09-26 it turned
out to need a new opcode; [milestone 668 (scrollback from the keyboard: shift and page up scrolls
the display terminal's history)](../0668-scrollback-from-the-keyboard.md) holds it.
