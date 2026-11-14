# Done when

An appendix to [the milestone](../1091-filesystem-fixture-outside-review.md).

- `crate-workflows`' two `deny.toml` files allow git sources from the `nifeos` organization, in a
  released template revision the new repository pins.
- `README.md` and the crate-root `BUGS` state the dependency on `filesystem_contract` and its form
  after the contract leaves. 1017's step 2 row no longer pairs the two crates. No test that
  links `byte_glob` lives in this crate.
- `prose_acronyms`, `typos`, `spelled_out`, `register_names`, `unit_symbols`, `british_spelling`
  and `question_shape` from the template, run over the crate with the arguments the `standalone`
  job passes, report zero findings.
- `helpers/lint_externalized_crates.py --count` reports 0 for `filesystem_fixture`, and the crate's
  entry leaves the lint's waiver table. No doc comment names a program, a lane, a milestone, a
  roadmap block, a ruling or a date.
- `durability` is either produced by a role that runs in the gates or is gone. The shared-frame
  witness's documentation agrees with `fs_shared_page_tests.rs` about which verdict to expect.
- A review package `notes/crate-reviews/filesystem-fixture.md` exists with a row in the index.
  `script/crate-standalone filesystem_fixture` reports the crate name and every name ratified, with
  only the path dependency left to fail.
- The scaffold's `mutants-exempts` carries the surviving equivalents with reasons, or the sites are
  gone and a census records none missed.
- The caller workflows set the three targets and `allow-unpublished`. The manifest carries a
  `repository` field and a publish comment a stranger can act on. A stranger run has a record before any tag.
- `cargo test`, `cargo clippy --all-targets`, `cargo doc` with warnings denied and the three
  bare-metal `--lib` builds are green on stable and on Rust 1.88. They run at the commit the move
  tags.
