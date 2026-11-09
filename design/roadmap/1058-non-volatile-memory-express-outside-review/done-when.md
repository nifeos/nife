# Done when, on a host, in CI and under QEMU or on hardware

An appendix to [the milestone](../1058-non-volatile-memory-express-outside-review.md).

- `Cargo.toml` carries no comment that points outside the repository, lists `nvme` among its
  keywords, states the minimum Rust version the weekly job confirms, and ships only the files a
  crates.io user needs. Nothing goes to crates.io while the publishing hold stands.
- Every public name has a ruling on record before the tag, and no public parameter holds a quantity
  or an address without saying so. The four register names of P0-2 stand as ruled.
- `Namespace` carries its unit, the two methods take none, and a harness proves the block bound
  with a falsification patch that `proofs / falsify` replays red.
- Every decoded field carries the mnemonic of the specification as a `doc(alias)`.
- The README follows the shared section order and states the ordering discipline on both rings. It
  shows a completion whose head frees the slot it completes, and it gives the install line for every
  tool it asks the reader to run. A new stranger run is on record with no open finding against it.
- The callers pin a release of the template that carries the mutation stack, `check /
  mutation-diff` is a required check, and `mutation / mutation` has one green full run on `main`.
- `cargo test`, `cargo kani` (every harness verifies, every patch fails), `cargo mutants` (no miss
  outside the ledger) and all three fuzz targets are green at the commit a tag would carry. The tag
  itself waits for its own ruling.
- P1-1 to P1-14 have closed in the repository, each by a pull request that cites this block by
  number, and each P2 item has either closed or become an entry in the README's BUGS.

## What needs QEMU or hardware

The review ran none of these.

- QEMU: a polled driver built from the example of the README and nothing else, against the `nvme`
  device of QEMU. It identifies the namespace and creates one I/O queue pair at depth 2, so both
  rings wrap within the test. It writes a block, reads it back, records every reported `SQHD`
  without an error, and flushes. Once P1-9 lands, the same run repeats with the controller set for
  a 16 KiB memory page.
- Hardware: the same driver on one real controller, with a namespace formatted to a non-zero
  `FLBAS`, so that the parse reads the format table at an index other than zero.
- A weakly ordered host (aarch64): a run long enough to cross the wraps of both rings thousands of
  times with the ordering discipline of P1-6 in place, and no stale field observed.
