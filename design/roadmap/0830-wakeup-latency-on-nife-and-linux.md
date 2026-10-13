---
status: NOT-STARTED
raised: 2026-10-08
promoted_from: schbench-on-nife-and-linux
milestone_dependencies: 225, 827
decision_dependencies: 262
machine_requirements: silicon with 4 or more cores, radon first, then xenon, argon when the TX1 arrives
specific_machine: none
needs_person: yes
---
# 830. Wakeup latency on nife and Linux, from each kernel's own scheduler trace

*(The title and the slug `wakeup-latency-on-nife-and-linux` are provisional: names are calef's,
and he has not ruled on these.)*

**In brief.** Measure wakeup latency, the time from a thread being made runnable to that thread
running on a CPU, as a distribution (p50, p99, p99.9 and the maximum), on nife and on Linux. Each
side reads it from its own kernel's scheduler trace, under the same load from a generator that runs
unmodified on both.

## The ruling

Not a port of `schbench`, because schbench is Linux-only: it calls `syscall(SYS_futex)`
and `syscall(SYS_gettid)` directly, and §265 (a C library started from relibc, whose Rust platform
layer holds the capabilities) does not build `syscall()`. Of the options put to calef, he chose (c):
measure the quantity the way the field does across operating systems, from each kernel's own
scheduler trace. calef, 2026-10-12 (UTC):

> c, and drop schbench

So this is not a port. §262 (nife is measured with the field's standard benchmarks) prefers one
upstream program, run unmodified on both systems (its second commitment); here that holds for the load and not for the instrument. **The comparison
is the same quantity measured by each kernel's own tracer, not one binary on both sides.** Every row
this block writes says so. It is how wakeup latency is compared between operating systems in
practice: no single tool runs on all of them, and each kernel's scheduler already knows when it made
a thread runnable and when it ran it.

## What it measures

Per thread, the interval from a wakeup event (the thread made runnable) to the next running-on-CPU
event for that thread, collected over the whole run into a histogram, and reported as percentiles
and a maximum. A thread made runnable by being created counts too, as Linux's `sched_wakeup_new`
does. The tail is the point: a p99.9 that is 100x the median is a finding.

## The instruments

- **Linux**: `runqlat` (BCC or bpftrace, which keeps a histogram in the kernel) as the primary, and
  `perf sched record` with `perf sched latency` as the cross-check. Both read `sched_wakeup` (and
  `sched_wakeup_new`) then `sched_switch`. Record the kernel version, the scheduler (EEVDF or CFS),
  `HZ` and the preemption model, and run `runqlat` without `-P`'s requeue counting unless nife
  measures preemption requeues too.
- **nife**: its own scheduler's wake and switch-in events, timestamped, in a build made for this
  measurement. What exists today and what is missing are in the next section.
- For reference, from memory and not checked here: Windows takes the same quantity from ETW's
  ready-thread and context-switch events (ready time), and FreeBSD, illumos and macOS from DTrace's
  `sched` provider (`enqueue` then `on-cpu`). They are not part of this block.

Both tracers cost something, and the costs are not the same. Each row records the run's throughput
with the tracer on and off, so a reader can see how much each instrument moved the thing it
measured.

## What nife must supply

Read on `main` at 1b51b412e, 2026-10-12 (UTC). nife's scheduler has a per-core event ring,
`kernel/src/sched/trace.rs`, built for first-silicon wedge dumps:

- It records the two events this needs, per thread. `Event::Wake` (Blocked to Ready, onto this
  core's queue) is recorded in `sched::wake`. `Event::SwitchTo` (picked and marked Running) is
  recorded in `schedule()`. A wake deferred because the target was still on its old CPU is recorded
  as `Event::WakeDeferred` in `sched::wake` and completed as `Event::WakeCompleted` in
  `finish_switch`. Each entry carries the thread id's low 48 bits.
- It records no time. `trace::record` packs the kind, an aux byte and the tid into one `u64`.
- It holds the last 16 events per core (`trace::DEPTH`), so a run's history is gone within a few
  switches.
- It is compiled out of `--features bench` builds (the `bench` twin of `mod trace` in
  `kernel/src/sched.rs`), the builds `script/bench` measures, because its cost showed on the IPC
  fast path.
- The only reader is `trace::dump`, which `sched::dump_threads` prints to the serial console when a
  soak or a test fails. Nothing reads it from a running system.
- A newly spawned thread is made runnable through placement without a `Wake` event; `place_on`
  records `Event::PlaceRemote` only for a remote placement.

So the events are in the right places, and none of the rest is usable as it stands. The proposal,
which is this block's first piece of work:

1. A per-thread timestamp written at the wake verdict (`Wake` or `WakeDeferred`) and at placement
   of a new thread, read from the counter `Instant` already uses. At `SwitchTo`, the difference goes
   into a per-core log2 histogram and the timestamp is cleared. This is what `runqlat` does in the
   Linux kernel (a start time keyed by thread, a histogram at switch), so the two sides compute the
   same thing the same way. It costs one counter read and one store at a wake, and a counter read, a
   subtraction and an increment at a switch.
2. Behind its own feature, neither the soak's nor `bench`'s, so `script/bench`'s baselines never
   carry it, and the tracer-on and tracer-off runs are two builds of one commit.
3. Read out over the serial console at the end of a run, as the soak already reports. A method that
   reads the histogram from a running system would touch the syscall surface, which is calef's
   call; it is not needed for a first number.
4. The counter must agree across cores, since a wake and its switch often happen on different ones.
   aarch64's `CNTVCT_EL0` is one system counter. riscv64's `time` CSR and x86_64's TSC (invariant
   only on newer parts) are to be confirmed on radon and xenon before their rows are written.

Reuse: the Linux side takes `runqlat` and `perf sched` as they ship. nife's side is kernel code,
which this project writes (§46 (thin primitives or whole subsystems; we write everything in
between), its 2026-10-04 amendment). It extends the existing `sched::trace` ring and takes
`runqlat`'s method (start time per thread, log2 histogram at switch) rather than its code.

## The load

`hackbench`, milestone 827 (hackbench on nife and Linux, as a scheduler benchmark and a multicore
soak), unmodified, in its thread mode. It runs on nife in #2009, for milestone 836 (a C library,
stage 2: threads), not yet merged. Same version, same groups, loops and message size on both
sides; same board, same core count, Linux booted in turn. A second load with idle gaps, so wakeups
land on idle cores and not only on busy ones, is worth adding once the first row exists. It must
also run unmodified on both.

## Which fatal risk it informs

Risk 5 (`5-not-reliable-on-multicore.md`, it cannot be made reliable on multicore, and the bugs
appear only on silicon). A wakeup that arrives very late, or never, is the visible edge of a
lost-wakeup or run-queue defect. A thread that never runs is a failure the soak (milestone 225 (run
the soak on radon, argon and xenon, which is the only place its answer means anything)) would
also catch. Milestone 792 (cross-core wake latency under HVF) saw millisecond cross-core wakes
under HVF; this block's instrument would measure that directly.

## How it runs continuously

QEMU TCG wall-clock timings are meaningless, so nothing here gates on them. The histogram code is
host-testable logic and gets host tests. The runs happen on silicon on a schedule, as the runbook
does for the soak, and write dated rows to `notes/benchmarks.md` with the Linux row beside them and
the instrument named in each. A run that cannot reach the board writes nothing rather than a TCG
number.

## What it waits on

Milestone 827 for the load, and #2009 for hackbench's thread mode on nife. The trace events are this
block's own work. It no longer waits on threads in C or on `syscall()`.

## Index row

Wakeup-latency tails on the boards, nife against Linux, each read from its own kernel's scheduler
trace under the same unmodified hackbench load. nife's ring has the events but no timestamps.
