---
status: AMENDED
raised: 2026-07-28
decided: 2026-07-28
ratified_by: calef
---

# 23. Multi-queue DMA confinement: the validator's second direction (milestone 30 (the network stack as a confined component))

Decided and built 2026-07-28. A virtio-net device needs two virtqueues (receive on queue 0, transmit
on queue 1). Receive is the direction where the *device writes into* the driver's memory rather than
reading from it. The §18 (the PCIe transport) seam and the shadow-ring validator were queue-0-only.
So the net driver's prerequisite is a proved second queue and a proved second direction. Both are
built under the same confinement discipline as the disk rather than bolted on when a NIC needs it.

**What changed, and what deliberately did not.** The validator (`validate_and_shadow`) did not
change at all. It bounds the address of every descriptor, `addr..addr+len` inside the driver's DMA
region, whichever way the device moves the bytes. That is the same property milestone 32's write
path relied on (§ notes/dma.md, "The write direction"). It is now asserted for the direction where
the *device* is the writer. A receive descriptor aimed at kernel memory would let the device
overwrite the kernel with an inbound packet. It is refused before the device is rung, for the same
reason and by the same check as a read descriptor aimed there. The new work is per-queue state and
plumbing. Each device carries a per-queue last-validated index and a per-queue ring block, and
`setup_queue`/`notify` take a queue number.

The queue-layout contract. Queue `q`'s descriptor table, available ring, and used ring sit at
`q * RING_BLOCK` (0x200) in both the driver's DMA region and the kernel-private shadow frame. One
shadow frame still holds every queue (MAX_QUEUES = 2, so 0x400 of a 4 KiB frame), asserted at
compile time. Queue 0's layout is byte-identical to the old single-queue layout, so the disk driver
needs no change. Its data buffers already begin at 0x200 (= queue 1's block), free because a disk
has no queue 1.

The surface stays narrow (§4 rule 3, the syscall-boundary discipline). No new syscall and no
new object. The `Virtio` capability's existing `SETUP_QUEUE` and `NOTIFY` methods each grew a queue
argument (`SETUP_QUEUE(num, queue)`, `NOTIFY(queue)`). That is the established way this project
adds capability semantics (object revocation grew `Untyped` the same way). The disk passes queue 0
for both, so its ABI is unchanged. An out-of-range queue is `BadQueue`.

Proof. Two new unit tests sit beside the existing confinement suite, on both ISAs. The first is
`the_validator_refuses_an_rx_descriptor_that_escapes_the_region`. An in-region device-writable
receive buffer validates; the same buffer aimed at kernel memory is refused. The second is
`a_second_queue_validates_on_its_own_block`. A good chain on queue 1 lands in queue 1's shadow
block while a sentinel in queue 0's block is untouched. A queue-1 escape is refused the same as
queue 0. See notes/net/the-driver-and-dhcp.md and notes/dma.md.

## Where the validator applies

Ruled 2026-11-10 (UTC). The validator is no named exception to §86's rule that confinement means
an IOMMU (§86 (whether an NVMe driver can leave the kernel)). A virtio driver behind an IOMMU is
confined by the IOMMU. A virtio-mmio driver has no IOMMU under it and is trusted, as an NVMe driver
on a machine without one is.

- QEMU's `virt` on aarch64 (SMMUv3), `virt` on riscv64 (`riscv-iommu-pci`) and `q35`
  (`intel-iommu`) give a virtio device on PCI an IOMMU, and nife has a driver for each
  (`kernel/src/arch/*/iommu.rs`).
- A virtio-mmio device sits behind none of them, so no IOMMU translates its DMA.
- nife has no virtio-iommu driver.

`crates/direct_memory_access_validator` and its Kani harnesses stay in the tree as the proved form
of §86's option 4, doorbell mediation. The kernel switches the shadow-ring validation on only for a
target that runs virtio without an IOMMU, such as a cloud machine, once one is real. The
validation this section describes is the mechanism for that case.
