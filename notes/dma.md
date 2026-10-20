# Confining DMA without an IOMMU

The security audit's most severe finding: a userspace virtio driver could make the device DMA over
any physical memory. This note is how that hole was closed. The detail behind it is in
[dma/](dma/README.md).

## Why it was a hole

The whole kernel is built to confine what a process can touch, and the MMU enforces it perfectly:
a process at EL0 cannot read, write, or execute a byte it was not mapped. But the device is not
a process, and it is not behind the MMU. A virtio block device is a second bus master: it reads
descriptors and does DMA against raw *physical* addresses, and page-table permissions: W^X, the
AP bits, the TTBR0/TTBR1 split, everything: simply do not apply to it.

The driver writes those physical addresses into the queue's descriptors. In milestone 9 the driver
owned the device registers and rang it directly, so a *hostile* driver could put the physical
address of the kernel image (or another process's frames) into a descriptor, mark it
device-writable, and issue a read. The device would DMA disk contents straight over kernel memory.
Nothing faulted, because the device honors no permissions. The driver was confined; the device it
drove was not.

Milestone 9's isolation was real for a driver *bug* (a bad address points at the driver's own
unmapped memory and faults it) and false for a driver *malice* (a deliberate kernel address just
succeeds). That is the gap.

## Why not an IOMMU

An IOMMU is the *clean* answer: it sits between the device and memory and translates every address
the device emits, confining it to a region the kernel programmed: generically, with zero device
knowledge in the kernel. That is why real systems use one, and it is what DECISIONS §10 meant by
"they had to bolt the isolation on afterwards with an IOMMU."

It is not reachable from here. QEMU `virt`'s SMMUv3 only covers the PCIe bus, not the platform
virtio-mmio devices we use. Getting behind it would mean switching to virtio-pci and writing a
PCIe enumerator, an SMMUv3 driver, and the virtio-pci transport: three substantial subsystems for
one gap. And real cheap hardware (a pre-4 Raspberry Pi) often has no usable IOMMU either, so the
software approach is the more broadly useful skill.

One honest nuance: an IOMMU is not *free* even when you have one. Someone still programs the
stage-2 tables that confine the device, and that someone is the kernel. The IOMMU buys
*generality* (no transport knowledge needed), not the absence of a trusted DMA policy.

## The fix: the kernel mediates the two DMA-critical powers

Without an IOMMU, something trusted must check every address the device will touch and refuse
anything outside the driver's region. The kernel takes back exactly two powers and leaves the rest
to the driver:

1. **The ring addresses.** The kernel programs `QUEUE_DESC/DRIVER/DEVICE` to fixed offsets in the
   driver's DMA region (`SETUP_QUEUE`). The driver never chooses them, so the rings themselves are
   always inside the region.
2. **The "go" signal.** The driver cannot write `QUEUE_NOTIFY`; it calls `NOTIFY`, and the kernel
   **validates every newly-published descriptor before ringing the device.** If any descriptor's
   `addr..addr+len` falls outside the region, the kernel refuses and the device is never told to
   go.

Everything else stays in the userspace driver: feature negotiation, the block request format,
sectors, reading results. The kernel owns the virtio transport (the descriptor and available
ring layout, enough to validate DMA) and knows nothing about block devices. The driver reaches
the device only through a `Virtio` capability; the device registers are no longer mapped into it.

This is a software stand-in for an IOMMU. It is less general (it understands the transport) but it
closes the hole: the device can only ever DMA within the driver's own region, so a hostile driver
can, at worst, corrupt itself.

## What is proved, and what is only mitigated (read this before quoting the milestone)

Milestone 35 machine-checked this boundary, and the shortest true summary of the result is longer than
one clause. So, the whole map first, because "DMA confinement is proved" said flat is wrong in a way
that matters for milestone 16a:

| The address path | What confines it | The evidence |
|---|---|---|
| An address in a **virtqueue descriptor** (every byte a disk or a NIC touches, and the GPU's command ring) | the shadow-ring validator below, plus the IOMMU where there is one | **Proved for every input.** Seven Kani harnesses over `crates/dma_validator`: both directions, indirect descriptors, chain cycles, ring-index wraparound, overflowing arithmetic, multi-queue blocks, and the mutated-after-validation race. Plus the end-to-end attacker tests, both ISAs, both transports. |
| The **page set of an IOMMU domain** (which frames the hardware will translate at all) | the domain builder | **Proved for every grant.** Six harnesses over `paging::domain`: the domain maps every whole page of the grant and no byte outside it. Format-independent, so one proof covers SMMUv3 and the RISC-V IOMMU. The remaining link (`Mapper::map` writes exactly the one leaf it is told) stays tested on both formats. |
| An address a driver writes into a **register page that names DMA addresses** on a machine with no IOMMU (radon's SD/MMC and Ethernet servers, e1000e without VT-d) | the driver's own code, and nothing else | **None.** The kernel cannot see these writes. Each driver says so in its own header and `BUGS`, and the booted system starts radon's two only once a bench boot proves them. |
| An address in a **device command payload** (virtio-gpu resource backings) | the IOMMU, and nothing else | **Partly proved, by the row above, and only there.** The validator cannot see these addresses at all: they are not in its input, so nothing about them is provable *from the transport*. What the domain proof buys is that the barrier's allow-list is exact; that the hardware then faults an address outside it stays an attacker test. |

Note what the middle row does for the third, because it is the one useful thing milestone 35 could prove
about the payload path and it is easy to miss. A payload-borne address is stopped by having no
translation in the device's domain, so "the domain maps exactly the grant" is precisely the property
that barrier needs, and it is now proved for every grant rather than tested on a few. The payload path
therefore improved from "tested end to end" to "the allow-list is proved exact, the hardware honoring it
is tested end to end". That is a real narrowing of the gap and it is *not* the same as closing it: the
transport still cannot see these addresses, and the hardware is still doing the enforcing.

That third row is the one to carry away. A virtio-gpu's backing addresses ride inside a
`RESOURCE_ATTACH_BACKING` command payload (DECISIONS §29, notes/framebuffer-contract.md). The
kernel bounds the descriptor that *carries* the command, so the payload bytes are in-region; the
addresses inside those bytes are never parsed. Proving the validator harder does nothing for this,
because the addresses never enter it, and teaching the transport to read virtio-gpu commands would put
device knowledge in the layer DECISIONS §18 keeps device-neutral and start a per-device arms race.
`the_iommu_refuses_the_gpu_a_framebuffer_outside_the_drivers_grant` is the evidence that the IOMMU
catches it, on both ISAs, by asserting on the hardware's own fault queue.

And this is exactly where the argument for proving the validator now, rather than later, inverts.
The reason milestone 35 was load-bearing is that milestone 16a's board, the VisionFive 2, **has no
IOMMU**. On first silicon the validator stops being defense in depth and becomes the sole DMA
confinement, so it had better be proved rather than sampled. That reasoning holds for the descriptor
path because the validator covers it. For the payload path there is no such comfort: with no IOMMU,
**nothing covers it.** Not the validator (wrong input), not the hardware (absent). A display driver
on that board is therefore either trusted with all of physical memory, or the transport grows a
virtio-gpu-aware check and pays the §18 cost on purpose. Whoever sequences 16a picks one; this note
exists so it is a decision and not a discovery. Under HVF the same gap is open, PCIe DMA being
unconfined there by standing default.

## The validator

Since milestone 35 (prove the DMA-confinement boundary) the validation logic lives in
`crates/direct_memory_access_validator`, a host-testable pure-logic crate the kernel's
`validate_and_shadow` calls. It is **machine-checked**. Seven Kani harnesses prove that the walk
terminates, and that no descriptor it copies into the shadow escapes the granted region or is
indirect. That holds for every input (both directions, multi-queue, chain cycles, ring-index
wraparound, and the mutated-after-validation race). This was the last isolation boundary that was
attacker-tested but not proved. notes/verification.md has the harness table and, more important, the
bounds with their justifications. The short version: the queue size the proof fixes is the system's
own `QSIZE`, not a proof convenience. The loop bounds are set one above what the code can need so
Kani's unwinding assertion turns them into a termination proof. The crate also now *owns* the ring
layout constants that `kernel/src/virtio.rs` aliases, because a proof about a copy of the layout
proves nothing about the layout that runs. The rest of this section describes what that logic does.

`kernel/src/virtio.rs::validate_and_shadow` is the security-critical code. On `NOTIFY` it walks the
available ring from the last-validated index to the current one, and for each new head follows the
descriptor chain, checking that every `addr..addr+len` (with overflow rejected) lies within
`[direct_memory_access_base, direct_memory_access_base + direct_memory_access_size)`. Each validated
descriptor is *copied into the shadow ring the device reads* (see the shadow-ring section below);
that copy, not the validation alone, is what closes the race. The chain walk is bounded by the queue
size, so a malicious `next`-pointer cycle cannot hang the kernel. The *outer* walk is bounded too:
the available ring holds only `QSIZE` slots, so a batch claiming more than `QSIZE` newly-available
entries is malformed and refused before a single descriptor is touched. Without that guard a driver
could set `avail.idx` tens of thousands past the last-validated index and spin the loop up to 65535
times, all under the `DEVICES` lock with interrupts masked (bounded, single-core, but a needless
latency spike). It is written to take the driver and shadow ring addresses and read/write word
pairs, so a test builds fake regions and exercises it directly.

## The proof

Three tests, and one of them is the attack:

- `the_validator_refuses_a_descriptor_that_escapes_the_dma_region`: a unit test that builds a fake
  region and checks a good chain passes, a descriptor pointing at kernel memory is refused, one
  running past the end is refused, and a cycle terminates.
- `the_kernel_refuses_a_dma_descriptor_that_escapes_the_drivers_region`: **end to end**: a
  malicious driver at EL0 holds a real `Virtio` capability, points a descriptor at the kernel
  image, and submits. The kernel refuses; the driver reports it.
- `a_userspace_driver_reads_a_file_from_a_virtio_disk`: the legit path still reads a file off the
  disk through the validated transport.
- `the_validator_refuses_an_indirect_descriptor` and `the_kernel_refuses_an_indirect_descriptor_escape`
  the indirect-descriptor bypass below, refused as a unit and end to end.
- `feature_negotiation_strips_indirect_and_packed`: a driver asking for either ring-layout feature
  gets the bit cleared before the device sees it.

Verified the confinement can fail: with the region check stubbed to accept everything, the attack
goes through and the end-to-end test fails.

## The shadow descriptor ring

An in-place check of descriptors in driver-writable memory was too weak in three ways. Indirect
descriptors (`VIRTIO_RING_F_INDIRECT_DESC`) hid the real addresses in a table the validator never
walked, and QEMU's virtio-blk offers the feature. Packed virtqueues (`VIRTIO_F_RING_PACKED`) change
the ring format the validator reads. And a device that fetches descriptors asynchronously can read
a descriptor the driver changed after the kernel validated it.

The cure is to stop reading from shared memory. The device reads a shadow descriptor table and
available ring in a kernel-private frame that is never mapped into the driver, and the kernel only
ever writes a validated descriptor into it. The data buffers and the used ring stay in the driver's
region, because the device writes no addresses there. `sanitize_driver_features` still strips the
indirect and packed bits, and the validator still refuses the `INDIRECT` flag, as defense in depth.
`the_shadow_ring_is_immune_to_a_descriptor_mutated_after_validation` proves the copy. The full
account is in [dma/shadow-ring.md](dma/shadow-ring.md).

## The write direction, the receive direction, and kill-mid-write (milestones 32 and 30)

The validator bounds addresses, never directions, so the write path (milestone 32 (a real filesystem) phase 1)
and the receive queue of a NIC (milestone 30 (the network stack as a confined component)) needed no change to it. A descriptor aimed at kernel memory is
refused whichever way the device would move the bytes. A NIC's second queue sits at its own
`RING_BLOCK` in both the driver's region and the shadow frame.

The kill-mid-write test found that a completion is the used ring advancing, not a wakeup, so
`complete_block` loops until `used.idx` moves. It also leans on a deliberate leak: a dead driver's
DMA frame is never reclaimed. Whoever builds DMA-region reclaim must quiesce the device first, or
the device performs a use-after-free. Both directions, the three load-bearing facts and the queue
layout are in [dma/directions.md](dma/directions.md).

## BUGS: the confinement is one-directional, and two drivers read it as if it were not

Everything above confines the driver to device direction: which addresses the device will be
sent to, and (through the IOMMU) which it can translate at all. Nothing here says anything about
what the device writes back, and it cannot: the used ring is inside the driver's own granted
region, so a device writing it is a device doing its job.

Milestone 43's audit (notes/shared-page-audit.md, finding 6) found two drivers reading that as a
guarantee it never was. `components/src/virtio_net_transport.rs` and
`components/src/keyboard_driver.rs` took the 32-bit buffer index out of the used ring and used it
unchecked. `rx_buf(id) = 0x400 + id * 0x2C0` leaves the one-page DMA region at `id = 4` and lands on
the driver's own heap around 1.5 million. So a device that lies once makes the network driver copy
its heap into a frame and hand it to smoltcp. The receive length was unbounded the same way. Both
now fail closed, and `entropy.rs` had always clamped, so the shape was an omission and not a policy.

What is still missing is the negative control. This tree tests DMA confinement by making the
*driver* attack (`crates/virtio`'s `run_attack` and the indirect-descriptor variant, both proving
the kernel refuses). There is no way to make the *device* attack, so the direction the IOMMU and the
validator exist for is the one with no test, and the two fixes above are unproven for that reason. A
harness that can write an arbitrary used ring under a driver is proposed as its own milestone. Under
QEMU with slirp nothing lies; on a board, or behind any device the host does not fully own, this is
the assumption doing the work.

## The tradeoff, stated plainly

This moves the virtio *transport* into the kernel, which slightly walks back milestone 9's "the
driver operates the device." That is a real cost, taken deliberately, and it is defensible:
confining DMA *is* a transport concern, and the kernel still knows nothing about block devices. The
alternative (trusting the driver with all of physical memory) is what a monolithic kernel does
with an in-kernel driver, and the whole point of this project is not to.

## And now, in hardware (milestone 16b)

The "why not an IOMMU" section above was written when we had none reachable. Milestone 16b reaches
one, on both boards: an SMMUv3 on aarch64 and the ratified RISC-V IOMMU on riscv, in front of the
PCIe bus, each confining a device to a domain the kernel programs (notes/iommu.md, DECISIONS §20).
So the clean answer is now the real answer for the PCIe transport.

This shadow ring is not removed. It is demoted to defense in depth. Two reasons it stays. First,
virtio-mmio has no IOMMU in front of it on either board, so the software confinement is still the
only thing guarding the mmio disk. Second, even where the IOMMU is present, keeping both means a
regression in either layer is caught by the other. The transport still refuses a format it cannot
police (indirect, packed) before a descriptor is built. The IOMMU still faults an escaping address
even if a validator bug ever let one through. The feature-stripping and the shadow copy cost almost
nothing and buy a second independent barrier, which is the right trade for the one DMA path that can
reach the whole machine.
