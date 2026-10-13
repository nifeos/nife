---
status: REFUSED
raised: 2026-09-20
refused_by: 197, 448
---
# 468. Proving `xtask`

Refused by milestone 197 (design/roadmap/0197-user-and-xtask-proofs.md), which records it in its
Follow-on. Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names
work a number, a status and a condition that would change it.

## The refusal, in its own words

From '197. `user/` and `xtask` are out of reach of the prover, for exactly the reason the kernel
was', under `## Follow-on`:

> Proving `xtask`. Its front door is already open, measured: `cargo kani -p xtask` compiles with
> no changes. The refusal is on value. A defect there cannot reach anything that runs, it is host
> code with tests and a debugger where Kani is least differentiated, its pure logic already lives
> in crates the suite proves, and its hand-written decoders exist to be a second opinion on those
> crates, so aiming one prover at both halves narrows the independence that justifies them. What
> would reverse it is `xtask` growing logic the target then trusts, and the shape to watch is the
> measured-boot digest.
>
> -- design/roadmap/0197-user-and-xtask-proofs.md

## Why it is here rather than only there

The mechanical obstacle does not exist: `cargo kani -p xtask` compiles with no changes, and the
block measured that. The refusal is on value. A defect in `xtask` cannot reach anything that runs,
it is host code with tests and a debugger where a prover is least differentiated, its pure logic
already lives in crates the suite proves, and its hand-written decoders exist to be a second opinion
on those crates, so proving both halves narrows the independence that justifies having two.

## Revisit

- **Condition.** Stated in the refusal: "What would reverse it is `xtask` growing logic the target
  then trusts, and the shape to watch is the measured-boot digest." That is a condition naming a
  specific piece of code, which makes it one of the few in this backfill that a reader can check by
  opening a file.

## Index row

A refusal on value rather than on tractability, with the tractability measured first so the two
could not be confused. Its condition names the exact shape that would reverse it, which is what a
revisit condition is supposed to look like.
