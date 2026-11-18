# Where this stands

An appendix to [the milestone](../1086-domain-name-resolution-contract-outside-review.md).

Finished: items 5 and 6 (Decisions 3 and 2, #3583), item 8, and the layout half of item 7.

Built and not merged: items 1, 4, 9, 12 and 13, the manifest of item 3, the rest of item 7, the
grant-table harness of item 10 and the size test of item 14, in #3641.

What remains after #3641, with what holds each:

- Item 2, the fixture's home: decision for the architect. It waits on decision 2 of the
  `socket_contract` package
  ([`notes/crate-reviews/socket-contract.md`](../../../notes/crate-reviews/socket-contract.md)).
- `OPERATION_ATTACH_PAGE_FRAME`: decision for the architect. It follows the socket contract's
  operation names, which that package lists for the architect.
- Item 14's refusal of an unknown transport: decision for the architect. It chooses a reply on the
  wire, and nobody has written up a fork yet.
- Item 3's `git` dependency and license files, item 10's fuzz target and item 11's exemptions
  (after a mutation census): release step, the move.
- A runtime log channel for the spawner: other; no lane holds it.
- The move itself: decision for the architect. The crate has no review package under
  `notes/crate-reviews/`; a package comes first, then the architect's review clearance.
