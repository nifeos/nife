# What was checked, and how

An appendix to [the milestone](../1109-spin-lock-outside-review.md).

Verified firsthand on a fresh shallow clone of `nifeos/nife` at the head of `main`. The checks ran
inside the workspace and in a standalone copy of the crate with a template-shaped manifest:

- Inside nife: `cargo test -p spin_lock` (4 host tests, 0 doc-tests). `cargo clippy --all-targets`
  and `cargo doc` with warnings denied. Builds for `aarch64-unknown-none-softfloat`,
  `riscv64imac-unknown-none-elf` and `x86_64-unknown-none`. The loom run (`RUSTFLAGS="--cfg loom -D
  warnings" cargo test --release -p spin_lock --lib`: 5 models, 0.02 s after the build). All clean
  on the pinned nightly.
- Standalone: `cargo test`, `cargo clippy --all-targets`, `cargo doc` and the loom run on stable.
  `cargo build` and `cargo test` on Rust 1.85, the edition's floor. Miri over the host tests. `cargo
  fmt --check`. `cargo llvm-cov` (92.66 % of lines, 94.12 % of regions, above the template's 85 %
  per-file floor). `cargo mutants --list` (28 mutants, 20 outside test modules, matching the
  recorded census of 20 tested, 5 caught, 1 missed, 14 unviable). `cargo deny check` with the
  template's `deny.toml` over the lockfile loom brings in (advisories, bans, licenses and sources
  all pass). `typos`. The template's `spelled_out.py`, `unit_symbols.py`, `abbreviations.py` and
  `rust_names.py` over `src/lib.rs` (83 names judged, 0 refused). `standalone.py`, which fails (item
  2).
- Both hand falsifications that `notes/interleaving/what-was-modeled.md` describes, replayed on
  copies. Turning the guard's release store to Relaxed fails exactly the four positive models and
  leaves the should-panic model passing. Turning the compare-exchange's success ordering to Relaxed
  does the same.
- Two experiments behind items 3 and 5. The template sample's lint table (no `cfg(loom)` in
  `check-cfg`) fails clippy with 14 `unexpected cfg` errors. Splitting `#[cfg(all(test,
  not(loom)))]` into `#[cfg(test)]` and `#[cfg(not(loom))]` drops the cargo-mutants listing from 28
  to 20.
- The crates.io API for the names in item 4 and for the prior-art crates.
- Every caller in the tree: `kernel` (`InterruptSafeMutex` in `kernel/src/sync.rs`, one static in
  `kernel/src/user/fs_service/two_dirs.rs`), `system_tests` (six statics) and
  `tools/host_reordering_litmus` (one static). All three live in nife today.

Not run: a mutation census (outside its window; the review cites the recorded figures instead), the
stranger test, `cargo semver-checks` (no baseline), the template's workflows themselves (no
repository exists), and any run on real aarch64 or riscv64 silicon.
