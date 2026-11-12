# What was checked, and how

An appendix to [the milestone](../1104-paging-outside-review.md).

Verified firsthand, on a fresh shallow clone at the head of `main`, with every cargo run under a
12 GiB memory cap:

- `cargo test -p paging`: 57 unit tests, 11 in `tests/blocks.rs`, 29 in `tests/mapping.rs`, 4 in
  `tests/nth_allocation.rs` and 4 documentation tests, all green.
- `cargo build -p paging --target` for `aarch64-unknown-none-softfloat`,
  `riscv64imac-unknown-none-elf` and `x86_64-unknown-none`: all build. The crate has no
  dependencies at all.
- `cargo clippy -p paging --all-targets -- -D warnings` and `cargo doc` with
  `-D warnings -D rustdoc::broken_intra_doc_links`: clean.
- `cargo kani -p paging` with Kani 0.68.0, the version the template's `.kani-version` pins:
  38 harnesses verified, 0 failures. Run time was not measured.
- `cargo llvm-cov -p paging`: 97.87% of regions, 98.14% of lines; the lowest file is `mapper.rs`
  at 91.89% of lines, above the template's 85% per-file floor.
- `cargo mutants --list -p paging`: 644 mutants. Nobody ran a census (item 9 says why).
- The template's helpers, run over the crate's sources with the template's own word list:
  `unit_symbols`, `question_shape`, `british_spelling`, `service_model` and `contract_names` judge
  1,077 names and refuse none. `file_length`, `function_length`, `comment_blocks` and
  `orphan_modules` pass. `spelled_out` refuses 84 definitions (item 5). `standalone` lists twelve
  findings (items 1 and 4). `falsify --check` refuses every one of the 38 harness records (item 3).
- The crates.io API, for the crate name (Decision 1) and the download figures in the prior-art
  table.
- The standard library's own source in the pinned toolchain: `u64::is_multiple_of`, which the crate
  calls in twelve places, is stable since Rust 1.87.0.
- The `x86_64` crate's source on docs.rs, for the `MapperFlush` comparison in the prior-art box.

Inferred from reading, not run: the overflow in `map_range` and `map_span` (item 10), the `walk`
end for a format with more than five levels (item 13), and the Miri warnings (item 19).

Not run: a mutation census (outside the 08:00 to 13:00 UTC window), the falsification replay
(form check only), Miri, an MSRV build at 1.87, `cargo semver-checks` (no previous release), a
publish dry run, and the template's `script/lint` as a whole.
