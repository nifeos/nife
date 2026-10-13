---
status: BUILT
raised: 2026-09-13
built: 2026-09-21
---
# 282. A thread's CPU time, and the `top` it makes possible

The decision is `design/decisions/0150-per-thread-cpu-accounting.md` (calef, 2026-09-13, ruling
the fork of milestone 126 (who else is running, and who is allowed to ask)), with DECISIONS §204
(how userspace asks where a thread runs).

Nothing was gated on calef here. §150 (how does a thread's CPU time reach userspace?) settles the
semantics, the sampling point and the unit, and §204 settles the wire shape: a selector inside the
window §150 left open for exactly that, not a widened `SURVEY` return. What remains his is the
program's name and whether the ranking view is a program at all; neither blocked the build, and the
naming note below says what he is being asked.

In brief. There was no per-thread CPU accounting anywhere in this kernel, dead or live. `Thread`
carried no time-on-CPU field, `sched::on_tick()` touched no per-thread state, and the only counter
was a machine-wide `preemptions()`. So this is new kernel state on the scheduler's hottest path, a
new record on an existing method, and then a program that reads it.

## What was built

1. A tick counter per thread slot, incremented in `sched::on_tick()` for whatever is running on
   that core: one bounds-checked index and one relaxed increment, which is §150's sub-choice 1.
2. `abi::survey::record::CPU_TIME`, the record a `SURVEY` selector asks for, answering
   milliseconds of scheduled on-CPU time. One constant, one name in `is_known`, one arm in
   `sched::survey_supervised`.
3. The consumers. `crates/ps` gained `Survey::join_cpu_time` (a second walk, joined on the tid)
   and `Survey::rank_by_cpu_time`, and `ps` prints a `TIME(ms)` column.
4. `top`: `components/src/top.rs` and `crates/top`, the same domain `ps` lists, ranked by CPU
   time, under a summary line a ranked view needs and a listing does not.

## The counter is an array, not a field, and that is the one departure from §150

§150's build note said *"a `u64` per `Thread`"*. It is a `[AtomicU64; MAX_THREADS]` beside the
thread table instead, and the reason is the sampling point the same decision chose.

The increment happens in interrupt context, where the only name a core has for its running thread is
a tid in its own per-CPU block. Turning that tid into a `&mut Thread` means taking `IPC_TABLES`, and
a timer interrupt that waits on a lock another core holds is a scheduler-latency hole opened at
every tick on every core. `try_lock` is worse rather than better: a dropped sample is a wrong
number rather than a coarse one, and it would be dropped exactly when the machine is busiest. The
slot index is already in the tid's low word, so an array keyed by slot needs no lock, no lookup and
no ordering beyond relaxed. Two kilobytes of `.bss`, fixed.

It is not on `cpu::PerCpu`, which milestone 527 (the `SURVEY` selector, and a thread's placement) made expensive (one `u64` there took
`size_of::<PerCpu>()` from 128 to 136 and cost riscv64's IPC fastpath 5.4%), and it was never a
candidate: the counter is per thread.

## §204's promise, measured

§204 replaced the widened return with a selector on the forecast that *"a mechanism that must be
redesigned at the sixth field is the wrong mechanism at the fourth"*, and the lane that built the
selector claimed a new fact would then cost about three lines of dispatch. It did. Adding
`CPU_TIME` to the kernel's wire surface was one constant in `abi`, one name in `is_known`, one arm
in the walk and one four-line conversion function. Everything else in this milestone is the
accounting itself and the programs that read it, neither of which the wire shape decides. The
promise held, on the first fact added after it was made.

## The name was earned rather than assumed

`top` means *the top N by resource consumption*; it is defined by ranking. A table of `TID` and
`STATE` alone has nothing to rank by, and the name would overclaim. Under §150's option 2 it is both
available and correct. Under the two refused options it would not have: wall-clock
age is not `%CPU`, and a sampled estimate is not a measurement.

Two things about the name are an architect's and neither blocks anything. The name `top` itself,
which ships provisional. And the prior question of whether this is a program at all: milestone 281
(`watch` holds exactly what `ps` holds) deleted `watch` on the rule that *two programs are two
programs when they hold different authority*, and `top` holds `ps`'s three slots exactly. What
differs is the question asked rather than the endowment, and the argument is written out both ways
in `crates/top`'s module docs. Folding it into `ps` as a flag is a day's work and stays available.

## BUGS

- The reader races the writer, by design. Each core's tick touches only its own running thread's
  slot, so the write needs no cross-core synchronization, but a reader on one core observing a
  counter another core is incrementing is a relaxed load of a value in flight. It reads a number
  that was true a moment ago, never a torn one. The same shape the per-CPU `TICKS` array already
  accepts, and rule 4 says state it rather than assume it.
- Tick-sampling is coarse and systematically so. A thread that runs entirely between two ticks
  is charged nothing, and one that happens to be on a CPU at every tick is charged for the whole of
  each. That is what Linux's `jiffies`-based `utime` also does, and it is the accepted cost of not
  touching `schedule()`'s hot path. Recorded here and on the record's own doc rather than discovered
  by whoever first compares two numbers that should have matched.
- **CPU time is an aggregate statistic and capabilities do not close that side channel.** A confined
  viewer holding `ENUMERATE` learns something **continuous** about threads it cannot otherwise name,
  where before it learned a state word: two reads measure how much work another thread did in
  between. §150 weighed that and accepted it, on the ground that `ENUMERATE` is already the right to
  learn what exists as distinct from acting on it, and the leak is bounded by the supervision
  subtree. §204 records that placement is strictly less than this. It is written where a reader
  meets the method (`abi::survey::record::CPU_TIME`, `components/src/top.rs`) and not only here.
- **`top` does not refresh, and there is no `%CPU` column.** Both are the same missing thing, which
  is a timed wait (milestone 106 (a wait that ends on either the interrupt or the deadline)). A spin-refresh would be charged to `top`'s own thread by the
  counter it ranks on, so `top` would truthfully report itself as the busiest thread on the machine,
  and a percentage needs two samples an interval apart. `crates/top`'s `BUGS` has the rest,
  including why a cumulative percentage against uptime was refused.
- **`top` cannot be asked for the top *N*.** `ArgSpec` is `Required` or `Forbidden` with nothing
  between, so an optional integer cannot be declared and a `top` that required one could not be
  typed bare. The same boundary limitation `crates/pgrep` records for its missing pattern.
- **A thread's own CPU time is not reachable without a survey of its supervisor's domain.** §204's
  second mechanism, the per-thread page, answers the self case for placement; nothing extends it to
  this figure, and a thread that wants to know what it has spent has no path to it today.

## Follow-on

- **None.** A refresh and a `%CPU` column are milestone 106's to unblock, not a milestone of their
  own; they are recorded in `crates/top`'s `BUGS` where a reader meets the program.

## Index row

The decision is §150 (how does a thread's CPU time reach userspace?), with §204 (how userspace asks where a thread
runs). Built a tick counter per thread slot incremented in `on_tick()`, exposed
as `abi::survey::record::CPU_TIME` in milliseconds, then a `TIME(ms)` column on `ps` and `top`
itself. The counter is an array beside the thread table rather than a field on `Thread`, because the
increment is in interrupt context and reaching a `Thread` there means `IPC_TABLES`. `top` is
*earned* by this rather than assumed: the name means ranking by resource use, and under the two
options §150 refused it would have overclaimed.
