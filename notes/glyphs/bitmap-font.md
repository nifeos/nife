# The bitmap font crate, its table and its specimen sheet

An appendix to [notes/glyphs.md](../glyphs.md), which keeps the font's geometry and provenance.
This file holds how `crates/bitmap_font` is built and tested, and how `examples/specimen.rs` reads
candidate fonts.

## Why a bitmap table

A scalable font wants an allocator, floating point, a hinting rasterizer and a file to load, and a
`no_std` component has none of them. Here the glyphs are a `static` and drawing one is a bit test.
The expected picture is a pure function that three parties evaluate independently: the terminal
that draws (`cell_pixel`), the kernel test that checks, and the host that reads the scanout back out
of QEMU. None of the three can be wrong in a way the others agree with, which is what makes text on
a screen provable rather than plausible.

`glyph` is total. A byte the font has no picture for draws `MISSING`, a hollow box, the convention
every terminal uses for "I cannot draw this". A font that returned nothing or a blank for an
unmapped byte would make a mojibake bug look like a spacing bug. Since the UTF-8 increment of
milestone 142 (a text display good enough that people use it instead of a GUI) the function takes a
`char`, so a multi-byte UTF-8 sequence decodes to one `char` and takes one cell, drawn as the box,
rather than a run of wrong pictures, one per byte. The repertoire did not grow with the signature; a
real repertoire needs a real font.

## The table

`src/glyphs.rs` is `kaypro-style-7x8.art` transcribed, generated data in its own file so the code
that uses the font is not buried in 128 rows of hexadecimal. The `.art` file is the authoring source
and the thing to edit, and `the_art_file_and_this_table_agree` parses it back and fails if the two
drift. The picture a reviewer reads and the bits a terminal draws cannot disagree.

The bit order is the one thing a font table gets wrong. One byte per row, top row first, and bit 0
is the leftmost pixel. That is the opposite of the convention most bitmap fonts use, and exactly
what produces a mirrored alphabet nobody notices until it is on a screen. The tests assert it
against an asymmetric glyph (`F`), and assert that bits 0 and 6, the gutter, are clear in every
glyph. A mirrored font hides in symmetric letters, which is why the examples print an `F`.

`every_glyph_in_the_table_is_read_and_plausible` asserts properties of the whole table, because the
art comparison reads the 128 rows through a loop over a parsed picture. A line-coverage tool sees
the parser run and the literal rows never do. That is the shape of every generated-data file, and
`script/coverage`'s exemption list is only for code that physically cannot run, never for code that
is merely untested. Each property would catch a real transcription accident:

- No printable character is blank. A row of zeroes where a letter should be is the commonest way a
  hand-transcribed table goes wrong, and it stays invisible until somebody types that character.
- Space is blank, the same assertion from the other side.
- No bit is set outside the seven drawn columns. Bit 7 set would smear into the neighboring cell.
- No two printable glyphs are identical. A copy-paste of that kind is one a reader cannot see and a
  specimen sheet barely shows.

## The specimen sheet

`examples/specimen.rs` prints the same text in every font it is given, so fonts are compared by
looking rather than by reading a table of byte counts. It reads four formats:

- GNU Unifont `.hex` (`CODEPOINT:BITS`, one glyph per line).
- Adobe `.bdf`, what Terminus, Spleen and most X11 bitmap fonts ship as.
- The `.art` format, a `#` and `.` picture per glyph, the only sane way to author a font by hand.
- A raw character-generator ROM (`.rom` or `.bin`), the format a font arrived in before it was a
  file.

```text
cargo run -p bitmap_font --example specimen
cargo run -p bitmap_font --example specimen -- --font unscii-8.hex --name unscii-8
cargo run -p bitmap_font --example specimen -- --font ter-u16n.bdf --name terminus-16
cargo run -p bitmap_font --example specimen -- --font bench/font-options/hand-drawn-8x8.art
cargo run -p bitmap_font --example specimen -- --font 81-146a.bin --name kaypro-ii
cargo run -p bitmap_font --example specimen -- --metrics \
    --font 81-146a.bin --name kaypro-ii --font gohufont-14.bdf --name gohufont-14
```

Several `--font` flags draw the fonts interleaved, sample line by sample line, because the eye
cannot compare across a scroll. The name sits in a left gutter on its block's first row, and a
specimen in a section of its own is an inventory, not a comparison. Half blocks, the default, show
the font at roughly its real aspect ratio: a terminal cell is about twice as tall as it is wide, so
one half block is about square. That is the mode for "does this look good". `--dots` prints one
character per pixel, the mode for "what exactly is in this glyph". Some terminals antialias U+2580
and U+2584, which makes strokes look softer than the font is, and `--dots` is then the tie-breaker.

`--metrics` prints the font's row for the table in notes/glyphs.md. Every candidate so far draws all
94 printable positions, and a font that does not is already disqualified by the `printable_drawn`
column, so a missing `x` or `g` gives a nonsense baseline rather than a refusal.

### Reading a character-generator ROM

A chargen ROM is a chip on the video board, addressed by character code and scanline, whose data
lines feed a shift register directly. It has no header and no metadata, so three things are derived
from the bytes, each checkable by looking at the ROM:

- The used half. A 2K part often carries a 1K font with the unused half uniform. Kaypro's `81-146a`
  is blank for its first 1024 bytes because the hardware selects that half during blanking, so the
  reader skips the leading uniform 1K-aligned blocks.
- Rows per glyph, which is then the used length over 128.
- Inversion. A font is mostly paper, so if most bits in the used region are set, the part stores ink
  as zero. MAME says the same thing with `ROMREGION_INVERT`.

One thing is a fact about the wiring and is only inferred: how many of the eight data lines reach
the shift register. Top bits that are paper in every byte are taken as unconnected, and the glyph is
the remaining span plus one blank gutter column on each side. That gutter is Kaypro's
(`kaypro_v.cpp` shifts out `0`, five bits, `0`), not a universal truth. A chargen wired with two
blank columns, or with the glyph left-aligned rather than right-aligned, comes out at the wrong
pitch and looks loose rather than wrong, the hardest kind of error to see; `--advance` overrides it.
