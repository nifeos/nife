# Where this stands

An appendix to [the milestone](../1101-memory-regions-outside-review.md).

Waiting on calef: Decision 1 (whether the crate leaves) and item 9's names.

Finished: item 1 (#3525), item 3's refusals (#3534), item 4 (#3532), items 7, 10, 12, 13, 14 and
15 and item 6(b) (#3567, with item 14's times in #3647), and item 6(d) (#3521). The maintainer has
decided Decisions 2, 3 and 4, provisionally.

What remains, and what holds each:

- Item 2, the crate cannot stand alone, with item 8's `documentation`, `readme` and `repository`
  and item 10's `BUGS` line: the move, which waits on Decision 1.
- Item 3, a Kani harness that runs any one operation from any accepted root: ready for a lane.
- Item 5, the loom runner: the move, which waits on Decision 1. The template's `proofs / loom` job
  (nifeos/crate-workflows#65) has merged and is in no release yet.
- Item 6(a), `ObjectKind::AddressSpace` under the template's address rule: ready for a lane, in
  nifeos/crate-workflows.
- Item 6(c), `CBMC` in `src/proofs.rs`, which the template's acronym gate refuses: ready for a lane.
- Item 6(e), `orphan_modules` at the pinned `v7`: a release step (a template release after `v7`,
  which carries the fix).
- Item 8, the dependency on `generational_table` by `git`: the publishing hold.
- Item 9, public names: a decision for calef (a naming pass).
- Item 11, a per-parent page count in place of the scan: ready for a lane.
- Item 16, the `Drop` panic and a by-value `return_to_parent`: ready for a lane.
- Decision 1, whether the crate leaves, and in what shape: a decision for calef.
- Decision 2, typed region names at the crate's boundary: ready for a lane.
- Decision 3, the crate refuses a count of zero and `retype_pages` leaves it: ready for a lane.
