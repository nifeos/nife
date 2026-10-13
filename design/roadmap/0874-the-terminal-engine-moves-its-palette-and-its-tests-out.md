---
status: BUILT
raised: 2026-10-11
built: 2026-10-12
milestone_dependencies: none
decision_dependencies: 266, 292
machine_requirements: none
specific_machine: none
needs_person: no
---
# 874. The terminal engine moves its palette and its tests out

*(The title and slug are provisional.)*

`crates/video_terminal/src/lib.rs` is 3,242 lines at `396187b0b`. §266 (a Rust source file stays under 2,000 lines) sets the ceiling.
Milestone 840 (the scheduler file is split along its seams) is the model for the form.
Milestone 846 (the file-server core moves its tests out) is the model for most of the answer.

The cut and every module name below are calef's call. A lane writes them up as a
`status: PROPOSED` file in `design/decisions/` before it moves a line, and its pull request
carries `needs-architect`.

The lane wrote it up on 2026-10-12 (UTC) as §292 (the terminal engine's crate root becomes an
index), `status: PROPOSED`, number provisional. This block was minted before §282 (when a file is
split, the parent becomes an index) was decided the same day. §282 answers its ruling 1
differently, since the reason given below for keeping `Attr` and `Vt` in `lib.rs` is the one
§282's third reason refused. §267's amendment of 2026-10-11 (UTC) answers ruling 2, since a
listed comment block now keeps its row when a split moves it unchanged. calef's ruling, recorded on
#1968 on 2026-10-12 (UTC), settled the one open question by precedent: the crate root becomes an
index. §292 records it.

Reuse: not applicable; this moves code and adds none. The crate already has two child modules in
their own files, `keymap` and `script`, which is the precedent.

## What is in the file

Measured on `396187b0b` from the file's own top-level items and its own banners. Ranges are
approximate at the edges, where a doc comment belongs to the item below it.

| lines | count | what |
|---|---|---|
| 1 to 136 | 136 | crate docs, `pub mod keymap`, `pub mod script` |
| 137 to 204 | 68 | geometry: `MAX_COLS`, `MAX_ROWS`, `MAX_CELLS`, the scrollback budget |
| 205 to 392 | 188 | `PALETTE`, its compile-time gate (`PaletteFaults`, `palette_faults`), `DEFAULT_FG`, `DEFAULT_BG` |
| 393 to 642 | 250 | the color enum, `Attr` and its flags, the dim midpoint, `Cell` |
| 643 to 767 | 125 | `CellRect` and `Damage` |
| 768 to 1720 | 953 | the parser's `State`, `Vt` and its one `impl` block (851 lines) |
| 1722 to 1752 | 31 | `pub mod status`, three constants |
| 1754 to 3242 | 1,489 | `mod tests`, 47 `#[test]` functions |

The code is 1,753 lines and already under the ceiling. The tests are 46% of the file. Of them, 145
lines (1891 to 2035) test the palette and nothing else: two reference tables and three tests.

## The verdict: move the palette with its tests, and move the rest of the tests

Three cuts of the code were weighed.

- The palette is self-contained. Nothing outside lines 205 to 392 reads `PaletteFaults`,
  `palette_faults` or `sorted_channels`, and its three tests use only those. Everything else reads
  `PALETTE`, `DEFAULT_FG` and `DEFAULT_BG`, which are `pub` already. It moves with its tests and
  nothing widens.
- `Attr` looks like a seam and is not one. `Vt::sgr` calls the private `Attr::with` and the six
  private flag constants (`BOLD` through `STRIKETHROUGH`). A parent cannot see a child's private
  items, so a `rendition` module would make seven items `pub(super)`.
- The parser half of `impl Vt` (`feed` through `scroll_damage`, about 500 lines) could be a child
  module with its own `impl Vt`. But the grid half calls `damage_all` and `damage_cell` too, so
  those widen. And the tests drive bytes through `feed` and assert on cells and pixels, so they
  do not divide between the halves. `Vt` is one type, and milestone 846's reasoning applies.

So the code splits only where it has a real seam, and the rest of the tests move to a child file.
They stay a child module, not integration tests: they reach private items such as `Attr::with`
and the flag constants.

## A proposed layout

Every name here is provisional. `palette` is private, and `lib.rs` re-exports `PALETTE`,
`DEFAULT_FG` and `DEFAULT_BG`, so every public path still resolves.

| file, provisional | from the table | about |
|---|---|---|
| `lib.rs` | everything but 205 to 392, and `#[cfg(test)] mod tests;` | 1,570 |
| `palette.rs` | 205 to 392, with its tests (1891 to 2035) inline | 340 |
| `tests.rs` | the other 44 tests and their helpers | 1,345 |

`tests.rs` meets milestone 840's bar of no new file over 1,500, with 155 lines to spare. The file
grew 1,206 lines in the 17 days to 2026-10-11. If the tests keep pace, a lane splits `tests.rs` by
topic. The scroll and scrollback tests (about 385 lines) are the largest group and the natural
first cut.

## What an architect has to rule

1. Whether the code splits at the palette only, rather than also at `Attr` or the parser. The
   recommendation is the palette only, because the other two widen private items. That reason
   holds at equal cost, so it is not about effort.
2. The palette's doc comment. It is one 54-line block, on the list in
   `design/comment-block-baseline.tsv` at `lib.rs:209`. That ratchet admits no new row, and a new
   file's blocks meet the 40-line cap outright. So moving the block to `palette.rs` fails `script/lint`
   unless the block comes down to 40 lines in the same change. The recommendation is to shorten
   it, under §267 (a comment states the constraint as it is now). Its "The gate" paragraph narrates
   the xterm palette this one replaced, which is history. If calef prefers the comment untouched,
   the palette stays in `lib.rs` and only the tests move: `lib.rs` ends near 1,755 and `tests.rs`
   near 1,490.
3. Every file name in the table.

No public crate API, wire format or syscall changes under any answer here.

## What moving the code breaks

Found with `grep` at `396187b0b`. A lane has to carry each one.

- No falsification patch names the crate, and the crate has no `falsifications` directory. No
  Kani harness is in it, and `kani-reach.yml` does not name it. No `#[path]`, `include!` or
  `include_str!` reaches into it. `xtask` depends on it as a crate (`scanout.rs`,
  `swish_check.rs`) and copies no file from it.
- 14 Rust files outside the crate name `video_terminal::`. With re-exports none changes, and none
  names `PALETTE`.
- `helpers/cap_abbreviation.py` keys an allowance on this path for the test
  `scrolls_report_movement_and_cap_at_the_screen`. The key moves to `tests.rs` with the test.
- `design/file-length-baseline.tsv` lists the file at 3,242. The row goes in the same change.
- `design/comment-block-baseline.tsv` has two rows. `lib.rs:1` (130 lines, the crate docs) stays
  and is unaffected. `lib.rs:209` (54 lines, the doc of `PALETTE`) lands in `palette.rs`, and the
  ratchet fails it there, as ruling 2 says. Short of the cut ruling 2 recommends, the other way
  through is a fix to the ratchet, letting a row move with its block, ruled separately.
- Milestone 705 (the graphical terminal runs full-screen programs), not started, cites
  `lib.rs:1373-1478` and three other ranges for the parser. They had drifted already. Replacing
  each with a function name (`ground`, `csi_final`) is cheaper than renumbering it.
- Four places say the PALETTE BUGS are in `lib.rs`: milestone 141 (a palette worth looking at),
  milestone 142 (a text display good enough that people use it instead of a GUI), twice, and
  `notes/solarized-and-bold-is-bright.md`. They need the new path. `notes/glyphs.md` maps "the VT
  engine" to `lib.rs`, which stays true.
- The palette's doc links `Attr` by bare name, which needs a `crate::` path from `palette.rs`.
  Its relative link to §104 (the rich-text font is DejaVu Sans Mono, and the palette is
  Solarized) still resolves, since `palette.rs` sits in the same directory.
- `notes/project-metrics/mutation-triage.csv` holds 37 keyed rows for the crate. The rows for
  `script.rs` carry no `script::` prefix, so a file module seems not to enter the key, and the
  moved free functions (`sorted_channels`) keep theirs. That is inferred from the ledger, not
  read in cargo-mutants. A lane confirms it with `cargo mutants --list`.
- 6 first-parent merges in the 14 days to 2026-10-11 touched it.

## Done when

1. `crates/video_terminal/src/lib.rs` is under 2,000 lines, and no new file is over 1,500.
2. The crate's 47 host tests pass with no test body changed.
3. No Rust file outside `crates/video_terminal/src/` changes, and no item's visibility widens.
4. `script/lint` passes, with the file-length row gone and the comment-block row resolved as ruled.
5. `cargo doc` for the crate reports no broken intra-doc link.
6. `script/test` passes on aarch64, riscv64 and x86_64.

## What was built

Built 2026-10-12 (UTC). `crates/video_terminal/src/lib.rs` went from 3,242 lines to 158, an index of
crate docs, `mod` lines and named re-exports, so its file-length and parent-index rows are gone.
Every public path still resolves, and no Rust file outside the crate changed. Every name below is
provisional.

| file | lines | holds |
|---|---|---|
| `lib.rs` | 158 | crate docs, `mod` lines, re-exports, `status`'s doc |
| `geometry.rs` | 72 | `MAX_COLS`, `MAX_ROWS`, `MAX_CELLS`, `SCROLLBACK_ROWS`, `SCROLLBACK_CELLS` |
| `palette.rs`, `palette/tests.rs` | 196, 146 | `PALETTE` and its gate, `DEFAULT_FG`, `DEFAULT_BG`; 3 tests |
| `cell.rs`, `cell/tests.rs` | 255, 86 | the color enum, `Attr` and its flags, `Cell`; 5 tests |
| `damage.rs`, `damage/tests.rs` | 132, 42 | `CellRect`, `Damage`; 2 tests |
| `vt.rs`, `vt/tests.rs` | 962, 1,217 | `State`, `Vt`; 37 tests |
| `status.rs` | 23 | the three report constants |
| `test_support.rs` | 11 | `attr`, shared by two test files |

No function body changed, and the 47 tests pass under their old names with a module added to each
path. Thirteen items became `pub(super)`, which names the same modules they reached as private items
of the crate root; done-when 3's "no item's visibility widens" holds in that sense and not in
keywords. The palette's 54-line comment moved unchanged and took its comment-block row with it.
`palette.rs`, `damage.rs` and `geometry.rs` import `super::*` under `#[cfg(doc)]`, since only their
doc links use it. The allowance in `helpers/cap_abbreviation.py` follows its test to `vt/tests.rs`,
and `notes/glyphs.md`, `notes/solarized-and-bold-is-bright.md` and a comment in `.cargo/config.toml`
name the new paths. `cargo mutants --list` keys mutants without a module, so the mutation ledger
needs no change.

## Follow-on

- **Decision.** Every module name is provisional and is calef's to rule in his naming pass, per
  `design/decisions/0292-the-terminal-engines-crate-root-becomes-an-index.md`.
- **Refused.** Repointing the citations of `video_terminal/src/lib.rs` under `design/` here (milestones
  141, 142 and 705). A developer lane edits no record under `design/` but its own block. Pull request
  #1968 lists each with the symbol it should name, for the maintainer's sweep.
- **Refused.** Splitting `vt/tests.rs` (1,217 lines) by topic now. It is under §266's ceiling and the
  1,500 aim; the scroll and scrollback tests are the natural first cut if it grows.

## Index row

The terminal engine's file is 3,242 lines, but its code is 1,753 and the rest is tests. `Vt` is
one type whose halves share private helpers, so it stays whole. The palette and its compile-time
gate move out with their own tests, and the other tests move to a child file. The layout, the
palette comment's fate and every name are an architect's call.
