# Done when

An appendix to [the milestone](../1099-machine-statistics-contract-outside-review.md).

- `python3 helpers/name_release.py --draft machine_statistics_contract` prints every row
  `ratified`. The pre-ruled renames land, and `script/crate-standalone` passes step 0
  (item 1).
- The manifest carries the literal license, the literal lint tables, `rust-version`,
  `description`, `readme`, `keywords` and `categories`. Both license texts and a README with the
  scaffold's sections are at the crate root. `script/crate-standalone --no-kani` reports no
  manifest, file or citation failure. `helpers/lint_externalized_crates.py --count` reports
  zero for the crate (items 2 and 3).
- Every accessor saturates. A unit test pins the `u64::MAX` page, and the totality harness passes
  (item 4).
- `counter`'s signature carries no lifetime the contract does not promise. `read` says why its
  loads are `Relaxed`. A `const` assertion refuses a target whose pointer cannot hold the fixed
  address (item 5).
- The architect has frozen the identity set with the line count inside it. The crate carries its
  own line-count constant with the compile-time check. The review record states the values
  (item 6, Decision 1).
- Three Kani harnesses pass, each with a falsification patch that `proofs / falsify` replays red.
  The README's "What is proven" section names them (item 7).
- A host test reaches `read` over an aligned frame and passes under Miri. cargo-llvm-cov reports
  no uncovered line in `src/lib.rs` (item 8).
- The header indices and the line offsets live in separate namespaces. The new names are in
  the ratified table (item 9).
- `passive_voice` and `prose_acronyms` from the template's head report nothing over the crate's
  README and source. The BUGS sections state only the contract's limits (item 11).
- `cargo test`, `cargo clippy --all-targets -- -D warnings` on stable and on 1.88, `cargo doc`
  with warnings denied, `cargo kani`, Miri and the three bare-metal builds are green. They are green at the commit
  the move copies.
