---
status: REFUSED
raised: 2026-09-20
refused_by: 214, 448
---
# 471. A `#[test_case]` that returns `Result<(), Skipped>`

Refused by milestone 214 (design/roadmap/0214-print-and-return-skips.md), which records it in its
Follow-on. Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names
work a number, a status and a condition that would change it.

## The refusal, in its own words

From '214. A test that prints "skipping" and returns is counted as passed', under `## Follow-on`:

> Rung one, a `#[test_case]` returning `Result<(), Skipped>` so `?` carries an absent fixture out
> of a helper and into the runner. It is the better mechanism on paper and it would have caught
> the helper cases structurally, and it is a return-type change on every `#[test_case]` in the
> tree plus an `Ok(())` on the end of each, for a defect the console check catches at the moment
> it happens. If the helper shape recurs, that is the argument for paying for it.
>
> -- design/roadmap/0214-print-and-return-skips.md

## Why it is here rather than only there

This is rung one of AGENTS.md's ladder and the block says so: a return type that carries an absent
fixture out of a helper with `?` makes the print-and-pass defect structurally impossible, where the
console check that shipped instead catches it at the moment it happens. It costs a return type
change on every `#[test_case]` in the tree plus an `Ok(())` on the end of each.

## Revisit

- **Condition.** Stated in the refusal: "If the helper shape recurs, that is the argument for paying
  for it." The console check catches the direct case, so what would justify the higher rung is a
  second instance of the case it cannot catch structurally.

## Index row

A refusal of the stronger mechanism in favor of the cheaper one, made explicitly and with the trade
priced: rung one costs an edit to every test in the tree, rung two catches the same defect at the
moment it happens. The condition is a recurrence, which is the honest trigger for buying a mechanism
you declined once.
