---
status: NOT-STARTED
raised: 2026-08-25
milestone_dependencies: none
decision_dependencies: none
machine_requirements: JH7110 silicon
specific_machine: none
needs_person: yes
---
# 163. The JH7110's PCIe root complex: a real driver for the PLDA XpressRICH controller

Three places name this as needing "its own milestone": `design/roadmap/0053-board-peripherals.md`
("driving it is its own milestone, not a bench fix"), `notes/visionfive2.md`'s own "PCIe" section
(identical words), and `design/decisions/0086-el0-nvme-driver.md`, which holds itself `PROPOSED`
pending exactly this work. This block gives that debt a home.

In milestone 159's sense: the board is on the desk and this needs hands on it.
Bringing up a real PCIe root complex means flashing, a serial console, and power-cycling a board
that will wedge, none of which a background lane can do (milestone 53's own words, said generally
about the NVMe/network work this is one piece of).

## What it needs

Drive the JH7110's PCIe root complex, a PLDA XpressRICH controller
(`starfive,jh7110-pcie` in mainline device trees). This is not the same device as QEMU's
`pci-host-ecam-generic`, which is explicitly a QEMU-only fake exposed by the `virt` boards
(`design/roadmap/0016-real-hardware-iommu.md`'s own framing of the two IOMMUs as "structural
siblings" only holds for the emulated case; the PLDA controller has no such sibling in this tree
today). `notes/visionfive2.md` records the honest current state: the kernel already reads PCIe
windows from a generic-ECAM device-tree node when one exists (`memory::init`, `mmu::map_everything`,
every probe in `kernel/src/pci.rs`), but on this board there is no such node, so nothing PCIe is
mapped or touched. A real driver for the PLDA controller is what would give that existing,
already-portable `kernel/src/pci.rs` probe path something real to find.

Getting this working is what would let §18's already-built PCIe transport layer reach real
devices behind it on this board: concretely, the real M.2 NVMe slot (milestone 53's storage half)
and any real PCIe-attached NIC, replacing the QEMU-only fake device with a real one.

## Why it matters, and what it unblocks

**DECISIONS §86** (whether an NVMe driver can leave the kernel, and what capability would let it)
holds itself `PROPOSED` specifically pending this: its own recommendation is "Option 2, but not
yet... hold this PROPOSED until the board-side work (the JH7110's PLDA XpressRICH root complex)
exists, for the same reason §23 built multi-queue confinement only after single-queue worked: the
second data point tells you which parts of the contract are QEMU artifacts." This milestone is
that board-side work.

Milestone 53 also names why NVMe-over-PCIe was chosen over SD/eMMC as the storage path in the
first place, partly on this driver's account: "a real PCIe root-complex driver compounds into
milestone 87's x86 machine where an MSHC driver serves one slot on one board."

**A second path to §86's specific question may exist, and it is not this one.** Milestone 87's
x86_64 machine (a Dell OptiPlex 7050 Micro, already on calef's desk) also has real NVMe hardware,
and milestone 161's work already landed real ACPI/MCFG table parsing against it. A separate
scoping lane, launched the same day this milestone was minted, is investigating whether x86 can
reach a "real hardware NVMe behind a real IOMMU" data point for §86 faster and cheaper than this
milestone will: x86's PCIe discovery mechanism is ACPI-described and architecturally identical
between QEMU's `q35` machine and real x86 hardware, unlike RISC-V's QEMU-only fake ECAM device, so
proving PCI enumeration correct under QEMU there is much stronger evidence than the equivalent
would be here. That does **not** make this milestone unnecessary: milestone 53's actual
storage-and-network completion on the VisionFive 2 still needs a real PLDA driver regardless of
what x86 provides for §86 specifically. But the two paths' relative priority should be read against
that scoping lane's finding, whichever way it goes, before this milestone is sequenced against
other hardware-gated work.

## What is needed

Hands-on work with the physical board: flashing, a serial console, and iterative bring-up against
real silicon, the same shape as milestone 159's "Gate: HARDWARE" framing. Not a background-lane
task. The driver's own logic (register layout, initialization sequence) can likely be transcribed
from a mainline Linux driver for the same controller the way `crates/jh7110_entropy` transcribed the
JH7110 TRNG's from `drivers/char/hw_random/jh7110-trng.c`, and host-tested before any hardware is
involved, but whether it actually enumerates real devices can only be verified on the board.

## Radon's storage depends on this, 2026-10-07

§260 (radon's block server serves a USB drive) puts this milestone on radon's storage path. The USB 3
ports sit behind a VL805 on PCIe. Until this root complex enumerates it, the booted system cannot
read a disk.

## Index row

Named as needing "its own milestone" in three places (milestone 53, notes/visionfive2.md,
DECISIONS §86) without ever getting one until now. Gate: HARDWARE. What DECISIONS §86 holds itself
PROPOSED pending: a real second data point beyond QEMU's fake ECAM device, so it can tell which
parts of the NVMe capability contract are QEMU artifacts.
