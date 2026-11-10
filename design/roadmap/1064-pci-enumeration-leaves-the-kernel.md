---
status: NOT-STARTED
raised: 2026-11-10
milestone_dependencies: 317, 1062, 1068
decision_dependencies: 319
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1064. PCI enumeration leaves the kernel

*(The number, title and slug are provisional.)*

Reuse: `crates/pci` already holds the decode logic and moves with the server. The prior art for
the kernel objects is in §319.

Ruled 2026-11-10 (UTC): PCI enumeration leaves the kernel, last of the extractions. It comes after
milestone 317 (the interrupt-remapping flags, and where MSI confinement actually lives) and
milestone 1062 (the device control planes leave the kernel). Parsing configuration space is parsing
what a device says, which milestone 242 (xHCI) calls the last thing to do at EL1. But whoever holds
configuration space sets BARs, bus mastering and MSI targets, which is DMA and interrupt authority.
So a userspace bus server needs kernel objects that bind each device to its DMA domain first.
§319 (a device and its DMA domain as kernel objects) is that rule: a device object that names a
requester id, a separate DMA-domain object it binds to, and no bus mastering until the device has a
domain. The kernel trusts the bus server to place BARs and write configuration space, and checks no
placement. MSI confinement is milestone 1068 (the kernel turns interrupt remapping on where an
IOMMU offers it), which this milestone waits on.

## Today

`kernel/src/pci.rs` (691 code lines) walks ECAM, places BARs, chooses MSI or INTx and brings
virtio-pci up; `crates/pci` (1,050 code lines) holds the decode logic. NVMe, e1000e, xHCI and
virtio-pci all reach their devices through it.

## What gates it

- §319, which this milestone builds: the device object, the DMA-domain object, the binding, and the
  refusal of bus mastering until a device has a domain.
- Milestone 1068 (the kernel turns interrupt remapping on where an IOMMU offers it). A userspace
  bus server that writes MSI targets needs remapping on wherever an IOMMU exists. A machine
  without an IOMMU runs trusted drivers, and there neither milestone confines anything.

## Index row

The kernel walks the PCI bus, places BARs and programs MSI. A bus server in userspace can do all of
that once the kernel has an object that ties each device to its IOMMU domain, so the server holds
configuration space without holding DMA authority. This builds §319's device and DMA-domain
objects and moves the bus to a userspace server, after milestone 1068 turns interrupt remapping on.
