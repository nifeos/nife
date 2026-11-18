# Where this stands

An appendix to [the milestone](../1081-capability-outside-review.md).

Finished: items 2 to 10, 12 and 15, and D1's crate name, by #3656 and #3674 and the lanes before
them; item 14 is declined with its reason. [The ledger](ledger.md)
says what each became.

What remains, with what holds each:

- Item 1, the standalone manifest, the license files and the README: release step, the move.
- Item 13, the `.github/mutants-exempts` lines: release step, the move.
- Item 11, a cargo feature for `peak`: release step; it is a published name, chosen at the crate's
  name review.
- The provisional names `delegate`, `GrantRequired`, `SlotOccupied`, `capacity` and `iter`:
  decision for the architect, at the crate's review.
- Item 3's two inline `GRANT` tests in the `REVOKE` arms of `kernel/src/syscall.rs`: other. No
  fork for the shape of a syscall-layer harness is written up, and no lane holds it.
- Item 16: other. It is optional, unmeasured, and no lane holds it.
- The move itself: decision for the architect. The crate has no review package under
  `notes/crate-reviews/`; a package comes first, then the architect's review clearance, before the
  repository exists.
