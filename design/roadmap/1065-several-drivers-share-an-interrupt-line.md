---
status: NOT-STARTED
raised: 2026-11-10
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1065. Several drivers share an interrupt line, or MSI replaces the lines

*(The number, title and slug are provisional.)*

Reuse: the kernel's own route table and the virtio drivers' wait loop for shared lines; the GICv2m,
ITS and AIA register layouts from their specifications for MSI. No third-party crate fits a kernel
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

## The two ways, and the fork

1. **Shared lines.** An intid routes to a small set of rendezvous, and an interrupt wakes each.
   The line stays masked until every bound driver has acknowledged it, since INTx is level
   triggered. The virtio drivers' wait loop (wait, quiet the device, re-enable, let `used.idx`
   decide, `notes/virtio.md`) already tolerates a spurious wake; NVMe, e1000e and xHCI need the
   same check of their own completion state. This works on every board with INTx, argon's GIC-400
   among them. Effort: M (a guess).
2. **MSI.** aarch64: a GICv2m driver, which TCG with `gic-version=2` and HVF both offer
   (`v2m@8020000`, `notes/machine-discovery.md`). Under TCG with `gic-version=3` an ITS driver
   serves instead. A GICv2m frame does not identify the device that wrote it, so it confines
   nothing; the ITS does. riscv64: the AIA (APLIC and IMSIC, `-machine virt,aia=aplic-imsic`),
   which replaces the PLIC path. Milestone 143 (silicon IOMMU) already lists that driver, because
   an RVA23 part does not speak PLIC. Effort: M for GICv2m, L to XL for the AIA (guesses).

The sizing recommends shared lines first. It is the smaller change, it serves every board with
INTx, and MSI without remapping on aarch64 confines no interrupt anyway. Part of that
recommendation is about effort. If both cost the same, MSI is the end state for PCI hardware: one
vector per queue, no sharing, and the only interrupt an RVA23 part has. This is a real fork, so
the architect chooses; the milestone builds the one the architect rules, and the other stays open.

With shared lines, the runner move totals L (a guess). With MSI on both architectures first, it
totals XL (a guess).

## The work, in order

1. The ruling between the two ways.
2. Shared lines: a fixed-size set of rendezvous per intid in `sched/interrupt_route.rs`, the
   acknowledgement count, and the completion check in each PCI driver that blocks. If binding a
   second driver changes what the `Irq` capability means, that change goes to the architect as a
   §10 (the capability-based microkernel process model) amendment first.
   Or MSI: GICv2m (or the ITS) and the AIA behind `alloc_msi_vector`, which then answers `Some`.
3. A system test on aarch64 and riscv64 that binds two drivers to one line (or two vectors) and
   shows both receive their interrupts.

## Done when

1. On aarch64 and riscv64, two PCI drivers whose functions swizzle onto one line each receive
   every interrupt their device raises, and a system test shows it on both architectures. Under
   MSI, each function has its own vector and the same test passes.
2. Binding a second driver to a line no longer removes the first, and a test asserts it.
3. The existing PCI and virtio gates pass on every architecture, with no test weakened.
4. `script/fastpath-footprint` reports no change.

## Index row

On aarch64 and riscv64 every PCI function takes one of four INTx lines, and a line routes to one
driver, so a second driver on a line silently takes it from the first. This lets several drivers
share a line, or replaces the lines with MSI, whichever the architect rules. Any PCI driver on
those architectures needs it, and the runners' move to virtio-pci waits on it.
