# What the review of e1000e covered

An appendix to [the milestone](../1057-e1000e-outside-review.md).

The reviewer verified these firsthand, in a fresh clone on a Linux host with Rust 1.99.1:

- `cargo build` for the host and for `aarch64-unknown-none-softfloat`,
  `riscv64imac-unknown-none-elf` and `x86_64-unknown-none`: all four build clean.
- `cargo test`: 67 unit tests and 2 doctests pass. `cargo clippy --all-targets` reports no warning,
  `cargo doc` with warnings as errors is clean, and `cargo deny check` passes advisories, bans,
  licenses and sources (one unmatched allowance, `Zlib`, is a note, not a failure).
- `cargo kani` (Kani 0.68.0, CBMC 6.11.0): 3 of 3 harnesses verify. With all three falsification
  patches applied in a copy, 3 of 3 fail, each at the assertion its patch header names.
- `cargo mutants` over `src/data_plane.rs` and `src/descriptor.rs` (100 mutants): 82 caught, 7
  missed, 7 unviable, 4 timeouts. The tests catch every mutant in `data_plane.rs` or time it out.
  The 7 survivors are all OR-to-XOR swaps in `descriptor.rs` that the compile-time assertions
  beside them argue are equivalent.
- The GitHub state of the repository: no tag, no release, two merged pull requests, no issue.
  `check` and `proofs` pass on `main`. The latest `weekly` run failed in two jobs (`stranger
  cadence` and `mutation`), and its five mutation shards reported 82 missed, 711 caught, 27
  unviable and 4 timeouts in all.
- Every register offset and bit constant, against FreeBSD's `e1000_regs.h`, `e1000_defines.h`,
  `e1000_ich8lan.h` and `if_em.h` and Linux's `regs.h` and `defines.h` as the reviewer knows them.
  The full I219 sequences, against FreeBSD `main`'s `e1000_ich8lan.c`, `e1000_phy.c`,
  `e1000_mac.c` and `if_em.c`, fetched for the purpose. No offset or bit is wrong. The write of the
  PCI vendor id to the read-only `STRAP` register around the global reset looks like an invention;
  it is FreeBSD's own (`e1000_reset_hw_ich8lan`). The 2.5 second wait on the Management Engine and
  the ten-second flash cycle bound are FreeBSD's values.

Inferred, not verified: the behavior of any sequence on silicon, and the effect of the register
writes the port omits (P1-2).

Not run: anything on hardware or under QEMU, the stranger test, `cargo-semver-checks` (no baseline
tag exists), a full mutation run outside the two files above, Miri (the weekly job ran it green),
and the `release` workflow of the template. The reviewer pushed, tagged, branched and opened
nothing in the repository.

Effort sizes: S is an hour, M a day, L more, or a bench.
