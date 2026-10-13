---
status: REFUSED
raised: 2026-09-20
refused_by: 2, 448
---
# 449. The EL2 to EL1 drop the vectors were promised alongside

Refused by milestone 2 (design/roadmap/0002-exception-vectors.md), which records it in its
Follow-on. Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names
work a number, a status and a condition that would change it.

## The refusal, in its own words

From '2. Exception vectors, and a fault that tells you what it was', under `## Follow-on`:

> The EL2 to EL1 drop the original row promised alongside the vectors. QEMU's `virt` machine
> enters a flat Image at EL1, so there was nothing to drop from, and building the mechanism with
> no caller would have been guessing at what a later board needs.
>
> -- design/roadmap/0002-exception-vectors.md

## Why it is here rather than only there

The mechanism is the standard aarch64 entry sequence: a kernel that finds itself at EL2 sets
`HCR_EL2.RW`, points `ELR_EL2` at its own EL1 continuation, sets `SPSR_EL2` to EL1h with interrupts
masked, and `eret`s down. It is not hard and it is not written, because on the machine this tree
develops against there is nothing to drop from.

## Revisit

- **Condition.** An aarch64 machine whose firmware hands this kernel EL2. The refusal's whole
  argument is that QEMU's `virt` enters a flat Image at EL1, so the mechanism would have had no caller
  and its shape would have been a guess at what a later board needs. A board that arrives at EL2
  supplies both the caller and the shape.

## Index row

An aarch64 kernel that boots at EL2 has to drop itself to EL1 before it can use EL1's translation
regime at all, and this tree has never needed to because QEMU's `virt` machine hands a flat Image
straight to EL1. The refusal is sound and it is conditional on one machine's behavior, which is
exactly the kind of refusal that goes stale silently when a second machine turns up.
