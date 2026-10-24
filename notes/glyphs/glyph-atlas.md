# The glyph atlas: coverage tables, and blending in linear light

An appendix to [notes/glyphs.md](../glyphs.md), which keeps the terminal's font and its provenance.
This file holds the design of `crates/glyph_atlas`: why its tables are data, and why its blend works
in linear light.

## The rasteriser runs once, on the host

`tools/glyph_atlas_generator` rasterises DejaVu Sans Mono on the host. Its output is the four files
under `crates/glyph_atlas/coverage/`, one byte of coverage a pixel. Its test fails unless those tables
are what it makes from `vendor/dejavu`, byte for byte.

So drawing a glyph is a lookup and a blend in linear light. That is exactly as pure as
`bitmap_font`'s bit test. The three parties that draw, predict and grade the terminal's picture each
compute it on their own, and none of them needs a font engine.

## Why the blend is in linear light

`blend` takes each channel to linear light through `SRGB_TO_LINEAR`, mixes it there, and brings it
back to the nearest sRGB code. Mixing the sRGB codes directly would make light text on a dark ground
look bolder than its outline, and dark text on light look thinner. That is because sRGB's codes are
not proportional to light. Solarized Dark is the first case. Half coverage of white on black is half
the light, which is sRGB 188, not code 128.

The arithmetic is integer only. The table is a `const`, and the way back is a search of the same
table, so no second table can disagree with the first.
