---
status: DECIDED
raised: 2026-09-24
decided: 2026-09-25
ratified_by: calef
---

# 218. Carry a Kani patch so riscv64 is proved, and send it upstream

calef ruled 2026-09-25 (UTC), under [§46 (thin primitives or whole subsystems)](0046-dependency-rule.md),
on the proposal milestone 589 (Kani can prove riscv64 from the hosts we already have) was promoted
from. *(Section and milestone numbers provisional until the merge queue lands them.)*

## The ruling

nife carries a patch to Kani that lets it target `riscv64gc-unknown-linux-gnu` from an aarch64 or
x86_64 host, and the same change goes upstream. The research behind it is milestone 589's block:
CBMC is host-independent and already knows riscv64, so what Kani lacks is a riscv64 *target*, not a
riscv64 *host*. Four places in Kani hard-code the host triple; the patch unpins them.

- What is carried: [`patches/kani-0.67.0-riscv64-target.patch`](../../patches/kani-0.67.0-riscv64-target.patch),
  in `git format-patch` form, pinned to Kani 0.67.0, the release the tree's records and local runs
  use. It stays the minimal tested shape (about fifty lines over four files), selected by an
  environment variable, `KANI_TARGET` (provisional; Kani's name to give upstream, not ours).
- How it is built: from the pinned release tag plus that file, never from a fork. The fork the
  upstream lane pushes is only the source of the upstream pull request.
- Its exit: [model-checking/kani#2402](https://github.com/model-checking/kani/issues/2402)
  ("Command-line flag to change model target or environment"). A separate lane reworks the change
  into an upstream-shaped `-Z` flag against that issue. The carried file leaves `patches/` when the
  tree's Kani pin reaches a release containing riscv64 target support, which is the rule
  `patches/README.md` already states for every file there.
- Its cost: a rebase at each Kani release the tree adopts (at 0.68.0, one of ten hunks needed a
  one-line fix). And one CI job, `prove the kernel on riscv64` (provisional), which builds the
  patched Kani on the existing arm64 runner, cached on the Kani version and the patch's hash.

## When to revisit

Switch to a maintained fork of Kani only if the carried change grows past a few hundred lines, or
nife comes to carry several Kani patches at once (the maintainer, confirmed with calef,
2026-09-25). Below that line a single patch file is cheaper to read, rebase and delete than a fork
is to keep in step, and its header says what it is for. Above it the rebase stops being a
one-hunk chore and a fork's history starts to earn its keep.

## What was refused

From the proposal's options, each with its reason:

- Upstream only, and wait. Lost on time, not merit: #2402 has been open since 2023-04-23 with
  nobody objecting and nobody sending a pull request, so riscv64 would stay unproved for an unknown
  time.
- A native riscv64 host (`radon` or the Scaleway RV1). It needs the same compiler patch anyway,
  plus a CBMC built from source, Linux on a board whose job is running nife, and a self-hosted runner
  executing pull request code on calef's LAN.
- Compile asm-free riscv64 files as a proof-only module on the aarch64 host (the
  `riscv64-code-the-prover-can-already-compile` proposal's option 1, which milestone 432 (the RISC-V IOMMU driver has no counterpart to the SMMU's proofs) builds).
  Not chosen, because its reach is smaller: the target flag compiles every riscv64 line with the real `crate::arch`
  underneath. It is not wasted: its harnesses run natively under this job once both land.
- A maintained fork now. One fifty-line patch does not justify a fork's upkeep; see the revisit
  condition above.

## What this does not change

It does not make `asm!`, fixed-address MMIO or the four riscv64 `.s` files provable; those stay
outside the prover on every host, and [`notes/kernel-proofs.md`](../../notes/kernel-proofs.md) says
how far the reach goes. It does not touch `design/fatal-risks.md`, which is calef's file; its
sentence that riscv64 is unreachable becomes false when this lands, and milestone 589's follow-on
routes that correction.
