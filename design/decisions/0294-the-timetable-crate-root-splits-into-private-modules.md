---
status: DECIDED
raised: 2026-10-12
decided: 2026-10-12
ratified_by: calef
---

# 294. The timetable crate root splits into private modules

Raised 2026-10-12 (UTC) as the cut that
[milestone 877 (the timetable crate root is split along its seams)](../roadmap/0877-the-timetable-crate-root-is-split-along-its-seams.md)
says is calef's call before a line moves. Written by lane
`milestone/877-the-timetable-crate-root-is-split-along-its-seams`. *(Section number provisional
until the merge queue lands it.)*

It blocked the move and nothing else.

## The ruling

calef asked nothing separate on this section. The block raised four questions on 2026-10-11,
before §282 (when a file is split, the parent becomes an index) was ruled, and rulings he had
already given answer each. The cut is by feature, as §272 (the grant-plan crate root splits into
private modules) and §279 (the system-initializer crate root splits into private modules) cut
theirs. The tests sit beside their code, per his rulings on
[#1913](https://github.com/nifeos/nife/pull/1913) and
[#1915](https://github.com/nifeos/nife/pull/1915). `lib.rs` becomes an index, per §282 (when a file is
split, the parent becomes an index), which calef set on
[#1959](https://github.com/nifeos/nife/pull/1959): *"Agreed, make the parent an index."* That
overrides the block's cut, which kept 1,690 lines in `lib.rs`. Modules are private with named re-exports, per
§272's fork 1 (*"Private modules"*). Names stay provisional for his naming pass. §294 stays its own
section, as §279 to §281 (the kernel's user-mode file splits into child modules of `user`) did,
rather than folding into §282. The maintainer recorded this on
[PR #1970](https://github.com/nifeos/nife/pull/1970#issuecomment-6116916808) at 02:55 (UTC) on
2026-10-12.

## What the file is today

`crates/timetable/src/lib.rs` is 2,468 lines on main at `17b23264e`, unchanged since the block
measured it at `396187b0b`. It holds three layers and their tests: the document parser, the
registry (what each entry becomes, and when it fires) and the printer. The block's own table has
the line ranges.

## The cut

Private modules, children of the crate root. `lib.rs` re-exports every public item by name, so no
path outside the crate changes. Each layer keeps its tests in an inline `mod tests` at its foot, as
this crate's `recurrence.rs` and `registration.rs` already do, so every moved test line is byte
identical. §272 (the grant-plan crate root splits into private modules) used `tests.rs` children
instead; here the files stay under 800 lines without them. Names are provisional. Measured as
built:

| file | holds | code | tests |
|---|---|---|---|
| `lib.rs` | crate docs, `mod` lines, named re-exports, `NANOS_PER_SEC`, `MAX_ENTRIES` | 118 | 0 |
| `document.rs` | `Schedule`, `Entry`, `Document`, `Error`, `parse` and its three helpers | 233 | 116, 7 tests |
| `admission.rs` | `Held`, `SHIPPED_HELD`, `Unbacked`, `Admission`; `admit` and its installed forms, `schedulable`, `unbacked` | 269 | 445, 13 tests |
| `registry.rs` | `Row`, the beat sentinels, `WallReading`, `Registry`; `next_after` | 457 | 325, 13 tests |
| `plan_text.rs` | `write_plan`, its four writers, `Audit` | 284 | 212, 5 tests |
| `test_support.rs` (`#[cfg(test)]`) | `REFERENCE`, `shown`, `with_clock`, which more than one layer's tests use | 29 | |

The two tests of the verdict word go to `registration.rs`'s own tests, beside
`registration::verdicts`, which they check; that file grows from 247 lines to 311. The two
constants stay in `lib.rs` as the exception §282 allows: one line each, and three of the four
layers use them. The largest file is `registry.rs` at 782 lines.

## The block's four questions, and what answers each

| block question | answered by | answer |
|---|---|---|
| 1. By feature, or tests only | §272 and §279 (the system-initializer crate root splits into private modules), and calef's rulings on #1913 and #1915 that tests sit beside the code they test, split by topic | By feature, each layer with its own tests. |
| 2. Whether the registry's 29 tests leave `lib.rs` | §282 | The registry itself leaves, so its tests go with it. The block's 1,690-line `lib.rs` is not an index. |
| 3. Private modules with re-exports, or public | §272's fork 1, calef: *"Private modules"* | Private, with named re-exports. `contract`, `recurrence` and `registration` stay public, as they are today. |
| 4. Every module name | calef's naming pass, as for #1919 and §272's fork 4 | Provisional, each marked with a `//! Name:` block. |

The block's premise for question 2 changed under it: it put the registry in `lib.rs` because the
file was then under the ceiling either way. §282 moves the registry out for regrowth, not for
length, which is this file's history: 1,619 lines on 2026-09-24 and 2,468 nine days later.

## What the move costs beyond moving code

- Visibility. Three items widen to `pub(super)`, which in a child of the crate root means the
  crate. `Registry::held`, a field, because `write_plan` reads it and now sits in a sibling; §282's
  third reason says that is where a reader should see the dependency. `admit` and
  `admit_installed_row`, because `Registry::register` and `Registry::plan_at_fire` call them. The
  test helpers in `test_support` are `pub(crate)` under `#[cfg(test)]`. Nothing else widens, and no
  public item changes. This is the block's done-when 2, as §282 amends it.
- The falsification patches. All five under `crates/timetable/falsifications/` that diff
  `lib.rs` patch `next_after`. The block thought one patched `unbacked`; its hunk header only names
  `unbacked` as context. Each is regenerated against `registry.rs` and shown to fail its harness
  again with `script/falsifications`.
- Intra-doc links in moved items resolve from their new module. Those the moved code's own imports
  do not cover get a `crate::` path. The crate docs' link to `Endowment` targets
  `grant_plan::Endowment`, since `lib.rs` no longer imports it.
- The ratchets. `design/file-length-baseline.tsv` and `design/parent-index-baseline.tsv` drop
  their rows for `lib.rs`. The crate docs stay at line 1 at 82 lines, so the comment-block row
  holds, and no moved comment block is over 40 lines.
- The Kani harnesses in `proofs.rs` name `crate::next_after` and `crate::recurrence`, which still
  resolve. `#[cfg(kani)] mod proofs;` stays in `lib.rs`.
- Citations under `design/` that name `lib.rs` for an item that moved are listed on the pull
  request for the maintainer, under §277 (cite a heading or a symbol, never a line number).

## What it does not decide

No public path, wire format or system call changes. No function body changes, and no test body.
