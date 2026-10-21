# Option G's upkeep: what a slow upstream leaves to nife

This appendix belongs to [milestone 705 (the graphical terminal runs full-screen programs)](../0705-the-graphical-terminal-runs-full-screen-programs.md).
It measures how much libvterm engine work would fall to nife, in C, if nife vendored 0.3.3
([option G](option-g.md)).

Measured on 2026-10-21 (UTC) on patagonia. Sources: the 0.3.3 tarball (the SHA-256 in option G's
table), upstream's own Bazaar branch at revision 845, and Neovim's `master` at `1944cb320`
(2026-10-20). Line counts are the added and removed lines of each commit's diff. They count
library code only, and leave out tests and build files.

## Upstream after 0.3.3

The canonical branch is the author's own Bazaar server, `bazaar.leonerd.org.uk/c/libvterm`.
`lp:libvterm` is a Launchpad import of it. That import is suspended and ends at revision 844, as do
the GitHub mirrors found.

| Revision | Date (UTC) | Change | Library lines |
|---|---|---|---|
| 840 | 2024-07-16 | a `premove` state callback before scroll moves | +38 −0 |
| 841 | 2024-07-16 | scrollback push uses `premove` | +13 −4 |
| 842 | 2024-07-16 | an `sb_pushline4` callback with a continuation flag | +28 −8 |
| 843 | 2024-07-16 | a test only | 0 |
| 844 | 2024-07-24 | the `REFLOW` macro inlined | +4 −6 |
| 845 | 2026-10-20 | bold-is-highbright only for SGR 30 to 37, not SGR 38 | +10 −5 |

That is six commits and no release in 39 months, +94 −24 lines in all, from one committer.
Revision 845 came 27 months after 844 and a day before this measurement. The upstream is slow,
not closed.

## Neovim's copy

Neovim vendored 0.3.3 unmodified on 2024-08-08 (`fa79a8ad6`). On 2025-01-03 (`d8bc08db7`) it moved
the code to `src/nvim/vterm/` and declared it Neovim's own, because it had changed too much to call
a bundled dependency. That commit also restyled every file, so a raw diff against 0.3.3 is mostly
formatting (+6,262 −6,569 in that commit alone) and measures nothing. The counts below are per
commit.

| Class | Commits | Library lines |
|---|---|---|
| Bug fix that applies to stock libvterm | 10 | +50 −26 |
| Bug fix in Neovim's own changes | 1 | +1 −1 |
| New protocol feature | 7 | +488 −52 |
| Neovim-only integration | 7 | +106 −659 |
| Refactor or style | 7 | +6,288 −6,627 |

The ten fixes cover an OSC terminator holding C0 bytes, and a parser flag cleared too early. Two
cover the CSI argument limit (raised, not bounded) and split UTF-8 after ASCII. The rest cover
composing characters at the right edge, reflow of the alternate screen, the cursor on resize,
truecolor SGR with a color-space id, and a double-width glyph past the edge (taken from Vim). None
is upstream at revision 845.

The features are the kitty keyboard protocol (+266 −37), OSC 8 hyperlinks (+91 −3), SGR dim and
overline (+61 −2), and DEC mode 2031 (+35). The rest are X1 and X2 mouse buttons (+19 −8), synchronized output in
mode 2026 (+13), and a newer DA1 reply (+7 −2).

This is not one year of nife's needs. It is 26 months of one heavy embedder with needs of its own.
The later commits are counted after the restyle, and a fix's size in Neovim's tree is close to,
but not exactly, its size against 0.3.3.

## Claimed gaps

0.3.3 has key and mouse encoders and reflow, so option G's appendix was wrong to credit only
libghostty-vt with them:

- Keys: `vterm_keyboard_unichar` and `vterm_keyboard_key` in `src/keyboard.c`, with bracketed paste
  in `vterm_keyboard_start_paste` and `vterm_keyboard_end_paste`.
- Mouse: `vterm_mouse_move` and `vterm_mouse_button` in `src/mouse.c`, with SGR mode 1006 in
  `src/state.c`'s mode handling.
- Reflow: `vterm_screen_enable_reflow` in `include/vterm.h`, applied in `src/screen.c`'s
  `resize_buffer`. Neovim has fixed two reflow bugs since, both counted above.

## The fuzz pass

A libFuzzer target over `vterm_input_write` ran on the host with AddressSanitizer and
UndefinedBehaviorSanitizer. It found crashes, and they are being reported privately.
