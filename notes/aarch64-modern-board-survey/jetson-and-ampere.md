# Jetson, DGX Spark and Ampere machines, for the modern aarch64 board survey

Appendix to [the modern aarch64 board survey](../aarch64-modern-board-survey.md).

Read 2026-10-21 (UTC) unless marked.

A source marked (secondary) is neither the vendor nor the upstream tree. A fact marked not found
had no primary answer; that does not mean the answer is no.

Core features (VHE, LSE, PAC, BTI, MTE, SVE) per core live in
[the core table](cores-and-sel4.md); each machine below gets one line for its cores.

Pinned-commit shorthand: `L:` = `https://github.com/torvalds/linux/blob/a804c5742bff224d4cb4d25dd383554d598be32b/arch/arm64/boot/dts/`
(torvalds/linux at a 7.3-rc commit).

Short names for sources used by more than one machine:

| short name | what | URL |
|---|---|---|
| T234-DT | mainline `tegra234.dtsi` | https://github.com/torvalds/linux/blob/master/arch/arm64/boot/dts/nvidia/tegra234.dtsi |
| T264-DT | mainline `tegra264.dtsi` | https://github.com/torvalds/linux/blob/master/arch/arm64/boot/dts/nvidia/tegra264.dtsi |
| NV-DTS | mainline NVIDIA DT directory | https://github.com/torvalds/linux/tree/master/arch/arm64/boot/dts/nvidia |
| TFA-tegra | upstream TF-A Tegra docs and SoC dirs | https://github.com/ARM-software/arm-trusted-firmware/blob/master/docs/plat/nvidia-tegra.rst , https://github.com/ARM-software/arm-trusted-firmware/tree/master/plat/nvidia/tegra/soc |
| UBOOT-tegra | mainline U-Boot `mach-tegra` | https://github.com/u-boot/u-boot/tree/master/arch/arm/mach-tegra |
| EDK2-NV | NVIDIA's edk2-nvidia UEFI | https://github.com/NVIDIA/edk2-nvidia |
| SR-LIST | Arm SystemReady past certifications | https://www.arm.com/architecture/system-architectures/systemready-compliance-program/systemready-past-certifications |
| JETSON-FAQ | NVIDIA Jetson FAQ, US list prices | https://developer.nvidia.com/embedded/faq |
| V3AE-TRM | Arm Neoverse V3AE TRM r0p2 | https://documentation-service.arm.com/static/67361832c7fc0d1f211d9e66 |

Two notes apply to every machine:

- SystemReady is now a compliance model rather than a certification program, so SR-LIST is historical. SR-LIST
- Upstream TF-A leaves MDCR_EL3.TPM clear, so PMU access does not trap to EL3. https://github.com/ARM-software/arm-trusted-firmware/blob/master/include/arch/aarch64/arch.h . No vendor firmware below documents its MDCR_EL3 setting: not found for every machine.

## Jetson Orin Nano Super Developer Kit (Tegra234)

| topic | fact |
|---|---|
| cores | 6x Cortex-A78AE, Armv8.2-A plus PAC; no BTI, MTE or SVE [1] [9] |
| PMU | PMUv3 (PMUVer 0x4); DT node `arm,cortex-a78-pmu` [1] [2] |
| GIC | `arm,gic-v3`; no ITS node; PCIe uses the DesignWare controller's own MSI [2]. Distributor model: not found (the Orin TRM needs a developer login) |
| IOMMU | three Arm MMU-500 (SMMUv2) instances, `nvidia,tegra234-smmu` [2] |
| IOMMU coverage | `iommus` or `iommu-map` on GPCDMA, audio, QSPI, SPI, SDMMC, HD Audio, XUSB host and device, all four MGBE, host1x and every PCIe controller [2] |
| PCIe isolation | each controller has `iommu-map-mask = <0x0>`, so every requester ID behind one root port gets one stream ID; devices behind a switch cannot be isolated from each other [2] |
| board NIC | GbE sits on a PCIe controller (no MGBE enabled), so it shares that controller's single stream ID [3]. NIC chip: not found |
| boot chain | BootROM, PSCROM, MB1 (on BPMP), MB2, UEFI [4]. Where TF-A and OP-TEE run: not found there |
| firmware openness | UEFI open (EDK2-NV) [5]. Upstream TF-A covers only Tegra194; mainline U-Boot stops at Tegra186 [6] [7]. NVIDIA's own TF-A source availability: not checked |
| DT or ACPI | boots Linux with a DTB through L4TLauncher; UEFI Secure Boot supported [5]. edk2-nvidia also carries a T234 DSDT, an IORT generator and an ACPI boot overlay [8]; ACPI mode reported less complete (secondary) [10] |
| SystemReady | no Jetson entry in any band [6]. BSA or SBSA level: not found |
| mainline Linux DTs | Nano dev kit (p3768+p3767) and AGX Orin (p3737+p3701) [11] |
| console | 3.3 V UART2 on header J14 pins 3, 4 and 11 [12]. DT `stdout-path` is the Tegra Combined UART, a mailbox-backed virtual UART; the SoC also has a 16550-style UARTA and a PL011 UARTI [2] [3]. Which UART reaches J14: not found |
| peripherals | M.2 Key-M 2280 (PCIe 3.0 x4) and 2230 (x2), M.2 Key-E Wi-Fi, 4x USB 3.2 Type-A at 10 Gbps, USB-C, GbE, DisplayPort, microSD [13] |
| price | $399 list, up from $249 [6] [14]; SparkFun $399, page shows both in stock and backorder [15]; Yahboom $369 for the 4 GB variant (secondary) [16] |
| desktop use | ships Ubuntu desktop; credible daily-use reports: not found |

Sources:

1. A78AE TRM r0p2 (core features, PMUVer): https://documentation-service.arm.com/static/62502fb164e3265f7c90b120
2. T234-DT
3. https://github.com/torvalds/linux/blob/master/arch/arm64/boot/dts/nvidia/tegra234-p3768-0000+p3767.dtsi
4. https://docs.nvidia.com/jetson/archives/r39.2.1/DeveloperGuide/AR/BootArchitecture/JetsonOrinSeriesBootFlow.html
5. https://docs.nvidia.com/jetson/archives/r39.2.1/DeveloperGuide/SD/Bootloader/UEFI.html , EDK2-NV
6. TFA-tegra, SR-LIST, JETSON-FAQ (by row)
7. UBOOT-tegra
8. https://github.com/NVIDIA/edk2-nvidia/blob/main/Silicon/NVIDIA/Drivers/ConfigurationManagerData/AcpiTableList/PlatformASLTablesLib/Dsdt_T234.asl , https://github.com/NVIDIA/edk2-nvidia/tree/main/Silicon/NVIDIA/Drivers/ConfigurationManagerData/Iort , https://github.com/NVIDIA/edk2-nvidia/blob/main/Silicon/NVIDIA/Tegra/DeviceTree/T234AcpiWinBoot.dts
9. AGX Orin `lscpu` showing `paca pacg`, no `bti` (secondary): https://forums.developer.nvidia.com/t/agx-orin-64gb-cpus-is-less-than-12/366792
10. https://nullr0ute.com/tag/jetson/ (secondary)
11. NV-DTS
12. https://docs.nvidia.com/jetson/archives/r38.2/DeveloperGuide/AT/BoardAutomation.html
13. https://docs.nvidia.com/jetson/orin-nano-devkit/user-guide/latest/hardware_layout.html
14. Price rise reported 2026-07-21 (secondary): https://hwbusters.com/news/nvidia-jetson-prices-jump-up-to-101-the-249-orin-nano-super-is-now-399/
15. https://www.sparkfun.com/nvidia-jetson-orin-nano-developer-kit.html
16. https://category.yahboom.net/collections/jetson/products/jetson-orin-nano (secondary, search snippet)

## Jetson AGX Orin Developer Kit, 64 GB (Tegra234)

Same SoC as the Orin Nano, so GIC, IOMMU, boot and firmware match the table above.

| topic | fact |
|---|---|
| cores | 12x Cortex-A78AE; flags include `atomics` and `paca pacg`, no `bti`, `mte` or `sve` (secondary) [1] |
| Ethernet | SoC MGBE at `10gbase-r`, with its own stream ID [2] |
| console | micro-B J26 enumerates four `/dev/ttyACM*` ports; console is UART3 at 115200 [3] [4]. JTAG header J502 [3] |
| peripherals | PCIe x16 slot (PCIe 4.0, 8 lanes electrical), M.2 Key-M and Key-E, USB 3.2 Gen 2 Type-C and Type-A, 10GBASE-T, DisplayPort with MST [3] |
| price | $3,499 list, up from $1,999 (JETSON-FAQ); SparkFun $3,499 on backorder [5]. An undated aggregator capture showed Arrow near $1,999 with a 4-week lead, predating the rise (secondary) [6] |
| desktop use | not found |

Sources:

1. https://forums.developer.nvidia.com/t/agx-orin-64gb-cpus-is-less-than-12/366792 (secondary)
2. https://github.com/torvalds/linux/blob/master/arch/arm64/boot/dts/nvidia/tegra234-p3737-0000+p3701.dtsi
3. https://docs.nvidia.com/jetson/agx-orin-devkit/user-guide/latest/hardware_layout.html
4. https://docs.nvidia.com/jetson/archives/r35.1/DeveloperGuide/text/AT/BoardAutomation.html
5. https://www.sparkfun.com/nvidia-jetson-agx-orin-64gb-developer-kit.html
6. https://thepartsdirect.com/part/945137300050000~nvidia (secondary)

## Jetson AGX Thor Developer Kit (Tegra264, P3971)

Shipping.

| topic | fact |
|---|---|
| cores | 14x Neoverse V3AE (Armv9.2-A) in the T5000 module [1] [2]. Whether Thor ties BROADCASTMTE high to enable MTE: not found |
| PMU | PMUv3p7 [3]; DT node `arm,armv8-pmuv3` [2] |
| GIC | `arm,gic-v3` with an `arm,gic-v3-its` child; PCIe `msi-map` targets the ITS [2] [4]. Redistributor stride 0x40000 suggests the GICv4 frame layout (inference) [4]. GICv4.x confirmed: not found (Thor TRM needs a login) |
| IOMMU | at least four SMMUv3 instances (`nvidia,tegra264-smmu`, `arm,smmu-v3`) plus `nvidia,tegra264-cmdqv` command-queue virtualization [2] [5] |
| IOMMU coverage | PCIe `iommu-map` spans the full 0x10000 requester-ID range with no mask, so devices behind a switch get distinct stream IDs [2] [6]. GPCDMA, audio, HD Audio and host1x have `iommus` [2] |
| coverage gap | mainline has no USB, MGBE, UFS or SDMMC nodes yet, so those masters cannot be checked; the latest `tegra264.dtsi` change (2026-08-31) added PCIe root ports [7] |
| boot chain | PSCROM, MB1, HPSEROM and SBROM, MB2, UEFI [8] |
| firmware openness | UEFI open (EDK2-NV), with T264 code and a T264 DSDT [9]. No upstream TF-A or U-Boot (TFA-tegra, UBOOT-tegra) |
| SystemReady | not listed (SR-LIST) |
| mainline Linux DTs | dev kit (p3971-0089 + p3834) enables only UARTs, GPCDMA, SMMUs, audio, HD Audio, PWM and fan [10] [11] |
| console | debug USB-C behind the lid cover [12]. Mainline console is `nvidia,tegra264-utc`, an NVIDIA-specific UART block; an `arm,sbsa-uart` exists but is disabled [10] [13] |
| peripherals | 2x USB 3.2 Gen 2 Type-A, 2x USB-C at 5 Gbps (one for recovery), 5 Gbps RJ45, QSFP28 (4x 25 GbE), DisplayPort, HDMI, M.2 Key-M with 1 TB NVMe, no PCIe slot [12]. M.2 lane count and generation: not stated |
| price | $5,499 list, up from $3,499 at the August 2025 launch (JETSON-FAQ). RS UK £2,820.99 ex VAT and RS Ireland €3,389.42, likely pre-rise (secondary) [14]. Current US retailer page: not found |
| desktop use | not found |

Sources:

1. https://docs.nvidia.com/jetson/agx-thor-devkit/user-guide/latest
2. T264-DT
3. V3AE-TRM
4. L:nvidia/tegra264.dtsi#L3895-L3918
5. L:nvidia/tegra264.dtsi#L3418 (further instances at L3438, L3511, L3531, L3994)
6. L:nvidia/tegra264.dtsi#L4092-L4260
7. https://github.com/torvalds/linux/commits/master/arch/arm64/boot/dts/nvidia/tegra264.dtsi
8. https://docs.nvidia.com/jetson/archives/r39.2.1/DeveloperGuide/AR/BootArchitecture/JetsonThorBootFlow.html
9. https://github.com/NVIDIA/edk2-nvidia/blob/main/Silicon/NVIDIA/Drivers/ConfigurationManagerData/AcpiTableList/PlatformASLTablesLib/Dsdt_T264.asl
10. https://github.com/torvalds/linux/blob/master/arch/arm64/boot/dts/nvidia/tegra264-p3971-0089+p3834.dtsi
11. NV-DTS
12. https://docs.nvidia.com/jetson/agx-thor-devkit/user-guide/latest/hardware_layout.html
13. L:nvidia/tegra264.dtsi#L3323
14. https://uk.rs-online.com/web/p/processor-development-tools/0606865 (secondary, search snippet)

## NVIDIA DGX Spark and partner GB10 boxes

| topic | fact |
|---|---|
| cores | 10x Cortex-X925 plus 10x Cortex-A725 (Armv9.2-A) [1]. Shipped kernel reports `sve`, `sve2`, `paca pacg` and `bti` but no `mte`, so MTE is off as shipped [2] |
| PMU | PMUv3p7 on both cores [3] |
| GIC | version and ITS: not found |
| IOMMU | type and IORT coverage: not found |
| firmware | AMI Aptio V UEFI [4]; signed capsule updates through LVFS; no ACPI, DT, SBSA or SystemReady detail in the porting guide [5] [6] |
| firmware stack | SoC firmware bundles UEFI, GPU init and a TrustZone partition reached over FF-A; the EC runs Zephyr (secondary) [7] |
| firmware openness | no open firmware and no upstream TF-A, U-Boot or EDK2 platform found |
| SystemReady | GB10 not listed; only Grace, GH200 and GB200 reference systems hold SR (SR-LIST) |
| console | no serial console or BMC listed [1]; not found elsewhere |
| peripherals | 1 or 4 TB M.2 NVMe, 4x USB-C (up to 3 with DP Alt Mode), 10 GbE RJ45, ConnectX-7 with 2x QSFP at 200 Gb/s, Wi-Fi 7, HDMI 2.1a [1] [8]. The Realtek Ethernet driver needed a patch under Fedora (secondary) [9] |
| price | Founders Edition MSRP rose from $3,999 to $4,699 the week of 2026-02-23 [10]. Out of stock at NVIDIA; Micro Center via Amazon at $4,999.99 (secondary, search snippet) [11]. A 64 GB variant and a $6,995 128 GB price reported (secondary, unverified) [12] |
| partner boxes | Acer Veriton GN100, ASUS Ascent GX10, Dell Pro Max, Gigabyte AI TOP ATOM, HP ZGX Nano, Lenovo, MSI [8]. ASUS GX10 1 TB from about $3,056 at Newegg, undated (secondary) [13] |
| desktop use | yes: a disclosed NVIDIA employee runs it daily on Ubuntu and Fedora (secondary) [9]. Reviews flag price and some stability reports (secondary) [14] |

Sources:

1. https://docs.nvidia.com/dgx/dgx-spark/hardware.html
2. SPEC CPU 2026 result submitted by Arm, Ubuntu 6.14.0-1013-nvidia: https://www.spec.org/cpu2026/results/res2026q2/cpu2026-20260210-00021.txt
3. X925 TRM https://documentation-service.arm.com/static/68374ea93f2acc596c2d9d93 , A725 TRM https://documentation-service.arm.com/static/6749c8a127eda361ad4dd099
4. https://www.ami.com/resources/amis-aptio-v-uefi-firmware-selected-for-the-worlds-smallest-ai-supercomputer-the-nvidia-dgx-spark-personal-ai-computer/
5. https://docs.nvidia.com/dgx/dgx-spark-porting-guide/latest/porting/software-requirements.html
6. https://docs.nvidia.com/dgx/dgx-spark-uefi/
7. https://forums.developer.nvidia.com/t/the-dgx-spark-firmware-stack/384721 (secondary)
8. https://www.nvidia.com/en-us/products/workstations/dgx-spark/
9. https://daniel.lawrence.lu/blog/2026-07-15-dgx-spark-as-daily-driver/ (secondary)
10. https://forums.developer.nvidia.com/t/2-23-2026-price-change-announcement/361713
11. https://windowsreport.com/nvidia-dgx-spark-hits-5000-ahead-of-the-rtx-spark-launch/ (secondary)
12. https://hothardware.com/news/nvidia-dgx-spark-64gb-release (secondary)
13. https://www.newegg.com/p/pl?d=gx10 (secondary)
14. https://www.techradar.com/pro/a-winner-but-you-need-to-be-all-in-on-ai-to-justify-one-the-first-nvidia-dgx-spark-reviews-are-in-could-this-be-your-next-dream-mini-pc (secondary)

## Ampere Altra: ADLINK AADP and System76 Thelio Astra

| topic | fact |
|---|---|
| cores | Neoverse N1, Armv8.2-A; no PAC, BTI, MTE or SVE [1]. Ampere lists Armv8.2+ and SBSA level 4 [2] |
| PMU | PMUv3, PMUVer 0x4 [1] |
| GIC | GICv3 [2]; the open UEFI writes an MADT with GicVersion 3 and ITS entries [3] |
| IOMMU | SMMUv3 with SR-IOV [2]. The IORT has one ITS group and one SMMUv3 per enabled root complex, mapping requester IDs 0 to 0xFFFF, plus PMCG nodes [4] |
| IOMMU coverage | the brief lists no on-die USB or Ethernet, so board NICs and USB sit on PCIe behind a per-root-complex SMMUv3 (inference from [2] and the board lists) |
| boot | UEFI and ACPI |
| firmware openness | UEFI open in edk2-platforms (JadePkg, AmpereAltraPkg) [5]. TF-A and SCP are signed binaries, and upstream TF-A has no Ampere platform [6]. Thelio Astra runs "System76 firmware (non-open)" [7] |
| SystemReady | ADLINK AADP-32/64/128 and AVADP-32 listed SR; Ampere Mt. Jade is SR and the first LS (SR-LIST) |
| console | SoC UART0 is a PL011 at 115200 [8]. AADP: DB9 COM port, RJ45 BMC console and an IPMI BMC on its own Ethernet [9] [10]. Thelio Astra (ASRock Rack ALTRAD8UD-1L2T): AST2500 BMC with IPMI GbE [7], KVM documented but not serial-over-LAN [11], 9-pin COM header (secondary) [12] |
| peripherals | 128 lanes PCIe Gen4 per socket, 8-channel DDR4-3200 ECC [2]. Thelio Astra: 4x PCIe 4.0 x16 slots (two at x8 with non-Max CPUs), 2x M.2 Gen4, 4x USB 3.2 Gen 1 Type-A, 2x 10GbE (X550) plus 2.5GbE (i210) or 2x 25GbE SFP28 on rev 1, VGA from the BMC only [7]. AADP-128: 3x Gen4 x16 and 4x x4, BCM57502 4x10G [10] |
| price | Thelio Astra from $4,099, in stock [13]; $3,299 at its October 2024 launch (secondary) [14]. ADLINK AADP-128-BCM $5,793.90, "Available Soon" [10]. NextComputing Edge XTP with 128-core Altra $7,555, ships 4 weeks after order [15] |
| desktop use | sold as a developer desktop; Phoronix called it quiet, power-efficient and the only assembled high-end ARM64 desktop (secondary, search snippet) [16]. Run under Linux and Windows (secondary) [14]. Long-term daily-driver write-up: not found |

Sources:

1. N1 TRM r4p1: https://documentation-service.arm.com/static/64b1250ddf6cd61d528c6c65
2. https://www.amperecomputing.com/briefs/ampere-altra-family-product-brief
3. https://github.com/tianocore/edk2-platforms/blob/master/Platform/Ampere/JadePkg/Drivers/AcpiPlatformDxe/AcpiMadt.c
4. https://github.com/tianocore/edk2-platforms/blob/master/Platform/Ampere/JadePkg/Drivers/AcpiPlatformDxe/AcpiIort.c
5. https://github.com/tianocore/edk2-platforms/tree/master/Platform/Ampere
6. https://github.com/tianocore/edk2-platforms/blob/master/Platform/Ampere/buildfw.sh , https://github.com/ARM-software/arm-trusted-firmware/tree/master/plat
7. https://system76.com/tech-docs/models/thelio-astra-a1-n1/specs
8. https://github.com/tianocore/edk2-platforms/blob/master/Silicon/Ampere/AmpereAltraPkg/AmpereAltraPkg.dsc.inc
9. https://www.cnx-software.com/2022/04/07/arm-systemready-sr-certified-ampere-altra-developer-platform/ (secondary)
10. https://shop.adlinktech.com/products/ampere-altra-development-platform-aadp-with-ampere-altra-max-m128-26-soc-6x-16gb-ddr4-dram-256gb-m-2-ssd-tower-chassis-with-liquid-cooling-750-watt-power-supply-including-integrated-broadcom-bcm57502-and-ocp-sfp-with-4-rj45-transceivers
11. https://download.asrock.com/Manual/BMC/ALTRAD8UD-1L2T.pdf
12. https://www.shi.com/product/46556044/ASRock-ALTRAD8UD-1L2T (secondary)
13. https://system76.com/desktops
14. https://www.jeffgeerling.com/blog/2025/system76-built-fastest-windows-arm-pc/ (secondary)
15. https://solutions.nextcomputing.com/products/tower-workstations/edge-xtp/
16. https://phoronix.com/review/system76-thelio-astra (secondary)

## AmpereOne and AmpereOne M

- The brief lists Armv8.6+, SBSA 5, GICv4.1, SMMUv3.2, memory tagging, 128 lanes PCIe 5.0 and 8-channel DDR5. https://amperecomputing.com/briefs/ampereone-family-product-brief
- An A192 `/proc/cpuinfo` shows `paca pacg` and `bti`, without `sve` or `mte` (secondary). https://community.amperecomputing.com/t/weekend-read-memory-tagging-in-ampere-one/963
- One user reports firmware leaving MTE off on AmpereOne M (secondary). https://community.amperecomputing.com/t/ampereone-m-a4-on-oracle-cloud-mte-not-exposed-even-on-bare-metal-id-aa64pfr1-el1-mte-0/3447
- The Mt. Mitchell reference platform is listed SR. SR-LIST
- Open firmware: not found. Public edk2-platforms carries only Altra. https://github.com/AmpereComputing/edk2-platforms
- NextComputing sells an Edge XTP with 192-core AmpereOne at $18,480, shipping 4 weeks after order. https://solutions.nextcomputing.com/products/tower-workstations/edge-xtp/
- An AmpereOne M desktop or dev platform for individuals: not found.

## Other Neoverse desktops sold new

- DGX Station GB300 (Grace, 72x Neoverse V2) listings start near $94,930 (secondary). https://www.tomshardware.com/desktops/nvidias-gb300-powered-dgx-station-desktop-tower-listed-for-nearly-usd100-000-online-enterprise-ai-powerhouse-now-available-to-buy-for-mere-mortals-with-lots-of-cash
- Dell Pro Max with GB300 starts at $175,496.74 (secondary, search snippet). https://www.dell.com/en-us/shop/desktop-computers/scr/desktops/appref=496-gb-memory
- Grace reference systems hold SR (SR-LIST). Both machines are far outside hobby range.
