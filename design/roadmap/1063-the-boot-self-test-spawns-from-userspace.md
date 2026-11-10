---
status: BUILT
raised: 2026-11-10
built: 2026-11-10
milestone_dependencies: none
decision_dependencies: 4
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1063. The boot self-test's spawning checks run from userspace

*(The number, title and slug are provisional.)*

Reuse: the checks and the verdict contract exist (`kernel/src/self_test.rs`, `crates/boot_ladder`);
this moves them. Nothing outside the tree tests nife's own boot.

Ruled 2026-11-10 (UTC): the boot self-test checks that spawn a user program move to a userspace
program the progenitor runs. The checks of the kernel's own internals stay in
`kernel/src/self_test.rs`, because they test what must run at EL1 (§4 (kernel shape)).

## Today

`kernel/src/self_test.rs` runs on every default boot and spawns, from the kernel,
`kernel_test_subject`, `memory_region_depleter`, `block_driver` (through
`user::virtio_service`) and, on a JH7110, `jh7110_entropy`, plus `user::supervised_child`'s fault
stub. Its own `BUGS` count up to six spawns per boot. The kernel files those checks need
(`user/virtio_service.rs`, `user/x86_programs.rs`, `user/supervised_child.rs`,
`user/memory_region_service.rs`) are about 900 code lines.

## The shape

1. A userspace self-test program, started by the progenitor after the kernel's own checks, runs the
   spawning checks with the budgets and capabilities the progenitor already holds.
2. The verdict keeps its contract: `crates/boot_ladder`'s strings, read by `crates/serial_console`.
   The kernel's half and the program's half each print their part, and the program prints the one
   verdict line.
3. The kernel spawns only the progenitor, which is milestone 811 (the boot services leave the
   kernel)'s fork 1 rule applied to the self-test.

## Done when

1. `kernel/src/self_test.rs` calls no `user::program` and no service starter.
2. Every check still runs on every architecture, and the verdict line reads as it does today.
3. A falsification that breaks one moved check fails the boot's verdict, as it does today.

## As built

`kernel/src/self_test.rs` runs `exceptions`, `mapping`, `frames`, `timer`, `scheduler`,
`preemption` and `device_interrupt`, and writes its tally into a page the hand-over grants the
progenitor. The progenitor runs `components/src/boot_self_test.rs` before the shell; it runs the
other eight and prints the verdict. `boot_ladder::verdict` is the tally and line format both
halves share, `boot_ladder::kernel_half` the page, and `boot_ladder::self_test_program` the
program's slots, addresses, image table and messages. notes/self-test-checks.md, "The two
halves", has the design.

| run | verdict |
|---|---|
| aarch64, `boot-check` | 10 of 15 passed, 5 absent: blockdev device_interrupt clock trng entropy |
| `x86_64`, `boot-check` | 12 of 15 passed, 3 absent: blockdev clock trng |
| aarch64, a virtio-mmio disk and virtio-rng | 12 of 15 passed, 3 absent: device_interrupt clock trng |
| `x86_64`, a virtio-pci disk | 13 of 15 passed, 2 absent: clock trng |
| aarch64, `boot-check --inject` | red: exceptions timer preemption userspace |
| `x86_64`, `boot-check --inject` | red: exceptions timer userspace |

riscv64 is gated by the merge queue's `boot-check`. Falsification: a progenitor that placed disk 0's
transport without `GRANT` failed `blockdev` by name and turned the disk boots' verdicts red.

## Exit criteria, as built

| criterion | state |
|---|---|
| 1 | Done: the module calls neither. |
| 2 | Done: every check runs on every architecture; the line's strings and format are `boot_ladder`'s. |
| 3 | Done: the injected `userspace` and the `blockdev` above. |

## BUGS

- The shape's third point is not met yet. On a JH7110 the hand-over still starts the TRNG's
  service, and on a machine with a virtio disk it registers disk 0 for the program. Moving those
  starters is milestone 811 (the boot services leave the kernel)'s work.
- The verdict reaches the console after the tour line, so a watcher reading the ladder's order
  needs `serial_console`'s `has_reached` rather than the ratchet alone.

## Follow-on

- **Milestone 811.** The JH7110 TRNG and disk 0 starters left in the hand-over
  (`kernel/src/user/self_test_hand_over.rs`) move out with the other boot services; its table's
  rows for `userspace`, `budget`, `blockdev` and `entropy` now name `boot_self_test`'s checks.
- **Recorded.** `supervised`'s pc check and the draws with no deadline are BUGS in
  `components/src/boot_self_test.rs`.
- **Recorded.** A boot whose kernel has no self-test prints no verdict, in
  `crates/system_initializer/src/self_test.rs`'s BUGS.

## Index row

The boot self-test spawns up to six user programs from the kernel on every boot. Spawning is the
progenitor's job, so the checks that need a user program move to a userspace self-test program the
progenitor runs, and the kernel keeps the checks of its own internals.
