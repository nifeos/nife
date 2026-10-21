# Oracle, Google, Azure and Grace rentals

Appendix to [the modern aarch64 board survey](../aarch64-modern-board-survey.md), and to [its server overview](server-and-cloud.md).

Read 2026-10-21 (UTC). A source marked (secondary) is neither the vendor nor the upstream tree. "Not found" means no primary answer, not no. "Inferred" means deduced from cited facts and not read.

## Oracle Cloud, Ampere A1 and AmpereOne

| topic | fact |
|---|---|
| Always Free | VM only: `VM.Standard.A1.Flex` (up to 2 OCPUs and 12 GB across instances) and `VM.Standard.E2.1.Micro`. No bare metal on the free tier [O2] |
| bare metal that exists | `BM.Standard.A1.160` (Altra Q80-30, 160 OCPU, 1024 GB) and `BM.Standard.A4.48` and `BM.Standard.A4.Ax.48` (AmpereOne-M, 96 cores, 768 GB). No `BM.Standard.A2` appears in the shape reference [O1] |
| AmpereOne | rentable as bare metal on the A4 shapes. One user reports `ID_AA64PFR1_EL1.MTE` is 0 on `A4.48` under Oracle firmware (secondary, one report) [O3] |
| SMMUv3, GIC | Ampere lists GICv3 and SMMUv3 for Altra [O4]. A kernel issue on an Ampere box shows `arm-smmu-v3` and its PMU devices (secondary, platform unnamed) [O5]. Oracle's docs say nothing about either. Inferred for bare metal, not found |
| custom boot | "Bringing your own image to a bare metal instance is not supported". Imported images on Arm shapes are paravirtualized, and the procedure sets BIOS firmware [O6]. This blocks the documented path to an unsigned kernel. Replacing the boot volume's contents from a stock Oracle Linux image is possible in principle (inferred); Secure Boot policy on Arm metal: not found |
| serial console | an SSH-based console connection through a proxy, one client at a time, 24 hour sessions; bare metal is covered, Arm shapes are never mentioned [O7]. Agent-drivable by `ssh` (inferred); the page documents no CLI command |
| price | A1 about $0.01 per OCPU-hour plus $0.0015 per GB-hour (secondary) [O8], so about $3.1 per hour for `A1.160` (arithmetic). A4 about $0.019 per OCPU-hour (secondary); memory rate and billing unit not read; Oracle's price list refused the fetcher [O9] |
| reset | by API (inferred; not re-read) |
| PMU | not found |

Sources, read 2026-10-21:

- O1 https://docs.oracle.com/en-us/iaas/Content/Compute/References/computeshapes.htm
- O2 https://docs.oracle.com/en-us/iaas/Content/FreeTier/freetier_topic-Always_Free_Resources.htm
- O3 (secondary) https://community.amperecomputing.com/t/ampereone-m-a4-on-oracle-cloud-mte-not-exposed-even-on-bare-metal-id-aa64pfr1-el1-mte-0/3447
- O4 https://amperecomputing.com/en/briefs/ampere-altra-family-product-brief (search snippet only); O5 (secondary) https://github.com/AmpereComputing/ampere-lts-kernel---DEPRECATED/issues/153
- O6 https://docs.oracle.com/en-us/iaas/Content/Compute/References/bringyourownimage.htm
- O7 https://docs.oracle.com/en-us/iaas/Content/Compute/References/serialconsole.htm
- O8 (secondary, search snippet) https://www.oracle.com/cloud/compute/arm/
- O9 https://www.oracle.com/cloud/price-list/ (403)

## Azure Cobalt, Google Axion, NVIDIA Grace, and other rentals

| machine | fact |
|---|---|
| Azure Cobalt 100 | Neoverse N2 virtual machines in the Dpsv6, Dplsv6 and Epsv6 families. No bare metal or dedicated host appears on Microsoft's overview [O10]. Absence is not proof. Cobalt 200 virtual machines are in preview (secondary) [O11]. Ruled out for this question |
| Google Axion C4A | bare metal is real and generally available: `c4a-standard-96-metal` (384 GB) and `c4a-highmem-96-metal` (768 GB), 96 vCPU, Neoverse V2; N4A (Neoverse N3) has no metal size [O12][O14]. Serial console: `serial-port-enable` metadata and `gcloud compute connect-to-serial-port`, 115200 baud for bare metal, scriptable [O16]. Public-image example uses `--no-shielded-secure-boot`; the UEFI requirement for a custom C4A metal image is not found [O15]. GIC, SMMU, PMU: not found. About $5.66 per hour for highmem (secondary tracker) [O17] |
| NVIDIA Grace | Neoverse V2; the kernel needs `ARM_PMUV3` and `ARM_PMU_ACPI` for its PMU [O18]; SMMUv3 with NVIDIA's command-queue extension, described by IORT [O19]. Lambda lists GH200 at $2.29 per GPU-hour billed per minute and does not say VM or bare metal [O21]. Vultr lists GH200 as bare metal, $1.99 to $2.99 per GPU-hour (secondary, snippet) [O22]. Custom kernel boot and console on either: not found |
| Equinix Metal | Altra bare metal, shut down: last day to access instances 2026-06-30 (secondary, search summary of Equinix's own docs and press) [O24][O25] |
| Hetzner RX | the matrix page says RX servers are unavailable [O26] |
| phoenixNAP | `a1.c5.large`: Altra Q80-30, 80 cores, 256 GB, 2x2 TB NVMe, $0.99 per hour, Phoenix or Ashburn. API has power on, off, reboot and reset [O28][O30]. Billing unit, custom ISO or iPXE, remote console on Arm: not found |
| Scaleway, OVH, Latitude.sh, Vultr CPU, Cherry | no current Arm bare-metal listing found. Not found, not nonexistent |

Sources, read 2026-10-21:

- O10 https://learn.microsoft.com/en-us/azure/virtual-machines/sizes/cobalt-overview
- O11 (secondary) https://mc.merill.net/message/AZ564451
- O12 https://docs.cloud.google.com/compute/docs/instances/bare-metal-instances
- O14 https://docs.cloud.google.com/compute/docs/general-purpose-machines
- O15 https://docs.cloud.google.com/compute/docs/instances/create-vm-from-public-image
- O16 https://docs.cloud.google.com/compute/docs/troubleshooting/troubleshooting-using-serial-console
- O17 (secondary) https://pcr.cloud-mercato.com/providers/google/flavors/c4a-highmem-96-metal.googleaxion/pricing
- O18 https://docs.nvidia.com/dccpu/grace-perf-tuning-guide/measuring-performance.html
- O19 https://lists.infradead.org/pipermail/linux-arm-kernel/2024-July/940400.html
- O21 https://lambda.ai/instances
- O22 (secondary) https://www.vultr.com/products/cloud-gpu/nvidia-gh200/
- O24 https://docs.equinix.com/metal/eos-faq (403; summary via search)
- O25 (secondary) https://www.datacenterdynamics.com/en/news/equinix-to-kill-off-metal-by-june-2026/
- O26 https://www.hetzner.com/dedicated-rootserver/matrix-rx
- O28 https://phoenixnap.com/offers/bmc-arm-servers (search snippet; direct fetch 404)
- O30 https://phoenixnap.com/kb/bmc-server-management-via-api
