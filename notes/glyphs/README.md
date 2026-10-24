# Glyphs: the appendices to notes/glyphs.md

[`notes/glyphs.md`](../glyphs.md) is the page to read: the font that ships, the VT engine, the
terminal, input, and the honest limits. The files here hold the survey and the reasoning behind
them, at a length the main page's prose budget cannot hold.

- [`font-design.md`](font-design.md): where the drawn font departs from the Kaypro II's ROM, and where
  the grid will not let it.
- [`bitmap-font.md`](bitmap-font.md): how `crates/bitmap_font` is built and tested, and how its
  specimen sheet reads candidate fonts.
- [`font-survey-2026-08-19.md`](font-survey-2026-08-19.md): every candidate font, its license, its
  measurements, and the screen constraint.
- [`kaypro-ii-rom.md`](kaypro-ii-rom.md): the Kaypro II character ROM, found, rendered and excluded.
- [`retro-fonts.md`](retro-fonts.md): the rest of the era's fonts.
- [`terminal-and-proof.md`](terminal-and-proof.md): one endpoint, the deadlock, and how text on a
  screen is proved.
- [`libghostty-vt.md`](libghostty-vt.md): what adopting Ghostty's VT core would cost.
- [`glyph-atlas.md`](glyph-atlas.md): the terminal font's coverage tables, and why its blend is in
  linear light.

Name: provisional, for the directory and every stem in it. The directory follows the `notes/<stem>/`
appendix convention §212 (a prose budget) set, so its name is the parent's stem. Each file is named
for its content. calef names directories and files, and `script/names --unratified` lists each stem.
