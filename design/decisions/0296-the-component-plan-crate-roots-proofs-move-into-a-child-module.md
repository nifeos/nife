---
status: DECIDED
raised: 2026-10-12
decided: 2026-10-12
ratified_by: calef
---

# 296. The component-plan crate root's proofs move into a child module

Raised 2026-10-12 (UTC) as the cut that
[milestone 882 (the component-plan crate root moves its proofs out)](../roadmap/0882-the-component-plan-crate-root-moves-its-proofs-out.md)
says is calef's call before a line moves. Written by lane
`milestone/882-the-component-plan-crate-root-moves-its-proofs-out`. *(Section number provisional
until the merge queue lands it.)*

## The ruling

The lane recommended A, the proofs alone, and built it while the question waited. calef,
2026-10-12 02:28 (UTC), on [PR #1971](https://github.com/nifeos/nife/pull/1971): "A". The Kani
proofs move to `proofs.rs`, and `lib.rs` keeps the declaration family and orchestration. A child
named `proofs` does not make `lib.rs` a parent under §282, so it does not become an index.
Orchestration stays where it is; the file has not grown since 2026-10-03. File names ship
provisional for the naming pass.

## What the file is

`crates/component_plan/src/lib.rs` is 2,067 lines on main at `17b23264e`, 67 over the ceiling of
§266 (a Rust source file stays under 2,000 lines).

| what | lines |
|---|---|
| crate docs | 161 |
| the declaration, `Refusal`, `Plan`, `plan`, `slot_of`, `str_eq` | 616 |
| orchestration: `MAX_LIVE`, `LiveInstance`, `OrchestrationRefusal`, `Dependents`, `dependents` | 145 |
| `mod tests`, 31 host tests | 739 |
| `mod proofs`, inline, 5 Kani harnesses | 406 |

It grew from 1,558 lines on 2026-09-26 to 2,094 on 2026-10-03. Most of that is orchestration, 272
lines with its tests, from milestone 23 (a capability-routed component OS with live replacement).
The rest is the line editor's swap (131) and boundary tests (156) from milestone 637 (triage the
crates the 2026-09-21 mutation census measured for the first time). It has not grown since.

## What is already decided, and so is not asked

- **The proofs leave, not the host tests.** calef ruled for 842 (#1913), 845 (#1915) and 846
  (§275) that tests sit beside the code they test. The proofs are compiled only under `cfg(kani)`,
  with their own constants, helpers and a 22-line comment on their shape; a reader of `plan` never
  needs them. `crates/timetable/src/proofs.rs` is the precedent for a Kani file of its own.
- **A child named `proofs` does not make `lib.rs` a parent** under §282 (when a file is split, the
  parent becomes an index). `helpers/parent_index.py` exempts `tests` and `proofs`. So this move
  needs no index and no baseline row.
- **Any other child does.** `lib.rs` has no row in `design/parent-index-baseline.tsv`, so a second
  child, orchestration for instance, makes it an index the day it lands.
- **Names** ship provisional for calef's naming pass, which is not part of this ruling.

## The fork: does orchestration leave too?

The block names orchestration as a real seam. It is its own milestone 23 residual, with its own
refusal type, its own bound and its own banner. calef took the matching seam in 846, when he chose
"b" and moved the block layer out of the file server core. So this is a real contender, not
padding. Under §282, though, taking it is no longer one more file. It makes `lib.rs` an index.

| | A: proofs only | B: orchestration too, `lib.rs` an index |
|---|---|---|
| `lib.rs` | 1,665, not a parent | the crate docs, `mod` lines and re-exports, about 180 |
| children | `proofs.rs`, 411 | `plan.rs` with the whole declaration family and `plan/tests.rs`; `orchestration.rs` and `orchestration/tests.rs`; `proofs.rs` |
| visibility | none changes | `str_eq` becomes `pub(crate)`: `dependents` calls it twice and a proof calls it |
| falsification patches | all five apply unchanged | all five regenerated, since each diffs `lib.rs` |
| `design/comment-block-baseline.tsv` | both rows stay | the `dependents` row moves to `orchestration.rs` |
| regrowth | `lib.rs` stays the default home for new code | new code has to land in a named file |

Splitting the declaration family itself across files is not offered. A declaration goes in and a
plan or a refusal comes out, and the handoff page of §209 (state handoff is an opaque blob over a granted frame, and it is optional) is a field of the declaration. Cutting
it would make a reader open several files to follow one call to `plan`; the block refused it and
no precedent argues for it.

### In this tree

| precedent | what it did | bearing |
|---|---|---|
| §275 (846), file server core | tests out by topic, and the block layer out at calef's "b" | the seam was taken when offered |
| §272 (842), grant-plan crate root | private modules, four helpers kept in the parent | before §282 |
| §279 (843), system-initializer root | 5,850 lines to a 350-line index | a crate with many features |
| `crates/timetable`, 2,465 lines | proofs in `proofs.rs`, the rest in `lib.rs` | the shape A produces |

External peers do not decide this. §282 read the standard library and found it does both, and
this fork is about whether one seam justifies an index, which is §282's own question.

### Recommendation: A

At equal cost I would still choose A, narrowly, and for two reasons that are not effort.

1. B's index is a rename in disguise. The family that is four fifths of the code moves whole into
   `plan.rs`, so a reader opens `plan.rs` where they opened `lib.rs`, and gains one file named for
   145 lines that a reader of `plan` already skips.
2. B adds a cross-file dependency that A does not have. `dependents` compares names with the
   private `str_eq`, which would have to widen. §282 says `pub(super)` is an honest marker of such
   a dependency; here it would be the only one, created by the cut.

The regrowth argument for B is real but weak today: the file has been flat since 2026-10-03, and
A leaves it 335 lines under the ceiling. The time to take orchestration is when the transitive
version the crate's `BUGS` describes is built, since that is what would grow it.

Effort favors A too (five regenerated patches, a moved baseline row, 31 tests sorted), which is
why the margin above is stated as narrow.

If calef says B, the lane revises this section to B's layout, regenerates the five patches,
moves the comment-block row and splits the tests by topic beside their code. The proofs move done
for A is the first commit of B either way, so nothing is thrown away.

## What A does

Built on the recommendation while the question waited, and kept as built after calef ruled A.

- `lib.rs` keeps lines 1 to 1661 and ends with `#[cfg(kani)] mod proofs;` and a two-line comment.
- `proofs.rs` holds the module body, de-indented by four spaces and otherwise unchanged. The
  `///` doc on `mod proofs` became the file's `//!` block, with "the host tests above" reworded to
  "the host tests in `lib.rs`", and a `//! Name:` line marked provisional.
- `design/file-length-baseline.tsv` loses the file's row.
- Harness paths stay `proofs::<harness>`, so the five patch names and their rows in
  `notes/project-metrics/falsification-times.tsv` do not change.

No public crate API, wire format or system call changes under either answer.
