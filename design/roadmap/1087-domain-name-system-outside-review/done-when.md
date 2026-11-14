# Done when

An appendix to [the milestone](../1087-domain-name-system-outside-review.md).

- `script/crate-standalone domain_name_system` reports zero failures with Kani included. Every public name has a ratified spelling, the manifest stands alone with `rust-version`, the README and both
  license files are present, and shipped text cites nife nowhere.
- `cargo +1.88 test` and the three bare-metal builds pass at the manifest's `rust-version`.
- The crate's own `fuzz/` directory builds. Its target runs clean for the template's budget on the
  three fixtures as seeds. `src/lib.rs` carries `#![forbid(unsafe_code)]`.
- The maintainer has decided decisions 1 to 3. The renames land across the crate and its four nife dependents. The
  template's `spelled_out`, `unit_symbols` and `register_names` helpers then report zero findings
  over `src/` and `tests/`.
- A record whose TTL has its high bit set yields `Answer::ttl()` of zero, with a test. The
  `Answer::ttl` doc cites RFC 2181 section 8.
- `.github/mutants-exempts` carries the three equivalent survivors with reasons, and
  `mutation / mutation` has one green full run on `main` before the tag.
- The README has the badge row, an example that compiles, the three harnesses by name and the Kani
  install line on one line. It shows one falsification run and carries the BUGS list, the minimum
  Rust version and the measured type sizes. `docs/stranger-test.md` records a run with no open
  finding.
- `cargo test`, `cargo kani` and the fuzz target are green at the commit the tag names.
