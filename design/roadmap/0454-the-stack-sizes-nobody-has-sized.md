---
status: REFUSED
raised: 2026-09-20
refused_by: 90, 124, 448
---
# 454. The two stack sizes this tree has parked rather than measured

Refused by milestone 124 (design/roadmap/0124-a-thread-is-born-where-it-lives.md) and milestone 90
(design/roadmap/0090-secondary-stack-guard.md), each of which records it in its Follow-on. Milestone
448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names work a number, a
status and a condition that would change it.

## The refusal, in its own words

From "124. A thread is born where it lives: the spawn path's copies", under `## Follow-on`:

> Raising `STACK_PAGES` from 4 to 8 as the fix. The guard page stays one page however large the
> stack is, so a frame bigger than 4096 bytes can still step over it in a single move and the
> overflow stays illegible. Growing the stack moves the fault further away without restoring the
> mechanism that makes it a fault at all; shrinking the frames below 4096 restores it. Growing the
> stack remains an independent question on its own merits.
>
> -- design/roadmap/0124-a-thread-is-born-where-it-lives.md

From '90. A guard page under the per-CPU secondary stacks', under `## Follow-on`:

> Shrinking the 64 KiB per-CPU secondary stacks, which the region move made tempting to fold in.
> The scope note refused it in advance and the measurement backs the refusal: milestone 84 puts
> the secondaries at 12% of their stacks (8.5 KiB of 64), so the missing guard was the finding and
> the sizing was not. Taking both at once would have made a stack-depth regression and a guard
> regression indistinguishable in one commit.
>
> -- design/roadmap/0090-secondary-stack-guard.md

## Why it is here rather than only there

Two blocks refused a stack-size change in the same month, in opposite directions, and both were
right to: one refused growing `STACK_PAGES` from 4 to 8 because growing the stack moves an illegible
overflow further away instead of restoring the guard page that would make it a fault, and the other
refused shrinking the 64 KiB per-CPU secondary stacks because a stack-depth regression and a guard
regression in one commit are indistinguishable. What neither did was set either number from
evidence, and the evidence that exists points both ways: milestone 84 (stack high-water: measure kernel stack depth) put the secondaries at 12% of their
stacks, 8.5 KiB of 64.

## Revisit

- **Condition.** A measurement that says a size is wrong, rather than a change that makes a bug
  harder to see. Milestone 124's block says it plainly: growing the stack remains an independent
  question on its own merits, and the frames have to come below 4096 bytes for the guard page to be a
  mechanism at all.

## Index row

Two refusals a month apart both declined to change a stack size, correctly, and neither set the
number from evidence. They are gathered here because they are one question wearing two faces, and
because the measurement that would settle either has already been taken once for the secondaries and
never for the thread stacks.
