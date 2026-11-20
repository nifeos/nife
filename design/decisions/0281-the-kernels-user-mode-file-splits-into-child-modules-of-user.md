---
status: DECIDED
raised: 2026-10-11
decided: 2026-10-11
ratified_by: calef
---

# 281. The kernel's user-mode file splits into child modules of `user`

Raised 2026-10-11 (UTC) as the cut that
[milestone 872 (the kernel's user-mode file is split along its seams)](../roadmap/0872-the-kernels-user-mode-file-is-split-along-its-seams.md)
says is calef's call before a line moves. Written by lane
`milestone/872-the-kernels-user-mode-file-is-split-along-its-seams`. *(Section number provisional
until the merge queue lands it.)*

It blocked the whole milestone until calef ruled both forks on 2026-10-11 (UTC), recorded on
[PR #1959](https://github.com/nifeos/nife/pull/1959). Nothing else waited on it.

## What the file is today

`kernel/src/user.rs` is 3,519 lines on main at `ae6b91e79`. Milestone 812 (`std::thread::spawn`
runs real threads in one address space) has merged, so the address-space registry already lives in
`kernel/src/user/address_space_registry.rs` (487 lines), behind `pub use address_space_registry::*`.
Measured from the file's own items, each range starting at its first doc comment:

| lines | count | what |
|---|---|---|
| 1 to 63 | 63 | module docs, imports, the user stack constants |
| 64 to 518 | 455 | `AddressSpace`, `Backing`, the overhead constants, `impl AddressSpace`, `ASIDS`, the registry's `mod` line |
| 519 to 570 | 52 | `impl Drop for AddressSpace` |
| 571 to 870 | 300 | `LoadError`, `load`, the timebase page, `map_segments`, `initrd`, `program` |
| 871 to 905 | 35 | `Mapping` and `Spawn` |
| 906 to 1112 | 207 | `INITRD_VA`, the per-architecture console constants, `PROGENITOR_ENTRY` through `PROGENITOR_ROLE` |
| 1113 to 1550 | 438 | `spawn_hello`, `DeviceRun`, the `run` family, `enter_frame`, the `outlaw` roles |
| 1551 to 1569 | 19 | the `x86_programs` declaration |
| 1570 to 1937 | 368 | the `x86_64` userspace demo and the two riscv64 demos |
| 1938 to 2630 | 693 | `boot_progenitor`, one function with a 56-line doc |
| 2631 to 2663 | 33 | `boot_usb_keyboard` |
| 2664 to 2872 | 209 | `mod` lines with their docs, and `term_print` |
| 2873 to 2935 | 63 | `boot_clock_page`, `VirtioBootGrant` |
| 2936 to 2963 | 28 | `VIRTIO_DMA_PHYS_OFFSET`, `write_dma_phys` |
| 2964 to 3340 | 377 | the virtio, entropy, e1000e and DesignWare starters, `boot_config_page`, `boot_screen_terminal` |
| 3341 to 3519 | 179 | `mod` lines with their docs, and `wait_for` |

Every starter (the eleven `boot_*` functions below `boot_progenitor`) has exactly one caller,
`boot_progenitor`. `write_dma_phys` is not a starter: `display_service` and `keyboard_service` call
it as `super::write_dma_phys`. The file has no tests and no Kani harness.

## The ruling on fork 1

calef, 2026-10-11 (UTC), on whether `AddressSpace` stays in the parent: *"Agreed, make the parent
an index."* He ruled it as a precedent for every split, not by the example of §271 (the scheduler
file splits into submodules of `sched`):

- When a file is split, the parent becomes an index: module docs, `mod` lines, `use` and
  re-exports, and nothing that can grow into a feature.
- Every type and function, the central one included, lives in a child named for it.
- A few small constants or helpers that every child uses may stay, if alone they would be odd in a
  file. The test is that nothing left can grow into a feature.

His reasons:

- The parent as default home is how these files grew. An index gives new code nowhere to land but
  a named file.
- A concept is found by its file name.
- `pub(super)` honestly marks what siblings reach into, and it is where accessors would later
  tighten it.
- The index rarely changes, so the hottest file draws fewer merge conflicts.

Prior art is mixed: std's `alloc::vec` keeps `Vec` in its parent, and `std::collections` is an
index. So this is a choice, and §271's `IpcTables` predates it. The precedent is recorded on its
own in §282 (when a file is split, the parent becomes an index), with the lint that holds it.

## The cut

Child modules of `user`, private, each taking `use super::*`. `user.rs` re-exports each whole
(`pub use <child>::*`), as 812 did for the registry. Every `user::` path a caller uses today still
resolves, so no Rust file outside `kernel/src/user.rs` and `kernel/src/user/` changes. Every module
name is provisional and left to calef's naming pass.

| file | holds | lines, about |
|---|---|---|
| `user.rs`, the index | module docs, imports, `USER_STACK_VA` and `USER_STACK_TOP`, every `mod` line with its doc | 455 |
| `user/address_space.rs` | `AddressSpace`, `Backing`, the overhead constants, `ASIDS`, `Drop` | 510 |
| `user/loader.rs` | `LoadError` through `program`, and `INITRD_VA` | 315 |
| `user/entry.rs` | `Mapping`, `Spawn`, `DELEGATED_TEST_INTERRUPT`, `spawn_elf_child_builder`, `DeviceRun`, the `run` family, `enter_frame`, the `outlaw` roles | 540 |
| `user/console_device.rs` | the console UART's interrupt, page and port-range constants, `uart_irq_and_source` | 110 |
| `user/boot_demos.rs` | the `x86_64` and riscv64 demos | 375 |
| `user/progenitor_boot.rs` | `PROGENITOR_ENTRY`, `PROGENITOR_ROLE`, `INIT_STACK_PAGES`, `boot_progenitor` | 735 |
| `user/device_starters.rs` | the eleven starters and `VirtioBootGrant` | 485 |
| `user/virtio_dma.rs` | `VIRTIO_DMA_PHYS_OFFSET`, `write_dma_phys` | 40 |
| `user/terminal_print.rs` | `term_print` | 45 |
| `user/test_support.rs` | `wait_for` | 35 |

No file is over 1,500.

What stays in the index, and why: `USER_STACK_VA`, `USER_STACK_TOP` and the assertion tying them,
three lines. Four of the five code children use them, as do 30 files outside `user`, and they cannot
grow into a feature. Every constant and helper the parent held besides those went to the child
whose topic it is, by its own doc: `INIT_STACK_PAGES` is "the progenitor's stack", `INITRD_VA` is
where `initrd()`'s archive is mapped.

## The ruling on fork 2: where the starters go

calef, 2026-10-11 (UTC): *"B"*. The starters get a module of their own, `device_starters`, and
`progenitor_boot` holds `boot_progenitor` alone. The case for B and against A, as put to him:


Every starter (the eleven `boot_*` functions below `boot_progenitor`) has exactly one caller,
`boot_progenitor`. The block recommended a module of their own, because milestone 811 (the boot
services leave the kernel) would then delete that one file. 811 has since been planned in order
(its "The work, in order"): one starter leaves per step, across seven steps, so no step deletes a
file whole. Each step also edits `boot_progenitor`'s grants for the slot it frees.

| | where boot-time endowment lives |
|---|---|
| seL4, `src/kernel/boot.c`, read 2026-10-11 | `create_initial_thread` beside the helpers that build what it is given: `create_root_cnode`, `provide_cap`, `create_bi_frame_cap`, `create_it_asid_pool`. |
| Linux, `init/main.c`, read 2026-10-11 | `do_basic_setup` runs `do_initcalls`; each driver's init function lives in that driver's own file. |
| this tree | `fs_service::root_directory` and `install_service::confirm` already sit in their services' files. |

- B, ruled. A module of their own, `device_starters`, about 485 lines. Recommended since the
  ruling on fork 1. A starter is found by a file named for what it is. Eleven functions and
  `VirtioBootGrant`'s three fields gain `pub(super)`, which marks exactly what `boot_progenitor`
  takes from them, the honesty the fork 1 ruling asks for. The module reads as 811's remaining
  inventory and empties as 811 lands.
- A. With `boot_progenitor`, one module of about 1,210 lines. This was the recommendation before
  fork 1 was ruled, on two grounds: one caller, so each 811 step edits one file rather than two,
  and nothing gains `pub(super)`. The ruling made the second a cost rather than a saving, and the
  first alone is a convenience. It is seL4's shape.
- C. Each beside its service, the Linux shape and this tree's two existing cases. Refused: two of
  them (`boot_clock_page`, `boot_config_page`) have no service, three share `VirtioBootGrant` and
  `boot_virtio_mmio_device`, and most of their bodies are `boot_progenitor`'s console narration
  ("network : NONE ...") rather than service wiring.

### Not asked: the per-architecture demos stay out of `kernel/src/arch/`

The block asked whether the `cfg(target_arch)` demos and console constants move under
`kernel/src/arch/`. It is not put to calef. Neither range holds `asm!`, a system register or
assembly; they call `sched` and the loader, so under `arch/` they would point the layering upward.
811's fork 1 (ruled A, 2026-10-07 UTC) already moves `x86_userspace_demo` behind a non-default
`tour` feature, so its future is decided there. This cut moves nothing out of `arch/` and adds no
architecture-specific code, so AGENTS.md rule 1 holds under it unchanged.

## What the move costs beyond moving code

- Visibility. A private item a sibling reaches becomes `pub(super)`. That is visible to `user`
  and its children, the same set that saw it before the split. It covers `AddressSpace`'s fields,
  `Backing`, `WINDOW_IN_OVERHEAD` (the loader reads it), `term_print`, the starters and
  `VirtioBootGrant`'s fields.
- The ratchets. `design/file-length-baseline.tsv` drops the `user.rs` row and gains none. Of the two
  comment-block rows, `AddressSpace::map_physical`'s doc moves to `address_space.rs` and
  `boot_progenitor`'s doc to `progenitor_boot.rs`. Each follows its block under the split rule of §267 (a comment states the
  constraint as it is now). That rule was added on 2026-10-11 (UTC), after the block was written.
- Falsifications. One record patches `user.rs` today
  (`user.spawn_mapping_revocation_tests.a_page_the_kernel_wired_is_unmapped_when_its_frame_is_revoked`),
  in `impl AddressSpace`. It is regenerated at the hunk's new home and shown red at its own
  assertion.
- Citations. The `user.rs:NNNN` citations in notes are repointed to symbols under §277 (cite a
  heading or a symbol, never a line number); those in other milestones' blocks go to a maintainer
  follow-up.
- Nothing keys on the module path. `script/fastpath-footprint`'s cold list matches
  `12AddressSpace` under any path; `script/stack-frame-check` names `disk_service`, which does not
  move.
- Proof. `script/test` on aarch64, riscv64 and x86_64 (with its AMD-Vi and OVMF legs),
  `script/fastpath-footprint`, Kani, full `script/lint` and `script/coverage`.

## What it does not decide

Module names, which are calef's naming pass. No public path, wire format, system call or lock changes,
and no function changes behavior.
