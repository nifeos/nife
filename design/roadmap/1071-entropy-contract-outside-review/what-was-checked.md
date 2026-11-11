# What was checked, and how

An appendix to [the milestone](../1071-entropy-contract-outside-review.md).

Verified firsthand, on a fresh shallow clone of `nifeos/nife` at `cf1328af` (the head of `main`
after #3480 merged), with the crate copied out of the workspace into a directory of its own:

- `cargo test` on stable and on 1.88, the manifest's `rust-version`: 15 unit tests and 4
  documentation tests pass, one of them the README's example.
- `cargo clippy --all-targets -- -D warnings` and `cargo doc` with `-D warnings -D
  rustdoc::broken_intra_doc_links -D rustdoc::invalid_html_tags`: clean.
- The library builds for `x86_64-unknown-none`, `aarch64-unknown-none-softfloat` and
  `riscv64imac-unknown-none-elf` on the tree's pinned nightly, and for `x86_64-unknown-none` on
  1.88.
- Kani 0.68.0 (CBMC 6.11.0): `Complete - 4 successfully verified harnesses, 0 failures, 4 total.`
- `cargo package --list`: 12 files, the four falsification patches among them; Cargo warns that the
  manifest has no `documentation`, `homepage` or `repository`.
- The template's helpers at its head (`2e651d8`), run over the crate: `spelled_out`,
  `unit_symbols`, `register_names`, `question_shape` and `abbreviations` report nothing;
  `readme_sections`, `readme_acronyms`, `inflated_wording`, `prose_figures`, `history_phrases`,
  `file_length`, `function_length`, `comment_blocks` and `orphan_modules` report nothing.
  `passive_voice` fails the README (item 3). `readme_badges` reports no badge row (item 4).
  `prose_caps` passes the README and fails `src/lib.rs`, which the template does not gate (item 16).
  `falsify.py --check` reports every harness as "replayable with no patch path" (item 13).
  `standalone.py` reports `publish = false`, 20 provenance lines and 2 lines naming the architect
  (items 8 and 14).
- Every caller in the tree (`kernel`, `components`, `crates/user_mode_runtime`,
  `crates/system_initializer`, `fixtures`, `system_tests`, the standard library overlay at
  `patches/std-nife/overlay/std/src/sys/random/nife.rs`), read for how it builds requests, reads
  replies and composes them.
- The recorded mutation figures: 40 mutants, 37 caught, 3 survivors, all three triaged as
  equivalent in `notes/project-metrics/mutation-triage.csv`, with reasons that hold on reading.
- The arithmetic claims of items 2, 11 and 17, by a throwaway test against the crate (not kept).
- Download figures for the prior-art crates, from the crates.io API.

Inferred from reading, not run: the dry run `script/crate-move --dry-run` and the template's
workflows on a staged tree (items 3, 4, 8, 13 and 14 say what those would report, from the
helpers run by hand). Nobody ran a mutation census; the figures above are the tree's records.

Not run: the stranger test, `cargo semver-checks` (no previous release), Miri, a fuzz run (the
crate has no target), `script/crate-standalone` and `script/names` (the package ran both over the
same names, and no public name has changed since).
