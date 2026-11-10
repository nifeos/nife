---
status: NOT-STARTED
raised: 2026-11-10
milestone_dependencies: none
decision_dependencies: 4
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1063. The boot self-test's spawning checks run from userspace

*(The number, title and slug are provisional.)*

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
2. Every check still runs on every architecture and the verdict line is unchanged.
3. A falsification that breaks one moved check fails the boot's verdict, as it does today.

## Index row

The boot self-test spawns up to six user programs from the kernel on every boot. Spawning is the
progenitor's job, so the checks that need a user program move to a userspace self-test program the
progenitor runs, and the kernel keeps the checks of its own internals.
