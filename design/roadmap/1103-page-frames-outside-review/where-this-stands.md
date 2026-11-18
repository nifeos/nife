# Where this stands

An appendix to [the milestone](../1103-page-frames-outside-review.md).

Waiting on calef: Decision 1 (`FRAME_SIZE` or `FRAME_BYTES`), and the move, which clearing a review
package approves.

Finished: item 4 (#3524), items 5, 6, 7, 9, 11, 13, 14, 15, 16, 18, 19 and 20 (#3567), and item
10 (#3521 for the regeneration, #3567 for the patches in the tree). The maintainer has decided
Decision 2, provisionally.

What remains, and what holds each:

- Items 1 and 2, the manifest, the license files and the README, written in nife before the move:
  ready for a lane.
- Item 3, the section, milestone and path citations left in `src/lib.rs`, `tests/allocator.rs` and
  the falsification headers: ready for a lane.
- Item 8, the renames (`FRAME_SIZE`, Decision 2's `physical_address` and `from_physical_address`,
  and the two-way names), which one lane performs together: a decision for calef (Decision 1).
- The template's gates at its head refuse two names no item lists: `question_shape` the private
  `fn get`, and `register_names` the test `an_unaligned_address_is_refused_without_a_panic`: ready
  for a lane.
- Item 12, a `free_contiguous` for a run: ready for a lane.
- Item 17, the mutation exemptions: a release step (the move).
- Decision 1, `FRAME_SIZE` or `FRAME_BYTES`: a decision for calef.
- The move: a decision for calef, taken by clearing a review package, which does not exist yet.
