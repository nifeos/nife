# Done when

An appendix to [the milestone](../1074-designware-ethernet-outside-review.md).

- `check / standalone`, `proofs / kani` and `proofs / falsify` are green on the first pull request
  in the crate's repository, with no baseline file and no allowance added for a finding this block
  names as the crate's to fix.
- `DataPlane::receive` returns after at most `RECEIVE_ENTRIES` consumed descriptors, a test shows
  it, and the twelve `gap` rows of the triage read `killed` in the next census.
- `Link` and `Speed` live in `link` with no re-export from the Motorcomm module, the sixteen gates
  are `pub(crate)`, and the five names of decision 3 carry the spelling ruled 2026-11-14 (UTC), each
  with its specification alias. This holds in the tree.
- The README has a build, test, proof and falsification section with the crate's own commands, its
  example runs under `cargo test`, and `radon` appears nowhere in the crate.
- The manifest carries `repository`, and no comment in it points at a file outside the repository.
- A new stranger run is on record with no open finding against the README.
