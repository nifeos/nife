# The engine plan under option A

This appendix belongs to [milestone 705 (the graphical terminal runs full-screen programs)](../0705-the-graphical-terminal-runs-full-screen-programs.md).
It holds the build plan for option A, extending `crates/video_terminal`: what the engine must do,
in which order, and how each step is proved. Its measurements are in [the programs'
census](program-census.md), [the reference engines](engine-matrix.md) and [Redox's
experience](prior-art-redox.md). calef's rulings on it are quoted where they apply. This file's
name is provisional.

## The target

calef, 2026-10-21 02:24 UTC: "Advertise xterm-256color plus COLORTERM=truecolor is the decision."
So the engine is a faithful xterm. It honors every capability the `xterm-256color` terminfo entry
promises, and 24-bit color, which it already draws. Features a program negotiates by query or mode
whatever TERM says (synchronized output, bracketed paste, focus events, the kitty keyboard protocol,
hyperlinks) are planned as their own steps.

The programs set the order. Ten were recorded: vim, Neovim, `less`, `top`, `htop`, `nano`, an
ncurses test, Emacs, Midnight Commander and tmux. `nano` is the port of milestone 170 (`nano`: a
real, full-featured screen editor). Replayed through today's engine, 13 of 97 checkpoints match the
reference screen; `vt100` matches 80. vttest passes 7 of 24 screens.

Four findings shape the order.

- `ESC ( B` prints a stray `B` today. It is in every ncurses program's `sgr0`, so `htop`'s first
  screen carries 404 of them. It is a three-byte sequence the parser ends after two.
- After that, cursor addressing (`CSI G`, `CSI d`, ECH) and scrolling regions account for most
  failures. With ASCII content the census predicts 79 of 97 checkpoints match after step 2.
- The replies a terminal gives decide what it is sent. Told "xterm", Neovim and tmux use left and
  right margins and synchronized output, and vim sends 37 capability queries. Told nothing, they
  send none of it.
- No program hung without replies. Neovim waits one second at exit for DA1, and Emacs waits 0.21 s
  at startup for DA2.

## The entry's capabilities, by step

The entry read is ncurses 6.6's `xterm-256color`.

| step | output capabilities |
|---|---|
| today | `bel bold dim rev invis sitm ritm smso rmso smul rmul blink` (parsed, not drawn), `setaf setab op AX`, `clear ed el el1 cup home cub cuf cuu cud` and their one-step forms, `cr ht xenl msgr npc` |
| 1 | `am smam rmam`, `civis cnorm cvvis`, `hpa vpa ech ind nel`, `smkx rmkx`, `km smm rmm`, `Ss Se`, and `sgr0`'s `ESC ( B` |
| 2 | `csr ri indn il il1 dl dl1`, and `indn`'s reverse (SD by a count) |
| 3 | `ich dch dch1 smir rmir mir rep bce` |
| 4 | `smcup rmcup sc rc`, and the soft reset in `is2 rs2` |
| 5 | `hts tbc cbt`, `acsc smacs rmacs` |
| 6 | `u6 u7 u8 u9 RV XR` |
| 8 | `ccc initc oc`, `flash`, `E3`, `rs1` |
| 9 | `smglp smglr smgrp mgc` |
| 10 | `mc0 mc4 mc5 mc5i`, `meml memu` |
| 11 | `XM kmous` |
| 12 | `Ms` |
| 14 | `BE BD PS PE` |
| accepted | `XT` (titles; there is no title bar), `Cs Cr` (cursor color) |

## How the engine names itself

calef, 2026-10-21 03:32 UTC, ruling the reply identity: "A". The engine answers as itself. DA1 is
`CSI ? 62 ; 22 c` (VT220 class with ANSI color) and DA2 `CSI > 1 ; 0 ; 0 c`, the defaults
libghostty-vt's engine uses too. XTVERSION names nife and its version, and DECRQM answers each mode
by what the engine implements at that commit. Programs adopt each negotiated feature the day its
step lands, and never before.

vim's and Emacs's version heuristics see an unknown terminal, an effect inferred and not measured.
Step 6 runs vim, Neovim, Emacs and tmux against the exact strings before it merges.

## When TERM changes

calef, 2026-10-21 03:35 UTC: "A". `system_initializer` gives every program `TERM=dumb` until step 10
lands, then `TERM=xterm-256color` and `COLORTERM=truecolor`. `XM`, `kmous` and `Ms` are recorded in
`BUGS` until milestones 179 (pointer input) and 180 (a clipboard) land, since steps 11 and 12 wait
on them. Every program in the census works by keyboard, and none used the clipboard.

## The steps

Each step is one pull request. "Lines" are engine and test lines as this tree writes them, comments
included. The predicted counts are upper bounds from the census replay.

| step | adds | turns green | lines (engine / tests) |
|---|---|---|---|
| 0a | replay harness and committed recordings | measures; nothing changes | 450 of tooling |
| 0b | fuzz target; first Kani harnesses | nothing | 160 |
| 1 | parser repairs, cursor addressing, small modes | vttest 9 of 24; census 13, or 17 if ASCII | 270 / 300 |
| 2 | scrolling regions and origin mode | vttest 17; census 25, or 79 if ASCII | 300 / 350 |
| 3 | insert and delete in place | vttest 22; `top` | 120 / 150 |
| 4 | alternate screen, saved cursor, soft reset | `nano`, Emacs, tmux; census 33, or 96 if ASCII | 260 / 250 |
| 5 | tab stops and line drawing | vttest 24 of 24 | 200 / 180 |
| 6 | the reply path | vttest's reports; Neovim's exit | 250 / 200 |
| 7 | wide and combining characters | census 96 of 97 with the CJK file | 180 / 200 |
| 8 | palette, reverse screen, scrollback clear | the ncurses test; 97 of 97 | 120 / 120 |
| 9 | left and right margins | margin-aware programs, once DECRQM says so | 250 / 300 |
| 10 | printer controller mode, memory lock | the entry, complete but for 11 and 12 | 80 / 80 |
| 11 | mouse reporting, SGR encoding included | `htop`, Midnight Commander, tmux, Neovim by mouse | 200 / 200 |
| 12 | OSC 52 | clipboard writes from vim and tmux | 60 / 60 |
| 13 | synchronized output | Neovim and tmux without tearing | 60 / 80 |
| 14 | bracketed paste | pastes arrive bracketed | 40 / 40 |
| 15 | focus events | Neovim, vim and tmux see focus | 30 / 40 |
| 16 | kitty keyboard protocol | Neovim's unambiguous keys | 250 / 250 |
| 17 | modifyOtherKeys | vim's and Emacs's modified keys | 80 / 80 |
| 18 | OSC 8 hyperlinks | links kept with their cells | 120 / 100 |

Step 0a, the replay harness. A host command in `xtask` (name provisional) replays committed
recordings through `Vt` and compares each checkpoint's grid with the oracle's (below). A
recording is the bytes a program wrote plus the offsets of its keystrokes, so grading needs no pty
and no installed program and runs in CI in milliseconds. The recordings are the ten census sessions
and vttest's menus 1, 2 and 8, about 240 KB. A capture helper records new sessions. The harness
fails when any count drops.

Step 0b adds the fuzz target and the first proofs over today's engine. Kani on a 672 KB value is untried;
this step finds out whether it holds the whole `Vt` or only extracted pieces.

Step 1. The parser consumes an `ESC` sequence with intermediates whole, executes C0 codes inside a
CSI, treats VT and FF as line feed, and dispatches private markers. Then CHA, HPA, VPA, HPR, VPR,
CNL, CPL, ECH, IND, NEL, DECALN, DECTCEM, DECAWM, DECSCUSR accepted, and DECCKM and DECKPAM in
`Vt`. `keymap` reads them: with `smkx` the entry promises `kcuu1=\EOA`, and ours sends `CSI A` in
every mode.

Step 2. DECSTBM, RI, SU, SD, IL and DL in the region, region-aware line feed and DECOM. This is where
[engine options](engine-options.md) put the risk: `line_feed`, the damage record and the scrollback
ring. Only a whole-screen scroll pushes a row to scrollback.

Step 3. ICH, DCH, IRM and REP, with erasure filling with the current background only (BCE). Today an
erase fills with the whole rendition.

Step 4. Modes 47, 1047, 1048 and 1049; DECSC, DECRC, SCOSC and SCORC with position, rendition,
origin mode, the wrap flag and character sets; DECSTR. The second grid is 192,000 bytes more in
every holder of a `Vt`: `display_terminal`'s `.data` (717 KB today), the kernel's witness and
`xtask`. Each step that adds state bumps the handoff `LAYOUT` and keeps a reader for the previous
one. calef's 2026-10-19 ruling on §209 (state handoff is an opaque blob over a granted frame, and it
is optional) requires that.

Step 5. HTS, TBC, CBT, CHT; G0 and G1, SO and SI, and DEC Special Graphics mapped to Unicode box
drawing. The atlas is ASCII, so the glyphs belong to milestone 142 (a text display good enough
that people use it instead of a GUI); the grid is right after this step.

Step 6. Part 3's design: `Vt::take_reply` hands back a bounded buffer and clears it, as
`take_damage` does, and `display_terminal` forwards it where a keystroke goes. A flood drops the
newest reply and says so in `BUGS`. DA1, DA2, DSR 5 and 6, DECXCPR, DECRQM, XTVERSION, `CSI 18 t`,
OSC 10 and 11 queries, and DECRQSS and XTGETTCAP, answered as above. Replies do not change the
grid, so the witness is untouched.

Step 7. Widths come from `unicode-width` (below). A wide character takes two cells and wraps whole, and
erasing either half clears both. A combining mark joins the previous cell; a cell holds one `char`,
so the mark is dropped and the columns stay right until a cell can hold more.

Step 8. OSC 4 and 104, DECSCNM, ED 3, and DECCOLM as clear and home with no change of width.

Step 9. DECLRMM and DECSLRM. Every operation from steps 2 and 3 learns the left and right margins,
which is why it is large; libvterm and libghostty-vt have it, neither Rust engine does.

Step 10. Printer controller mode swallows text until `CSI 4 i`, `mc0` prints nothing (there is no
printer), and HP memory lock (`ESC l`, `ESC m`) keeps the rows above the cursor from scrolling.

Step 11. Modes 9, 1000, 1002 and 1003 with encodings 1005, 1006 and 1015. It waits on milestone 179
(pointer input) to route clicks and motion to `display_terminal`.

Step 12. OSC 52 sets a selection. It waits on milestone 180 (a clipboard) for somewhere to put it.

Steps 13 to 18 each add one negotiated feature with its DECRQM or query answer, so a program uses it
only after asking. Bracketed paste waits on milestone 180; focus events need a window to lose focus,
which milestone 632 (graphics on demand) provides.

## What the C programs need from the terminal layer

Measured on `main` at d123bc55d: the C library maps neither onto the terminal contract today.
`vendor/relibc/header` has no `termios` module, so `tcgetattr` and `tcsetattr` do not exist.
`ioctl` answers every request, `TIOCGWINSZ` included, with `ENOTTY`, and `isatty` is always false.
Milestone 170 owns all three: its stage 1 adds `termios.h` over the line editor's raw mode and maps
`TIOCGWINSZ` onto a new window-size operation (calef's rulings of 2026-10-18). On the display
terminal that size comes from the surface, so it needs nothing from this engine. This plan does not
duplicate that work.

## The witness, fuzzing and Kani

There is one engine, so the three witnesses (the component draws, the kernel predicts, `xtask`
grades the scanout) agree by construction. Each step adds its sequences to `script.rs`, so the
kernel suite proves them on all three architectures, and reports what that costs.

The fuzz target feeds arbitrary bytes into `Vt::feed` and checks after every call:

- no panic;
- cursor, region and damage inside the grid, and the wrap flag only at the last column;
- the same grid however the bytes are split between calls;
- the same grid after a handoff written mid-stream and absorbed.

Kani proves:

- no panic and no out-of-bounds index for every input up to a length step 0b measures;
- the grid invariants after any step;
- the arithmetic of region clamps, region scrolls, tab stops, margins and width lookup,
  exhaustively, on extracted pure functions;
- that leaving the alternate screen restores the primary grid and cursor exactly.

## The test oracle

calef, 2026-10-21 03:37 UTC: "A". `alacritty_terminal` 0.26.0 is the replay harness's oracle, a
host-only dev-dependency under §46 (thin primitives or whole subsystems; we write everything in
between). Its two failing vttest screens, 2.7 and 2.9, are allowlisted with reasons. calef's
2026-10-19 hold on its §46 take stands, so step 0a grades against committed expected screens until
the hold lifts.

## Character widths

calef, 2026-10-21 UTC: widths come from the `unicode-width` crate, which takes ordinary dependency
updates like any other ("yes", 03:56 UTC). A width table maintained in the tree is refused: "We are
not going to maintain A. I don't even know how we would know to change it."

The engine and `alacritty_terminal` 0.26.0 both ask for 0.2, so the workspace's one `Cargo.lock`
gives them the same copy. The crate reaches the kernel's graph through the witness; the ruling is
the §46 take.

## What keeps A from stalling

Redox's own engine, `ransid`, is option A's shape, and it stalled: 61 of its 76 commits were in its
first two years, no sequence was added after 2017, and it never had a test. Redox replaced it with
`alacritty_terminal` in its console and with COSMIC Terminal on its desktop. Each of its causes has
an answer here.

- No tests and no conformance run, so nothing said what was missing. Step 0a puts vttest and the
  census into CI before any engine change, and its counts cannot drop.
- It emitted drawing events, so its output could never be compared as a value. `Vt` is a value,
  checked by three witnesses, a differential oracle, a fuzz target and Kani.
- Its target was its author's own programs. This one is a terminfo entry calef ruled on, every
  capability assigned to a step above.
- One author. These steps are lanes, each a pull request that a stranger can run the harness on.

What would make A the wrong call is the same thing that ended `ransid`: the steps stop landing. The
exit is written here so nobody has to rediscover it. It is Redox's, `alacritty_terminal` behind
`display_terminal`, which is option F with its held take. Unlike Redox, nife would have to carry the
pty cut, because `polling` does not know nife's target.

## The estimate, revised

[Engine options](engine-options.md) estimates 500 to 900 lines of engine code. Counted here: about
2,900 engine lines in all. Steps 1 to 12, the faithful xterm, are about 2,300 of them; steps 13 to 18,
the negotiated features, about 600. Around them sit about 3,000 lines of tests, 450 of harness and 250
of fuzz and proofs.

The figure scales the same functions in `alacritty_terminal` 0.26.0 by 1.5 for this tree's comment
density (`vt.rs` is 1,361 lines, 883 of code). `alacritty_terminal` lacks step 9; libvterm's
`state.c` names the left and right margins on 53 lines, which sized it. Read it as 2,200 to 3,600. It covers `xterm-256color` whole, 24 of 24
vttest screens, the reply path and every census program's screens. [The
census](program-census.md#where-this-census-is-thin) says what it did not reach.
