---
status: REFUSED
raised: 2026-09-20
refused_by: 249, 448
---
# 477. A power-cycled boot series over radon's smart plug

Refused by milestone 249
(design/roadmap/0249-the-boot-lottery-is-sampled-by-a-person-walking-to-the-board.md), which records
it in its Follow-on. Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal
that names work a number, a status and a condition that would change it.

## The refusal, in its own words

From '249. The boot lottery is sampled by a person walking to the board, so nine draws is a whole
evening', under `## Follow-on`:

> A power-cycled series over radon's smart plug, as an alternative to SRST. It is the better
> experiment (a power cycle is what the nine control boots were) and it is a lane spent on a guess
> until the firmware has actually refused reset type 1. notes/soak.md's outcome table is where
> that finding would arrive; raise it then.
>
> -- design/roadmap/0249-the-boot-lottery-is-sampled-by-a-person-walking-to-the-board.md

## Why it is here rather than only there

A power cycle is what the nine control boots were, so a power-cycled series is the better experiment
than one driven by SRST. It was refused because it is a lane spent on a guess: the hypothesis it
would test is that the firmware refuses reset type 1, and nobody has seen it do that.

## Revisit

- **Condition.** Stated in the refusal, including where the evidence would arrive: "a lane spent on
  a guess until the firmware has actually refused reset type 1. `notes/soak.md`'s outcome table is
  where that finding would arrive; raise it then."

## Index row

The better experiment, refused because the observation that would justify it has not been made and
the board time is real. Its condition names the file the triggering observation would land in, which
is the most checkable shape a revisit condition takes in this tree.
