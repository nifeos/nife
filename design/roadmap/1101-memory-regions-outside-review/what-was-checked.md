# What was checked, and how

An appendix to [the milestone](../1101-memory-regions-outside-review.md).

Verified firsthand, in a fresh depth-50 clone of `nifeos/nife` at `75bb255a` (the merge of #3515)
and a clone of `nifeos/crate-workflows` at its head (`v7` is the newest tag), on the tree's pinned
nightly.

- `cargo test -p memory_regions`: 30 unit tests, 6 documentation tests and 2 compile-fail
  documentation tests pass. `cargo clippy -p memory_regions --all-targets -- -D warnings` and
  `cargo doc` with broken links denied are clean. The crate builds for
  `riscv64imac-unknown-none-elf`.
- `cargo kani -p memory_regions` with Kani 0.68.0: all 10 harnesses verify. The whole run takes 14
  minutes 31 seconds wall clock, and the peak resident set is 3.6 gibibytes. The four arithmetic
  harnesses take under half a second each. The six subtree harnesses take 79 to 203 seconds each
  (`a_carve_keeps_every_count_exact_and_under_its_ceiling` the longest). The patch
  `proofs.split_stays_within_budget_and_progresses.patch` applied (at an offset of 15 lines in this
  tree) and turned its harness red on "attempt to add with overflow". The other nine patches were
  not replayed.
- The template's own gates, run by hand at the template's head over the crate's tracked Rust and
  manifest. `spelled_out`, `unit_symbols`, `british_spelling`, `service_model`, `contract_names`,
  `prose_figures`, `file_length`, `comment_blocks`, `function_length` and `orphan_modules` report
  nothing. `register_names` reports one name, `question_shape` two, `prose_acronyms` three, and
  `standalone` eleven lines. The `abbreviations` report (report only) lists 25 components in 3
  files. Items 2 and 6 carry them.
- `script/crate-move --dry-run --no-build --template <checkout> --template-rev v7
  memory_regions`: 9 findings for the move lane. They are no README, standalone, orphan modules at
  `v7`, one regenerated falsification patch the form gate refuses, citations, and the word `nife`.
  Items 2 and 6 carry them. The staged patch was then checked again with the template's head
  `falsify.py --check`, which refuses it too.
- Three throwaway tests in a scratch copy, deleted afterwards. One was an overflow on a root at the
  top of the page-number space (item 3). One was a double release undercounting the root (item 4).
  One was a child's ceiling under a small root ceiling (item 10).
- The dependency's source at the pinned revision, for the claim `NO_PARENT` makes about
  `u64::MAX`. The dependency documents the same word as `EntryName::NEVER_MINTED`, never minted.
- The tree's own records. These are the mutation census rows for the crate (the newest: 115 mutants,
  109 caught, 6 unviable, none missed) and `helpers/kani-peaks.tsv` (one row, 75, measured before
  the six subtree harnesses existed). They also include `notes/project-metrics/falsification-times.tsv`
  (rows for the four arithmetic harnesses only), the kernel caller `kernel/src/memory_region.rs`,
  the design note `notes/memory-regions.md`, the proposal `notes/region-split-on-a-full-table.md`,
  and the `interleavings` job of `.github/workflows/ci.yml`.
- The loom search, as `script/interleaving-check` runs it (`RUSTFLAGS="--cfg loom -D warnings"
  cargo test --release -p memory_regions --lib`). The six `interleavings` harnesses pass, the
  falsification witness among them, in 0.21 seconds after the build.

Inferred from reading, not run: the behavior of the kernel's `SPLIT` after item 1. The proposal
note says the syscall still answers `OutOfMemory`.

Not run: a mutation census (outside its window; the figures above are the tree's records). Also not
run: the template's workflows (no repository exists), a build at a `rust-version` (the manifest has
none), `cargo semver-checks` (no release), the stranger test and a publish rehearsal. Nothing ran
under a fuzz corpus, since the crate has no fuzz target of its own.
