# Done when

An appendix to [the milestone](../1085-device-tree-blob-outside-review.md).

- `standalone.py --manifest-dir .` exits 0 in the new repository. Both license files, the inline
  license and lints, `rust-version` and a README in the template's section order are present. No
  citation of a milestone, a section, a path into another tree, a provenance block or a person
  remains.
- `spelled_out.py`, `british_spelling.py`, `unit_symbols.py` and `question_shape.py` report zero
  findings over `src/` and `tests/`.
- `node_reg` on a tree whose first prefix match carries no `reg` returns `Ok(0)`, with a hostile
  test. `node_prop` on the same tree still returns `Ok(None)`.
- `reserved_memory_regions` on a tree with `/soc/reserved-memory` returns `Ok(0)`, with a hostile
  test; the two existing reserved-memory tests still pass.
- Every cursor step is a checked add, or the crate refuses to compile for a pointer width other
  than 64 and the README says so.
- `from_bytes` refuses a structure block that runs past `size_dt_struct`, and a walker never reads
  a token at or past that end. A hostile test replaces `FDT_END` with `FDT_NOP` and sees
  `Truncated`.
- `cargo test`, `cargo clippy --all-targets`, `cargo doc` with warnings denied, `cargo kani` with
  every harness green and every falsification patch red, and `cargo mutants` with no missed mutant,
  at the commit the move tags.
- A `fuzz/` directory in the new repository builds the walk target on nightly, and the README says
  how to run it.
- The rulings on D1 to D6 sit in the record, and every public name carries the spelling ruled.

## The ledger

What this lane did with the items, kept here because the block itself and every source file the
items touch (`crates/device_tree_blob/src/lib.rs` and its hostile-input tests) are changed by an
open pull request (#3559), and a second edit to them would collide.

| item | state |
|---|---|
| P0-1, P0-2, P0-3, P1-5 | left: the move's, or edits to the source file above |
| P1-1 to P1-4 | done in #3559 |
| P2-1 to P2-6 and the rest of P2 | left: each edits the source file above; P2-1 (the error's shape) is also a published name |
| decisions | left: the architect's, as the block states them |
