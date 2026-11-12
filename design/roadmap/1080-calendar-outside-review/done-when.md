# Done when

An appendix to [the milestone](../1080-calendar-outside-review.md).

- The package carries the name the architect chose, in its manifest, its repository and the
  six manifests of this tree that name it.
- `script/crate-move --dry-run calendar` stages eleven patches that `git apply --check -p1`
  accepts, and `helpers/crate_move.py --selftest` has a fixture whose hunk ends on a blank line.
- `standalone.py` reports nothing; the README has every section the stranger cadence names; a
  stranger run is on record with no open finding against the README.
- `UtcOffset::from_hm(i32::MAX, 0)` and `from_hm(i32::MIN, 0)` return `Err(Error::BadOffset)` in
  every profile, with tests and a harness that carries its falsification patch.
- The template's `spelled_out`, `unit_symbols` and `register_names` checks refuse no name in the
  crate, and `.github/accepted-words` lists `unix`, `utc`, `rfc` and `iso` with reasons.
- `Error` implements `Display` and `core::error::Error`; `Civil::from_days_since_epoch` exists
  and the timetable calls it.
- The parse harness reaches 28 bytes or a second harness covers the 27-byte form, and the three
  test rows of item 9 exist.
- `.github/mutants-exempts` names the surviving mutants with their reasons, and `mutation /
  mutation` has one green full run on `main` before the tag.
- `cargo test`, `cargo kani`, `proofs / falsify` and `check / mutation-diff` are green at the
  commit the tag names.
