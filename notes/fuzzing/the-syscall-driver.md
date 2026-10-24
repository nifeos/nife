# Fuzzing: the seeded syscall driver

An appendix to [`notes/fuzzing.md`](../fuzzing.md) for
`system_tests/src/user/syscall_fuzzer_tests.rs`, the driver milestone 752 (a seeded syscall driver
with a shadow model) built. Its roadmap block,
[`design/roadmap/0752-a-seeded-syscall-driver-with-a-shadow-model.md`](../../design/roadmap/0752-a-seeded-syscall-driver-with-a-shadow-model.md),
holds what it reaches, its measured cost, the replay table of its six falsifications and the weekly
sweep. This file holds the oracle's design and the long account of each entry in the module's
`BUGS`.

## The oracle

The oracle is a model, not a crash. A confinement defect delivers something wrong and crashes
nothing, so a fuzzer that watched only for panics would pass it. A kernel panic still ends the run,
as it ends any test.

After every operation, the actor that ran it reads back its whole capability table, and the
conductor compares it slot by slot with the model. A capability that lands where the model says
nothing should is a failure even when no register mentions it.

The driver is coverage-blind, as an effort trade.

## Determinism

Nothing in the driver reads a clock or a core number to decide what to do. A seed therefore replays
the same operations in the same order on every run and every architecture. A failure prints the
seed, the step, the call and a one-line replay.

## Kernel threads, not an EL0 program

The actors are kernel threads with ordinary capability tables, entering through `syscall::invoke`
as `reap_tests` and `notification_tests` do. The oracle needs to know which side of a rendezvous
parked first. Only the kernel can say that without guessing at timing, and the driver asks it
through `sched::thread_death_disposition`. The trap entry's register marshaling is not exercised
here; every EL0 test in the suite crosses it.

## The corpus, measured

`CORPUS` holds the seeds kept because they found something. Seed 180 was the first that a
5,000-seed sweep found red under the patch for milestone 633 (an outside agent attacks the
confinement claim). That defect needs a delegation left staged by a plain `RECEIVE`, then a later
plain `SEND` collected by `RECEIVE_CAP` (2026-10-04, UTC). Seed 559 is the first a 5,000-seed sweep
found red under the revocation-in-flight patch, at step 14.

Under the committed seeds, all six patches were replayed under QEMU on aarch64, riscv64 and x86_64,
and each turned the driver red. Four went red at seed 0 (steps 17 and 18). Milestone 633's went red
at seed 19, step 63, inside the suite's range. Revocation in flight went red only at seed 559.

## The BUGS, in full

- `SYS_CAP_DELETE` is driven through `sched::delete_current_cap`, not `syscall::dispatch`.
  `TrapFrame` has no setter for the syscall number, so a kernel thread cannot build a frame
  `dispatch` would read as `SYS_CAP_DELETE`. `dispatch`'s arm is that one call and `Ok(0)`.
- Not generated: `REAP`, `SURVEY`, `MemoryRegion`, `AddressSpace`, `ThreadControlBlock`,
  `Timer`, `Irq`, `PageFrame::MAP` and `SLICE`, a bound notification, and death messages. No actor
  holds a TCB capability, so `BIND` reaches only its refusals. Each is a model extension, not a
  rewrite. A timer needs the model to reason about time, which is why it was left.
- Unexpected wakes are seen late. An actor the model says is parked is checked for a stray
  answer at every step and at teardown. A wrong wake is caught, but possibly some steps after the
  operation that caused it.
- The corpus is tied to the kernel's walk, not only to this generator. A change to `pick_op` or
  `endow` re-rolls every seed. So does a kernel change that alters any answer the model predicts.
  A corpus seed can then rot green with its defect still reachable, and the corpus must be re-found
  with the roadmap block's replay table. A new finding's seed goes into `CORPUS` with its reason.
