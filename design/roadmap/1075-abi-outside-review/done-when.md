# Done when

An appendix to [the milestone](../1075-abi-outside-review.md).

- `cargo clippy --all-targets -- -D warnings` and `cargo test` are green on stable and at 1.88 in
  the staged repository, with no `clippy.toml`.
- `helpers/lint_externalized_crates.py` counts zero hits for the crate, and the dry run's citation
  and nife-word checks pass.
- No doc comment in `src/` names a path, module, crate, program, rule number or claim number of
  another repository; the crate's docs build with `-D warnings` and every link resolves.
- `src/virtio.rs` names only errors `Error` has; the crate root carries the six-word register table
  for all three architectures; the README's BUGS section lists what the modules list.
- No dated phrase remains in `src/` outside a `Name:` block; `prose caps`, `passive voice` and
  `readme_badges` pass on the README.
- A test fails when a `pub const` number of the crate is not pinned; the new repository carries the
  rule that a pinned line changes only with `ABI_VERSION`.
- `futex::is_admitted` and `TryFrom<i64> for Error` exist; `admitted` and `from_ret` do not.
- A review package for `abi` exists under `notes/crate-reviews/` with a row in the index, every public
  item carries a `Name:` block, and the architect has ruled A and B.
- The first weekly mutation run in the new repository has one `mutants-exempts` line per survivor
  or a test that kills it.
