# What was checked, and how

An appendix to [the milestone](../1102-network-time-protocol-outside-review.md).

Verified firsthand, on a fresh shallow clone of `nifeos/nife` at the head of `main`
(`59b97d5ccc3f29b2e4004b951cca2b43eabe9e95`) and a clone of `nifeos/crate-workflows`:

- `cargo test -p network_time_protocol`: 25 unit tests and 2 documentation tests pass, in under
  2 s with the workspace's `opt-level = 2` override for the crate. `cargo clippy --all-targets
  -D warnings` and `cargo doc` with warnings and broken intra-doc links denied are clean, on the
  workspace's nightly toolchain.
- The crate builds for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and
  `x86_64-unknown-none`. It is `#![no_std]` with no `alloc`.
- Kani 0.68.0 verifies all seven harnesses in 40 s. The bounded nanosecond harness takes 25 s of
  that. Each of the seven falsification patches applies cleanly, and each, applied alone and run
  against its own harness with `--exact`, ends `VERIFICATION:- FAILED`. Each replay restored the
  source afterward.
- The template's checks, run directly from `crate-workflows/helpers` over `Cargo.toml`, `src/lib.rs`
  and `src/verification.rs`: `contract_names`, `british_spelling`, `register_names` and
  `prose_acronyms` each report findings at a ceiling of 0 (items 3 to 6). `spelled_out`,
  `unit_symbols`, `question_shape`, `unsafe_scan`, `comment_blocks`, `file_length` and
  `function_length` pass. `abbreviations` (report only) lists 18 components. `standalone` lists
  twelve findings (items 1, 2 and 8).
- nife's own `script/crate-review-packet --no-rustdoc`, `script/names` and
  `helpers/name_release.py --draft` over the crate.
- The text of RFC 5905, for the reference implementation's `packet()` checks, the global parameter
  table and the spelling of leap indicator 3.
- The crates.io API, for the download figures in the prior-art table.
- The recorded mutation figures in `notes/project-metrics/mutation-census.csv` (latest row: 185
  mutants tested, 166 caught, 5 missed, 14 unviable) and the triage of the five survivors in
  `notes/project-metrics/mutation-triage.csv`.

Inferred from reading, not run: the overflow in item 12, the behavior of the unoptimized exhaustive
sweep under the template's jobs (item 10), and whether the move's provenance strip reaches a `//!`
block (item 8).

Not run: a mutation census (outside its window), coverage, Miri, the stranger test,
`cargo-semver-checks` (no previous release), the MSRV build, `script/crate-standalone`, a fuzz
campaign, and the template's workflows themselves (no repository exists yet).
