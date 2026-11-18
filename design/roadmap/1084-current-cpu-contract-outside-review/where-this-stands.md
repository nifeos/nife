# Where this stands

An appendix to [the milestone](../1084-current-cpu-contract-outside-review.md).

Finished: items 1 to 11, 13, 15 and 16, and Decision A, by #3542, #3565 and #3657.
[The ledger](ledger.md) says what each became.

What remains, with what holds each:

- The provisional names of Decision B: decision for the architect, at the crate's review.
- Item 14, the type `u64` of `CPU_OFFSET_BYTES`: decision for the architect, at the same review.
- Item 12's farm check: other, not built. It needs a check that reads `cfg(test)` and `cfg(kani)`,
  and no lane holds it.
- The move itself: decision for the architect. The crate has no review package under
  `notes/crate-reviews/`; a package comes first, then the architect's review clearance, then the
  move in the order of milestone 1017 (the contract crates leave in this order).
