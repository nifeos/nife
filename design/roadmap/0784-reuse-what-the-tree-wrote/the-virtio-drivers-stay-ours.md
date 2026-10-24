# The virtio drivers stay ours: items (c1) to (c4), refused

Appendix to [784. Reuse what the tree wrote](../0784-reuse-what-the-tree-wrote.md).

calef ruled on 2026-10-24 (UTC): option D, refused, for (c1) input, (c2) GPU, (c3) block and (c4)
net. nife's virtio drivers stay its own.

## The reason, as each component carries it

`keyboard_driver`, `gpu_driver`, the `virtio` crate and `virtio_net_transport` each carry this line:

> Reuse: write, under §46 (thin primitives or whole subsystems). Ruled by calef on 2026-10-24
> (UTC), option D, on milestone 784 (reuse what the tree wrote)'s items (c1) to (c4).
> `virtio-drivers` assumes a driver lays out its own virtqueues, and nife's confinement does not
> allow that: the kernel builds every ring itself, eight entries at fixed offsets, Kani-proved in
> `direct_memory_access_validator`, so no driver can aim device DMA.

The kernel programs each queue's three ring addresses (`SETUP_QUEUE`), and a driver never writes a
queue address register. The layout is `LAYOUT_QSIZE`, eight entries, at `AVAIL_OFF` and `USED_OFF`
inside the queue's ring block. The shadow-ring validator's harnesses are written against that
layout. A library that sizes and places its own rings has to be bent to fit it, and every way to
bend it gives something up.

## The options set aside

- (A) Make the kernel's ring layout generic over queue size, so the crate's sizes fit. It widens a
  proved layout, and its harnesses, for a library's convenience.
- (B) Let a driver propose a layout and have the kernel validate it. That hands a driver the choice
  the fixed layout exists to withhold.
- (C) Patch the vendored crate down to eight entries. calef: *"C is not an option."*

## What he was given beside the options

- Redox, whose filesystem nife runs (§34 (RedoxFS is the primary filesystem, on three
  conditions)), wrote its own virtio drivers, `virtio-core`, `virtio-blkd`, `virtio-netd` and
  `virtio-gpud`, rather than use `virtio-drivers`.
- The crate's dependents are mostly rCore teaching kernels, plus Android's Trusty.
- The input device was moved onto the crate as a trial and measured. It met the fixed layout by
  answering the crate's ring allocations with the kernel's offsets, in a shim crate of 257 code
  lines, and used `VirtQueue` at eight entries directly because the crate's `VirtIOInput` fixes 32.

The trial's numbers, from `script/bench --keyboard` on 2026-10-24 (UTC), Linux x86_64, pinned
QEMU 11.1.2, TCG with `-icount shift=0,sleep=off`, one hart, debug userspace. Ticks are each
architecture's own counter, so compare a row only along its own line. The row definitions are in
[the keyboard rows](../../../notes/benchmarks/kernel-bench-harness.md).

| | hand-written | on the crate | change |
|---|---|---|---|
| aarch64 `keyboard_up` | 45,998 | 57,624 | +25.3% |
| aarch64 `keyboard_event`, per press | 2,107 | 2,345 | +11.3% |
| riscv64 `keyboard_up` | 2,803 | 4,375 | +56.1% |
| riscv64 `keyboard_event`, per press | 341 | 380 | +11.4% |
| x86_64 `keyboard_up` | 544,406 | 657,046 | +20.7% |
| x86_64 `keyboard_event`, per press | 32,978 | 36,616 | +11.0% |

A key press cost 11% more on every architecture with the same syscalls, the difference being the
crate's free list, `Result` plumbing, bounds checks and ring atomics in an unoptimized build.
Bring-up cost 21% to 56% more, part of it a bigger image to load; the split was not measured. Both
columns ran the trial's own kernel and wiring; the shipping driver's figures are in the keyboard
rows.

## What the trial left on the hand-written driver

Three things the trial found serve the driver nife keeps, and are on it:

- Every `no_std` program enters through `user_mode_runtime::program_entry!`, which gives `x86_64`
  the entry stack its psABI promises (notes/user-mode-runtime.md, "Entering a process").
- `script/bench --keyboard` and its two rows, which never gate.
- A used-ring entry naming a buffer the driver never posted stops `keyboard_driver` with
  `E_COMPLETION` rather than being dropped.

## Milestone numbers that must not be reused

The integrator assigned 947 (input), 948 (GPU), 949 (block) and 950 (net) to (c1) to (c4) on
2026-10-24 (UTC). Their pull requests, #2687, #2686, #2688 and #2690, closed with this ruling, and
no block was ever merged under those numbers. `script/claim` refuses only a number an open pull
request holds, so this page is what keeps them from being handed out again with a different
meaning. A proposal one of those branches wrote, that the confined transport take the crate's queue
sizes, is option A and never landed.
