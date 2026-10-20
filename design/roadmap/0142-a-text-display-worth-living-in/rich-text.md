# Rich text: what `Attr` had to grow, and what increment five built

This appendix belongs to [milestone 142 (a text display good enough that people use it instead of a
GUI)](../0142-a-text-display-worth-living-in.md). It was moved out of that page on 2026-10-12 (UTC)
under [§212 (a prose budget)](../../decisions/0212-a-prose-budget-for-every-document.md), and the
main page is complete without it. It holds the rich-text analysis as written at minting, and the
detail of what lane `milestone/142-text-display` built on 2026-09-26.

## The cell at minting

`video_terminal::Attr` was one byte: four bits of foreground index, three bits of background index,
one bit of reverse. `Cell` was that byte plus a character byte, so a cell was two bytes and the grid
a fixed array. That was the whole of "rich".

What a display somebody would live in carries, in rough order of how much people notice:

| Attribute | Cost | Note |
|---|---|---|
| 24-bit foreground and background | 48 bits a cell | SGR 38;2 and 48;2. This is what every syntax highlighter emits and the single largest gap |
| UTF-8, and a `char` rather than a byte | 32 bits a cell | notes/glyphs.md already names the decoder's home: the VT engine, with `bitfont::glyph`'s signature becoming `char` |
| Real bold and italic | a second and third face | see below, this reverses a recorded decision |
| Underline, and its styles | 3 bits | SGR 4:1..4:5, and a separate underline color if we are being honest about "rich" |
| Strikethrough, dim, blink, invisible, overline | 5 bits | cheap, and expected |
| Hyperlinks (OSC 8) | a side table | the one modern terminals added that people actually use |
| Double-width cells | 1 bit and a layout rule | falls out of UTF-8 the moment CJK arrives |

A cell goes from 2 bytes to roughly 16. On a 91x27 grid that is 39 KB against 5 KB, which the heap
holds without argument. Scrollback is what makes it interesting: a thousand lines of history at 91
columns is about 1.5 MB. `MAX_COLS` and `MAX_ROWS` were 32 and 16, with the grid a fixed array in
`.bss`. Somewhere between here and there the grid stops being a `static` and starts being an
allocation, and that is the change increment two actually was.

## Bold is bright, reversed on purpose

One recorded decision reverses, and it should be reversed on purpose rather than by accident.
`crates/video_terminal` says *"bold is bright"*, because a bold weight needs a second font and at
8x8 a bold face is a smudge. At an anti-aliased 11-point cell both halves of that reason expire: a
bold face is legible, and the atlas has room for it. So "rich" means shipping four faces rather
than one. That is a license question four times over and a table four times larger. It is the
hidden cost in calef's word *rich*, and a family with no italic is disqualified by it and by nothing
else. [§166 (the rasteriser dependency, and whether the glyph atlas ships one face or
four)](../../decisions/0166-the-rasteriser-and-how-many-faces.md) ruled four on 2026-10-05 (UTC),
with italic as DejaVu's Oblique.

## What 2026-09-26 built

§166 was then still `PROPOSED` and milestone 141 (a palette worth looking at) was NOT-STARTED, so
increments three and six stayed blocked; 141 landed 2026-09-29. What needed neither was built.

That pass built everything in increment five but the faces:

- A rendition is two `video_terminal::Colour`s (an index into xterm's 256-color table, or 24-bit)
  and a byte of flags.
- SGR `38;2`, `48;2`, `38;5`, `48;5`, the bright backgrounds, dim, underline, concealed and
  crossed-out all draw, each with its off switch.
- Bold is still bright, now as a flag resolved at paint time, so the atlas can draw a real bold face
  later without the parser changing.
- A cell went from 8 bytes to 16 (a const assert holds it), so a `Vt` is 724,416 bytes, up from
  362,208.
- The greeting the three-party scanout check grades now carries a 24-bit color and an underline.
  The pixel-exact proof therefore covers both on every ISA that runs the display suite.

## What finished increment five

- SGR 1 and 3 draw `glyph_atlas::BOLD`, `OBLIQUE` and `BOLD_OBLIQUE`, and bold no longer
  brightens: the color is drawn as set.
- The parser reads `:` as ECMA-48's sub-parameter separator, so `4:0` to `4:5` choose none, single,
  double, curly, dotted or dashed, `21` is double, and `38:2::r:g:b` and `38:5:n` are the colon
  spellings of the extended colors. Sub-parameters a parameter does not use are skipped.
- SGR 58 colors the underline in either spelling and 59 returns it to the foreground.
- A cell is twenty bytes. `MAX_CELLS` fell to 9,600 and `SCROLLBACK_CELLS` to 24,000 so that a
  `Vt` is smaller than before and the handoff blob, now four words a cell (layout 2), still fits
  its 132 pages. A layout-1 blob is still read, its underline flag becoming a single underline.

The navigation cluster is BUILT: home, end, insert, delete, page up and page down, each with
xterm's shift form, tested against the real line discipline.

Scrollback from the keyboard is PROPOSED, not the small follow-up this block first called it. In the
boot a person uses, `line_editor` sits in front of `display_terminal`, so the component holding the
history never sees a keystroke. The route needs a new opcode, and [milestone 668 (scrollback from
the keyboard)](../0668-scrollback-from-the-keyboard.md) weighs four. It edits the two programs
milestone 23 (a capability-routed component OS with live replacement) is building live replacement
for, so it should follow that lane. The keys (`CSI 5;2~`, `CSI 6;2~`) are sent since 2026-09-26.

Increment four blends the atlas's coverage in linear light. Dim is not a coverage value and does not
use it: the linear-light midpoint of white and black is sRGB 188, which reads as barely dimmed, so
dim stays an sRGB midpoint.
