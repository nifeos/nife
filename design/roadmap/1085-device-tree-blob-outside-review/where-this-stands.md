# Where this stands

An appendix to [the milestone](../1085-device-tree-blob-outside-review.md).

Finished: P0-2, P0-3, P1-1 to P1-4, and P2-1 to P2-14 but the README half of P2-9, by #3612 and the
lanes before it; D1 to D6 are decided by the maintainer, provisionally, and applied.

What remains, with what holds each:

- P0-1 (the manifest, the license files and the README), P1-5 (the `fuzz/` directory) and the
  README half of P2-9: release step, the move. Each lapses if nife takes `dtoolkit` first.
- The path to `dtoolkit`: other. Two fixes wait on review by the `dtoolkit` maintainers
  (google/dtoolkit #111 and #112), and the switch waits for a `dtoolkit` release that carries the
  gaps; taking it is then a dependency decision for the architect.
- `Region::size`, a length with no unit (the follow-on below): decision for the architect, at the move's naming
  review.
- The provisional names of D1 to D6: decision for the architect, at the crate's review.
- The move itself, if the crate still moves: decision for the architect. The crate has no review
  package under `notes/crate-reviews/`; a package comes first, then the architect's review
  clearance.
