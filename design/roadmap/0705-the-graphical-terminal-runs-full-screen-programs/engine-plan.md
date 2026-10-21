# The engine plan under option A

This appendix belongs to [milestone 705 (the graphical terminal runs full-screen programs)](../0705-the-graphical-terminal-runs-full-screen-programs.md).
It holds the build plan for option A, extending `crates/video_terminal`: what the engine must do,
in which order, and how each step is proved. Its measurements are in [the programs'
census](program-census.md) and [the reference engines](engine-matrix.md). Five decisions are
calef's and are marked where they arise. This file's name is provisional.

## Where the target comes from

The target is what the programs send and what the advertised terminfo entry promises. Another
engine's feature list is not the target, because it records what that engine's users asked for. So
the order of work is: measure the programs, choose the entry, list its capabilities, then read the
reference engines for behavior and for anything the programs need that the census missed.

Ten programs were recorded: vim, Neovim, `less`, `top`, `htop`, `nano`, an ncurses test, Emacs,
Midnight Commander and tmux. `nano` is the port of milestone 170 (`nano`: a real, full-featured
screen editor). Replayed through today's engine, 13 of 97 checkpoints match the reference screen;
`vt100` matches 80. vttest passes 7 of 24 screens.

Four findings shape the plan.

- `ESC ( B` prints a stray `B` today. It is in every ncurses program's `sgr0`, so `htop`'s first
  screen carries 404 of them. It is a three-byte sequence the parser ends after two.
- After that, cursor addressing (`CSI G`, `CSI d`, ECH) and scrolling regions account for most
  failures. With ASCII content the census predicts 79 of 97 checkpoints match after step 2.
- The replies a terminal gives decide what it is sent. Told "xterm", Neovim and tmux use left and
  right margins and synchronized output, and vim sends 37 capability queries. Told nothing, they
  send none of it.
- No program hung without replies. Neovim waits one second at exit for DA1, and Emacs waits 0.21 s
  at startup for DA2.

## Decision 1: the TERM nife advertises

Today `system_initializer` sets `TERM=dumb`. The engine must honor every capability of the entry it
advertises, so this choice is the list.

- **A. `xterm-256color`** (recommended). Every system that runs a program over ssh has it, and the
  programs measured sent the same sequences under it as under `xterm`, so 256 colors cost nothing
  the engine does not already do. Its cost is a short list of capabilities the plan does not
  implement (below), which must be accepted as exceptions or implemented.
- **B. A nife entry**, `xterm-256color` minus those exceptions. Exact, but it exists only where
  nife installs it. An ssh session into Linux sends the name, the remote host has no entry, and
  ncurses refuses to start. Users then set `TERM` by hand, which is the failure B exists to avoid.
  Its name would be calef's.
- **C. `xterm`.** The same capabilities with 8 colors. No reason to prefer it: the engine already
  draws 256 and 24-bit color.

The entry read is ncurses 6.6's `xterm-256color`. Its output capabilities, by the step that
honors them:

| step | capabilities |
|---|---|
| today | `bel bold dim rev invis sitm ritm smso rmso smul rmul blink` (parsed, not drawn), `setaf setab op` and `AX`, `clear ed el el1 cup home cub cuf cuu cud` and their one-step forms, `cr ht xenl msgr npc` |
| 1 | `am smam rmam`, `civis cnorm cvvis`, `hpa vpa ech ind nel`, `smkx rmkx`, `Ss Se`, and `sgr0`'s `ESC ( B` |
| 2 | `csr ri indn il il1 dl dl1`, and `indn`'s reverse (SD by a count) |
| 3 | `ich dch dch1 smir rmir mir rep bce` |
| 4 | `smcup rmcup sc rc`, and the soft reset in `is2 rs2` |
| 5 | `hts tbc cbt`, `acsc smacs rmacs` |
| 6 | `u6 u7 u8 u9 RV XR` |
| 8 | `ccc initc oc`, `flash`, `E3`, `rs1` |
| accepted and ignored | `XT` (titles; there is no title bar), `Cs Cr` (cursor color), `km smm rmm` (meta), `BE BD PS PE` (bracketed paste; nife has no paste to bracket) |
| exceptions | `mc0 mc4 mc5 mc5i` (a printer), `meml memu` (xterm's memory lock), `smglp smglr smgrp mgc` (left and right margins), `XM kmous` (mouse, decision 5), `Ms` (clipboard) |

The exceptions are safe only together with decision 2. No program measured used margins or the
clipboard unless told it was talking to xterm, and ncurses does not use the printer or margin
capabilities to draw.

## Decision 2: how the engine names itself

Every reply is a format programs read, so it is calef's.

- **A. An honest identity** (recommended). DA1 `CSI ? 62 ; 22 c` (VT220 class with ANSI color), DA2
  `CSI > 1 ; 0 ; 0 c`, XTVERSION naming nife and its version, and DECRQM answering 0 for every
  mode the engine does not implement. libghostty-vt's engine uses the same DA1 and DA2 by default.
  Measured effect: the programs send the no-reply set, which is the plan's target.
- **B. Claim xterm**, as the census's xterm-reply runs did. Programs then send DECSLRM,
  synchronized output and capability queries, which the plan would have to implement or the
  screen is wrong.

Under A, vim's and Emacs's version heuristics see an unknown terminal. Step 6 runs vim, Neovim,
Emacs and tmux against the exact strings before it merges, since that effect was inferred and not
measured.

## The steps

Each step is one pull request. "Lines" are engine and test lines as this tree writes them, comments
included; how they were estimated is under "The estimate, revised". The predicted counts are upper
bounds from the census replay.

| step | adds | turns green | lines (engine / tests) |
|---|---|---|---|
| 0a | host replay harness and committed recordings | measures; nothing changes | 450 of tooling |
| 0b | fuzz target; first Kani harnesses | nothing | 160 |
| 1 | parser repairs, cursor addressing, small modes | vttest 1.5, 2.1 (9 of 24); census 13, or 17 if ASCII | 270 / 300 |
| 2 | scrolling regions and origin mode | vttest 17 of 24; census 25, or 79 if ASCII | 300 / 350 |
| 3 | insert and delete in place | vttest 22 of 24; `top` | 120 / 150 |
| 4 | alternate screen, saved cursor, soft reset | `nano`, Emacs, tmux; census 33, or 96 if ASCII | 260 / 250 |
| 5 | tab stops and line drawing | vttest 24 of 24 | 200 / 180 |
| 6 | the reply path | vttest's reports; Neovim's exit | 250 / 200 |
| 7 | wide and combining characters | census 96 of 97 with the CJK file | 180, plus a generated table / 200 |
| 8 | palette, reverse screen, scrollback clear | the ncurses test; 97 of 97 | 120 / 120 |

### 0a. The replay harness

A host command in `xtask` (name provisional) that replays committed recordings through `Vt` and
compares each checkpoint's grid with the oracle's (decision 3). A recording is the bytes a program
wrote plus the byte offsets of its keystrokes. Grading needs no pty and no installed program, so
it runs in CI in milliseconds. A capture helper records new sessions on a developer's machine.

The recordings are the ten census sessions and vttest's menus 1, 2 and 8, about 240 KB. They are
recorded under the reply identity of decision 2, and re-recorded once step 6 can answer.

Tests: the harness reports today's 13 of 97 and 7 of 24, and fails when a count drops.

### 0b. The fuzz target and the first proofs

A `fuzz/` target over `Vt::feed` (below), seeded with the recordings, and the first Kani harnesses
over today's engine. Kani on a 672 KB value is untried; this step finds out whether it holds the
whole `Vt` or only the extracted pieces, before any step depends on it.

### 1. Parser repairs, cursor addressing, small modes

- The parser: an `ESC` sequence with intermediates (`ESC ( B`, `ESC # 8`) consumed whole; C0 codes
  inside a CSI executed and the sequence continued; VT and FF as line feed; private markers parsed
  and dispatched, which every later mode needs.
- CHA, HPA, VPA, HPR, VPR, CNL, CPL, ECH, IND, NEL, DECALN.
- DECTCEM drives the existing `set_cursor_visible`; DECAWM; cursor blink and DECSCUSR accepted.
- DECCKM and DECKPAM in `Vt`, read by `keymap` to send `ESC O A` in application cursor mode. With
  `smkx` the entry promises `kcuu1=\EOA`, and ours sends `CSI A` in every mode.

Tests: one per sequence in `vt/tests.rs`, plus `keymap` tests for both cursor modes.

### 2. Scrolling regions and origin mode

DECSTBM, RI, SU, SD, IL and DL inside the region, region-aware line feed and DECOM. The risk named
in [engine options](engine-options.md) lives here: `line_feed`, the damage record and the
scrollback ring. Only a scroll of the whole screen pushes a row to scrollback, as in every reference
engine. The damage rectangle of a region scroll covers the region, not the screen.

Tests: region edge cases taken from vttest 2.7 to 2.10 (where `alacritty_terminal` is wrong and
`vt100` right), damage bounds, scrollback untouched by a region scroll.

### 3. Insert and delete in place

ICH, DCH, IRM and REP, with erasure filling with the current background (BCE). Today an erase fills
with the whole current rendition; xterm and `alacritty_terminal` take only the background.

### 4. Alternate screen, saved cursor, soft reset

Modes 47, 1047, 1048 and 1049; DECSC and DECRC (and SCOSC and SCORC), saving position, rendition,
origin mode, the wrap flag and character sets; DECSTR; `CSI 22 t` and `23 t` consumed. The second
grid is 9,600 cells of 20 bytes, 192,000 bytes more in every holder of a `Vt`: `display_terminal`'s
`.data` (717 KB today), the kernel's witness and `xtask`. The alternate screen keeps no scrollback.

Each step that adds state (1, 2, 4, 5, 7) bumps the handoff `LAYOUT` and keeps a reader for the
previous layout. calef's 2026-10-19 ruling on §209 (state handoff is an opaque blob over a granted
frame, and it is optional) requires it.

### 5. Tab stops and line drawing

HTS, TBC, CBT, CHT; G0 and G1 designation, SO and SI, and DEC Special Graphics mapped to their
Unicode box-drawing characters. Drawing those needs glyphs the atlas does not have: its repertoire
is ASCII, and widening it is milestone 142 (a text display good enough that people use it instead
of a GUI). The grid is right after this step; the pixels follow 142.

### 6. The reply path

Part 3's design as written: `Vt::take_reply` hands back a bounded buffer and clears it, as
`take_damage` does, and `display_terminal` forwards it where a keystroke goes. A flood drops the
newest reply and says so in `BUGS`. Replies: DA1, DA2, DSR 5 and 6, DECXCPR, DECRQM, XTVERSION,
`CSI 18 t`, OSC 10 and 11 queries answered from the palette, and DECRQSS and XTGETTCAP answered
"invalid", all under decision 2. Replies do not change the grid, so the witness is untouched.

Tests: each reply's bytes; the drop on overflow; the component's forwarding in the QEMU suite.

### 7. Wide and combining characters

Width from a table (decision 4); a wide character takes two cells, wraps whole at the margin, and
overwriting or erasing either half clears the other. A combining mark joins the previous cell. A
cell holds one `char`, so a mark is kept only if a later step widens the cell; until then it is
dropped and the columns stay right, which is what programs depend on.

### 8. Palette, reverse screen, scrollback clear

OSC 4 and 104 (`initc`, `oc`), DECSCNM (`flash`), ED 3, and DECCOLM as clear and home with no
change of width.

## The kernel's witness stays predictive

Under A there is one engine, so the three witnesses (the component draws, the kernel predicts,
`xtask` grades the scanout) keep agreeing by construction. What keeps them meaningful is that
`script.rs`, the shared script, exercises the new features. Each step adds its sequences to the
script, so the kernel suite proves them on all three architectures. The suite's cost grows with
the script; each step reports it.

## Fuzzing

One target, `video_terminal_feed`. It feeds arbitrary bytes and checks, after every call:

- no panic;
- the cursor inside the grid, the region inside the screen, the wrap flag only at the last column;
- the damage rectangle inside the grid;
- splitting the same bytes at any point gives the same grid (the parser carries its state);
- a handoff written mid-stream and absorbed into a fresh `Vt` gives the same grid.

## What Kani proves

- No panic and no out-of-bounds index for any input of up to N bytes from the reset state, N
  chosen by what step 0b measures Kani can hold.
- The grid invariants after any step: cursor column below `cols` and row below `rows`; region top
  below bottom, bottom at most `rows`; wrap flag only at the last column; `sb_len` at most capacity.
- The arithmetic, exhaustively on extracted pure functions: region clamps, the rows a region scroll
  moves, tab-stop operations, cursor clamps under origin mode, width lookup.
- Entering and leaving the alternate screen restores the primary grid and cursor exactly.

## Decision 3: an oracle in the tests

Grading needs an expected screen for every checkpoint. §46 (thin primitives or whole subsystems; we
write everything in between) counts a dev-dependency, so this is calef's.

- **A. `alacritty_terminal` 0.26.0 as a host-only dev-dependency of the replay harness**
  (recommended). It passes 22 of 24 vttest screens; its two failures go on an allowlist with
  reasons. It shares the hold calef put on its §46 take on 2026-10-19. Until that lifts, step 0a
  uses C.
- **B. `vt100` 0.17.0.** Five crates and no patch, but a weaker oracle: 17 of 24 vttest screens,
  and it disagrees with `alacritty_terminal` at 17 of the 97 census checkpoints.
- **C. No oracle in the graph.** Committed expected screens, reviewed by a person. It costs the
  same, but regenerating them needs an engine outside the tree, which a stranger with only this
  repository cannot run. That fails the third principle, which is why A is recommended, not effort.

## Decision 4: where character widths come from

`video_terminal` is in the kernel's dependency graph (`kernel/Cargo.toml`), through the witness.

- **A. A table generated in the tree** (recommended) from Unicode's `EastAsianWidth.txt` and the
  combining classes, with the generator committed and the Unicode version pinned. About 80 lines of
  generator and 500 of table, and no dependency reaches the kernel.
- **B. The `unicode-width` crate**, which `alacritty_terminal` and `vt100` both use. The tree's rule
  is to take existing code first, but this one enters the kernel's graph, and the crates Kani proves
  are the ones the rule says to write.

The recommendation is about the kernel's graph, not effort; the two cost about the same.

## Decision 5: what is out of 705

Recommended out, each with its reason:

- Mouse reporting. `htop`, Midnight Commander, tmux and Neovim turn it on, and all work by keyboard
  without it. It needs pointer input routed into `display_terminal`, which is its own milestone;
  this lane would propose it.
- Left and right margins, synchronized output, the kitty keyboard protocol, OSC 8 and 52. Asked
  for only after a terminal claims them (decision 2).
- 132 columns, double-size lines and VT52 (vttest menus 3, 4, 7), and resizing. The grid size is
  negotiated with the screen, and that contract is outside this engine.
- Glyphs beyond ASCII, which belong to milestone 142.

## The estimate, revised

[Engine options](engine-options.md) estimates 500 to 900 lines of engine code by analogy. Counted
here, the engine is about 1,700 lines: steps 1 to 8, with `keymap`, the handoff layouts and the
component's reply forwarding. Around it sit about 1,750 lines of tests and 450 of harness tooling.
The fuzz target and first proofs are 160 lines, growing to about 250, and the generated width
table is about 500.

The engine figure comes from the sizes of the same functions in `alacritty_terminal` 0.26.0,
scaled by 1.5 for this tree's comment density (`vt.rs` is 1,361 lines, 883 of code). On top is
what `alacritty_terminal`'s grid provides and ours must write: region-aware scrolling and damage. Read it as
1,300 to 2,100.

It covers `xterm-256color` less decision 1's exceptions, all 24 graded vttest screens, the reply
path, and every census program's screens with ASCII or wide text. It does not cover mouse, margins,
glyphs beyond ASCII, or anything the census did not reach; [the census](program-census.md) says
where it is thin.

## Open decisions

1. The TERM nife advertises: `xterm-256color` with the listed exceptions (recommended), a nife
   entry, or `xterm`.
2. The reply identity: honest VT220 class naming nife (recommended), or claim xterm.
3. The test oracle: `alacritty_terminal` as a host dev-dependency (recommended, under its hold),
   `vt100`, or none.
4. Character widths: a generated table in the tree (recommended), or `unicode-width`.
5. Scope: mouse, margins, synchronized output, 132 columns and glyphs out of 705 (recommended).
