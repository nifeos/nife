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
with it. Parts 1 to 3 were checked in the tree or upstream on 2026-10-02 (UTC). Options F and G were
measured on 2026-10-18 (UTC). A claim from memory is marked as such.

The engine choice is a dependency decision (§46 (thin primitives or whole subsystems; we write
everything in between)). Part 4 gives options A to G, and calef chose A ([Ruling](#ruling)).

## Part 1: The gap, and the goal

`crates/video_terminal` (milestone 29, the display terminal) implements printable text with UTF-8, CR,
LF, BS, TAB, CSI `A B C D H f J K m` (SGR with 256-color and 24-bit) and RIS (`csi_final` and `esc`,
`crates/video_terminal/src/vt.rs:725-831`). Its own header lists what it leaves out: no scrolling
regions, no alternate screen, no origin mode, fixed tab stops, no mouse, no reporting sequences, no
reflow (`lib.rs:26-28`, `notes/glyphs.md` "Honest limits"). Anything else is swallowed whole
(`vt.rs:741`, `827-830`).

Milestone 142 (a text display good enough that people use it instead of a GUI) is PARTIAL and is about
glyph quality, scrollback, UTF-8 and grid size. None of its increments is about DEC-mode
compatibility, so this is not already owned.

Goal. `vim`-class editors, `less` and a repainting `top` draw correctly on the graphical terminal.
Today none of the three exists on nife.

- The only full-screen program in the tree is `rmle` (milestone 169 (the smallest real text editor, as the forcing function for raw terminal input)).
- There is no pager (milestone 334 (color and the pager: the spawn protocol's other two thirds) is NOT-STARTED).
- `watch` was cut (milestone 281 (`watch` holds exactly what `ps` holds, so it is nothing)), and nife's own `top` prints one table and never repaints.

So the terminal half and the program half are
separate work. This proposal is the terminal half; the programs arrive through whatever ports them
(milestone 198 (a package manager, and the trivial install that makes a second customer possible) is the road).

**Measure.** Two numbers, both taken before and after.

1. *Screens matching a reference* on the host, driving the real programs. A host harness runs the
   program in a pty, feeds its output to the engine, and compares the 80x24 grid with the same
   session captured from a reference terminal. A throwaway version, using `tmux capture-pane` as the
   oracle, produced the census.
2. *`vttest` menus passed*, by menu. Menus 1 (cursor movement), 2 (screen features) and 6 (terminal reports) are the ones full-screen programs need, with 8 (VT102 insert and delete).
   Menu 2 holds scrolling regions, origin mode and tab stops; 3, 4, 5 and 7 (character sets, double-size, keyboard,
   VT52) are out of scope. The menu names are from memory.

`vttest` cannot run on nife today. It is a C program over `termios`, `ioctl(TIOCGWINSZ)`, `select`
and signals, and the tree has no libc and no `termios`. The C seam is freestanding and makes no
syscalls (§31 (the foreign-language seam: C holds no capabilities and makes no syscalls)). So
`vttest` runs in the host harness: its output into the engine, a screen dump at each "Push
<RETURN>" pause, graded against goldens recorded once from a reference terminal and reviewed by a
person. Menu 6 has nothing to grade without the reply path in Part 3.

## Part 2: What the three programs need that is missing

The census, taken on the host with vim 9.1 and the OS `less`, is in [the census appendix](0705-the-graphical-terminal-runs-full-screen-programs/census.md). In short, `Vt` does not act on these:

- DECSTBM scrolling regions (vim), reverse index `ESC M` (`less`), and the alternate screen (`?1049`, both).
- Cursor position and device attribute reports, which need the reply path in Part 3.
- Insert and delete, cursor addressing, scroll by n, save and restore cursor, and small mode switches such as DECTCEM, DECAWM and DECCKM.

`top` is the nearest to working; its gap is DECAWM. The replay found the `less` backward-scroll failure deterministic (22 of 24 rows differ), and one vim split-window case flaky.

## Part 3: Keeping the engine a value when it has to reply

Reports need the engine to produce bytes the program reads. `Vt` "never writes to its input"
(`lib.rs:27`), and `csi_final` says so in a comment (`vt.rs:827-829`). The property worth keeping is
that the engine holds no endpoint and makes no call, not that it never has output.

Proposal, the shape `take_damage` already has (`vt.rs:398`). `Vt::feed` keeps its signature. A new
`take_reply` hands back a bounded buffer of reply bytes and clears it, "taken, not read", as damage
is. The component forwards those bytes where a keystroke goes, into `display_terminal`'s
`OPERATION_BYTES` path (`components/src/display_terminal.rs:522`). The line discipline in front of
it, then `OPERATION_READRAW`, deliver them to the program. A flood of queries drops the newest reply
and says so in `BUGS`, which fails towards a program that times out rather than a terminal that
blocks. Replies do not change the grid, so the expected-picture check is untouched.

Every engine here fits this shape. libghostty-vt (option B) has a write callback
(`GHOSTTY_TERMINAL_OPT_WRITE_PTY`), `alacritty_terminal` (option F) sends `Event::PtyWrite`, and
libvterm (option G) keeps an output buffer that `vterm_output_read` drains; each is collected into
the same buffer.

## Part 4: The engine question, as options

Answers to questions 2 and 4 of the seven are the same for A to E, so they are here once; F and G answer their own.

- *What does this tree do in the analogous case?* It built `line_editor` against the prior-art rule's
  default (`notes/line-discipline.md`, "The build-vs-reuse call"), built `video_terminal` before any
  foreign engine existed, and recorded the libghostty-vt cost in `notes/glyphs/libghostty-vt.md` and
  §37 (text as a value). Vendoring policy for a foreign library is §18 (the PCIe transport), the
  RedoxFS precedent.
- *Is the premise true?* Partly. The gap is real and measured, but it needs a program to hit it, and
  none ships (Part 1). The framing "no `std`, no allocator" is this program's inherited choice, not
  a constraint. `display_terminal` is `no_std` and keeps its grid in `.data`. A `std` process on nife
  has a heap and threads (`notes/std.md`), and `jig` is a `std` program that makes raw syscalls. What
  is still `no_std` is the kernel's witness, which predicts the scanout with `video_terminal`.
  Separately, libghostty-vt is freestanding but not allocation-free (its `allocator.h`).

The full entries for A to E are in [the engine options appendix](0705-the-graphical-terminal-runs-full-screen-programs/engine-options.md).
F and G have their own ([F](0705-the-graphical-terminal-runs-full-screen-programs/option-f.md),
[G](0705-the-graphical-terminal-runs-full-screen-programs/option-g.md), with
[G's upkeep](0705-the-graphical-terminal-runs-full-screen-programs/option-g-upkeep.md)), and vttest's results are in
[the conformance appendix](0705-the-graphical-terminal-runs-full-screen-programs/conformance.md).

- A. Extend `video_terminal`. The Part 2 list, an estimated 500 to 900 lines of engine code and as
  many of tests. No dependency, and the kernel's witness keeps predicting every feature.
- B. libghostty-vt as a second engine behind the C seam. Complete, MIT, but it needs Zig 0.16, an
  allocator and a cells-based check, and its C API is declared unstable.
- C. `vte` alone. A parser, not an engine: the grid, regions, second screen and replies stay ours. At
  equal cost A wins over C.
- D. A then B. A delivers the goal; B arrives later for milestone 23 (a capability-routed component OS with live replacement).
- E. libghostty-vt replaces `video_terminal`. Loses the second engine and puts an unstable API on the
  only display path.
- F. `display_terminal` becomes a `std` program on a full Rust engine.
- G. The same `std` program on libvterm, a C engine linked through nife's C library. B's shape
  without Zig.

### F and G, measured

| | A: ours | F: `alacritty_terminal` 0.26.0 | F: `vt100` 0.17.0 | G: libvterm 0.3.3 |
|---|---|---|---|---|
| Builds for nife's three targets | yes | with its pty and event loop cut out | unmodified | compiles and links through the C library |
| Part 2 checks (15) | 0 | 15 | 13 | 15 |
| vttest screens (24) | 7 | 22 | 17 | 23 |
| Engine code | in 18 KB | about 180 KB | about 55 KB | 37 to 44 KB |
| Heap per 80x24 terminal | 717 KB static | 2.2 to 4.1 MB | 0.26 MB probe peak | 0.15 MB |
| Replies, damage | neither | both | neither | both |
| New to the base graph | nothing | 10 crates | 5 crates | one C library |
| Ran under QEMU on nife | in the suite | yes, 15 of 15 | yes, 13 of 15 | yes, 15 of 15 |

`termwiz` 0.23.3 does not build for nife and is not an engine. `alacritty_terminal`'s heap floor is
2.2 MB because `vte` reserves 2 MiB for synchronized updates; lowering its own storage step only
removes the jump to 4.1 MB.

The kernel cannot link a `std` engine, so under F or G its witness keeps `video_terminal`. The
three-witness check becomes differential: two engines drawing through one renderer must agree pixel
for pixel on the scripted screen. `video_terminal` survives as the oracle §37 values. The limit is
that the kernel predicts only what `video_terminal` implements, so the full-screen features are
graded by Part 1's host harness instead.

## Ruling

calef, 2026-10-21 UTC: the engine is A, extending `video_terminal`. "A sounds good. However we
should flush it out more fully." G is refused. On the §46 take of libvterm 0.3.3: "It seems like a
poor dependency to build on." The §46 take of `alacritty_terminal` 0.26.0 stays held for the
dependency census (calef, 2026-10-19 UTC: "Hold it"), with `r-efi`, `plist` and `pulldown-cmark`.
705 is a priority (calef, 2026-10-18 UTC: "Full screen program support seems like a priority.").

## Why A

A takes no dependency, and the kernel's witness keeps predicting every feature. G's upstream has one
committer and six commits in the 39 months after 0.3.3. Neovim, its heaviest embedder, carries ten
fixes upstream does not have ([G's upkeep](0705-the-graphical-terminal-runs-full-screen-programs/option-g-upkeep.md)).
F waits on the held `alacritty_terminal` take, and its pty cut would be carried for good.

## What is needed from calef

Nothing. A's gap plan is lane work.

## Part 5: Priority, and whether the serial console has the same gap

calef ruled on 2026-09-30 that graphics is not at boot and "will sit there largely unused for some
time" (milestone 632 (graphics on demand)). That makes the question of which terminal a customer meets first a real one.

The serial path has no such gap, because nife does not interpret escapes there.
`components/src/console.rs` copies the bytes of an `OPERATION_WRITE` to the UART verbatim, and the terminal
contract says "ANSI from the application reaches the screen intact"
(`notes/terminal-contract.md`, "What a terminal owes a program, and what it does not"). The user's host terminal (`xterm`, `Terminal.app`, `tmux`)
is the VT engine, so `vim`-class programs get a complete one for free. `rmle` already does this: it
emits `ESC[?25l`, `ESC[H`, `ESC[K`, `ESC[7m` and a cursor-position `CSI H` (`components/src/rmle.rs:497-531`) and
depends on nothing the display terminal lacks. Replies from the host terminal arrive on the UART
as input bytes. They reach the program through `OPERATION_READRAW` while raw mode is on
(`notes/terminal-contract.md`, "The IPC protocol"), so DSR and DA already work in principle on the serial path;
I did not test it.

The serial path has a different gap. The contract carries no window size, so `rmle` fixes 80x24
(`rmle.rs` "No terminal size negotiation") and a differently sized host window redraws wrongly
(`notes/terminal-contract.md`, its "Terminal size tracking" limit). That is a separate piece of work and is not this proposal.

Where this proposal matters: a screen the user is looking at, with no host terminal in the loop.
That is the `graphical_terminal` session of milestone 632, and the firmware-screen console, where
`console.rs` hands "the same stream" to `display_terminal` beside the UART (`MODE_SCREEN`).
A machine with a monitor and no serial cable meets this gap with the first
full-screen program it runs.

A second gap sits beside it. A `graphical_terminal` session is a prompt that echoes its lines and
runs no program (its module note and `BUGS`). So a full-screen program reaches the display path only
through the firmware-screen console, until a session can run one.

## Part 6: Dependencies, and what is blocked

- Milestone 632 (graphics on demand) is unaffected by the engine choice: the engine sits behind
  `display_terminal`, and the launch builds the same endpoints whichever engine answers them.
- Milestone 29 (the display terminal) and milestone 142 (a text display good enough that people use
  it instead of a GUI) are the blocks this touches. 142's remaining increments are about glyph
  quality and do not collide with the engine's state work.
- Milestone 784 (reuse what the tree wrote)'s item (b) is `vte` alone. Under A, `video_terminal` is
  the engine, so whether it parses with `vte` is 784's to settle.
- A program to measure against (a port of `less`, `vim` or procps `top`) is milestone 198's road and
  not this one's. The host-harness measure does not need one.

## What would be built

What A builds is in [the engine options appendix](0705-the-graphical-terminal-runs-full-screen-programs/engine-options.md).

## Index row

`crates/video_terminal` implements printable text and a handful of CSI sequences, which is not enough for `vim`, `less` or `top`. Options A to G are measured, vttest included. calef chose A, extending `video_terminal`, and refused G (libvterm).
