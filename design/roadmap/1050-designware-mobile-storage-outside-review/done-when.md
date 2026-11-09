# Done when, on a host, in CI and on hardware

An appendix to [the milestone](../1050-designware-mobile-storage-outside-review.md).

- Every P0 item is closed or carries a recorded decision from the milestone's list.
- Every P1 item is closed. The recovery item (P1-1) and the deadline item (P1-2) are confirmed on a
  VisionFive 2 by the storage bench, with the measurements in the milestone's `BUGS` section or the
  README of the crate.
- `weekly` on `main` is green: the mutation shards complete, each survivor is exempted or caught,
  and the stranger run is recorded within 28 days.
- The git tag `v0.1.0` exists on the first green commit after the P0 items, placed with the go of
  the architect, and the semver rule holds a pull request that breaks the API. No version is
  published.
- The README states the trait contract and adds the dependency line; docs.rs and GitHub show the
  same front page.
- The P2 items are filed as issues in the repository of the crate, or in a follow-on block here.

## What needs the board

None of these is done. Each is a line in the storage bench of the kernel, run on a VisionFive 2:

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
