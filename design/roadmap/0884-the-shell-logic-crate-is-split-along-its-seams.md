---
status: BUILT
raised: 2026-10-11
built: 2026-10-12
milestone_dependencies: none
decision_dependencies: 266, 282, 290
machine_requirements: none
specific_machine: none
needs_person: no
---
# 884. The shell logic crate is split along its seams

*(The title and slug are provisional.)*

`crates/swish/src/lib.rs` is 3,850 lines at `396187b0b`. Its row in
`design/file-length-baseline.tsv` says 3,885. §266 (a Rust source file stays under
2,000 lines) sets the ceiling. Milestone 840 (the scheduler file is split along its seams) is the
model, and milestone 842 (the grant-plan crate root is split along its seams) is the closest case.

This is the host-testable crate behind the shell. Milestone 70 (`swish`'s remaining logic in a
crate) lifted it out of the program on 2026-08-02. It renders every sentence the prompt prints and
decides what a typed line is, with no capability in hand.

The cut and every module name below are calef's call. A lane writes them up as a
`status: PROPOSED` file in `design/decisions/` before it moves a line, and its pull request
carries `needs-architect`.

The lane wrote it up on 2026-10-12 (UTC) as §290 (the shell logic crate root splits into private
modules by feature). It asks nothing. §272 (the grant-plan crate root splits into private modules)
and §279 (the system-initializer crate root splits into private modules) answer the first two
questions below, and names go to calef's naming pass. §282 (when a file is split, the parent
becomes an index) was decided after this block was written, and it moves out of `lib.rs` the items
the cut below kept there.

Reuse: not applicable; this moves code and adds none. The crate already has four submodules
(`bare`, `complete`, `jig_versions`, `sequence`), each under 500 lines, which is the precedent.

## What is in the file

Measured on `396187b0b` from the file's own top-level items. Ranges start at each item's doc
comment.

| lines | count | what |
|---|---|---|
| 1 to 101 | 101 | crate docs, the four module declarations, imports |
| 102 to 317 | 216 | `Say`, `HEAP_MAX_BYTES`, `Status` and `$?`, `Route` and `route` |
| 318 to 564 | 247 | pattern versus text, `designation`, `write_set`, `echo`, quoting (`Piece`, `pieces`) |
| 565 to 703 | 139 | batching at the bound: `Sweep`, `write_sweep`, `write_batch`, `write_num` |
| 704 to 783 | 80 | `apropos`: what a documentation search prints |
| 784 to 887 | 104 | `time`: `Untimed`, `write_duration`, `write_timing` |
| 888 to 1148 | 261 | `write_say`, `write_pwd`, `write_help`, `write_refusal`, the fixed sentences |
| 1149 to 1250 | 102 | `caps <path>`: `write_image_caps`, `write_note_asks` |
| 1251 to 1350 | 100 | `write_vouch`, the usage lines, `write_outcome` |
| 1351 to 1501 | 151 | the shell's own endowment: `write_holdings`, `write_census`, the bind and config rows |
| 1502 to 2012 | 511 | `write_caps` over a whole line, `write_preview` and its 386-line row writer |
| 2014 to 3850 | 1,837 | `mod tests`, 83 `#[test]` functions |

The test module carries its own banners, one per topic above. The largest are the preview (381
lines), `caps` over a whole line (203) and the shell's own endowment (131).

## The verdict: split by feature, tests beside their code

Moving only the tests is not enough here. Lines 1 to 2013 are code, so `lib.rs` with a
`mod tests;` line would be 2,015 lines and still over the ceiling.

It is also the wrong shape. The file is not one type that should stay whole. It is a dozen
renderers, each with its own banner, its own milestone and its own tests. Each feature moves with
its tests into one private child module, so a lane adding a sentence edits one file of code and
the tests that hold it.

Nothing widens. Two private helpers are shared across features. `write_config_values` stays in
`lib.rs`, which every child can reach. `write_preview_rows` serves both `caps <path>` and the
preview, so those two share a module.

## A proposed cut

Every name here is provisional. The modules are private, and `lib.rs` re-exports their public
items, so every `swish::` path the program uses still resolves.

| module, provisional | from the table, with its tests | about |
|---|---|---|
| `lib.rs` | 1 to 317, `write_num`, `write_config_values`, re-exports; routing and `$?` tests, shared test helpers | 520 |
| `words` | 318 to 564; pattern, `echo`, quoting and designation tests | 600 |
| `batching` | 565 to 703 less `write_num` | 140 |
| `apropos` | 704 to 783 | 180 |
| `timing` | 784 to 887 | 200 |
| `sentences` | 888 to 1148, 1251 to 1350 | 550 |
| `endowment` | 1351 to 1501; endowment, bind and census tests | 340 |
| `preview` | 1149 to 1250, 1502 to 2012; preview, `caps` and `2>` tests | 1,380 |

`lib.rs` ends near 520 lines. No new file is over 1,500, which is 840's accepted standard.

## How this relates to 844

Milestone 844 (the shell program is split along its seams) covers `components/src/swish.rs`, the
program, and not this crate. Its section "Not the milestone-70 lever" refuses lifting more logic
here because this file would cross 4,000. After this milestone that reason is gone, since no file
in the crate is near 2,000. Milestone 70's other reason stands: what stayed in the program is
capability movement, not logic.

The two can run independently, in either order or at once. 844's Done when changes no Rust file
outside `components/src/swish/`. This one changes none outside `crates/swish/src/` except one
comment, below. With private modules and re-exports, none of the program's `swish::` paths
changes. If one goes first, take this one: it is smaller, and an architect ruling 844 can then
weigh the lever on its remaining reason alone.

Milestone 847 (the swish-check harness is split along its seams) does not key on this crate.
`xtask` does not depend on `swish`, and `xtask/src/swish_check.rs` spells `FAULTED_SENTENCE`'s
words itself. A move changes no sentence, so no transcript check moves.

## What an architect has to rule

1. Private modules with re-exports, or public modules. The recommendation is private: the four
   existing modules are public, but making these public would add a second path to every item.
   That is a public API change and a fork of its own. The reason holds at equal cost.
2. Where shared test helpers live: `shown`, `spec_of`, `listing` and `here_holds`. The
   recommendation is a `#[cfg(test)]` module in `lib.rs` that each child's tests import. That
   widens only test code.
3. Every module name in the table.

## What moving the code breaks

Found with `grep` at `396187b0b`.

- The comment-block ratchet, §267 (a comment states the constraint as it is now), lists two
  blocks here. `helpers/comment_block_ratchet.py` keys a row on path, first line and occurrence.
  It matches rows to blocks file by file in line order, by position, and never compares the line
  number. So a block that moves within `lib.rs` passes. A block that moves to a new path does
  not, and git's rename detection will not pair a new module with a `lib.rs` that still exists.
- `lib.rs:1`, 84 lines, is the crate doc. It stays in `lib.rs` and keeps its row. Any update to
  its "What is in here" list must not take it past 84 lines.
- `lib.rs:598`, 45 lines, is `write_sweep`'s doc and doctest, and lands in `batching`. The new file
  has no row, and adding one fails as a gained row. Its `lib.rs` row then has no block behind it.
  So the same change removes that row and cuts the block to 40 lines, or a fix is ruled
  separately. The cut can take the paragraph comparing Unix `xargs` to a note, which is §267's
  remedy, and leave the doctest's assertions alone.
- `design/file-length-baseline.tsv` row 18 goes in the same change, since the file ends under
  2,000.
- `helpers/cap_abbreviation.py` keeps `the_heap_cap_is_named_in_kib` by file. If that test moves,
  its key moves with it, or the count grows by one and the gate fails.
- `xtask/src/swish_check.rs` line 638 names "`swish::tests`'s preview test". That test moves to
  `preview`, so the comment changes. If 847 has moved the script table, the edit lands there.
- Doc comments link items by bare name, such as `write_caps`'s link to `write_holdings`. An
  intra-doc link resolves from the module that holds it, so each cross-module one needs a `crate::`
  path. The six doctests already use `swish::` paths and keep working.
- `script/lint`'s alloc check already greps the whole `crates/swish/src` directory, so a new
  module is covered. This is unlike 844, which must widen the same check on the program side.
- 15 tracked notes and design files name the path, and two cite it as `lib.rs:NNNN`.
  `notes/adding-a-program.md` sends a newcomer to `write_outcome` here; it should name `sentences`.
- No Kani harness and no falsification patch touches this file, and the crate has no
  `falsifications` directory.
- 15 first-parent merges in the 14 days to 2026-10-11 touched it, and 12 of them also touched the
  program. A lane adding a shell sentence edits both, so take this when few such lanes are open.

## Done when

1. `crates/swish/src/lib.rs` is under 2,000 lines, and no new file is over 1,500.
2. No Rust file outside `crates/swish/src` changes, except the one comment in
   `xtask/src/swish_check.rs`.
3. The crate's 83 host tests and 6 doctests pass with no assertion changed. `script/test` passes on
   aarch64, riscv64 and x86_64, and so does `script/swish-check`.
4. `helpers/comment_block_ratchet.py --check` and `helpers/cap_abbreviation.py --check` pass, with
   the `lib.rs:598` row and the file-length row removed.
5. `cargo doc` for the crate reports no broken intra-doc link.
6. Each new module carries a `//! Name:` block marked provisional.

## What was built

Built 2026-10-12 (UTC) on `17b23264e`. `crates/swish/src/lib.rs` went from 3,850 lines to 133, an
index of the crate docs (unchanged), `mod` and `use` lines and named re-exports, so its file-length
and parent-index rows are gone. Every name below is provisional and carries a `//! Name:` line.

| file | lines | tests | holds |
|---|---|---|---|
| `lib.rs` | 133 | | crate docs, `mod` and `use` lines, every public item re-exported by name |
| `say.rs` | 106 | 50 | `Say`, `HEAP_MAX_BYTES`, `write_say` |
| `status.rs` | 105 | 73 | `Status`, `STATUS_WORD` |
| `routing.rs` | 77 | 60 | `Route`, `route` |
| `words.rs` | 259 | 348 | pattern versus text, `designation`, `write_set`, `echo`, quoting |
| `batching.rs` | 132 | 49 | `Sweep`, `write_sweep`, `write_batch` |
| `number.rs` | 23 | 14 | `write_num` |
| `apropos.rs` | 89 | 100 | `write_found`, `write_apropos` |
| `timing.rs` | 114 | 76 | `Untimed`, `write_untimed`, `write_duration`, `write_timing` |
| `sentences.rs` | 285 | 152 | `pwd`, `help`, refusals, the fixed sentences, `write_vouch`, `write_outcome` |
| `endowment.rs` | 162 | 186 | `write_holdings`, `write_census`, the bind row, `write_config_values` |
| `preview.rs` | 672 | 740 | `Vouched`, `caps <path>`, `write_capabilities`, `write_preview`, the row writer |
| `test_support.rs` | 54 | | `shown`, `spec_of`, `listing`, `endowment` |

The tests column is each module's `tests.rs` child. The 83 tests moved unchanged, split by what each
calls rather than by the banner it sat under. Two were added: `script/coverage` counts no doctest,
and `write_sweep` and `route`'s `time` and `xargs` arms ran only in doctests, which the crate root's
average had hidden. Alone, `batching.rs` measured 18.9% against the 85% floor. Each child takes `use super::*` except `status`,
which needs nothing from the root. `write_config_values` became `pub(super)` and the test helpers
`pub(crate)`; nothing public widened.

One change reaches past `crates/swish/src`, beyond the comment in `xtask/src/swish_check.rs`.
Re-exporting `write_caps` and `write_image_caps` by name would grow the cap ratchet. So a commit
ahead of the move applies calef's 2026-10-06 (UTC) ruling against abbreviating capability early, as
#1913 did for `CapKind`. They are `write_capabilities` and `write_image_capabilities`, the
worklist's own target, and `components/src/swish/capabilities.rs` calls them by the new names.

`write_sweep`'s 45-line doc carried its comment-block row to `batching.rs`, and the heap cap test
its `cap` allowance to `say/tests.rs`. The notes that sent a reader to `lib.rs` for `write_outcome`,
the status and quoting name the module now.

## Follow-on

- **Decision.** §290,
  `design/decisions/0290-the-shell-logic-crate-root-splits-into-private-modules-by-feature.md`,
  asks nothing; it is the maintainer's to record as decided by precedent. Every module name is
  calef's in his naming pass.
- **Recorded.** `design/naming/capability-worklist.md` still lists `write_caps` and
  `write_image_caps`. A developer lane edits no record under `design/` but its own block, so the
  row is the maintainer's to mark done.
- **Refused.** Repointing the line-number citation of `write_apropos` in
  `design/roadmap/0279-documentation-service.md` here, for the same reason. The function is now in
  `crates/swish/src/apropos.rs`.

## Index row

The shell's host-tested logic crate is 3,850 lines, and its code alone is 2,013, so moving only
the tests would not bring it under the ceiling. This splits it by feature into private modules,
each with its own tests, behind unchanged public paths. It is independent of the program's split
in 844 and reopens the lifting lever 844 refused. The cut and every name are an architect's call.
