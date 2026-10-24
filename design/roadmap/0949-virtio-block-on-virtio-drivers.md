---
status: NOT-STARTED
raised: 2026-10-24
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: Apple Silicon host with Hypervisor.framework, for the filesystem throughput rows
specific_machine: none
needs_person: no
---
# 949. The virtio block device runs on `virtio-drivers`

*Name: provisional. The number 949 was assigned by the integrator on 2026-10-24 (UTC); the title
and slug are drafts. Branch `milestone/949-virtio-block-on-virtio-drivers`.*

Item (c3) of [784 (reuse what the tree wrote)](0784-reuse-what-the-tree-wrote.md): the block roles
of `crates/virtio` drive the device through `vendor/virtio-drivers` (0.13.0 plus upstream #232,
item (c0)) instead of hand-built rings. 784 asks for one device per milestone, so this one is block
only. Its siblings are 947 (input), 948 (GPU) and 950 (net).

Reuse: this milestone is the reuse. `virtio-drivers` was weighed in 784's table and taken by
calef on 2026-10-22 (UTC) as the vendored fork; the new crates it brings into the base graph
(`zerocopy`, `safe-mmio`, `thiserror`, `enumn`, `log`, `bitflags`) are the ones 784 listed.

## What the crate has to meet

A block driver here runs at EL0 and holds no device registers. The kernel hands it a DMA region,
an `Irq` capability and a `Virtio` capability whose four methods are the whole transport
([notes/virtio.md](../../notes/virtio.md), "The driver crate"). The kernel keeps the two powers that
aim the device: it chooses the ring addresses in `SETUP_QUEUE`, and on `NOTIFY` it copies and
validates the driver's descriptors into a private shadow ring before ringing the device
([notes/dma/shadow-ring.md](../../notes/dma/shadow-ring.md)). The crate's driver therefore sits on
two adapters this milestone writes in `crates/virtio`:

- A `Transport` over the capability. Register reads and the four safe writes go through
  `READ_REGISTER` and `WRITE_REGISTER`; `queue_set` becomes `SETUP_QUEUE` and refuses to proceed
  unless the addresses the crate chose are the ones the kernel programs; `notify` becomes `NOTIFY`.
  `queue_unset` has no capability method and does nothing, which is safe because a block role never
  drops its driver and the kernel resets the device for the next one.
- A `Hal` over the DMA region. `dma_alloc` serves the virtqueue from the region's ring area.
  `share` returns a buffer's physical address when it lies in the region, as the block server's
  data pages do, so a transfer stays zero-copy. Anything else, such as the request header and status
  byte the crate keeps on the stack, it bounces through a slot in page 0. `Hal`'s methods are
  static, so the region's two base addresses live in a process global.

Two device features have to be hidden from the crate, and the adapter does it by clearing them in
`read_device_features`, so the crate's own negotiation stays truthful:

- `VIRTIO_RING_F_INDIRECT_DESC` (bit 28). The kernel strips it from the driver's feature word, but
  the crate computes its negotiated set locally and would build indirect chains the validator
  refuses.
- `VIRTIO_RING_F_EVENT_IDX` (bit 29). The device reads `used_event` from the available ring, which
  is the kernel's shadow, and `validate_and_shadow` mirrors the heads and the index but not that
  field. It stays zero, so after the first completion the device would suppress every interrupt
  until the index wraps, and the server waits on that interrupt.

The crate does the rest of what our driver does, with three differences the server absorbs without
a patch:

- `begin_init` writes `FEATURES_OK` and never reads it back (784's probe found this). The server
  reads the status after `VirtIOBlk::new` and refuses to serve on a failed negotiation.
- `flush()` answers `Ok(())` when the device never offered `VIRTIO_BLK_F_FLUSH`. That is the yes
  that does nothing which `filesystem_contract::blk::FLUSH` forbids, so the server reads the
  negotiated bit from the driver word the adapter recorded and answers EOPNOTSUPP itself.
- `flush()` spins on the used ring; there is no `flush_nb` in 0.13.0. Reads and writes use
  `read_blocks_nb` and `write_blocks_nb` and keep the interrupt wait ("A completion is the used ring
  advancing"). A flush busy-waits, which QEMU completes inside the `NOTIFY` and real asynchronous
  hardware would not.

## The open question: the crate's queue does not fit the confined layout

This is where the lane stopped. The kernel's queue layout is a fixed contract for eight entries:
descriptor table at 0x000, available ring at 0x080, used ring at 0x100, one queue per 0x200 block
(`crates/direct_memory_access_validator`, `LAYOUT_QSIZE` and the offsets beside it, which Kani
proves disjoint). `setup_queue` in `kernel/src/virtio.rs` refuses any `num` above 8.

`VirtIOBlk` fixes its queue at 16 entries (`QUEUE_SIZE` in `vendor/virtio-drivers/src/device/blk.rs`,
private, no parameter), and `VirtQueue::new` refuses a transport whose maximum is smaller. No
adapter can bridge that: report 8 and the crate refuses; report 16 and `SETUP_QUEUE` refuses. The
siblings meet the same wall. `VirtIOInput` uses 32 entries, and `VirtIOGpu` uses 2, which fits the
count but not the offsets: the crate puts the available ring straight after the descriptor table,
at 0x020, where the kernel expects 0x080. Only `VirtIONet`, whose size the caller picks, fits
today.

The fix is a change to what the kernel and every virtio driver agree on, a kernel-driver layout
behind a syscall method. That is an architect's call, and the lane did not guess it.

**Needs calef.** How does the confinement accept a queue the crate lays out?

- **(A), recommended.** Size the layout by the queue, for a power-of-two `n` up to 32. The
  descriptor table sits at 0, the available ring at `16n` and the used ring at `32n`, with one
  queue per `64n` block. Every queue on a device uses the same `n`. The validator already takes
  `qsize`; its layout constants become functions of `n`, and the Kani harness for disjoint blocks
  runs over every `n` (at 32, two queues fill the one shadow frame exactly). At `n = 8` the formula
  gives today's 0x000, 0x080, 0x100 and 0x200, so every first-party driver keeps working unchanged,
  and blk (16), input (32) and GPU (2) all fit. `SETUP_QUEUE`'s arguments do not change, only the
  sizes it accepts. Its cost is an exception at the `Hal`. The crate asks for a whole page for the
  used ring, and the `Hal` hands it the `32n` offset inside the ring area instead. The adapter's
  `queue_set` check makes that loud if the crate's layout ever changes.
- **(B).** `SETUP_QUEUE` takes the driver's ring offsets, packed into its unused third argument
  (descriptor table and used ring; the available ring follows the table, as the crate and the
  specification's packing put it). The kernel checks that each lies in the region, is aligned and
  does not overlap the other, and sizes its shadow by `n`. The `Hal` then hands out real pages. It
  is more general than anything in the tree needs, it adds a packed word to the syscall surface,
  and the kernel keeps a second, fixed-layout mode for the drivers that pass 0.
- **(C).** Carry a second divergence in `vendor/virtio-drivers` that sets `VirtIOBlk`'s queue to 8.
  It is the smallest diff, but upstream has no reason to take it, it re-applies on every pin bump,
  and input and GPU would each need their own. The brief for this lane rules it out beyond (c0).
- **(D).** Refuse (c3): the block device stays on the first-party driver, and 784 records why.

Recommendation: (A). It has the fewest moving parts: no new argument, one formula that the existing
layout is a case of, and one proof extended rather than a second mode. Choosing it is not about
effort; (C) is less work and loses on the forever-divergence. If the answer is no to (A) and (B),
this milestone becomes (D), and so does most of 947 and 948. Whichever lane takes the ruling
builds the kernel half once, and the others merge it.

## The exit criterion's instrument

784 asks for a `script/bench` number before and after on all three architectures, and the rows of
milestone 38 (filesystem throughput) must not regress without saying so. The instrument does not reach the block
device on two of the three.

- The icount suite, the only `script/bench` leg on riscv64 and x86_64, has no row that touches a
  block device. The riscv64 bench boot runs no virtio at all (`bench_riscv` in `xtask/src/bench.rs`).
- The rows that do, `fs_read` and the `fs_seq_read` family, run only under `--real --smp`, which is
  Hypervisor.framework on an Apple Silicon host, so aarch64 only.

The "before" numbers taken for this milestone, at base `b933583ba` on 2026-10-24 (UTC), are the
icount suite on all three with `--check` against the committed baselines:

| ticks per iteration | aarch64 | riscv64 | x86_64 |
|---|---|---|---|
| `ipc_rtt` | 1043 | 172 | 17018 |
| `relay_rtt` | 2083 | 343 | 33977 |
| `call_reply` | 1094 | 182 | 17842 |
| `broker_rtt` | 2184 | 363 | 35640 |
| `spawn_reap` | 3520 | 561 | 46150 |
| `--check` | passed, every row within 10% | passed | passed |

QEMU 11.1.2 on all three, matching the baselines' emulator. Ticks compare within a column, not
across: each architecture's timer runs at its own rate.

They say only that the base is clean. The after run must repeat them, and add `script/bench --real
--smp` on an Apple Silicon host for the filesystem rows before and after. A filesystem leg on
riscv64 and x86_64 would need a non-icount `--smp` boot with the RedoxFS disk attached, which no
milestone builds yet. Recommendation: accept aarch64 for the filesystem rows and all three for the
icount suite, and say so in 784's (c3) line when this lands.

## Index row

The block device is the first of the four virtio devices 784 moves onto the vendored
`virtio-drivers`, which replaces hand-built rings driven by bytes a hostile device writes. It waits
on a ruling about how the kernel's confined queue layout accepts the crate's queue sizes, a
question 947 and 948 share.
