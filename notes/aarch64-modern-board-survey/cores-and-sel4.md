# Arm cores and seL4 support, for the modern aarch64 board survey

Appendix to [the modern aarch64 board survey](../aarch64-modern-board-survey.md).

Read 2026-10-21 (UTC) unless marked. "(secondary)" marks a claim that rests on a non-primary source. "Not found" means a search turned up no primary answer; it does not mean the answer is no.

seL4 trees are cited at pinned commits, with this shorthand:

| short | tree at commit (commit date) |
|---|---|
| K: | https://github.com/seL4/seL4/blob/d97d707b95a583afb8de13bbfe9b86a0d5bc0fe2/ (2026-10-21) |
| CI: | https://github.com/seL4/ci-actions/blob/4da5cd571c255e6637d04cd099dcbde47a8aee1f/ (2026-10-16) |
| B: | https://github.com/seL4/sel4bench/blob/18f9d5f079bc23551eccbdc2202cae95c72c69d7/ (2026-09-23) |
| W: | https://github.com/seL4/website/blob/c149c103fd2ec92f05e5c47b4d8939dcc39b39d1/ (2026-10-21, source of sel4.systems) |
| D: | https://github.com/seL4/docs/blob/07f7f27aa3012c7c8ebde862b24d54573b312a2d/ (2026-10-12, source of docs.sel4.systems) |

## 1. Core features

Values come from each core's Technical Reference Manual (TRM), mostly its ID register pages. PMU "v3" is ID_AA64DFR0_EL1.PMUVer 0x4 where the TRM gives the register (PMUv3 with the Armv8.1 extensions).

| core | arch level | VHE | LSE | PAN | PAC | BTI | MTE | SVE/SVE2 | PMU |
|---|---|---|---|---|---|---|---|---|---|
| Cortex-A53 | Armv8.0-A | no | no | no | no | no | no | no | v3 |
| Cortex-A72 | Armv8.0-A | no | no | no | no | no | no | no | v3 |
| Cortex-A55 | Armv8.2-A + LDAPR, optional dot product | yes | yes | yes (v8.2) | no | no | no | no | v3 (0x4) |
| Cortex-A76 | Armv8.2-A + LDAPR, dot product | yes | yes | yes (v8.2) | no | no | no | no | v3 (0x4) |
| Cortex-A78 / A78C | Armv8.2-A + LDAPR, dot product, SSBS | yes | yes | yes | no | no | no | no | v3 |
| Cortex-A78AE | Armv8.2-A + RAS, SPE, LDAPR, dot product, SSBS, PAuth | yes | yes | yes (PAN2) | yes, see note | no | no | no | v3 (0x4) |
| Neoverse N1 | Armv8.2-A + RAS, SPE, LDAPR, dot product, SSBS | yes | yes | yes | no | no | no | no | v3 (0x4) |
| Cortex-A520 | Armv9.2-A | yes | yes (LSE2) | yes | yes (PAuth2) | yes | MTE2/MTE3 | yes / yes | v3p7 |
| Cortex-A720 | Armv9.2-A | yes | yes (LSE2) | yes (PAN2/3) | yes (PAuth2, FPAC) | yes | MTE2/MTE3 | yes, 128-bit / yes | v3p7, no v3p8 |
| Cortex-A725 | Armv9.2-A | yes | yes (LSE2) | yes (PAN3) | yes (PAuth2, FPAC) | yes | MTE2/MTE3 if BROADCASTMTE | yes, 128-bit / yes | v3p7 |
| Cortex-X925 | Armv9.2-A | yes | yes (LSE2) | yes (PAN3) | yes (PAuth2, FPAC) | yes | MTE2/MTE3 if BROADCASTMTE | yes, 128-bit / yes | v3p7 |
| Neoverse V3AE | Armv9.2-A | yes (NV2 too) | yes (LSE2) | yes (PAN3) | yes (PAuth2, FPAC) | yes | MTE2/MTE3 if BROADCASTMTE | yes, 128-bit / yes | v3p7, no v3p8 |
| AmpereOne (Ampere-1, Ampere-1A) | Armv8.6+ | implied by v8.6 | implied by v8.6 | implied by v8.6 | yes (secondary) | yes (secondary) | Ampere-1A only; see note | no (secondary) | not found |

Sources:

- A53: https://developer.arm.com/documentation/100236/latest (Arm TRM; not re-read on 2026-10-21).
- A72: https://developer.arm.com/documentation/100095/latest (Arm TRM; not re-read on 2026-10-21).
- A55: https://developer.arm.com/documentation/100442/0200/Functional-description/Introduction-to-the-Cortex-A55-core/Supported-standards-and-specifications and the register pages under https://developer.arm.com/documentation/100442/0200/Register-descriptions/AArch64-system-registers
- A76: https://developer.arm.com/documentation/100798/0401/Introduction-to-the-Cortex-A76-core/Features and its ID register pages under https://developer.arm.com/documentation/100798/0401/AArch64-System-registers (ID_AA64ISAR1 bits [19:4] RES0, so no PAC; ID_AA64PFR1 bits [63:8] RES0, so no BTI or MTE; ID_AA64PFR0 bits [55:32] RES0, so no SVE).
- A78: https://developer.arm.com/documentation/101430/0102/Functional-description/Introduction/Supported-standards-and-specifications
- A78AE: TRM r0p2, 101779_0002_07, section 2.1 and ID register pages: https://documentation-service.arm.com/static/62502fb164e3265f7c90b120
- N1: TRM r4p1, 100616_0401_02: https://documentation-service.arm.com/static/64b1250ddf6cd61d528c6c65
- A520: https://developer.arm.com/documentation/102517/0004/The-Cortex-A520--core/Supported-standards-and-specifications
- A720: https://developer.arm.com/documentation/102530/0002/The-Cortex-A720--core/Supported-standards-and-specifications and https://developer.arm.com/documentation/102530/0002/The-Cortex-A720--core/Cortex-A720--core-features
- A725: TRM r0p2, 107652 issue 05: https://documentation-service.arm.com/static/6749c8a127eda361ad4dd099
- X925: TRM r0p2, 102807 issue 06: https://documentation-service.arm.com/static/68374ea93f2acc596c2d9d93
- V3AE: TRM r0p2, 101595 issue 07: https://documentation-service.arm.com/static/67361832c7fc0d1f211d9e66
- AmpereOne: Armv8.6+, memory tagging and "ROP/JOP mitigation" in https://amperecomputing.com/briefs/ampereone-family-product-brief . An A192 `/proc/cpuinfo` shows `paca pacg` and `bti`, with no `sve` or `mte`: https://community.amperecomputing.com/t/weekend-read-memory-tagging-in-ampere-one/963 (secondary). Ampere-1 has no MTE: https://www.phoronix.com/news/AmpereOne-1A-GCC-13-Compiler (secondary).

The A78AE TRM contradicts itself on PAC. Its feature list (section 2.1) includes Pointer Authentication (v8.3) and Enhanced PAC without FPAC (v8.6). Its own ID_AA64ISAR1_EL1 page shows bits [19:4], the PAC fields, as RES0. Shipping parts settle it: an AGX Orin (A78AE r0p1) reports `paca pacg`, and no `bti`, `mte` or `sve`: https://forums.developer.nvidia.com/t/agx-orin-64gb-cpus-is-less-than-12/366792 (secondary).

MTE in a core is not enough to use it. The SoC must provide tag storage, and firmware must enable it (SCR_EL3.ATA and a tag carve-out). On A725, X925 and V3AE, the TRMs say MTE2/MTE3 exist only when the SoC ties the BROADCASTMTE input to 1 (V3AE and X925 TRMs above). Known outcomes:

- GB10 (X925 + A725) as shipped reports no `mte` in its flags: https://www.spec.org/cpu2026/results/res2026q2/cpu2026-20260210-00021.txt (a SPEC result submitted by Arm).
- Jetson Thor (V3AE): whether MTE is enabled is not found.
- AmpereOne M on bare metal reads ID_AA64PFR1_EL1.MTE = 0, which the reporter attributes to firmware: https://community.amperecomputing.com/t/ampereone-m-a4-on-oracle-cloud-mte-not-exposed-even-on-bare-metal-id-aa64pfr1-el1-mte-0/3447 (secondary).
- CIX P1 (A720 + A520): Radxa enables MTE only through a self-built BIOS from the edk2-cix sources: https://forum.radxa.com/t/mte-feature-is-now-available-to-developers/28156 (secondary).

## 2. seL4 support tiers

### Tier definitions

Each tier is read from seL4's own trees at the commits above. A board sits in the highest tier it meets.

| tier | meaning | where it is read |
|---|---|---|
| published | IPC numbers on sel4.systems/performance.html | the page renders `_data/benchmarks.json`: W:performance.html, W:_data/benchmarks.json |
| sel4bench on hardware | board is in sel4bench's `hw-run` CI matrix | the `hw-run` job in B:.github/workflows/sel4bench.yml and the identical one in CI:.github/workflows/sel4bench-hw.yml |
| sel4test on hardware | `platforms.yml` entry with `req:` boards and no `no_hw_test` or `no_hw_build` | CI:seL4-platforms/platforms.yml; the matrix is built by CI:sel4test-hw-matrix/build.py, which drops those flags |
| builds only | `platforms.yml` entry with `no_hw_test: true`, so a hardware image is built and never run | CI:seL4-platforms/platforms.yml |
| none | no `src/plat` directory for the SoC, nothing merged | K:src/plat |

Between "builds only" and "none" the trees also hold kernel platforms with no CI hardware entry at all (`declare_platform` in K:src/plat/<p>/config.cmake). None of the surveyed boards falls there. In seL4/seL4 pull requests, the sel4test hardware job runs only with the `hw-test` label (CI:.github/workflows/sel4test-hw.yml) and sel4bench only with `hw-bench` (K:.github/workflows/hw-tests.yml).

Only one aarch64 board is published: Jetson TX1 (Cortex-A57), runs `TX1_64` and `TX1_MCS_64` (W:_data/benchmarks.json, entries `TX1_64` and `TX1_MCS_64`). The `hw-run` matrix holds four aarch64 boards: imx8mm_evk, odroid_c2, tx1, tx2.

A separate axis is the verified configuration. K:CAVEATS.md (its AArch64 section) lists the AArch64 platforms with one ("Armv8-a with hypervisor extensions only, no SMMU, with FPU, with fast path"), each backed by K:configs/AARCH64_<plat>_verified.cmake.

### Surveyed board families

| board family | SoC, seL4 platform | tier | notes |
|---|---|---|---|
| Raspberry Pi 5 | BCM2712, `bcm2712` | builds only | `no_hw_test`, comment "the seL4 foundation has no RPI5 hardware"; platform added in 16.0.0; has a verified config |
| Radxa ROCK 5B/5B+, Orange Pi 5/5 Plus/5 Max | RK3588/RK3588S | none | no PR found for rk3588 or rock5; nearest are rk3568 (builds only) and rockpro64 (sel4test on hardware) |
| Radxa Orion O6/O6N, Orange Pi 6, Minisforum MS-R1 | CIX P1 | none | |
| MediaTek Genio 510 to 1200, Genio Pro | MT83xx, MT8894 | none | |
| NXP i.MX 95 boards | i.MX 95 | none | searches for `imx95` and `imx9` empty |
| NXP FRDM i.MX 8M Plus (contrast) | i.MX 8M Plus, `imx8mp-evk` | builds only | `no_hw_test`, "no board in the machine queue"; platform added in 13.0.0 |
| TI AM62, AM69 | AM62x, J784S4 | none | |
| Qualcomm X Elite, QCS6490, IQ-9075/IQ-8275 | Hamoa, Kodiak, Lemans/Monaco | none | the only Qualcomm platform, `apq8064`, is AArch32 only |
| Jetson Orin Nano, AGX Orin | Tegra234 | none | open PR #1135 "NVIDIA Jetson Orin support" |
| Jetson AGX Thor | Tegra264 | none | |
| DGX Spark and GB10 boxes | GB10 | none | |
| Ampere Altra systems | Altra | none | no ACPI-based Arm server platform in `src/plat` |
| AmpereOne | AmpereOne | none | |

For comparison, older aarch64 boards seL4 tests on hardware (CI:seL4-platforms/platforms.yml): TX1 (published), TX2, i.MX8MM EVK and Odroid C2 (sel4bench), and i.MX8MQ EVK, i.MX93, Odroid C4, ZCU102, STM32MP2, TQMa8XQP and ROCKPro64 (sel4test).

Sources:

- Platform keys and flags: CI:seL4-platforms/platforms.yml, keys `RPI5`, `IMX8MP_EVK`, `ROCK3B` and `ROCKPRO64`.
- Platform list (the aarch64 set is bcm2711, bcm2712, fvp, hikey, imx8m-evk, imx93, maaxboard, odroidc2, odroidc4, qemu-arm-virt, quartz64, rk3568, rockpro64, stm32mp2, tqma8xqp1gb, tx1, tx2, zynqmp): K:src/plat
- apq8064 is AArch32 only: K:src/plat/apq8064/config.cmake
- Release history: K:CHANGES.md, under 16.0.0 (2026-07-22: stm32mp2, bcm2712, Cortex-A76), and under 13.0.0 (2024-07-01: Cortex-A55, imx8mp-evk, Odroid C4, MaaXBoard, TQMa8XQP). The Pi 5 pull request is https://github.com/seL4/seL4/pull/1515 (merged as `b7ef16a42d6d87b776462e66b8803365aea9f41d`).
- Orin: https://github.com/seL4/seL4/pull/1135 (opened 2023-11-17, last updated 2026-03-11, head `67c48035d13db82459c5bf40dd85d1c51aa0c4d1`).
- RK3588, CIX, Genio, i.MX 95, Qualcomm aarch64: `gh search prs --repo seL4/seL4` returned nothing for each name.

### Hypervisor (EL2)

- `KernelArmHypervisorSupport` is offered for Cortex-A15, A35, A53, A55, A57, A72 and A76 (K:src/arch/arm/config.cmake, `KernelArmHypervisorSupport`). No core newer than the A76 is on that list.
- Every AArch64 verified config turns hypervisor support on and the SMMU off (K:configs/include/AARCH64_verified_include.cmake), so the verified AArch64 kernel runs at EL2.
- Hypervisor builds tested on hardware (`aarch_hyp: [64]`): IMX8MQ_EVK, IMX93, ODROID_C4, ZYNQMP, STM32MP2, TX1, TX2. ROCK3B and MAAXBOARD build them without running. RPI5 and IMX8MP_EVK are not in `aarch_hyp` (CI:seL4-platforms/platforms.yml).
- SMP with MCS and hypervisor together is tested only on odroidc4, tx1 and tx2 (K:CAVEATS.md, the SMP and MCS caveats).
- The docs mark `virtualization: true` for Rpi5 and imx8mp, among others (D:Hardware/Rpi5.md). That is documentation, weaker evidence than CI.

### SMMU

- The only SMMU driver is SMMUv2: K:src/drivers/smmu/smmuv2.c, registered for `arm,mmu-500` (K:src/drivers/smmu/config.cmake).
- `KernelArmSMMU` depends on `KernelPlatformTx2` (K:src/arch/arm/config.cmake, `KernelArmSMMU`), and TX2 wires it in (K:src/plat/tx2/config.cmake). So SMMU support is TX2 only.
- There is no SMMUv3 code: no file in the tree matches `smmuv3`, and no pull request does.
- Open pull requests: https://github.com/seL4/seL4/pull/761 "Zynqmp ultra96 smmu" (since 2022-02-02, updated 2026-10-20) and https://github.com/seL4/seL4/pull/760 "Smmu v2 fixes improvements".

### GICv3

The interrupt controller is set per platform by `INTERRUPT_CONTROLLER` in K:src/plat/<p>/config.cmake, with option `KernelArmGicV3` (K:src/arch/arm/config.cmake).

- GICv3: imx8m-evk (mq, mm, mp), imx93, maaxboard, rk3568, rockpro64, quartz64, tqma8xqp1gb, fvp. qemu-arm-virt can select v2 or v3 (K:CHANGES.md, the qemu-arm-virt entry).
- GICv2: bcm2711, bcm2712 (K:src/plat/bcm2712/config.cmake), odroidc2, odroidc4, tx1, tx2, zynqmp, hikey, stm32mp2.
- GICv3 virtualization was added and tested on i.MX8QXP in 13.0.0 (K:CHANGES.md, under 13.0.0). 16.0.0 enables `ICC_SRE_EL2` in hypervisor configs (K:CHANGES.md, under 16.0.0).
