# Where this stands

An appendix to [the milestone](../1083-clock-contract-outside-review.md).

Finished: Decisions 1 and 2 and items 5 and 12 (#3576), item 6 (#3559), and items 2, 4, 7 to 11,
13 to 15 with every Decision 3 name but two (#3640).
[The ledger](ledger.md) says what each became.

What remains, with what holds each:

- Item 1's two renames, `NS_PER_SECOND` to `NANOSECONDS_PER_SECOND` and `propose::req` to
  `request`: decided by the maintainer, provisional, and not built. #3567, which they waited for,
  has merged, so nothing holds them.
- Item 1's badge row, acronym expansions and `accepted-words` entries: release step, the move.
- Item 3, the first mutation run and its exemptions: release step, the move.
- The stranger test: release step, the move.
- The move itself: decision for the architect. The review package
  ([`notes/crate-reviews/clock-contract.md`](../../../notes/crate-reviews/clock-contract.md)) still
  waits on the overlay spellings, which Decision 3 decides provisionally. Once the two renames land,
  the package goes to the architect for review clearance.
