# Buyable Arm servers and workstations

Appendix to [the modern aarch64 board survey](../aarch64-modern-board-survey.md), and to [its server overview](server-and-cloud.md).

Read 2026-10-21 (UTC). A source marked (secondary) is neither the vendor nor the upstream tree. "Not found" means no primary answer, not no. "Inferred" means deduced from cited facts and not read.

## Buyable servers and workstations

| machine | fact |
|---|---|
| ASRock Rack ALTRAD8UD-1L2T | Deep Micro-ATX, Altra or Altra Max. AST2500 BMC on OpenBMC, with KVM, a virtual-media page and toggles for BMC SSH and network IPMI [B7]. The guide has no Serial-over-LAN (SOL) section. BIOS has COM1 console redirection labeled for out-of-band management, Secure Boot with Standard and Custom modes and "Enroll Efi Image" [B8]. SOL working: not verified. Secure Boot factory default and a UEFI shell: not found. Price: $1,039 at SHI [B9], $869 at Newegg (secondary, snippets); used CPUs about $325 to $675 (secondary) [B16][B17]. Idle power: about 66 W to 123 W reported (secondary) [B11] |
| Ampere firmware for it | Jade reference IORT code: one ITS group, one SMMUv3 node and one root-complex node per enabled PCIe root complex, mapping RIDs 0 to 0xFFFF, optional PMU nodes [B4]. Ampere's own edk2-platforms mirror is archived (last push 2026-09-03); tianocore's still carries `JadePkg`; no AmpereOne platform directory is visible [B3][B5]. TF-A and SCP are signed binaries (the Ampere appendix). `PMCCNTR_EL0` at EL1: no document found |
| Supermicro ARS-110M-NR | 1U, one socket, IPMI 2.0 with KVM and virtual media on a dedicated BMC port, a serial port and a micro-USB serial port [B12]. SOL is not documented for the model (generic Supermicro IPMI supports it, secondary [B13]). Barebone about $2.1k to $2.7k, listed out of stock (secondary) [B14]. A 1U will be loud (inferred) |
| Gigabyte R152, R182, G242 | AST2500 with MegaRAC web console (secondary) [B15]. SOL: not found. R152-P31 configured $5,859 (secondary) [B16]. G242-P36 Altra Max about 121 W lowest and 261 W average under load (secondary) [B10] |
| AmpereOne | A192-32X $5,555 per chip; ASRock Rack AMPONED8-2T bundle $4,799 at Newegg (snippet, secondary) [B19][B20]; `ipmitool sol activate` used on an AmpereOne ASRock Rack bundle (secondary, forum) [B22]. No AmpereOne M or Aurora part sold to individuals was found |
| Grace servers | Supermicro `ARS-111GL-NHR` (GH200) is marked "Discontinued SKU (EOL)", with a BMC LAN port and SystemReady 2.5 [B23]; resale from about $5k to $45k (secondary) [B24]. GPTshop GH200 workstations launched at $41.5k and up (secondary) [B25] |
| SolidRun HoneyComb LX2 | in stock at $1,192.80 ex VAT without RAM (page modified 2026-08-13) [B28]. A72 cores (Armv8.0, no PAC or BTI), GICv3 with ITS, but the IOMMU is an `arm,mmu-500`, which is SMMUv2 [B29]. No BMC listed. Does not meet the SMMUv3 need |
| Kunpeng 920, ThunderX2 | Kunpeng boards were sold in China only (secondary) [B31]; Gigabyte's ThunderX2 R281-T91 is discontinued [B18]. Linux carries ThunderX2-specific SMMUv3 errata handling [B32] |

Sources, read 2026-10-21:

- B1 https://github.com/torvalds/linux/tree/master/arch/arm64/boot/dts/ampere (404)
- B3 https://github.com/tianocore/edk2-platforms/tree/master/Platform/Ampere (commit `1c500d13`)
- B4 https://github.com/tianocore/edk2-platforms/blob/master/Platform/Ampere/JadePkg/Drivers/AcpiPlatformDxe/AcpiIort.c
- B5 https://github.com/AmpereComputing (edk2-platforms archived)
- B7 https://download.asrock.com/Manual/BMC/ALTRAD8UD-1L2T.pdf
- B8 https://download.asrock.com/Manual/BIOS/ALTRAD8UD-1L2T.pdf
- B9 (secondary) https://www.shi.com/product/46556044/ASRock-ALTRAD8UD-1L2T
- B10 (secondary) https://phoronix.com/review/gigabyte-ampere-g242-p36/4
- B11 (secondary) https://community.amperecomputing.com/t/real-world-idle-power-draw-for-altra-max-128c-home-builds/3319
- B12 https://www.supermicro.com/en/products/system/megadc/1u/ars-110m-nr
- B13 (secondary) https://serverascode.com/2013/11/13/ipmi-serial-over-lan-supermicro.html
- B14 (secondary) https://www.sabrepc.com/ARS-110M-NR-Supermicro-S6215734
- B15 (secondary) https://www.gigabyte.com/it/Enterprise/Rack-Server/R152-P31-rev-100
- B16 (secondary) https://www.avadirect.com/GIGABYTE-R152-P31-Ampere-Altra-Processors-SATA-NVMe-1U-Rackmount-Server-Computer/Configure/14690414
- B17 (secondary) https://www.ebay.com/itm/236114347147
- B18 (secondary) https://www.gigabyte.com/ro/ARM-Server/R281-T91-rev-100
- B19 (secondary) https://www.newegg.com/p/N82E16813140167
- B20 (secondary) https://www.servethehome.com/asrock-rack-amponed8-2t-bcm-motherboard-review-with-ampere-ampereone-a192-32x-arm-cpu/
- B22 (secondary) https://community.amperecomputing.com/t/i-got-ampereone-asrock-rack-bundle-and-no-cpu-reading/3282
- B23 https://www.supermicro.com/en/products/system/grace/1u/ars-111gl-nhr
- B24 (secondary) https://www.storagereview.com/zh-CN/news/supermicro-nvidia-gh200-grace-hopper-servers-now-shipping
- B25 (secondary) https://wccftech.com/ai-workstation-pc-e-nvidia-gpu-nvidia-cpu-grace-hopper-superchip-starts-41k-usd//amp/
- B28 https://shop.solid-run.com/product/SRLX216S00D00GE064H09CH/
- B29 https://github.com/torvalds/linux/blob/master/arch/arm64/boot/dts/freescale/fsl-lx216x.dtsi
- B31 (secondary) https://liliputing.com/tag/kunpeng-d920s10/
- B32 https://github.com/torvalds/linux/blob/master/drivers/iommu/arm/arm-smmu-v3/arm-smmu-v3.c
