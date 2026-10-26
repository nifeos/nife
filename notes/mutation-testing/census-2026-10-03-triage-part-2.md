# The 2026-10-03 census's survivors, part two

Milestone 637 (triage the crates the 2026-09-21 mutation census measured for the first time),
provisional number, 2026-10-03. The first twelve crates are in
[census-2026-10-03-triage](census-2026-10-03-triage.md), which also states the method. Every number
here is `script/mutation -p <crate>` on the lane's worktree against the same run,
[37108924347](https://github.com/nifeos/nife/actions/runs/37108924347).

## documentation

65 missed: 32 killed, 33 equivalent. The 2026-09-13 ledger in [documentation](documentation.md)
recorded 17 equivalents and 30 deferrals for this crate. Eighteen of the 30 deferrals were
reachable with a fixture, and so were 14 survivors in the table fold that ledger never listed.

The new tests are in `crates/documentation/tests/render.rs`, under the survivor-triage headings.

The full sweep afterwards, 2026-10-03: 1,089 mutants, 949 caught, 66 timeouts, 41 unviable and 33
missed, the 33 equivalents below. That is 96.9% killed, from 93.8% in the census run.

### Killed (32)

- The table's last-column fold (`take_table_row`, 16). Blanks after the closing pipe, a tab among
  them, an escaped closing pipe, and a cell that ends in a backslash. The arena edge is the other
  half: four 2,046-byte rows and an indented 8-byte row land on 8,192 exactly and stay one chunk,
  while one byte more starts a second. The indented row is what separates `body.end - body.start`
  from a sum.
- The depth bound (6 mutants, 3 tests). Each test nests three levels with a different marker at
  each (strong, strike, link) and puts markup at the fourth, which must stay text. Because every
  site that deepens the scan is on one of those paths, `depth + 1` to `depth * 1` dies at each.
- Single-tilde and strike-closer arithmetic (2): `~a ~~b~~` and a strike that opens away from byte
  0 and byte 2, where `i + 2` and `i * 2` agree.
- `[label]` at the end of a line (3, `rb + 1 < end` in three forms). The previous line leaves a `(`
  in the byte after it, and a scanner that reads it looks for a `)` before the line starts.
- The image marker's width (`unit_bytes`), a wrapped destination with two-byte characters
  (`unit_wrapped`), and `before_unit`'s `>` at a margin of zero (`#  Title`, which pays a blank
  it does not owe).
- `past_quote` (2): a second `>` inside a quoted fence is code, and a line with fewer markers than
  the fence opened at is not indexed past its end.

### Equivalent (33)

Seventeen are the 2026-09-13 groups, unchanged. This pass added 16, which are the ones it
could not kill and could argue away.

- `lookup` (3), `search`'s `done < count`, `title_of`, `finish`'s `used > 0`, `line_done`'s two
  space-skips and `heading`'s text start: the 2026-09-13 ledger's groups, each still reported after
  a second sweep.
- The output cursor where nothing reads it (5): `rule`, the three `col +=` in `flush_table`'s row
  loops and its `c < cols[r]`. Also `vis > *width`, which assigns the same value under `>=`.
- `read_align` (2): starting at the pipe and walking `<=` past the end both land on a skipped,
  empty spec.
- `take_table_row` (6), new. The break that follows the last slot subsumes the loop test
  `i <= end && col < TABLE_COLS`. The `j - 1 == i` clause of the pipe test is redundant, because the
  byte before the slot is the previous pipe and never a backslash. `j > i` against `>=` stops on the
  separator pipe either way. `start + n < TABLE_TEXT` never binds: a row is at most `LINE_MAX`
  (2,048) bytes and the arena is flushed before it can overfill.
- `inline` (8), new. Every one reads or compares the byte at the end of a range, and each
  consequence falls out of the next check. A one-byte-ahead `strong` or `~` test at the last byte
  leaves `open` or `i + 2` past the end, where `closer` returns `None`. The blank-skip loop stepping
  to `end + 1` leaves `word` past the end, and `emit` ignores an empty or inverted range. An
  image's `!` read at the last byte finds no `]` in the empty remainder.
- `unit`'s uppercase copy (1): `(r.end - k).min(64)` as a division only changes the chunk size.
  Progress is guaranteed because `r.end > k`, so the bytes written are the same.

### What the equivalents say about the code

The `inline` equivalents are one shape, a lookahead guarded only by what follows it. Eight of them
disappear if the scanner takes its lookahead from a slice that stops at the range's end, which makes
the over-read unrepresentable and not merely harmless. That is the top rung of the ladder in
`CLAUDE.md`. It is filed as the proposal
[the-inline-scanner-reads-from-a-slice-that-ends-with-its-range](../../design/roadmap/0771-the-inline-scanner-reads-from-a-slice-that-ends-with-its-range.md)
and not done here, because it touches every branch of a renderer that 71 tests pin.

## filesystem_protocol

115 missed: 74 killed, 41 equivalent. The sweep afterwards covers 733 mutants and reports 41 missed.

All 74 kills are in the public `fixture` module, which the kernel tests and guest programs share
and which nothing on the host had asserted:

- `walk::sized_byte` (9) and `walk::wide_name` (19): the byte pattern and the name pattern a host
  check and a guest must agree on. Tests pin both at the positions where each operator differs.
- The `walk` totals (31): `WALK_DIRS`, `WALK_FILES`, `WALK_ENTRIES`, `WALK_BYTES` and
  `WALK_COMPONENTS`, with the hand derivation beside each number.
- Fifteen `twotrees` flag constants shifted to zero (`1 << n` as `1 >> n`), by a test that every
  flag is its own single bit and none repeats.

The 41 equivalents are three shapes.

- `1 << 0` as `1 >> 0` (10 constants): both are 1.
- `|` as `^` (29): every operand is a distinct single bit, or a field in its own bit range
  (`handle << 40`, `len & MAX_LEN`, a name length under 128 beside bit 7). The rights unions
  (`dir::ALL`, `REMOVE_TREE`, the verb table's pairs) and the witness's `EXPECTED` word are the
  largest groups.
- The `while i < TABLE.len()` in a `const _` assertion (2): the loop is the compile-time check that
  the table is in opcode order. A mutant of it only disables the check, and a build that still
  passes is the output.

## timetable

93 missed: 79 killed, 14 equivalent. The sweep afterwards reports 14 missed.

The oracle table has no interval rule and no range with an hourly step. Most kills are the rules it
never reached. Counts per group are approximate; the total of 79 is exact.

- Interval phase (`in_phase`). A line counted from `starting` in days, in weeks that begin on
  Monday, or in months. Each test picks a start whose day or week number is not a multiple of the
  interval. The weekly ones use 3 and 7 as well as 2, because a sum, a difference and a scaled
  count agree for 2.
- `range`'s edges. An hour that starts and ends together. A step that stops at the end and does not
  run to midnight. A range from midnight with no earlier hour to name. The last time reached,
  named from either side of the start minute. A span that is not a whole number of steps.
- The grammar's words. The day and month names the oracle never used (`wed`, `sun`, `feb`, `may`
  to `sep`, `nov`, `dec`), `1st`, `3rd` and `4th`, `expect`'s guard, and `first weekday` as an
  RRULE.
- Digits and widths. `+9` (which Rust parses as 9), `by 00060m`, `by 0m` (which would divide by
  zero), `24:00`, `12:60`, a one-digit hour, and each of the three date fields. A line that starts
  and ends the same day is accepted.
- `through` is inclusive, `HORIZON_DAYS` holds a 99-month gap, and `next_time` ignores hour bits
  past 23.
- Registration and the scheduler. `last_reachable`, `held`, the verdict word's second kept bit, a
  third identical line against two identical old ones (a second `lent` bit), an installed
  calendar line before and after the clock, an installed file operand, and the page layout's sums.

The 14 equivalents:

- `|` as `^` (3 in `registration`): `seq << 8` beside a byte, a kind beside a shifted code, a kept
  bit beside both. Each pair has disjoint bits.
- `next_time`'s start hour `after < 0` (3): the loop skips an hour whose floor is 59 or more anyway,
  so starting at hour 0 changes the work and not the answer.
- `next`'s horizon (2): `d + HORIZON_DAYS` as `d * HORIZON_DAYS` and `99 * 31 + 62` as `99 * 31 * 62`
  only make the search longer. A rule that matches nothing is answered `None` either way, only
  later.
- `range`'s `step < 60` as `<=` (1): a 60-minute step through the first branch builds the same masks
  and the same payload as the second.
- `range`'s `h < 24` (1): `h <= eh` has already failed, because `eh` is at most 23.
- `month_day_matches`'s `n > 0` (1) and `write_rrule`'s `n < 0` (1): `Nth(0, ..)` is not a value the
  grammar produces, and 1 to 4 and -1 are the only others.
- `due`'s `r.wall_next == SPENT` (1): redundant with `w < wall_next` unless the clock reads
  `i64::MAX` minutes.
- `unbacked`'s third clause, `Diagnostics::File` (1): `plan` passes no operators, so it cannot
  arrive today. The comment above it says it is there for the day that changes.

## video_terminal

64 missed: 37 killed, 27 equivalent. The sweep afterwards reports 27 missed.

- `script.rs` (25, all killed). Its scroller and window-line functions were exercised and never
  compared. The tests feed the driver's writes and the oracle's bytes into two terminals and compare
  every row, mangle exactly one line, and check `window_scroll_line`'s bytes at three counts.
- `Attr` (12, killed). Each flag's predicate, SGR 2, setting a flag twice, bold stopping at color 7
  (index 8 is already bright), the dim midpoint, and the underline on the last glyph row.
- `Damage::repaint_rect` with nothing scrolled (1, killed).

The 27 equivalents are the shapes the earlier ledger already names.

- `|` as `^` or `1 << 0` as `1 >> 0` (16): the channel packs in `Colour::resolve` and `midpoint`, `utf8_code`'s
  continuation bits, `BOLD`, and `sorted_channels`' three compare-and-swaps, where an equal pair swaps to
  itself.
- `CellRect::union`'s four comparisons (4): a tie picks the same value from either side.
- `clamp_cols` and `clamp_rows` at the maximum (2): the clamp returns the maximum either way.
- `push_scrollback_row`'s `row * cols` (1): `row` is always 0 at its one call site.
- The `0x20..=0x2f | 0x3c..=0x3f` arm (1), `erase_display`'s `to > from` (1), `extended_colour`'s
  `i < n` (1), and `pixel`'s `col < cols` (1): the check beside it subsumes each.
- `scroll_damage`'s `r > 0` guard and its `r - 1` cell (3): `take_damage` damages the same cell
  from the drawn position, and the box is a bounding box.

## machine_discovery

52 missed: 29 killed, 23 equivalent. The sweep afterwards reports 23 missed.

- DMAR (15). A structure of the minimum length 4 is skipped and the walk goes on. One that overruns
  the table ends it. A DRHD or RMRR shorter than its fixed part is ignored, and a scope keeps its
  start bus and every hop at depths 1 to 4. A scope with no hops or with five is marked and the
  list continues. A path is bounded on both sides.
- MADT (5). A GICC, distributor or redistributor that stops after its header is `Other`, and the
  GICC's enabled and online-capable bits are read separately.
- FADT (6). The ARM boot flags and the reset register are read from the offsets ACPI states less
  the 36-byte header, PSCI and HVC are separate bits, and the reset register is read whole.
- x86_64 (2). RDSEED needs its own bit, and a union of a set with itself does not clear it.
- GTDT (1). `GTDT_ACTIVE_LOW` is bit 1.

The 23 equivalents:

- Constant tables (6): the `while` loops of `aarch64`'s duplicate-code check and `riscv64`'s. Each is a
  compile-time assertion, and a mutant of it only disables the check.
- `1 << 0` and `|` against `^` on disjoint fields (9): the flag constants, `PixelOrder::store`'s
  channels, `parse_hex`'s nibble, `eid`'s byte, and `pmu_event`'s type and code.
- Guards that cannot differ on a 64-bit host, or on the value at hand (6).
  - `Framebuffer`'s `bytes <= usize::MAX` is always true on a 64-bit host.
  - `is_well_formed`'s `limit > base` fails the 4 KiB size test beside it at equality.
  - `read_scope_list`'s `len - 6` has the parity of `len + 6`.
  - `plic`'s `n < MAX_CONTEXT_HARTS`: the cpu list holds that many at most.
  - Two `<` against `<=` on equal values in `riscv64::Isa::from_device_tree`.
- Inside `#[cfg(kani)]` (2): `x86_64`'s `N`, in a proof module that has no function name for
  `exclude_re` to match.
