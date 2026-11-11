# What was checked, and how

An appendix to [the milestone](../1074-designware-ethernet-outside-review.md).

Verified firsthand, on a fresh clone at the head of `main`, on the tree's pinned nightly toolchain:

- `cargo test -p designware_ethernet`: 87 unit tests and 1 documentation test pass. `cargo clippy
  --all-targets -- -D warnings` is clean. `cargo doc --no-deps` with
  `-D rustdoc::broken_intra_doc_links -D rustdoc::invalid_html_tags -D warnings` is clean.
- The crate builds for `riscv64imac-unknown-none-elf`, so `no_std` holds. The template's
  `unsafe_scan.py` finds no `unsafe` outside tests; there is none anywhere in the crate.
- The README's example compiles and runs when copied into an example target by hand. `cargo test`
  does not compile it (item 7).
- All six falsification patches apply today, with `patch -p3` from the crate root as the README
  says and with `patch -p1` from the tree root. Kani itself was not run here.
- Every helper the template's `check / standalone` job runs, at the template's head and again at
  nife's vendored pin (the two agree):
  - `spelled_out` gives 2 findings and `register_names` 91.
  - `prose_acronyms` gives 26.
  - `passive_voice` gives 3 in the README against an allowance of 1.
  - `prose_caps` finds one 53-word sentence against a cap of 40.
  - `typos_gate` refuses the crate for lacking `_typos.toml`.
  - `unit_symbols`, `question_shape`, `british_spelling`, `service_model`, `contract_names`,
    `prose_figures`, `history_phrases`, `orphan_modules`, `check_links`, `readme_sections`,
    `readme_acronyms` and the three length ratchets find nothing.
  - `abbreviations` reports 27 components (report only).
  - `standalone.py` reports `publish = false`, 178 provenance lines, one section citation, one
    milestone citation and one architect's name, all in text the move deletes.
  nife holds the first three counts as report baselines in `helpers/template_report_baseline.txt`.
- `cargo package --list` ships `src/`, `falsifications/`, the manifest, the README and both license
  files, and warns that the manifest has no `repository`, `documentation` or `homepage`.
- Every item the kernel, `components` and `machine_discovery` reach in the crate, by grep, to judge
  what the public surface must keep.
- Download figures for the prior-art crates, from the crates.io API.

Read, not run: the recorded mutation census and its triage (`notes/project-metrics/mutation-census.csv`,
`mutation-triage.csv`). The latest census row tested 638 mutants, caught 597, missed 12, timed out
14 and found 15 unviable. The triage holds 105 rows for the crate: 87 killed, 6 equivalent with an
argument each, and 12 `gap` rows, every one a loop that never ends under the mutant (item 3).
Nobody reran the mutation census; it belongs in its window.

Not run: Kani, cargo-mutants, Miri, a build at `rust-version` 1.88 (the tree pins a nightly), the
bare-metal builds for aarch64 and x86_64, a release dry run, and the hardware itself. Every claim
about what the controller or the PHY does on silicon rests on the sources the crate cites and on
the bench record in `notes/designware-ethernet.md`, not on a measurement made for this review.
