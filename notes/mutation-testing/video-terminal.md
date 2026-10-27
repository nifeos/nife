# `video_terminal`

How `video_terminal`'s 79 survivors of the 2026-09-19 census were closed, under milestone 326 (nobody
has been assigned to turn a mutation score upward) part 3. It verifies the crate's row in
[notes/mutation-testing.md](../mutation-testing.md). The baseline's own `video_terminal` triage is
in [baseline-survivors-grant-plan-to-swish](baseline-survivors-grant-plan-to-swish.md).

## 2026-09-20: `video_terminal`, milestone 326 (turn a mutation score upward) part 3

`video_terminal` was the largest untriaged survivor set in the tree: 377 viable mutants, 79
survivors, 79.0% caught in the 2026-09-19 census. The discipline is the one
[new-crate-backlog](new-crate-backlog.md) states. Every number below was re-derived with
`script/mutation -p video_terminal` on this lane's own worktree, not trusted from the census. Every
kill was verified by re-running the sweep and watching the mutant die. Every equivalence claim is a
mutant the second run still reports.

Before: 298 caught, 79 missed, 0 timeouts, 15 unviable (79.0% of viable). After: 361 caught, 16
missed, 0 timeouts, 15 unviable (95.8%). This run's `before` column matches the census row for
row. The crate's scope explains why: it has no Kani harnesses and no loom model (no `proofs::`,
`verification::` or `interleavings::` module anywhere). So it carries none of the artifacts the
earlier sections found, where a score counted code that was never the crate's.
`cargo mutants -p video_terminal --list` confirms the mutant set is `lib.rs`, `keymap.rs` and
`script.rs`.

| file | survivors | killed by a test | equivalent | recorded gap |
|---|---|---|---|---|
| `lib.rs` | 79 | 63 | 16 | 0 |
| `keymap.rs` | 0 | 0 | 0 | 0 |
| `script.rs` | 0 | 0 | 0 | 0 |
| **total** | **79** | **63** | **16** | **0** |

`keymap.rs` and `script.rs` carried no survivors; every mutant in either was caught before this
lane touched anything. All 79 survivors are in `lib.rs`. No exclusion and no recorded gap was
needed: each closed as a killed test or a demonstrated equivalence.

Five tests were added, each closing coverage the crate never had, for 63 survivors.

### Multi-byte UTF-8 decoding had never been tested (18 survivors, 17 closed)

The eighteenth is equivalent, below. Milestone 142 (a text display good enough that people use it
instead of a GUI) added UTF-8 decoding in increment 2. Every mutant survived in `Vt::ground`'s
continuation-byte branch, in its shift-and-accumulate arithmetic, and in all three lead-byte masks
(two-, three- and four-byte sequences). Nothing in the suite had ever fed a non-ASCII byte to
`feed`.

`utf8_decodes_every_sequence_length_and_recovers_from_a_bad_one` feeds one character of each length
(`é`, `日`, `🎉`). It also feeds a truncated sequence (a lead byte followed by a non-continuation
byte) and a lone continuation byte that can never start one. It checks the exact `char` landed, not
just that something non-blank did. On inspection the decoder itself is correct RFC 3629: the
lead-byte ranges, the overlong-form exclusion and the replacement-character fallback all match.
This was a coverage gap, not a defect.

### Scrollback had no test at all (42 survivors, 41 closed)

Milestone 142's increment 2 also added scrollback. Nothing tested `scroll_up`, `scroll_down`,
`view_offset` or `scrollback_len`, and nothing scrolled far enough to read history back. The 42
survivors sat in `SCROLLBACK_CELLS` itself, `Vt::cell`'s history branch, `scrollback_cell`,
`push_scrollback_row`, the two getters, and both scroll methods. The forty-second,
`push_scrollback_row`'s source index, is equivalent, below.

`scrollback_survives_the_rings_wrap_and_reads_back_in_order` feeds 350 lines through a two-row
grid. That is 349 scrolls, past `SCROLLBACK_ROWS`'s 300-row cap. The ring wraps and its oldest 49
pushes are overwritten, which is the shape needed to prove the ring's modular arithmetic beyond its
first lap. Six checkpoints (`view_offset` 1, 2, 50, 150, 299, 300) each check both display rows.

Row 0 alone cannot distinguish `Vt::cell`'s `age = view_offset - 1 - row` from a mutant's `+ row`,
because at `row == 0` the two terms are equal. Row 1 is what makes the sign matter. At
`view_offset == 1`, row 1 comes from the live half of `Vt::cell` instead
(`live_row = row - view_offset`), which closes that arithmetic's own mutant.

`SCROLLBACK_CELLS` closes as a side effect of the same test. That constant sizes the `scrollback`
array. A mutant computing it as `MAX_COLS + SCROLLBACK_ROWS` (432 cells) rather than
`MAX_COLS * SCROLLBACK_ROWS` (39,600) panics on an out-of-bounds index early in the first feed
loop, before any assertion runs. A panicking test is a caught mutant, not a special case.

### `reset_to` had never been called (1 survivor, the whole function body)

Every other test builds a `Vt` at its final size with `Vt::new`, or never resizes. So the function
that exists for a runtime geometry was never exercised. Its caller is
`components/src/display_terminal.rs`'s own bring-up, per its doc comment.
`reset_to_clears_everything_and_retargets_the_geometry` feeds content that scrolls something into
history, then retargets to a different size. It checks the geometry, the cursor, every cell and the
damage rectangle. It also checks that the old grid's scrollback did not survive the retarget:
clearing `sb_len` is `reset_to`'s own job, separate from `scroll_down`'s.

### The size clamp had never been tested at its boundary (2 of 4 survivors)

The other 2 are equivalent, below. `clamp_cols` and `clamp_rows` had no test at
`cols == MAX_COLS` or `cols == MAX_COLS + 1`. So a mutant relaxing `cols > MAX_COLS` to
`cols == MAX_COLS` survived. It clamps only the exact boundary value and lets anything past it
through unclamped. `geometry_clamps_exactly_at_the_boundary_not_one_off_it` checks the boundary,
one past it, and zero, in both dimensions.

### `put` and `damage_cell`'s guard had never been exercised (2 survivors)

`if col >= self.cols || row >= self.rows`, relaxed to `&&`, refuses a coordinate only when both
axes are out of range. No caller in this crate ever passes a partly-out-of-range coordinate.
`print` always writes at the cursor, which is always in bounds, and `erase_line` bounds its column
with `.min(self.cols)` before calling `put`. So the guard was dead as far as any public-API test
could tell. `put_and_damage_cell_reject_a_partly_out_of_range_coordinate` calls both private
methods directly, through the `use super::*` access every test in this module has. It passes one
axis in range and the other not, on both axes, and checks that the private `cells` field and
`damage()` are untouched.

### Sixteen are equivalent, each argued from the code

- `Attr::DEFAULT`, 2 mutants (`DEFAULT_FG | (DEFAULT_BG << 4)`). `DEFAULT_BG` is `0`
  (`pub const DEFAULT_BG: u8 = 0;`). Shifting `0` either way by any amount is `0`, and OR or XOR of
  `7` with `0` is `7`. So the `|`-to-`^` and `<<`-to-`>>` mutants compute the same constant, and no
  test could separate them.
- `CellRect::union`'s four min/max selections, 4 mutants
  (`if self.col < o.col { self.col } else { o.col }` and its siblings for `row`, `right`, `bottom`).
  A selector of this shape returns the same value on both branches when `a == b`, because then they
  are the same number. `<` and `<=` (or `>` and `>=`) disagree only about which branch fires at that
  one point, and there it no longer matters. So no input at all can tell the strict comparison from
  the non-strict one.
- The `>=` half of the size clamp, 2 mutants (`cols > MAX_COLS as u32` in `clamp_cols`, and the
  same in `clamp_rows`). It is the `union` argument applied to
  `if cols > MAX_COLS { MAX_COLS } else { cols }`. At `cols == MAX_COLS` the else branch returns
  `cols`, which is `MAX_COLS`, so both branches agree at the one point `>` and `>=` differ. The `==`
  half above is a real gap: it clamps only the boundary value and lets everything past it through.
- `Vt::pixel`'s `col < self.cols` in the cursor-overlay check, 1 mutant. The term is reached only
  after `col == self.col` matched earlier in the same `&&` chain. `self.col` is never
  `>= self.cols`: every cursor-moving path clamps with `.min(self.cols - 1)` or
  `.min(self.rows - 1)`, and `print`'s deferred wrap never advances `col` past `cols - 1`.
  `Vt::erase_display`'s doc comment states this invariant in full. So whenever `col == self.col`
  holds, `col < self.cols` holds too, and `<=` cannot change the branch.
- `Vt::ground`'s continuation-byte accumulator, 1 of the 18 UTF-8 mutants
  (`(self.utf8_code << 6) | (b & 0x3f) as u32`). The shift leaves the low six bits zero, and
  `b & 0x3f` is exactly six bits, so the operands never share a set bit. OR and XOR agree on every
  such input, whatever a caller does.
- `Vt::push_scrollback_row`'s source index, 1 mutant (`row * cols` read as `row / cols`). It is a
  private method with one call site, `Vt::line_feed`, which always passes the literal `0`. The row
  about to scroll off the top is always the grid's first row, because scrolling makes room at the
  bottom. `0 * cols` and `0 / cols` are both `0` on the only input the function is ever given.
- `Vt::csi`'s private-use and intermediate-byte arm, 1 mutant (deleting
  `0x20..=0x2f | 0x3c..=0x3f => self.ignore = true`). No other arm in the same `match` covers those
  ranges (`b'0'..=b'9'`, `b';'`, `0x40..=0x7e`). So deleting the arm routes them to the catch-all
  `_ => { self.ignore = true; }`, which does the same thing. The two arms stay separate so a reader
  sees the two families of swallowed byte named apart: a sequence this engine chose not to
  implement, and a stray control code.
- `Vt::erase_display`'s damage guard, 1 mutant (`if to > from`). `to > from` holds on every
  reachable input. Mode 0's `end - here = (rows - row) * cols - col >= cols - col >= 1`, since
  `row < rows` and `col < cols` are invariants this type keeps everywhere. Mode 1's is
  `here + 1 >= 1`. The default's is `end = rows * cols >= 1`, because `Vt::clamp_cols` and
  `Vt::clamp_rows` floor both at 1. So `to == from` is unreachable, and `>=` cannot change the
  branch.
- `Vt::sgr`'s bit-independent recolouring, 3 mutants. Two are `(p as u8 - 30) | bright` (the
  16-color foreground codes, 30 to 37) and `(p as u8 - 90) | 8` (the bright foreground codes, 90
  to 97). Both left operands occupy bits 0 to 2 only, since `p - 30` and `p - 90` each range
  `0..=7`. `bright` and the literal `8` occupy bit 3 only. So OR and XOR agree on every input, as
  with the UTF-8 accumulator. The third is `p as u8 - 40` (the background codes, 40 to 47) read as
  `p as u8 + 40`. `Attr::new` masks its `bg` parameter with `& 0x07` before storing it. The two
  values differ by exactly 80, a multiple of 8, so the masked result is identical for every `p` in
  `40..=47`.

### No defect in shipped behavior was found

Every survivor was a missing test or one of the sixteen equivalences above. Each equivalence is
demonstrated, not asserted. The two cases that are not selector arguments (`erase_display` and
`pixel`) rest on an invariant named and cited at the function itself, not only here.
