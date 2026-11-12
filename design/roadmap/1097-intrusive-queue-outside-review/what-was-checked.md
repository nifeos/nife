# What was checked, and how

An appendix to [the milestone](../1097-intrusive-queue-outside-review.md).

Verified firsthand, on a fresh shallow clone of `nifeos/nife` at the head of `main`, with the tree's
pinned nightly unless the line names a toolchain:

- `cargo test -p intrusive_queue`: 5 unit tests, 2 documentation tests and 1 compile-fail test pass.
  `cargo clippy --all-targets -- -D warnings` and `cargo doc` with warnings and broken links denied
  are clean. The crate builds for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf`
  and `x86_64-unknown-none`.
- The crate copied out of the workspace with the template's `[lints]` block passes `cargo test` and
  `cargo clippy --all-targets -- -D warnings` on Rust 1.85, 1.88 and current stable. Edition 2024
  needs 1.85, so that is the lowest `rust-version` the manifest can state.
- Kani 0.68.0 verifies the one harness in 16 seconds at 197 MB peak. The falsification patch
  applies, fails exactly the assertion its header names (`q.is_empty() == (m_head == m_tail)` on
  push, pop, push), and reverts cleanly.
- Miri with strict provenance passes the crate's own tests under Stacked Borrows. Two further Miri
  experiments, written for this review and not in the tree, are the evidence for item 5 below.
- cargo-llvm-cov: 97.5 percent of lines, 25 of 26 functions (the uncovered one is
  `Default::default`), well above the template's 85 percent floor.
- `cargo mutants --list`: 19 mutants. This review ran no census (the window rule). The recorded baseline
  says asserting `len` and `is_empty` mid-drain fixed three earlier survivors.
- The template's name checks over the crate (`spelled_out`, `unit_symbols`, `question_shape`,
  `british_spelling`, `service_model`): 69 names judged, none refused. Function length, comment
  blocks and file length are within the template's caps.
- The template's `standalone` check over the crate as it stands: refused, for the reasons items 1
  to 3 list. Its `unsafe_scan` refuses a fuzz opt-in, correctly: the crate has `unsafe` in shipped
  source.
- `cargo check` of both consumers (`inter_process_communication` with all targets, `kernel` for
  `x86_64-unknown-none`) with a `T: Send` bound added to the queue's `Send` impl, for item 4.
- `cargo package --list`: the crate would ship `src/lib.rs`, the manifest, the lockfile and the
  falsification patch, and Cargo warns that the manifest has no description, documentation,
  homepage or repository.
- Download figures and the method list of `intrusive-collections` from the crates.io and docs.rs
  pages. The `cordyceps` documentation pages did not resolve from here; what the text says of its API
  below comes from memory and carries an unverified mark.

Inferred from reading, not run: the behavior of the kernel's `mint_token` under Miri (item 5 shows
the same shape in isolation; the kernel itself was not run under Miri), and the mutation outcome
of the 19 listed mutants.
