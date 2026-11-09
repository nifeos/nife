# Done when, on a host, in CI and on hardware

An appendix to [the milestone](../1049-jh7110-entropy-release-review.md).

On a host, from the repository alone:

- Items 2 (prepared in the release pull request) and 5 to 13 have merged, and `cargo semver-checks
  --baseline-rev v0.2.0` reports the break that makes the next version 0.3.0.
- `cargo kani` verifies every harness, the new ones for items 6, 7 and 10 included, and every
  falsification patch under `falsifications/` fails its own harness and no other.
- cargo-mutants reports no survivor outside `.github/mutants-exempts`.
- `cargo package --list` shows only the files item 11 names, and `cargo publish --dry-run` passes
  once `publish = false` is out.
- The template's seven naming checks and `standalone` pass at the bumped pin, and the report-only
  README checks list nothing.

In CI, on `main` at the release commit: `check /` (seven jobs, `mutation-diff` included), `proofs /
kani`, `proofs / falsify` and `mutation / mutation` green; `weekly` green on its next schedule; a
release dry run green.

On hardware, with a VisionFive 2 on the bench; the review could run none of these and claims none:

- A driver built on the release commit, after the platform ungates the clocks and releases the
  reset, completes a reseed (`SEED_DONE`) and a generation (`RAND_RDY`) by polling, and the 32
  bytes are not all zero and differ between two generations.
- `STATUS_256_BIT_OUTPUT` reads 1 once the driver writes `MODE_256_BIT_OUTPUT`, and the value it reads
  before that write, with `FEATURES` and `BUILD_CONFIGURATION` dumped, answers what the silicon
  defaults to (item 6's "cannot be assumed either way").
- Whether one snapshot ever shows `RAND_RDY` and `RAND_GENERATING` together after a second
  `CONTROL_COMMAND_GENERATE` (item 7).
- `STATUS_SEEDED` and the `LAST_RESEED` field before the first reseed after reset.
- The time from `CONTROL_COMMAND_GENERATE` to `RAND_RDY` and from `CONTROL_COMMAND_RESEED` to
  `SEED_DONE`, so the README can give a measured poll bound instead of none.
- The residue in `INTERRUPT_STATUS_REGISTER` after writing
  `INTERRUPT_STATUS_DOCUMENTED_BITS_MASK`, and whether bits 2 and 3 latch when `AUTO_AGE` or
  `AUTO_RQSTS` is nonzero.
- Whether a long run shows `LFSR_LOCKUP`, and that a reseed clears it.
- A capture of at least one mebibyte run through a statistical suite, recorded as a measurement
  beside the README's "quality unmeasured" line, not as a proof.

