---
status: DECIDED
raised: 2026-10-12
decided: 2026-10-12
ratified_by: calef
---

# 295. The line editor's crate root splits into module files

Raised 2026-10-12 (UTC) as the cut that
[milestone 878 (the line editor's inline modules move to their own files)](../roadmap/0878-the-line-editors-inline-modules-move-to-their-own-files.md)
says is calef's call before a line moves. Written by lane
`milestone/878-the-line-editors-inline-modules-move-to-their-own-files`. *(Section number
provisional until the merge queue lands it.)*

It blocked the move and nothing else.

## The ruling

calef asked nothing separate on this section. The block's three questions are answered by
decisions he had already made, and the maintainer recorded that on 2026-10-12 at 02:58 (UTC) on
[PR #1969](https://github.com/nifeos/nife/pull/1969#issuecomment-6116941685): *"Settled by
precedent; nothing asked."*

1. By module, or tests only: by module. calef chose the code over the tests alone in §275 (the file
   server core's tests move into child modules by topic) and §274 (the filesystem-protocol crate
   root splits into its module files), and `proto`, `component` and `handoff` are already modules.
2. Whether the editor's 46 tests stay in `lib.rs`: no. Under §282 (when a file is split, the parent
   becomes an index), a crate root that declares a child holds only docs, `mod` lines, `use` and
   re-exports. `LineDisc` leaves too, and its tests go beside it in the shape §274 ruled: `x.rs`
   ends with `#[cfg(test)] mod tests;`, and the tests live in `x/tests.rs`.
3. File names were not asked. They ship provisional for calef's naming pass, as every split since
   §279 (the system-initializer crate root splits into private modules) has.

The layout below therefore goes further than the block's, which kept `LineDisc` and `RawQueue` in a
1,540-line `lib.rs`. §282 was decided after the block was written.

## What the file was

`crates/line_editor/src/lib.rs` was 2,290 lines on main at `17b23264e`: 144 lines of crate docs,
three inline public modules (`proto`, `component`, `handoff`), the editing engine, the raw queue,
the output side, and `mod tests` with 54 tests.

## The layout

| file | holds | lines |
|---|---|---|
| `lib.rs` | crate docs, the three public modules' docs on their `pub mod` lines, `mod` lines, re-exports | 198 |
| `line_disc.rs` | `LINE_MAX`, `HIST`, `PROMPT_MAX`, `Event`, `LineDisc`, the cursor-move helpers | 541 |
| `raw_queue.rs` | `RAW_QUEUE_MAX`, `RawQueue` | 82 |
| `output.rs` | `Sink`, `expand_output` | 35 |
| `proto.rs`, `component.rs`, `handoff.rs` | each module's body | 186, 171, 166 |
| `x/tests.rs` for `line_disc`, `handoff`, `raw_queue`, `proto`, `output` | 40, 6, 4, 3 and 1 tests | 18 to 576 |
| `test_support.rs` | `Screen` and `feed_all`, the terminal model the tests share | 105 |

`line_disc`, `raw_queue` and `output` are private. `lib.rs` re-exports their public items by name,
the 842 pattern, so every `line_editor::` path the 40 Rust files outside the crate name resolves as
before. The three public modules keep their paths. Their docs stay as `///` on the `pub mod` line,
as §274 kept them, so no doc text changed and its links still resolve from the crate root.

A test goes beside the module it tests, by §274's rule: beside `x` when every other module it names
is one `x` names. The six `handoff` tests drive a `LineDisc` and a `RawQueue`, which `handoff`
names, so they are `handoff`'s. Two of them use the editor tests' terminal model, which therefore
moves out of any one test module into `test_support.rs`, as milestone 872 (the kernel's user-mode
file is split along its seams) did. The block counted two `proto` tests; a third, that the two
start bits are distinct low bits, names only `proto` and joins them.

## What the move needed beyond moving lines

- `pub(super)` on 11 of `LineDisc`'s fields and all 3 of `RawQueue`'s, the ones `handoff::save`
  and `restore` read. In a child of the crate root, `pub(super)` means the crate, which is exactly
  where a private field of the crate root was visible, so nothing widens; it is the marker §282's
  third reason asks for.
- `pub(super)` on `Screen`, three of its fields, `new`, `text` and `feed_all`, test code only.
- Seven intra-doc links in `line_disc.rs` and `raw_queue.rs` gained an explicit `crate::` target,
  because they now resolve from a module that does not import the name.
- `use` lines at the top of each new file. No function body changed.
- The section comment over the mutation-census tests is repeated in each tests file that holds one
  of them, as §274 did.

## What else was considered

- The block's layout, `LineDisc` and `RawQueue` kept in `lib.rs`. §282 refuses it: the parent stays
  the default home for new code, which is how this file grew from 1,476 lines to 2,290 in 17 days.
  Keeping it would take an `Index exception` mark on about 20 items.
- `handoff` as a child of `line_disc`, so it reads the fields without `pub(super)`. That makes
  `line_disc.rs` a parent that must itself be an index, and moves `handoff` out of a path 40 files
  name or behind another re-export.
- `save` and `restore` moved beside their types, so no field is marked. The blob's layout would then
  be split across three files, away from the `Reader` and `Writer` that define it. Refused at equal
  cost.
- Tests only. Leaves a 1,310-line root holding three unrelated modules and the engine, the regrowth
  §282 exists to stop.

## Records that follow the move

- `design/file-length-baseline.tsv` loses the file's row. `design/comment-block-baseline.tsv`'s row
  for the crate docs at the top of `lib.rs` stays where it is.
- `lib.rs` is an index with no exception, so `design/parent-index-baseline.tsv` gains no row.
- `notes/project-metrics/mutation-triage.csv`: cargo-mutants 27.1.0 names a function by its inline
  module and not by its file, so `cargo mutants --list` dropped the `handoff::` prefix from 12 keys
  (and `proto::` from keys the ledger does not hold). The 12 rows are rewritten; every `line_editor`
  row matches a listed mutant.
- `notes/terminal-contract.md` links `LineDisc` in `line_disc.rs`.
- [Milestone 668 (scrollback from the keyboard)](../roadmap/0668-scrollback-from-the-keyboard.md)
  cites `csi_final` in `lib.rs`; it is in `line_disc.rs` now. That block is the maintainer's to
  repoint.

## What it does not decide

No public path, wire format, layout or system call changes. Every new module and file name is
provisional.
