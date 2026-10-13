---
status: REFUSED
raised: 2026-09-20
refused_by: 161, 215, 448
---
# 459. Legacy INTx interrupt routing on x86_64

Refused by milestone 161 (design/roadmap/0161-x86-64-kernel-port.md) and milestone 215
(design/roadmap/0215-x86-64-pci-interrupt-routing.md), each of which records it in its Follow-on.
Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names work a
number, a status and a condition that would change it.

## The refusal, in its own words

From "161. The x86_64 kernel port: bring up the HAL's third architecture", under `## Follow-on`:

> Item 2's other half, PCI interrupt routing over INTx, was deliberately not taken by milestone
> 215: ACPI's `_PRT` is AML and this tree will not grow an interpreter, and hardcoding q35's
> swizzle would pass every gate here and might still fail on xenon.
>
> -- design/roadmap/0161-x86-64-kernel-port.md

From "215. A PCI function's interrupt reaches nothing on x86_64, so no userspace driver can run
there", under `## Follow-on`:

> Legacy INTx routing, in both available versions. Reading ACPI's `_PRT` means an AML interpreter
> this tree does not have and would then have to maintain and verify, for four numbers. Hardcoding
> `q35`'s swizzle passes every gate on patagonia and might fail on the OptiPlex, which would be
> discovered at a null modem, this project's most expensive place to discover anything.
>
> -- design/roadmap/0215-x86-64-pci-interrupt-routing.md

## Why it is here rather than only there

x86_64 PCI interrupts reach userspace over MSI or MSI-X. INTx, the legacy pin-based path, needs to
know which global interrupt a function's pin lands on, and there are exactly two ways to learn it:
read ACPI's `_PRT`, which is AML and would mean an interpreter this tree would then have to maintain
and verify for four numbers, or hardcode `q35`'s swizzle, which passes every gate on patagonia and
might fail on xenon. Two blocks refused it with the same reasoning: milestone 161 (the x86_64 kernel port) deferred it, and milestone 215 (a PCI function's interrupt reaches nothing on x86_64) refused it outright.

## Revisit

- **Condition.** A PCI function on a machine this tree runs that offers neither MSI nor MSI-X, which
  is the only case INTx serves. The refusals are explicit about where that would be discovered: xenon,
  at a null modem, which milestone 215's block calls this project's most expensive place to discover
  anything.

## Index row

Both available routes to INTx lose for reasons that have nothing to do with effort: one grows an AML
interpreter, the other guesses a swizzle that would fail on the one machine nobody can watch. Two
blocks reached the same refusal independently, and this gives it the single home that would have
made the second one unnecessary.
