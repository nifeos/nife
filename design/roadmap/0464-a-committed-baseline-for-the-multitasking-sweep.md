---
status: REFUSED
raised: 2026-09-20
refused_by: 168, 448
---
# 464. A committed baseline and a `--check` for the multitasking sweep

Refused by milestone 168 (design/roadmap/0168-multitasking-benchmark.md), which records it in its
Follow-on. Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names
work a number, a status and a condition that would change it.

## The refusal, in its own words

From '168. A multi-tasking workload benchmark: the number that would decide the event-kernel
question', under `## Follow-on`:

> A committed baseline and a `--check` for this sweep, the way `script/bench` gates against
> `bench/baseline-*.txt`. Refused because the icount instrument's determinism is what makes that
> gate meaningful, and a workload whose entire subject is scheduling under contention is not
> deterministic on any accelerator this tree has. A gate here would be asserting a tolerance
> nobody has measured, which is how `script/lint` has already lost three checks.
>
> -- design/roadmap/0168-multitasking-benchmark.md

## Why it is here rather than only there

`script/bench` gates against committed baselines because the icount instrument is deterministic: the
same build executes the same number of instructions, so a difference is a change rather than noise.
The multitasking sweep's subject is scheduling under contention, which is not deterministic on any
accelerator this tree has, so a committed baseline would be asserting a tolerance nobody has
measured.

## Revisit

- **Condition.** A deterministic instrument for a contended workload, or a measured variance to set
  a band from. The refusal names the failure mode precisely: a gate asserting an unmeasured tolerance
  is how `script/lint` has already lost three checks.

## Index row

A benchmark without a gate looks like an oversight and is a deliberate refusal here, because the one
instrument that makes a gate meaningful cannot run the workload being gated. The condition is an
instrument rather than an argument, and until one exists the sweep reports without asserting.
