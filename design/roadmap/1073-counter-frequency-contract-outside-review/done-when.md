# Done when

An appendix to [the milestone](../1073-counter-frequency-contract-outside-review.md).

- `cargo xtask std-src` and the farm stamp read every copied contract crate's source from the
  checkout `cargo metadata` names, proven by a build of the `std` farm with one such crate pinned
  by `rev` (item 1).
- `cargo +stable clippy --all-targets -- -D warnings` and the template's `--cfg kani` clippy pass
  are green on the standalone copy, on 1.88 and on the current stable (item 2).
- The BUGS sections of the README and the module documentation describe only what the contract
  cannot express, and no sentence in the crate states what any kernel checks (item 3).
- `hertz`'s documentation and the README state that the band is the reader's to apply. A unit
  test pins `build_page(0)` to `Some(0)`, and `user_mode_runtime::cntfrq_checked` applies
  `is_plausible` (item 4).
- A `const` assertion refuses a target whose pointer cannot hold `PAGE_VIRTUAL_ADDRESS`, and
  `new`'s safety section says so (item 5).
- No file in the crate names a path, a crate or a machine outside the repository, apart from the
  cited register names. `script/crate-move --dry-run` reports no finding against the crate's own
  files (items 6 and 7 and the baseline's three edits).
- `prose_acronyms` and `passive_voice` from the template's head report nothing over the crate's
  README and source (item 8).
- Two Kani harnesses pass, each with a falsification patch that `proofs / falsify` replays red.
  The README's "What is proven" section names them (item 9).
- The architect has frozen the identity set, recorded by its values in the review package.
- `cargo test`, `cargo clippy` on stable, `cargo doc` with warnings denied, `cargo kani` and the
  three bare-metal builds are green at the commit the move copies.
