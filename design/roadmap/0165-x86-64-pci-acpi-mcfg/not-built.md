# What was deliberately not built: an appendix to milestone 165

An appendix to [milestone 165 (x86_64 PCI enumeration)](../0165-x86-64-pci-acpi-mcfg.md). It is the
block's record of the runner it left alone and the NVMe attach it investigated and did not make.

## What was deliberately not built

- `helpers/qemu-runner-x86_64.sh` was not touched. q35 already presents six PCI functions with zero
  `-device` flags: host bridge, VGA, a default NIC, ISA bridge, SATA/AHCI and SMBus, measured via
  QEMU's monitor `info pci`. That is what made proof possible without changing the runner's default
  device set at all. Changing it was the exact fork the brief for this lane flagged as needing to
  stop and report rather than silently changing. It did not arise.

- An actual `-device nvme` attach was investigated and is not safe today. That is worth recording
  precisely because it looked free. `xtask` already sets `NIFE_NVME` unconditionally for every
  architecture's leg, and both aarch64 and riscv64 runners already wire it; only x86_64's does not.
  But `kernel::nvme::tests::the_nvme_disk_serves_the_block_interface_end_to_end` hard-asserts
  `crate::iommu::active()`, and `arch::x86_64::iommu::active()` is a permanent `false` (no VT-d
  built). VT-d is roadmap item 6 of milestone 161 (the x86_64 kernel port). `nvme::bring_up()` itself tolerates an inactive IOMMU: it
  runs the controller unconfined, printing nothing. So attaching the device would not skip the
  existing test. It would turn it into a hard failure on the confinement assertion. Getting a real
  NVMe device working on x86, confined, is therefore gated on VT-d/DMAR parsing, not on anything
  this milestone touches. This was established by reading `nvme.rs` and `arch/x86_64/iommu.rs`
  directly rather than by running the experiment and breaking the suite.
