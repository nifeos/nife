# Done when, on a host, in CI and on hardware

An appendix to [the milestone](../1050-designware-mobile-storage-outside-review.md).

- Every P0 item has closed or carries a recorded decision from the list in the milestone.
- Every P1 item has closed. The storage bench confirms the recovery item (P1-1) and the deadline
  item (P1-2) on a VisionFive 2, and the measurements go in the `BUGS` section of the milestone or
  the README of the crate.
- `weekly` on `main` passes: the mutation shards complete, an exemption or a test covers each
  survivor, and the stranger run carries a date within the last 28 days.
- The git tag `v0.1.0` exists on the first green commit after the P0 items, placed with the go of
  the architect, and the semver rule holds a pull request that breaks the API. Nothing goes to
  crates.io.
- The README states the trait contract and adds the dependency line; docs.rs and GitHub show the
  same front page.
- The P2 items sit as issues in the repository of the crate, or in a follow-on block here.

## What needs the board

The review ran none of these. Each is a line in the storage bench of the kernel, run on a VisionFive 2:

- The IDMAC path on silicon: a multi-block read and write through a region, read back through the
  polled path. The README says it has run only against the simulator.
- The eMMC socket: identification, the `EXT_CSD` read and the capacity, with an eMMC module fitted.
- Recovery (P1-1): a read interrupted by pulling the card or by an injected CRC error, then a
  successful command on the same card.
- Busy durations (P1-2) for a single-block and a multi-block write on both the microSD and the
  eMMC, logged against the deadline defaults.
- The polarity of `CDETECT`, and hot removal (P2-3).
- The 50 MHz card-interface clock assumption against the clock tree or a scope, and the 25 MHz card
  clock it yields.
- The `VERID` and `HCON` words of both controllers, logged, so the 2.40a FIFO offset and the
  interface fields of the IDMAC are facts rather than readings of the tree.
- The scratch write (`MasterBootRecord::scratch_sector_range`) written and restored on the boot
  card.
