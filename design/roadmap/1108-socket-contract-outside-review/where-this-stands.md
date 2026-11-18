# Where this stands

An appendix to [the milestone](../1108-socket-contract-outside-review.md).

Waiting on calef: item 9's names and the review package's open decisions.

Finished: items 1 and 4 (#3517), item 3 (#3526, its proof #3580), item 11 (#3580), item 7 (#3580,
#3610). D1 and D2 have rulings.

What remains, and what holds each:

- Item 2, the manifest: a release step (the move). The comment and `repository` are in place; the
  caller's `allow-unpublished` exists only in the new repository.
- Item 5, the README against the scaffold: a release step (the move).
- Item 6, the three equivalent mutants: a release step (the move).
- Item 8, one type for offsets and lengths: another crate, `user_mode_runtime`, whose window API
  takes `u64` offsets; no milestone holds that change. #3580 added `FRAME_BYTES` and the byte
  order.
- Item 9, names: a decision for calef (the naming pass, decision 4 of the review package).
- Item 10, inline "provisional" prose: a release step (the move).
- The move itself: a decision for calef (the review package's decisions 1, 2, 3 and 5: the identity
  values, where `fixture`, `ephemeral` and `polled_wait` live, and the clearance).
