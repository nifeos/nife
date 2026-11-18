# Where this stands

An appendix to [the milestone](../1088-elf-outside-review.md).

Finished: the rename to `static_elf` (#3595), and every item that needs no move (#3565, #3642).
Those are the citations, the seeds, the manifest and license files, the page-size input and the
casts. They also include `Display`, the D3 names, the README and its example, the proofs and the
entry-point refusal. D1 and D2 are
decided by the maintainer, provisionally. [The ledger](ledger.md) says
what each became.

What remains, with what holds each:

- The scaffold's other files and the fuzz target's move: release step, the move.
- The four mutation timeouts: release step, the first weekly run in the new repository.
- The stranger test: release step, the move.
- The shared test forge: other, not built; no finding stands behind it, and no lane holds it.
- `NATIVE_MACHINE`, kept public where D1 lists it to leave, and the provisional names: decision for
  the architect, at the crate's review.
- The move itself: decision for the architect. The crate has no review package under
  `notes/crate-reviews/`; a package comes first, then the architect's review clearance.
