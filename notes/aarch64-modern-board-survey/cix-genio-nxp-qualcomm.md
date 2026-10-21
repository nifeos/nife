# CIX P1, MediaTek Genio, NXP i.MX 95, TI and Qualcomm boards, for the modern aarch64 board survey

Appendix to [the modern aarch64 board survey](../aarch64-modern-board-survey.md).

Read 2026-10-21 (UTC) unless marked. Per-core features (VHE, LSE, PAN, PAC, BTI, MTE, SVE, PMU)
come from Arm's technical reference manuals and live in [cores-and-sel4.md](cores-and-sel4.md).

Device tree (DT) evidence is cited at pinned commits:

- `L:` is `https://github.com/torvalds/linux/blob/a804c5742bff224d4cb4d25dd383554d598be32b/arch/arm64/boot/dts/` (Linux 7.3-rc).
- `T:` is `https://github.com/ARM-software/arm-trusted-firmware/blob/552f249c7313c5888955e8e57ca348f8ccfd88a9/` (TF-A).
- `U:` is `https://github.com/u-boot/u-boot/blob/62c9805d8d57cad407f0295879cc9a6182da464a/` (U-Boot).

An IOMMU covers a master only if the master's node carries `iommus`, or its PCIe host carries
`iommu-map`. A master with neither does untranslated DMA as far as the OS is told. GIC versions
read from redistributor stride are marked "(inference)". A stride of 0x20000 per CPU is the GICv3
two-frame layout; 0x40000 means the GICv4 vLPI and vSGI frames are present.

An MTE-capable core is not enough for MTE. The memory system must provide tag storage, and
firmware must enable it (SCR_EL3.ATA and a tag carve-out).

## Radxa Orion O6 (CIX P1 CD8180)

| item | fact |
|---|---|
| CPU | 4x A520 + 8x A720 in two clusters, Armv9.2; A720 up to 2.6 or 2.8 GHz, 12 MB L3 |
| GIC | `arm,gic-v3` + ITS at 0x0e050000; GICR 0x300000 for 12 CPUs, 0x40000 stride, GICv4 layout (inference); vendor DT says `arm,gic-700` |
| IOMMU | three SMMUv3 instances (syshub, mmhub, pciehub); see below |
| MTE | works with a self-built BIOS; see below |
| Boot | EDK2 UEFI + ACPI ships; DT also works; SystemReady SR v2.5 |
| Serial | four PL011 (`arm,pl011`); console uart2; serial console header on the board |
| PMU | separate `arm,cortex-a520-pmu` and `arm,cortex-a720-pmu` nodes; PMUv3p7; firmware gating not found |
| I/O | x16 slot wired Gen4 x8; M.2 M-key Gen4 x4; M.2 E-key Gen4 x2 + USB; 2x 5GbE; 4x USB 3.2 Gen2 (2 A, 2 C); 2x USB 2.0 |

Sources:

- CPU DT: L:cix/sky1.dtsi#L24-L146
- CPU clocks, sales via partners only: https://radxa.com/products/orion/o6/
- GIC DT: L:cix/sky1.dtsi#L736-L751
- Vendor GIC node: https://github.com/armbian/linux-cix/blob/2943eb37d2172fccd0246433ce934b6a98c28332/arch/arm64/boot/dts/cix/sky1.dtsi#L2397
- UARTs: L:cix/sky1.dtsi#L382-L414 ; console: L:cix/sky1-orion-o6.dts#L21
- PMU: L:cix/sky1.dtsi#L261-L267
- Product brief (I/O, console header): https://dl2.radxa.com/orion/o6/docs/radxa_orion_o6_product_brief.pdf

### SMMU coverage, vendor DT against mainline

Mainline DT has no SMMU node at all. Its five PCIe hosts carry `msi-map` only, with no
`iommu-map`. So under mainline DT nothing is translated.

The vendor DT (Armbian's `linux-cix` at commit 2943eb37) has three `arm,smmu-v3` nodes. Syshub and
mmhub are `status = "disabled"` in the dtsi; pciehub is enabled.

| instance | masters it covers in vendor DT | mainline |
|---|---|---|
| smmu_pciehub | all five PCIe root complexes (x8, x4, x2, x1, x1) via `iommu-map` | absent |
| smmu_syshub | both GMACs via `iommus` | absent |
| smmu_mmhub | display and NPU | absent |
| none | USB (Cadence USBSSP and USBHS): no `iommus` | absent |

Under ACPI the coverage is whatever CIX's IORT table says; the IORT is not found in public sources.
Users report heavy `arm-smmu-v3` log spam on recent kernels (secondary).

- Mainline PCIe: L:cix/sky1.dtsi#L609-L735
- Vendor SMMU nodes: https://github.com/armbian/linux-cix/blob/2943eb37d2172fccd0246433ce934b6a98c28332/arch/arm64/boot/dts/cix/sky1.dtsi#L505-L540
- Same vendor file: PCIe maps at L2122-L2390, GMACs at L1415 and L1436, USB at L1454-L2030.
- Log spam, idle power, desktop friction: https://github.com/geerlingguy/sbc-reviews/issues/62 (secondary)

### MTE

Radxa announced MTE on the O6, at first only through a BIOS built from the edk2-cix sources. The
kernel needs `CONFIG_ARM64_MTE` and `KASAN_HW_TAGS`. A user reports tag failures on 64 GB boards and
suspects the tag carve-out does not match the memory map; this is unconfirmed. PAC and BTI have no
reported firmware gating (not found otherwise).

- Announcement: https://forum.radxa.com/t/mte-feature-is-now-available-to-developers/28156 (secondary, vendor forum)
- 64 GB failures: https://forum.radxa.com/t/mte-on-64-gb-failures-explanation-cix-please-help/29781 (secondary)

### SystemReady certificate

The board holds SystemReady SR v2.5. The certificate lists SRS 2.2, ACS 2.0.0, and BSA 1.0c with
SBSA level 6, plus BBR/SBBR. Firmware is given as "UEFI 9.0.0, ATF v2.7". Testing ran with only the
eight A720 cores enabled. A listing on Arm's own device list is not confirmed.

- Certificate: https://dl.radxa.com/orion/o6/images/bios/SystemReady/240411/arm-systemready-sr-certification-radxa_sz.pdf
- Report: https://www.cnx-software.com/2025/04/30/radxa-orion-o6-mini-itx-motherboard-gets-arm-systemready-sr-v2-5-certification/ (secondary)

### Firmware openness

Firmware lives in vendor forks, not upstream. CIX publishes EDK2 based on a 2022 edk2 commit,
with O6 platform code under `edk2-platforms/Platform/Radxa/Orion/O6`. A binary `edk2-non-osi`
repository sits beside it, and CIX also keeps a TF-A fork. Upstream TF-A has no `cix` or `sky1`
platform, and U-Boot has no CIX board doc.

Radxa states the SystemReady BIOS comes from CIX and Radxa has no source for it. Some repositories
and binaries are withheld under a binary distribution agreement. A reviewer reports
`bootloader1.img` is closed (secondary).

- BIOS source: https://github.com/cixtech/bios
- TF-A fork in the org listing: https://github.com/cixtech
- Upstream absence: T:plat and U:doc/board
- Withheld code: https://forum.radxa.com/t/initial-code-release-of-orion-o6/26057 (secondary)

### Mainline status

Per CIX's wiki, UART landed in 6.17-rc1 and PCIe in 6.19-rc1. USB is under review, and the on-chip
GMAC is marked "won't do". Mainline DT was initramfs-level at first merge.

- https://github.com/cixtech/linux-mainline/wiki
- https://github.com/torvalds/linux/commits/master/arch/arm64/boot/dts/cix/sky1.dtsi

### Price and stock

Arace lists $226.90 on sale (regular $289), with every variant sold out. Launch pricing was "from
$199" for 8 GB (secondary). One reviewer paid $251 for 16 GB and $299 for 32 GB. Lead time is not
stated.

- https://arace.tech/products/radxa-orion-o6
- https://www.notebookcheck.net/Orion-O6-Open-source-mainboard-offers-numerous-ports-expansion-options-and-fast-NPU.937226.0.html (secondary)
- Reviewer prices: the sbc-reviews issue above.

### Desktop use

Enthusiasts use it as a desktop, with friction (secondary). Idle draw is 14.2 W. Reports include
HDMI issues and shutdown or reboot regressions on 6.19 and later. ACPI mode hides the GPU on stock
Ubuntu. CIX recommends Debian 13 with its own 6.6 or 7.0 kernels. Source: the sbc-reviews issue
above.

## Other CIX P1 boards

- Radxa Orion O6N uses the CD8160: A720 at 2.6 GHz, 2.5GbE instead of 5GbE, Nano-ITX, EDK2 UEFI. https://www.cnx-software.com/?p=158462 (secondary)
- Mainline has no O6N DTS; L:cix/ holds only `sky1-orion-o6.dts` and `sky1-xcp.dts`. SystemReady for the O6N: not found.
- O6N price: Arace pre-order, only 32 GB orderable at $649, others sold out. https://arace.tech/products/radxa-orion-o6n
- O6N China launch: 1,499 to 1,599 yuan. https://www.ithome.com/0/888/873.htm (secondary)
- Orange Pi 6 Plus: $223.90 (16 GB) and $268.89 (32 GB) on AliExpress in 2025. https://www.cnx-software.com/category/processors/cix/ (secondary)
- Orange Pi 6 (CD8180, June 2026): $239 to $569; OS image folders were empty at publication. https://www.cnx-software.com/2026/06/25/orange-pi-6-cix-cd8180-12-core-arm-sbc-gets-2-5gbe-networking-smaller-form-factor-drops-battery-support/ (secondary)
- Minisforum MS-R1: a P1 mini PC; price not found.
- These share the O6's CPU, GIC and IOMMU. Each vendor builds CIX's BIOS; per-vendor builds are not verified.

## MediaTek Genio

All Genio parts below use A78 + A55 cores (Armv8.2), a vendor M4U IOMMU rather than Arm SMMU, and
a 16550-type UART (`mediatek,mt6577-uart`) with the console at 921600. GIC is `arm,gic-v3` with
no ITS. UEFI, ACPI and SystemReady: not found.

| part | cores | IOMMU coverage | boot | board price |
|---|---|---|---|---|
| Genio 1200 (MT8395) | 4x A78 2.2 GHz + 4x A55 2.0 GHz | PCIe via `iommu-map`, multimedia; Ethernet, xHCI/MTU3, MSDC, UFS have no `iommus` | mainline U-Boot defconfig; closed DDR loader | NIO 12L $99, sold out; EVK not found |
| Genio 700 (MT8390) | 2x A78 2.2 GHz + 6x A55 2.0 GHz | PCIe via `iommu-map`; Ethernet, xHCI, MMC uncovered | BootROM, closed DDR loader, U-Boot SPL, BL31, OP-TEE, U-Boot | EVK EUR 1,135.99, 13 to 16 weeks |
| Genio 510 (MT8370) | 2x A78 + 4x A55, 2.0 GHz | as Genio 700 | as Genio 700 | EVK not found |
| Genio 720 / 520 (MT8391 / MT8371) | 2x A78 + 6x A55; 2.6 or 2.2 GHz | no Linux DT; not found | U-Boot defconfigs; MT8189 family | 720 EVK about EUR 1,230; NIO 5A not found |
| Genio Pro 5100 (MT8894) | X925 + 3x X4 + 4x A720, Armv9.2 | not found | not found | not found |

Genio 1200 sources:

- CPU, GIC, IOMMU, UART, PMU: L:mediatek/mt8195.dtsi#L57-L192 , #L465-L473 , #L932 , #L1062 , #L313-L438
- Uncovered masters: same file, Ethernet L1301, USB L1394-L1584, MSDC L1430-L1458, UFS L1473; PCIe maps L1611, L1660.
- Specs: https://genio.mediatek.com/genio-1200
- U-Boot: U:configs/mt8395_genio_1200_evk_defconfig ; TF-A documents only the Chromebook flow: T:docs/plat/mt8195.rst
- NIO 12L: https://docs.radxa.com/en/nio/12l ; https://arace.tech/products/radxa-nio-12l
- Canonical-certified Ubuntu, five years of updates: https://www.cnx-software.com/2024/04/11/radxa-nio-12l-low-profile-mediatek-genio-1200-sbc-ubuntu-certification-5-years-software-updates/ (secondary)

Genio 700 and 510 sources:

- DT: L:mediatek/mt8188.dtsi#L66-L201 , GIC #L939 , IOMMU #L1388 , PCIe map #L2025 ; Ethernet L1736, xHCI L1724, MMC L1834-L1861.
- Boot chain and closed loader: U:doc/board/mediatek/mt8188-genio-evk.rst ; TF-A: T:docs/plat/mt8188.rst
- Whether Genio's BL31 is the upstream port or a MediaTek fork: not found.
- Specs: https://genio.mediatek.com/genio-700 ; https://genio.mediatek.com/genio-510
- Price: https://www.digikey.es/en/products/detail/mediatek/GENIO-700-EVK/24633568 (secondary)

Genio 720, 520 and Pro sources:

- https://genio.mediatek.com/genio-720 ; U:configs/mt8391_genio_720_evk_defconfig ; T:docs/plat/mt8189.rst
- Mainline has no `mt8189.dtsi` at the pinned commit (L:mediatek/ listing).
- EVK price: https://www.mouser.bg/c/embedded-solutions/?pg=5 (secondary)
- NIO 5A: https://www.cnx-software.com/2025/05/20/radxa-nio-5a-credit-card-sized-mediatek-genio-520-sbc-showcased-at-computex-2025/ (secondary)
- Genio Pro: https://www.cnx-software.com/?p=166840 (secondary); docs reportedly under NDA.

Genio boards are IoT and HMI parts; desktop use: not found.

## NXP i.MX 95 and i.MX 8M Plus

| item | i.MX 95 (EVK, FRDM-IMX95, FRDM-IMX95-PRO) | FRDM i.MX 8M Plus (contrast) |
|---|---|---|
| CPU | 6x A55 (+ M7, M33), Armv8.2 | 4x A53, Armv8.0 |
| GIC | `arm,gic-v3` + ITS; 0x20000 stride, GICv3 layout (inference) | `arm,gic-v3`, no ITS |
| IOMMU | Arm MMU-700 (SMMUv3), disabled in mainline | none |
| Boot | upstream TF-A `PLAT=imx95` + ELE blob; mainline U-Boot | upstream TF-A; no FRDM U-Boot defconfig |
| Serial | LPUART (`fsl,imx95-lpuart`) | i.MX UART (`fsl,imx8mp-uart`) |
| I/O | 2x PCIe Gen3 x1, USB 3.0 + 2.0, 10GbE + 2x GbE | PCIe Gen3 x1, 2x USB 3.0, 2x GbE |
| Price | FRDM-PRO $300, pending stock; FRDM 15-week lead | about $221 to $232 |

On the i.MX 95 the SMMU covers USB3, USB2, both PCIe hosts and NETC Ethernet. uSDHC and eDMA have
no `iommus`. The node is `status = "disabled"`, and neither the EVK nor FRDM DTS enables it. NXP
posted an RFC to bypass StreamID 0 because eDMA3 goes through a TBU. The M33 runs the imx-sm system
manager. SystemReady is not found for i.MX 95; NXP marks SystemReady-IR only for i.MX 8M EVK images.

i.MX 95 sources:

- DT: L:freescale/imx95.dtsi#L42-L132 , GIC #L534-L552 , SMMU #L1773-L1783 , LPUART #L911
- Same file: USB L1904, L1928; PCIe L1988, L2081; NETC L2230; uSDHC L1242-L1276; eDMA L634, L709, L1366.
- Boards: L:freescale/imx95-19x19-evk.dts , L:freescale/imx95-15x15-frdm.dts
- MMU-700 named in commits: https://github.com/torvalds/linux/commits/master/arch/arm64/boot/dts/freescale/imx95.dtsi
- StreamID RFC: https://lkml.iu.edu/hypermail/linux/kernel/2410.1/10592.html
- Boot: T:docs/plat/imx9.rst ; U:doc/board/nxp/imx95_evk.rst ; U:doc/board/nxp/imx95_frdm.rst
- Specs: https://www.nxp.com/products/i.MX95
- SystemReady note: https://www.nxp.com/design/design-center/software/embedded-software/i-mx-software/embedded-linux-for-i-mx-applications-processors:IMXLINUX
- FRDM-IMX95-PRO (two M.2, 10GbE, USB-C debug UART, $300): https://www.cnx-software.com/2026/08/11/nxp-frdm-imx95-pro-i-mx-95-board-features-10gbe-faster-6400-mt-s-lpddr5-memory-dual-m-2-expansion/ (secondary)
- FRDM-IMX95 stock: https://www.digikey.com.br/en/products/detail/nxp-usa-inc/FRDM-IMX95/29285213 (secondary); unit price not found.

i.MX 8M Plus sources:

- DT: L:freescale/imx8mp.dtsi#L68 , GIC #L2400 , UART #L1121 ; console L:freescale/imx8mp-frdm.dts#L15
- TF-A: T:docs/plat/imx8m.rst
- Price: https://www.avnet.com/americas/product/nxp/frdm-imx8mplus/evolve-120302042/ (secondary)
- Price: https://www.futureelectronics.com/p/development-tools--microcontroller-microprocessor--32-bit-eval-board/frdm-imx8mplus-nxp-8205681 (secondary)

Toradex, Variscite, TQ and PHYTEC i.MX 95 modules have mainline DTs (L:freescale/ listing); prices
not collected.

## TI AM62 and AM69 (excluded)

AM62 uses A53 and AM69 uses A72, both Armv8.0, with GICv3 and ITS. Neither mainline dtsi has an
`arm,smmu*` node or any `iommus`. TI uses its own Peripheral Virtualization Unit, and its TRM
section for AM69 was reported empty (secondary). TF-A and U-Boot support is strong.

- L:ti/k3-am62-main.dtsi#L18-L38 ; L:ti/k3-j784s4-j742s2-main-common.dtsi#L202-L220
- https://e2e.ti.com/support/processors-group/processors/f/processors-forum/1470371/tda4vh-q1-documentation-issues (secondary)
- T:docs/plat/ti-k3.rst ; U:doc/board/ti/j784s4_evm.rst

## Qualcomm

| item | Snapdragon X Elite (X1E80100) | QCS6490 (RB3 Gen 2, RUBIK Pi 3, Dragon Q6A) | IQ-9075, IQ-8275 |
|---|---|---|---|
| CPU | 12x Oryon (custom); Armv8.7-A reported; PAC, BTI seen; SVE, MTE not found | 8x Kryo 670 (A78/A55-derived), Armv8.2 | A78C + A55 class, Armv8.2 |
| GIC | GICv3, 0x40000 stride (inference), ITS | GICv3, 0x20000 stride (inference), ITS disabled | GICv3, no ITS |
| IOMMU | MMU-500 (SMMUv2) for USB, UFS, SDHC; SMMUv3 for PCIe reserved by Gunyah | MMU-500 (SMMUv2) for PCIe, UFS, SDHC, USB | MMU-500 |
| Boot | closed UEFI + ACPI; Linux uses DT; Gunyah at EL2 | closed PBL/XBL, then TF-A port or mainline U-Boot | TF-A ports, QTI-signed BL2 |
| Serial | Qualcomm GENI | Qualcomm GENI | not checked |
| Price | Dev Kit ($899) cancelled | RUBIK Pi 3 $179 list; Q6A $95, sold out | not found |

X Elite notes. At EL1, PCIe is translated by the hypervisor, not the OS; the PCIe nodes carry no
`iommu-map`. The `x1-el2.dtso` overlay enables the SMMUv3 and maps PCIe3 to PCIe6a, and adds
`msi-map` because ITS emulation under Gunyah is broken for some controllers. Upstream TF-A has no X1
port and U-Boot has no X1 board doc. SystemReady: not found. Mainline carries about 30 X1 laptop DTs.
The X2 Elite (Glymur) adds SME (secondary).

- DT: L:qcom/hamoa.dtsi#L72-L213 , GIC #L8535-L8558 , SMMU #L8412 , #L8520-L8532 , PMU #L372
- Overlay: L:qcom/x1-el2.dtso ; console: L:qcom/x1e001de-devkit.dts#L49
- ISA: https://en.wikipedia.org/wiki/Oryon (secondary)
- cpuinfo flags: https://openbenchmarking.org/s/Snapdragon%20X%20Elite%20X1E78100%20Qualcomm%20Oryon (secondary)
- X2: L:qcom/glymur.dtsi#L43 ; https://signal65.com/research/snapdragon-x2-elite-and-x2-elite-extreme-raising-the-bar-for-ai-pcs/ (secondary)
- Dev Kit cancelled: https://www.tomshardware.com/desktops/mini-pcs/qualcomm-abruptly-cancels-snapdragon-x-elite-dev-kit-refunds-customers-for-mini-pc-ends-sales-and-support-for-the-device-immediately (secondary)
- IQ-X EVK, no public price: https://docs.qualcomm.com/bundle/publicresource/80-80023-261/topics/iqx-ug-introduction.md

QCS6490 notes. The TF-A `rb3gen2` port replaces Qualcomm's TZ image. It still needs the binary
`qtiseclib` and a QTI-signed BL2. Mainline U-Boot replaces Qualcomm's EDK2 in `uefi_a` on RUBIK Pi 3
and RB3 Gen 2. Desktop use: not found.

- DT: L:qcom/kodiak.dtsi#L207-L382 , ITS #L6899-L6914 , SMMU #L6733 , UART #L1462
- TF-A: T:docs/plat/qti/rb3gen2.rst
- U-Boot: U:doc/board/qualcomm/rubikpi3.rst ; U:doc/board/qualcomm/rb3gen2.rst
- RUBIK Pi 3: https://www.thundercomm.com/rubik-pi-3-ces-2025/ ; https://www.thundercomm.com/rubik-pi-3-99/
- Dragon Q6A: https://arace.tech/products/radxa-dragon-q6a

IQ-9075 and IQ-8275 sources:

- L:qcom/monaco.dtsi#L52 , #L5065 ; L:qcom/lemans.dtsi#L4763
- T:docs/plat/qti/monza.rst ; T:docs/plat/qti/lemans_evk.rst

## Other Armv9 hardware

- Rockchip RK3688 and RK3668 (Armv9.3): announced, no buyable board found. https://www.cnx-software.com/2024/10/04/rockchip-rk3688-armv9-3-aiot-processor-to-feature-a-16-tops-npu-ufs-4-0-storage/ (secondary)
- Samsung Exynos dev boards: not found.
- NVIDIA Jetson Thor and DGX Spark are covered in another appendix.
