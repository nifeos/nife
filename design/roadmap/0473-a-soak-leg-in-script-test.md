---
status: REFUSED
raised: 2026-09-20
refused_by: 219, 448
---
# 473. A soak leg inside `script/test`

Refused by milestone 219 (design/roadmap/0219-a-workload-that-does-not-stop.md), which records it in
its Follow-on. Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that
names work a number, a status and a condition that would change it.

## The refusal, in its own words

From '219. The boot tour ends and the kernel halts, so there is nothing to soak', under `##
Follow-on`:

> A soak leg inside `script/test`. Twenty seconds per architecture would stop the feature silently
> ceasing to compile, and it was judged too expensive for a gate every lane runs on every push.
>
> -- design/roadmap/0219-a-workload-that-does-not-stop.md

## Why it is here rather than only there

Twenty seconds per architecture would stop the soak workload silently ceasing to compile, which is
the failure it guards. It was judged too expensive for a gate every lane runs on every push, and
that judgment is about where the check runs rather than about whether it is worth running.

## Revisit

- **Condition.** A cadence that can absorb a minute of emulator time. The refusal prices the check
  against the per-push gate specifically, so a scheduled workflow is where the same twenty seconds per
  architecture would cost nothing anybody is waiting on.

## Index row

A cheap check refused because of where it would have to run, not because of what it costs in the
abstract. The condition is a home rather than a redesign, which makes this one of the easiest
refusals in the backfill to reverse.
