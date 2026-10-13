---
status: NOT-STARTED
raised: 2026-10-08
promoted_from: hackbench-on-nife-and-linux
milestone_dependencies: 225, 812, 835, 809, 801, 907
decision_dependencies: 262, 265
machine_requirements: silicon with 4 or more cores, radon first, then xenon, argon when the TX1 arrives
specific_machine: none
needs_person: yes
---
# 827. hackbench on nife and Linux, as a scheduler benchmark and a multicore soak

*(The title and slug are provisional.)*

Model: milestone 833 (the same storage benchmark on nife and Linux, by porting real fio).

**In brief.** Port `hackbench` (rt-tests; about 700 lines of C) and run it on the boards, once as a
benchmark and again as a long soak. Groups of sender and receiver tasks pass small messages over
sockets or pipes, so every message is a cross-task, often cross-core, handoff.

## What it measures

Wall time for G groups of 20 senders and 20 receivers to exchange N messages. It stresses the
scheduler, wakeups, IPC and cross-core migration together, with no useful compute.

## Which fatal risks it informs

Risk 4 (the per-crossing cost): total time over message count is a cost per handoff that is
comparable across OSes. Risk 5 (`5-not-reliable-on-multicore.md`): the load is thousands of tasks
with constant cross-core wakeups, which is the shape that exposes lost wakeups and races. Run for
hours it is a second soak workload beside milestone 225 (run the soak on radon, argon and xenon).
A hang or corrupted message count is a defect found, not a number.

## What nife must supply, as questions

- Processes or threads? hackbench has both modes. Which maps onto nife's spawn cost and capability
  grants, and does a group of 40 fit the capability-slot budget?
- Sockets or pipes? nife has neither as a Unix object. Is the port over IPC endpoints (a changed
  program) or over a pipe layer (a changed kernel path being measured)? Record which, and why.
- Is `fork` needed? If so the port has to substitute `spawn`, which Linux then must be asked to
  match, or the comparison is not like for like (see the spawn caveat in `notes/benchmarks.md`).

## The Linux comparison

The same hackbench revision, pinned by rt-tests tag, same group count, loops, message size and
mode, same board, with the Linux kernel version and scheduler (EEVDF or CFS) recorded. Three runs,
median and spread; core count fixed with `--smp` or `taskset` identically on both.

Reuse: hackbench is taken, ported and not rewritten.

## What it waits on

The C library is §265 (a C library started from relibc), and its first stage, milestone 835 (a C
library, stage 1: files, clock and memory), is built. This program's other needs are below.

Threads or fork. hackbench's thread mode needs threads, and its process mode forks. Threads are
milestone 812 (`std::thread::spawn` runs real threads in one address space), option A of §105
(`std::thread::spawn` stays declined, until a customer needs it), reopened 2026-10-07 and not yet
scheduled. nife has no `fork` by design (§10 (process model: capability-based, microkernel)), so a
path that needs it never unblocks. The port runs the program's no-fork mode where one exists, and a
row that cannot run is recorded with its reason, as `notes/benchmarks.md` already does for spawn.

## How it runs continuously

QEMU TCG wall-clock timings are meaningless, so nothing here gates on them. Where a host-side
harness allows, CI gates on deterministic instruction counts through `script/bench` (a pinned
icount baseline per workload step, `--check` failing on drift). The wall-clock runs happen on
silicon on a schedule, as the runbook does for the soak, and write dated rows to
`notes/benchmarks.md` with the Linux row beside them. A run that cannot reach the board writes
nothing rather than a TCG number.

## Done when

*§297 (software reaches a nife system as a package `jig` installs from basalt's index, the kernel
included) binds this. calef, 2026-10-12 (UTC): "They should all be installed via jig through
basalt."*

hackbench must run on nife installed by `jig install` from basalt's package index. basalt builds the
package against the nife commit it pins. The install waits on milestone 809 (the package client
becomes a program, `jig`, with the verbs an index needs). It also waits on milestone 801 (packages
over the internet: a host name, HTTPS and the distribution's index). The package itself is milestone
907 (hackbench is packaged in basalt, and its test installs it with `jig`). No work toward that
starts until those milestones are built (calef, 2026-10-12 UTC: "For our other programs let's block
start on packaging.").

## Index row

hackbench on the boards, as a cost per cross-task handoff and as a second multicore soak. Waits on a
C library and on threads (milestone 812), since its other mode forks.
