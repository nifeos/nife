---
status: PARTIAL
raised: 2026-10-24
milestone_dependencies: 784
decision_dependencies: 46
machine_requirements: none
specific_machine: none
needs_person: no
---
# 947. The virtio input device runs on `virtio-drivers`

*Name: provisional. The number 947 was assigned by the integrator on 2026-10-24 (UTC); the title
and slug are provisional. Lane: `milestone/947-virtio-input-on-virtio-drivers`.*

Item (c1) of [784 (reuse what the tree wrote)](0784-reuse-what-the-tree-wrote.md), written as its
own milestone because 784 moves one device per milestone. Its siblings are 948 (GPU), 949 (block)
and 950 (net).

## Index row

`keyboard_driver` drives the virtio keyboard through `vendor/virtio-drivers` (0.13.0 plus upstream
#232) instead of a hand-written queue, over the kernel's confined transport and behind each
architecture's IOMMU, with its cost measured before and after on all three.

## What it moves

`components/src/keyboard_driver.rs` negotiated features, laid out its event queue, posted buffers
and walked the used ring by hand. After this milestone the crate does all four: `Transport::
begin_init` and `finish_init` for the status dance, `VirtQueue` for the descriptor table, the free
list and both rings. The driver keeps what is nife's: the keymap, the two delivery modes, the
`0xDEAD_...` bring-up codes, and the check on a device-written buffer index.

Reuse: `virtio-drivers` 0.13.0 plus #232, as `vendor/virtio-drivers`, ruled by calef on #2586
(2026-10-22 UTC). The crate's `VirtIOInput` is not used, for the two reasons below.

## Joining the crate to the confined transport

The appendix [virtio-drivers-probe.md](0784-reuse-what-the-tree-wrote/virtio-drivers-probe.md)
left two questions for (c1): how the crate's `VirtQueue`, which allocates its own rings, meets a
kernel that places the rings itself, and where the static `Hal` keeps its DMA window.

The answer is two types in `user_mode_runtime::confined_virtio` (provisional), behind the
`virtio-drivers` feature, so 948 to 950 can use the same pair:

- `ConfinedTransport` implements the crate's `Transport` over a `Virtio` capability. Register reads
  and writes are `READ_REGISTER` and `WRITE_REGISTER`; `queue_set` is `SETUP_QUEUE`, after checking
  that the addresses the crate chose are the ones the kernel will program; `notify` is `NOTIFY`.
  A refusal is kept and read back by the driver, because the trait's methods return nothing.
- `WindowHal` implements `Hal` over the one DMA window the spawner mapped. `dma_alloc` hands out the
  kernel's fixed ring blocks in queue order: the descriptor table and available ring at
  `queue_block(q)`, the used ring at `queue_block(q) + USED_OFF`, the offsets
  `direct_memory_access_validator` owns. `share` translates a buffer inside the window to its
  physical address and copies nothing; a buffer outside it is a panic, which the kernel's validator
  would have refused anyway. The window lives in three atomics set once at start, the global the
  static methods force.

`VirtIOInput` is not used because it fixes its queues at 32 entries, and the confinement drives at
most `LAYOUT_QSIZE`, eight. It also boxes its event buffers, and `keyboard_driver` has no heap. The
driver uses `VirtQueue<WindowHal, 8>` directly; what `VirtIOInput` adds over it is about twenty
lines, and the driver has its own.

## The measurement

784's exit criterion is a `script/bench` number before and after on all three architectures. No
existing row touches a virtio device, so this milestone adds two, reported only by
`script/bench --keyboard` and never gated:

- `keyboard_up`: ticks from spawning the driver to its `KEYBOARD_UP` report. Feature negotiation,
  queue setup and the first eight buffers, with no host involvement.
- `keyboard_event`: ticks from the keyboard's interrupt reaching the kernel to the driver's
  `COMMIT` arriving at the kernel standing in for the compositor, summed over 16 key presses the
  host sends over the QEMU monitor, one at a time.

Results are recorded in the status section below.

## Done when

- `keyboard_driver` builds on `vendor/virtio-drivers` and nothing in it walks a ring by hand.
- The keyboard system test passes on aarch64, riscv64 and x86_64, behind each IOMMU.
- `script/bench --keyboard` numbers before and after, on all three, are recorded here.

## Status

PARTIAL: in progress on the lane.

## BUGS

- A used-ring entry naming a buffer the driver never posted ends the driver, where the hand-written
  queue dropped the entry and went on. The crate cannot skip a used entry it will not pop, so a
  lying device stops the keyboard instead of being ignored. A device can stop a keyboard anyway by
  not answering, so nothing is lost that confinement held.
