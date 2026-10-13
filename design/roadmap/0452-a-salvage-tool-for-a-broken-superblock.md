---
status: REFUSED
raised: 2026-09-20
refused_by: 110, 448
---
# 452. A salvage tool for a volume whose headers are all bad

Refused by milestone 110 (design/roadmap/0110-recovery-from-a-partition.md), which records it in its
Follow-on. Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names
work a number, a status and a condition that would change it.

## The refusal, in its own words

From '110. The recovery tool takes a device and a partition', under `## Follow-on`:

> Repair. If no header in the ring is valid the tool says so and stops. A format-aware salvage
> tool is a real thing to want and is a different program with a different risk profile: this one
> is read-only by design, and a salvager that guesses at a broken superblock is the opposite of
> that.
>
> -- design/roadmap/0110-recovery-from-a-partition.md

## Why it is here rather than only there

`tools/redoxfs_host` is read-only on a device by design, and when no header in the ring is valid it
says so and stops. A salvager would instead guess: reconstruct a plausible superblock, walk what it
finds, and hand back files it cannot prove are files. That is a different program with a different
risk profile, pointed at somebody's only copy.

## Revisit

- **Unstated.** The refusal names no trigger, and none can honestly be inferred from it. What would
  price a salvager against the read-only tool is a real broken volume somebody needs back, and the
  refusal does not say that, so this reads as a gap rather than as a settled matter. Anyone who hits
  that case should write the condition here before writing the tool.

## Index row

A format-aware salvager is a real thing to want and is the opposite of what the recovery tool is:
one reports and stops, the other guesses at a broken superblock. The refusal is on risk rather than
on effort, and it states no condition, which is recorded here as the gap it is.
