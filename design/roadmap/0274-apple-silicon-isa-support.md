---
status: NOT-STARTED
raised: 2026-09-10
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 274. Apple Silicon's own core is untested, and not for the reason first recorded

Measurement, not a design fork.

## Why the default run does not answer the question

`cargo xtask test --arch aarch64` without `--hvf` does not run under HVF. The suite takes the HVF
leg only when `--hvf` is passed (`xtask/src/suite.rs`, `let hvf = std::env::args().any(|a| a ==
"--hvf")`); without it, the aarch64 leg runs under TCG's default `cortex-a72`, which
`notes/entropy.md` documents as predating `FEAT_RNG`. So that run's RNDR skip and PMUv3 pass
measure **QEMU's emulated `cortex-a72`, not the physical core**.

The HVF leg boots this kernel on a GICv3, the runner's HVF default, through the driver that
milestone 227 (a GICv3 driver, because GICv2 boots and silently loses every interrupt) built;
`notes/hvf-leg.md` has its runs. This block holds no HVF reading of `FEAT_RNG`.

## What this milestone is
1. The `neoverse-n2` leg of this test has passed once, by hand. `.github/workflows/ci.yml`'s
   `test` job runs `script/ci-build` on `ubuntu-24.04-arm` over QEMU/TCG with no `NIFE_CPU` set, so
   it takes the default `cortex-a72`. The `cpu-matrix` job that does sweep CPU models is
   riscv64-only by its own name. Milestone 162 (real hardware entropy on x86_64 and aarch64), which built this
   test, recorded that the default skips it, and its correctness claim rests on one manual
   `script/test --arch aarch64 --cpu neoverse-n2` run. `321 passed, 3 skipped` (or `328 passed, 2
   skipped` on riscv64) reads as clean in CI and on this dev machine, and nothing compares the skip
   count against a baseline to notice that one of them is never exercised as designed.
2. Whether Apple Silicon implements `FEAT_RNG` is unknown, not "confirmed absent". The HVF leg is
   where it is read.

## What this needs

1. Give aarch64's TCG leg the same CPU-matrix coverage riscv64 has, so `neoverse-n2` runs on
   every CI merge instead of having run once, by hand.
2. Correct `entropy_tests.rs`'s comment, which reads as though `--cpu neoverse-n2` reliably fixes
   the skip. It fixes the TCG case only.
3. Read `FEAT_RNG` on the physical core from the HVF leg (`cargo xtask test --arch aarch64 --hvf`).
   Checking radon (the JH7110's own core) is a separate question about a different chip and stays
   open regardless.

## BUGS

- A skip that stands in for a pass is caught by no gate: `script/lint` and CI's own
  required-check machinery both consider `321 passed, 3 skipped` a clean run, and nothing compares
  the skip count against a baseline the way other counted-claims checks in this tree do. Same shape
  as milestone 268's finding 4 (the tour's checks report failure by printing a word and continuing):
  a real assertion existed, and nothing was reading its result.

## Follow-on
- **Milestone 227.** Its GICv3 driver is what lets the HVF leg boot, so the
  physical core can be measured from this machine.
- **Milestone 222.** Milestone 222 (the one command a person runs before pushing has a leg that
  fails instead of skipping) covers HVF's silent-failure shape; this is a second instance of the
  same class.
- **Milestone 268.** The same class of defect as this block's BUGS entry: a real check whose
  verdict nothing enforces. Worth reading together rather than as coincidence.

## Index row

The aarch64 default run measures TCG's `cortex-a72`, not the physical core, and the `neoverse-n2`
leg of this test has passed once, by hand. Whether Apple Silicon implements `FEAT_RNG` is unknown;
the HVF leg, which boots on a GICv3 since milestone 227, is where it gets read.
