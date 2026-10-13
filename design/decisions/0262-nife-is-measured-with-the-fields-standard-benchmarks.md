---
status: AMENDED
raised: 2026-10-08
decided: 2026-10-08
ratified_by: calef
---

# 262. nife is measured with the field's standard benchmarks, run unmodified, against Linux

## The ruling

The maintainer had filed eight proposals, one per standard benchmark, each naming the fatal risks it
would inform. calef, 2026-10-08 (UTC):

> Do we have milestones for all of these and have we tied them to their risks? Because this looks
> like our list of things to go after. We can compare against other OSs using standards with lots of
> weight behind them using methods used in many papers. That seems right. That seems like something
> we could optimize to as well, which is likely necessary.

## What it commits the project to

1. The benchmarks are the field's own. A benchmark qualifies when papers and vendors already report
   it, so a stranger can set nife's number beside one they trust. An in-tree program that measures
   the same idea is an instrument, not the comparison.
2. They run unmodified. The program's source is the upstream source at a pinned version. Where nife
   needs a layer under it (a C library, a timer), the layer is ours and the program is not touched.
   A row that needed a change says which change, in the row.
3. The method is the published one: the flags, sizes, repetition and reporting the papers use, so a
   reader can check the setup without trusting ours.
4. The peer is Linux on the same machine, booted in turn, and seL4 where a benchmark exists for it.
5. These numbers are targets. The project optimizes toward them, and a milestone that moves one is
   on the ranking function's path, since performance is part of what "runs it" means.

## The guard against benchmarketing

Point 5 carries its own risk. A project that optimizes to a benchmark can win the benchmark and
lose the workload. The guard is the one `AGENTS.md` already states under *Measuring, pushing back,
and correcting the record*, applied to every number these milestones produce:

- Each number says what it means and where it is not apples to apples, in the row, as
  `notes/benchmarks.md` does for the map tie and the spawn caveat.
- A tie or a loss is recorded as plainly as a win. A loss is a finding about where to work.
- No change whose only effect is on the benchmark. A tuning lands when it helps the workload the
  benchmark stands for, and the commit says which workload. Detecting the benchmark, special-casing
  its sizes, or matching its access pattern is refused.
- Both systems are configured the same way, and a setting that moves the number is recorded on both
  sides: Linux's governor, scheduler, offloads and polling mode, nife's driver mode and build.

A published benchmark number is a fact that leaves the machine. The decisions skill puts that on the
list of things to be methodical about, because a stranger may already have quoted it.

## The list, and the risks each informs

As of 2026-10-08 (UTC). Each fatal-risk file links back to these milestones under *Benchmarks that
inform this risk*. "C library" means the open question below.

| benchmark | milestone | risks | waits on |
|---|---|---|---|
| lmbench, a subset | 826 | 4 | C library; `fork` rows never run (§10) |
| hackbench | 827 | 4, 5 | C library; threads (812), or `fork` for its process mode |
| iperf3 | 828 | 4, 6 | C library; a NIC on silicon (494, 53); threads (812) from 3.16 |
| netperf TCP_RR | 829 | 4, 6 | C library; a NIC on silicon (494, 53); netserver forks by default |
| schbench | 830 | 5 | C library; threads (812) |
| SQLite speedtest1 | 831 | 1, 4 | C library |
| STREAM | 832 | 4 | C library; threads (812) for the per-core run only |
| fio | 833 | 1, 6 | C library; threads (812) |
| ioping | 834 | 1, 6 | C library |
| sel4bench | 25 | 4 | a PMU cycle counter on silicon; runs on seL4, not on nife |

`sel4bench` is folded in by reference. Milestone 25 (cross-OS performance comparison) already holds
it as its one outstanding item, and duplicating it would make two records of one fact.

Any of these running unmodified is also evidence for risk 1. The table names risk 1 only where a
proposal chose the benchmark for it.

## What this does not decide

The C library. Every benchmark above is POSIX C. §31 (the foreign-language seam) lets C make no
syscalls, and milestone 478 (tier three: full POSIX behind the foreign-language seam) is refused. Its
condition for revisiting is "a component somebody needs that cannot be adapted to the narrower
tiers". Point 2 forbids adapting the program, so this ruling may be that component. That is calef's
call, not this section's, and each of the nine milestones carries `decision_dependencies:
unwritten` until it is written up and ruled.

`fork` stays refused. §10 (the capability-based microkernel process model) has no `fork`, and
nothing here reopens it. A benchmark path that forks runs in its no-fork mode or is recorded as not
applicable, with the reason.

The in-tree CoreMark does not meet this standard. `crates/coremark` is a Rust reimplementation, and
its own header says the certified score needs the unmodified reference C. Milestone 558 (a CoreMark
score on three architectures) publishes a number a stranger will read as CoreMark, so it needs that
caveat until the reference C runs.

## What else was considered

- In-tree programs that measure the same thing. Milestone 25's EL0 programs and `crates/coremark`
  stay as instruments, and they are fast to gate on. A stranger cannot compare them with a published
  number without trusting our reading of the original, which is the gap this ruling closes.
- A suite harness such as the Phoronix Test Suite. From memory, not checked here: it runs many of
  these programs behind its own scripts, which would be one more layer to port and to explain.
  Porting the programs directly keeps the setup the papers describe.
- SPEC CPU. From memory: licensed per seat, and a compute benchmark, which STREAM and CoreMark
  already cover for a microkernel's purposes. Not refused, not listed.

## BUGS

- "Weight behind them" is judged, not measured. Nobody has counted citations per benchmark, and
  ioping in particular is an operator's tool that papers cite less than fio. It is listed because it
  moves risk 6 soonest, and its block says so.
- The selection is one agent's, from the eight proposals and calef's ioping lean. A benchmark the
  field weighs heavily may be missing; a new one is a proposal under `design/roadmap/proposals/`.
- Nothing gates point 3. A run that changes the published flags is caught only by review of its row.

## Amendment, 2026-10-12 (UTC): wakeup latency replaces schbench

calef ruled on #2009 ([his comment](https://github.com/nifeos/nife/pull/2009#issuecomment-6130684610)), choosing to measure wakeup latency from each kernel's own
trace: *"c, and drop schbench"*. He then ruled that this section be amended to match, on #2021:
*"Amend"*. Both are dated 2026-10-12 (UTC).

The table's row for milestone 830 now reads:

| benchmark | milestone | risks | waits on |
|---|---|---|---|
| wakeup latency (each kernel's own scheduler trace) | 830 | 5 | a timed scheduler trace |

This row is the one exception to point 2. It compares the same quantity, measured by each kernel's
own tracer, and not one program run unmodified on both systems. schbench is Linux-only: it calls
`syscall(SYS_futex)` and `syscall(SYS_gettid)`, which §265 (a C library started from relibc,
whose Rust platform layer holds the capabilities) does not build. The load is still one unmodified
program, hackbench, as milestone 830 (wakeup latency on nife and Linux, from each kernel's own
scheduler trace) records with its instruments.
