---
status: NOT-STARTED
raised: 2026-11-10
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1065. MSI replaces the interrupt lines on aarch64 and riscv64

*(The number, title and slug are provisional.)*

Reuse: the kernel's own route table, and the GICv3 ITS and AIA register layouts from their
specifications. No third-party crate fits a kernel
interrupt route.

Ruled 2026-11-10 (UTC): the QEMU runners keep virtio-mmio, and §18 (the PCIe transport: one driver,
two buses, the seam in the kernel)'s item 3 stands. A later move of the aarch64 and riscv64 runners
to virtio-pci needs this milestone and milestone 1066 (the IOMMU device tables grow past 64
devices) first.

## Why it stands on its own

On aarch64 and riscv64 every PCI function takes INTx. `alloc_msi_vector` answers `None` in both
`arch/aarch64/interrupt_controller.rs` and `arch/riscv64/interrupt_controller.rs`, and
`pci::intx_interrupt_number` swizzles each function onto one of four lines by device number.
`sched::bind_interrupt` (`kernel/src/sched/interrupt_route.rs`) stores one rendezvous per intid,
so a second driver that binds a line silently takes it from the first. The first then waits on an
interrupt that never comes, which is a hang and not an error.

The test legs already attach five PCI functions, so every line already has a driver on it
(`notes/entropy.md`). The entropy driver works around it by reading the used ring before it
blocks. Any further PCI driver on these two architectures meets the same limit, whether or not the runners move: an NVMe,
an e1000e and an xHCI on one board take three of four lines before any virtio device arrives.

Milestone 1064 (PCI enumeration leaves the kernel) relies on related work. A userspace bus server
that programs MSI targets needs the interrupt side settled and, on top of it, interrupt remapping.
1064 supplies neither this milestone nor 1066.

## Interrupt remapping is not this milestone

Milestone 317 (the interrupt-remapping flags, and where MSI confinement actually lives), a
built milestone, reports what each machine offers and turns no remapping on: `GCMD.IRE` stays clear on `x86_64`,
aarch64 has no ITS driver, and riscv64 writes `msiptp` mode Off. No milestone owns turning
interrupt remapping on. The runner move does not need it, since the kernel, not a driver, programs
every MSI target today. 1064 does need it, and its block records that.

## The rule

Ruled 2026-11-11 (UTC): MSI replaces the interrupt lines, and this milestone builds nothing that
duplicates milestone 1068 (the kernel turns interrupt remapping on where an IOMMU offers it).

- Until MSI lands, the kernel refuses with an error a bind on an interrupt line that already has a
  driver, instead of silently taking the line from the first driver.
- aarch64: MSI goes through the GICv3 ITS. One ITS driver serves this milestone and 1068's step 4
  remapping, and this milestone builds it. This milestone does not build GICv2m.
- riscv64: MSI goes through the AIA (APLIC and IMSIC, `-machine virt,aia=aplic-imsic`), which
  replaces the PLIC path. It is the same driver milestone 143 (silicon IOMMU) lists, because an
  RVA23 part does not speak PLIC.
- Shared lines stay open: an intid routing to a small set of rendezvous, each woken by an
  interrupt. nife builds them only if a supported board proves to have no usable MSI controller.
  Nobody has checked yet whether argon or radon has one.

Effort: the ITS L, the AIA L to XL (guesses). With MSI first, the runner move to virtio-pci totals
XL (a guess).

## The work, in order

1. The refusal: `sched::bind_interrupt` answers an error when the intid already has a rendezvous,
   and the first driver keeps its line. The error is new behavior of an existing method, so the
   change records its semantics in `design/decisions/`, and its name ships provisional.
   The test legs' entropy driver binds a line that already has a driver today (`notes/entropy.md`),
   so this step meets it first.
2. The GICv3 ITS driver on aarch64 (command queue, device table, `MAPD` and `MAPTI`) behind
   `alloc_msi_vector`, which then answers `Some`.
3. The AIA on riscv64 behind `alloc_msi_vector`.
4. A system test on aarch64 and riscv64 in which two PCI functions that swizzle onto one INTx line
   each receive their interrupts on their own vectors.

## Done when

1. On aarch64 and riscv64, two PCI drivers whose functions swizzle onto one INTx line each receive
   every interrupt their device raises on their own MSI vector, and a system test shows it on both
   architectures.
2. The kernel refuses a second driver's bind on a line that already has one, the first keeps
   receiving its interrupts, and a test asserts both.
3. The existing PCI and virtio gates pass on every architecture, with no test weakened.
4. `script/fastpath-footprint` reports no change.

## Index row

On aarch64 and riscv64 every PCI function takes one of four INTx lines, and a line routes to one
driver, so a second driver on a line silently takes it from the first. This refuses a second binding
first, then replaces the lines with MSI: the GICv3 ITS on aarch64 and the AIA on riscv64. Any PCI driver on
those architectures needs it, and the runners' move to virtio-pci waits on it.
