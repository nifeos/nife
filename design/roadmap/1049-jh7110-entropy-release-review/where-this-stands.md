# Where this stands

An appendix to [the milestone](../1049-jh7110-entropy-release-review.md).

## Finished

- Items 6 to 10 and 13: `nifeos/jh7110_entropy#19`.
- Items 11, 12 and 14: `nifeos/jh7110_entropy#20`.
- Items 15, 16 and 18: `nifeos/jh7110_entropy#21`. The README's `BUGS` records item 18.
- Item 5: `nifeos/jh7110_entropy#26`, `#27` and `#30`. The ruleset on `main` requires
  `check / mutation-diff`.
- Item 19: `nifeos/jh7110_entropy#27`, which lowers `rust-version` to 1.85.
- Item 4: the board runs at `ee0bde8` (`nifeos/jh7110_entropy#22` and `#23`) and at the tag
  `v0.3.0-pre.1`, commit `497d3ec` (`nifeos/jh7110_entropy#25`, nife #3701). The `src/` on `main`
  is the tagged one.
- The tag `v0.3.0-pre.1` exists, and its release run and full mutation run passed.
- Item 17 needs no work: the block declines it.

## What remains

| item | what remains | blocker |
|---|---|---|
| 1 | Publish 0.3.0 to crates.io | publishing hold |
| 2 | Remove `publish = false` and `allow-unpublished: true` in the release pull request | release step |
| 3 | Create the first-publish token, approve the `crates-io` environment, bind the trusted publisher | publishing hold |
| Done when, host | `cargo publish --dry-run` with `publish = false` out | release step |
| Done when, host | The report-only README checks list nothing | unverified |
| Done when, CI | `weekly` green; the last run failed only `settings drift`, on a ruleset check it now carries | other: waits on the next weekly run |
| Done when, hardware | Whether a reseed clears `LFSR_LOCKUP`; no lockup has appeared | bench |
| Done when, hardware | `STATUS_SEEDED` and `LAST_RESEED` at the silicon's reset state, not the bootloader's | bench |
