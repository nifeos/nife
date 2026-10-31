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
| kernel-thread driver under TCG, 1 guest (second measurement) | 15 to 18 seeds/s |
| user-mode driver under TCG, 1 guest (second measurement) | 9 to 10 seeds/s |
| `redoxfs_server_session`, 1 process | 794 runs/s |
| `redoxfs_server_session`, 8 workers | about 5,470 runs/s in all |

The KVM rows and the redoxfs rows come from the first measurement. The two TCG rows come from a
second measurement, corrected on 2026-10-31 (UTC). Those were hand runs of x86_64 guests under TCG,
one guest at a time and two runs per driver. They used QEMU 11.1.2, 256 MiB of guest RAM and 2
vCPUs, on a shared multi-core build host with a load average of 25 to 30. The kernel-thread runs gave 17.6 and
15.4 seeds/s and the user-mode runs gave 9.9 and 8.8. One earlier TCG run of the kernel-thread
driver, on a quiet host inside the suite, gave about 34 seeds/s. Nobody explained the gap, so treat
the TCG rows as a floor measured under load.

Resident memory is about 100 MB per guest. At 16 guests the kernel-thread rate per guest falls to
about 360 to 450 seeds/s.

## What it means for a budget of 300,000 seeds

At one guest, 300,000 seeds take about 12 minutes on the kernel-thread driver and about 21 minutes
on the user-mode driver. Against the second measurement's TCG rows, KVM at one guest runs about 24 to 27 times faster
for the kernel-thread driver, which is about 420 against 15 to 18 seeds/s. For the user-mode driver
it runs about 22 to 25 times faster, which is about 220 against 9 to 10 seeds/s. The ratio
for the kernel-thread driver would be about 12 against the 34 seeds/s quiet-host run.

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
so that row does not tell the two apart. 8 and 16 guests would, and nobody has run them yet; until
someone does, size a concurrent run at 4 guests or fewer.
