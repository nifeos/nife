---
status: BUILT
raised: 2026-09-01
built: 2026-09-01
---
# 215. A PCI function's interrupt reaches nothing on x86_64, so no userspace driver can run there

The x86_64 disk wiring of milestone 164 (x86_64 userspace can't build `aes`) wedged, because a PCI
function's interrupt reached nothing.

It was minted with no hardware gate, on the grounds that QEMU's `q35` reproduces all of it, and
that held: this whole milestone was made on patagonia.

The title is now a statement about the past. A userspace driver on x86_64 reads a file off a
`virtio-blk-pci` disk and writes a block back to it, and the completion arrives as an **MSI-X**
message the device writes straight to the local APIC. Measured below.

## What it was

`pci::intx_irq(base, dev, pin)` is `base + ((dev + pin - 1) % 4)`, and
`arch::x86_64::mmu::PCI_IRQ_BASE` was `0` and said in its own comment that it was a marker rather
than a value. So the virtio-blk function at device 4 pin 1 resolved to intid `0`, and
`arch::x86_64::irq::enable(0)` put that through `isa_routing` to the PIT's line: the confined
block server was armed on the timer. Nothing said so, which is the shape of the bug rather than an
aside. The wiring succeeded, the driver blocked on an interrupt that was never going to be its, and
the suite wedged with no verdict.

## What was chosen, and what it refused

Route B, MSI-X. The routing question is not answered, it is deleted: the device is handed
the address to write and the value to write there, so nothing board-specific has to be encoded
anywhere.

Route A, legacy INTx, is refused, and the refusal is the valuable half. Two versions were
available and both lose:

- *Read ACPI's `_PRT`.* It is AML, this tree has no interpreter, and growing one for four numbers
  is a project that would then have to be maintained and verified. `notes/x86-port/acpi-and-pci.md` had already
  written that refusal down before there was a device on the bus to need it.
- *Hardcode `q35`'s swizzle.* It would pass every gate on this machine. It fails on the
  OptiPlex, or rather it might, and nobody could tell which from here, so milestone 87 would
  discover it at a null modem. That is this project's most expensive place to discover anything,
  and the reason a route with no board-specific table beats one with a plausible table.

MSI-X was more code than the hardcode and much less than the interpreter, so it is not the
convenient answer either way; it won on the OptiPlex risk. `AGENTS.md`'s test applies cleanly:
yes, it would still be the choice if both options cost the same.

And a third thing was refused: the `PCI_IRQ_BASE` fallback. When a machine says it wants MSI
and the function has no MSI-X, bring-up **fails loudly**. Falling back to `intx_irq(0, ..)` there
is the original bug wearing the clothes of a graceful degradation.

## The design, and why the trap handler is three lines

An intid on x86_64 was already two things: a **vector** for a local APIC source (there is no
controller input to name) and a legacy IRQ number for an IO APIC line. An MSI is a local APIC
source in the only sense that matters, because the device writes the vector straight to the APIC.
So an MSI intid is its vector, and three things collapse rather than needing building:

- `irq::enable` has nothing to do for one, and that is an answer rather than a stub: the message is
  edge-delivered and already over. A driver's `Irq::ACK` is correspondingly a no-op.
- `exceptions.rs` asks `sched::irq_route(vector)` directly. The vector-to-intid inversion that
  file's BUGS section records as owed for an IO APIC line never arises here, because there is no
  line in between to have named it. That entry is still true and still owed; it is now owed only
  for the console UART, which is the last candidate.
- Nothing above `arch/` changed shape. `kernel/src/pci.rs` asks `arch::irq::alloc_msi_vector`,
  which answers `None` on both `virt` boards, and their INTx swizzle runs exactly as before.

The three vector bands are disjoint by construction rather than by anyone remembering:
`MAX_REDIRECTION_ENTRIES` is now `MSI_VECTOR_BASE - GSI_VECTOR_BASE`, so an IO APIC that reported an
absurd entry count cannot reach an MSI vector.

## The VT-d fault, which this block owed an answer on

Milestone 164's lane saw two QEMU lines together and could not tell whether they were one bug or
two:

```
qemu-system-x86_64: Interrupt Mask set, irq is not generated
qemu-system-x86_64: vtd_iommu_translate: detected translation failure (dev=00:04:00, iova=0xffde000)
```

**Neither is a confinement gap, and the first line is not about the device at all.** Both come out
of the VT-d unit: `Interrupt Mask set, irq is not generated` is emitted by `vtd_generate_interrupt`
when the unit wants to *report a fault by interrupt* and its own Fault Event Control mask bit is
set. That bit is set because this kernel never programs the fault-event registers: it polls
(`iommu::take_fault`). Read as "the virtio device's INTx is masked", which is how it looks beside a
PCI bug, it is evidence for a diagnosis it has nothing to do with.

And the confinement is proved rather than argued. `kernel::virtio::tests::the_iommu_faults_a_dma_that_escapes_the_domain`
now runs on x86_64 for the first time (it early-returned before, for want of a PCI disk to confine)
and passes: it points a confined device at a frame outside its domain, and VT-d faults on exactly
that frame. The same pair of QEMU lines is what that test's success looks like from outside. Two
userspace round trips through the same confined device complete in the same boot, so `iommu::confine`
covers a real virtio-pci device's DMA region correctly.

That answers the "one piece of work or two" question this block asked: one. No fatal-risk-7
finding here.

## What it delivered, measured

`script/test`, all three legs green on the same tree:

| leg | passed | skipped |
|---|---|---|
| aarch64 | 310 | 3 |
| riscv64 | 314 | 2 |
| x86_64 | 214 | 44 |

The x86_64 leg was 212/44 on the same tree before this milestone (derived: the two tests this block
un-gated are `a_userspace_driver_reads_a_file_over_the_pcie_transport` and
`a_userspace_driver_writes_a_block_over_the_pcie_transport`, and nothing else changed cfg).

**The skip count did not move, and reading that as "no progress" is the mistake.** What moved is
what the passing tests *prove*. The read test asserts `ROUTED_IRQS` increased, so it fails if the
completion arrives by polling rather than by interrupt; and the IOMMU escape test stopped being a
no-op on this architecture.

Correction, 2026-09-04 (the `maintainer/msix-completion-flake` lane): that sentence about the
read test was not true on this architecture when it was written. `arch::x86_64::exceptions::
ROUTED_IRQS` also counted the local APIC timer, which aarch64's and riscv64's never did. A timer
tick landing in the test's window satisfied the assertion whatever the device had done, and a
polling driver would have passed it most of the time. The counter now counts what its two siblings
count and the sentence is true as written. That divergence is also half of why the test flaked here;
see notes/load-sensitive-assertions.md. The 44 skips are still waiting on fixtures and on `std`, which is the
handoff below.

## What this does not cover

The rest of the fixtures. One `virtio-blk-pci` disk is attached. The RedoxFS image, the GPT and
blank disks, the NIC, the GPU, the keyboard and the RNG are each a line in
`helpers/qemu-runner-x86_64.sh` plus a wiring, and every one of them now has a working interrupt
underneath it. That is a proposed milestone and this lane deliberately does not number it
(numbers are the integrator's, and 216 is already the board console). Its measure is the 36 tests
taking a "no RedoxFS disk attached" arm, and its first item is making the FS server's disk lookup
transport-blind, which is the half of milestone 164's revert that outlived the interrupt bug.

The `NIFE_DISK` comment in `xtask` still claims more than the runner attaches, in the sense
that both other runners derive four sibling images from it and this one derives one. That is the
same proposed milestone, not a separate defect.

Interrupt remapping. A VT-d unit with `intremap=on` reinterprets a write to
`0xfee0_0000..0xfef0_0000` as an index into a remapping table and rejects the compatibility-format
message this builds. The runner does not enable it and firmware leaves it off by default, so this
holds today. It is recorded in `arch::x86_64::irq`'s BUGS as the first thing to check if MSI stops
arriving on a machine whose firmware turns it on.

## BUGS

- **An MSI vector is never handed back.** `alloc_msi_vector` is a bump counter over a 63-vector
  band, so a device brought up twice (which the suite does) spends two. A free list with no free
  path would be machinery nothing calls; the number to watch is `find_*_device` calls per boot.
- Every message is addressed to the boot core, exactly as `route_gsi`'s destination is. Nothing
  distributes device interrupts on this architecture yet.
- One MSI-X vector per function, entry 0. Every driver here waits on a single queue's
  completion; a multi-queue driver would want more, and would want a per-queue table index rather
  than the one `PciVirtioDevice::msix_vector` carries.
- **Proved on QEMU, not on silicon.** What only xenon can confirm is in `notes/x86-port/acpi-and-pci.md`: the
  OptiPlex's firmware leaves interrupt remapping off. A real function's MSI-X table is
  reachable once *firmware* rather than this kernel has placed its BARs. A machine with
  more than one local APIC still delivers to the boot core's id.

## Follow-on

- **Refused.** Legacy INTx routing, in both available versions. Reading ACPI's `_PRT` means an AML
  interpreter this tree does not have and would then have to maintain and verify, for four numbers.
  Hardcoding `q35`'s swizzle passes every gate on patagonia and might fail on the OptiPlex, which
  would be discovered at a null modem, this project's most expensive place to discover anything.
  This refusal is milestone 459 (design/roadmap/0459-legacy-intx-interrupt-routing.md), which carries
  it with the condition that would change it.
- **Refused.** A `PCI_IRQ_BASE` fallback for a machine that wants MSI where the function has no
  MSI-X. Bring-up fails loudly instead, because falling back to `intx_irq(0, ..)` there is the
  original bug wearing the clothes of a graceful degradation.
- **Recorded.** `design/roadmap/0215-x86-64-pci-interrupt-routing.md`'s own `BUGS`: an MSI vector is
  never handed back. `alloc_msi_vector` is a bump counter over a 63-vector band, so a device
  brought up twice spends two. The number to watch is `find_*_device` calls per boot.
- **Recorded.** `design/roadmap/0215-x86-64-pci-interrupt-routing.md`'s own `BUGS`: every message is
  addressed to the boot core. Nothing distributes device interrupts on this architecture.
- **Recorded.** `design/roadmap/0215-x86-64-pci-interrupt-routing.md`'s own `BUGS`: one MSI-X vector
  per function, entry 0. A multi-queue driver would want more, and a per-queue table index rather
  than the single one `PciVirtioDevice::msix_vector` carries.
- **Recorded.** `notes/x86-port/acpi-and-pci.md` holds what only xenon can confirm: the OptiPlex's firmware
  leaves interrupt remapping off. A real function's MSI-X table is reachable once firmware
  rather than this kernel has placed its BARs. A machine with more than one local APIC
  still delivers to the boot core's id.
- **Recorded.** `kernel/src/arch/x86_64/interrupt_controller.rs`'s `BUGS`: a VT-d unit with `intremap=on` rejects
  the compatibility-format message this builds, so it is the first thing to check if MSI stops
  arriving on a machine whose firmware turns remapping on.
- **Recorded.** `kernel/src/arch/x86_64/exceptions.rs`'s `BUGS`: the vector-to-intid inversion is
  still owed. MSI never needs it, because an MSI intid is its vector; it is now owed only for the
  console UART, which is the last candidate.
- **Milestone 303.** The x86_64 test fixtures now that a function's interrupt works: milestone 303
  (x86_64's FS service has a server and no disk it can find) builds the FS server's transport-blind
  disk lookup and attaches the RedoxFS image. The remaining devices are a newer numbered block.

## Index row

A userspace driver now reads a file off a `virtio-blk-pci` disk on x86_64 and writes a block back,
with the completion arriving as an MSI-X message the device writes straight to the local APIC. MSI-X, and the INTx refusal is the valuable half: reading ACPI's `_PRT` needs an AML
interpreter, and hardcoding q35's swizzle would pass every gate here and might fail on xenon,
which milestone 87 would discover at a null modem. MSI-X has no board-specific table to be wrong
about. The design fell out rather than being built: an MSI intid is its vector, so `irq::enable` and `Irq::ACK` are correctly no-ops and the vector-to-intid inversion an IO APIC
line would need never arises. A machine that wants MSI and meets a function without it now refuses loudly instead of falling back to `intx_irq(0, ..)`, which was the original bug. **The
VT-d fault milestone 164 saw is not a confinement gap**: both QEMU lines come from the VT-d unit
itself (`Interrupt Mask set` is its own fault-event interrupt, masked because this kernel polls).
The escape test now runs on x86_64 for the first time and passes. Legs: aarch64 310/3, riscv64
314/2, x86_64 214/44 (from 212/44). The remaining fixtures (RedoxFS, GPT, blank, NIC, GPU,
keyboard, RNG) and a transport-blind FS-server disk lookup are a proposed milestone this lane left
unnumbered.
