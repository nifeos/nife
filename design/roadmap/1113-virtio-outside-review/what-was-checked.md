# What was checked, and how

An appendix to [the milestone](../1113-virtio-outside-review.md).

Verified firsthand, on a fresh shallow clone of `nifeos/nife` at `cf1328af6` and a clone of
`nifeos/crate-workflows`, on the host named in the review's final message:

- `cargo build -p virtio`, `cargo clippy -p virtio --all-targets`, `cargo test -p virtio` and
  `cargo doc -p virtio --no-deps` with warnings denied: all clean on the workspace toolchain
  (cargo 1.102.0-nightly). The crate also builds for `aarch64-unknown-none-softfloat`.
- `cargo test -p virtio`: zero unit tests, one documentation test (`no_run`, so it compiles and
  never runs). No Kani harness and no falsification patch exist (`script/crate-review-packet`).
- `script/crate-standalone --no-kani virtio`: 16 failures. Four path dependencies leave the   crate
  (`address_space_map`, `nifefs`, `filesystem_contract`, `user_mode_runtime`). The crate   inherits
  `license` and `lints` from the workspace. It has no `rust-version`, no README, no   `LICENSE-MIT`
  and no `LICENSE-APACHE`. The shipped text holds 9 milestone citations, 17 paths   into this tree,
  3 section citations and one architect's name. The build step skipped because a   dependency sits
  outside the copy.
- `script/names virtio` and `helpers/name_release.py --draft virtio`: the architect ratified the
  crate name,   and the seven public functions carry no `Name:` block.
- Every `unsafe` block (two), every arithmetic expression and every loop in `src/lib.rs`, by
  reading, against virtio 1.2 as remembered. The review did not open the specification to confirm
  the section numbers   below, and it marks them unverified.
- Every caller: `components/Cargo.toml` is the only manifest that depends on the crate, and
  `components/src/block_driver.rs` is the only file that calls it.
- The five other virtio drivers in the tree (`components/src/virtio_net_transport.rs`,
  `keyboard_driver.rs`, `gpu_driver.rs`, `entropy.rs`, and `kernel/src/virtio.rs`), for what they
  share with this crate: a `grep` for the magic value and for `S_FEATURES_OK` finds the same
  handshake constants in all six places.
- Download figures for the prior-art crates, from the crates.io API at the time of the review.

- Not run: a mutation census, which sits outside its window. `notes/mutation-testing.md` records
  that the crate compiles for the host but cannot run a line without a kernel, so a census would
  measure nothing.
- Not run: the QEMU suite. `system_tests/src/user/block_driver_tests.rs` and
  `riscv_virtio_tests.rs` exercise the roles. The review read the entry points and did not boot
  them.
- Not run: the template's workflows, because no repository exists.
- Not run: the publish rehearsal, because the crate carries `publish = false`.

Inferred from reading, not run: items 6, 9 and 14.
