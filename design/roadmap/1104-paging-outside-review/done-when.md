# Done when

An appendix to [the milestone](../1104-paging-outside-review.md).

- `Cargo.toml` carries `license`, `rust-version`, `description`, `repository`, `readme`, `keywords`,
  `categories` and its own `[lints]` table. `publish = false` is gone in the release that publishes.
  `standalone.py` reports zero findings.
- The crate and its repository carry the name Decision 1 rules, and the kernel's and
  `system_tests`' manifests and imports follow it.
- `falsify.py --check` passes for all 38 records, and one replay run shows every patch failing
  exactly its own harness.
- `spelled_out.py` reports zero findings. Every kept specification name opens its doc with the full
  name and source and carries `#[doc(alias)]`. `PageSize`'s variants carry the spelling Decision 2
  rules.
- `Mapper::new`'s `# Safety` states the allocator's obligations, and both allocation sites check
  alignment.
- One `Flags::ALL` of ten feeds every harness and host test, with a test on its length; Kani
  verifies 38 harnesses still, each over ten constructors.
- `.github/mutants-exempts` carries the ported exemptions with reasons, and one full
  `mutation.yml` run on `main` is green inside the window.
- `map_range` and `map_span` return `MapError::BadRegion` instead of overflowing, with a test.
- The README has `## EXAMPLES` with a visible `Mapper` example, `## BUGS` naming the limits in
  items 12, 15 and 18, the formats table, the build, prove and falsify commands with the measured
  Kani time, and a stranger run is on record.
- `cargo test`, `cargo kani`, `cargo clippy -- -D warnings`, `cargo doc` with warnings denied, the
  three bare-metal builds and `cargo +1.87 test` are green at the commit the move tags.
