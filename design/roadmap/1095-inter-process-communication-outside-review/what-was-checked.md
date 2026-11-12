# What was checked, and how

An appendix to [the milestone](../1095-inter-process-communication-outside-review.md).

Verified firsthand, on a fresh shallow clone of `nifeos/nife` at the head of `main`, on a Linux
host with stable Rust 1.99.1 and Kani 0.68.0:

- `cargo test -p inter_process_communication`: 29 host tests and 5 documentation tests pass.
  `cargo clippy --all-targets -- -D warnings` and `cargo doc` with warnings denied are clean.
- `cargo build --lib` for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and
  `x86_64-unknown-none` succeeds, so the `no_std` claim holds on the three targets nife names.
- `cargo kani` verifies all 15 harnesses (8 rendezvous, 4 notification, 3 timer) in 26 seconds
  with a peak of about 220 MB. Each of the 14 falsification patches applies cleanly
  (`git apply --check`). The patches were not replayed under Kani; the template's `falsify` job
  does that.
- One extra falsification, applied by hand and reverted: replacing the rendezvous's saturating
  pending-count increment by a plain `+ 1` turns `signal_preserves_the_invariant` red on Kani's
  overflow check. Item 8 is about that.
- `cargo llvm-cov`: 97.76 percent of lines, 98.46 percent of regions, 2 of 86 functions never
  executed. The template's floor is 85.
- `cargo mutants --list`: 178 mutants (68 in `rendezvous.rs`, 52 in `notification.rs`, 33 in
  `futex.rs`, 25 in `timer.rs`). This review ran no census; item 9 records what the tree's own census
  says.
- The template's name and standalone gates ran over the crate's sources with the template's own
  word lists. The findings: `spelled_out` 2, `question_shape` 1, `register_names` 2,
  `british_spelling` 1, `unit_symbols`, `service_model` and `contract_names` 0, `abbreviations`
  (report only) 0, `standalone` 13 findings, `typos` (en-us) 8 findings. Items 3, 5 and 7 carry
  them.
- Every caller in the tree (`grep -rn inter_process_communication --include=Cargo.toml`): the
  kernel is the only one. The review read the kernel's wrappers (`kernel/src/sched/rendezvous.rs`,
  `notification.rs`, `timer.rs`, `futex.rs`, `interrupt_route.rs`) to judge whether the
  crate's surface fits its one user.
- The crate name and its dependency's name on crates.io, through the crates.io API: neither
  `inter_process_communication` nor `intrusive_queue` exists there. Download figures for the
  prior-art crates in the box below come from the same API.

Inferred from reading, not run: the size figures in item 11 (counted from field types, not
measured with `size_of`), and the reachability of the kernel-side observation reported to the
maintainer separately.

Not run: a mutation census (outside its window), `cargo semver-checks` (no previous release),
Miri, a fuzz target (the crate has none), `cargo deny` (no `deny.toml` yet), a release dry run,
and the template's `script/lint` as a whole.
