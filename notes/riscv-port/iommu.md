# The riscv64 IOMMU driver

*Name: provisional, minted 2026-10-23 (UTC) by the riscv64 lane of milestone 929 (comments in the
Linux style across first-party Rust).*

An appendix to [notes/riscv-port.md](../riscv-port.md). It carries the design behind
`kernel/src/arch/riscv64/iommu.rs`. The portable seam, the domain builder and the confinement test
are [notes/iommu.md](../iommu.md).

## The shape

The ratified RISC-V IOMMU (v1.0.1) is the architectural twin of aarch64's SMMUv3, and the rhyme is
the point of building both. A device directory table takes the place of a stream table, keyed by
the PCIe requester id (the specification's `device_id`). A device context takes the place of an STE
and CD: its first-stage `iosatp` is the IOMMU's copy of `satp`, naming an Sv39 root the seam built.
A command queue carries invalidations, and a fault queue records blocked transactions.

On QEMU's `virt` the IOMMU is itself a PCI function, `riscv-iommu-pci` (1b36:0014). Its registers
live in BAR 0, which the kernel places like any other BAR, since the kernel is the firmware here
([notes/pcie.md](../pcie.md)). The seam does that PCI work and hands the driver its register base.

`init` refuses a unit without Sv39 first-stage support. It polls both queues rather than taking
their interrupts. The directory is one level and one page: with `MSI_FLAT` the extended 64-byte
context covers requester ids 0 to 63, all of bus 0. Installing it arms translation, and from that
write an unattached device is blocked with `DDT_ENTRY_INVALID` rather than reaching memory. Before
`init` the reset `ddtp` mode is `Off`, which blocks every transaction, so the pre-init window fails
closed by architecture; the SMMU needs an explicit `GBPA.ABORT` for the same posture.

Sv39 single-stage translation requires `U` on every leaf, because a device does not request
supervisor privilege. That is why the seam builds domains with user-data flags (`paging::domain`).

`attach` writes the address and tag words first and the valid bit last, with a barrier between,
so the IOMMU never sees a valid entry with a stale root. `iohgatp` stays Bare, since there is no
guest. Then `IODIR.INVAL_DDT` with DV set drops that one device's cached context, `IOTINVAL.VMA`
with no address and no PSCID drops every translation, and `IOFENCE.C` orders both before any later
transaction. A command is published with a barrier before the tail write, and the driver polls the
head, which is the architectural contract; QEMU happens to process the queue on the tail write.

`scope_of` answers "the whole bus" whenever the unit is up, because the device tree's `iommu-map` is
an identity over it, so there is no second unit for a device to belong to. VT-d is where that stops
being true (`arch::x86_64::iommu::scope_of`). The RISC-V IOMMU has no firmware table of reserved
DMA regions like VT-d's RMRRs.

## MSI confinement

QEMU's unit sets `CAPS.MSI_FLAT` (measured `CAPS = 0x78c2cf4f10`, bit 22 set, on QEMU 11.1.1's
`virt`), so every context here is the extended format and carries `msiptp`, `msi_addr_mask` and
`msi_addr_pattern`. `attach` writes all of them zero, `msiptp.MODE = Off`, and no MSI page table is
allocated. The machine description reports the capability bit, kept apart from the context size it
implies: one is a capability and the other a layout.

MSI confinement lives in a different place on each architecture. On x86_64 it is a separate IOMMU
feature (`ECAP.IR`), because VT-d intercepts writes to the MSI address range, so DMA remapping alone
does not cover them. On aarch64 it is a separate device, the GICv3 ITS, which this tree's GICv2
machine lacks. Here it is one mode field in a context this driver already writes ([milestone 317
(the interrupt-remapping flags)](../../design/roadmap/0317-interrupt-remapping-flags/riscv64.md)).

## What the proofs cover

The Kani harnesses run on an aarch64 host, never on riscv64, through a proof-only module in
`arch/mod.rs` that exists under `cfg(all(kani, target_arch = "aarch64"))`. Inside it `crate::arch`
is aarch64's, so nothing that calls through `crate::arch` is proved. Neither harness does:
`device_context`, `is_in_directory`, `context_offset` and `iodir_inval_ddt` are word arithmetic.
`script/lint` fails if this file gains an unrecorded `crate::arch` reference, since then a harness
could prove aarch64's code while reading as riscv64's
([notes/kernel-proofs.md](../kernel-proofs.md), stub-list item 8).

The register offsets and bit constants are not proved and cannot be. The harnesses read fields
back at the positions the specification (v1.0.1, chapter 3) gives them, so a misreading makes the
code and the proof wrong together, and the boot-time confinement test stays necessary.

The SMMU splits a 64-bit address across two 32-bit words shared with control bits. This context is
written in whole 64-bit stores, so that hazard does not exist here. The harnesses ask the same two
questions in this format's shape instead.

- Does the context name exactly the domain the seam built? `iosatp.MODE` is Sv39 for every root,
  with no assumption, because MODE 0 is Bare: translation switched off rather than pointed wrong.
  The PPN, reserved bits, PSCID and remaining words are exact for a page-frame root below 2^56.
- Can one requester id's entry, or its invalidation, land on another's? The stride depends on a
  capability read at run time, so the bound must agree with it in both formats, and both are taken
  symbolically. `IODIR.INVAL_DDT` must decode to this device, with DV set; without DV it means
  every device, and the old context could stay cached for the device just re-attached.

A wrong PSCID is invisible to every test. The confinement test attaches one device, and `attach`
invalidates the whole cache after every write, so two domains sharing a tag never both have live
entries. It would matter once invalidation narrows to one PSCID.

## BUGS

- What `msiptp.MODE = Off` does to a transaction is a reading of the specification, not a
  measurement. It says MSI address translation is not performed and the write is translated as an
  ordinary memory access, which would keep an MSI-shaped write inside the `iosatp` domain. No boot
  here has tested it, and nothing forges an MSI.
- No board ships the ratified RISC-V IOMMU (milestone 143 (silicon IOMMU)), and the
  JH7110 has none. So the base 32-byte context format, for silicon without MSI support, has run zero
  times. This architecture is confined in the emulated case only, with no second witness, where
  x86_64 has xenon.
- `take_fault` is drained only by the confinement test; no production fault handler exists.
