# What was checked, and how

An appendix to [the milestone](../1086-domain-name-resolution-contract-outside-review.md).

Verified firsthand, on a fresh shallow clone of `main` at the merge of #3467:

- `cargo test -p domain_name_resolution_contract`: 20 unit tests and 1 documentation test pass on
  the pinned nightly. `cargo clippy --all-targets` is clean. `cargo doc --no-deps` with
  `rustdoc::broken_intra_doc_links`, `rustdoc::invalid_html_tags` and warnings denied is clean.
- `script/crate-move --dry-run --no-build` against a local checkout of the template at `v7`. It
  found 8 problems: no README, a manifest that does not stand alone in 10 lines, 5 citation classes
  and 54 lines that mention nife. It staged no falsification patch because the crate has none, so
  the review did not exercise the open `crate_move.py` fix (#3521, diff output no longer stripped).
  The build half (`--no-build` left out) was not run.
- The template's helpers ran at the template's head over the staged tree. `spelled_out.py` gave 2
  findings, `register_names.py` 4 findings, `abbreviations.py` gave 7 components that are not words
  (report only) and `prose_acronyms.py` gave 10 acronyms unexpanded at first use. `unit_symbols.py`
  and `question_shape.py` gave 0 findings over 228 names. `helpers/template_report_baseline.txt`
  lines 93 to 96 record the same four counts for this crate.
- `helpers/lint_externalized_crates.py` finds 54 hits. That is the crate's waiver ceiling at line 40
  of that file, so the crate gained none since the lint arrived.
- The review read every caller in the tree: `components/src/domain_name_resolver.rs` (the resolver),
  `crates/system_initializer/src/image_request.rs` and `children.rs` (the spawner),
  `patches/std-nife/overlay/std/src/sys/net/connection/nife.rs` (the std client, through the copy
  `xtask/src/farm/copied.rs` lines 117 to 121 makes of `src/wire.rs`),
  `fixtures/src/name_resolver_test_client.rs`, `system_tests/src/user/name_resolver_tests.rs`,
  `std_exerciser/src/bin/std_resolve.rs`, `crates/capability_manifest_note`, `crates/approval_file`,
  `crates/installed_programs` and `fuzz/fuzz_targets/capability_manifest_note_decode.rs`. The last
  five use `is_canonical_zone` and the two limits only.
- The rendered rustdoc of the crate root shows the layout table with its header and first row as a
  table and its second row as literal pipe characters. The cause is that the row for `OFF_ADDRESSES`
  breaks across two source lines (`src/lib.rs` lines 7 to 12).
- The recorded mutation figures are the census rows in `notes/project-metrics/mutation-census.csv`
  (the latest: 157 mutants, 134 caught, 6 missed, 0 timed out, 17 unviable) and the six missed sites
  in `notes/project-metrics/mutation-shape-sites.csv` lines 45 to 50. Each site is a `|` replaced by
  `^` between disjoint bit fields. No test can distinguish them because the operators agree on
  disjoint bits. `.cargo/mutants-baseline.txt` line 34 still carries the first census's row (112
  caught), which the later rows have overtaken.
- There is no `unsafe` block, no Kani harness, no falsification patch and no fuzz target of the
  crate's own. The one fuzz target that links it (`capability_manifest_note_decode`) links other
  crates of the tree as well, so the dry run keeps it here.

Inferred from reading, not run: the size of `Grants` (item 14), the behavior of the spawner when a grant draws a refusal (item 6), and the template's live workflows over the staged tree.

Not run: a mutation census (outside its window), the template's heavy jobs (coverage, Miri,
semver), a stranger test, `script/crate-standalone`, and the dry run's build half.
