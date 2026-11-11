# Done when

An appendix to [the milestone](../1078-boot-ladder-outside-review.md).

- `script/crate-standalone boot_ladder` reports no failure other than `proofs: no Kani harnesses`,
  or none at all once item 6's harness lands.
- The manifest states `license`, `rust-version`, `description`, `repository`, `readme` and the
  template's `[lints]` as literals, and no comment in the crate points at a file outside it.
- `README.md`, `CHANGELOG.md`, both license files and `_typos.toml` exist; the README carries
  `EXAMPLES` and `BUGS`, states the matching rule and the version policy, and a stranger run is on
  record.
- No doc comment names a nife path, a milestone, a pull request number or a person.
- A `const` assertion refuses a duplicated, empty or ill-formed check name and an earlier length
  at or above the current one, with a test that shows each refusal.
- `cargo test` runs at least the boundary, substring and invariant tests; when `Verdict` lands, the kernel prints through it, and `serial_console` and `xtask` parse through it. Then a round-trip test passes, and one Kani harness with its falsification patch runs green and red respectively.
- `git grep -n "nife capability shell" -- ':!crates/boot_ladder' ':!*tests.rs' ':!*.md'` in nife
  finds nothing, and `script/lint` fails if it ever does.
- The architect records the three rulings above, and the one rename lane applies them, `Stage::Prompt`
  and `Stage::Tour` included.
