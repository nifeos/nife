# Rented aarch64: the offers and the blocker

An appendix to [`notes/rented-metal.md`](../rented-metal.md), which holds the four requirements,
the recommendation for each architecture and BUGS. This file holds why no aarch64 cloud machine
boots nife, the offers table as read on 2026-09-23, and the Oracle Ampere A1 experiment.

The loader writes a device tree from the ACPI tables (`uefi_loader/src/device_tree_from_acpi.rs`),
and under QEMU with ACPI on the stick reaches the same shell prompt as with a firmware tree
(verified against `uefi_loader/src/arch/aarch64/mod.rs` and
[`notes/boot-stick/device-tree-handover.md`](../boot-stick/device-tree-handover.md)). So the first
blocker below, the loader reading only a firmware device tree, is lifted. The second, the kernel's
link address, stands. No cloud machine has been booted, so every `no (nife)` below is unverified
rather than known.

## The image exists, and requirement 1 is unverified rather than failed

calef, 2026-09-23: *"aarch64 has no cloud options. That doesn't seem right. What about AWS?"*

1. The image exists. `xtask/src/stick.rs`'s `payload_aarch64` builds `BOOTAA64.EFI` for the
   `aarch64-unknown-uefi` target, out of the aarch64 archive, the kernel, and `uefi_loader/`.
   `BOOTAA64.EFI` at the EFI system partition's default path is exactly what a UEFI aarch64 machine
   boots.
2. "Unverified" is not "disqualified". A requirement nobody checked is a gap in the evidence, not a
   failure by the offer. The tables score requirement 1 with a vocabulary that keeps the two apart,
   and it applies to every architecture. The four values are in the parent's table under "The four
   hard requirements". The last, `no (nife)`, is where this whole column lands.

Every aarch64 offer below passes requirements 2, 3 and 4 and lets you bring your own image. The
blocker is in this repository, it is written down, and no amount of money moves it.

## Why no aarch64 cloud machine booted nife on 2026-09-23

`uefi_loader/src/arch/aarch64/mod.rs` finds the device tree by searching the UEFI configuration
table for `efi::DEVICE_TREE_GUID`, copies it, patches `/chosen/linux,initrd-*`, and branches to the
kernel with `x0` = that tree. Its own `BUGS` section says what that costs, and it said so before
this file existed:

> The device tree must be offered by the firmware. EDK2 on QEMU `virt` offers it only with
> `acpi=off`; an aarch64 machine that describes itself only with ACPI (most servers) cannot boot
> this kernel at all, which is a kernel limit rather than a loader one.

Every aarch64 server cloud is one of those machines, and by specification rather than by
accident. Arm's Server Base Boot Requirements make UEFI and ACPI the boot contract for Arm servers,
and the clouds comply. AWS documents that on arm64 with no boot mode specified the resulting
instance boot mode is UEFI. Azure's Cobalt 100 Arm series is Generation 2 only. Google's own
image-import requirements state flatly that "The boot disk must support ACPI." A third-party port of
another OS to Graviton records the consequence in the exact form nife would meet it. Graviton boots
via ACPI with no timer node, so the generic-timer interrupt IDs have to come from the ACPI GTDT
rather than from a device tree that is not there.

There is a second, smaller blocker behind the first, from the same `BUGS` section: the kernel is
linked for `0x4008_0000`, which is QEMU `virt`'s RAM. A cloud machine whose RAM starts elsewhere
cannot place the kernel there. That is the same limit milestone 127 (the seL4 machine)'s argon port
has to lift, not one the cloud adds.

And this is exactly the shape fatal risk 9 (the HAL is a fiction, and an architecture costs a
restructure rather than a port) predicts, arriving early and cheaply. The x86_64 side of the very
same loader already does the ACPI job: `uefi_loader/src/arch/x86_64/mod.rs` takes the ACPI RSDP from
the firmware's configuration table and synthesizes an `hvm_start_info` for the kernel. So nife
already has an ACPI discovery path, on one architecture only, and aarch64 and riscv64 are
device-tree-only. A machine that hands the kernel ACPI instead of a device tree is not a new
architecture; it is the same aarch64 with different firmware, and nife does not boot on it. That is
risk 9's failure mode showing up as a firmware difference rather than an ISA difference,
which is the gap the risk's own text does not cover.

What this says about the money: nothing here is a purchasing problem. The aarch64 column is
blocked on an ACPI discovery path for aarch64, which is a milestone. It is proposed below rather than
assumed.

## The offers, priced properly

| Provider | Offer | Cores | RAM | Per hour | Free or trial | 1 boot | 2 console | 3 power API | 4 cores | Source, read 2026-09-23 |
|---|---|---|---|---|---|---|---|---|---|---|
| AWS | EC2 `c7g.large` (Graviton3) | 2 vCPU | 4 GiB | $0.073 | 12-month free tier excludes it | **no (nife)** | yes | yes | yes | [c7g](https://aws.amazon.com/ec2/instance-types/c7g/), [price](https://instances.vantage.sh/aws/ec2/c7g.large) |
| AWS | EC2 `c7g.4xlarge` (Graviton3) | 16 vCPU | 32 GiB | $0.58 | none | **no (nife)** | yes | yes | yes | [price](https://instances.vantage.sh/aws/ec2/c7g.4xlarge) |
| AWS | EC2 `c7g.metal` (Graviton3) | 64 vCPU | 128 GiB | $2.32 | none | **no (nife)** | yes | yes | yes | [price](https://instances.vantage.sh/aws/ec2/c7g.metal) |
| AWS | EC2 `c8g.metal-24xl` (Graviton4) | 96 vCPU | 192 GiB | $3.83 | none | **no (nife)** | yes | yes | yes | [price](https://instances.vantage.sh/aws/ec2/c8g.metal-24xl) |
| Oracle | Always Free Ampere A1 | 2 OCPU | 12 GB | **free** | Always Free, standing | **no (nife)**, see below | yes | yes | yes, 2 | [Always Free](https://docs.oracle.com/en-us/iaas/Content/FreeTier/freetier_topic-Always_Free_Resources.htm), [BYOI](https://docs.oracle.com/en-us/iaas/Content/Compute/References/bringyourownimage.htm), [console](https://docs.oracle.com/en-us/iaas/Content/Compute/References/serialconsole.htm) |
| Azure | `Dpsv6` series (Cobalt 100 Arm64) | 2 to 96 vCPU | 8 to 384 GiB | not read | none | **no (nife)** | yes, boot diagnostics | yes | yes | [Dpsv6](https://learn.microsoft.com/en-us/azure/virtual-machines/sizes/general-purpose/dpsv6-series) |
| Google | C4A Axion, incl. `c4a-standard-96-metal` | up to 96 vCPU | up to 768 GB | not read | none | **no (nife)** | yes | yes | yes | [Arm on Compute](https://docs.cloud.google.com/compute/docs/instances/arm-on-compute), [import](https://docs.cloud.google.com/compute/docs/images/import-existing-image) |
| Hetzner | RX line (Arm64 dedicated) | - | - | - | - | **unavailable** | - | - | - | [RX matrix](https://www.hetzner.com/dedicated-rootserver/matrix-rx/) |
| Hetzner | Cloud CAX31 (Ampere Altra VM) | 8 vCPU | 16 GB | ~€15.99 to €25.57/mo | none | **no (offer)** | VNC only | yes | yes | [cloud](https://www.hetzner.com/cloud/), prices third-party, see BUGS |
| Scaleway | Elastic Metal, Arm | - | - | - | - | **none offered** | - | - | - | [pricing](https://www.scaleway.com/en/pricing/elastic-metal/) |

aarch64 is not a thin column, which is the correction. AWS, Azure and Google all sell Arm
compute, Google and AWS both sell Arm bare metal, and all three take a custom image. The first
draft looked at one instance type and generalized.

A `.metal` instance is not required, and this is the largest price finding in the file. The four
requirements are met by an ordinary virtualized Graviton instance: the EC2 serial console is
supported on *"All virtualized instances built on the Nitro System"*, the EC2 API does power, and
`c7g.large` has two cores. So the relevant aarch64 price is $0.073 an hour, not $2.32 -- a factor
of 32 -- and a 16-core `c7g.4xlarge` at $0.58 sits between them. Bare metal buys removed
virtualization, which matters for a timing claim and not for a soak that only has to cross cores.
Pricing the most expensive row in the family makes the architecture look unaffordable when it is
not.

One caveat on bare metal specifically cuts the other way. AWS's boot-mode page says *"All
instances built on the AWS Nitro System support both UEFI and Legacy BIOS, except the following: bare
metal instances"*, and the serial-console page excludes `a1.metal` and `g5g.metal` by name. So the
`.metal` rows carry a boot-mode caveat that the virtualized rows do not, and the cheap rows are the
better target anyway.

## Oracle Ampere A1, and exactly what would resolve it

Oracle's offer is resolved far enough to act on, in the unfavorable direction.

The Oracle page's ambiguity is about Linux guests: Arm shapes take custom imported images in
paravirtualized mode only, the listed guests are all Linux or Windows, and the stated requirement is
a *"Linux kernel version 3.4 or later"*. That ambiguity is not the deciding one. Ampere A1
instances boot UEFI and are SBBR-class, so they present ACPI and no device tree, and nife's
aarch64 path needs a device tree. Requirement 1 is `no (nife)` for the same reason as every other
row, and Oracle's own wording is not what settles it.

What would resolve it definitively, and it is one observation rather than an argument: boot any
aarch64 instance on any of these providers with `BOOTAA64.EFI` on an EFI system partition, and read
the serial console. `uefi_loader`'s aarch64 path prints the firmware's memory map when it cannot
place the kernel, and the device-tree lookup either finds `DEVICE_TREE_GUID` in the configuration
table or does not. That is a free experiment on Oracle Always Free, it costs an evening, and it
produces a real answer instead of a reading of a documentation page. It should be run, and it should
be run expecting it to fail, because a failure here names the assumption precisely.
[`oracle-a1.md`](oracle-a1.md) is the procedure.
