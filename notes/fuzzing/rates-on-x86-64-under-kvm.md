# Fuzzing: the in-guest drivers' rates on x86_64 under KVM, measured

*Name: provisional.*

An appendix to [`notes/fuzzing.md`](../fuzzing.md). It records rates, so that a budget in seeds
turns into hours. The consumer is the fuzz-budget proposal for risk 7's criterion (d), which sizes a run
in seeds and CPU-hours. It sits in pull request 2955 as
`design/roadmap/proposals/the-fuzz-budget-that-counts-for-risk-7.md`. That proposal's measurements section does not carry these figures, because its
pull request was open and waiting on an architect when they came in. A follow-up links this page
from it. Every date is 2026-10-31 (UTC).

## What ran

The kernel-thread driver, milestone 752 (a seeded syscall driver with a shadow model), and the
user-mode confinement driver, milestone 779 (fuzz the surface a confined process can reach), each ran
60 seconds per guest. The runs used 1, 2, 4, 8 and 16 guests at once. The host was a dedicated x86_64 virtual
machine, and the guests ran x86_64 under KVM. The host target `redoxfs_server_session` ran on the
same machine.

## Rates, per guest

| run | rate |
|---|---:|
| kernel-thread driver, 1 guest | about 420 seeds/s |
| kernel-thread driver, 2 to 8 guests | about 440 to 460 seeds/s |
| user-mode driver, 1 guest | about 230 seeds/s |
| user-mode driver, 2 to 8 guests | about 220 seeds/s |
| kernel-thread driver under TCG | about 20 seeds/s |
| user-mode driver under TCG | about 8 seeds/s |
| `redoxfs_server_session`, 1 process | 794 runs/s |
| `redoxfs_server_session`, 8 workers | about 5,470 runs/s in all |

Resident memory is about 100 MB per guest. At 16 guests the kernel-thread rate per guest falls to
about 360 to 450 seeds/s.

## What it means for a budget of 300,000 seeds

At one guest, 300,000 seeds take about 12 minutes on the kernel-thread driver and about 21 minutes
on the user-mode driver. KVM runs roughly twenty times faster than TCG for the first driver and
roughly thirty times for the second.

## What nobody measured

aarch64 and riscv64 under KVM. The host has TCG only for them, so their rates are unknown. The TCG
rows say nothing about a hosted runner, which has its own figures in the proposal.

## False reds when guests run side by side

Each guest ran 60 seconds of seeds. A red here is a run that failed and whose seed passed alone.
The first and third columns are a two-second deadline on the counter; the second and fourth are
`testing::wait_on_every_core`, two seconds of ticks on every core (notes/kernel-test-harness.md).
Every figure is from 2026-10-31 (UTC).

| guests | kernel-thread, counter | kernel-thread, every core | user-mode, counter | user-mode, every core |
|---:|---|---|---|---|
| 1 | 0 of 1 | not run | 0 of 1 | not run |
| 2 | 0 of 2 | not run | 0 of 2 | not run |
| 4 | 1 of 4 | 0 of 16 | 0 of 4 | 0 of 16 |
| 8 | 4 of 8 | not run | 1 of 8 | not run |
| 16 | 6 of 16 | not run | 6 of 16 | not run |

The every-core column at 4 is four runs of four guests, two of them beside 40 busy loops at the
same priority, which cut the rate by a third. At 4 guests the counter deadline rarely fails either,
so that row does not tell the two apart; 8 and 16 guests do, and are unmeasured. Until they are
measured, size a concurrent run at 4 guests or fewer.
