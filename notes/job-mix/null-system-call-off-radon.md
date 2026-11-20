# The null system call off radon, 2026-10-05

*An appendix of [the null-system-call note](null-system-call-under-load.md), measured by
`lane/null-system-call-under-load` on patagonia with no board. The file name is provisional (see the README).*

Milestone 761 (capability lookup off the global lock) reached `main` inside #1617 (the 64-slot table) before #1630 itself landed. So
`main` at `d0ca36c5b` already looks up capabilities off `IPC_TABLES`. Eight interleaved TCG runs
each of `main` and #1630's head (`4b07821a`) overlapped at every point. The baseline is therefore an
ablation: `main` with `current_cap` holding `IPC_TABLES` across the lookup again, as it did before
761. That changes one thing. The ablation and the sweep of 1, 2, 3 and 4 tasks were scratch
patches, never committed.

## The questions

Drafted before any run, as `main` against #1630. Once 761 turned out to be on `main` they were
reworded for the ablation, and Q1's second clause (the excess should match the measured wait) was
added after the runs. Say so rather than let the table look more prior than it was.

The excess is this note's usual quantity: `null_system_call`'s growth from one task to N, minus the
growth `compute` shows between the same points (its share of preemption and interrupts), divided
by 64 traps. Only the excess is read here, never a tick.

| Question | Fixable defect if | Architectural cost if |
|---|---|---|
| Q1. Does the global lock in the lookup produce the excess on its own? | adding it back brings the excess back, and the excess matches the measured wait | adding it back changes nothing |
| Q2. With it gone, is anything left? | the excess falls to within the runs' spread of zero | an excess remains while `current_cap` waits for nothing |
| Q3. Is it queueing? | the excess grows with each added contender | flat in contenders, or a step at two (line traffic, interrupts) |

## TCG, riscv64

`script/job-mix --arch riscv64 --smp 4 --release`, eight runs a side interleaved, host load about
2. Medians, range in brackets, ticks of the 10 MHz timer per 64-trap job.

| `null_system_call` | 1 task | 2 | 3 | 4 |
|---|---|---|---|---|
| ablation (lookup under `IPC_TABLES`) | 684 [646-692] | 949 [922-976] | 1,041 [1,005-1,048] | 1,170 [1,140-1,184] |
| `main` (lookup under the thread's own lock) | 678 [641-688] | 868 [837-889] | 936 [923-971] | 1,000 [996-1,029] |
| excess per trap, ablation | | 1.58 | 3.67 | 4.14 |
| excess per trap, `main` | | 0.40 | 1.97 | 1.64 |

`compute` read 326, 404, 383 to 390 and 428 to 432 on both sides. One `--lock-wait` run of the
ablation found `IPC_TABLES` held on 24% of `current_cap` calls at four tasks, a wait of 3.49 ticks
per trap; two of `main` found 20 contended calls in 479,000. So under TCG the lock was most of
the excess and grew with every contender (Q1, Q3), and 1.6 ticks per trap remain that no lock
explains. With the spawn job stubbed out, `main`'s remainder at four tasks fell to 0.90 [0.79-1.01]
(six runs): about half of what is left travels with spawning. TCG stops every vCPU for a remote
fence, so that half is not evidence about radon.

## HVF, aarch64: four real cores

Milestone 227 (a GICv3 driver) made `script/job-mix --hvf` boot, which [the job-mix
note's cross-check](hvf-cross-check-2026-09-19.md)
said it could not. The full mix could not run there when these were taken, so these runs stub the
spawn job on both sides; [`spawn-job-miscompiled-yield.md`](spawn-job-miscompiled-yield.md) fixed that afterwards. Six runs a side, ticks of the 24 MHz counter per 64-trap job:

| `null_system_call` | 1 task | 2 | 3 | 4 |
|---|---|---|---|---|
| ablation | 54 [52-56] | 56 [54-57] | 54 [53-57] | 61 [59-64] |
| `main` | 48 [47-52] | 47 [46-48] | 46 [46-47] | 47 [46-47] |

`compute` read 620 to 637 throughout. `main` is flat to within a tick, an excess of -0.02 per trap.
The ablation's excess at four tasks was 0.11 per trap [0.06-0.18], and two `--lock-wait` runs of
it measured 0.10 and 0.11 ticks per trap of `current_cap` waiting (3.4% and 4.6% of calls). The
whole excess is the wait. The extra uncontended acquisition costs the one-task job 6 ticks, about
4 ns a trap.

Not apples to apples with radon, and the size is not the point. macOS schedules the vCPUs, a
cross-core wakeup leaves the guest, and the mix's throughput does not scale under HVF at all
(`jpm_median` 8.8 to 9.4 million at every point), so `IPC_TABLES` is far quieter here than on
radon's 41%. What HVF adds is real caches and real atomics: on them, with the lock out of the
path, the cheapest system call does not slow down as cores get busy.

## What this answers

The lock was a cause, it was this kernel's choice, and 761 removed it: Q1 yes on both machines,
Q2 yes on real cores for a mix without spawning. Q3 says queueing under TCG. Nothing found here
is a cost the capability model forces. What remains open is radon's size, and whether spawning's
share survives there, which is radon's next evening.

## The payloads for that evening

`main` is what ships, so two payloads from `d0ca36c5b` (761, the 64-slot table) stand beside
761's own. Same bench rules, thresholds and one-task guard as [the note's](null-system-call-under-load.md#the-radon-run-that-decides-it):

```sh
cd <worktree root>/null-system-call-under-load
script/board-netboot --root target/board-main             # boots 1 to 3
script/board-netboot --root target/board-main-lock-wait   # boots 4 and 5
```

That worktree has to outlive its pull request until the boots run; prune it after.

