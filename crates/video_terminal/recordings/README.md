# The terminal replay recordings

What the replay harness (`cargo xtask vt-replay`, xtask/src/vt_replay.rs) grades, for
milestone 705 (the graphical terminal runs full-screen programs)'s step 0a. One session is
three files, and they move together:

- `<name>.rec`, the recording: the bytes the program wrote, with the offsets of its keystrokes.
- `<name>.expected.txt`, the oracle's screen at each graded checkpoint.
- the session's row in `manifest.toml`: what ran, when, under which TERM, the checkpoint count,
  and the match count the gate holds.

## The recording file

```text
bytes 0..4   the magic "NVTR"
bytes 4..6   version, u16 little-endian, currently 1
then events to the end of the file, each:
    0x00  u32 little-endian length, that many bytes   output the program wrote
    0x01  u32 little-endian length, that many bytes   one keystroke the user sent
```

A keystroke event carries the input bytes so a session can be re-recorded; the harness never
feeds them to the engine. Each keystroke's checkpoint is the grid after every output byte
written before that keystroke, which is the screen the user was looking at when they pressed
it. The screen after the program exits carries no keystroke, so it is never graded. A row of
an expected file never begins with `@ checkpoint `, which is what marks the next block.

## The expected screens, and where they come from

The oracle is `alacritty_terminal` 0.26.0, calef's ruling of 2026-10-21 03:37 UTC. His
2026-10-19 hold on its §46 (thin primitives or whole subsystems; we write everything in
between) take stands, so the oracle is not a dependency of this tree: its screens were
generated once, committed, and are graded as committed artifacts.

The committed screens were generated on 2026-10-28 (UTC), on patagonia, by a throwaway
generator that replayed each recording through the oracle and dumped the 24 text rows of the
visible screen at every graded checkpoint, with trailing blanks trimmed, control characters as
blanks, and the same text-only rule the census states. The generator lands in the tree as a
host-only dev-dependency of `xtask` when the hold lifts; until then, regenerating a session's
screens means repeating that recipe against this file's format.

## Recording a session again

`cargo xtask vt-replay --record <name>` runs the session's keystroke schedule in an 80x24 pty
through macOS's `script(1)` (no dependency), with an empty home, a fixed locale, and TERM from
the manifest. The ten census programs are the versions the manifest rows name, and the
ncurses test compiles from `ncurses_screen_test.c` against Homebrew's ncursesw 6.6
(`NIFE_NCURSES_HOME` overrides the prefix). Then generate the expected screens by the recipe
above and set the row's `checkpoints` and `matches`; a recording, its expected screens and its
row move together, and a session that is re-recorded produces a different byte stream.

## BUGS

- The comparison is text only. Attributes, colors and the cursor are not compared, so the
  counts flatter every engine, ours included, exactly as the census's counts do.
- A wide character loses the column match by construction: `Vt` holds one `char` per cell
  until the plan's step 7, the oracle two columns, so any checkpoint whose screen carries one
  fails.
- The committed screens carry no review yet beyond this lane's own reads; the step 0a pull
  request is where a reviewer compares them against the recordings.
- The ten programs' sessions are this lane's own schedules, written to the census's shape; the
  census's exact schedules were throwaway and are not kept. The counts are not the census's
  counts, and the plan's predictions are not against these sessions.
