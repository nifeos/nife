# The programs' census, widened

This appendix belongs to [milestone 705 (the graphical terminal runs full-screen programs)](../0705-the-graphical-terminal-runs-full-screen-programs.md).
It holds the measurements behind [the engine plan](engine-plan.md): what ten full-screen programs
send, what they ask for and wait for, how today's `video_terminal` draws them, and which vttest
screens fail for which feature. [The first census](census.md) covered vim, `less` and `top`; this
one repeats them under the TERM the plan proposes and adds seven.

## Method

Measured on 2026-10-21 (UTC) on patagonia, macOS 26.6.2, against `video_terminal` at `main`
810666a39. The harness was throwaway and is not kept; step 0 of the plan builds the kept one.

Each program ran in an 80x24 pty with `TERM=xterm-256color`, an empty home directory and a scripted
session of 3 to 22 keystrokes, 0.3 to 3.5 seconds apart. Every byte the program wrote was recorded.
Each session ran twice. In one, the harness answered queries as xterm 390 would (DA2
`CSI > 41 ; 390 ; 0 c`, DECRQM "recognized, reset" for every mode, XTVERSION `XTerm(390)`, colors
for OSC 10 and 11). In the other it answered nothing, which is what ours does today. vim, `less`,
the ncurses test and `htop` also ran under `TERM=xterm`.

The file the editors and pagers opened is `vt.rs` with one line prepended that holds CJK, a
combining accent and an emoji. So every session meets wide characters by construction, and the
counts of them say nothing about how often real files have them.

Each recording was then replayed through three engines, ours, `alacritty_terminal` 0.26.0 and
`vt100` 0.17.0, and the 24 rows of text compared at every keystroke. `alacritty_terminal` is the
reference: it passes 22 of vttest's 24 screens ([conformance](conformance.md)). It is not ground
truth, and `vt100` agrees with it at 80 of the 97 checkpoints below.

## The programs

| program | version | terminal library | session |
|---|---|---|---|
| vim | 9.1, macOS | ncurses 5.7, the OS terminfo | scroll, split, vertical split, delete, search, insert CJK, quit |
| Neovim | 0.12.6, Homebrew | its own | the same keys as vim |
| `less` | 668, macOS | ncurses 5.7 | page forward and back, `G` then `k`, search |
| `top` | macOS | ncurses 5.7 | two refreshes, sort, quit |
| `htop` | 3.5.3 | ncursesw 6.6 | tree view, move, help, page, quit |
| `nano` | 9.2 | ncursesw 6.6 | thirty lines down, page, insert CJK, cut, quit |
| ncurses test | a 60-line program written for this | ncursesw 6.6 | box, line drawing, 256 colors, a scrolling window, insert and delete, `flash` |
| Emacs | 31.1, `-nw -Q` | ncurses 6.6 | scroll, split, insert CJK, quit |
| Midnight Commander | 4.8.33, `-u` | S-Lang | move, switch panel, quit (it did not exit; the harness ended it) |
| tmux | 3.8, empty config | ncursesw 6.6 | split both ways, run commands and vim inside, close |

`nano` is the port of milestone 170 (`nano`: a real, full-featured screen editor) and builds on
ncurses through basalt. The `kilo` of milestone 169 (`kilo`: the smallest real text editor) was not
run; from memory it sends `CSI ? 25 l/h`, `CSI H`, `CSI K`, `CSI 2 J`, SGR 7 and `CSI 6 n`, all of
which steps 1 and 6 cover.

## What each program sends that ours does not act on

From the no-reply runs, which set the plan's order, because the plan recommends answering as itself
and not as xterm (the plan's decision 1). Each sequence is followed by the step that adds it. "No effect" lists what
ours swallows harmlessly: input-side modes, queries, cursor shape and the like.

| program | changes the screen | no effect when swallowed |
|---|---|---|
| vim | `?25` (1), `CSI L M r` (2), leaving `?1049` (4), wide and combining text (7) | `?1004 ?12 ?1 ?2004`, `ESC = >`, `>4;2m`, `?4m`, `0%m`, DSR 6, DA2, OSC 10 and 11 queries, `22;2t` |
| Neovim | `?25`, ECH `CSI X`, `ESC ( B` (1), `CSI L M r` (2), `?1049` (4), wide (7) | mouse `?1002 ?1006`, `?1004 ?2004 ?1`, DECSCUSR, DA1, DSR, DECRQM, kitty `?u`, XTGETTCAP, DECRQSS, OSC 11 query |
| `less` | `ESC M` (2), `?1049` (4), wide (7) | `?1`, `ESC = >` |
| `top` | `?7`, `CSI G d`, `ESC ( B` (1), `CSI r` (2), DCH `CSI P` (3), `?1049` (4) | `?1034 ?1`, `CSI 4 l` |
| `htop` | `?25 ?7`, `CSI G d`, `ESC ( B` (1), `CSI r` (2), `?1049` (4) | mouse `?1000 ?1006`, `?12 ?1`, `CSI 4 l`, `22;0;0t` |
| `nano` | `?25 ?7`, `CSI G d`, `ESC ( B` (1), `CSI S r` (2), `?1049`, `ESC 7 8` (4), wide (7) | `?12 ?1 ?2004`, `CSI 4 l` |
| ncurses test | `?25 ?7`, `CSI G d`, `ESC ( B` (1), `CSI T r` (2), `?1049`, `ESC 7 8` (4), `ESC ( 0` (5), wide (7), `?5` (8) | `?12 ?1`, `CSI 4 l` |
| Emacs | `?25`, `CSI d` (1), `CSI M r` (2), ICH `CSI @` (3), `?1049` (4), wide (7) | `?1004 ?12 ?1 ?2004`, DA1, DA2, XTVERSION |
| Midnight Commander | `ESC ( B` (1), `CSI r` (2), `?1049` (4) | mouse `?1002 ?1006`, `?2004 ?1`, `?1001s`, OSC 0 and 7 |
| tmux | `?25`, `CSI X d`, `ESC ( B` (1), `CSI S r` (2), `?1049` (4), wide (7) | mouse `?1000 ?1002 ?1003 ?1006`, `?2004 ?2031`, DA1, DA2, XTVERSION, DECRQM, DSR 996, `CSI 14 t` and `18 t`, OSC 10 and 11 queries |

Under `TERM=xterm` the four programs run both ways sent the same set, except that `htop` used ECH
once. The sessions are timing-dependent, so one sequence more or less is noise.

`ESC ( B` is the largest single defect. It is in `sgr0` for both entries, ours prints its `B`, and
`htop`'s first screen carries 404 of them. `CSI G` and `CSI d` (`hpa`, `vpa`) come next: ncurses 6
moves the cursor with them whenever they are shorter than `cup`.

## What claiming to be xterm changes

The xterm-reply runs sent these on top of the table above, and nothing else:

- Neovim: DECSLRM (`?69`, `CSI s` with margins), synchronized output `?2026`, and `?2027 ?2031
  ?2048`, each after DECRQM said the mode was recognized.
- tmux: DECSLRM, `?2026` and `>4;2m`, after XTVERSION named XTerm.
- vim: 37 XTGETTCAP queries, after DA2 said xterm 390.
- Emacs: modifyOtherKeys and the OSC 10 and 11 queries, after DA2.

So DECSLRM and synchronized output are features a terminal is asked for only after it says it has
them. An engine that answers DECRQM with 0 for a mode it lacks, and names itself, is not sent them.

## Queries, and which programs wait

| program | asks | effect of no answer, measured |
|---|---|---|
| vim | DSR 6 (twice), DA2, OSC 10 and 11 | none seen; screen settled in 0.02 s either way |
| Neovim | DA1, OSC 11, kitty `?u`, DECRQM 69 and 2048 | exit takes 1.00 s longer: it waits a second for DA1 |
| Emacs | DA2, then DA1 and XTVERSION | startup 0.21 s without an answer; 1.03 s with xterm's, which leads it to ask more |
| tmux | DA1, DA2, XTVERSION, DECRQM 2026, DSR 996, `14 t`, `18 t`, OSC 10 and 11 | none seen |
| `less`, `top`, `htop`, `nano`, ncurses test, Midnight Commander | nothing | not applicable |

No program hung without replies. The reply path buys correctness for the programs that ask (vim's
ambiguous-width probe, tmux's size check) and one second at Neovim's exit; it is not what makes
any of them start.

## Today's screens against the reference

Checkpoints are the screen just before each keystroke; the screen after the program exits is
left out. "Match" means all 24 rows of text equal the reference's. Attributes and colors are not
compared, so these numbers flatter every engine equally.

| program | checkpoints | ours matches | `vt100` matches | `ransid` matches |
|---|---|---|---|---|
| vim | 21 | 1 | 15 | 5 |
| Neovim | 21 | 0 | 15 | 0 |
| `less` | 12 | 6 | 10 | 10 |
| `top` | 3 | 0 | 3 | 3 |
| `htop` | 6 | 0 | 6 | 6 |
| `nano` | 7 | 0 | 6 | 6 |
| ncurses test | 1 | 0 | 0 | 0 |
| Emacs | 9 | 1 | 9 | 2 |
| Midnight Commander | 5 | 5 | 5 | 5 |
| tmux | 12 | 0 | 11 | 3 |
| total | 97 | 13 | 80 | 40 |

Midnight Commander matches because it draws with `cup`, `el`, SGR and UTF-8 box characters, all of
which ours handles. Its boxes draw as missing-glyph boxes, since the glyph atlas is ASCII
(widening the repertoire belongs to milestone 142 (a text display good enough that people use it
instead of a GUI), not to this engine).

The plan's predictions come from the same recordings. A checkpoint is predicted to match once every
screen-changing sequence sent before it is implemented. That is an upper bound: it assumes each
step is exact and ignores that a later repaint can hide an earlier mistake.

| after step | 0 | 1 | 2 | 3 | 4 | 5, 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|---|
| matches, of 97 | 13 | 13 | 25 | 28 | 33 | 33 | 96 | 97 |
| matches if the file had been ASCII | 13 | 17 | 79 | 82 | 96 | 96 | 96 | 97 |

## vttest, screen by screen

vttest 2.7 under `TERM=xterm`, menus 1, 2 and 8, the 132-column screens excluded, 24 screens.
Screens are numbered by their place in the menu, 132-column ones included, as in
[conformance](conformance.md). Ours passes 7: 1.6, 2.4, 2.6, 2.12, 2.13, 2.14 and 8.1. Redox's `ransid` passes 14 ([prior art](prior-art-redox.md)). 2.14 is
DECSCNM's light background; it passes on text alone and would fail a pixel comparison until step 8.

| screen | what it tests | what ours lacks, from the bytes on that screen | passes after step |
|---|---|---|---|
| 1.1 | the border of `E` and `*` | IND, NEL, DECALN (1); RI, DECSTBM reset (2) | 2 |
| 1.3 | autowrap mixed with controls | DECSTBM, DECOM (2) | 2 |
| 1.5 | control characters inside escape sequences | executing C0 inside CSI (1) | 1 |
| 2.1 | wraparound off and on | DECAWM (1) | 1 |
| 2.2 | tab stops | HTS, TBC (5) | 5 |
| 2.7 to 2.10 | soft and jump scroll inside a two-line region | DECSTBM, RI (2); `?4` is accepted and ignored | 2 |
| 2.11 | origin mode | DECSTBM, DECOM (2) | 2 |
| 2.15 | save and restore cursor | DECSC, DECRC (4) with G0 and SO/SI (5) | 5 |
| 8.2 | insert and delete line | IL, DL inside a region, DECOM (2) | 2 |
| 8.3 | insert mode | IRM (3) | 3 |
| 8.4 to 8.7 | delete and insert character | DCH, ICH (3); 8.6 also sends DECDWL, ignored as in `alacritty_terminal` | 3 |

So the count goes from 7 to 9 after step 1, 17 after step 2, 22 after step 3 and 24 after step 5.
vttest's reports (menu 6) wait on step 6. esctest was not run: it grades by asking the terminal
(cursor reports and DECRQCRA checksums), so it needs step 6 first, and only its cursor-report
subset fits an engine that does not implement rectangle checksums.

## Where this census is thin

- One scripted session per program, minutes long in total. Large parts of each program were never
  reached: vim's `:terminal`, Emacs's menus, `htop`'s setup screen, tmux's copy mode.
- macOS builds. vim, `less` and `top` use the OS's ncurses 5.7 and its older terminfo. Linux's
  procps `top` was not run; the first census read its source.
- No session over ssh into Linux. No Linux host was available to this lane. A program there emits
  what its own terminfo says for the TERM sent, so the ncurses 6.6 programs above are the closest
  proxy, not a measurement.
- The reply identity the plan recommends was not tried. The no-reply runs stand in for it, on the
  argument that a program not told "xterm" behaves as if told nothing. That is an inference.
- Input was not graded. Whether arrow keys reach `htop` and `nano` correctly in application cursor
  mode is read from terminfo (`kcuu1=\EOA`), not measured.
- `alacritty_terminal` is the reference and is wrong on two vttest screens; a checkpoint where it is
  wrong would count against ours here.
