# Done when

An appendix to [the milestone](../1101-memory-regions-outside-review.md).

- `split` refused for any reason leaves the parent as it was. The former doctest is a regression
  test. `a_refused_split_moves_nothing` covers the full-table refusal. No `# BUGS` entry in the
  crate, the design note or the kernel speaks of a phantom child.
- `insert_root_with_ceilings` refuses a zero-page root and a run that leaves the page-number
  space, with tests, and no input to any public method panics (a Kani harness says so).
- A second release of one object writes nothing and reports it. The root's count in item 4's
  scenario stays exact, and `a_release_keeps_every_count_exact` drives a release the region does not
  hold.
- The staged tree from `script/crate-move --dry-run` has no finding: README, licenses,
  `rust-version`, inline lints, no citation of this tree, no `nife`, and every regenerated patch
  passes `falsify.py --check`.
- Every template gate in `check / standalone` passes over the crate's source: `register_names`,
  `question_shape`, `prose_acronyms`, `orphan_modules`.
- The six loom harnesses run in the repository's CI, the witness among them, and the README names
  what loom checks and what Kani proves.
- Decisions 1 to 4 have rulings on record, and the public API carries the ruled shape before any
  tag.
- `cargo test`, `cargo kani`, the loom search and `cargo mutants` (none missed) are green at the
  commit a release would tag.
