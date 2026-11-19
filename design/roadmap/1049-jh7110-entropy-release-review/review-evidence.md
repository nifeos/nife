# What the review verified firsthand

An appendix to [the milestone](../1049-jh7110-entropy-release-review.md). It records what the
review lane ran or read at `98c211e`; the items' status lines say what holds on `main` now.

The lane ran or read every claim in this section on a host; nothing here ran on a board.

- `cargo build`, `cargo test` (18 unit tests, 2 doctests), `cargo clippy --all-targets -D warnings`,
  `cargo doc` with warnings denied and `cargo fmt --check` pass on stable 1.99.1 and on 1.88, the
  manifest's `rust-version`.
- Kani 0.68.0 verifies all four harnesses. Each of the four falsification patches applies with
  `git apply -p1` and makes its own harness fail.
- cargo-mutants: 79 mutants, 76 caught, 1 unviable, 2 missed, and the 2 missed are the two
  `1 << 0` constants `.github/mutants-exempts` exempts with a reason.
- cargo-semver-checks reports no API change between the tag `v0.2.0` and `main`.
- `cargo package` builds a 37,612-byte archive; `cargo publish --dry-run` was not run because the
  manifest refuses it (`publish = false`).
- The 20 register offsets, 4 command values and 20 bit constants match Linux's
  `drivers/char/hw_random/jh7110-trng.c` and NetBSD's `sys/arch/riscv/starfive/jh7110_trng.c`,
  read from their current trees. NetBSD also names field bits in `FEATURES` and `BCONF` that the
  crate does not.
- The source holds no `unsafe`, no dependency, no contraction in prose, and no mention of the
  parent project beyond the organization and App names the workflows need.
- The template pin `d332215` sits on `nifeos/crate-workflows` `main`, 57 commits behind it. Since the pin the template has moved the full mutation run into its own `mutation.yml`. It has made
  `check / mutation-diff` a required check on every crate repository's `main`. It has also taught the release gate to refuse a release without a green full mutation run at the release commit.
- The newest weekly run (`42658233510`) failed in `weekly / mutation`: the verdict step found
  `0 of 1 shards left an outcomes.json` after the shard itself passed in 55 seconds. The weekly
  run before it was green. The defect is in the pinned template's artifact handling, not in the crate.
- The current template's helpers, run over the checkout, give this result. The seven naming checks refuse 0 of 162 names, and `standalone` passes. The report-only checks list three README section-order findings, one
  acronym (`TRNG` in the crate doc's first sentence, before its expansion) and 20 name components
  that are not dictionary words (`reseed`, `unseeded`, `buf`, and test-name components).
- crates.io has no `jh7110_entropy`. rand_core's current release is 0.10.1; its traits are
  `TryRng`, `Rng`, `CryptoRng` and `TryCryptoRng`, with `RngCore` and `TryRngCore` kept as
  compatibility traits. getrandom's README documents an opt-in `custom` backend selected by the
  `getrandom_backend` configuration flag.
- The GitHub team `nifeos/crate-maintainers` that `CODEOWNERS` names exists.

Not run: a comparison against the JH7110 Technical Reference Manual (no copy in hand), the vendor
Linux and U-Boot trees, Miri, fuzzing, a build on a 1.85 toolchain, the template's release dry run,
and anything on hardware.

## Items 4, 5, 7 and 10 as the review raised them

All four are done; the milestone's status line for each says where it landed.

### Item 4: The published tree ran on the board twice, at one commit

The bench ran on radon (a VisionFive 2) on 2026-11-09 and 2026-11-10 (UTC) against crate commit
`ee0bde8feb9f78a67da011c928f9a2a697f3bbc1`, with one payload built from
nife `9a1c55963b`. It loaded over the network after a cold power-on; the person says the second followed
an hour unpowered. The checker printed PASS both times. Tables, logs and SHA-256 sums are in
[bench/radon-2026-11-09/](../../../bench/radon-2026-11-09/README-1049.md) and
[bench/radon-2026-11-10/](../../../bench/radon-2026-11-10/README-1049.md).

- Passed, both runs: the polled reseed and generations, 256-bit output after `MODE_256_BIT_OUTPUT`, and
  1,048,576 driver-path generations with no timeout, wrong width or all-zero output.
- Recorded, not judged: the first `STAT` read was `0x08040308` both times (`SEEDED` = 1, `LAST_RESEED` = 4).
  That repeats across two power-ons; it is the state after the bootloader, not shown to be silicon reset. `MODE`, `FEATURES` and `BCONF` were read before any `MODE` write, so "`BCONF` bit 3 set means
  256-bit" holds only if nothing wrote `MODE` since reset. `STAT` bits 19 to 23
  never set. The tables differ in the reseed median and the `RAND_RDY` with `RAND_GENERATING` counts.
- Open: no `LFSR_LOCKUP` appeared, so whether a reseed clears it is unknown.
- Statistics: two captures; 2 and 1 of 524 FIPS 140-2 blocks failed (bar at most 3). They show no gross defect. They prove nothing about unpredictability. The
  second run's `ent` output recomputes the same statistics: a cross-check, not an independent suite.

Two-way. Effort M, bench time.

### Item 5: Bump the template pin and adopt what the template now expects

`ci.yml` and the five
sibling callers pin `d332215`, 57 commits behind. At the current template the full mutation run lives in a caller `.github/workflows/mutation.yml`. That file carries the repository's own cron line, which `python3 helpers/mutation_plan.py schedule jh7110_entropy` prints in a template checkout. `check / mutation-diff` is a required check that `script/apply-settings` adds to the ruleset,
`.github/ci-job-budgets` gains `mutation-diff 30` with its reason, and the release gate
refuses a release with no green full mutation run at that commit. The red weekly run
(`42658233510`) is the pinned template's defect and goes away with the bump. Two-way. Effort S.

### Item 7: The decode ignores a generation or reseed in flight

`STATUS_GENERATING` and
`STATUS_RESEEDING` exist as constants, and `generation_outcome` does not
read them. `generation_outcome` answers `Ready` for a snapshot with `RAND_RDY` latched while `STAT` reports a generation in flight, and `RANDOM_NUMBER_0..7` may be mid-rewrite at that instant. Linux's driver
polls both bits clear before issuing a command, which is why it never meets this; a pure decoder
can return `NotReady` while either bit shows, which costs a caller one more poll and nothing else.
The hardware can show `RAND_RDY` and `RAND_GENERATING` together: the bench saw both in thousands of
snapshots after a second `GENERATE` over a latched `RAND_RDY` (item 4). Recommended: fold the two bits into the `NotReady` arm, extend
`neither_bit_set_is_always_not_ready` or add a harness, and add the falsification patch. The
function's signature does not change; its answers for some inputs do. Two-way. Effort S.

### Item 10: `Pool` has no proof

The README says `Pool` has unit tests and no harness. Its invariants are small enough for Kani. For any `n: u64` and any generator that answers or refuses, `take` returns a count of at most `min(n, 8)`, serves the generator's bytes in order
across one refill seam, zeroes what it served and never panics. When the generator refuses, it answers a short count rather than zeros. The buffer is 32 bytes and the gather loop runs at most 8 times, so the harnesses stay cheap. Each one carries a falsification patch. Two-way. Effort M.
