# x86_64 PCI enumeration: the appendices to milestone 165

[Milestone 165 (x86_64 PCI enumeration)](../0165-x86-64-pci-acpi-mcfg.md) carries the gap it
closed, what was built, what the 2026-09-02 pass corrected, the two decisions it made, what is
proven where, what is owed and its follow-on. This directory carries what its prose budget has no
room for.

- [`not-built.md`](not-built.md): the runner the block left alone and the NVMe attach it did not
  make.
- [`decision-86.md`](decision-86.md): which half of §86 (whether an NVMe driver can leave the
  kernel)'s evidence the work reaches.
- [`acpi-walk-bound.md`](acpi-walk-bound.md): the ACPI walk that refused every table on a machine
  with real RAM, and its gate.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default,
`design/roadmap/X/` beside `design/roadmap/X.md`. calef names directories.*

- `not-built` Name: provisional. After the parent's "What was deliberately not built". calef names
  files.
- `decision-86` Name: provisional. After the parent's "What this does and does not establish for
  DECISIONS §86". calef names files.
- `acpi-walk-bound` Name: provisional. After the parent's "The ACPI walk refused every table on a
  machine with real RAM". calef names files.
