---
status: REFUSED
raised: 2026-10-24
refused_by: 188
---
# 951. A hand-written IPC fastpath: a second path through `CALL` and reply that skips the scheduler

Refused by milestone 188 (design/roadmap/0188-ipc-fastpath.md), whose phase 4 this was. calef closed
188 on 2026-10-24 (UTC): *"Close 188, refuse phase 4"*. Milestone 448
(design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names work a number, a status
and a condition that would change it. The number was assigned by the integrator of that day.

## The refusal, in its own words

From '188. The IPC fastpath: the gate measures a shape userspace does not use, and three cheaper
cuts come before a hand-written path', under its phase 4 section:

> Do not start phase 4 on footprint arithmetic alone. Start it when a layout control shows a
> latency difference that footprint explains. Then build option 1 first, aimed at that 34%. If no
> such difference appears, this milestone closes saying phase 4 was not worth its permanent
> verification cost.
>
> -- design/roadmap/0188-ipc-fastpath.md

## Why it is refused rather than held

The trigger 188 named can no longer fire. Milestone 370 (a layout control for the perturbation
experiments) built the control, and E3 was to run the padding as a dose under it. On 2026-10-20
(UTC) step 0 read `the sled precedes 0 of 16 hot symbols` in all eight images: milestone 796 pins
the hot trap path in `.text.hot`, ahead of the padding, so no pad or shift moves a hot instruction.
calef retired E3's question that day (notes/footprint-perturbation.md). A held phase whose only
trigger is gone is a wait on nothing, so 188 closed the way it said it would.

What the refusal declines is a standing cost, not a single build. A second path through `ipc_call`,
`ipc_receive_cap` and `ipc_reply` has to hold the five invariants 188 lists. Either it calls the
proved `Rendezvous` methods and skips only the scheduler (188's option 1), or it replicates the
decision and needs its own harnesses and an equivalence proof. Both are verification work for as
long as the kernel exists, and nothing measured shows the latency they would buy.

§95 (a hand-written IPC fastpath, and whether it can stay proven) is unchanged: it ratified the
eligibility predicate as buildable and kept the fastpath itself gated on a measurement. This
refusal is that gate, carried with a status. The figures and the case for and against are in
188's appendix, [phase-4-evidence.md](0188-ipc-fastpath/phase-4-evidence.md).

## Revisit

- **Condition.** A measured round-trip latency gap that the scheduler bypass explains. For instance,
  an executed-footprint measurement from the PMU (E5, milestone 708 (an executed footprint ladder
  for radon)) could show it. It would have to show that the scheduler's re-selection costs a round
  trip time a fastpath would recover. That is `schedule`, `finish_switch`, `wake` and `switch_to`,
  34% of aarch64's CALL/reply closure after phase 3. Static footprint arithmetic alone does not meet it, which is 188's own
  rule. If it is met, build 188's option 1 first.

## Index row

The hand-written IPC fastpath milestone 188 held as its phase 4. It was refused on 2026-10-24,
when milestone 796's pin left its trigger (a layout control showing a latency difference footprint
explains) unable to fire. It reopens on a measured round-trip gap the scheduler bypass explains.
