---
status: DECIDED
raised: 2026-11-10
decided: 2026-11-10
ratified_by: calef
---

# 319. A device and its DMA domain as kernel objects, so the PCI bus can leave the kernel

*(The title and every name below are provisional.)*

PCI enumeration leaves the kernel, last of the extractions (milestone 1064 (PCI enumeration leaves
the kernel)). The kernel keeps two objects for it: a device object and a DMA-domain object. The
kernel trusts the bus server in userspace with configuration space. Confinement of DMA rests on the
domain object and the IOMMU, and confinement of interrupts on interrupt remapping, which is a
milestone of its own that 1064 waits on.

## The problem

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
  them; interrupt remapping does. Milestone 317 (the interrupt-remapping flags) makes that
  askable and turns none of it on.

ECAM gives every function its own 4 KiB page of configuration space (`kernel/src/pci.rs`'s
`BUS_BYTES`: 32 devices of 8 functions of 4 KiB per bus), so a function's configuration space is
already grantable as a `DeviceFrame` page.

## The rule

1. **A device object and a separate DMA-domain object.** The kernel mints a device object per
   function it discovers, and the object names that function's requester id. A DMA domain is an
   object of its own: it holds the IOMMU page table, and its holder makes it from memory like any
   other object. An invoke on a device object binds the device to a domain. The kernel refuses to
   set bus mastering on a device until it has a domain. One domain can serve several devices, such
   as the functions of a multi-function card, and revoking a domain stops every device bound to it
   (§16 (object revocation)).
2. **The kernel trusts the bus server with configuration space.** The server places BARs and
   writes configuration space, and the kernel does not check BAR placement against the windows the
   firmware named. A bad placement misdirects the CPU's accesses. It does not widen what a device
   can reach by DMA, because the device's domain and the IOMMU alone confine that. The confinement
   argument stays in the kernel, as §4 (kernel shape) requires.
3. **Interrupt remapping is a prerequisite milestone, not part of 1064.** A bus server that writes
   MSI targets can aim an interrupt at a vector it was never given unless interrupt remapping
   confines it. Milestone 1068 (the kernel turns interrupt remapping on where an IOMMU offers it)
   builds that, and milestone 1064 depends on it.

Confinement means an IOMMU. On a machine without one, nothing confines a device's DMA or its
interrupts, and the kernel trusts the drivers there. Both objects and interrupt remapping apply
only where an IOMMU exists.

## The options that lost

- A device object alone, with the domain inside it and a kernel check of BAR placement. It lost
  because one domain could not serve several devices, and a multi-function card or a group the
  IOMMU cannot tell apart needs exactly that.
- The bus server programs the IOMMU itself, with the IOMMU's registers granted to a userspace
  platform server. It takes the confinement argument out of the kernel, against §4 (kernel shape).
- The kernel keeps enumeration. The extraction that milestone 1064 builds refuses it.

## Prior art, recalled, not re-read

- seL4 on x86 has an `IOSpace` capability per PCI requester id and `IOPageTable` objects the root
  task maps into it. The kernel walks the DMAR at boot and leaves configuration space to userspace.
  The rule here is close to it.
- Fuchsia (Zircon) splits an `IOMMU` object from a `BTI` (bus transaction initiator) object made
  from it per requester id, and pins memory through the BTI. The PCI bus driver runs in userspace
  and Zircon trusts it with configuration space. The rule here is the same shape.
- Genode's platform driver runs in userspace, owns configuration space, and in recent releases
  programs the IOMMU itself. nife trusts its bus server with configuration space the same way, but
  keeps the IOMMU in the kernel.
- Redox runs `pcid` in userspace, trusts it with configuration space, and confines no DMA.
- Linux VFIO confines by IOMMU group, the smallest set of devices the IOMMU can tell apart, and
  virtualizes configuration-space writes to the BARs and MSI fields. Xen's `pciback` filters a
  guest's configuration-space writes. Both are stricter than the rule here, which goes with
  Fuchsia, seL4, Genode and Redox in trusting the bus server and checks no write.

Where nife differs: nife already has domains in the kernel (`iommu::confine`) on all three
architectures, so the two objects add a kind and a binding rather than new hardware support.

## A concrete example

Suppose e1000e is function 00:02.0 on QEMU's `q35`. The bus server holds the device object for
requester id 0x0010 and places the function's BARs. The control process of milestone 1062 (the
device control planes leave the kernel) receives the device object, makes a domain from its own
memory budget, and binds the device to it. Only then does the kernel let bus mastering on. If the
bus server placed a BAR over RAM, the CPU's accesses through it go astray, but the device's DMA
still reaches only the domain's pages. Revoking the domain stops the device.

## What this unblocks

Milestone 1064 (PCI enumeration leaves the kernel), and with it the last 500 or so lines of
`kernel/src/pci.rs` and the kernel's link to `crates/pci`, once milestone 1068 (the kernel turns
interrupt remapping on where an IOMMU offers it) has landed.
