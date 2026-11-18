# Where this stands

An appendix to [the milestone](../1112-user-mode-runtime-outside-review.md).

Waiting on calef: ratifying item 13's names, and the move, which clearing a review package approves.

Finished: items 1, 6, 7, 10, 12, 14 and 16 (#3580), item 2 (#3527), items 4, 5, 11, 13 and 15
(#3611), item 17 (#3629). The two texts items 4 and 16 made stale are fixed. The crate's own host
test checks the `link.ld` base, so the `xtask` copy is gone. The manifest's `socket_contract`
comment states the real reason the frame type lives in the runtime. The maintainer has decided
Decisions 1, 2 and 3, provisionally.

What remains, and what holds each:

- Item 3, the manifest and the files the standalone check refuses: ready for a lane.
- Item 8, text that speaks to a reader inside nife (the sources still cite sections, milestones
  and `notes/`): ready for a lane.
- Item 9, acronyms not expanded at first use: ready for a lane.
- Ratifying the names item 13 renamed, and whether `read_u8` to `write_u64` give way to `read::<T>`
  (Decision 1): a decision for calef, at the names review the move requires.
- The move itself: a release step, the move, which waits on step 3 of milestone 1017 (the contract
  crates leave in this order). It also waits on a decision for calef, who clears a review package,
  which does not exist yet.
