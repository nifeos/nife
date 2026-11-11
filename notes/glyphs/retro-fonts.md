# The rest of the era, and the one failure mode the retro fonts share

*Name: ratified 2026-10-18 (calef, convention-rulings Rule 3).*

An appendix to [notes/glyphs.md](../glyphs.md). It holds the survey of early machines' fonts beyond
the Kaypro II, and what it is evidence for.

calef asked whether any other early machine had a font worth wanting. Four were checked, and the
answer is the same for all but one, for a reason that is more useful than the individual verdicts. A
permissive notice on a retro-font repository usually covers the packaging and not the glyphs, because
the glyphs were somebody else's to begin with. Once that is the question you ask, the field empties
fast.

| Font | The original bits | The dump or recreation | Verdict |
|---|---|---|---|
| Kaypro II `81-146a` | Kaypro Corp, 1982, no grant | Retroarchive and `ivanizag`, no license stated | **excluded** |
| DEC VT220, `htayj/DEC-Fonts` | DEC, ROM-derived via VT100.net | MIT, and the repo says it does not reach the glyphs | **excluded** |
| DEC VT220, GlassTTY | Slavinsky's own redrawing | Unlicense, public domain | **clean, but not a bitmap** |
| BBC Micro 8x8 | Acorn, in the MOS ROM | Linux's copy is GPL-2.0; others are ROM extractions | **excluded** |
| Atari ST 8x16, `ntwk/atarist-font` | Atari, the TOS high-res font | BSD-3 over what its own README calls a rebranding | **excluded** |
| VileR's Oldschool PC pack | various | CC BY-SA 4.0 | **excluded, share-alike** |

`htayj/DEC-Fonts` is the one that handles this correctly, and it is worth reading for that alone. Its
`README.org` says the fonts "incorporate historical DEC VT220 glyph data from a ROM-derived image
published by VT100.net". Then, in the same paragraph, it says that "The MIT grant covers only rights
held by the repository author and does not relicense pre-existing material." That is the honest form
of the sentence every other repository here leaves out. It also disqualifies the font, and the author
evidently knew that and wrote it anyway.

`ntwk/atarist-font` is the same situation without the sentence. Its `LICENSE` is a BSD-3-Clause
notice reading "Copyright 2015 ntwk". Its `README.md` describes the work as "a rebranding of the
high-resolution system font originally featured on the Atari ST home computer", and credits the file
it is based on to a third-party retro-fonts page. A notice cannot grant rights its author never held.
So this is Fixedsys Excelsior again: a third party's summary of someone else's license, which the
survey already refuses.

The BBC Micro is excluded twice over, which is worth stating because the obvious source is the trap.
Linux carries the font at `lib/fonts/font_acorn_8x8.c`, under the same `GPL-2.0` that already put
`font_8x16.c` out, and copyleft on a table compiled into every binary is settled here. Every other
copy found is an extraction from Acorn's MOS ROM with no license attached, which is the Kaypro answer.
There is no third source.

GlassTTY VT220 is the one clean license in the table, and still cannot be rendered here. Viacheslav
Slavinsky's `LICENSE` opens "This is free and unencumbered software released into the public domain".
Because it is his own redrawing rather than a ROM trace, that grant reaches the glyphs. But it ships
as a TrueType outline. Turning an outline into an 8-pixel bitmap needs a rasteriser and a threshold,
which is a design decision rather than a conversion: the specimen would then be a picture of our
hinting rather than of Slavinsky's font. Rendering it honestly means a FreeType or FontForge
dependency, and §46 (thin primitives or whole subsystems) says a dependency is a decision. It is left
as a proposed milestone rather than done badly.

What the survey is actually evidence for. The era's fonts are not available, and the two that are
already in this comparison are the exception rather than the sample. `unscii` is public domain because
its author redrew it, and `gohufont` is WTFPL because its author wrote it. A font this project can use
is one somebody made and gave away, not one a machine once displayed. That is the whole reason the
shortlist looks the way it does, and it will not change by looking harder.

None of these needs a loader, a rasteriser, a filesystem or an allocator, which is the property that
keeps them candidates at all. Anything that did would break the rendered picture as a pure function,
and with it the three-party agreement that proves the text on the screen.
