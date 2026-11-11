# What was checked, and how

An appendix to [the milestone](../1078-boot-ladder-outside-review.md).

Verified firsthand on a fresh clone of `nifeos/nife` at its head, in a scratch directory, with every
cargo run under a 12 GiB memory cap:

- `cargo build`, `cargo test` (0 unit tests, 0 documentation tests), `cargo clippy --all-targets
  -D warnings` and `cargo doc` with warnings denied: all clean on the pinned nightly.
- The crate builds for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and
  `x86_64-unknown-none`.
- `script/crate-review-packet boot_ladder`: 10 public constants, 0 `unsafe` sites, 0 dependencies,
  0 Kani harnesses, 0 identity values, 4 dependents in nife (`components`, `serial_console`,
  `kernel`, `xtask`).
- `script/crate-standalone boot_ladder`: 11 failures, listed under P0 below; the build step passed
  for the host and the three targets on stable.
- Every call site in the tree (`grep -rn boot_ladder:: --include=*.rs`), the kernel's verdict
  printer, `serial_console`'s matchers and `xtask`'s, read in full.
- The crate-workflows template's README, scaffold, `coverage-floor.sh` and `falsify.py`, to see
  what its jobs do with a crate that has no tests and no harnesses.

Not run: a mutation census (outside its window; the crate has no functions to mutate), `cargo semver-checks` (no release to compare against), a stranger test (no README exists to test) and the template's workflows (no repository exists). Prior-art facts below that this review recalled rather than read say so.
