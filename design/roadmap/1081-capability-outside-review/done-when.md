# Done when

An appendix to [the milestone](../1081-capability-outside-review.md).

- `script/crate-move --dry-run` over the crate reports zero findings against the crate: the
  manifest, the licenses and the README are in place, and the citation counts are zero.
- `helpers/crate_move.py` regenerates a patch whose hunk ends in a blank line without losing it,
  inserts `repository` after `license.workspace`, and its selftest covers both; all fourteen staged
  patches pass `git apply --check -p1`.
- One function in the crate holds the delegation rule with the `GRANT` check. `Delegation::derive` in the kernel calls it. A harness proves it for every rights pair and ships with a patch that `proofs / falsify` replays red. `get_with` has a caller or is gone.
- `get(1 << 32)` is `NoSuchSlot` on every target, with a test.
- `capacity` exists and the kernel's four sites use it; `len` either returns `used` or does not
  exist.
- `insert_at_or_above` and a second `put` appear in the two state harnesses, and a harness states
  that the mark never falls.
- The three known survivors are in `mutants-exempts` with their reasons before the first weekly run.
- The architect rules the crate's name (D1) before anyone creates the repository.
- `cargo test`, `cargo kani`, the template's `check` and `proofs` workflows, and `cargo mutants`
  (none missed beyond the three exempted) are green at the commit the move pins.
