# What was checked, and how

An appendix to [the milestone](../1079-byte-sink-contract-outside-review.md).

Verified firsthand, on a fresh shallow clone of `nifeos/nife` at the head of `main` and a fresh
clone of `nifeos/crate-workflows`:

- The whole crate (467 lines of `src/lib.rs`, 26 of `Cargo.toml`) and every caller. The callers are thirteen programs in `components/`, `crates/system_log`, `crates/primitive_benchmark_contract`, `crates/system_initializer`, `fixtures/`, `system_tests/`, `kernel/` (behind `bench`), the fuzz workspace (`system_log_session` only), and the std PAL copy. `cargo xtask std-src` generates that copy (`sys/pal/nife/sinkproto.rs`) from this file with its test module and `# Examples` stripped.
- `cargo test -p byte_sink_contract` (11 unit tests, 2 documentation tests), `cargo clippy
  --all-targets -D warnings`, `cargo doc` with broken links and invalid HTML denied, and `cargo build
  --lib` for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and
  `x86_64-unknown-none`: all clean on the tree's pinned nightly.
- A standalone copy with a literal manifest (`rust-version = "1.85"`, the template's three lints,
  no dev-dependency, the `abi` pin test removed) builds on stable 1.99.1 and passes its 10 remaining unit tests and 2 documentation tests.
- `cargo mutants --list` counts 79 mutants. That matches the census row (79 tested, 73 caught, 4 unviable, 2 missed, 0 timed out) and the survivor classification (both missed are equivalent).
- The template's gates over the standalone copy: `spelled_out`, `unit_symbols`, `register_names`,
  `question_shape`, `service_model` judge 91 names and refuse none. `british_spelling` refuses one (item 7). `abbreviations` (report only) lists nine components. `standalone` reports no README, no license files, three paths into this tree, five section citations, four provenance blocks, three lines naming the architect and three milestone citations.
- `helpers/lint_externalized_crates.py --count`: 2 hits, at the crate's waiver ceiling.
- Five candidate Kani harnesses (item 5), written on the standalone copy and run one at a time with Kani 0.68.0. Four verify in 3 to 4 seconds each: `unpack` never returns a count above sixteen; `classify` is `Gone` exactly for `GONE`; a set severity reads back and disturbs nothing; `eof()` is never a bytes message. The round trip verifies in 12 seconds when the length is a concrete loop variable under `#[kani::unwind(18)]` with the sixteen bytes symbolic. The same property with a symbolic length ran 35 minutes at 8 GB. It did not finish under a seven-minute cap on a second attempt, so item 5 prescribes the concrete-length form.

Inferred from reading, not run: the behavior on a target whose `usize` is narrower than 32 bits
(item 11). Also inferred: the download figures and register counts in the prior-art box, marked unverified there.

Not run: a mutation census (outside its window; the census rows above stand in), the fuzz target that links this crate (`system_log_session` fuzzes `system_log`, not this decoder), Miri, the QEMU-side `sink_tests` and `script/names`.
