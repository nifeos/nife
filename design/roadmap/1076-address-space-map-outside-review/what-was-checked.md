# What was checked, and how

An appendix to [the milestone](../1076-address-space-map-outside-review.md).

Verified firsthand, on a fresh shallow clone of `nifeos/nife` at `137639a3` and a copy of the
crate standing alone with a literal manifest (`rust-version = "1.87"`, `license = "MIT OR
Apache-2.0"`, the three template lints inline):

- In the tree, with the pinned nightly: `cargo test -p address_space_map`, 6 unit tests and 1
  documentation test, all green.
- Standing alone, the test target does not compile: `include_str!("../../user_mode_runtime/link.ld")`
  has nothing to read. With that one test removed, on stable 1.99.1: 5 unit tests and 1
  documentation test green, `cargo clippy --all-targets -D warnings` clean on the host and on
  `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and `x86_64-unknown-none`,
  `cargo doc` clean with `-D rustdoc::broken_intra_doc_links -D rustdoc::invalid_html_tags`.
- Rust 1.88 builds and tests the copy. Rust 1.85 refuses it: `u64::is_multiple_of` is unstable
  there (`unsigned_is_multiple_of`). That it stabilized in 1.87 is from memory and unverified; no
  1.86 or 1.87 toolchain was on the machine.
- Kani 0.68.0, the version the template pins: 9 harnesses, 9 verified, 0 failures. The run prints
  five future-incompatibility warnings from the crate's own source (item 10).
- The template's helpers over the copy: `standalone.py` refuses it on nine counts (items 3, 4, 5). `falsify.py --check` refuses all nine harness records (item 2). `spelled_out.py` refuses 9 names (item 6). `question_shape.py` refuses 1 (item 6). `register_names.py` refuses 5 harness names, every one a false positive (item 13). `unit_symbols.py`, `british_spelling.py`, `unsafe_scan.py`,
  `comment_blocks.py`, `file_length.py`, `function_length.py` and `orphan_modules.py` pass.
- The recorded mutation census (`notes/project-metrics/mutation-census.csv`, its newest row and the
  weekly series): 45 mutants, 23 caught, 1 missed, 21 unviable. The one survivor is `check_image`'s
  `>` to `>=`, triaged equivalent; reading `check_image` confirms it (item 11). This review ran no census.
- Every caller in nife: 26 manifests depend on the crate by path, and the tree names its items at
  281 sites. The most-used items are `pair_page` (196), `IMAGE_BASE` (42), `PAGE` (23),
  `STACK_TOP_PAGE` (20), `service_window` (15), `VERSION` and `INTERFACE_ID` (15 each),
  `runtime_window` (11). Nothing outside the crate reads `BANDS`.
- FreeBSD's `sys/amd64/include/vmparam.h` and Linux's `arch/x86/include/asm/elf.h` and
  `page_64_types.h` at their current heads, for the prior-art box. Download figures from the
  crates.io API.

Inferred from reading, not run: the behavior of the template's `proofs / falsify` job on the rewritten patches (item 2). Also inferred: whether `cargo xtask std-src` resolves a git dependency when it generates `memory_region_heap`'s core into std (the move section).

Not run: a mutation census (outside its window), `cargo semver-checks` (no release to compare),
Miri (the crate has no `unsafe`), the template's prose and README gates (there is no README yet).
