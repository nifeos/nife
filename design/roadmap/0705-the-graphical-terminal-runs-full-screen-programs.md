---
status: NOT-STARTED
raised: 2026-10-02
promoted_from: the-graphical-terminal-runs-full-screen-programs
milestone_dependencies: 29, 142
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 705. The graphical terminal runs full-screen programs

*Title and slug are drafts.*
Raised by a maintainer-assigned lane, from calef's question of whether the graphical terminal can run
`vim`, `less` and `top`, and what the engine choice that §29 (framebuffer grant) deferred has to do
with it. Every claim below was checked in the tree or upstream on 2026-10-02 (UTC); a claim from memory
is marked as such.

The engine choice is a dependency decision (§46 (thin primitives or whole subsystems; we write everything in between)), so Part 4 gives options and
no recommendation (A to E; E was added at calef's request on 2026-10-03 UTC).

## Part 1: The gap, and the goal

`crates/video_terminal` (milestone 29, the display terminal) implements printable text with UTF-8, CR,
LF, BS, TAB, CSI `A B C D H f J K m` (SGR with 256-color and 24-bit) and RIS
(`crates/video_terminal/src/lib.rs:1373-1478`). Its own header lists what it leaves out: no scrolling
regions, no alternate screen, no origin mode, fixed tab stops, no mouse, no reporting sequences, no
reflow (`lib.rs:117-123`, `notes/glyphs.md` "Honest limits"). Anything else is swallowed whole
(`lib.rs:1373-1390`, `1475-1478`).

Milestone 142 (a text display good enough that people use it instead of a GUI) is PARTIAL and is about
glyph quality, scrollback, UTF-8 and grid size. None of its increments is about DEC-mode
compatibility, so this is not already owned.

Goal. `vim`-class editors, `less` and a repainting `top` draw correctly on the graphical terminal.
Today none of the three exists on nife.

- The only full-screen program in the tree is `rmle` (milestone 169 (the smallest real text editor, as the forcing function for raw terminal input)).
- There is no pager (milestone 334 (color and the pager: the spawn protocol's other two thirds) is NOT-STARTED).
- `watch` was cut (milestone 281 (`watch` holds exactly what `ps` holds, so it is nothing)).

So the terminal half and the program half are
separate work. This proposal is the terminal half; the programs arrive through whatever ports them
(milestone 198 (a package manager, and the trivial install that makes a second customer possible) is the road).

**Measure.** Two numbers, both taken before and after.

1. *Screens matching a reference* on the host, driving the real programs. A host harness runs the
   program in a pty, feeds its output to `video_terminal`, and compares the 80x24 grid with the same
   session captured from a reference terminal. I built a throwaway version of this for the inventory
   below, using `tmux capture-pane` as the oracle. It is the measure that exists today.
2. *`vttest` menus passed*, by menu. Menus 1 (cursor movement), 2 (screen features) and 6 (terminal reports) are the ones full-screen programs need, with 8 (VT102 insert and delete).
   Menu 2 holds scrolling regions, origin mode and tab stops; 3, 4, 5 and 7 (character sets, double-size, keyboard,
   VT52) are out of scope. The menu names are from memory; `vttest` is not installed here (Homebrew
   has 20251205).

**Can `vttest` run on nife today? No.** It is a C program over `termios`, `ioctl(TIOCGWINSZ)`, `select`
and signals. The tree has no libc and no `termios`. `grep termios` finds only a note about a serial
tool and a rejected crate in milestone 40 (documentation as a system service). The C seam is freestanding and makes no syscalls (§31 (the foreign-language seam: C holds no capabilities and makes no syscalls)).
`notes/c-seam.md` tier 3, "Full POSIX", is "later, if ever". `vttest` appears in the tree once,
as a reason in `notes/glyphs/libghostty-vt.md`. The closest measure is therefore the host
harness: `vttest` in a pty, its output into `Vt`, a screen dump at each "Push <RETURN>" pause, graded against goldens
recorded once from a reference terminal and reviewed by a person. `vttest` has no machine-readable
verdict, so the goldens are the verdict. Menu 6 has nothing to grade without the reply path in Part 3.
I did not run `vttest`.

## Part 2: What the three programs need that is missing

The census, taken on the host with vim 9.1 and the OS `less`, is in [the census appendix](0705-the-graphical-terminal-runs-full-screen-programs/census.md). In short, `Vt` does not act on these:

- DECSTBM scrolling regions (vim), reverse index `ESC M` (`less`), and the alternate screen (`?1049`, both).
- Cursor position and device attribute reports, which need the reply path in Part 3.
- Insert and delete, cursor addressing, scroll by n, save and restore cursor, and small mode switches such as DECTCEM, DECAWM and DECCKM.

`top` is the nearest to working; its gap is DECAWM. The replay found the `less` backward-scroll failure deterministic (22 of 24 rows differ), and one vim split-window case flaky.

## Part 3: Keeping the engine a value when it has to reply

Reports need the engine to produce bytes the program reads. Today it cannot: it "never writes to its
input", which is what sans-IO means here (`lib.rs:119-121`), and `csi_final` says so in a comment
(`lib.rs:1475-1478`). The property worth keeping is that the engine holds no endpoint and makes no
call, not that it never has output.

Proposal, the shape `take_damage` already has. `Vt::feed` keeps its signature. A new `Vt::take_reply`
hands back a bounded buffer of reply bytes and clears it, "taken, not read", exactly as damage is
(`lib.rs:1176` and the crate-level example). The component forwards those bytes where a keystroke
goes: into `display_terminal`'s `OPERATION_BYTES` path (`components/src/display_terminal.rs:513-523`), so the
line discipline in front of it and then `OPERATION_READRAW` deliver them to the program. The buffer has a
fixed bound; a flood of queries drops the newest reply and says so in `BUGS`, which fails towards a
program that times out rather than a terminal that blocks. The three-witness expected-picture check is
untouched, because replies do not change the grid. The wiring and the bound are a lane's to build and
an architect does not need to choose them.

Under option B the library's own shape is a callback (`GHOSTTY_TERMINAL_OPT_WRITE_PTY`, read in
`vt_terminal.h`); the shim would collect it into the same `take_reply` buffer.

## Part 4: The engine question, as options

Answers to questions 2 and 4 of the seven are the same for every option, so they are here once.

- *What does this tree do in the analogous case?* It built `line_editor` against the prior-art rule's
  default (`notes/line-discipline.md`, "The build-vs-reuse call"), built `video_terminal` before any
  foreign engine existed, and recorded the libghostty-vt cost in `notes/glyphs.md` ("What adopting
  libghostty-vt would cost now", since #1918 the appendix `notes/glyphs/libghostty-vt.md`) and §37
  (text as a value), which says "Architect's call, not taken here". Vendoring policy for a foreign
  library is §18 (the PCIe transport), the RedoxFS precedent.
- *Is the premise true?* Partly. The gap is real and measured above, but it needs a program to hit it,
  and no such program ships (Part 1). Separately, libghostty-vt is freestanding but not allocation-free
  (tier 1 of `notes/c-seam.md`'s "The libc question, answered by tier"). Upstream's
  `allocator.h` says it "does require memory allocation for various operations", that on a native
  freestanding target the default allocator always fails, and that the consumer must supply one. So
  tier 1 of the C seam holds, but "fixed buffers, no allocation" does not. `display_terminal` has no
  allocator today.

### A. Extend `video_terminal`

The list in Part 2. Estimate: 500 to 900 lines of engine code plus about as many of tests. This
is an estimate by analogy, not a measurement: the engine today is 1,711 lines before its tests, plus
`keymap.rs` (527) and `script.rs` (274), and `display_terminal.rs` is 536. A second grid and a
region-aware scroll touch `line_feed`, the damage record and the scrollback ring, which is where the
risk is, not the sequence count.

1. Considered instead: B and C, each under its own entry, and D for the ordering.
2. See the shared answer.
3. Prior art: xterm's control-sequence document and `vttest` define the behavior (not read here; named
   from memory). Every real engine in section C or B is an existence proof.
4. See the shared answer.
5. Cost: lines above, no new dependency, no new toolchain, host-testable in milliseconds. Build and
   binary size: not measured; the debug `display_terminal` ELF is 1.77 MB on aarch64, which says
   nothing about the engine's share.
6. Fully reversible. No wire format changes, and the engine is behind `display_terminal`.
7. Yes, at equal cost: it keeps the three-witness proof (`pixel(x, y)`) with no rebuild, and it is the
   only option with no dependency in the shipping graph.

### B. libghostty-vt as a second engine behind the C seam

Full entry in [the engine options appendix](0705-the-graphical-terminal-runs-full-screen-programs/engine-options.md). Summary, read from upstream `main` on 2026-10-02:

- The C API is declared incomplete and "definitely going to change"; the behavior is stable.
- It needs Zig 0.16.0 or later in the build, an allocator the component does not have, and a cells-based rebuild of the expected-picture check.
- It buys the alternate screen, reports, mouse and key encoders, reflow and scrollback. MIT license.
- A dependency in the shipping graph is the irreversible category in `CLAUDE.md`. Cost is unmeasured beyond that.

### C. `vte` (the Rust crate)

Full entry in the same appendix. `vte` 0.15.0 is a parser, not an engine: the grid, regions, second screen and replies stay ours, and it saves only the sequence recognition this engine already has and tests. At equal cost A wins over C. C is listed so that a refusal, if it comes, has its reason on record.

### D. A then B, or B then A

The two are not exclusive: B behind the seam does not remove A, and `notes/glyphs.md` already
recommends keeping `video_terminal` as the reference the foreign engine is checked against.

- **A then B.** The full-screen goal is met without a toolchain, and B arrives later for its own
  reason (milestone 23). The cost is that the expected-picture check is not rebuilt until B, and A's
  region and alternate-screen code becomes the reference B is graded against, which is a useful
  property and not an accident.
- **B then A.** Full-screen programs come from B's completeness first. The cost is that the goal waits
  on the Zig toolchain and the cells-based check, and A is then written afterwards as a second engine
  for no customer reason. I see no argument for this order other than the milestone 23 claim.

1. Considered instead: A alone, B alone. Already above.
2. to 4. See the shared answers.
5. Cost: the sum, spread over two lanes; the first delivers the goal.
6. As reversible as each half.
7. For A then B, yes at equal cost, because the first step delivers the measure. For B then A, the
   ordering is effort-neutral and I would not choose it.

### E. Replace `video_terminal` with libghostty-vt

B without keeping A's engine: libghostty-vt becomes the only VT engine behind `display_terminal`, and
`video_terminal` is retired (or kept only as a host-side test fixture). Facts about the library are
those in B; this entry is about what losing the second engine costs and buys.

- *Against.*
  - `video_terminal`'s `pixel(x, y)` carries the three-witness check: the component draws, the kernel
    test predicts, and xtask grades the scanout. libghostty-vt gives cells, so the check has to be
    rebuilt on cells with a small renderer we own.
  - Losing the second engine loses differential testing. §37 (text as a value) and `notes/glyphs.md`
    call that comparison the stronger milestone 23 (a capability-routed component OS with live
    replacement) demonstration.
  - As the only engine, every upstream API break ("definitely going to change") breaks the display
    path. That path then needs the Zig toolchain and a supplied allocator just to work.
- *For.*
  - No 500 to 900 lines of our own engine code to keep up.
  - No double maintenance, and no triage of disagreements between two engines.
  - Immediate completeness: DEC modes, resize, grapheme clustering, mouse, and a `vttest` history
    (claimed in `notes/glyphs.md`, not verified here).
- *What would make E more attractive.* An upstream API declared stable.

1. Considered instead: A (keeps the second engine, costs the lines), B and D (keep both engines).
2. to 4. See the shared answers.
5. Cost: B's toolchain, vendoring and allocator costs, plus the cell-based check, minus the engine
   lines of A. Not measured beyond what B records.
6. Harder to reverse than B: once `video_terminal` is gone, bringing it back is a rewrite, and the
   display path depends on a library whose API is declared unstable.
7. Not decided here. At equal cost I would still keep two engines, because the differential check is
   the claim; E wins only if that claim is not worth the second engine's upkeep.

## Ruling

calef, 2026-10-03 UTC: "not yet". The display paths sit largely unused (the 2026-09-30 ruling) and
nothing is blocked, so the engine choice waits. The proposal stays `PROPOSED` and nothing is built.

Revisit when either of these happens:

- someone needs a full-screen program at a display path, or
- libghostty-vt stabilizes its C API.

## Part 5: Priority, and whether the serial console has the same gap

calef ruled on 2026-09-30 that graphics is not at boot and "will sit there largely unused for some
time" (PR #1493, graphics on demand, number provisional). That makes the question of which terminal a customer meets first a real one.

**The serial path has no such gap, because nife does not interpret escapes there.**
`components/src/console.rs` copies the bytes of an `OPERATION_WRITE` to the UART verbatim, and the terminal
contract says "ANSI from the application reaches the screen intact"
(`notes/terminal-contract.md`, "What a terminal owes a program, and what it does not"). The user's host terminal (`xterm`, `Terminal.app`, `tmux`)
is the VT engine, so `vim`-class programs get a complete one for free. `rmle` already does this: it
emits `ESC[?25l`, `ESC[H`, `ESC[K`, `ESC[7m` and a cursor-position `CSI H` (`components/src/rmle.rs:526-560`) and
depends on nothing the display terminal lacks. Replies from the host terminal arrive on the UART
as input bytes. They reach the program through `OPERATION_READRAW` while raw mode is on
(`notes/terminal-contract.md`, "The IPC protocol"), so DSR and DA already work in principle on the serial path;
I did not test it.

The serial path has a different gap. The contract carries no window size, so `rmle` fixes 80x24
(`rmle.rs` "No terminal size negotiation") and a differently sized host window redraws wrongly
(`notes/terminal-contract.md`, its "Terminal size tracking" limit). That is a separate piece of work and is not this proposal.

**Where this proposal matters.** A screen the user is looking at, with no host terminal in the
loop: the `screen` session of PR #1493 on a machine with a display, and the firmware-screen console, where
`console.rs` hands "the same stream" to `display_terminal` beside the UART (`MODE_SCREEN`, `console.rs:19-30`).
A machine with a monitor and no serial cable meets this gap with the first
full-screen program it runs.

So the order of customer relevance is: the serial path works now and needs a size fix; the display
paths need this milestone, and there is no program to measure against until one is ported. I
could not verify that a program launched from `screen` reaches `display_terminal` through
`OPERATION_RAWMODE` end to end; #1493 is unmerged and I did not read its guest tests.

## Part 6: Dependencies, and what is blocked

- The graphics-on-demand work (#1493, provisional number 632, `screen` launched from the swish prompt) is unaffected by
  the engine choice: the engine sits behind `display_terminal`, and the launch builds the same
  endpoints whichever engine answers them. It is not listed in the frontmatter because it is not on
  `main` and its number is provisional.
- Milestone 29 (the display terminal) and milestone 142 (a text display good enough that people use
  it instead of a GUI) are the blocks this touches. 142's remaining increments are about glyph
  quality and do not collide with the engine's state work.
- Blocked until the choice is made again: options B, D and E cannot start. Option A needs no ruling and could start
  now, but A is also step one of D, so starting it quietly would pick D's first half.
- A program to measure against (a port of `less`, `vim` or procps `top`) is milestone 198's road and
  not this one's. The host-harness measure does not need one.

## What would be built, if the answer is A

1. A host harness in `xtask` (name provisional): a program in a pty, its output into `Vt`, a screen
   dump compared with a committed golden.
2. `Vt::take_reply`, DSR 6, DA, then `ESC M`, `ESC 7`/`8`, `CSI L M @ P X G d S T`, DECSTBM, DECAWM,
   and the alternate screen, each with a test that fails first.
3. `display_terminal` forwarding the reply bytes into its keystroke path.
4. `notes/glyphs.md` and `notes/terminal-contract.md` updated; the stale "no allocation" claim
   corrected wherever it is repeated.

## Index row

`crates/video_terminal` implements printable text and a handful of CSI sequences, which is not enough for `vim`, `less` or `top`. Proposed: close the gap; the engine choice is a dependency decision, so options A to E are given with no recommendation.
