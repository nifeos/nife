# Done when

An appendix to [the milestone](../1093-globally-unique-identifier-partition-table-outside-review.md).

- `check / standalone` passes on the new repository: no `Name:` block, no nife path, the badge row
  present, `prose_acronyms` at zero, `spelled_out` and `register_names` at zero after the rename
  lane the package plans lands.
- `Entry::blocks` returns `None` for a range its count cannot hold, with a test and a harness.
- `parse_backup` exists, a test opens the `sgdisk` fixture from its tail alone and restores a
  primary header byte for byte, and a harness with a patch covers the round trip through
  `write_backup_header`.
- `check_disk_size` exists and the README's contract names the three checks a caller runs after
  `parse`; the `parse` documentation and the README example show `Header::decode` before the array
  read.
- `#![forbid(unsafe_code)]` is in the crate root, and the repository's `fuzz.yml` carries the opt-in
  with the moved target.
- `#![allow(missing_docs)]` is gone and `cargo doc` with warnings denied is clean.
- `Error` implements `core::error::Error`; `BackupMismatch` and `MbrProblem` implement `Display`;
  the three enums carry `#[non_exhaustive]` or a recorded reason not to.
- `.github/mutants-exempts` carries the six recorded equivalents and `mutation / mutation` has one
  green full run on `main`.
- The README has the scaffold's proof and falsification sections with this crate's figures, and a
  stranger run is on record with no open finding.
- The architect rules the two decisions above, and the rename lane applies decision 1 if it rules that way.
- `cargo test`, `cargo kani` and `cargo mutants` (none missed, none unexempted) are green at the
  commit the release tags.
