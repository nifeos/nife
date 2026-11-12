# Done when

An appendix to [the milestone](../1076-address-space-map-outside-review.md).

- The crate builds, tests, lints and documents standing alone with no reference into nife. The linker-script test lives in `user_mode_runtime` and is green there.
- The template's `proofs / falsify` replays all nine patches red in the crate's own repository. `proofs / kani` prints no warning that names a line of the crate.
- `check / standalone` is green: literal manifest with `rust-version`, README with the scaffold's
  sections, both license files, no path, section, milestone or person's name in the docs, every
  spelled-out word spelled out.
- The crate root doc holds the band table and placement order. A recorded stranger run against the README has no open finding.
- The architect rules the three decisions. Whichever renames the rulings call for land in the move pull request, with
  `script/names` enumerating the sites before the sweep.
- nife pins the crate by `rev` in every manifest in one pull request, with a `[workspace.dependencies]` entry. `cargo xtask std-src` still generates `memory_region_heap`'s core.
- `.github/mutants-exempts` carries the one equivalent mutant. The first full `mutation` run on
  `main` is green.
- `crates/paging` asserts its split shifts against the crate's. The declarations check reads the
  pinned checkout for interface id 2.
