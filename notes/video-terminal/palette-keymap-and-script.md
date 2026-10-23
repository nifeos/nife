# The VT engine: its palette, its keymap and its test script

An appendix to [the VT engine](../video-terminal.md). It holds the design behind `video_terminal`'s
palette, `video_terminal::keymap` and `video_terminal::script`.

## The palette

`PALETTE` is canonical Solarized Dark from Schoonover's published table
(github.com/altercation/solarized, "The Values", `16/8 TERMCOL`), as §104 (the rich-text font is
DejaVu Sans Mono, and the palette is Solarized) chose. Solarized's bright half is mostly its gray
ramp: entries 8, 10, 11, 12 and 14, with 9 orange and 13 violet. Indices 0 to 7 are the normal
colors in ANSI order and 8 to 15 their bright forms. Entry 14 is `0x93a1a0`, one unit of blue off
Schoonover's `0x93a1a1`, so that it passes the gate, and
`the_palette_is_solarized_dark_but_for_one_unit` holds that this is the whole difference. `93,a1,a1`
repeats a channel, so a swapped green and blue would leave it unchanged. The nudge is a CIE76
difference of 0.56, under the roughly 2.3 quoted as just noticeable.

The gate is a compile-time check, so any palette that compiles is a good test instrument and the
choice among them can be about looks. Milestone 141 (a palette worth looking at, and a gate that
lets it be one) names three properties, each closing one way a corrupted pixel could land on a
different legal color:

1. No entry repeats a channel. Such an entry survives a swap of its two equal channels, so a
   channel-order bug in the blit leaves it untouched.
2. No two entries are channel permutations of each other, a duplicate included, or a swap turns
   one legal color into the other.
3. No channel is `0xff` in more than one entry. A saturating write or a dropped shift tends to pin
   a channel at `0xff`, and with one such entry that fault has few legal colors to land on.

These are this tree's reconstruction of what the palette's original comment reached for, not a
specification anybody wrote down. A fourth failure mode nobody has named would pass them. The host
tests run the check against a palette known to fail, which is what makes a clean result mean
something.

### The palette's BUGS

- The defaults are ANSI 7 on ANSI 0, `base2` on `base02`: a pale cream on the dark highlight
  color, not Solarized Dark's `base0` on `base03`. The fix is to point `DEFAULT_FG` and
  `DEFAULT_BG` at 12 and 8. A rendition color is a full indexed color, `Indexed(u8)`, so the rendition
  can name 8.
- The properties guard the sixteen, not every color a cell can hold. A 256-color or 24-bit color
  can land a swapped channel on a legal color, and the pixel-exact scanout comparison carries that
  load.
- Whether changing one number leaves this "Solarized" is calef's question under §104. It is
  recorded where a reader meets the constant, so nobody reasons from the belief that it is
  Schoonover's table untouched.

## The keymap

A virtio-input device sends `virtio_input_event` records carrying Linux evdev codes, a physical
key going down or up, in a numbering unrelated to the keycap. Turning those into a terminal's
bytes is a table and a little state, so it lives on the host (§7 (testing: QEMU harness +
host-testable crates, from commit one)) rather than in a driver. It is in this crate because it is
the display terminal's input half. The bytes are the terminal contract's driver half
(`OPERATION_BYTES`), so a keystroke from this device and one from the UART are the same by the
time anything downstream sees them.

It covers a US layout's main block, shifted and unshifted, and the arrow and navigation clusters,
each with its shift form. Control chords on letters come from milestone 242 (USB host and HID,
because on commodity hardware the keyboard is not a UART). Control folds `@` through `_` and their
lower-case twins onto `0x00..=0x1f`, so `^C` is 0x03 with or without shift. Not covered: keypad,
function keys, control or alt on a cluster key, compose, dead keys, other layouts, and key repeat
beyond what the device sends. Device repeats count as presses, so holding a key types.

The main block is one flat table, because a layout is data and the failure of a 58-arm match is a
row silently missing. The cluster codes start at 102, far above the table's 57, so they get their
own match, and a const assertion keeps the table below them. The cluster sends xterm's sequences in
its normal cursor mode, which `line_editor` already reads. The parameter `1` is written only when a
modifier forces a second one (`CSI 1;2 A`, never `CSI 1 A`). There is no SS3 variant; `line_editor`
treats SS3 arrows the same (`ss3_arrows_are_understood`). An unmapped key produces nothing rather
than a placeholder, so reaching for a function key does not corrupt the line.

## The test script

`video_terminal::script` holds what milestone 29 (a display terminal: framebuffer, virtio-gpu, and
a foreign component)'s tests put on the screen. Text on a screen is proved only when more than one
independent witness can compute the picture. The kernel-side test sends these bytes and predicts
the framebuffer by running the engine; the host-side scanout check runs the same engine and grades
what QEMU displays. Neither reads the other's answer, and the terminal component holds none of
it. `graphics_contract::pixel` and `compositor::SCENE` make the same move for rungs one and two.

`script::COLS` is a literal because this crate does not depend on the framebuffer contract; the
component and the kernel wiring assert at compile time that the two agree. The greeting is built
to be hard to produce by accident:

- Four rows of text, so a stride error or a one-row shift is visible.
- Five renditions: a 24-bit foreground, default, green, an underline and a reversed block. A
  terminal that ignored SGR, dropped `38;2` or drew underlined text without its line fails. The
  24-bit value is Solarized's yellow, `#b58900`, which is in neither the sixteen nor the 256-color
  table, so a terminal that rounded it fails.
- A `\r\n` pair, which is what `line_editor::expand_output` puts on the wire for a Unix `\n`.
- A bold italic word drawn in `glyph_atlas::BOLD_OBLIQUE`, and a curly underline in SGR 58's red,
  so a terminal that drew the regular face, a straight line or the text's color fails.
- Descenders and an underscore (`y`, `_`), the rows a font truncated to seven rows would lose.

The negative control changes one letter: a checker that asked only whether the screen is blank
would pass every run. The compositor windows print different banners and take different letters,
so a keystroke routed to the wrong client is a wrong picture, the capability claim of §33 (the
compositor's authority is memory, not messages) made visible.

The scroller feeds 48 lines, one `OPERATION_WRITE` each, then a five-line tail in one write. Five
rows are in use when it starts, so it scrolls a 24-row grid 34 times, both per present and several
within one. It is the only pixel-exact witness that pushes a picture past its own height, so it is
the end-to-end witness for the scroll path, where the `x86_64` swish leg spends most of its time. Each
line carries its own index, so a picture scrolled by the wrong count disagrees at every digit. The
windows scroll seven lines each, which runs the `MODE_WINDOW` present path's scroll publishing. One
function hands the writes to every witness, so no witness can feed only the per-line half and end
scrolls ahead of its screen.

