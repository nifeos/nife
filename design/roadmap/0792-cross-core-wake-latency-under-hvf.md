---
status: NOT-STARTED
promoted_from: cross-core-wake-latency-under-hvf
raised: 2026-10-05
milestone_dependencies: none
decision_dependencies: none
machine_requirements: aarch64 silicon with Hypervisor.framework; riscv64 silicon
specific_machine: none
needs_person: no
---
# 792. Cross-core wake latency under HVF

Raised 2026-10-05 (UTC) by `lane/null-syscall-hvf`. A measurement, not a fix. Title and slug are
drafts.

## What was seen

In [the HVF appendix](../../notes/job-mix/null-syscall-hvf-full-mix.md) the job mix's spawn job
(two children, built, run and reclaimed one at a time) cost, per job, in 24 MHz ticks:

| Other cores | spawn per job |
|---|---|
| idle (one task) | 59,000 (2.5 ms) |
| busy in syscall-free user code (condition C, two and three tasks) | 108,000 to 167,000 (4.5 to 7 ms) |
| busy in the full mix (four tasks) | 2,700 (0.11 ms) |

Radon's one-task figure is 1,172 ticks of a 10 MHz clock, 117 us
([radon's evening](../../notes/job-mix/radon-2026-10-04.md)). So under HVF a child placed on
another core waits about a millisecond when that core is idle, and several when it is running user
code without trapping.

## What the tree does, read 2026-10-05

`sched::place_on` pushes a remote thread into the target's inbox and returns the core that owes a
reschedule SGI. aarch64's `RESCHED_SGI` handler calls `drain_inbox`, which sets `need_resched`, and
`preempt_if_needed` at the bottom of the interrupt path runs `schedule()`. So the kernel does not
wait for
the 100 Hz tick (`arch::aarch64::timer::TICK_HZ`). The lane's first guess that it did was wrong.
That makes the delay most likely the delivery of the SGI: under QEMU's HVF the GICv3 is emulated in
userspace, and a vCPU in `wfi` or in a guest loop has to be kicked out of `hv_vcpu_run`.

## The questions

1. From `place_on` to the child's first instruction, how long under HVF, TCG and on radon, with the
   target idle and with it busy in user code? The trace ring's `PlaceRemote` and `switch` events
   carry what is needed.
2. If HVF alone is slow, is it the idle wake (`wfi`), the kick of a running vCPU, or both?
3. Does any number this tree quotes from HVF (the job mix's one-task points, the soak under `--hvf`)
   carry this delay without saying so?

Reuse: the kernel's trace ring (`PlaceRemote` and `switch` events) and `script/job-mix` are
the instruments; nothing new is built unless they cannot answer question 1.

## Done means

The three answers with numbers in a note. A kernel defect found on the way becomes its own proposal.
If the cost is QEMU's, a `BUGS` entry beside `script/job-mix --hvf` says which HVF figures carry it.

## Index row

Under HVF a child placed on another core waits about a millisecond when that core is idle and several when it runs user code without trapping, which is the largest term in the spawn job. This is a measurement, not a fix: the block narrows the cause to SGI delivery and asks for the experiment that confirms it.
