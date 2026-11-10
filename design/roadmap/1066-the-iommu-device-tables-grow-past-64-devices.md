---
status: NOT-STARTED
raised: 2026-11-10
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1066. The IOMMU device tables on aarch64 and riscv64 grow past 64 devices

*(The number, title and slug are provisional.)*

Reuse: both drivers already build their tables; this grows them in place. The kernel drives the
IOMMU, and Kani proves these drivers, so no third-party crate applies.

Ruled 2026-11-10 (UTC): the QEMU runners keep virtio-mmio, and §18 (the PCIe transport: one driver,
two buses, the seam in the kernel)'s item 3 stands. A later move of the aarch64 and riscv64 runners
to virtio-pci needs this milestone and milestone 1065 (several drivers share an interrupt line, or
MSI replaces the lines) first.

## Why it stands on its own

A PCI requester id is bus, device and function: `bus << 8 | device << 3 | function`. Both IOMMU
drivers index their table by it, and both stop at 64:

- aarch64: the SMMUv3 stream table is linear, 64 entries of 64 bytes in one frame
  (`STRTAB_LOG2 = 6`, `kernel/src/arch/aarch64/iommu.rs`). `table_offset` panics past it.
- riscv64: the RISC-V IOMMU device directory is one level in one frame (`DDTP_MODE_1LVL`,
  `kernel/src/arch/riscv64/iommu.rs`). With the extended 64-byte context it holds requester ids 0
  to 63, and `context_offset` panics past it.

So neither driver confines anything past bus 0's slots 0 to 7. The riscv64 runner already uses
all eight and put the e1000e at `00:03.1` to stay inside (#1632, when a ninth slot panicked the driver). A device behind
a bridge, on any bus but 0, is out of reach. Any PCI driver on these two architectures meets the
ceiling, whether or not the runners move.

The comment above `STRTAB_LOG2` says 64 entries cover bus 0's 32 devices twice over. They cover
eight, since each device has eight function numbers. The build corrects it.

Milestone 1064 (PCI enumeration leaves the kernel) relies on related work: §319 (a device and its
DMA domain as kernel objects) binds each device to its domain, and that binding is an entry in
these tables. 1064 does not grow them.

## The work, in order

1. aarch64: a 256-entry linear stream table (four frames, `STRTAB_LOG2 = 8`), which covers all of
   bus 0. A two-level stream table covers every bus and is the shape real SMMUs with many streams
   use; it is the choice if the build finds a bus beyond 0 on any board this tree targets. Effort:
   S (a guess).
2. riscv64: a two-level device directory (`DDTP` mode 3), a root frame of pointers to leaf frames
   allocated as devices attach. Effort: M (a guess).
3. The Kani bounds in both drivers' `mod proofs` move to the new limits, and the proof that no
   device reaches another device's entry holds over the whole range.
4. The riscv64 runner moves the e1000e to a slot of its own, and both architectures confine a
   function at a requester id of 64 or more.

Total: S to M (a guess).

## Done when

1. On aarch64 and riscv64 the kernel attaches and confines a PCI function at requester id 64 or
   above, and the function faults on a DMA outside its domain. A system test on each architecture
   shows it.
2. The Kani harnesses that bound requester ids prove the new range.
3. Neither driver panics on a requester id the board can produce, and each refuses one beyond the
   new table with an error, not a panic.
4. The existing IOMMU and PCI gates pass on every architecture, with no test weakened.

## Index row

The SMMUv3 stream table on aarch64 and the RISC-V IOMMU device directory on riscv64 each hold 64
requester ids, which is bus 0's first eight slots; a ninth slot panics the driver. This grows the
SMMU table to all of bus 0 and gives the RISC-V IOMMU a two-level directory. Any PCI driver on
those architectures needs it, and the runners' move to virtio-pci waits on it.
