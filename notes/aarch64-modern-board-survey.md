# A modern aarch64 board for nife's first ARM machine

*Name: provisional, minted 2026-10-21 (UTC) by survey/aarch64-modern-board.*

*Name provisional (`aarch64-modern-board-survey`), per the rule that a lane ships a provisional name
and says so. Surveyed 2026-10-21 (UTC); every price and stock figure is that day's.*

This survey asks which aarch64 board nife should own when the board is chosen for modern features
rather than for the seL4 comparison. [The seL4-comparison survey](aarch64-board-survey.md) answers
the other question, and its deciding criterion (silicon under seL4's published numbers) is not used
here. seL4's support tier is still scored, so the cost of separating the two questions is visible.

The evidence is in appendices, each with a URL and a read date on every claim. This page
carries the scoring and the recommendation. It records no choice: the board is calef's call.

- [Arm cores and seL4 support](aarch64-modern-board-survey/cores-and-sel4.md): per-core features
  from Arm's TRMs, and seL4's tiers read from its own trees.
- [Raspberry Pi 5 and RK3588 boards](aarch64-modern-board-survey/pi5-and-rk3588.md).
- [CIX P1, MediaTek Genio, NXP i.MX 95, TI and Qualcomm](aarch64-modern-board-survey/cix-genio-nxp-qualcomm.md).
- [Jetson, DGX Spark and Ampere](aarch64-modern-board-survey/jetson-and-ampere.md).
- [ARM servers and cloud bare metal](aarch64-modern-board-survey/server-and-cloud.md): Graviton,
  Oracle, Google, Azure, Grace rentals, and buyable Ampere servers.

## What the criteria are for, traced to the tree

- The IOMMU on the DMA path. nife confines drivers, and a confined driver whose device can DMA
  anywhere is not confined. The kernel already drives an SMMUv3 (notes/iommu.md: the SMMUv3 walks
  the same VMSAv8-64 format `Mapper` builds, found as a device-tree node by `memory::smmu_region`).
  So the question per board is which DMA masters sit behind an SMMUv3, read from the Linux device
  tree's `iommus` and `iommu-map` properties or the ACPI IORT. An MMU-500 (SMMUv2) or a vendor IOMMU
  is a new driver, not a proving run.
- The GIC. The kernel speaks GICv2 and GICv3 (milestone 227 (a GICv3 driver)). An ITS is what turns PCIe MSIs into
  LPIs, so a board with PCIe devices and no ITS keeps nife on wired or vendor-bridged interrupts.
- Architecture level. Armv8.1 brings VHE and LSE atomics, and v8.1 to v8.2 bring PAN. PAC and BTI
  (v8.3 and v8.5) harden control flow, MTE (v8.5) tags memory, and SVE2 is Armv9's vector set.
  calef's standing preference is to require newer features rather than carry paths for old boards.
- Boot. A board that follows Arm SystemReady (UEFI, with ACPI or a device tree, and BSA/SBSA) needs
  less board-specific code. Upstream TF-A and mainline U-Boot or EDK2 decide whether the firmware
  can be rebuilt and read.
- Debug and measurement. A serial console a person or `script/board-console` can read, and a PMUv3
  cycle counter that firmware leaves to EL1 (milestone 74 (cycle counters) reads `PMCCNTR_EL0`).
- Peripherals. PCIe, NVMe, xHCI and Ethernet are what make the board a computer rather than a
  console.

## The scorecard

PAC, BTI, MTE and SVE2 are read from each core's Arm TRM; "MTE" in the core does not mean the SoC
enables it. SMMU coverage is what the upstream or vendor device tree describes. The seL4 tier is
read from seL4's own trees.

| board | cores, level | PAC/BTI/MTE/SVE2 | GIC, ITS | SMMUv3 coverage | boot | seL4 tier | price, stock |
|---|---|---|---|---|---|---|---|
| Radxa Orion O6 (CIX P1) | 8x A720 + 4x A520, v9.2 | yes/yes/vendor BIOS only/yes | GIC-700 (v3, v4 frames), ITS | 3 instances; vendor DT covers PCIe and both GMACs, not USB; mainline DT has no SMMU node | SystemReady SR v2.5, UEFI+ACPI; CIX EDK2 and TF-A forks, some binaries closed | none | $199 to $299 at launch; sold out at Arace |
| Ampere Altra (Thelio Astra, ADLINK AADP) | Neoverse N1, v8.2 | no/no/no/no | v3, ITS | one per PCIe root complex, all RIDs; USB and NICs are PCIe devices | SystemReady SR (AADP, Mt. Jade); UEFI open in edk2-platforms; TF-A and SCP signed binaries | none | Thelio from $4,099, in stock; AADP $5,794, "available soon" |
| Radxa Rock 5B+ / Orange Pi 5 Plus (RK3588) | 4x A76 + 4x A55, v8.2 | no/no/no/no | GIC-600 (v3), 2 ITS, non-shareable erratum | all 5 PCIe root complexes (NVMe, RTL8125 NICs); not USB, eMMC, SATA, GMAC | upstream TF-A BL31 (DRAM init blob), mainline U-Boot, community EDK2 | none (rk3399, rk3568 nearest) | 5B+ sold out everywhere checked; OPi 5 Plus 16 GB about $355 to $390 |
| NXP i.MX 95 (FRDM, EVK) | 6x A55, v8.2 | no/no/no/no | v3, ITS | MMU-700: USB, PCIe, NETC; not uSDHC or eDMA; node disabled in mainline | upstream TF-A and U-Boot (ELE blob); no UEFI | none (i.MX 93 runs sel4test) | FRDM-PRO about $300 to $315, pending stock; 15-week lead |
| Jetson AGX Thor | 14x Neoverse V3AE, v9.2 | yes/yes/unknown/yes | v3, ITS | PCIe per RID; USB and Ethernet not yet in mainline DT | NVIDIA edk2-nvidia UEFI (open); no upstream TF-A or U-Boot | none | $5,499; no US retailer page found |
| Jetson Orin Nano Super | 6x A78AE, v8.2 + PAC | yes/no/no/no | v3, no ITS node | MMU-500 (SMMUv2); one stream ID per PCIe controller | edk2-nvidia UEFI; no upstream TF-A for T234 | none (Orin PR open since 2023) | $399, backorder |
| Raspberry Pi 5 | 4x A76, v8.2 | no/no/no/no | GIC-400 (v2), no ITS | none on PCIe, USB, Ethernet or SD | TF-A rpi5 (minimal); stock spin-table; not SystemReady | builds only (no Foundation hardware) | 4 GB $110, 8 GB $175, in stock |
| MediaTek Genio 1200/700/510 | A78 + A55, v8.2 | no/no/no/no | v3, no ITS | MediaTek M4U on PCIe only; vendor IOMMU, not SMMUv3 | U-Boot; TF-A only in a coreboot flow; closed DDR loader | none | NIO 12L $99 sold out; EVKs $1,100 to $1,400 |
| Qualcomm QCS6490 (RB3 Gen 2, RUBIK Pi 3) | Kryo 670, v8.2 | no/no/no/no | v3, ITS disabled | MMU-500 (SMMUv2) on PCIe, USB, UFS, SDHC | partial upstream TF-A (signed BL2), mainline U-Boot | none | RUBIK Pi 3 $179 list |
| DGX Spark (GB10) | 10x X925 + 10x A725, v9.2 | yes/yes/off/yes | not found | not found | closed AMI UEFI | none | $4,699 MSRP, reported out of stock |

AmpereOne (v8.6+, GICv4.1, SMMUv3.2) is SystemReady SR on Ampere's reference board, but the only
machine sold to an individual found was a $18,480 workstation, so it is left out of the ranking.
Snapdragon X Elite's PCIe SMMUv3 is held by Qualcomm's Gunyah hypervisor unless the machine boots
at EL2, and the dev kit was cancelled. TI's AM62 and AM69 have no Arm SMMU. RK3688 is announced
with nothing upstream and no boards shipping. The appendices carry the sources for each line.

## No board has everything

Three things almost never come together: Armv9 features, an SMMUv3 in front of every DMA master,
and open upstream firmware.

- The Armv9 boards with PAC, BTI and SVE2 (Orion O6, Thor, GB10) either leave a master out of
  the SMMU's reach or ship firmware nobody outside the vendor can rebuild.
- The one machine with an SMMUv3 in front of all I/O (Ampere Altra) gets there by having almost no
  on-die I/O: everything is a PCIe device behind a per-root-complex SMMU. Its N1 cores have none of
  PAC, BTI, MTE or SVE.
- The boards with the most open firmware (RK3588 with upstream TF-A and U-Boot, i.MX 95 with
  upstream TF-A and U-Boot) are Armv8.2, and neither covers USB on RK3588 or SD on i.MX 95.
- No board found exposes MTE through stock firmware. The Orion O6 has it through a self-built
  BIOS from CIX's EDK2 sources, with one unconfirmed report of tag faults on 64 GB boards.
- No vendor documents whether its firmware traps PMU access. Upstream TF-A leaves it untrapped,
  so on RK3588 nife controls it; on the O6, Altra and Jetson it is a bench check.
- None of the modern candidates has any seL4 platform. seL4's aarch64 tree has no SMMUv3 driver
  at all (its only SMMU driver is SMMUv2, on TX2). Choosing for features means the seL4 comparison
  stays on a separate machine, whichever one milestone 127 (the seL4 machine) settles on.

## What each top candidate trades away

Radxa Orion O6. It gives the full Armv9.2 set nife could adopt (PAC, BTI, SVE2, MTE with work),
GICv4-class interrupts with an ITS, SMMUv3, SystemReady SR and PL011 consoles. Its I/O is a
desktop's: a Gen4 x16 slot at x8, M.2 Gen4 x4, two 5GbE and USB 3.2. What it gives up:

- USB is outside the SMMU in the vendor device tree. A PCIe xHCI card in the x16 slot sits behind
  the PCIe hub's SMMU, so nife can confine USB by not driving the on-die controller. This is a
  configuration fix, and it costs a slot.
- The upstream device tree has no SMMU nodes, and the shipping boot path is ACPI. nife discovers
  its SMMU from a device tree today. So the port reads either the vendor tree or the ACPI IORT,
  and neither is upstream-reviewed.
- Firmware is CIX's forks of a 2022 EDK2 and of TF-A, with binaries under a distribution
  agreement. It cannot be rebuilt from upstream.
- Stock. Every variant was sold out at Radxa's main partner on the survey date, with no lead
  time stated.

Ampere Altra. It is the most complete isolation story on the list and the most standard firmware
(SystemReady SR, open EDK2 in tianocore's tree, a BMC and a PL011). It gives up every architecture
feature after v8.2, and it costs $4,099 or more, roughly fifteen to twenty times an O6. It is a
server platform that happens to come in a tower, so "a second board within an architecture is
cheap" (the first claim of milestone 241 (what a fourth board would have to be for)) is not what it would test.

RK3588 (Rock 5B+, Orange Pi 5 Plus). It is the cheapest board with an SMMUv3 and an ITS, and the
only one with upstream TF-A and mainline U-Boot both building it. The SMMU covers the PCIe-attached
NVMe and 2.5GbE NICs; USB, eMMC and the on-die MACs are uncovered. It is Armv8.2 with no PAC, BTI,
MTE or SVE. Its ITS needs non-shareable, non-coherent tables (erratum RK3588001), a quirk nife would
carry. Most RK3588 boards were sold out at list price on the survey date.

## Server hardware and cloud bare metal

Server Arm parts are the most standardized platforms (UEFI, ACPI, SBSA). They were scored on the
same criteria. A rented machine adds four more: how a custom OS boots, the serial console, cost per
bench hour, and how to reset a hung instance. The evidence is in
[the server appendix](aarch64-modern-board-survey/server-and-cloud.md).

- Whether an SMMUv3 is visible in front of the devices is unknown for every rentable machine. The
  best evidence is AWS's own DPDK text saying arm64 metal has the IOMMU on by default (written for
  Graviton1). On Altra, Ampere's reference firmware builds one SMMUv3 per PCIe root complex in the
  IORT. All of these are described by ACPI, never a device tree.
- AWS Graviton metal is the one rentable path that fits nife's needs on paper. It has UEFI with
  ACPI, Secure Boot off by default, an EC2 serial console an agent can drive from the CLI,
  per-second billing and an API reset. Its cores run from Armv8.4 (Graviton3) to Armv9.2 (Graviton5). Cost is $2.32
  per hour (`c7g.metal`) to $8.35 (`c9g.metal-48xl`) (secondary). AWS says a metal instance can
  take twenty minutes to become reachable, so it is a gate, not an inner loop.
- Oracle's Always Free tier is VMs only. Its bare metal (Altra, AmpereOne-M) exists but does not
  support bringing your own image. Azure Cobalt has no bare metal found. Google's C4A metal is real
  (Neoverse V2) and its serial console is scriptable, but Google documents nothing about its GIC,
  SMMU or custom-image boot. Grace rentals are priced per GPU-hour with no boot path found.
- A bought Altra board with a BMC (ASRock Rack ALTRAD8UD-1L2T, about $1.5k to $2.5k built, partly
  inferred) is the cheaper way to the Altra isolation story than a Thelio. It is still Armv8.2 with
  no PAC or BTI, and Serial over LAN is documented for none of the boards.
- seL4 has no platform for any of them, so no server machine gives a seL4 comparison either.

What each machine would carry beside the others:

| machine | carries |
|---|---|
| patagonia (HVF) | the inner loop and the test suite. It is virtual, so it shows no real SMMU, ITS or PMU behavior. What HVF does expose is being checked by another lane. |
| Orion O6, owned | the daily hardware bench: UART on the desk, a power switch, Armv9.2 features, and the O6's ACPI IORT as the first real SMMU description. |
| rented Graviton metal | a hardware gate with a real PCIe fabric and a core from a different vendor than the O6's. It also lets the IORT parser and SMMUv3 code run on silicon before the O6 can be bought. It does not replace an owned board: boots are slow and a bench run costs money. |

The milestones that wait on argon (74, 101, 25, 225 and 201) and whether a rented machine could carry
each are in [their own appendix](aarch64-modern-board-survey/rented-machines-and-waiting-milestones.md).

## Recommendation

Ranked, with the criteria that decided each place:

1. Radxa Orion O6. Decided by architecture level and the SMMUv3. It is the only board under $1,000
   with Armv9.2's PAC, BTI and SVE2, which fits calef's preference for requiring newer features.
   Its isolation gap (on-die USB) can be closed by configuration, with a PCIe xHCI card behind the
   SMMU. Altra's gap (no feature past v8.2) cannot be closed by anything.
2. Ampere Altra, as a Thelio Astra. Decided by isolation and firmware openness. Choose it over the
   O6 if an SMMUv3 in front of every DMA master with no configuration caveat outranks Armv9
   features, and if $4,099 is acceptable. The O6's lead over it is partly price. At equal cost the
   two would be close: Altra wins isolation and firmware outright, and the O6 wins every
   architecture feature.
3. Orange Pi 5 Plus or Radxa Rock 5B+. Decided by open firmware and cost. Choose it if rebuilding
   every firmware stage from upstream matters more than PAC, BTI and MTE.

Rented server hardware does not change that ranking. It adds a step, and it is not a board:

- Keep the O6 first for an owned machine.
- Rent AWS Graviton metal as a gate, not a replacement. Start with `c7g.metal` for one short run
  that records the IORT, `dmesg | grep -i smmu` and the IOMMU groups; the run costs about a dollar
  or two (inferred). It turns the survey's central unknown into a measurement. Then
  `c8g.metal-24xl` ($3.83 per hour) if Armv9.0's BTI and SVE2 matter. Opening an AWS account is
  calef's call.
- Probe Google C4A metal second, and phoenixNAP's Altra ($0.99 per hour) only after asking about
  console and custom boot.
- Skip Oracle (no custom image on bare metal), Azure (no bare metal), and Grace rentals (price,
  and no boot path found).
- A bought ALTRAD8UD-1L2T replaces the Thelio as the Altra option if Altra is wanted, and only
  after SOL is confirmed with ASRock Rack.

Not recommended, with the criterion that excluded each:

- Raspberry Pi 5: no IOMMU, and GICv2.
- Jetson Orin: SMMUv2 with one stream ID per PCIe controller, and no ITS.
- Jetson Thor: price, and an immature upstream tree.
- i.MX 95: Armv8.2 on A55s, and its SMMU disabled upstream.
- Genio and QCS6490: a vendor IOMMU or SMMUv2.
- DGX Spark: closed firmware, and nothing about its IOMMU published.

## To check before buying an Orion O6

- The ACPI IORT on the shipping firmware: which masters it puts behind which SMMU, and whether the
  syshub SMMU (disabled in the vendor dtsi) is usable for the GMACs.
- That a PCIe xHCI card in the x16 slot gets its own stream ID behind the PCIe hub's SMMU.
- The entry exception level from CIX's UEFI, and whether a device-tree boot is supported for a
  non-Linux payload.
- `PMCCNTR_EL0` readable at EL1 and EL0 under the shipped TF-A.
- MTE with a CIX-source BIOS, on the RAM size bought.
- A seller with stock, and the lead time.

## BUGS

- The SystemReady SR claim for the O6 rests on Radxa's certificate PDF; Arm's own device list
  returned 404 when read, and Arm has since moved SystemReady to self-declared compliance.
- Altra's on-die I/O is inferred from Ampere's brief listing none, not from a statement that the
  board's USB and NICs sit behind the SMMU.
- Several prices come from search snippets and are marked "(secondary)" in the appendices. All
  prices drifted with DRAM costs during 2026; re-check before ordering.
- The NVIDIA Orin and Thor TRMs need a developer login and were not read; their IOMMU facts come
  from the upstream device tree.
- GICv4 on the O6 and RK3588 is inferred from redistributor stride, not read from a GIC TRM.
- The server appendix's own BUGS list applies: no IORT from any rentable machine, and prices from
  aggregators.
