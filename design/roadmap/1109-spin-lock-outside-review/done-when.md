# Done when

An appendix to [the milestone](../1109-spin-lock-outside-review.md).

- A template release carries a `loom` job, and the crate's caller enables it. The moved repository's
  `proofs` run shows five models executed, or the job fails when a crate declares `cfg(loom)` and
  none ran.
- `check / standalone` passes on the moved repository: README with `## EXAMPLES` and `## BUGS`,
  both license files, manifest literals with `rust-version`, no citation of a nife path, no
  `Name:` block.
- The moved manifest's `check-cfg` names `cfg(loom)` and `cargo clippy --all-targets -- -D
  warnings` is clean on stable.
- `cargo mutants --list` names 20 mutants, the `pause` line sits in `.github/mutants-exempts`
  with its reason, and one full `mutation` run on `main` is green.
- A falsification patch for the compare-exchange ordering exists and a gate replays it red.
- `cargo test` runs at least one doc-test, and a stranger run is on record with no open finding
  against the README.
- `get_mut`, `into_inner`, `Default`, `From<T>` and `Debug` exist with tests.
- The record carries the architect's answer on the name, and the package, repository and README agree
  with it.
