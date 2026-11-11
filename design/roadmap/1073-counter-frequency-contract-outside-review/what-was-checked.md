# What was checked, and how

An appendix to [the milestone](../1073-counter-frequency-contract-outside-review.md).

Verified firsthand, in a fresh shallow clone of `nifeos/nife` at the merge of pull request 3461
and a clone of `nifeos/crate-workflows` at its head, on a copy of the crate standing alone:

- `cargo fmt --check` (the template's `rustfmt.toml`), `cargo test` (five unit tests, four
  documentation tests, the README's example among them) and `cargo doc` with broken links and
  warnings denied: clean on stable 1.99.1 and on 1.88, the manifest's `rust-version`.
- `cargo clippy --all-targets -- -D warnings`: red on stable 1.99.1 and on 1.88 (item 2), green on
  the nightly this tree pins. The template's `--cfg kani` clippy pass over its shim: green.
- The crate builds and passes clippy for `x86_64-unknown-none`, `aarch64-unknown-none-softfloat`
  and `riscv64imac-unknown-none-elf`, apart from item 2, which is the same line on every target.
- Miri (`cargo miri test` on the pinned nightly): all nine tests pass.
- cargo-llvm-cov: 100% of regions, functions and lines in `src/lib.rs`.
- The template's helpers over the standalone copy:
  - `standalone.py` fails only on the `Name:` blocks, which the move strips.
  - `spelled_out`, `unit_symbols`, `register_names`, `contract_names`, `british_spelling`,
    `question_shape`, `service_model`, `abbreviations`, `readme_sections`, `readme_acronyms`,
    `inflated_wording`, `history_phrases`, `prose_figures`, `function_length`, `file_length` and
    `comment_blocks` report nothing.
  - `prose_acronyms` and `passive_voice` report findings (item 8).
  - `unsafe_scan` refuses a fuzz opt-in, correctly, for the four `unsafe` sites.
- `script/crate-move --dry-run` into an empty directory. The template at `v7` scaffolds. The host
  tests and the three bare-metal builds pass on 1.88, and the pinned template's cheap checks pass.
  The three findings it reports are the README's nife sentence (baseline) and two lines in the
  template's own scaffold (BUGS). The mutation baseline row it carries over reads "missed 0".
- Every caller: `kernel/src/user/loader.rs` (the writer and the mapper), the three
  `kernel/src/arch/*/timer.rs` (the band), `crates/user_mode_runtime/src/clock.rs` (the reader),
  `crates/process_builder/src/lib.rs` (the relay), five programs under `components/src/` (the
  `INTERFACES` declaration), `system_tests/src/user/counter_frequency_tests.rs`, and the `std`
  platform layer that `xtask/src/farm/generate.rs` writes from this crate's source.

Inferred from reading, not run: the behavior on a target whose `usize` is narrower than 64 bits
(item 5). Also inferred: whether the template's `proofs / kani` job passes on a crate with no
harness (item 9), and why the pinned nightly's clippy accepts the line stable refuses (item 2).

Not run: a mutation census (outside its window; the carried row says none missed). Also not run
are a fuzz campaign (the crate has no fuzz target and, with `unsafe` in shipped source, cannot opt
in to the public one). Nor did the review run `cargo semver-checks` (no release to compare
against) or the template's full workflow set against a real repository.
