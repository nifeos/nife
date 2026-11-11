# What was checked, and how

An appendix to [the milestone](../1083-clock-contract-outside-review.md).

Verified firsthand, on a copy of the crate taken out of the workspace (its manifest minus the
`publish` line, no workspace file), which is the shape the move produces:

- `cargo test`: 12 host tests and 3 doctests pass on stable 1.99.1 and on 1.88, the manifest's
  `rust-version`. The README's example is one of the doctests.
- `RUSTFLAGS="--cfg loom" cargo test --release`: the four interleaving models pass in 12 seconds.
- `cargo build --lib` for `x86_64-unknown-none`, `aarch64-unknown-none-softfloat` and
  `riscv64imac-unknown-none-elf`.
- `cargo doc` with warnings denied, `cargo fmt --check`, `cargo package --list` (seven files plus
  `Cargo.lock`; one warning, the missing `repository` key).
- `cargo clippy --all-targets -- -D warnings` fails on stable, on 1.88 and under `--cfg kani`, in
  the same place (item 2). Inside nife it passes because nife's `clippy.toml` carries the word.
- `cargo llvm-cov`: 95.85% of lines, 97.02% of regions. The crate meets the template's floor of 85% per file.
  The nine uncovered lines are the reader's spin branch, the writer's retry branch and `spin_hint`.
- `cargo kani` and the template's `falsify.py --check`: both pass with zero harnesses, by saying
  there is nothing to verify.
- The template's `standalone` job, step by step, with its helpers at head: `spelled_out`,
  `unit_symbols`, `register_names`, `british_spelling`, `question_shape`, `contract_names`,
  `service_model`, `abbreviations`, `prose_acronyms`, `prose_figures`, `passive_voice`,
  `history_phrases`, `inflated_wording`, `readme_sections`, `readme_badges`, `file_length`,
  `comment_blocks`, `function_length`, `unsafe_scan`. Findings are items 1 and 2.
- Every caller in the tree, by `Cargo.toml` and by `use`: eight path dependencies, the standard
  library overlay through its generated copy of `src/lib.rs`, and the `clock` program in
  `components/src/clock.rs`. That program is the only process this tree maps the page read/write for.

Inferred from reading, not run: the behavior on a target without 64-bit atomics (item 8) and the behavior of `initialize` across a restart of the `clock` program (item 6). The mutation figures are also inferred. They are the baseline's (54 caught, 3 missed, 7 timeouts, 5 unviable) and the regression
note's triage of the three survivors. The review did not run the mutation census; its window is the one the
template's planner states.

Not run: the stranger test, `cargo semver-checks` (no baseline), the template's workflows end to
end, and nife's `script/crate-standalone` and `script/crate-move --dry-run`, whose results the
review package records.
