---
status: BUILT
raised: 2026-09-04
built: 2026-10-16
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 377. Two screendump decoders, in two crates, reading the same font

Filed as a proposal on 2026-09-04 by the milestone 243 lane, out of its own
duplication; promoted by milestone 433 (drain the proposal pile to zero, and keep it
there) on 2026-09-19. Built 2026-10-16 (UTC) on branch
`milestone/377-one-screendump-decoder-not-two`. The three functions had moved once before the
merge: they stood in `xtask/src/scanout.rs`, not at the `main.rs` lines named below. The one
decoder is `board_console::screen::decode` (name provisional). It takes the ink and paper
colors, an optional alphabet, and the scale the picture was painted at. `xtask`'s three are
gone. The swish-check leg reads through it, with the terminal's own colors and the surface
assertion kept at its call site.

It is a refactor with a test on each side; what it needs is a lane, not a decision.

**In brief.** There are now two pieces of code in this tree that read a QEMU screendump back into
text by matching 7x8 cells against `bitmap_font`:

- `xtask/src/main.rs`: `parse_ppm`, `decode_cell`, `scanout_rows`, written by milestone 177 for the
  graphical `swish-check` leg. Hardcoded to `graphics_protocol`'s surface geometry and to
  `video_terminal::Attr::DEFAULT`'s colors, and it takes an explicit alphabet.
- `crates/serial_console/src/screen.rs`, written by milestone 243 (a machine with no serial port has
  no way to say anything, and no gate can read it) for the framebuffer console's gate.
  Any geometry, any 24-bit PPM, `screen_console`'s colors, the whole printable alphabet, and its own
  host tests that paint with the crate the kernel links and read the result back.

They are the same function with different constants. That is the shape this tree's rule 7 exists to
refuse one level down. The reason it happened is ordinary: the second was written by a lane that had
read the crates and the scripts and not the eleven thousand lines of `xtask`. The first is not
where a reader would look for it.

## What it should be

One decoder, in `board_console::screen`, parameterised by the two things that actually differ (the
ink and paper colors, and optionally an alphabet), with `xtask`'s three functions deleted and
`scanout_rows`' geometry assertion kept at its call site where it belongs. `board_console` is the
right home: it is already the crate named "how a gate reads a machine it cannot see". It already
holds the recognizer both callers feed.

## Why it is worth doing rather than recording

Not for the lines. **A gate that carries its own copy of a definition is a gate that cannot
disagree with the thing it gates**, and there are now two copies of "what a character looks like on
this screen" that could drift apart in ways neither test would see. Milestone 243's version already
asserts itself against the kernel's own painter; milestone 177's asserts itself against a font
table. Merging them makes the stronger of those two the only one.

## The hazard

It touches milestone 177 (wire the graphical terminal stack into the real interactive boot) and its
graphical `swish-check` leg, which is a real gate on a real path, and the
colors differ between the two callers. A lane doing this should make the swish-check leg pass
before and after with no change to its assertions, and should keep the two color schemes as data
rather than unifying them. The terminal's default colors are `video_terminal`'s to choose and the
kernel console's are `screen_console`'s.

## Follow-on

- **None.**

## Index row

Two pieces of code in this tree read a QEMU screendump back into text by matching 7x8 cells against
`bitmap_font`. One is milestone 177's: three functions inside `xtask`, hardcoded to
`graphics_protocol`'s geometry and `video_terminal::Attr::DEFAULT`'s colours. The other is milestone
243's `board_console::screen`, which takes any geometry, any 24-bit PPM and the whole printable
alphabet and tests itself by painting with the crate the kernel links. They are the same function with
different constants, which is what rule 7 refuses one level down. It happened because the
second was written by a lane that had read the crates and the scripts and not eleven thousand lines
of `xtask`. The reason to merge them is not the lines: there are two copies of "what a character
looks like on this screen" that can drift apart in ways neither test would see, and only one of
them asserts itself against the kernel's own painter. The hazard is that it touches milestone 177's
graphical `swish-check` leg, so the two color schemes stay data rather than being unified, and the
leg must pass before and after with no change to its assertions.
