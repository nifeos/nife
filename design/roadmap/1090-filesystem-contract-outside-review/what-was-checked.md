# What was checked, and how

An appendix to [the milestone](../1090-filesystem-contract-outside-review.md).

Verified firsthand on a fresh shallow clone of `main` (`137639a3`), inside a memory-capped scope:

- `cargo test -p filesystem_contract`: 58 unit tests and 6 documentation tests pass on the
  pinned nightly. The same, copied out of the workspace, passes on Rust 1.88, the manifest's
  `rust-version`, and on stable 1.99.1.
- `cargo clippy --all-targets -- -D warnings` is clean on the pinned nightly inside nife and red
  outside it. On stable 1.99.1 it reports 8 errors in the library and 9 in the test build, all
  `doc_markdown` on the word `RedoxFS`. Nife's workspace `clippy.toml` lists that word under
  `doc-valid-idents` and the scaffold does not. On 1.88 the same, plus one `dead_code` warning on `verb::in_opcode_order`.
- `cargo doc` with `RUSTDOCFLAGS=-D warnings` is clean.
- The library builds for `x86_64-unknown-none`, `aarch64-unknown-none-softfloat` and
  `riscv64imac-unknown-none-elf`.
- Kani 0.68.0 verifies the three harnesses in 6.6 seconds of wall clock at 383 MB resident; each
  falsification patch, applied with `patch -p3` from the crate root as the README says, turns its
  own harness red, and the source restores cleanly.
- `cargo package --list` ships `src/`, `falsifications/`, the manifest, the README and both
  license files.
- `cargo llvm-cov` per file: every source file is at 94 percent or above except `src/rm.rs`, at 0
  of 3 lines.
- The template's gates, run from a standalone copy with the template's own helpers, report the
  following. `spelled_out` finds 17, `register_names` 4, `question_shape` 1, `prose_acronyms` 12.
  `passive_voice` finds 5 in 754 README words against a cap of 1. `prose_caps` finds a longest
  sentence of 82 words against 40 and 5.5 bold spans per 1,000 words against 4. `standalone.py`
  finds five and `typos` one. `british_spelling`, `unit_symbols`, `service_model`, `contract_names`,
  `prose_figures`, `history_phrases`, `file_length`, `function_length`, `comment_blocks`,
  `orphan_modules`, `readme_sections`, `readme_acronyms` and `inflated_wording` are clean.
  `unsafe_scan` finds no `unsafe`.
- nife's externalization lint (`helpers/lint_externalized_crates.py`): 21 hits in the crate, 7
  `nife-word` and 14 `consumer-name`, at the waiver ceiling.
- nife's naming worklist (`helpers/name_checks.py crates/filesystem_contract`): 10 unratified
  wire-or-format names failing `spelled-out`.
- Every caller of the crate's decoders and packers in the tree, by grep, to see which invariants
  the callers hold that the crate does not.

Inferred from reading, not run: three things. One is the behavior of the `usize` casts on a 32-bit
target (item 19). Another is the overflow in `reply_errno` at `i64::MIN` (item 6, by the language's
rule for `-` on a signed integer in a debug build). The third is what the template's `coverage` job
would print (item 4, from the helper's `awk` and the lcov the review produced).

Not run: a mutation census (outside its window), the template's workflows end to end, a fuzz run,
a release dry run, Miri. The mutation figures below are the ones `notes/mutation-testing/` records.
