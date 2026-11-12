# What was checked, and how

An appendix to [the milestone](../1106-program-argument-contract-outside-review.md).

Verified firsthand, on a build host, from the clone:

- `cargo test` on stable 1.99.1: 7 unit tests and 1 documentation test pass. `cargo clippy
  --all-targets -- -D warnings` and `cargo doc` with `-D warnings -D
  rustdoc::broken_intra_doc_links` are clean. The library builds for
  `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and `x86_64-unknown-none`. All of
  this ran on a copy of the crate under a manifest with literal `license` and `[lints]` tables. The
  crate's own manifest inherits both from nife's workspace, and the workspace pins a nightly this
  review did not install.
- The template's helpers over the crate. `spelled_out.py` reports 7 names over its ceiling of 0.
  `unit_symbols.py`, `register_names.py`, `question_shape.py`, `british_spelling.py` and
  `service_model.py` report 0 of 86 names. `unsafe_scan.py` finds the two `unsafe` sites, so the
  weekly fuzz opt-in stays refused, as the template intends for a crate with `unsafe`.
  `standalone.py` reports 12 findings and "this crate does not stand alone yet".
- `cargo mutants --list` names 71 mutants, the same count the weekly census records (71 tested, 55
  caught, 1 missed, 15 unviable, 0 real holes). This review read the census and did not run it.
- Kani 0.68.0 over two probe harnesses written for this review. A parser harness over a 40-byte
  symbolic page (no panic, and the iterator yields exactly `len()` items) verifies in 37 seconds
  under a 12 GiB cap. A builder-to-parser round trip over the fixed 4,096-byte page exhausts the
  same cap in CBMC before a verdict.
- Every caller in the tree: `grant_plan` (the only crate that links it), `components` (ten programs
  read `INTERFACE_ID` and `VERSION`), the `std` overlay's `sys/args/nife.rs` (the only reader of
  `from_va`), and `xtask`. The `xtask` copies `src/lib.rs` verbatim into the `std` port.

Inferred from reading, not run: the per-file coverage against the template's floor of 85 (item 8).
Also inferred: the behavior on a target whose `usize` is narrower than 64 bits (item 10), that a
builder over a shorter buffer makes the round-trip proof fit (item 12), and that the crate builds at
Rust 1.88 (item 1).

Not run: the farm's `std` generation (it needs nife's pinned nightly), cargo-llvm-cov, Miri,
cargo-semver-checks (no release to compare against), a mutation census, and any fuzzing.
