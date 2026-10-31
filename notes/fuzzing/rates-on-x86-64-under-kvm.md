# Fuzzing: the in-guest drivers' rates on x86_64 under KVM, measured

*Name: provisional.*

An appendix to [`notes/fuzzing.md`](../fuzzing.md). It records rates, so that a budget in seeds can
be turned into hours. The consumer is the fuzz-budget proposal for risk 7's criterion (d), pull
request 2955 (`design/roadmap/proposals/the-fuzz-budget-that-counts-for-risk-7.md`), which sizes a
run in seeds and CPU-hours. That proposal's measurements section does not carry these figures yet
because its pull request was open and waiting on an architect when they were taken; a follow-up
links this page from it. Every date is 2026-10-31 (UTC).

## What was run

The kernel-thread driver (milestone 752) and the user-mode confinement driver (milestone 779), each
for 60 seconds per guest, with 1, 2, 4, 8 and 16 guests at once, on x86_64 under KVM, on a dedicated
x86_64 virtual machine. The host target `redoxfs_server_session` ran on the same machine.

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
on the user-mode driver. KVM is therefore roughly twenty times faster than TCG for the first and
roughly thirty times for the second.

## What was not measured

aarch64 and riscv64 under KVM: the host has TCG only for them, so their rates here are unknown. The
TCG rows say nothing about a hosted runner, which has its own figures in the proposal.

## Why concurrency is not yet a lever

Both drivers fail on wall-clock deadlines when guests run side by side (1 of 4 guests at 4 for the
kernel-thread driver, 4 of 8 at 8, 6 of 16 at 16; user-mode 0 of 4, 1 of 8, 6 of 16). A seed
that failed under load was green when replayed alone, so the failures are the harness's. The entry
that tracks the fix is in milestone 779's `BUGS`; until it is fixed, a concurrent run can report
false reds, so the 1-guest rows are the ones to size from.
