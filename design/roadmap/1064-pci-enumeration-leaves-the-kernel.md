---
status: NOT-STARTED
raised: 2026-11-10
milestone_dependencies: 317, 1062
decision_dependencies: 319
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1064. PCI enumeration leaves the kernel

*(The number, title and slug are provisional.)*

Ruled 2026-11-10 (UTC): PCI enumeration leaves the kernel, last of the extractions, after milestone
317 (the interrupt-remapping flags) and milestone 1062 (the device control planes leave the kernel).
Parsing configuration space is parsing what a device says, which milestone 242 (xHCI) calls the last
thing to do at EL1. But whoever holds configuration space sets BARs, bus mastering and MSI targets,
which is DMA and interrupt authority. So the kernel needs an object that binds a device to its DMA
domain before a userspace bus server can hold the bus. That object is
§319 (a device and its DMA domain as kernel objects), PROPOSED, and this milestone waits on its
ruling.

## Today

`kernel/src/pci.rs` (691 code lines) walks ECAM, places BARs, chooses MSI or INTx and brings
virtio-pci up; `crates/pci` (1,050 code lines) holds the decode logic. NVMe, e1000e, xHCI and
virtio-pci all reach their devices through it.

## What gates it

- §319's ruling, which this milestone builds.
- Interrupt confinement. Milestone 317 makes it askable on all three architectures and confines no
  interrupt. A userspace bus server that writes MSI targets needs remapping turned on, and no
  milestone builds that yet. Whether it is this milestone's first step or a milestone of its own is
  part of §319.

## Index row

The kernel walks the PCI bus, places BARs and programs MSI. A bus server in userspace can do all of
that once the kernel has an object that ties each device to its IOMMU domain, so the server holds
configuration space without holding DMA authority. This builds that once §319 is ruled.
