# The contract crates leave in this order: how a breaking change lands across two repositories

[The proposal](../the-contract-crates-leave-in-this-order.md) holds the order and the policy. This appendix works two cases through it, one real and one hypothetical.

A real one, 2026-10-31 (UTC). `jh7110_entropy` pull request #5 renamed five constants and merged at
22:22 (`nifeos/jh7110_entropy@535f79c`). nife pull request #2979 opened at 22:27, moved the three
manifests that pin the crate and the program that names the constants, and merged at 23:15. Fifty-three
minutes end to end. nife's `main` never broke, because it pinned the old commit until the bump.

The contract case, worked for `block_roster` after step 4: a hypothetical page change to version 2,
with the durations the proposal measured.

1. The contract repository merges version 2: `VERSION = 2`, magic `blkrosv2`, Cargo `0.2.0`, a dated
   ledger entry. A trial run against nife's and `disk-tools`' current pins shows what breaks.
   About 7 minutes of the crate's CI.
2. nife bumps its `rev`, and the kernel writes version 2. Its pull request names the landed commit.
   About 50 minutes through the merge queue.
3. `disk-tools` bumps its `rev`, and `disk_surveyor` reads version 2.
4. basalt moves the three pins in one pull request. The coherence check passes only with all three.
   About 10 to 15 minutes of gate.

The page is single-version, so step 4 is atomic. If basalt moved nife alone, the agreement check
would refuse: the kernel serves (5, 2) and `disk_surveyor` declares (5, 1). A message contract
serves N and N-1, so steps 2 and 3 may ship in different basalt bumps. As §201 (one roadmap until
a citation has to cross) requires, the pull requests in steps 3 and 4 name what they wait for, and
step 1 names nothing.
