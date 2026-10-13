---
status: NOT-STARTED
raised: 2026-09-26
promoted_from: bound-the-host-pass
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 663. Bound the host pass, so a test that spins fails instead of running for days

*Title and slug are drafts.*
Raised by #1323 (a doctest that spun forever under a mutant). The title is provisional.

The change is inside `xtask`, touches no syscall surface and needs no dependency.

## What happened

Two orphaned doctest runners from `jh7110_entropy` ran for five days at 99% CPU each until they
were killed on 2026-09-26. A session working on milestone 326 (turn a mutation score upward) had
hand-applied a mutant that makes `Pool::take` spin, run `cargo test -p jh7110_entropy --doc` to
confirm the hang, and then killed `cargo`. Pull request #1323 reproduced what followed. A SIGKILL to
`cargo` does not reach rustdoc's `rust_out` runner or its child, so both are reparented and keep
going. A SIGTERM, the signal `helpers/qemu-bounded.sh` sends, did clean them up.

The same pull request fixed that doctest and its one sibling in `memory_corruption_canary_gate`, and swept the other
153 runnable doctests for loops that could spin. That closes the known cases. It does not bound the
next one.

## What bounds a host test today

Nothing inside the gate. CI's `test` job runs `cargo xtask test` with `timeout-minutes: 30` on the
job, so a hanging host test costs a run half an hour and then dies with the runner, reported as a
cancellation rather than as a failing test. Locally, `cargo xtask test` and `script/verify` have no
bound at all, which is how a leaked pair lasted five days on patagonia.

The kernel legs are different. Their own guest watchdogs end a hung suite, and `hvf_kernel_leg` in
`xtask/src/suite.rs` says so as the reason it needs no host-side deadline. The host pass has no
equivalent: a Rust test that loops is just a process that never exits.

## The shape

In `xtask/src/suite.rs`, the three host `cargo test` invocations (the workspace pass, and the two
own-workspace passes for `tools/redoxfs_host` and `redoxfs_server`) are spawned in a process group of
their own and waited on with a wall-clock deadline. On expiry xtask sends SIGTERM to the whole
group, waits a short grace, then SIGKILLs the group, and fails the pass with a message naming the
deadline and the command. A spinning test then fails the gate loudly, in CI and locally, and leaves
nothing behind.

The tree already owns children this way. `xtask/src/boot_check.rs` and
`xtask/src/disk_throughput.rs` each hold a child past a deadline and kill its descendants before the
child itself, because killing only the wrapper orphans the emulator. This is the same lesson for
`cargo test`.

## What has to be measured first

- The deadline's value. It should come from the host pass's measured wall-clock on a cold and a warm
  build, with generous headroom, and not from a guess. A deadline that fires on a slow CI runner is
  worse than none.
- Whether rustdoc's doctest children stay in the group cargo was started in. Pull request #1323
  showed SIGTERM reaching them through `qemu-bounded.sh`. That a group signal reaches them is the
  likely reading, and it has not been run.

## What was considered and lost

- A per-test timeout inside libtest. Stable libtest can report a slow test but not kill one, and
  doctests run through rustdoc anyway. Recalled rather than re-read; the lane should check.
- cargo-nextest, which has a terminating slow-test timeout. It is a dependency, which §46 (thin
  primitives or whole subsystems) makes a decision. It changes how every host test is run, and from
  memory it does not run doctests, which are the case that failed.
- Lowering CI's job timeout. It bounds CI only, still reports a cancellation, and does nothing for a
  local run.

## Index row

Two doctest runners ran for five days at 99% CPU after a mutant made a test spin. Proposed: bound the host pass in `xtask` so a test that spins fails instead of running for days.
