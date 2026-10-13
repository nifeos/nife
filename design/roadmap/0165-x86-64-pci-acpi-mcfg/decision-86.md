# What milestone 165 does and does not establish for §86: an appendix

An appendix to [milestone 165 (x86_64 PCI enumeration)](../0165-x86-64-pci-acpi-mcfg.md). It is the
block's reading of which half of [§86 (whether an NVMe driver can leave the
kernel)](../../decisions/0086-el0-nvme-driver.md)'s evidence this work reaches and which half it
cannot.

## What this does and does not establish for DECISIONS §86

What it establishes: the *discovery* half of what a real hardware data point needs. x86's PCIe
enumeration mechanism is ACPI's MCFG naming an ECAM window, a chipset register enabling its decode,
then ordinary config-space reads. It is now proven under QEMU using the same mechanism real x86
hardware uses. Nothing about the mechanism itself is QEMU-specific the way the device is on RISC-V.
Milestone 87's OptiPlex, once it boots this kernel, should need no code change here to also
enumerate its own real PCI bus. Only the values (its own MCFG base, its own BAR hole) will differ.
If real UEFI leaves ECAM already enabled, `enable_pcie_ecam`'s rewrite should be a no-op.

What it does not establish: §86 is about a *confined userspace NVMe driver*, and confinement is
exactly the piece this milestone cannot touch. x86 has no IOMMU driver at all (VT-d, item 6). So
there is no path from what is built here to "NVMe behind a real IOMMU" without first building VT-d.
That is a materially larger piece of work than this one: a DMAR table walk, translation table setup,
and fault handling, the same shape milestone 16b's SMMUv3/RISC-V IOMMU drivers took. It is out of this lane's
scope by the brief that started it.

The proposed JH7110 driver (see the note about it at the top of the parent page) has the same gap
in reverse shape. It would reach real NVMe hardware over a real (non-QEMU-fake) discovery
mechanism. But RISC-V's IOMMU driver already exists, from milestone 16b (real hardware and IOMMU-backed driver isolation). So its path
to a *confined* data point is shorter once its hardware bring-up lands.

Read plainly: x86 got the cheaper half of §86's evidence (discovery) at low cost. The JH7110 path
still owns the harder half (confinement over a real, non-fake root complex) for its own
architecture, and x86 needs VT-d built first to catch up on that half here. Neither path alone gives
§86 the full data point it is holding out for.
