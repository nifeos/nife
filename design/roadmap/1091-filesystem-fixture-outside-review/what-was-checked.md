# What was checked, and how

An appendix to [the milestone](../1091-filesystem-fixture-outside-review.md).

Verified firsthand on the clone (the review's report holds `hostname`, not this page):

- `cargo test -p filesystem_fixture` (16 unit tests and the README's one doctest), `cargo clippy
  --all-targets` and `cargo doc` with warnings denied. All are clean on the pinned nightly.
- `cargo build --lib` for `x86_64-unknown-none` and `riscv64imac-unknown-none-elf`, clean.
- The crate and `filesystem_contract` copied into a two-member scratch workspace and tested on Rust
  1.88, the manifest's `rust-version`. The 16 tests and the doctest pass, and the `x86_64-unknown-none`
  build is clean.
- `script/crate-standalone filesystem_fixture`: five failures (the crate name is provisional; 244 names lack ratification; the path dependency on `filesystem_contract` leaves the copy; it skipped the build and the proofs).
- `script/crate-review-packet filesystem_fixture`: 1,178 lines of library and 561 of tests, 14
  modules and 4 functions. It finds no `unsafe`, no Kani harness and no falsification patch. It
  finds nine dependents in nife (`byte_glob` and `system_tests` as dev-dependencies, the rest normal).
- `helpers/lint_externalized_crates.py --count`: 18 hits, exactly at the crate's ceiling. By hand:
  ten are the word `nife` (seven in prose, three in data literals). Eight are program names (`fs_subtree_caretaker` four times, `redoxfs_server` twice, `fs_nameset_caretaker` and `swish`
  once each).
- The template's own gates, run by hand from a clone of `nifeos/crate-workflows` with the arguments
  the `standalone` job passes: `spelled_out` 8 refusals, `register_names` 2, `prose_acronyms` 10.
  `unit_symbols`, `question_shape`, `british_spelling`, `contract_names`, `history_phrases`,
  `prose_figures`, `readme_sections`, `readme_acronyms` and `inflated_wording` are clean.
  `abbreviations` lists 88 components that are not words (report only). `standalone.py` has four
  findings. `typos` with the template's en-us configuration has two errors.
- A grep of every `pub` item against the tree outside the crate. Every item has a reader except
  the five of `durability`.
- The recorded mutation census (`notes/project-metrics/mutation-census.csv`): 216 mutants tested,
  183 caught, 14 missed, 19 unviable, no timeouts, in four consecutive weekly runs; the triage
  classifies all 14 as equivalent. No census ran for this review.
- The commit history of `crates/filesystem_contract/Cargo.toml` and of the crate through the
  GitHub API, to date the removal of the dev-dependency and the last changes.
- The arithmetic of the constants: the walk totals, the blank disk's partition bounds and alignment,
  the throughput shape's compile-time assertions, the Unix seconds of the `touch -t` instant, the
  BFS type code, and the two `const fn`s at their edges. The data partition starts and ends on
  4096-byte boundaries and stops 2,049 blocks short of the backup table.

Inferred from reading, not run: three questions. Does `proofs.yml` pass with zero harnesses? The
`counter_frequency_contract` package has the same shape, so the sibling move will show it. Does
`check / coverage` pass at its 85 percent floor? The four functions are all exercised. Does any
existing crate repository in the organization already depend on another by `git`? The review found none among the repositories listed, which is why item 1 is a template gap. Not run: the stranger
test, a mutation census, the publish rehearsal.
