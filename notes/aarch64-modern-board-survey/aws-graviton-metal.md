# AWS Graviton bare metal

Appendix to [the modern aarch64 board survey](../aarch64-modern-board-survey.md), and to [its server overview](server-and-cloud.md).

Read 2026-10-21 (UTC). A source marked (secondary) is neither the vendor nor the upstream tree. "Not found" means no primary answer, not no. "Inferred" means deduced from cited facts and not read.

## AWS Graviton3, Graviton4 and Graviton5 metal

| topic | fact |
|---|---|
| sizes | G3: `c7g.metal` 64 vCPU 128 GiB, `m7g.metal`, `r7g.metal`, `c7gn.metal`. G4: `c8g`, `m8g`, `r8g` in `metal-24xl` and `metal-48xl`. G5: `c9g`, `m9g`, `r9g` `metal-48xl`, 192 vCPU [A7][A8][A9][A14] |
| cores | G3 Neoverse V1, Armv8.4-A, SVE, PAC, no BTI. G4 Neoverse V2, Armv9.0-A, SVE2, PAC, BTI. G5 Neoverse V3, Armv9.2-A, PAC, BTI [A4]. MTE: not stated for any generation (Arm's V1 and V2 TRMs did not load: [A42]) |
| GIC | GICv3 with ITS from third-party ports only (Nanos on a virtualized instance, Haiku on `c7g.metal`) (secondary) [A26][A27]. GICv4: not found |
| SMMUv3 | see the table in [the overview](server-and-cloud.md). Third-party evidence that the driver runs on Graviton metal: Arm's SMMUv3 driver showed in perf on a Graviton bare-metal host, fixed by lazy mode or passthrough (secondary) [A29]; Arm's learning path sets `iommu.passthrough=1` on an AWS-kernel 192-CPU Neoverse host, instance type not named (secondary) [A21]. ENA and NVMe stream IDs, SMMU revision, ATS and PRI: not found |
| firmware | UEFI with ACPI, and ACPI hot-plug of PCI devices [A14]. No device tree (secondary) [A25] |
| custom boot | Graviton boot mode is UEFI; an AMI takes `--boot-mode uefi` [A10][A11]. Secure Boot is off by default and the machine starts in setup mode, so unsigned code boots [A13]. Bare metal does not keep non-volatile UEFI variables across stop and start [A12]. Image path is an EBS snapshot registered as an AMI (inferred from the boot-mode doc; Haiku's pipeline did it on `c7g.metal`) (secondary) [A27] |
| entry level | not found. Xen needs EL2 and runs only on metal (secondary) [A22][A24], so a UEFI-started kernel probably enters at EL2 (inferred) |
| serial console | all bare metal except `a1.metal`, `g5g.metal`, Macs and `u-*` [A2]. Account opt-in with `aws ec2 enable-serial-console-access`, then `aws ec2-instance-connect send-serial-console-ssh-public-key`, then `ssh` to `serial-console.ec2-instance-connect.<region>.aws` within 60 seconds [A1][A3]. One session per instance, about an hour, 30 seconds to tear down [A1]. The doc uses `ttyS0` at 115200 [A1]; the UART is 16550-compatible and found by ACPI SPCR (secondary) [A26]. So an agent can drive it from the CLI, through a pty-driven `ssh` session (inferred). Whether UEFI output reaches it on Graviton metal: not confirmed; `get-console-output --latest` returned a full boot log on `c7g.metal` (secondary) [A27]. No screenshots on metal or Graviton [A15] |
| price | `c7g.metal` $2.32, `c8g.metal-24xl` $3.83, `c9g.metal-48xl` $8.35 per hour, us-east-1 (secondary: instances.vantage.sh [A30][A32][A33]). Per-second billing, 60 second minimum, no bare-metal exception found [A18]. A start after a stop opens a new billing period [A16]. Spot prices appear on the aggregator; an AWS statement that metal can run on spot: not found |
| quota | the vCPU quota for the family may need an increase for 64 to 192 vCPUs (inferred; the quota page did not load [A39]) |
| hung instance | reboot, stop and stop with `--skip-os-shutdown` are API calls that need no console [A16][A17]. AWS says a metal instance can take "up to 20 minutes or more" to become reachable [A14] |
| PMU | AWS says all hardware PMUs are exposed on metal, with SPE and CMN counters metal-only [A5]. `PMCCNTR_EL0` at EL1 or EL0 and any EL3 trap: not found |
| EL2 and KVM | KVM on metal works (secondary) [A38]. Nested virtualization on G4 or G5: not found. Graviton5's Nitro Isolation Engine: whether metal sizes use it, and what a metal OS then sees, is not found [A36] |
| cost of one test | a 20-minute boot plus a run is about $0.8 to $1.5 on `c7g.metal` (inferred from [A14] and the price) |

Sources, all read 2026-10-21 (secondary marked):

- A1: https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/connect-to-serial-console.html
- A2: https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/ec2-serial-console-prerequisites.html
- A3: https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/configure-access-to-serial-console.html
- A4: https://github.com/aws/aws-graviton-getting-started/blob/main/README.md
- A5: https://github.com/aws/aws-graviton-getting-started/blob/main/perfrunbook/debug_hw_perf.md
- A7: https://docs.aws.amazon.com/ec2/latest/instancetypes/co.html
- A8: https://aws.amazon.com/ec2/instance-types/m9g/
- A9: https://aws.amazon.com/ec2/instance-types/c9g/
- A10: https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/ami-boot.html
- A11: https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/launch-instance-boot-mode.html
- A12: https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/uefi-variables.html
- A13: https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/how-uefi-secure-boot-works.html
- A14: https://docs.aws.amazon.com/ec2/latest/instancetypes/ec2-nitro-instances.html
- A15: https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/instance-console.html
- A16: https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/ec2-instance-reboot.html
- A17: https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/Stop_Start.html
- A18: https://aws.amazon.com/ec2/pricing/on-demand/
- A19: https://github.com/amzn/amzn-drivers/blob/master/userspace/dpdk/README.md (sections 6.3.1, 6.3.2 and the FAQ; re-read directly)
- A21 (secondary): https://learn.arm.com/learning-paths/servers-and-cloud-computing/tune-network-workloads-on-bare-metal/5_iommu/
- A22 (secondary): https://edera.dev/stories/xen-now-runs-on-arm-systemready-platforms
- A24 (secondary): https://patchew.org/Xen/DM6PR07MB43168B0D4DEA80BF2474B9D89AC3A@DM6PR07MB4316.namprd07.prod.outlook.com/
- A25 (secondary): https://nanovms.com/changelog/arm-graviton-missing-device-tree
- A26 (secondary): https://nanovms.com/dev/tutorials/running-nanos-on-aws-graviton
- A27 (secondary): https://github.com/felipedbene/Haiku-Graviton
- A29 (secondary): https://www.linkedin.com/posts/danielolops_earlier-this-week-i-spent-significant-time-activity-6989023086150905856-KORp
- A30, A32, A33 (secondary): https://instances.vantage.sh/aws/ec2/c7g.metal , .../c8g.metal-24xl , .../c9g.metal-48xl
- A36: https://www.amazon.science/blog/ec2s-formally-verified-isolation-engine-provides-mathematical-assurance-of-virtual-machine-isolation
- A38 (secondary): https://dev.to/aws-builders/create-and-run-windows-on-arm-virtual-machines-on-aws-graviton-processors-using-qemu-and-kvm-310j
- A39: https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/ec2-instance-quotas.html (text did not load; claim unverified)
- A42: https://developer.arm.com/documentation/101427 , https://developer.arm.com/documentation/102375 (did not render)
