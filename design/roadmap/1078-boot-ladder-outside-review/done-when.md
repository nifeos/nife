# Done when

An appendix to [the milestone](../1078-boot-ladder-outside-review.md).

- `script/crate-standalone boot_ladder` reports no failure other than `proofs: no Kani harnesses`,
  or none at all with item 6's harness landed.
- The manifest states `license`, `rust-version`, `description`, `repository`, `readme` and the
  template's `[lints]` as literals, and no comment in the crate points at a file outside it.
- `README.md` and both license files exist, and the README carries `EXAMPLES` and `BUGS` and states
  the matching rule and the version policy. `CHANGELOG.md`, `_typos.toml` and a stranger run on
  record come with the move.
- No doc comment names a nife path, a milestone, a pull request number or a person.
- A test refuses a duplicated, empty or ill-formed check name and an earlier length at or above the
  current one.
- `cargo test` runs the boundary, substring, invariant and round-trip tests. The kernel prints
  through `Verdict`, and `serial_console` and `xtask` read through `ParsedVerdict`. One Kani harness
  runs green and its falsification patch turns it red.
- `git grep -n "nife capability shell" -- ':!crates/boot_ladder' ':!*tests.rs' ':!*.md'` in nife
  finds nothing, and `script/lint` fails if it ever does.
- The three provisional renames stand as built, `Stage::Prompt` and `Stage::Tour` included, until
  the architect revisits them.
