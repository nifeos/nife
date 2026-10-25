---
status: PROPOSED
raised: 2026-10-24
milestone_dependencies: 578, 801
decision_dependencies: none
machine_requirements: aarch64 SBBR cloud firmware with virtio over PCIe
specific_machine: oracle-a1 (an Oracle Cloud Ampere A1 instance; calef ruled 801's aarch64 leg onto it on 2026-10-24 UTC)
needs_person: yes
---
# An aarch64 cloud instance drives its PCIe devices, so `jig` runs on it

Raised 2026-10-24 (UTC) by `maintainer/record-2026-10-24-b`. calef ruled that day that milestone
801 (packages over the internet)'s aarch64 leg runs on an Oracle Cloud Ampere A1 instance, whose
network is `virtio-net-pci`. nife has a virtio-net driver, so that is the right machine to choose
over AWS Graviton, whose ENA has none. But the booted system on an A1 does not reach that NIC
today, for two reasons in the tree, and the instance offers nothing else `jig` needs.

## What an A1 boot gets today

The A1 boot is milestone 578 (an ACPI discovery path for aarch64)'s item 7,
[notes/rented-metal/oracle-a1.md](../../../notes/rented-metal/oracle-a1.md). Under QEMU with the same
description (`acpi=on`, GICv3, a `virtio-blk-pci` disk and a `virtio-net-pci` NIC) it reaches `$`
with `pcie : none`:

| what `jig` needs | today on an ACPI aarch64 boot | why |
|---|---|---|
| the NIC | not driven | `uefi_loader::device_tree_from_acpi` writes no PCI host bridge (its BUGS: `ranges` and `interrupt-map` live in AML), and the booted system claims only a virtio-net on MMIO (milestone 590 (the booted system starts its network stack) left the PCIe NIC unclaimed) |
| entropy | none | no virtio-rng on MMIO, and Neoverse N1 has no `RNDRRS` (`entropy : NONE` in the QEMU run) |
| a file service | none | the boot volume is on PCIe too |
| the wall clock | none | the written tree has no RTC node |

## The work

1. The loader describes the PCIe host bridge from ACPI: MCFG's ECAM window is read already
   (`mcfg_entry`), and the interrupt routing has to come from `_PRT` or from MSI.
2. The booted system claims a virtio-net, virtio-blk and virtio-rng behind PCIe, as milestone 590's
   block already names for "widening both" (the NIC and the rng). Over PCIe each sits behind the
   SMMU when there is one.
3. The wall clock from the firmware: the loader reads UEFI `GetTime` before it exits boot services
   and hands the reading over, the way the x86_64 kernel hands the clock service its CMOS reading.
4. Which of those devices an A1 actually offers is unmeasured. Item 7's boot can list them from the
   firmware's shell (`pci`), which the A1 note now asks for.

If the A1 offers no entropy device, the instance has no source nife can use, and where a cloud
guest's entropy comes from becomes a question for calef. This proposal does not guess at it.

Reuse: the virtio drivers, the PCIe transport and the SMMU seam exist and run in the kernel
harness; the work is description and wiring. AML interpretation for `_PRT` is the part with a
choice: a crate such as `acpi`'s AML half, or MSI-X, which needs no AML. Unsurveyed.

## Exit criteria

1. Under QEMU's `acpi=on` aarch64 machine with the A1's devices, `jig update` and `jig install` pass
   against the test peer, gated in CI.
2. On an A1 instance, `jig update` reads `basalt.nifeos.org`'s index and `jig install ioping`
   installs it, from the console history.

## BUGS

- The device inventory in item 4 is a reading of Oracle's documentation (paravirtualized mode is
  virtio over PCIe) until a boot lists it.
