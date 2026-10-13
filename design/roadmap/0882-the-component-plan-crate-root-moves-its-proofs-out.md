---
status: BUILT
raised: 2026-10-11
built: 2026-10-12
milestone_dependencies: none
decision_dependencies: 266, 296
machine_requirements: none
specific_machine: none
needs_person: no
---
# 882. The component-plan crate root moves its proofs out

*(The title and slug are provisional.)*

`crates/component_plan/src/lib.rs` is 2,067 lines at `396187b0b`, the smallest file over the
ceiling that §266 (a Rust source file stays under 2,000 lines) sets.
Milestone 846 (the file-server core moves its tests out) is the model, and this block
reaches a similar answer for a different layer.

Its row in `design/file-length-baseline.tsv` says 2,094. The file lost 27 lines on 2026-10-09,
when milestone 863 (the comment-block sweep continues) cut its crate docs, and nobody banked the
row. The ratchet allows that, since a row is a ceiling that only falls.

The layout and every name below are calef's call. A lane writes them up as a `status: PROPOSED`
file in `design/decisions/` before it moves a line, and its pull request carries
`needs-architect`.

The lane wrote it up on 2026-10-12 (UTC) as §296 (the component-plan crate root's proofs move into
a child module), re-measured on main at `17b23264e` (2,067 lines). Which part leaves first is
answered by calef's rulings on 842, 845 and 846: tests sit beside their code. Under §282 (when a
file is split, the parent becomes an index), the orchestration question became a choice between
this move and an index. That was the one question asked, and calef answered "A" (proofs only) on
2026-10-12 (UTC), on #1971. §296 is DECIDED.

Reuse: not applicable; this moves code and adds none. `crates/timetable/src/proofs.rs` is the
precedent in this tree for Kani harnesses in their own file.

## What is in the file

Measured on `396187b0b` from the file's own top-level items. Ranges are approximate at the edges.

| lines | count | what |
|---|---|---|
| 1 to 161 | 161 | crate docs, one comment block of 158 lines |
| 162 to 508 | 347 | the declaration: the bounds, `Direction`, `CapNeed`, `MapNeed`, `Handoff`, `Requirements`, `Provisions` |
| 509 to 647 | 139 | `Refusal`, `Plan` and its accessors |
| 648 to 777 | 130 | `plan`, `slot_of`, `str_eq` |
| 778 to 922 | 145 | dependency-aware orchestration: `LiveInstance`, `Dependents`, `dependents` |
| 923 to 1661 | 739 | `mod tests`, 31 `#[test]` functions |
| 1662 to 2067 | 406 | `mod proofs`, inline, 5 Kani harnesses with their falsification records |

The code is 922 lines, docs included. Of the 406 lines of proofs, 124 prove orchestration and the
rest prove the wiring.

## The verdict: a small move, and the proofs are the part to move

The planning code is one family of types. A declaration goes in, and a plan or a refusal comes
out. The handoff page of §209 (state handoff is an opaque blob over a granted frame, and it is
optional) is a field of the declaration, not a separate feature. Cutting the family across files
would make a reader open several to follow one call to `plan`. A fuller split is not worth taking
at 922 lines of code.

Four single moves bring the file under 2,000:

| move | `lib.rs` ends near | what else changes |
|---|---|---|
| the proofs to `proofs.rs` | 1,665 | nothing |
| the host tests to `tests.rs` | 1,330 | nothing |
| orchestration, with its host tests | 1,750 | one patch, one comment-block row |
| orchestration, with its tests and its two proofs | 1,625 | two harness paths, so two patch names; one patch diff |

The recommendation is the first. The proofs are a separate layer: compiled only under
`cfg(kani)`, with their own constants, their own helpers and a 22-line comment on why they are
shaped as they are. A reader of the planning code never needs them. The host tests are different.
They are the cases a reader checks the code against. Milestone 840 (the scheduler file is split
along its seams) set the standard of tests beside their code, and it holds here.

Moving the proofs also moves no name. `script/falsifications` derives a harness path from the file
as well as the inline `mod` blocks, so `proofs.rs` under `lib.rs` still yields
`proofs::<harness>`. The five patches, their file names and the five rows in
`notes/project-metrics/falsification-times.tsv` stay as they are.

Inline is the more common shape in this tree: nine crates keep `mod proofs { }` inline, and two
keep a file. That convention exists, but nothing rules it, and this crate is the one over the
ceiling.

## A proposed layout

Every name here is provisional.

| file, provisional | from the table | about |
|---|---|---|
| `lib.rs` | 1 to 1661, and `#[cfg(kani)] mod proofs;` | 1,665 |
| `proofs.rs` | 1662 to 2067 | 410 |

## What an architect has to rule

1. Whether the proofs, rather than the host tests, are what leaves. The recommendation is the
   proofs, for the reasons above. They hold at equal cost.
2. Whether orchestration also leaves now, as a child module with its host tests. It is a real
   seam: its own milestone 23 (a capability-routed component OS with live replacement) residual,
   its own refusal type, its own bound and its own banner. At equal cost it would be worth taking.
   The recommendation is to leave it, and that is about effort: one regenerated patch and a moved
   baseline row buy 145 lines of code a reader of `plan` can already skip. Take it when the
   transitive version this crate's `BUGS` describes is built.
3. The file name `proofs.rs`.

No public crate API, wire format or syscall changes under any answer here.

## What moving the code breaks

Found with `grep` at `396187b0b`.

- Five falsification patches under `crates/component_plan/falsifications/` diff this file. They
  patch the `impl` blocks of `Direction` and `PageKind`, and `plan`, `str_eq` and `dependents`. All
  of that stays in `lib.rs`, so no patch changes. Their hunk headers have been 27 lines off since
  milestone 863 and apply by offset. A lane confirms each still applies and fails.
- The five `Falsification:` records sit above their harnesses and move with them.
- The crate is named in `kani-reach.yml`'s list and in `script/verify`'s table by crate name, not
  path.
- `#[cfg(kani)] mod proofs;` is the line a careless move drops. Commit `d0b1ff821` dropped
  `timetable`'s, and its proofs went stale with every gate green. Milestone 743 (no orphaned Rust
  source) now fails that in `script/lint`.
- The `///` doc comment on `mod proofs` becomes the new file's `//!` block. It says "the host
  tests above", which needs rewording.
- Nothing copies or includes this file: no `#[path]`, no `include!`, and `xtask` does not copy the
  crate into std. `helpers/interface_stability.py` lists `component_plan` as a contract crate and
  reads its public API from rustdoc. A private `cfg(kani)` module is not part of it.
- Four manifests depend on the crate (`components`, `fixtures`, `line_editor`, `swap_protocol`), and
  seven Rust files use it. None changes.
- Milestone 849 (a package declares what it needs at run time, apart from what it links) cites
  `lib.rs:355`, `:216` and `:396`. They were right at 2,094 lines and have pointed 27 lines too low
  since milestone 863. This move does not shift them, since all three are above line 1662. They
  are stale already, and a function name would keep them true.
- `design/comment-block-baseline.tsv` keys two rows on this file, at `lib.rs:1` and `lib.rs:849`:
  the crate docs (158 lines) and the doc of `dependents` (46 lines). Both stay in `lib.rs`, so
  neither is affected.
- The row in `design/file-length-baseline.tsv` goes in the same change.
- Six first-parent merges in the 14 days to 2026-10-11 touched it.

## Done when

1. `crates/component_plan/src/lib.rs` is under 2,000 lines, and its row is gone from
   `design/file-length-baseline.tsv`.
2. No Rust file outside `crates/component_plan/src/` changes, and no item's visibility widens.
3. The 31 host tests and the doc tests pass with no test body changed, and so does `script/test` on
   aarch64, riscv64 and x86_64.
4. The five Kani harnesses keep their paths, pass, and each of the five patches still makes its
   harness fail.
5. `proofs.rs` carries a `//! Name:` line marked provisional.

## What was built

Built 2026-10-12 (UTC) on #1971, as §296 records. `crates/component_plan/src/lib.rs` went from
2,067 lines to 1,665 and ends with `#[cfg(kani)] mod proofs;`. The five Kani harnesses moved
unchanged into `crates/component_plan/src/proofs.rs` (411 lines, a provisional `//! Name:` line),
and the file's row is gone from `design/file-length-baseline.tsv`. Harness paths stay
`proofs::<harness>`, so no falsification patch, patch name or `falsification-times.tsv` row
changed. Orchestration stays in `lib.rs`. The stale citations in milestone 849 are not touched
here.

## Follow-on

- **Decision.** Orchestration stays in `lib.rs`. `design/decisions/0296-the-component-plan-crate-roots-proofs-move-into-a-child-module.md`
  records calef's "A" and the time to revisit it: when the transitive orchestration this crate's
  `BUGS` describes is built, since that is what would grow the file. The same section ships the
  new file's name provisional for calef's name-ratification pass.
- **Recorded.** `design/roadmap/0849-a-package-declares-what-it-needs-at-run-time.md` cites this
  file by line number for `Requirements`, `Direction` and `Requirements::depends_on`, and those
  numbers have been 27 lines low since milestone 863. This move does not shift them. A lane edits
  only its own block, so they are listed for the maintainer on #1971.

## Index row

`crates/component_plan/src/lib.rs` is 2,067 lines, 67 over the ceiling, and its baseline row still
says 2,094. Its planning code is one family of types and should not be cut. This moves the inline
Kani proofs into `proofs.rs`, which brings the file to about 1,665 and changes no harness path or
patch. Orchestration stays, at calef's ruling (§296); names ship provisional.
