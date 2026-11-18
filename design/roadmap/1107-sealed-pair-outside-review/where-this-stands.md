# Where this stands

An appendix to [the milestone](../1107-sealed-pair-outside-review.md).

Nothing here waits on calef.

Finished: items 2, 3, 5, 7, 8, 10, 11 and 16 (#3580), items 6, 9 and 13 (#3610). Decision A has
a ruling, and the maintainer has decided Decision B, provisionally.

What remains, and what holds each:

- Item 1, the order of leaving: a release step (the move), which Decision A puts after `nifefs` and
  `measured_boot` have repositories of their own.
- Item 4, the crate does not stand alone: a release step (the move).
- Item 12, raw digests inside the archive: ready for a lane (the condition is in `BUGS`; neither
  the span change nor the test exists).
- Item 14, a second proof for the table comparison: ready for a lane.
- Item 15, `hex_string` and `measured_entries`: a release step (the move decides them).
- Decision A in the inventory's `sealed_pair` row and in the contract crates' leaving order: ready
  for a lane.
