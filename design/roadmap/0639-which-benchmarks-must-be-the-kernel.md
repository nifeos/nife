---
status: NOT-STARTED
raised: 2026-09-21
promoted_from: which-benchmarks-must-be-the-kernel
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 639. Which benchmarks must be the kernel, and which are just programs

*Title and slug are drafts.*
Raised by calef, asking which other special kernel builds should be
userspace programs. `bench` is the one that cannot be answered without measuring, which is why it is
a proposal rather than a lane.

The measurement it asks for needs no board and no decision.

## The question

`kernel/Cargo.toml`'s `bench` feature is *"the benchmark boot (milestone 21 (performance measurement: benchmarks with teeth)): run the
microbenchmarks and halt"*, and `icount` builds on it. `notes/job-mix.md` records why the
multi-tasking workload did not go there: *"`kernel/src/bench.rs` would have been the obvious home
and is the wrong one"*, because the number had to be taken on radon and *"the bench boot has never
run on a board."*

That reason is about board plumbing, not about privilege, and milestone 523 (moving the job-mix
supervisor into userspace) is removing the board-plumbing reason for one workload. So the
question returns: does `bench` need to be a kernel build at all?

## Why the answer is "some of it", and why that needs measuring

Some of what `bench` times is genuinely kernel-internal. A context switch measured from a
userspace supervisor includes the syscalls that got in and out, which is precisely the cost the
measurement is trying to exclude. Moving those would not relocate a measurement, it would replace it
with a different one that has the same name, which is the most damaging thing a benchmark can do.

And some of it plainly is not. A spawn, a map, an IPC round trip and a filesystem read are all
things a program does, and the honest number for them is the number a program sees.

So the deliverable is a classification, not a move: for every benchmark `script/bench` runs, say
whether its number would change if it were taken from EL0, and by roughly how much. Where it would
not change, the benchmark is a program. Where it would, the benchmark stays and the block should say
why in one sentence, so nobody asks again.

## What makes this worth doing rather than leaving alone

- The icount tripwire is a required check and its baselines are committed. Anything that moves a
  measurement moves those baselines, and the work on milestone 519 (what this project costs, tracked where it cannot rot) has just made the cost of churn
  visible. Getting the classification right once is cheaper than discovering it per-benchmark.
- **`bench` has never run on a board**, so every number it produces is a QEMU number. Any benchmark
  that becomes a program becomes runnable on radon, argon and xenon the day it moves, which is a
  larger gain than the tidiness.
- DECISIONS §96 (process kernel or event kernel) rests on exactly this distinction: what the
  kernel costs on a crossing, against what a workload experiences. A benchmark suite that cannot say
  which side of that line each of its numbers sits on is weaker evidence than it looks.

## What this must not become

**Not a rewrite of milestone 21.** Its numbers are published, its baselines are gated, and the
comparison against lmbench and `sel4bench` in milestone 25 (cross-OS performance comparison) depends
on them meaning what they have always meant. A benchmark that moves must say what it used to measure
and what it measures now, in the same place a reader meets the number.

## Index row

The `bench` kernel feature is a special boot nobody has checked needs to be one. Proposed: measure which benchmarks genuinely need kernel privilege and move the rest to ordinary programs.
