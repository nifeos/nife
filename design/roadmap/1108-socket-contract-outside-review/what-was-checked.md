# What was checked, and how

An appendix to [the milestone](../1108-socket-contract-outside-review.md).

Verified firsthand, on a fresh shallow clone at the head of `main`
(`cf1328af62c0f35aab9900cc385c94a25a12a587`), with the workspace toolchain (a nightly, 1.102):

- `cargo test -p socket_contract`: 20 unit tests and 5 documentation tests pass, the README's
  example among them.
- `cargo clippy -p socket_contract --all-targets -- -D warnings` and `cargo doc` with warnings
  denied: clean.
- The crate builds for `x86_64-unknown-none`, `aarch64-unknown-none-softfloat` and
  `riscv64imac-unknown-none-elf`.
- Kani 0.68.0 (the template's `.kani-version`) verifies all three harnesses. The run prints one
  future-incompatibility warning from the `const _: () = assert!(...)` in `ephemeral`, which the
  package already notes.
- Every caller in the tree, by `grep` over `Cargo.toml` files and `socket_contract::` paths. It
  found eight path dependencies, the std platform layer's copy made by `xtask/src/farm/generate.rs`,
  and the network stack's three users of `ephemeral` and `polled_wait`.
- The network stack's dispatch and operations (`components/src/socket_server.rs`,
  `socket_operations.rs`, `socket_table.rs`), the client runtime's frame accessors
  (`crates/user_mode_runtime/src/socket_frame.rs`), the std platform layer's socket code
  (`patches/std-nife/overlay/std/src/sys/net/connection/nife.rs`) and the test client
  (`components/src/socket_test_client.rs`). The comparison is what the contract says against what
  both sides do.
- The mutation census rows for the crate in `notes/project-metrics/mutation-census.csv` and the
  triage of its survivors.
- The template's README, its sample manifest and its crate README scaffold, for what the move
  will expect of the manifest and the README.
- Download figures for the prior-art crates, from the crates.io API.

Inferred from reading, not run: the build at Rust 1.88, and the manifest's `rust-version` (the
review package ran it). Also inferred is every statement below about how Linux, the BSDs and Fuchsia
behave. Those statements are from memory and marked unverified where they carry a number.

Not run: `script/crate-standalone` and `script/crate-move --dry-run` (the package ran both), a
mutation census (outside its window), `cargo semver-checks` (no previous release), the stranger test
and the template's workflows (they run in the new repository).
