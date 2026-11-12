# Done when

An appendix to [the milestone](../1092-glob-outside-review.md).

- The crate has the registry name Decision 1 rules, applied in `Cargo.toml`, the three dependents
  and the docs, with no alias for the old spelling.
- The seven public names have ratification in a review package `notes/crate-reviews/<name>.md` with a row
  in `notes/crate-reviews.md`, under Decision 2. `name_release.py --draft` reports no "no block".
- `tests/fixture_agreement.rs` lives in a crate that stays. `glob` is a dev-dependency there. The
  matcher's manifest has no dev-dependency.
- `python3 helpers/standalone.py --manifest-dir crates/<name>` from the template exits 0 with no
  `--allow-unpublished`, except for the `publish = false` line. The move's caller covers that line
  with `allow-unpublished: true` as the other crates do.
- The README passes `readme_sections.py` with no finding and states the three surprises in its first
  paragraph. It carries the corners table and the `**`, qualifiers and POSIX-classes refusals. It
  cites the algorithm's source.
- `question_shape` refuses nothing. `src/lib.rs`, `src/tests.rs` and `src/verification.rs` exist.
  The repository's `.cargo/mutants.toml` excludes the proof file.
- The six patches diff `a/src/lib.rs`. The six records name `falsifications/...`. `falsify.py`
  (without `--check`) reports six failures of six, each at its own assertion.
- `#![forbid(unsafe_code)]` is present. `cargo clippy --all-targets -- -D warnings`, `cargo doc`
  with broken links denied, `cargo test`, the three bare-metal builds and `cargo kani` pass on the
  move commit. The `check` and `proofs` workflows are green there.
- The module doc and `matches` state that `/` is a byte.
- The P2 items become issues in the new repository, or carry a recorded refusal with a reason.
