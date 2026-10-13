---
status: SUPERSEDED
raised: 2026-09-20
superseded_by: 835, 836, 837, 838
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 478. Tier three: full POSIX behind the foreign-language seam

The work is milestones 835 to 838 (a C library in four stages: files, clock and memory; threads;
sockets; `posix_spawn`). The benchmark ruling (PR #1854) requires the field's standard benchmarks
to run unmodified, and the nine programs it names as milestones 826 to 834 are POSIX C, which no
narrower tier may adapt. §265 (a C library started from relibc) records calef's choice of relibc and
amends the first rule of §31 (the foreign-language seam). Full POSIX includes `fork`, which the fork
ruling (PR #1856) declines permanently, so "tier three" is three stages and a spawn, with no fork in
any of them. The refusal is kept below because it is the argument the stages answer.

Refused by milestone 36 (design/roadmap/0036-foreign-component.md), which records it in its
Follow-on. Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names
work a number, a status and a condition that would change it.

## The refusal, in its own words

From '36. A foreign-language component, seam first (spike; feeds 29 and 23)', under `## Follow-on`:

> Tier three, full POSIX (`open`, `fork`, `socket`, threads), stays out. It needs a real libc
> port, which DECISIONS §15 prices at "later, if ever", and a component that wants it is a
> different and much larger project than this one.
>
> -- design/roadmap/0036-foreign-component.md

## Why it is here rather than only there

The foreign-language seam has tiers, and the third is `open`, `fork`, `socket` and threads: enough
POSIX that arbitrary C could be ported rather than adapted. It needs a real libc port, which §15 (the native ABI: formalize the convention) prices at "later, if ever", and a component that wanted it would be a different and
much larger project than this one.

## The condition it was refused under, now met

- **Condition.** A component somebody needs that cannot be adapted to the narrower tiers. Nothing on
  the roadmap has asked, and the refusal's phrasing is a pricing rather than a prohibition, which is
  why this is recorded as refused rather than as impossible.
- **Met, 2026-10-08 (UTC).** The benchmark ruling's nine standard benchmarks (milestones 826 to 834,
  on PR #1854) are the component, and they cannot be adapted by rule. See the paragraph at the top.

## Index row

The largest single thing this tree had declined to build, refused for want of a component that
would need it. The standard benchmarks are that component, and the work is a C library started from
relibc, staged by consumer and without fork.

