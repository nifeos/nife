# Where this stands

An appendix to [the milestone](../1089-environment-contract-outside-review.md).

Finished: items 1 (#3529), 2, 8, 9 and 10, the `BUGS` half of item 12, and the first harness of
item 5 (#3566). Item 7 is finished with the identity freeze (#3585): `INTERFACE_ID` 13, `VERSION`
1, `MAGIC` `ENVCONF1` and a 107-byte page. The test `the_frozen_identity_and_widths_are_pinned`
pins all four.

What remains, with what holds each:

- Item 3, `PAGE_BYTES` to `PAGE_CONTENT_BYTES` and `ConfigPage` to `ConfigurationPage`: ruled, not
  built. The page-family rule and the spelled-out rule decide both.
- Item 4, `time_zone`, `locale` and `terminal_type` with their siblings: decided by the maintainer,
  provisional, and not built.
- Item 5's two wider harnesses: other, not built. Each needs its text scan bounded or its shape
  split before Kani finishes it.
- Item 11, a safe reader over a borrowed page: other, not built; a two-way, additive door the
  maintainer decides.
- Item 6's standalone files and item 12's README row: release step, the move.
- The provisional names: decision for the architect, at the crate's review.
- The move itself: decision for the architect. The crate has no review package under
  `notes/crate-reviews/`; a package comes first, then the architect's review clearance.
