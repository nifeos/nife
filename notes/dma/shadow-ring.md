# The shadow descriptor ring, and the two gaps that led to it

*Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes.*

An appendix to [notes/dma.md](../dma.md), moved there on 2026-10-11 (UTC) under §212 (a prose
budget). It records the two features that defeated the in-place
check, the race that remained, and how the shadow ring closes both.

## Two ways the in-place check was still too weak

A second read of the validator turned up a gap that the "walk the chain, check each address" story
misses. The check assumes it sees every address the device will use. Two virtio features break that
assumption, and both are negotiable by the driver, because the kernel passes `DRIVER_FEATURES`
writes straight through.

Indirect descriptors (`VIRTIO_RING_F_INDIRECT_DESC`, feature bit 28). A descriptor flagged
`INDIRECT` does not describe a buffer. Its `addr` points at a *table* of more descriptors, and the
device follows that table. The old validator only understood `NEXT` chains, so it treated the
indirect descriptor as an ordinary buffer: it confirmed the table's address was in-region (it is,
the attacker puts the table in its own region) and stopped. It never walked the inner descriptors,
which is exactly where the attacker writes the kernel address. QEMU's virtio-blk offers this
feature, so a malicious driver could negotiate it and escape. This one is reachable on the current
target, not just on hypothetical async-DMA hardware.

Packed virtqueues (`VIRTIO_F_RING_PACKED`, feature bit 34). The whole ring format changes. The
validator understands only the split ring (separate descriptor table, available ring, used ring),
so against a packed ring it would be reading the wrong bytes while the device reads a ring nobody
checked.

The fix has two layers, both cheap:

1. Refuse the features at negotiation. `sanitize_driver_features` strips bit 28 from the low
   feature word and bit 34 from the high word before the value reaches the device. Without the
   negotiated feature, the spec forbids the driver from using it and QEMU rejects an `INDIRECT`
   descriptor outright. The honest driver asks for neither, so nothing legitimate breaks.
2. Refuse the flag at validation. The validator returns false on any descriptor carrying
   `VIRTQ_DESC_F_INDIRECT`. With the feature already stripped no honest descriptor sets it, so this
   costs one branch and makes the confinement fail closed if layer 1 ever regresses.

The deeper lesson is that both of these, and the time-of-check/time-of-use question below, are the
same shape of problem: the validator reads descriptors out of memory the driver keeps mapped
writable. Patching each feature is treating symptoms. The cure is to stop reading from shared
memory at all.

## The residual race, and the complete fix: a shadow descriptor ring

Even for a plain split ring with no fancy features, the check and the device's use are separated in
time. The kernel validates the descriptors, then writes `QUEUE_NOTIFY`, then the device reads the
descriptors. If the device reads them by its own asynchronous DMA (as real hardware does), the
driver can change a descriptor's `addr` after the kernel validated it and before the device
dereferences it. Validating memory the writer still controls is a time-of-check/time-of-use race by
construction.

On the current target it does not fire. The driver is a single EL0 thread that cannot run between
the kernel's validation and its `QUEUE_NOTIFY` write. QEMU's virtio-blk pops descriptors
synchronously inside that MMIO write, capturing the addresses before any guest code resumes. So
this is recorded as a design limit, not a live hole. On hardware where the device fetches
descriptors asynchronously, it is real.

The complete fix is a shadow descriptor ring:

- Allocate a kernel-private page (not mapped into the driver) for the descriptor table and the
  available ring.
- In `SETUP_QUEUE`, program the device with the addresses of that private page instead of offsets
  in the driver's region.
- On `NOTIFY`, copy the driver's descriptors into the private ring, validate the copy, and only then
  ring the device.

Now the device reads descriptors the driver physically cannot touch, so there is nothing left to
race. The data buffers stay in the driver's region (it has to fill and drain them), and that is
fine. A driver racing its own buffer *contents* only corrupts its own I/O, never anyone else's
memory, because the *addresses* the device uses are the validated copies. The used ring can stay
shared too, since the device writes only indices and lengths there, not addresses. The copy also
subsumes the feature problem: the copy step either refuses `INDIRECT`/packed or recursively copies
and validates an indirect table, so there is one place that decides what the device is allowed to
read.

The cost is small (one page per device, a 128-byte copy per submit, the same validation against the
copy), which is the real argument for doing it rather than masking feature bits forever.

This is now built. `Device` carries a `shadow_base` (one frame, allocated and zeroed in
`register`, never mapped into the driver). `setup_queue` programs `QUEUE_DESC` and `QUEUE_DRIVER`
(descriptor table and available ring) at the shadow page, and leaves `QUEUE_DEVICE` (the used ring)
in the driver's region so the driver still reads its own completions. `validate_and_shadow` replaces
the old in-place `validate_avail`. For each newly-available head it walks the driver's chain,
validates every descriptor, and copies the validated bytes into the shadow at the same index. It then
mirrors the head into the shadow available ring and publishes the shadow's `avail.idx` last. A `dsb`
(`arch::direct_memory_access_write_barrier`) orders the shadow writes before the `QUEUE_NOTIFY`,
because the device is a separate observer. The driver's ABI does not change at all: it still builds
its rings at the same offsets in its own region and reads the same used ring, unaware that the
device now reads a copy.

The invariant that makes it airtight: the kernel only ever writes a validated descriptor into the
shadow. So every descriptor the device can reach in the shadow has an in-region address. The
device bounds its own chain walk to the queue size. A descriptor the driver mutates after the
check lands only in the driver's own copy, which nothing reads. `the_shadow_ring_is_immune_to_a_
descriptor_mutated_after_validation` proves exactly that: validate, then point the driver's
descriptor at the kernel, and assert the shadow still holds the validated address. The end-to-end
disk read (`a_userspace_driver_reads_a_file_from_a_virtio_disk`) still passes, which is the proof
the copy is functionally transparent: the device reads its descriptors from the shadow, DMAs into
the driver's data buffers, and the driver gets its file.

The feature-stripping and the `INDIRECT`-flag refusal stay as defense in depth in front of the
copy. The copy step does not walk indirect tables, so it refuses the flag rather than recursively
shadowing them, which is the simpler and stricter choice for a device that never needs indirect
descriptors.
