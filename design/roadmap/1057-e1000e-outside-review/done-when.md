# Done when, on a host, in CI and on hardware

An appendix to [the milestone](../1057-e1000e-outside-review.md).

- P0-1 to P0-4 have closed. The `Hardware` trait carries `read`, `write` and `read_flash_status`,
  and `read16` appears nowhere. The weekly `mutation` and `stranger cadence` jobs pass on `main`.
  The git tag `v0.1.0` sits on the first commit on `main` at which the weekly gates pass after P0-2
  and P0-3, placed with the go of the architect, and the semver gate holds a pull request that
  breaks the API. Nothing goes to crates.io, and no release run happens.
- P1-1 to P1-6 and P1-8 have closed in the repository, each by a pull request that cites this
  block by number.
- P1-7 has a result, whichever way it went: a bring-up on a Sunrise Point I219 with a cable, with
  the bring-up notes, the link state and the frame counts in both directions written into the
  README's BUGS.
- The P1-8 procedure has run once against the `-device e1000e` of QEMU, and the README says what a
  pass prints.
- `cargo kani` verifies every harness, the new ones included, and every falsification patch fails
  its harness, on the tagged commit.
- Each P2 item has either closed or become an entry in the README's BUGS, so the crate states each
  limit a reader would otherwise discover.

## What needs hardware or QEMU

The review ran none of these.

- The I219 bring-up with a cable to a switch (P1-7): link state, and frames in both directions.
- The effect of porting the `GCR`, `WUFC` and `VET` writes, if P1-2 ports them, checked on the same
  part.
- The 82574L data plane under QEMU's `-device e1000e` (P1-8).
- The cost of one tail write per frame against a batched tail (P2-4), under QEMU and on hardware.
