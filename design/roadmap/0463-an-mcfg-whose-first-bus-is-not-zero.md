---
status: REFUSED
raised: 2026-09-20
refused_by: 165, 448
---
# 463. An MCFG whose first bus is not zero

Refused by milestone 165 (design/roadmap/0165-x86-64-pci-acpi-mcfg.md), which records it in its
Follow-on. Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names
work a number, a status and a condition that would change it.

## The refusal, in its own words

From "165. x86_64 PCI enumeration: wire `kernel/src/pci.rs` to ACPI's MCFG", under `## Follow-on`:

> Adjusting for an MCFG whose first bus is not 0. `kernel/src/pci.rs` addresses a function with an
> absolute bus number, and subtracting `lo << 20` names a base below the window
> `mmu::map_everything` maps, turning config reads into reads of whatever sits underneath it.
> Every machine seen reports 0 and none is required to, so it is checked and refused rather than
> fixed up.
>
> -- design/roadmap/0165-x86-64-pci-acpi-mcfg.md

## Why it is here rather than only there

`kernel/src/pci.rs` addresses a function with an absolute bus number against the ECAM window the
MCFG describes. If a machine reports a first bus other than 0, that arithmetic is wrong by `lo <<
20`, and the failure is not a refusal to enumerate: it is config reads landing on whatever the
kernel mapped underneath the window. So the value is checked and the kernel refuses rather than
adjusting.

## Revisit

- **Condition.** A machine reporting a non-zero first bus in its MCFG. Every machine seen so far
  reports 0 and none is required to, which is the refusal's own wording, and because the value is
  checked rather than assumed the bell rings as a loud bring-up failure rather than as a wrong answer.

## Index row

The fix-up is four lines and was refused anyway, because getting it wrong turns PCI configuration
reads into reads of unrelated memory. The check is in place, so this is a refusal that announces
itself the first time a machine disagrees, which is the rare case where the condition enforces
itself.
