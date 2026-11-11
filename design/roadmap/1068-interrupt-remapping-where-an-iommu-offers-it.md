---
status: NOT-STARTED
raised: 2026-11-10
milestone_dependencies: 317
decision_dependencies: 319
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1068. The kernel turns interrupt remapping on where an IOMMU offers it

*(The number, title and slug are provisional.)*

Reuse: milestone 317 (the interrupt-remapping flags) already reads whether each machine offers
interrupt remapping, and the three IOMMU drivers in `kernel/src/arch/` already own the tables this
extends. The kernel drives the IOMMU, so no third-party crate applies.

§319 (a device and its DMA domain as kernel objects) trusts a userspace bus server with
configuration space, and configuration space holds each device's MSI and MSI-X targets. An MSI is a
memory write to a special address, so DMA remapping does not confine it. A server, or a driver it
hands a device to, could aim an interrupt at a vector it was never given. Interrupt remapping is
what refuses that, and §319 makes it a milestone of its own that milestone 1064 (PCI enumeration
leaves the kernel) waits on.

Confinement means an IOMMU. This milestone applies only where one exists. On a machine without one,
nothing confines a device's interrupts or its DMA, and the kernel trusts its drivers.

## Today

Milestone 317 makes the question askable and turns no remapping on. Each IOMMU driver reports what
the machine offers and leaves it off:

- x86_64: VT-d offers it (`ECAP.IR`) under QEMU's `intel-iommu` and on xenon. The kernel never sets
  `GCMD.IRE`, and a test asserts `GSTS.IRES` stays clear. AMD-Vi has run only on QEMU.
- riscv64: QEMU's `riscv-iommu-pci` offers the MSI page table (`CAPS.MSI_FLAT`), and `attach`
  writes `msiptp.MODE = Off`. No silicon ships the ratified RISC-V IOMMU (milestone 143 (silicon
  IOMMU)).
- aarch64: MSI translation lives in the GICv3 ITS, which no driver here programs
  (`kernel/src/drivers/gicv3.rs`'s BUGS). QEMU's `virt` offers it with `gic-version=3`.

Of the owned machines, only xenon has an IOMMU nife can drive, so on silicon only the x86_64 VT-d
driver could use remapping today. radon's JH7110 has no IOMMU. argon's Tegra210 SMMU is NVIDIA's own
design, which the SMMUv3-only aarch64 driver cannot drive. The QEMU runners attach SMMUv3,
`riscv-iommu-pci` and `intel-iommu`, so the main suites run behind an IOMMU on every architecture.

## Prior art, recalled, not re-read

- Linux VFIO refuses to hand a device to userspace on a machine without interrupt remapping, unless
  an administrator sets `allow_unsafe_interrupts`.
- Fuchsia and seL4 program interrupt remapping inside the kernel's IOMMU code, beside DMA
  remapping, and userspace never writes the remapping table.

Where nife differs: nothing. The kernel owns the remapping table, the same as it owns each DMA
domain, and the bus server writes only what the table lets through.

## The work, in order

1. x86_64 VT-d: an interrupt remapping table, `GCMD.IRE` set, and each MSI the kernel grants
   written as a remapped entry that names the one vector the kernel granted its holder. The test
   milestone 317 holds (`GSTS.IRES` stays clear) turns into its opposite. Effort: M (a guess).
2. riscv64: an MSI page table per device context, `msiptp` in flat mode, with one entry per granted
   interrupt file. Effort: S to M (a guess).
3. x86_64 AMD-Vi: its interrupt remapping table, under QEMU's `amd-iommu`. Effort: S to M (a guess).
4. aarch64: the GICv3 ITS (command queue, device table, `MAPD` and `MAPTI`), so a device's MSIs
   translate through its own table. Effort: L (a guess); it is the bill milestone 317 names as owed.
5. A test on each architecture in the shape of milestone 202 (every confinement test is a ritual
   until somebody breaks the confinement and watches it fail). A component holding a device's
   MSI-X table page aims an interrupt at a vector it was never granted, and the hardware refuses it.

Total: L (a guess).

## A concrete example

Under QEMU's `q35`, the bus server writes e1000e's MSI-X table entry 0 to deliver vector 0x41, a
vector the kernel granted to the storage driver and not to the network driver. With remapping on,
the address the server can write names only an index into the kernel's remapping table, and the
entry at that index names the network driver's own vector. The write goes to the network driver or
nowhere. Without remapping, it reaches the storage driver.

## Done when

1. On x86_64 (VT-d and AMD-Vi), riscv64 and aarch64 under QEMU, the kernel turns remapping on and
   the test in step 5 shows a forged MSI refused.
2. On xenon, the same test passes on VT-d.
3. A machine with no IOMMU, or one whose IOMMU offers no remapping, boots as it does today, and its
   machine description says remapping is absent.
4. The existing IOMMU and PCI gates pass on every architecture, with no test weakened.

## Index row

An MSI is a memory write, so DMA remapping does not confine it, and nife turns no interrupt
remapping on. This turns it on in the VT-d, AMD-Vi and RISC-V IOMMU drivers and drives the aarch64
GICv3 ITS, so a userspace bus server that writes MSI targets cannot aim an interrupt at a vector it
was not given. Milestone 1064 waits on it, and it applies only where an IOMMU exists.
