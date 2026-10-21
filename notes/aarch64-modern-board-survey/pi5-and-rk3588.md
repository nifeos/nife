# Raspberry Pi 5 and RK3588 boards, for the modern aarch64 board survey

Appendix to [the modern aarch64 board survey](../aarch64-modern-board-survey.md).

Read 2026-10-21 (UTC) unless marked. "(secondary)" marks a claim resting on a non-primary source.
"not found" means the fact was looked for and no source stated it.

L: is torvalds/linux at commit `a804c5742bff` (top of `master` on 2026-10-21, version 7.3), so
`L:/arch/arm64/boot/dts/rockchip/rk3588-base.dtsi` means
https://github.com/torvalds/linux/blob/a804c5742bff/arch/arm64/boot/dts/rockchip/rk3588-base.dtsi.
Line numbers are from that commit. Below, `bcm2712.dtsi` and `rp1-common.dtsi` live under
`L:/arch/arm64/boot/dts/broadcom/`, and the `rk3588*` files under `L:/arch/arm64/boot/dts/rockchip/`.

The Cortex-A76 and A55 feature tables are in [cores-and-sel4.md](cores-and-sel4.md). In short, both
SoCs are Armv8.2-A with VHE, LSE and PAN, and neither has PAC, BTI, MTE or SVE.

## Raspberry Pi 5 (BCM2712)

Pi 500, Pi 500+ and CM5 use the same SoC.

| item | fact |
|---|---|
| CPU | 4x Cortex-A76 at 2.4 GHz, PSCI enable method; a separate DT covers the D0 stepping |
| GIC | GICv2 (`arm,gic-400`); no GICv3, no ITS, no LPIs, 8 CPUs at most |
| MSI | Broadcom MIP blocks turn MSI writes into SPIs (64 from SPI 128, 8 from SPI 247); pcie0 has its own MSI controller |
| IOMMU | none in mainline DT; the downstream kernel adds Broadcom IOMMUs for HEVC, PiSP back end and HVS only |
| DMA masters behind an IOMMU | none: PCIe, USB, Ethernet and SD/eMMC are all untranslated |
| TF-A | upstream `PLAT=rpi5`, a minimal BL31 for EL2 payloads; its doc says it is not secure (no secure memory controller); loaded as `armstub8-2712.bin` |
| U-Boot | board code knows Pi 5, CM5, CM5 Lite and Pi 500; the `rpi_arm64_defconfig` docs stop at Pi 4; peripheral coverage not found |
| EDK2 | worproject/rpi5-uefi is archived; RP1 Ethernet never worked |
| SystemReady | not certified (Pi 4 and Pi 400 were, under IR and ES); a GICv2 with no SMMU cannot meet BSA |
| serial | PL011 `uart10` at 0x7d001000, `stdout-path = "serial10:115200n8"`, on the 3-pin JST-SH debug connector; Pi 500 and 500+ list no debug connector; Pi 5 connector voltage not found |
| PMU | PMUv3; mainline DT has no `arm,cortex-a76-pmu` node, the downstream DT does |
| PCIe | pcie0 x1, pcie1 x1 (external FFC), pcie2 x4 (to RP1), all `max-link-speed = <2>` |
| NVMe | Pi 5 needs an M.2 HAT on the FFC; Pi 500+ has an internal M.2 2280 slot |
| USB, Ethernet | 2x USB 3.0 and 2x USB 2.0 on two DWC3 xHCI, and Gigabit Cadence GEM, all inside RP1 |
| SD | `brcm,bcm2712-sdhci` on the SoC |
| price, stock | vendor 16GB $305; PiShop 4GB $110, 8GB $175, 16GB $305, in stock; Adafruit 8GB $200, in stock, 2 per customer; Pi 500 $180, Pi 500+ from $410 |
| desktop | Ubuntu certifies Pi 5 for 26.04 LTS and Core 24 with desktop images; Raspberry Pi OS runs the downstream kernel; Fedora status not found |

The RP1 chain is the main cost for nife. USB and Ethernet need a PCIe root-complex driver, then the
RP1 bus driver, then DWC3 or GEM. RP1's PCI driver is mainline (`L:/drivers/misc/rp1`).

Prices rose with RAM costs. On 1 Apr 2026 the 4GB went up $25, the 8GB $50 and the 16GB $100. On
1 Oct 2026 the 2GB went from $65 to $77.50. The 16GB went from $120 through $145 and $205 to $305.

Sources:
- `bcm2712.dtsi`: CPUs and PSCI lines 64 to 175, `gic-400` line 338, MIP and PCIe lines 513 to 644, PL011 line 245, SDHCI line 210, BT UART line 281
- `L:/arch/arm64/boot/dts/broadcom/bcm2712-rpi-5-b-base.dtsi` lines 13 to 17 (console); `rp1-common.dtsi` lines 121, 165, 179 (GEM, DWC3)
- https://github.com/raspberrypi/linux/blob/rpi-6.12.y/arch/arm64/boot/dts/broadcom/bcm2712-ds.dtsi lines 26 to 27 (PMU), 283 to 311 and 375 to 393 (IOMMUs)
- https://www.raspberrypi.com/products/raspberry-pi-5/ (clock, caches, PCIe 2.0 x1, price)
- https://trustedfirmware-a.readthedocs.io/en/latest/plat/rpi5.html
- https://github.com/u-boot/u-boot/blob/master/board/raspberrypi/rpi/rpi.c lines 199 to 218; https://docs.u-boot.org/en/latest/board/broadcom/raspberrypi.html
- https://github.com/worproject/rpi5-uefi
- https://www.arm.com/architecture/system-architectures/systemready-compliance-program/systemready-past-certifications
- https://www.raspberrypi.com/documentation/computers/raspberry-pi.html (BOOT_UART); https://www.raspberrypi.com/documentation/microcontrollers/debug-probe.html (3.3 V probe)
- https://www.raspberrypi.com/products/raspberry-pi-500/ ; https://www.raspberrypi.com/products/raspberry-pi-500-plus/
- https://www.pishop.us/product/raspberry-pi-5-8gb/ (and the 4gb and 16gb pages); https://www.adafruit.com/product/5813
- https://www.raspberrypi.com/news/a-new-3gb-raspberry-pi-4-for-83-75-and-more-memory-driven-price-increases/ ; https://www.omgubuntu.co.uk/2026/10/raspberry-pi-2gb-costs-how-much-now (secondary); https://www.notebookcheck.net/Raspberry-Pi-5-now-costs-up-to-205-due-to-RAM-crisis.1218213.0.html (secondary)
- https://ubuntu.com/download/raspberry-pi

## RK3588 and RK3588S, shared by every board below

| item | fact |
|---|---|
| CPU | 4x Cortex-A55 and 4x Cortex-A76, PSCI; RK3588S drops `rk3588-extra.dtsi` |
| GIC | GICv3 (GIC-600) at 0xfe600000, two ITSes (`its0` 0xfe640000, `its1` 0xfe660000), all `dma-noncoherent`, MBI range 424 to 479 |
| GICv4 | not claimed by DT or quirk; GIC-600 TRM not checked |
| IOMMU | two SMMUv3 (MMU-600): `mmu600_pcie` at 0xfc900000 enabled; `mmu600_php` at 0xfcb00000 disabled and unreferenced |
| behind SMMUv3 | all five PCIe root complexes: pcie3x4 (RID 0x0000, its1), pcie3x2 (0x1000, its1), pcie2x1l0 (0x2000, its0), pcie2x1l1 (0x3000, its0), pcie2x1l2 (0x4000, its0) |
| not behind any IOMMU | USB xHCI (DWC3), EHCI/OHCI, GMAC0/1, SATA, SDMMC/SDIO/eMMC |
| media IOMMUs | Rockchip-proprietary units front NPU, VPU, RGA, VEPU, VDEC, VICAP and VOP; AV1 uses `verisilicon,iommu-1.2` |
| TF-A | upstream `plat/rockchip/rk3588`, BL31 only; DDR init is still the rkbin blob as TPL |
| U-Boot | mainline, with `generic-rk3588` and the board targets below |
| EDK2 | edk2-porting/edk2-rk3588 (community, active), ACPI (tested against Windows only) or DT with a mainline mode on Platinum boards; SMMU and ITS in its ACPI tables not stated |
| SystemReady | IR list has Rockchip TB-RK3588X and ArmSoM Sige7; none of the boards below; no ES certification and no BSA claim found |
| serial | DesignWare APB 16550 UARTs; console `serial2` at 1,500,000 baud on all four boards |
| PMU | PMUv3 on both clusters, separate A55 and A76 PMU nodes on partitioned PPI 7 |

Three quirks matter to nife here.

- Erratum RK3588001: Linux matches GIC IIDR 0x0201743b on rk3588 and forces ITS and redistributor tables non-shareable. LPI property and pending tables, ITS command queues and ITS tables must be mapped Non-cacheable or cleaned to PoC. The 32-bit PA quirk (3568002) is rk3566/rk3568 only.
- The PMU interrupt is PPI 7 split by `ppi-partitions` (A55 partition 0, A76 partition 1), so the kernel must handle per-cluster PPI affinity.
- On boards with a PCIe NIC, Ethernet is behind SMMUv3. On RK3588S boards, Ethernet is GMAC1 and untranslated. USB is never translated.

`mmu600_php` sits beside the USB3 and SATA block, which suggests it was meant for them. That is an inference; no DT or TRM text says so.

Rockchip's binary BL31 is the vendor default; its MDCR settings were not found. With upstream TF-A, nife owns MDCR_EL3.

Sources:
- `rk3588-base.dtsi`: CPUs lines 94 to 229, PMU lines 390 to 399, PSCI line 402, GIC lines 2587 to 2616, UARTs lines 783 to 790 and 2796 to 2916
- `rk3588-base.dtsi`: SMMUv3 lines 554 and 565, media IOMMUs lines 1168 to 1691, pcie2x1l1/l2 lines 2188 to 2266, USB/GMAC/SATA/SD lines 467 to 534 and 2292 to 2428
- `rk3588-extra.dtsi`: pcie3x4 lines 344 to 372, pcie3x2 lines 431 to 459, pcie2x1l0 lines 483 to 509, lines 51 and 535
- `L:/drivers/irqchip/irq-gic-v3-its.c` lines 4867 to 4986; `L:/Documentation/arch/arm64/silicon-errata.rst`
- https://trustedfirmware-a.readthedocs.io/en/latest/plat/rockchip.html ; https://github.com/ARM-software/arm-trusted-firmware/tree/master/plat/rockchip/rk3588
- https://docs.u-boot.org/en/latest/board/rockchip/rockchip.html ; https://github.com/u-boot/u-boot/blob/master/doc/board/rockchip/rockchip.rst lines 151 to 181
- https://github.com/edk2-porting/edk2-rk3588
- https://www.arm.com/architecture/system-architectures/systemready-compliance-program/systemready-past-certifications

## Per-board facts (RK3588 family)

| item | Radxa ROCK 5B / 5B+ | Orange Pi 5 (RK3588S) | Orange Pi 5 Plus | Orange Pi 5 Max |
|---|---|---|---|---|
| debug UART | 40-pin header pins 8, 10, 6; voltage not found | 3-pin header; voltage not stated | 3-pin header; voltage not stated | on the 40-pin header |
| NVMe | 5B: M.2 Gen3 x4; 5B+: two M.2 Gen3 x2 | M.2 on pcie2x1l2, Gen2 x1 | M.2 2280 on pcie3x4 | M.2 on pcie3x4 |
| NVMe behind SMMUv3 | yes | yes | yes | yes |
| Ethernet | 2.5GbE on PCIe (RTL8125B, secondary) | 1 GbE on GMAC1 | 2x 2.5GbE RTL8125BG on pcie2x1l1 and l2 | 2.5GbE RTL8125BG on pcie2x1l1 |
| Ethernet behind SMMUv3 | yes | no | yes | yes |
| USB (never behind IOMMU) | two DWC3 xHCI, a third on 5B | xHCI host0 and host2 | xHCI host0 and host1 | 2x USB 3.0, 2x USB 2.0 |
| U-Boot target | `rock5b-rk3588` | `orangepi-5-rk3588s` | `orangepi-5-plus-rk3588` | `orangepi-5-max-rk3588` |
| EDK2 | 5B Platinum, 5B+ Bronze | Platinum | Platinum | not listed |
| price, stock | 5B+ $80 to $427 list, all sold out (Allnet, Arace); 5B $289.95, out of stock (ameriDroid) | launch $60 (2022, secondary); current not found | about $355 Amazon, $390 16GB distributor, limited stock (secondary) | launch $125 to $145 (2024, secondary); $389 16GB distributor (secondary) |
| desktop (Armbian) | Standard, 6.18 mainline and 6.1 vendor, Ubuntu 26.04 desktops | Standard, same kernels | Standard, same kernels | Community, edge 7.3 and vendor 6.1 |

The 5B+ DT cuts pcie3x4 to two lanes and enables pcie3x2. CNX reports the 5B+ 8GB launched at $90
and now lists at $129.99, with marketplace listings near $450 (secondary).

Sources, ROCK 5B / 5B+:
- `rk3588-rock-5b-5bp-5t.dtsi` lines 9, 19, 475 to 497, 976, 1050 to 1069; `rk3588-rock-5b-plus.dts` lines 51 to 95; `rk3588-rock-5b.dtsi` line 77
- https://docs.radxa.com/en/rock5/rock5b/radxa-os/serial ; https://radxa.com/products/rock5/5bp/
- https://shop.allnetchina.cn/products/rock5-model-b-1 ; https://arace.tech/products/radxa-rock-5b-plus ; https://ameridroid.com/products/rock5-model-b ; https://www.cnx-software.com/?p=169435 (secondary)
- https://www.armbian.com/rock-5b/

Sources, Orange Pi 5:
- `rk3588s-orangepi-5.dtsi` lines 10, 14, 646; `rk3588s-orangepi-5.dts` line 28; `rk3588s-orangepi-5-5b.dtsi` lines 72, 221, 232
- http://www.orangepi.org/html/hardWare/computerAndMicrocontrollers/details/Orange-Pi-5.html
- https://www.cnx-software.com/?p=101401 (secondary); https://www.armbian.com/orange-pi-5/

Sources, Orange Pi 5 Plus:
- `rk3588-orangepi-5.dtsi` lines 13, 22, 363 to 367, 823, 845 to 857; `rk3588-orangepi-5-plus.dts` lines 268 to 284, 399
- http://www.orangepi.org/html/hardWare/computerAndMicrocontrollers/details/Orange-Pi-5-plus.html
- https://computingforgeeks.com/best-raspberry-pi-5-kits-sbc-alternatives/ (secondary); https://orangepi.net/product-tag/orange-pi-5-plus (secondary)
- https://www.armbian.com/orange-pi-5-plus/

Sources, Orange Pi 5 Max:
- `rk3588-orangepi-5-max.dts`; `rk3588-orangepi-5-compact.dtsi` lines 7, 66 to 68, 137, 159
- http://www.orangepi.org/html/hardWare/computerAndMicrocontrollers/details/Orange-Pi-5-Max.html
- https://liliputing.com/orange-pi-5-max-is-a-credit-card-sized-rk3588-pc-with-2-5-gbe-ethernet-pcie-3-0-and-wifi-6e/ (secondary); https://orangepi.net/product/orange-pi-5-max-16gb-ram (secondary)
- https://www.armbian.com/orange-pi-5-max/

## Newer Rockchip parts

RK3688 (Armv9.3, announced 2024) and RK3668 (Cortex-A730 and A530, announced 2025) have no DTSI in
Linux, no binding compatible and no TF-A platform. No shipping boards were found.

Sources: https://www.cnx-software.com/2024/10/04/rockchip-rk3688-armv9-3-aiot-processor-to-feature-a-16-tops-npu-ufs-4-0-storage/ (secondary); https://www.cnx-software.com/2025/07/18/rockchip-unveils-rk3668-10-core-arm-cortex-a730-cortex-a530-soc-with-16-tops-npu-rk182x-llm-vlm-co-processor/ (secondary); `L:/arch/arm64/boot/dts/rockchip`; https://github.com/ARM-software/arm-trusted-firmware/tree/master/plat/rockchip
