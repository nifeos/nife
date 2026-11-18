# Where this stands

An appendix to [the milestone](../1102-network-time-protocol-outside-review.md).

Waiting on calef: the move, which clearing a review package approves; the package does not exist
yet.

Finished: items 13, 15, 16, 19, 20 and 21, item 12's saturating accessors, item 9's
`#![forbid(unsafe_code)]` and item 14's paragraph on `Query::accept` (#3567). The maintainer has decided
Decisions 1, 2 and 3, provisionally.

What remains, and what holds each:

- Item 7, a private `NANOSECONDS_PER_SECOND` in place of `clock_contract` (Decision 1): ready for a
  lane.
- Items 4, 5, 18 and 22, the names Decision 2 settles: ready for a lane.
- Item 17, `Query::new` and the low-bit randomizer leave (Decision 3): ready for a lane.
- Item 3, the contract-crate opening form: ready for a lane.
- Item 6, acronyms (`IPv4`, `DOWN` and `MAXSTRAT` still fail the template's gate): ready for a lane.
- Item 12, a Kani harness over the accessors: ready for a lane.
- Items 1, 2, 8, 10 and 11, item 9's fuzz opt-in and target, and item 14's README line: a release
  step (the move).
- Item 2's stranger record: another person (a stranger run before the first tag).
- The move: a decision for calef, taken by clearing a review package, which does not exist yet.
