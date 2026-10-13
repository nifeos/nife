---
status: BUILT
raised: 2026-10-11
built: 2026-10-12
milestone_dependencies: none
decision_dependencies: 266, 282, 294
machine_requirements: none
specific_machine: none
needs_person: no
---
# 877. The timetable crate root is split along its seams

*(The title and slug are provisional.)*

`crates/timetable/src/lib.rs` is 2,468 lines at `396187b0b`. It
has not grown since 2026-10-03. §266 (a Rust source file stays under 2,000
lines) sets the ceiling. Milestone 842 (the grant-plan crate root is split along its seams) is the
model for this block.

The cut and every module name below are calef's call. A lane writes them up as a
`status: PROPOSED` file in `design/decisions/` before it moves a line, and its pull request
carries `needs-architect`.

**BUILT, 2026-10-12 (UTC).** The lane wrote the cut up as §294 (the timetable crate root splits
into private modules). §282 (when a file is split, the parent becomes an index) answers its
rulings 1 to 3, and where the cut below disagrees, §282 holds: the registry leaves `lib.rs` too. `lib.rs` is 118 lines: the crate docs, the module lines, a named
re-export of every public item, and the two constants. The rest moved unchanged into four private
modules, each with its tests inline beside it: `document` (349 lines), `admission` (714),
`registry` (782) and `plan_text` (496), with `test_support` for the three helpers more than one
layer's tests use. The two tests of the verdict word joined `registration.rs`'s tests. Every module
name ships provisional. All five falsification patches patch `next_after` (none patches
`unbacked`), and they were regenerated against `registry.rs`.

Reuse: not applicable; this moves code and adds none. The crate already has four submodules. Two of
them, `recurrence` (1,271 lines) and `registration` (247), hold their own `mod tests` beside their
code. That is the precedent this cut follows.

## What is in the file

Measured on `396187b0b` from the file's own top-level items. Ranges are approximate at the edges,
where a doc comment belongs to the item below it.

| lines | count | what |
|---|---|---|
| 1 to 109 | 109 | crate docs, module declarations, `NANOS_PER_SEC`, `MAX_ENTRIES` |
| 110 to 335 | 226 | the document: `Schedule`, `Entry`, `Document`, `Error`, `parse` and its three helpers |
| 336 to 536 | 201 | `Held`, `Unbacked`, `Admission`, `Row`, the beat sentinels, `WallReading` |
| 537 to 860 | 324 | `Registry` and its one `impl` block |
| 861 to 973 | 113 | admission: `admit`, `admit_installed`, `schedulable`, `unbacked` |
| 974 to 1036 | 63 | `next_after`, the fire arithmetic the Kani proofs cover |
| 1037 to 1189 | 153 | `write_plan` and its four private writers |
| 1190 to 1309 | 120 | `Audit` |
| 1310 to 2466 | 1,157 | `mod tests`, 40 `#[test]` functions |
| 2467 to 2468 | 2 | `#[cfg(kani)] mod proofs;` |

The code is 1,309 lines and the tests are 47% of the file. The tests fall into three subjects:

| tests of | tests | about |
|---|---|---|
| the document: parsing, its errors, `Error::line`, `last_reachable` | 7 | 110 |
| the printed plan and `Audit` | 4 | 185 |
| registering, arming, firing, replacing, the calendar clock, installed programs | 29 | 860 |

## The verdict: two layers leave with their tests, and the registry stays

The code is not one type. It is three layers. The document parser turns text into entries and
never sees a grant. The registry decides what each entry becomes and when it fires. The printer
turns a registry into the sentences a person reads. Each outer layer has private helpers nothing
else calls: `strip_comment`, `split_word` and `interval_nanos` serve only `parse`, and the four
writers serve only `write_plan` and `Audit`.

So the by-feature axis fits here, and the tests-only axis does not. Moving only the tests would
leave `lib.rs` at about 1,310 lines and put 1,158 lines of tests in one `tests.rs`. That is
cheaper, and it meets the ceiling. It also splits each test from the layer it checks, which this
crate's `recurrence` module already declines to do.

The registry, admission and `next_after` stay in `lib.rs` with their 29 tests. They are one
subject: almost every one of those tests parses, registers, arms and fires.

## A proposed cut

Every name here is provisional. The new modules are private, and `lib.rs` re-exports their items,
so every path a caller uses today still resolves.

| module, provisional | from the table | about |
|---|---|---|
| `lib.rs` | 1 to 109, 336 to 1036, the registry's 29 tests, `mod proofs;`, re-exports | 1,690 |
| `document` | 110 to 335 and its 7 tests | 340 |
| `plan_text` | 1037 to 1309 and its 4 tests | 470 |

`plan_text` is not `plan`, on purpose. `grant_plan::plan` is the function every entry is checked
with, and a module of that name here would read as a second one.

`lib.rs` ends near 1,690 lines, 310 under the ceiling. It grew 849 lines in the nine days to
2026-10-03, so that margin is real but not large. Ruling 2 says what the next step would be.

## What an architect has to rule

1. By-feature or tests only. The recommendation is by-feature, for the reasons above. It costs
   more than tests only, so the recommendation is not about effort.
2. Whether the registry's 29 tests also leave, for a `tests.rs` beside `lib.rs`. That would take
   `lib.rs` to about 830. The recommendation is not now: they test the code they sit beside, and
   the file is under the ceiling without it. Take it if `lib.rs` nears 2,000 again.
3. Private modules with re-exports, or public modules like the crate's existing three. The
   recommendation is private. `contract` and `registration` are public because other programs read
   them as a protocol. The new modules are internal layers whose items already have root paths,
   and public ones would give each item a second path. The reason holds at equal cost.
4. Every module name in the third table.

No wire format or syscall changes under any answer here.

## What moving the code breaks

Found with `grep` at `396187b0b`.

- Five falsification patches under `crates/timetable/falsifications/` diff this file. Four patch
  `next_after` and one patches `unbacked`, and both stay in `lib.rs`, so the diffs need not change.
  Their hunk headers are already about 270 lines off and apply by offset. A lane confirms each still
  applies and fails. The file names carry the module path `proofs`, which does not move.
- No Kani harness is in this file. The ten in `proofs.rs` reach `crate::next_after` and
  `crate::recurrence`, which still resolve. The crate is named in `kani-reach.yml`'s list and in
  `script/verify`'s table by crate name, not path.
- The `#[cfg(kani)] mod proofs;` line must survive the move. Commit `d0b1ff821` dropped it once, and
  the proofs went stale with every gate green. Milestone 743 (no orphaned Rust source) now fails
  that in `script/lint`.
- Nothing copies or includes this file: no `#[path]`, no `include!`, and `xtask` does not copy the
  crate. Its tests `include_str!` `components/timetable.conf`. The moved printer tests use that
  constant and the `shown` helper, so those two test-only items become `pub(super)`. No item outside
  `#[cfg(test)]` widens.
- `write_plan` reads the private field `Registry::held`. A child module may read its parent's
  private fields, so it stays private. `Document`'s private fields move with it, and `lib.rs` reads
  them only through the public `entries()`.
- Intra-doc links in moved items, such as the link to `Registry::programs` in `Audit`'s docs,
  resolve from the module that holds them and need `crate::` paths.
- Three manifests depend on the crate (`components`, `fixtures`, `system_tests`), and six Rust
  files use it. With re-exports none changes. The doc tests already spell `timetable::parse`,
  `timetable::write_plan` and `timetable::Audit`.
- No `lib.rs:NNNN` citation names this file. The comment-block baseline's row for `lib.rs:1` keys
  the crate docs (82 lines), which do not move. Five design files name the path, each for an item
  that stays.
- `design/file-length-baseline.tsv` carries a row at 2,468. It must go in the same change.
- Four first-parent merges in the 14 days to 2026-10-11 touched it, so collision risk is low.

## Done when

1. `crates/timetable/src/lib.rs` is under 2,000 lines, no new file is over 1,500, and its row is
   gone from `design/file-length-baseline.tsv`.
2. No Rust file outside `crates/timetable` changes, and no item outside `#[cfg(test)]` widens.
3. All 40 tests from this file still run with no test body changed, and so do the crate's other
   host tests, its doc tests, and `script/test` on aarch64, riscv64 and x86_64.
4. The five patches that diff this file still apply and fail their harnesses.
5. `cargo doc` for the crate reports no broken intra-doc link.
6. Each new module carries a `//! Name:` block marked provisional.

## Follow-on

- **Decision.** The cut, and why §282 and earlier rulings answer this block's four questions, is
  `design/decisions/0294-the-timetable-crate-root-splits-into-private-modules.md`. Every module name
  ships provisional for calef's naming pass. No further split is owed: the largest file the cut
  made is `registry.rs` at 782 lines, and `lib.rs` is an index.

## Index row

`crates/timetable/src/lib.rs` is 2,468 lines: 1,309 of code in three layers and 1,157 of tests.
This moves the document parser and the plan printer into private child modules, each with its own
tests, and leaves the registry with its tests in `lib.rs` at about 1,690. The axis, the layout and
every name are an architect's call.
