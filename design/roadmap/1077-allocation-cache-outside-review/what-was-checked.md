# What was checked, and how

An appendix to [the milestone](../1077-allocation-cache-outside-review.md).

Verified firsthand, on a fresh shallow clone of `main` and on a standalone copy of the crate
outside the workspace:

- `cargo test -p allocation_cache` (6 unit tests, 1 documentation test) in debug and release. `cargo clippy --all-targets -D warnings`, and `cargo doc --no-deps` with broken intra-doc links and invalid HTML tags denied: all clean on stable.
- The crate builds for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and
  `x86_64-unknown-none`. It also builds on Rust 1.88, the `rust-version` every sibling crate states and this one does not.
- Miri over all six unit tests, with default flags and with `-Zmiri-strict-provenance`, against
  nife's patched `vendor/talc`: clean. Then against the unpatched `talc` 5.1.1 that crates.io serves. The crate's own repository will build that version, since nife's `[patch.crates-io]` does not follow it. It is clean too, under default flags, strict provenance and three Miri seeds.
- The template's `check / standalone` helpers, run from the template at its head over the crate: `standalone.py` lists ten findings (item 1). `spelled_out`, `unit_symbols`, `register_names`, `british_spelling`, `service_model`, `contract_names`, `question_shape`, `prose_acronyms`, `prose_figures`, `file_length`, `comment_blocks` and `function_length` report zero. `abbreviations` (report only) flags one test-name component, `uncached`.
- `script/crate-standalone --no-kani`: ten failures, all in the name, manifest, files and citation steps. The build step passes for the host and the three bare-metal targets.
- `cargo mutants --list -p allocation_cache`: 55 mutants. The last two recorded censuses in `notes/project-metrics/mutation-census.csv` show the same count (55 mutants, 45 caught, 0 missed, 10 unviable). The review ran no census.
- The release-mode behavior of `class` on a size no `Layout` can carry (item 4), and that an
  all-zero `Cache` is empty (item 5), each by a test added to the standalone copy.
- `cargo package --list` on the standalone copy ships `Cargo.toml`, `Cargo.lock` and `src/lib.rs`,
  and warns that the manifest names no `documentation`, `homepage` or `repository`.
- `size_of::<Cache>()` is 144 bytes, aligned to 8. `MOST_CACHED_BYTES` is 34,816, as the crate documentation says.
- The upstream state of the talc fix nife carries: pull request 57 on the talc repository is open and unmerged. crates.io's newest talc is 5.1.1.

Inferred from reading, not run: the behavior of `class_bytes` on an index past `CLASSES` (item 4). Also inferred: whether the template's `proofs` job passes a crate with no harness (item 11).

Not run: Kani (the crate has no harness), a mutation census, a fuzz campaign, the stranger test, the release dry run and the `std` farm (`cargo xtask std-src`) after a simulated move.
