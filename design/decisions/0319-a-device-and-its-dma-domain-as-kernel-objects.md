---
status: PROPOSED
raised: 2026-11-10
---

# 319. A device and its DMA domain as kernel objects, so the PCI bus can leave the kernel

*(Section number provisional until the merge queue lands it. The title and every name below are
provisional.)*

PCI enumeration leaves the kernel, last of the extractions (ruled 2026-11-10 UTC; milestone 1064
(PCI enumeration leaves the kernel)). This section is what that needs from the kernel, written for
a later ruling. It designs nothing yet.

## The question

Today `kernel/src/pci.rs` holds the bus. It walks ECAM, places BARs, sets the command register's
bus-master bit, chooses MSI or INTx, and confines each device's requester id to an IOMMU domain
before it grants the device to a driver. A userspace bus server can parse configuration space as
well as the kernel can; what it must not get by holding it is DMA authority. Three powers sit in
configuration space:

- BARs say where a device's registers appear in physical memory. A server that moves a BAR over
  RAM or over another device's window aims the CPU's accesses there.
- Bus mastering turns a device's DMA on. Before the kernel has bound the device to a domain, a
  device with bus mastering on reaches whatever the IOMMU's default allows.
- MSI and MSI-X targets are memory writes to a special address. DMA remapping does not confine
  them; interrupt remapping does, and milestone 317 (the interrupt-remapping flags) makes that
  askable without turning it on.

So the question is which kernel objects let a userspace server hold configuration space while the
kernel keeps the binding between a device and its DMA domain, as §4 (kernel shape) requires of a
confinement argument.

One fact makes part of this cheap. ECAM gives every function its own 4 KiB page of configuration
space (`kernel/src/pci.rs`'s `BUS_BYTES`: 32 devices of 8 functions of 4 KiB per bus), so a
function's configuration space is already grantable as a `DeviceFrame` page. The page is the easy
half. The hard half is that writing that page sets the three powers above.

## The options

1. A device object. The kernel mints one per function it discovers from ACPI or the device
   tree, keyed by requester id. Its methods bind the function to a DMA domain, grant its
   configuration page and its BARs, and refuse to set bus mastering until the function has a domain. The bus
   server holds every device object and hands each one to a driver. The kernel parses enough of
   configuration space to refuse a BAR placed outside the windows the firmware named.
2. A DMA-domain object separate from the device object. The domain holds the IOMMU page table
   and its holder makes it from memory like any other object. A device object names a requester id,
   and an invoke on the device binds it to a domain. One domain can then serve several devices,
   such as a multi-function card, and revoking the domain stops every device bound to it.
3. The bus server programs the IOMMU itself. The kernel grants the IOMMU's registers to a
   userspace platform server. This takes the confinement argument out of the kernel, against §4. It
   appears here because the prior art below has it.
4. The kernel keeps enumeration. The status quo, which the 2026-11-10 ruling refused, listed
   for completeness.

This section makes no recommendation. A new object kind is close to a one-way door (§10 (process model:
capability-based, microkernel), §16 (object revocation)), and each option fixes what every future
PCI driver depends on.

## Prior art, recalled, not re-read

- seL4 on x86 has an `IOSpace` capability per PCI requester id and `IOPageTable` objects the
  root task maps into it; the kernel walks the DMAR at boot. That is close to option 2, with
  configuration space left to userspace.
- Fuchsia (Zircon) splits an `IOMMU` object from a `BTI` (bus transaction initiator) object made
  from it per requester id, and pins memory through the BTI. The PCI bus driver runs in userspace.
  That is option 2.
- Genode's platform driver runs in userspace, owns configuration space, and in recent releases
  programs the IOMMU itself. That is option 3.
- Linux VFIO confines by IOMMU group, the smallest set of devices the IOMMU can tell apart, and
  virtualizes configuration-space writes to the BARs and MSI fields. It refuses a device to
  userspace without interrupt remapping unless an administrator sets `allow_unsafe_interrupts`.
- Redox runs `pcid` in userspace with no IOMMU confinement, so Redox trusts its drivers.

Where nife differs: nife already has domains in the kernel (`iommu::confine`) on all three
architectures, so options 1 and 2 add an object and a binding rather than new hardware support.

## A concrete example

Suppose e1000e is function 00:02.0 on QEMU's `q35`. Under option 2 the bus server holds the device object
for requester id 0x0010. The control process of milestone 1062 (the device control planes leave the
kernel) receives it, makes a domain from its own memory budget, binds the device to it, and only
then may set bus mastering. The device object's grant refuses a BAR the server moved over RAM.

## What waits on the answer

Milestone 1064 (PCI enumeration leaves the kernel), and with it the last 500 or so lines of
`kernel/src/pci.rs` and the kernel's link to `crates/pci`.

## Questions for the ruling

1. A device object (1), a device and a domain object (2), or neither.
2. Whether the kernel checks BAR placement against firmware windows, or trusts the bus server with
   placement inside them.
3. Whether interrupt remapping is a prerequisite milestone or part of milestone 1064.
