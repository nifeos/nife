# ARM servers and cloud bare metal, for the modern aarch64 board survey

Overview appendix to [the modern aarch64 board survey](../aarch64-modern-board-survey.md). The detail and sources are in three files:

- [`aws-graviton-metal.md`](aws-graviton-metal.md)
- [`oracle-google-azure-grace-rentals.md`](oracle-google-azure-grace-rentals.md)
- [`buyable-arm-servers.md`](buyable-arm-servers.md)


Read 2026-10-21 (UTC) unless marked. Nothing was launched, rented, bought or signed up for.

A source marked (secondary) is neither the vendor nor the upstream tree (a blog, forum, price
aggregator or search snippet). A fact marked "not found" had no primary answer; that does not mean
the answer is no. A fact marked "inferred" is deduced from other cited facts and was not read.
Many vendor pages (Newegg, eBay, Gigabyte, Oracle's price list, Vultr, AWS quota page) refused the
fetcher, so those prices come from search snippets and are marked.

Per-core features live in [the core table](cores-and-sel4.md). Machine-level facts for Altra and
AmpereOne already in [the Ampere appendix](jetson-and-ampere.md) are not repeated.

## The answer to the deciding question

Whether a real SMMUv3 sits in front of the devices and reaches the OS is **not established for any
rentable machine**. What the sources do support:

| machine | SMMUv3 in front of its devices, visible to the OS | strength |
|---|---|---|
| AWS Graviton3, 4, 5 `.metal` | Probably. AWS's DPDK README says metal instances support the IOMMU on arm64, calls the arm64 IOMMU an SMMU, and says it is enabled by default; its only arm64 example is `a1.metal` (Graviton1) [A19]. No IORT dump, `dmesg` or IOMMU-group listing for G3 or later was found. | primary text, wrong generation |
| OCI `BM.Standard.A1.160`, `A4.48` | Not stated by Oracle. Ampere lists SMMUv3 for the silicon [O4]. | inferred |
| GCP `c4a-*-metal` | Not stated by Google. | not found |
| phoenixNAP `a1.c5.large` | Not stated. Altra silicon has SMMUv3. | inferred |
| Grace (GH200 and similar) | Yes in silicon and kernel: NVIDIA's command-queue extension is an SMMUv3 extension that the driver matches by IORT model number [O19]. A rentable Grace box exposing it: not found. | silicon verified |
| Buyable Altra (ASRock Rack, Supermicro) | Ampere's reference firmware builds one SMMUv3 node per enabled PCIe root complex, mapping all requester IDs, plus an ITS group [B4]. Whether ASRock's or Supermicro's builds use that code: inferred. | source for Jade reference only |

How it is described: ACPI IORT on every machine here. Mainline Linux has no Ampere device tree
(`arch/arm64/boot/dts/ampere` is a 404 [B1]), and AWS requires UEFI with ACPI tables for Graviton
images [A14]. So nife would parse the IORT on all of them, which is the same work the O6's shipping
firmware needs.

One cheap measurement settles AWS: a `c7g.metal` for a few minutes, then `acpidump -n IORT`,
`dmesg | grep -i smmu` and `ls /sys/kernel/iommu_groups` (inferred cost below). It was not run.

## Scorecard

Prices are on-demand or list, in USD, from the sources below. "Boot" is how a custom aarch64 OS
reaches the machine.

| machine | cores, level | GIC, ITS | boot of a custom kernel | serial console | per hour, minimum | reset | PMU cycle counter |
|---|---|---|---|---|---|---|---|
| AWS `c7g.metal` (Graviton3) | Neoverse V1, v8.4, PAC, SVE; no BTI | GICv3+ITS (secondary only) | UEFI+ACPI, Secure Boot off by default; EBS snapshot to AMI | EC2 serial console, CLI | $2.32 (secondary), 60 s | API | metal exposes all PMUs; counter register not stated |
| AWS `c8g.metal-24xl` (Graviton4) | Neoverse V2, v9.0, PAC, BTI, SVE2 | as above | as above | as above | $3.83 (secondary), 60 s | API | as above |
| AWS `c9g.metal-48xl` (Graviton5) | Neoverse V3, v9.2, PAC, BTI | not found | as above | as above | $8.35 (secondary), 60 s | API | as above |
| OCI `BM.Standard.A1.160` (Altra) | N1, v8.2 | GICv3 (Ampere); ITS not found | bring-your-own-image on bare metal unsupported | SSH console connection | about $3.1 (arithmetic from per-OCPU rate, secondary) | API | not found |
| OCI `BM.Standard.A4.48` (AmpereOne-M) | Ampere-1a, MTE field 0 (secondary) | not found | as above | as above | rate not read | API | not found |
| GCP `c4a-standard-96-metal` | Neoverse V2, v9.0 | not found | UEFI requirement for metal not stated | `gcloud` serial port, 115200 for metal | $5.66 for highmem (secondary) | API | not found |
| phoenixNAP `a1.c5.large` | Altra Q80-30, N1 | Altra silicon | custom ISO, iPXE, console on Arm: not found | not found | $0.99 | API power cycle | not found |
| ASRock Rack ALTRAD8UD-1L2T build | Altra or Altra Max, N1 | GICv3+ITS | BMC virtual media; Secure Boot default not found | BIOS COM1 redirect; SOL not verified | $0 (owned), about $1.5k to $2.5k to build (inferred) | BMC power control | not found |
| Supermicro ARS-110M-NR | Altra or Altra Max | as above | virtual media; Secure Boot default not found | SOL not documented for the model | $0, barebone about $2.1k to $2.7k, all out of stock | IPMI | not found |
| Azure Cobalt 100, 200 | Neoverse N2 | VMs only | no bare metal or dedicated host found | n/a | n/a | n/a | n/a |
| Grace, GH200 rentals | Neoverse V2 | SMMUv3 yes | custom kernel path unknown | unknown | $1.99 to $2.99 per GPU-hour (secondary) | unknown | unknown |

The seL4 tier is "none" for every machine above: seL4's `src/plat` has no Graviton, Ampere, Axion
or Grace platform (read from the tree on 2026-10-21) and no SMMUv3 driver, as
[the core table](cores-and-sel4.md) records. So none gives a seL4 comparison without a seL4 port;
a PMU on the rented hardware does not change that.

## BUGS

- No IORT, `dmesg` or IOMMU-group output exists in any source for a rentable Arm machine other than
  the Graviton1 DPDK text. The central claim of the survey's server section is therefore inferred.
- Every rental price here is from an aggregator, snippet or arithmetic, except phoenixNAP's own
  offer (via a search snippet). AWS's pricing pages are rendered client-side and were not read.
- No source for any machine documents `MDCR_EL3` or `PMCCNTR_EL0` access at EL1 or EL0.
- SOL is verified for no buyable board by a primary source.
- Several vendor pages (Oracle price list, Vultr, Equinix FAQ, Newegg, Gigabyte) refused the
  fetcher; their facts rest on search results.
- Arm's Neoverse V1 and V2 TRMs did not render, so MTE on Graviton is not found.
