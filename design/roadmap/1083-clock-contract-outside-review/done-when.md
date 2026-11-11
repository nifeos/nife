# Done when

An appendix to [the milestone](../1083-clock-contract-outside-review.md).

- Decisions 1 and 2 have recorded rulings. `VERSION`, `INTERFACE_ID` and the magic read ratified, and
  `the_identity_is_the_one_readers_were_built_against` pins the values that ship.
- If Decision 1 changes the layout, the four loom models pass on the new shape, and each still asserts that it reached every branch it expects. `read` has no spin on another process's progress. The overlay's generated copy, `components/src/clock.rs` and `components/src/date.rs` build, and the clock and date system tests pass.
- The lane applies every name in Decision 3's table, no `#[doc(alias)]` carries an old spelling, and the
  template's `spelled_out`, `unit_symbols`, `register_names` and `british_spelling` helpers refuse
  nothing in the crate.
- `prose_acronyms`, `passive_voice` and `readme_badges` pass on the crate's README and source with
  no baseline file; `cargo clippy --all-targets -- -D warnings` passes on the crate alone, outside
  nife.
- `Cargo.toml` carries no comment that names a path outside the repository, and carries
  `repository`.
- `initialize` is idempotent, with a host test that calls it twice around a publish and reads the
  same generation after as before.
- Two Kani harnesses prove, each with a falsification patch that `proofs / falsify` replays red;
  nife's `script/crate-standalone` passes its `proofs` check on the copy.
- `.github/mutants-exempts` names every recorded survivor and timeout with its reason, and one full
  `mutation` run on `main` is green.
- The README's first paragraph says what the crate is not, and names the 64-bit atomics the crate
  needs; the crate refuses a target without them with a sentence.
- The stranger test is on record for the new repository with no open finding against the README.
