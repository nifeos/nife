---
status: REFUSED
raised: 2026-09-20
refused_by: 18, 448
---
# 466. Verus, and unbounded proof

Refused by milestone 18 (design/roadmap/0018-verify-capability-core.md), which records it in its
Follow-on. Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names
work a number, a status and a condition that would change it.

## The refusal, in its own words

From '18. Verify the capability core, then spread inward', under `## Follow-on`:

> Verus, and unbounded proof generally. Bounded model checking on Rust was taken deliberately as
> the tractable path against seL4's Isabelle/HOL refinement, and the block keeps Verus as
> something to revisit only if a specific property needs a loop invariant rather than as work
> anybody owes.
>
> -- design/roadmap/0018-verify-capability-core.md

## Why it is here rather than only there

The verification story here is bounded model checking on Rust, taken deliberately as the tractable
path against seL4's Isabelle/HOL refinement proof. Verus is the other available shape: an SMT-backed
verifier that can carry loop invariants and prove properties over unbounded inputs, at the cost of
writing those invariants.

## Revisit

- **Condition.** A specific property that needs a loop invariant, which is the refusal's own
  wording. It is explicitly not work anybody owes: the block keeps Verus as something to revisit when
  a property demands it rather than as a plan, and nothing in the proof suite has demanded one yet.

## Index row

The choice of bounded model checking over unbounded proof is the single largest decision in this
tree's verification story, and it was recorded as a bullet in a block. The condition is a property
rather than a preference, which is the right shape: bounded checking fails at a specific place and
that place is the trigger.
