---
status: REFUSED
raised: 2026-09-20
refused_by: 14, 448
---
# 457. Sub-page packing for kernel objects

Refused by milestone 14 (design/roadmap/0014-kernel-objects-from-untyped.md), which records it in
its Follow-on. Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that
names work a number, a status and a condition that would change it.

## The refusal, in its own words

From '14. Kernel objects from untyped: remove the kernel heap', under `## Follow-on`:

> Sub-page object packing, meaning a decision about whether several TCBs share a page. Object
> sizes were not measured when the milestone ran, and a packing commitment made ahead of the
> measurement is a guess that later code would be built on.
>
> -- design/roadmap/0014-kernel-objects-from-untyped.md

## Why it is here rather than only there

Every kernel object retyped from untyped memory takes at least a page today. Whether several thread
control blocks should share one is a real question about memory efficiency, and it is a commitment:
once two objects share a page, revocation, reclamation and alignment all inherit the choice.

## Revisit

- **Condition.** Object sizes measured. The refusal is that a packing commitment made ahead of the
  measurement is a guess that later code would be built on, so the measurement is the whole gate and
  it is cheap to take.

## Index row

Packing several kernel objects into a page is a commitment the rest of the object model inherits,
and it was refused because the sizes it would be based on had never been measured. The condition is
a measurement rather than a design argument, which makes this one of the cheaper refusals in the
tree to reopen.
