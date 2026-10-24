---
status: BUILT
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
existing row touches a virtio device, so this milestone adds two, printed only by
`script/bench --keyboard` and never gated
([the keyboard rows](../../notes/benchmarks/kernel-bench-harness.md)):

- `keyboard_up`: from the driver's spawn being queued, after the kernel has wired the device, to its
  `KEYBOARD_UP` report. Loading the image, feature negotiation, queue setup and the first eight
  buffers, with no host involvement.
- `keyboard_event`: from the keyboard's interrupt reaching `irq_notify` to the driver's `COMMIT`
  arriving at the kernel standing in for the compositor, summed over 16 key presses the host makes
  one cue at a time.

Measured 2026-10-24 (UTC) on Linux x86_64 under the pinned QEMU 11.1.2 and nightly-2026-10-24, TCG
with `-icount shift=0,sleep=off`, one hart, the debug userspace every image ships. "Before" is the
hand-written driver at this branch's bench commit, with the same kernel and wiring as "after".
Ticks are each architecture's own counter (62.5 MHz on aarch64, 10 MHz on riscv64, the calibrated
TSC on x86_64), so compare a row across its own line only.

| | before | after | change |
|---|---|---|---|
| aarch64 `keyboard_up` | 45,998 | 57,616 | +25.3% |
| aarch64 `keyboard_event`, per press | 2,107 | 2,345 | +11.3% |
| riscv64 `keyboard_up` | 2,803 | 4,374 | +56.0% |
| riscv64 `keyboard_event`, per press | 341 | 380 | +11.4% |
| x86_64 `keyboard_up` | 544,406 | 656,956 | +20.7% |
| x86_64 `keyboard_event`, per press | 32,978 | 36,751 | +11.4% |

What the numbers mean:

- A key press costs about 11% more on every architecture, about 240 aarch64 ticks or 3,800
  instructions. The path is the same in syscalls: one `NOTIFY` per re-posted buffer and one
  interrupt acknowledgment per drain, as before. The difference is the crate's own work in an
  unoptimized build: the free list, the `Result` plumbing, the bounds checks and its atomics on
  the rings. An optimized build would narrow it, and no image ships one today.
- Bring-up costs more. The image is bigger (the crate, `bitflags`' formatting and `log`'s call
  sites come along) and loading it is inside the row; how much of the difference that is was not
  separated. Bring-up happens once per boot.
- Both rows were identical across two aarch64 runs of the hand-written driver. riscv64 moved by
  one tick between runs. x86_64's `keyboard_event` moved by 0.4% between two runs of the same
  driver, so read its row to about half a percent.
- Code lines (blank and comment lines removed): `keyboard_driver.rs` went from 252 to 207, and
  `user_mode_runtime::confined_virtio`, which 948 to 950 can share, is 257. The ring walking the
  driver no longer holds is the crate's, under its upstream tests and users.

## Done when

- `keyboard_driver` builds on `vendor/virtio-drivers` and nothing in it walks a ring by hand.
- The keyboard system test passes on aarch64, riscv64 and x86_64, behind each IOMMU.
- `script/bench --keyboard` numbers before and after, on all three, are recorded here.

## Status

BUILT, 2026-10-24 (UTC). Each item under "Done when" holds: the driver drives the queue only through
`VirtQueue`; `a_keystroke_from_a_virtio_keyboard_becomes_a_terminal_byte` passes on aarch64,
riscv64 and x86_64 (PVH and OVMF), behind SMMUv3, the RISC-V IOMMU and VT-d; the table above is
the bench.

Building it surfaced a latent `x86_64` entry bug: the kernel enters `_start` with the stack
16-byte aligned, and the psABI expects eight bytes off. `user_mode_runtime::program_entry!` is the
one-`call` shim std's entry already has, and `keyboard_driver` uses it. The other `no_std` programs
still enter misaligned, recorded in that macro's `BUGS` and in notes/user-mode-runtime.md,
"Entering a process".

## BUGS

- A used-ring entry naming a buffer the driver never posted ends the driver, where the hand-written
  queue dropped the entry and went on. The crate cannot skip a used entry it will not pop, so a
  lying device stops the keyboard instead of being ignored. A device can stop a keyboard anyway by
  not answering, so nothing is lost that confinement held.
