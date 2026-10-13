---
status: REFUSED
raised: 2026-09-20
refused_by: 226, 448
---
# 476. The `qemu-bounded.sh` self-test, in a gate

Refused by milestone 226 (design/roadmap/0226-qemu-bounded-orphans.md), which records it in its
Follow-on. Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names
work a number, a status and a condition that would change it.

## The refusal, in its own words

From '226. `qemu-bounded.sh` leaves an emulator behind, and the next run blames the wrong thing',
under `## Follow-on`:

> The self-test is in no gate, on purpose. It starts real emulators and costs about a minute
> against every lane, for a script that changes twice a year, which is a poor trade. That is rung
> two declined deliberately, and the block says plainly that it is a foot gun: a later
> simplification of `qemu-bounded.sh` will not be caught by CI.
>
> -- design/roadmap/0226-qemu-bounded-orphans.md

## Why it is here rather than only there

The self-test starts real emulators and costs about a minute, against a script that changes twice a
year. It is in no gate on purpose, and the block calls that what it is: rung two declined
deliberately, and a foot gun, because a later simplification of `helpers/qemu-bounded.sh` will not
be caught by CI.

## Revisit

- **Condition.** A cadence that can absorb about a minute of real emulators, which is the same
  condition milestone 473 (design/roadmap/0473-a-soak-leg-in-script-test.md) states for the same
  reason. A scheduled workflow is the obvious home for both, and neither block reached for it.

## Index row

A self-test that exists and runs nowhere, recorded by its own block as a foot gun rather than as a
design. The condition is a place to run it, and it is the second refusal in this backfill whose
whole obstacle is that the only cadence considered was the per-push gate.
