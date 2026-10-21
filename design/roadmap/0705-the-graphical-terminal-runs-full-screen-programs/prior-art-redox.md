# Prior art: Redox wrote its own engine, then replaced it

This appendix belongs to [milestone 705 (the graphical terminal runs full-screen programs)](../0705-the-graphical-terminal-runs-full-screen-programs.md).
It holds the one case found of an operating system project doing what option A proposes, and what
happened. [The engine plan](engine-plan.md) draws its conclusion from it.

Read on 2026-10-21 (UTC) from `gitlab.redox-os.org` (the `ransid`, `orbterm`, `base` and `redox`
repositories) and the crates.io copy of `ransid` 0.5.0. `ransid` was built and graded with the same
recordings as the other engines; nothing else from Redox was run.

## What `ransid` is

"Rust ANSI Driver", MIT, by Jeremy Soller. It parses with `vte` 0.8 and keeps the cursor, the
rendition, the region and a handful of modes. It holds no grid. Every effect leaves as a drawing
event (`Char`, `Rect`, `Move`, `ScreenBuffer`, `Input`), and the program using it keeps the cells.
It is 941 lines in two files, with no tests.

Redox used it in three places: `orbterm` (the desktop terminal) and, in `base`, the text console
`fbcond`, the boot log `fbbootlogd` and the VESA driver, through a shared `console-draw` crate.

## Why it stalled

The commit history, 76 commits in all:

| years | commits | what they were |
|---|---|---|
| 2016 and 2017 | 61 | the engine as it stands |
| 2018 | 6 | underline and strikethrough from a contributor, a release |
| 2021 and 2022 | 5 | scrolling, `Debug`, the move to `vte` 0.8 |
| 2025 | 4 | Clippy fixes and comments naming escape sequences |

Seventy of the 76 are by its author. Nothing after 2017 added a sequence.

What it never implemented, read from the 0.5.0 source:

- insert and delete line (`CSI L`, `CSI M`), erase character (`CSI X`), insert mode and REP;
- deferred wrap: the code for it is present and switched off (`let xenl = false`);
- tab stops other than every eight columns;
- character sets: the G0 and G1 fields exist and are never consulted, and SO and SI are ignored;
- the difference between a private mode and an ANSI one: `CSI h` reads the first parameter and
  ignores the `?` (a `TODO: Check intermediate` marks it), so `CSI 4 h` and `CSI ? 4 h` are the same;
- wide and combining characters, DECRQM, DA2, OSC color queries. Any `CSI c`, DA2 included,
  answers `CSI ? 6 c`.

Its region scroll carries two `TODO: Use min and max to ensure correct behavior` comments.

Measured with the plan's recordings: 14 of vttest's 24 screens (it fails 1.3, 1.5, 2.2, 2.7, 2.9,
2.11, 2.15, 8.2, 8.3 and 8.4) and 40 of the census's 97 checkpoints. That is ahead of ours today (7
and 13) and behind `vt100` (17 and 80).

The reading, which is an inference from the record and not something Redox wrote: one person built
it in two years. It had no tests and no conformance run to say what was missing. Because it emitted
drawing events and not a grid, nothing could hold its output as a value and compare it. Once it
drew Redox's own programs well enough, nothing measured it again.

## What replaced it

- The desktop terminal: COSMIC Terminal. `redox` commit ec1dbccb01 (2024-05-06), "Use
  cosmic-term for desktop instead of orbterm". `orbterm` left the desktop configuration on
  2026-03-04. It still exists, still on `ransid` 0.5.0.
- The text console: `alacritty_terminal`. `base` commit f15cee2ff7 (2026-09-02),
  "drivers/graphics/console-draw: Switch from ransid to alacritty_terminal". Its message: it
  "allows removing a fair amount of custom code" and adds reflow on resize and more escape codes.
  The change was 480 lines added and 368 removed; 232 of the added lines are `Cargo.lock`.

So Redox's console today is `fbcond` drawing through `console-draw`, on `alacritty_terminal` 0.26.0.

## Whether `alacritty_terminal` needed a Redox patch

No. `base` takes 0.26.0 from crates.io with `default-features = false`, and its `Cargo.toml` has no
`[patch]` entry for it, `vte` or `polling`. COSMIC Terminal takes 0.25.1 from crates.io, and Redox's
recipe builds it unmodified with no patch directory.

It builds unpatched because Redox's support is upstream in the crates under it. `polling` 3.11 has
a Redox backend (it uses its epoll path), and `libc` and `rustix` know Redox. The patches Redox did
carry were elsewhere: `drm`, `drm-sys` and `rustix`, removed on 2026-03-06 once releases with Redox
support shipped ("Remove no longer necessary patches").

nife is in the other position. `polling` refuses nife's target, which is why option F had to cut
the pty and event loop out of `alacritty_terminal` ([option F](option-f.md)). Redox's route works
because Redox is a POSIX-like target that upstream crates support; nife is not one.

## What it means for option A

`ransid` is option A's shape: an operating system's own engine, small, written for its own
programs. Redox's experience says such an engine stalls once it draws its author's programs, and that
the exit is a mature engine. [The engine plan](engine-plan.md#what-keeps-a-from-stalling) answers
each cause in turn.
