# The job mix supervisor: its shape and its limits

An appendix to [notes/job-mix.md](../job-mix.md). It carries the reasons behind
`kernel/src/job_mix.rs`, the supervisor that builds the pool, releases a subrun's worth of tasks,
owns the wall clock and prints. `crates/job_mix` defines the workload, and
`fixtures/src/job_mix_task.rs` is one task. The number feeds §96 (process kernel or event kernel,
and how to decide it), whose live input is performance under multitasking load, which
micro-benchmarks cannot see.

## Why it is shaped like the soak

It is shaped like `kernel/src/soak.rs`, not `kernel/src/bench.rs`, because the number has to be
taken on radon and the bench boot has never run on a board. So it has `--features job_mix`, the tour
ends in it, its console lines are machine-readable, and `script/job-mix` is the QEMU rehearsal. Unlike the
soak it ends, printing `job_mix::DONE` and halting. It parks rather than exits: the watcher sees the
marker and tears the run down, and a forgotten QEMU costs nothing in `wfi`.

## The pool, and the release

The pool of `job_mix::MAX_TASKS` processes is built once and the first N are released per subrun.
Spawning per subrun would measure the region allocator's fragmentation. So parked tasks' kernel
stacks exist throughout, which is right for §96's question and unlike AIM7's own subruns. Each
task's untyped budget and child-done endpoint are built with the pool for the same reason: a
subrun times the jobs, not the supervisor's set-up. `job_mix::TASK_BUDGET_PAGES` says why a task
never shares its region.

The go-ahead goes out one synchronous send at a time, a skew under a part in a thousand. A shared
flag would have parked tasks spinning, measuring the pool size instead of N.

The echo servers are spawned before the tasks. A mixer whose server is not yet receiving parks on
the endpoint, so the order is not load-bearing for correctness; it keeps a fresh boot's first
subrun from timing a server's spawn.

Each task's grants are the least each use needs, in the slot order `crates/job_mix` defines for
both sides. A task may write its report and may not read other tasks' reports. It may read its own
go endpoint and may not release anybody, itself included. It may call a server and may not answer
as one. It may spend its own region and nobody else's, and it hears only from its own children.
Each task's seed is distinct, odd and well spread, so no two walk the mix in the same order, which
is the property `job_mix::order` exists for.

The per-kind lines are totals over every task and repeat at a point, beside the per-job figure, so
a reader can re-derive the division.

## BUGS

- A single boot's jobs-per-minute is not a result. On radon the rate spans fifteenfold with the
  boot-time placement lottery, so the census prints with every run and the procedure asks for
  repeated boots.
- Each point is the median of `job_mix::REPEATS` (the constant has the evidence), with the fastest
  and slowest on the same line and every repeat on its own `job-mix-repeat:` line.
- The per-subrun breakdown exchange happens after the clock stops but warms the report path between
  repeats, identically for every point.
- It cannot compare against an event kernel, which does not exist; §96 stays open either way.
- The supervisor is one more thread, blocked in `RECEIVE` through every timed window.
- The clock includes draining the N task reports, a small cost that grows with N.
- Any message on the report endpoint counts as a report. A stray `CALL` there (answered `Gone` under
  §246 (a plain `RECEIVE` never takes a capability)) reads as zero jobs and wedges the sweep
  ([spawn-job-miscompiled-yield.md](spawn-job-miscompiled-yield.md)). Checking the badge and shape
  would name it.
