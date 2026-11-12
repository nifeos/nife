# What was checked, and how

An appendix to [the milestone](../1112-user-mode-runtime-outside-review.md).

Verified firsthand, on a fresh shallow clone of `main` on a Linux host, with the pinned nightly:

- `cargo check` and `cargo clippy -- -D warnings` for `aarch64-unknown-none-softfloat`,
  `riscv64imac-unknown-none-elf` and `x86_64-unknown-none`: clean, apart from one cargo warning on
  the aarch64 build (item 12). `cargo doc` with `-D warnings`: clean.
- `cargo check` with no `--target`, on the `x86_64` Linux host: it builds. The manifest's reason
  for `test = false` ("aarch64-only inline asm") no longer holds.
- `script/crate-standalone user_mode_runtime`: 19 failures, listed under items 3 and 8.
- `script/crate-move --dry-run` into a scratch directory, against template release v7: the
  staged tree carries zero `Name:` blocks, the three targets in its `ci.yml`, and a list of 58
  nife-side files that name the crate. It has eight findings.   First, no README (item 3). Second,
  the copy does not build because it inherits `license`   (item 3). Third, ten standalone lines, the
  first the missing license files (item 3). Fourth, 35   milestone, 32 section and 34 tree-path
  citation lines (item 8). Fifth, two lines naming the   architect, one in the crate (item 8) and
  one in the template's own pull request template. Sixth,   ten lines mentioning nife, all in
  scaffold files the template supplies (`.githooks/commit-msg`,   `deny.toml`,
  `docs/stranger-test.md`, `rustfmt.toml`). The last two are
  the template's to fix. The run also notes that the crate has no row in the mutation baseline,
  and that template v7 does not yet run the spelled-out-word and unit-symbol gates, so those arrive
  with a later template release.
- The template's helpers over the crate's ten source files: `abbreviations.py` reports 53
  components that are not words in 452 names (item 13); `prose_acronyms.py` reports 24 acronyms
  unexpanded at first use (item 9); `british_spelling.py`, `comment_blocks.py` and
  `function_length.py` pass.
- nife's externalized-crates lint: five hits against a waiver ceiling of five, so the lint itself
  is at its limit and no new mention of nife may land.
- Every `unsafe` block (133 occurrences of the word across the sources), which I read against its
  `SAFETY` comment. Two findings (items 1 and 2).
- The Rust reference's wording for the `nomem` option, which I fetched and quote in item 2.
- Download figures for the prior-art crates, from the crates.io API.

Absent, so not checked: a review package, a Kani harness, a recorded mutation result (the
mutation-testing notes record that the host suite excludes the crate), a README, a changelog, a host
test.

Inferred from reading, not run:

- The exact way `check / coverage` and `check / mutation-diff` fail on a crate with no test
  target (item 4).
- Whether firmware exists that leaves `CNTFRQ_EL0` at zero (item 6).
- Whether every kernel object type refuses an unknown method with `BadMethod` rather than another
  error (item 14).
