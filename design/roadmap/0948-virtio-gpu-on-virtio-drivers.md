---
status: IN-PROGRESS
branch: milestone/948-virtio-gpu-on-virtio-drivers
raised: 2026-10-24
milestone_dependencies: 784
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 948. The virtio GPU device runs on virtio-drivers

*Name: provisional. The integrator assigned the number 948 on 2026-10-24 (UTC). The title and slug
are drafts.*

Item (c2) of [784 (reuse what the tree wrote)](0784-reuse-what-the-tree-wrote.md): `gpu_driver`
moves onto `vendor/virtio-drivers`, which carries 0.13.0 plus rcore-os/virtio-drivers#232 under
item (c0). 784 takes one device per milestone; input, block and net are 947, 949 and 950. The exit
criterion is 784's: a `script/bench` number before and after, on all three architectures, recorded
here.

Reuse: the whole point. What stops it is below, and it is an architect's call.

## Index row

`gpu_driver` drives the virtio GPU through `vendor/virtio-drivers` instead of its own rings and
command encoding, with a bench number before and after on all three architectures. Waits on a
ruling: the carried crate cannot flush a damaged rectangle, and the kernel's confinement cannot
host its queues.

## What the driver does today

`components/src/gpu_driver.rs` is a confined EL0 program, 303 code lines. It owns no device
registers and no ring addresses. The kernel's `Virtio` capability answers register reads and
refuses the queue-address and notify registers. It places each queue's rings at fixed offsets in the
driver's DMA region (`SETUP_QUEUE`), and validates and shadows every descriptor before ringing the
device (`NOTIFY`; notes/dma.md). Against that transport the driver:

1. negotiates `VERSION_1` and, when offered, `ACCESS_PLATFORM`, and checks `FEATURES_OK` read back;
2. runs one control queue of 8 entries and never sets up the cursor queue;
3. creates one 1120x624 `B8G8R8X8_UNORM` resource whose backing is pages 1 onward of its DMA region,
   the frames the client also maps (`graphics_contract`);
4. on each client `FLUSH`, sends `TRANSFER_TO_HOST_2D` and `RESOURCE_FLUSH` for the damaged
   rectangle only;
5. waits for each completion on the device's interrupt, deciding by the used index;
6. in its second role, attaches a 4-byte backing at a frame outside its IOMMU domain, for the
   escape test in `system_tests/src/user/display_tests.rs`.

## What the carried crate can do

Read in `vendor/virtio-drivers/src/device/gpu/mod.rs` and `src/queue.rs`, at the pin in
`vendor/virtio-drivers.pin`.

| Driver need | 0.13.0 plus #232 | Upstream `master` at `c76e09d7` (2026-10-20 UTC), unreleased |
|---|---|---|
| 1. `ACCESS_PLATFORM` | yes (#232). `begin_init` never reads `FEATURES_OK` back, so the driver checks status after `new` | the same |
| 2. queue size | `QUEUE_SIZE = 2`, and `new` sets up the cursor queue too | `QUEUE_SIZE = 16` (#257) |
| 3. resource on our frames | `change_resolution(w, h)` takes our size; the backing comes from `Hal::dma_alloc`, which our `Hal` can answer with the surface pages. Format fixed at `B8G8R8A8_UNORM` (code 1, ours is 2) | the same format |
| 4. damaged-rectangle flush | **no.** `flush()` sends the whole resource; `transfer_to_host_2d` and `resource_flush` are private | yes: both public since #257 (merged 2026-08-14 UTC) |
| 5. completion by interrupt | **no.** `request` calls `add_notify_wait_pop`, which spins on the used index; nothing in `VirtIOGpu` waits on an interrupt | no; #265, open, makes the control queue asynchronous |
| 6. backing at a chosen address | **no.** `resource_attach_backing` is private | yes, public since #257 |

So the carried crate cannot do items 4 and 6, and does item 5 a different way. The lane brief says to
stop there rather than patch the vendored crate beyond (c0), and it stopped.

Item 4 is not cosmetic. A whole-resource transfer moves 2,795,520 bytes (1120 x 624 x 4) on every
flush. A 14x26 glyph cell is 1,456 bytes, so one keystroke echo asks the device to copy about 1,900
times what changed. That figure is arithmetic, not a measurement.

Item 6 could be forced without the crate's help: a `Hal` whose `dma_alloc` answers the victim
address for a 1x1 resource makes `change_resolution(1, 1)` attach 4 bytes there. That is a test
reaching through a safety contract: `Hal::dma_alloc` promises memory the driver may touch. The test
would also stop exercising the honest driver's bring-up path, which is why the role shares the
binary today.

## What the kernel's confinement cannot host

This half is not about the GPU and is not the crate's fault. It blocks 947 (input, `QUEUE_SIZE =
32`) and 949 (block, 16) the same way, and is written up once as a proposal:
[the confined transport takes the crate's queue sizes](proposals/the-confined-transport-takes-the-crates-queue-sizes.md).

- The kernel drives exactly 8 entries per queue. `SETUP_QUEUE` refuses more (`kernel/src/virtio.rs`,
  `setup_queue`), so #257's 16-entry queue cannot be set up at all.
- A smaller queue is accepted and then mis-shadowed. `notify` hands `validate_and_shadow` the fixed
  `QSIZE` of 8, so the shadow's available-ring slot is `idx % 8` while a 2-entry device reads
  `idx % 2`. From the third request the device would read a stale head.
- The rings sit where the kernel says, not where the crate allocates them. The crate's
  `VirtQueue::new` takes two page-aligned allocations from `Hal::dma_alloc` (descriptors plus
  available ring, then the used ring) and passes their addresses to `Transport::queue_set`. The
  kernel ignores any address and uses `queue * 0x200 + {0x000, 0x080, 0x100}`. Only an 8-entry
  queue's available ring lands at `0x080`, and a `Hal` cannot hand out the used ring at `0x100`
  without breaking its own page-alignment and no-aliasing contract.
- The kernel silently clears `INDIRECT_DESC` from the driver's feature word
  (`sanitize_driver_features`), while the crate believes what it wrote and builds indirect
  descriptors, which `NOTIFY` then refuses. A nife `Transport` must hide `INDIRECT_DESC` (and
  `EVENT_IDX`, whose `used_event` the shadow does not carry) from `read_device_features`, so the
  crate negotiates only what the confinement polices.

## The bench has no row to move

No `script/bench` row exercises a virtio device under icount, on any architecture
(`bench/baseline-*.txt`). Device rows (`fs_read`, `fs_throughput`) run only under `--real --smp`,
which is Hypervisor.framework on aarch64. An interrupt-driven flush parks the hart in `wfi`, and
under `-icount shift=0,sleep=off` an idle hart warps virtual time, so a flush row is not
deterministic until someone shows it is. The crate's spinning completion would not idle, which
makes a before and after on the same row measure two different waits. 784's exit criterion
therefore needs a new row whose determinism is measured first, built before the driver moves so
that "before" is the current driver.

## Needs calef

How should the GPU reach the crate, given that 0.13.0 plus #232 cannot flush a damaged rectangle or
attach a chosen backing? Options, recommendation first:

- **(a) Carry #257 as a second divergence in `vendor/virtio-drivers`.** It is upstream's own merged
  code, nothing of ours. It applies cleanly to the carried tree, adding 650 and removing 35 lines,
  all in `src/device/gpu/mod.rs`, so the fork test still passes. It brings the 2D primitives
  public (items 4 and 6) and virgl 3D code we do not call. 948 then builds after the confinement
  proposal lands and after 947 has written the nife `Transport` and `Hal`, which the GPU reuses.
  It drops when a release carries #257. Recommended: one queue implementation for every virtio
  device means the confinement supports one driver-side layout, not two forever, and the damaged
  rectangle stays.
- **(b) Wait for an upstream release carrying #257.** Nothing new is carried; asking the
  maintainers for a release is calef's act. `gpu_driver` stays as it is until then. The
  confinement proposal is needed either way.
- **(c) Take 0.13.0 plus #232 as carried.** Every flush sends the whole surface, the escape test
  goes through the `Hal` substitution above, and the resource format changes to `B8G8R8A8`. Needs
  the confinement change for a 2-entry queue. Not recommended: it trades the damaged rectangle and
  an honest escape test for not carrying upstream code.
- **(d) Keep `gpu_driver` ours, with a `Reuse:` line.** The device writes the driver only a 24-byte
  response header and a 408-byte display description, and the driver reads three words of them.
  So the trust 784 ranks this item for is smallest here. Against (a): the confinement would keep the
  fixed 8-entry layout alongside the crate's.

Under (a) or (b), item 5 stays a difference: the crate spins on the used index for every command.
That burns the hart for each device round trip, which the bench row would show, and #265 is the
upstream path out of it.

## What is built

The analysis above and its proposal. No code: every option but (d) needs the confinement change
first, and (d) needs only a `Reuse:` line.

## Work once ruled

Under (a), in order:

1. The bench row: a timed run of `FLUSH`es of one glyph cell and of the whole surface, driven
   through `display_service`. Measure its determinism on all three architectures, then record it
   here as "before".
2. The second divergence, in `vendor/virtio-drivers.divergence.patch` and `vendor/README.md`.
3. `gpu_driver` on the crate through 947's `Transport` and `Hal`. It needs a heap for the crate's
   `Box`es, and bounce space for its 4 KiB request and response buffers. That is two more frames in
   `display_service::DMA_PAGE_FRAMES` and in the progenitor's run.
4. The escape role through `resource_attach_backing`.
5. The bench row again, recorded here as "after".
