---
status: PARTIAL
raised: 2026-10-05
milestone_dependencies: none
decision_dependencies: none
machine_requirements: x86_64 AMD silicon with an IOMMU (AMD-Vi) and an IVRS
specific_machine: none
needs_person: yes
---
# 764. AMD-Vi confines device DMA on x86_64

Raised 2026-10-05 (UTC) by
calef's plan to use a recent AMD desktop as fatal risk 9's second x86 machine, after argon. On an AMD
machine the ACPI tables carry an IVRS and no DMAR, so the kernel found no IOMMU and printed
`skipped, no DMAR`. The confinement claim silently did not hold there: the NVMe driver ran
unconfined and the xHCI driver refused to start. Built and proven under QEMU on patagonia; the first
boot on AMD silicon is what remains, which is why this is `PARTIAL`.

## What it does

The kernel reads the IVRS (`machine_discovery::acpi::ivrs`) and brings up every AMD-Vi unit it names,
over a device table in which every entry denies all DMA. It then confines each device the four
confine callers (NVMe, e1000e, xHCI, virtio) attach, through the same `crate::iommu` seam VT-d uses. Faults
come from the unit's event log through `take_fault`. A machine with neither table keeps booting and
says so:

```text
  vt-d        : skipped, no DMAR (this machine's IOMMU is AMD-Vi)
  amd-vi      : unit 0xfed80000 up (1 of 1, ivhd type 0x11), 256 device table entries all blocked, translation enabled (status confirmed), 0 ivmd device(s) mapped
```

```text
  iommu       : NONE: this machine's ACPI names neither a DMAR (VT-d) nor an IVRS (AMD-Vi), so no device's DMA is confined; every driver below runs unconfined
```

## The pieces

- `crates/page_tables/src/x86_64.rs`: `AmdVi`, the host page-table format, and `PageFormat::table_entry`
  now takes the level, because an AMD-Vi directory entry names the level it points at.
- `crates/machine_discovery/src/acpi/ivrs.rs`: the IVRS decoder, choosing the highest IVHD type the
  table carries, recording device-id ranges, aliases and IVMDs.
- `kernel/src/arch/x86_64/amd_vi.rs`: the driver. `kernel/src/arch/x86_64/iommu.rs` hands each seam
  call over when a unit is up.
- `kernel/src/iommu.rs` and each architecture's `iommu::build_domain`: the DMA format is chosen at
  run time, since x86's depends on the machine.
- `helpers/qemu-runner-x86_64.sh`: `NIFE_IOMMU=amd` (with `dma-remap=on`) and `none`, both
  provisional. `xtask/src/suite.rs` boots both x86_64 test images a second time on the AMD-Vi
  machine.
- `script/falsifications`: an `Environment:` line (provisional) so a record can name the machine.

notes/amd-vi.md has the reasoning, the six places QEMU 11.1.1 differs from AMD 48882, and the
evidence table.

## Proof

On the AMD-Vi QEMU machine, the whole x86_64 kernel and system suites pass (129 and 237), the
virtio and NVMe DMA-escape tests among them, with the same skips as the VT-d machine. The
falsification is a device table entry with `Mode` 0 and `IR`/`IW` kept. It turns the virtio escape
test red, replayable through
`kernel/falsifications/virtual_io.tests.the_iommu_faults_a_dma_that_escapes_the_domain.patch`. It turns
the NVMe escape test red too, attested on 2026-10-05, since that test's one record is the
domain-widening patch every unit shares.

## Reuse

None exists to take. crates.io was searched on 2026-10-05 for `amd-vi`, `amd iommu` and `iommu`,
and has no AMD-Vi driver. It has an SMMUv3 simulator, VFIO and IOMMUFD wrappers for a Linux host,
and a `hardware` abstraction layer, not examined further because it is a whole HAL rather than a
driver. Linux's `drivers/iommu/amd/` (GPL-2.0) was read at v6.12 for the order of operations and
the leaf bits, and is a client of Linux's IOMMU framework, not something to lift. §46 (thin
primitives or whole subsystems) puts an IOMMU driver on the verification path, which is the
write-it case.

## Follow-on

- **Outstanding.** The first boot on AMD silicon: the desktop calef named for fatal risk 9. QEMU is
  the only AMD-Vi this has run on, and notes/amd-vi.md's "What a first AMD board should show" is
  the checklist. Checked 2026-10-05: no AMD bench log exists under `bench/`.
- **Recorded.** QEMU 11.1.1 writes no address into AMD-Vi event records, so on this leg the escape
  tests tie a fault to their device by requester id; `kernel/src/arch/x86_64/amd_vi.rs`'s BUGS. It
  ends when `.qemu-version` reaches a release carrying QEMU commit 4adfb431c0.
- **Recorded.** Interrupt remapping is never enabled, VT-d's posture too;
  design/roadmap/0317-interrupt-remapping-flags.md owns the question.
- **Recorded.** `notes/confinement-claims.md` has no AMD-Vi row because that note is over its
  word budget; notes/amd-vi.md carries the claims until it is trimmed.
- **Milestone 102.** Milestone 102 (what a confined device's fault reaches) owns the fault
  interrupt and the call to `amd_vi::quarantine` that §163 (where a confined device's IOMMU fault
  is delivered) asks for, on all four units at once.

## BUGS

- An IVMD naming every device (type 20h) is mapped into every domain but pre-attaches no device,
  and the IVMD path has run zero times (QEMU writes none).
- Without `EFR.IASup`, `init` flushes 65,536 domain ids one command each; the cost on silicon is
  unmeasured.

## Index row

The confinement claim stops at the IOMMU the kernel can drive, and on x86_64 that was VT-d only, so
on any AMD machine every device ran with the whole of memory in reach while the boot read as
normal. This makes fatal risk 9's AMD machine a confined one.
