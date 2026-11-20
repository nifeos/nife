---
status: NOT-STARTED
promoted_from: block-driver-ships-without-its-attacker-roles
raised: 2026-10-05
milestone_dependencies: 261
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 790. The shipped block driver carries no attacker roles

Raised by the lane for milestone 261 (the NVMe driver leaves the kernel) (`lane/nvme-dma-escape`,
#1647) on 2026-10-05 (UTC), while gating that milestone's NVMe DMA-escape attacker behind
`components/confinement_attackers`. Title and slug are drafts.

## The defect

`components/src/block_driver.rs` dispatches on the role word its spawner passes in `arg0`, and two
of those roles are the DMA-confinement tests' attackers: `VIRTUAL_IO_ATTACK` (8, a descriptor aimed at
kernel memory) and `VIRTUAL_IO_ATTACK_INDIRECT` (13, the indirect-descriptor escape). Their bodies are
`virtio::run_attack` and `virtio::run_attack_indirect` in `crates/virtio/src/lib.rs`, and nothing
gates them. So every shipped `block_driver` binary contains both attacks, and any spawner that
passes 8 or 13 selects one. The module doc says the attackers share the binary on purpose: they
differ from the honest driver by one descriptor, and sharing the setup code is what makes the
attack a fair test. That argument is about sharing source, though. It does not need the shipped
image to carry the attack.

The NVMe server had the same shape, with a sentinel in its transfer buffer instead of a role word.
#1647 removed it from the shipped build.

## What it would take

Do what #1647 did for NVMe:

- Gate the two match arms in `block_driver`'s `_start` on `confinement_attackers`, and gate
  `run_attack` and `run_attack_indirect` behind a `virtio` crate feature that the components feature
  turns on. A shipped driver handed role 8 or 13 then reaches the `_ => panic!()` arm.
- Gate the kernel spawners, `virtual_io_service::start_attacker` and `start_attacker_indirect` (and
  their role constants), on `system_tests`, if a check finds they are not already.
- `xtask`'s `CONFINEMENT_ATTACKERS` switch and `script/lint`'s feature clippy already cover
  `components`, so the build side needs nothing new.
- Each attacker test should fail by name when it gets an honest driver (a boot outside
  `cargo xtask test`), the way the NVMe escape test now does, and not as a confinement failure.

Both tests' replayable falsifications have to keep applying and going red.

Reuse: the mechanism milestone 261 (the NVMe driver leaves the kernel) built in #1647, taken
whole: the `components/confinement_attackers` feature, `xtask`'s `CONFINEMENT_ATTACKERS` switch and
`build_programs`, and `script/lint`'s feature clippy. Nothing outside the tree is involved.

## BUGS

- `VIRTUAL_IO_BLK_WRITE_ABANDON` (31, panics mid-operation) looks test-only as well. Whether it belongs
  behind the same feature has not been checked.

## Index row

`block_driver` dispatches on a role word, and two roles are the DMA-confinement attackers, so every shipped binary contains both attacks. Gating them out of the shipped binary keeps the test fair and removes an attack a spawner could select.
