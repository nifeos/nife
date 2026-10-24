---
status: PROPOSED
raised: 2026-10-24
milestone_dependencies: 784
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# The confined transport takes the crate's queue sizes

Raised on 2026-10-24 (UTC) by the lane for milestone 948 (the virtio GPU device runs on
virtio-drivers), branch `milestone/948-virtio-gpu-on-virtio-drivers`. It found what stops 784's
items (c1) to (c3). Name: provisional.

Reuse: `crates/direct_memory_access_validator` already takes the queue size as an argument. The
change is what the kernel passes and where it places the rings.

## The gap

Every crate device in `vendor/virtio-drivers` fixes its own queue size and allocates its own rings.
Input takes 32, block 16, and the GPU 2 in 0.13.0 or 16 on upstream's main line. The kernel's
confined transport (`kernel/src/virtio.rs`, notes/dma.md) fixes both instead:

- 8 entries per queue. `setup_queue` refuses more. `notify` passes the fixed `QSIZE` to
  `validate_and_shadow`, so a smaller queue is shadowed at `idx % 8` while the device reads
  `idx % num`.
- Ring offsets `queue * 0x200 + {0x000, 0x080, 0x100}`, the same in the driver's region and the
  shadow page. The crate's `VirtQueue::new` takes two page-aligned `Hal::dma_alloc` allocations:
  descriptors plus available ring, then the used ring. It passes their addresses to
  `Transport::queue_set`, and the kernel would have to ignore them.

So only net, whose size is a type parameter, can run on the crate behind the confinement today.
The probe behind 784's (c0) drove the keyboard from the kernel and never met this.

## A shape

The kernel still chooses every ring address, and the driver never does. The choice now follows the
negotiated size and the crate's flexible layout:

- `SETUP_QUEUE` takes `num` up to a new ceiling (64 covers every crate device) and records it per
  queue, and `NOTIFY` passes that `num` to the validator.
- In the driver's region, queue `q` owns two pages: descriptors and available ring in the first,
  used ring in the second. A nife `Hal` answers the crate's two allocations for queue `q` with
  exactly those pages, and the nife `Transport::queue_set` refuses any other address.
- The shadow page keeps every queue's descriptor table and available ring. At 64 entries that is
  1,158 bytes a queue, so two queues still fit one frame.
- The existing drivers move to the new layout in the same change. A per-device layout flag would
  avoid that, and it is the exception this tree warns about.

The validator's Kani harnesses fix `QSIZE` at 8 (notes/dma.md, "The proof"). They have to cover the
new ceiling, and `distinct_queues_occupy_disjoint_blocks` the new layout.

## Why an architect rules it

notes/dma.md's confinement is the kernel's DMA policy, and its layout is proved. Changing what the
kernel places and validates is a design fork, not a lane's choice. Three of 784's four device
milestones wait on it.

## Exit

The keyboard, block and GPU drivers run on the crate's own queue sizes behind the confinement, on
all three architectures. The validator's proofs cover the new ceiling. The DMA confinement system
tests pass unchanged.
