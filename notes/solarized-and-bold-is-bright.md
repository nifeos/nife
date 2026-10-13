# Bold is bright, and Solarized's bright slots are grays

Written 2026-09-29 (UTC) by the resumed lane for milestone 141 (a palette worth looking at). Name
provisional, this file's included; calef names things.

This note is what the PALETTE BUGS entry in `crates/video_terminal/src/palette.rs` promises: the
options for bold under Solarized Dark, and a recommendation. The question was found on the way,
not raised by anyone, and the answer is calef's.

## The problem, measured

SGR 1 answers bold by setting the bright bit, not by borrowing a heavier face (the `Attr` rule,
recorded in notes/glyphs.md). Canonical Solarized Dark puts grays in most bright slots, as
Schoonover designed it, so bold on four of the eight colors leaves the hue entirely:

| color | bright slot | what bold paints |
|---|---|---|
| green (2) | 10 | `base01`, dark gray |
| yellow (3) | 11 | `base00`, body gray |
| blue (4) | 12 | `base0`, body gray |
| cyan (6) | 14 | `base1`, light gray |

Red, magenta and white keep real colors: orange, violet and `base3`. The daily damage is `ls
--color`, whose executables are bold green and whose directories are bold blue. Under this palette
both come out gray. grep and git hit the same wall less often; `ls` hits it on every listing.

The fault is nobody's alone. The bold rule predates the palette and is a recorded decision. The
gray arrangement is Schoonover's structural choice, and §104 (the palette is Solarized) keeps his
table canonical on purpose.

## The options

Each option names its cost. Three are refused; two are live.

1. Record it and wait. Milestone 142 (a text display good enough that people use it) widens
   rendition to 256-color and truecolour, and bold can become a real weight or a genuinely lighter
   color there. Cost until then: the commonest color-coded output in a shell is gray. This is
   today's behavior, so choosing it deletes nothing.
2. Brighten off-palette. Derive a lighter version of the entry's RGB at paint time. Refused. The
   color stops being one of the sixteen numbers the name Solarized covers, which is the
   look-versus-artifact line §104 (the palette is Solarized) draws. The three parties that agree on
   a table would have to agree on a derivation instead.
3. Remap the bright slots. Put lighter accent hues in slots 10, 11, 12 and 14, as several popular
   Solarized port files do. Refused. It departs from the published table in four more places on top
   of the recorded nudge, and "canonical", the word §104 (the palette is Solarized) ratified, stops
   meaning anything.
4. Minimum contrast at paint time. Bump the foreground whenever it sits too close to the
   background, the way iTerm2's option does. Refused. It is a perceptual rule in the pixel path,
   more machinery than the problem has earned, and every witness of the picture duplicates it.
5. Keep the hue when the bright slot is a gray. One const table beside PALETTE names the bright
   slots that hold grays; `Attr::colours` consults it, and bold on those four colors keeps the
   base color. Bold on red, magenta and white still lands on orange, violet and `base3`, which are
   Schoonover's own pairings. On-palette, deterministic, host-testable, and milestone 142 (a text
   display good enough that people use it) can replace it without ceremony. The cost is honest: a
   fact about the palette lives in the rendition code, and bold green will look identical to green.

## Recommendation

Option 5, if the daily case is to be fixed before milestone 142 (a text display good enough that
people use it) lands. Option 1 is the fallback and costs nothing, since it is what the tree already
does. The choice is calef's; with the options on a page it is one sentence long. Nothing milestone
141 (a palette worth looking at) delivered depends on the answer, and this note holds the fork so
the question does not live only in a conversation.
